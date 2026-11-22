//! This computer's sound input, standing in for the inputs of the MPC Sample while an app runs here.
//!
//! The input is opened only while the app listens at its inputs, which it does once it or the musician has
//! said where on the device to listen, and closed again when it does not. What
//! comes in waits for a moment and is handed to the app a stretch at a time, at the rate the app runs at. It is
//! passed to the app and nowhere else.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, SupportedStreamConfig};
// For `f32::from_sample`. It is brought in without its name, which the kinds of sample below go by.
use cpal::Sample as _;

const SILENCE: [f32; 2] = [0.0; 2];
/// The input and the speakers each run to a clock of their own and take sound in stretches of their own
/// length. This much is kept waiting between them so that neither runs the other dry.
const SECONDS_KEPT_AHEAD: f64 = 0.03;
/// More than this much waiting is sound the app would hear late, so the oldest of it is let go.
const SECONDS_OF_SLACK: f64 = 0.1;

// ---------------------------------------------------------------------------
// Sound that has come in and waits to be handed to the app
// ---------------------------------------------------------------------------

/// Sound that has come in and has not been handed on yet. Nothing is handed on until enough has gathered, so a
/// stretch that is asked for is whole or it is silence, and never breaks off part of the way.
#[derive(Debug)]
pub struct WaitingSound {
    frames: VecDeque<[f32; 2]>,
    kept_ahead: usize,
    slack: usize,
    longest_stretch_asked_for: usize,
    is_gathering: bool,
}

impl WaitingSound {
    pub fn at(frames_a_second: u32) -> Self {
        let frames_in = |seconds: f64| (seconds * f64::from(frames_a_second)) as usize;
        Self { frames: VecDeque::new(), kept_ahead: frames_in(SECONDS_KEPT_AHEAD), slack: frames_in(SECONDS_OF_SLACK), longest_stretch_asked_for: 0, is_gathering: true }
    }

    fn take_in(&mut self, frame: [f32; 2]) {
        if self.frames.len() >= self.enough_to_begin() + self.slack {
            self.frames.pop_front();
        }
        self.frames.push_back(frame);
    }

    /// Fills a stretch for the app with what has come in, and with silence when too little has.
    pub fn hand_on(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.longest_stretch_asked_for = self.longest_stretch_asked_for.max(left.len());
        self.is_gathering = self.is_gathering && self.frames.len() < self.enough_to_begin();
        for (left, right) in left.iter_mut().zip(right) {
            let frame = if self.is_gathering { None } else { self.frames.pop_front() };
            self.is_gathering = frame.is_none();
            [*left, *right] = frame.unwrap_or(SILENCE);
        }
    }

    fn forget(&mut self) {
        self.frames.clear();
        self.is_gathering = true;
    }

    fn enough_to_begin(&self) -> usize {
        self.kept_ahead + self.longest_stretch_asked_for
    }
}

/// What waits is shared by the input, which puts sound in, and the speakers, which hand it to the app.
pub type SharedWaitingSound = Arc<Mutex<WaitingSound>>;

pub fn held(waiting: &Mutex<WaitingSound>) -> MutexGuard<'_, WaitingSound> {
    waiting.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// ---------------------------------------------------------------------------
// From the rate sound comes in at to the rate the app runs at
// ---------------------------------------------------------------------------

/// Brings sound to another rate as it arrives. Each frame handed on is read off the line between the two
/// frames that came in either side of it.
struct AtAnotherRate {
    frames_in_for_each_out: f64,
    place_after_the_last_frame: f64,
    last_frame: [f32; 2],
}

impl AtAnotherRate {
    fn from(comes_in_at: u32, wanted_at: u32) -> Self {
        Self { frames_in_for_each_out: f64::from(comes_in_at) / f64::from(wanted_at.max(1)), place_after_the_last_frame: 0.0, last_frame: SILENCE }
    }

    /// Hands on every frame that falls between the last frame that came in and this one.
    fn carry(&mut self, came_in: [f32; 2], mut hand_on: impl FnMut([f32; 2])) {
        while self.place_after_the_last_frame < 1.0 {
            let along = self.place_after_the_last_frame as f32;
            let between = |side: usize| self.last_frame[side] + (came_in[side] - self.last_frame[side]) * along;
            hand_on([between(0), between(1)]);
            self.place_after_the_last_frame += self.frames_in_for_each_out;
        }
        self.place_after_the_last_frame -= 1.0;
        self.last_frame = came_in;
    }
}

// ---------------------------------------------------------------------------
// The computer's input
// ---------------------------------------------------------------------------

/// The computer's usual sound input and the format it gives.
struct ComputersInput {
    device: cpal::Device,
    format: SupportedStreamConfig,
}

impl ComputersInput {
    fn of_this_computer() -> Option<Self> {
        let device = cpal::default_host().default_input_device()?;
        let format = device.default_input_config().ok()?;
        Some(Self { device, format })
    }

    /// Puts whatever comes in into `waiting`, at `frames_a_second`, for as long as the stream is kept.
    fn heard_into(&self, waiting: SharedWaitingSound, frames_a_second: u32) -> Option<Stream> {
        let at_the_apps_rate = AtAnotherRate::from(self.format.sample_rate(), frames_a_second);
        let stream = match self.format.sample_format() {
            SampleFormat::F32 => self.stream_of::<f32>(waiting, at_the_apps_rate),
            SampleFormat::I16 => self.stream_of::<i16>(waiting, at_the_apps_rate),
            SampleFormat::U16 => self.stream_of::<u16>(waiting, at_the_apps_rate),
            _ => None,
        }?;
        stream.play().ok()?;
        Some(stream)
    }

    fn stream_of<Sample: SizedSample>(&self, waiting: SharedWaitingSound, mut at_the_apps_rate: AtAnotherRate) -> Option<Stream>
    where
        f32: FromSample<Sample>,
    {
        let channels = usize::from(self.format.channels()).max(1);
        let take = move |came_in: &[Sample], _: &cpal::InputCallbackInfo| {
            let mut waiting = held(&waiting);
            for frame in came_in.chunks_exact(channels) {
                at_the_apps_rate.carry(left_and_right_of(frame), |frame| waiting.take_in(frame));
            }
        };
        self.device.build_input_stream(self.format.config(), take, |_| {}, None).ok()
    }
}

/// The first two channels are left and right. A single channel, as a microphone gives, is heard on both sides.
fn left_and_right_of<Sample: SizedSample>(frame: &[Sample]) -> [f32; 2]
where
    f32: FromSample<Sample>,
{
    let side = |side: usize| f32::from_sample(frame[side.min(frame.len() - 1)]);
    [side(0), side(1)]
}

// ---------------------------------------------------------------------------
// Listening, or not
// ---------------------------------------------------------------------------

/// The computer's input, open for as long as the app listens at its inputs and closed when it does not. It
/// stays where it was made, since an open input cannot be handed from one thread to another.
pub struct Listener {
    waiting: SharedWaitingSound,
    frames_a_second: u32,
    is_listening: bool,
    open_input: Option<Stream>,
}

impl Listener {
    pub fn putting_sound_into(waiting: SharedWaitingSound, frames_a_second: u32) -> Self {
        Self { waiting, frames_a_second, is_listening: false, open_input: None }
    }

    /// Opens the input when the app starts to listen and closes it when the app stops. Closing it leaves
    /// nothing waiting, so the app hears silence at once. A computer with no input is not asked again until
    /// the app next starts to listen.
    pub fn follow(&mut self, app_listens: bool) {
        if app_listens == self.is_listening {
            return;
        }
        self.is_listening = app_listens;
        self.open_input = None;
        held(&self.waiting).forget();
        if app_listens {
            self.open_input = ComputersInput::of_this_computer().and_then(|input| input.heard_into(Arc::clone(&self.waiting), self.frames_a_second));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 1_000;
    const KEPT_AHEAD: usize = 30;
    const SLACK: usize = 100;

    fn handed_on(waiting: &mut WaitingSound, frames: usize) -> Vec<f32> {
        let (mut left, mut right) = (vec![9.0; frames], vec![9.0; frames]);
        waiting.hand_on(&mut left, &mut right);
        assert_eq!(left, right.iter().map(|right| -right).collect::<Vec<f32>>());
        left
    }

    fn take_in_counting_from(waiting: &mut WaitingSound, first: usize, frames: usize) {
        (first..first + frames).for_each(|number| waiting.take_in([number as f32, -(number as f32)]));
    }

    #[test]
    fn with_nothing_come_in_the_app_is_handed_silence() {
        let mut waiting = WaitingSound::at(RATE);

        assert_eq!(handed_on(&mut waiting, 4), [0.0; 4]);
    }

    #[test]
    fn nothing_is_handed_on_until_enough_has_gathered_and_then_it_comes_in_order() {
        let mut waiting = WaitingSound::at(RATE);
        const STRETCH: usize = 10;

        take_in_counting_from(&mut waiting, 1, KEPT_AHEAD + STRETCH - 1);
        let too_soon = handed_on(&mut waiting, STRETCH);
        take_in_counting_from(&mut waiting, KEPT_AHEAD + STRETCH, 1);
        let once_enough_has_gathered = handed_on(&mut waiting, STRETCH);

        assert_eq!(too_soon, [0.0; STRETCH]);
        assert_eq!(once_enough_has_gathered, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    }

    #[test]
    fn once_it_runs_dry_it_gathers_again_before_handing_any_more_on() {
        let mut waiting = WaitingSound::at(RATE);
        const STRETCH: usize = 40;

        take_in_counting_from(&mut waiting, 1, KEPT_AHEAD + STRETCH);
        let first = handed_on(&mut waiting, STRETCH);
        let run_dry = handed_on(&mut waiting, STRETCH);
        take_in_counting_from(&mut waiting, 500, 5);
        let still_gathering = handed_on(&mut waiting, STRETCH);

        assert_eq!((first[0], first[STRETCH - 1]), (1.0, STRETCH as f32));
        assert_eq!(run_dry[..KEPT_AHEAD], (STRETCH + 1..=STRETCH + KEPT_AHEAD).map(|number| number as f32).collect::<Vec<f32>>());
        assert_eq!(run_dry[KEPT_AHEAD..], [0.0; STRETCH - KEPT_AHEAD]);
        assert_eq!(still_gathering, [0.0; STRETCH]);
    }

    #[test]
    fn sound_that_would_be_heard_too_late_is_let_go_oldest_first() {
        let mut waiting = WaitingSound::at(RATE);
        const FAR_TOO_MANY: usize = 1_000;

        take_in_counting_from(&mut waiting, 1, FAR_TOO_MANY);
        let heard = handed_on(&mut waiting, 1);

        assert_eq!(heard, [(FAR_TOO_MANY - (KEPT_AHEAD + SLACK) + 1) as f32]);
    }

    #[test]
    fn forgetting_leaves_silence_at_once() {
        let mut waiting = WaitingSound::at(RATE);
        take_in_counting_from(&mut waiting, 1, 500);

        waiting.forget();

        assert_eq!(handed_on(&mut waiting, 3), [0.0; 3]);
    }

    fn carried(at_another_rate: &mut AtAnotherRate, came_in: &[f32]) -> Vec<f32> {
        let mut out = Vec::new();
        came_in.iter().for_each(|number| at_another_rate.carry([*number, -number], |frame| out.push(frame[0])));
        out
    }

    #[test]
    fn sound_at_the_apps_own_rate_is_handed_on_as_it_came() {
        let mut same_rate = AtAnotherRate::from(48_000, 48_000);

        assert_eq!(carried(&mut same_rate, &[1.0, 2.0, 3.0]), [0.0, 1.0, 2.0]);
    }

    #[test]
    fn sound_that_comes_in_slower_is_filled_in_between_its_frames() {
        let mut twice_as_fast = AtAnotherRate::from(24_000, 48_000);

        assert_eq!(carried(&mut twice_as_fast, &[1.0, 2.0, 3.0]), [0.0, 0.5, 1.0, 1.5, 2.0, 2.5]);
    }

    #[test]
    fn sound_that_comes_in_faster_has_frames_left_out_however_it_is_cut_up() {
        let mut half_as_fast = AtAnotherRate::from(48_000, 24_000);
        let mut in_two_goes = AtAnotherRate::from(48_000, 24_000);

        let at_once = carried(&mut half_as_fast, &[1.0, 2.0, 3.0, 4.0, 5.0]);
        let mut in_pieces = carried(&mut in_two_goes, &[1.0, 2.0]);
        in_pieces.extend(carried(&mut in_two_goes, &[3.0, 4.0, 5.0]));

        assert_eq!(at_once, [0.0, 2.0, 4.0]);
        assert_eq!(in_pieces, at_once);
    }

    #[test]
    fn one_channel_is_heard_on_both_sides_and_only_the_first_two_of_many_are_heard() {
        assert_eq!(left_and_right_of(&[0.5f32]), [0.5, 0.5]);
        assert_eq!(left_and_right_of(&[0.25f32, -0.25, 0.9, 0.9]), [0.25, -0.25]);
        assert_eq!(left_and_right_of(&[i16::MAX, i16::MIN]), [f32::from_sample(i16::MAX), -1.0]);
    }
}
