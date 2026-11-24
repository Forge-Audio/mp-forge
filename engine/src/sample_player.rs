//! The player for a Sample app: the app run in a program of its own, heard through the computer's speakers, with
//! what it shows written out for the drawing of the MPC Sample.
//!
//! It is started by the app, so a Sample app that crashes or freezes stops the player and nothing else. The app
//! writes one line on standard input for each thing the musician touches, as JSON. On standard output the player
//! writes what the Sample app shows, about thirty times a second and only when it has changed: always the same
//! number of bytes, the picture first, then the lights, and last whether the app waits for the musician to
//! pick a sound file on the computer.
//!
//! Whatever on the device the musician would set, they set in the app, on the pages of MP Forge's kit, and the
//! computer follows:
//!
//! - The sound files the musician added to the app are read here once, before the app starts, and handed to it.
//!   A computer has no SD card, so when the app opens the kit's page for picking a file from the card, the
//!   musician picks one on the computer, and it is read here and handed over the same way.
//! - At its inputs the app hears silence until it listens at one of the device's: its jacks, its microphone or
//!   USB. This computer's sound input stands in for whichever that is.
//! - MIDI keyboards and controllers plugged into this computer play the app, standing in for both of the
//!   device's MIDI sockets. The kit passes on what the app is set to be played by.
//! - A real MPC Sample on its USB-C cable, running MP Forge's own program, is the app's screen, lights and
//!   controls as well as the drawing. Nobody switches that on: it is used while it is there.

use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::app_sounds::sounds_of;
use crate::device_link::DeviceLink;
use crate::live_player::{Speakers, private_copy_of};
use crate::midi_in;
use crate::sample_app::{BUTTONS, Input, LEVEL_STRIPS, PADS, RunningApp, SCREEN_HEIGHT, SCREEN_WIDTH, Shown, SoundIn, library_of};
use crate::sd_card::{self, SdCard};
use crate::sound_in::{self, Listener, SharedWaitingSound, WaitingSound};

/// The workbench command that starts the player for the Sample app built in a folder.
pub const PLAY_SAMPLE_APP_COMMAND: &str = "play-sample-app";
/// How many bytes say what an app shows: its picture, the colour of each pad, which buttons are lit, how many
/// lights of each strip beside the screen are lit, and one more that is 1 while the app waits for the musician
/// to pick a sound file on the computer.
pub const BYTES_SHOWN: usize = SCREEN_WIDTH * SCREEN_HEIGHT * 4 + PADS * 3 + BUTTONS.len() + LEVEL_STRIPS + 1;

const RATE_WITHOUT_SPEAKERS: u32 = 44_100;
/// About as often as a screen is drawn again.
const SHOWS_EVERY: Duration = Duration::from_millis(33);

/// What the musician did about a file the app waits for: the one they picked on the computer, or none when
/// they decided against it.
type Picked = Option<PathBuf>;

/// Plays the app built in `app_dir` until the app that started the player closes its end.
pub fn play_on_standard_io(app_dir: &Path) -> Result<(), String> {
    let speakers = Speakers::of_this_computer();
    let frames_a_second = speakers.as_ref().map_or(RATE_WITHOUT_SPEAKERS, Speakers::sample_rate);
    let app = Arc::new(Mutex::new(loaded_from(app_dir, frames_a_second)?));
    let coming_in: SharedWaitingSound = Arc::new(Mutex::new(WaitingSound::at(frames_a_second)));
    let _heard_while_this_is_kept = speakers.map(|speakers| speakers.play(sound_of(Arc::clone(&app), Arc::clone(&coming_in)))).transpose()?;
    midi_in::keep_listening(played_into(Arc::clone(&app)));
    let device = DeviceLink::to_the_mpc_sample(touched_on(Arc::clone(&app)));
    let (pick, picks) = mpsc::channel();
    thread::spawn({
        let app = Arc::clone(&app);
        move || Showing { app: &app, picks, frames_a_second, device }.keep_on(Listener::putting_sound_into(coming_in, frames_a_second))
    });
    for line in io::stdin().lock().lines().map_while(Result::ok) {
        match what_was_said(&line) {
            Some(Said::Touched(input)) => held(&app).tell(input),
            Some(Said::Picked(picked)) => drop(pick.send(picked)),
            None => {}
        }
    }
    Ok(())
}

/// What a MIDI message that arrives is handed to: the app is told of it, when it is one an app is told of.
fn played_into(app: Arc<Mutex<RunningApp>>) -> impl Fn(&[u8]) + Send + Sync + 'static {
    move |message: &[u8]| {
        if let Some(played) = Input::midi(message) {
            held(&app).tell(played);
        }
    }
}

/// What everything touched on a real device is handed to: the app is told of it.
fn touched_on(app: Arc<Mutex<RunningApp>>) -> impl Fn(Input) + Send + Sync + 'static {
    move |touched: Input| held(&app).tell(touched)
}

/// What a line from the screen says: something the musician touched on the device, or what they did about a
/// sound file the app waits for.
#[derive(Debug, PartialEq)]
enum Said {
    Touched(Input),
    Picked(Picked),
}

fn what_was_said(line: &str) -> Option<Said> {
    let said = serde_json::from_str::<Value>(line).ok()?;
    let picked = said.get("picked").map(|picked| Said::Picked(picked.as_str().map(PathBuf::from)));
    picked.or_else(|| Input::said_by_the_screen(&said).map(Said::Touched))
}

/// The kit promises an app that it is never called from two places at once, so every call takes this lock.
fn held(app: &Mutex<RunningApp>) -> MutexGuard<'_, RunningApp> {
    app.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The app is loaded from a copy of its own, so a new build never changes the file under a running player.
fn loaded_from(app_dir: &Path, frames_a_second: u32) -> Result<RunningApp, String> {
    let copy = private_copy_of(&library_of(app_dir)).map_err(|error| format!("The built app could not be read: {error}"))?;
    let loaded = RunningApp::load(&copy, f64::from(frames_a_second), sounds_of(app_dir, frames_a_second));
    let _ = fs::remove_file(&copy);
    loaded
}

/// What the speakers play: the app's sound, a stretch at a time. At its inputs the app is handed whatever has
/// come in at the computer's own, which is silence while nobody listens there.
fn sound_of(app: Arc<Mutex<RunningApp>>, coming_in: SharedWaitingSound) -> impl FnMut(&mut [[f32; 2]]) + Send + 'static {
    let (mut in_left, mut in_right, mut left, mut right) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    move |heard: &mut [[f32; 2]]| {
        for stretch in [&mut in_left, &mut in_right, &mut left, &mut right] {
            stretch.resize(heard.len(), 0.0f32);
        }
        sound_in::held(&coming_in).hand_on(&mut in_left, &mut in_right);
        held(&app).make_sound(&in_left, &in_right, &mut left, &mut right);
        for (frame, (left, right)) in heard.iter_mut().zip(left.iter().zip(&right)) {
            *frame = [fit_to_hear(*left), fit_to_hear(*right)];
        }
    }
}

/// Sound that is not a number, or louder than full, is never passed to the speakers as it is.
fn fit_to_hear(made: f32) -> f32 {
    if made.is_finite() { made.clamp(-1.0, 1.0) } else { 0.0 }
}

/// The showing of a running app, and the following of what it has set between one showing and the next.
struct Showing<'a> {
    app: &'a Mutex<RunningApp>,
    picks: Receiver<Picked>,
    frames_a_second: u32,
    device: DeviceLink,
}

impl Showing<'_> {
    /// Shows what the app shows for as long as somebody reads it. The app is left free to go on making its
    /// sound while a file is read for it.
    fn keep_on(&self, mut listener: Listener) {
        let mut last_written = None;
        loop {
            thread::sleep(SHOWS_EVERY);
            self.answer_what_was_asked_of_the_card();
            self.hand_over_what_was_picked();
            listener.follow(held(self.app).sound_in() != SoundIn::Off);
            let (shown, waits_for_a_file) = shown_by(&mut held(self.app));
            self.device.show(&shown);
            let written = as_bytes(&shown, waits_for_a_file);
            if last_written.as_ref() == Some(&written) {
                continue;
            }
            if write_out(&written).is_err() {
                return;
            }
            last_written = Some(written);
        }
    }

    fn answer_what_was_asked_of_the_card(&self) {
        let asked = held(self.app).asked_of_the_card();
        if let Some(answer) = asked.map(|asked| SdCard::picked_on_the_computer().answer(asked, self.frames_a_second)) {
            held(self.app).take_answer(answer);
        }
    }

    /// A file picked once the kit's page has been closed again is let go: the app no longer waits for one.
    fn hand_over_what_was_picked(&self) {
        let Ok(picked) = self.picks.try_recv() else { return };
        let Some(into_sample) = held(self.app).browsing_into() else { return };
        match picked {
            Some(file) => {
                let answer = sd_card::picked_on_the_computer(&file, into_sample, self.frames_a_second);
                held(self.app).take_answer(answer);
            }
            None => held(self.app).close_browser(),
        }
    }
}

/// Writes one showing out whole, for whoever started this program to read.
pub(crate) fn write_out(bytes: &[u8]) -> io::Result<()> {
    let mut out = io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

/// What the app shows, and whether it waits for the musician to pick a sound file.
fn shown_by(app: &mut RunningApp) -> (Shown, bool) {
    (app.show(), app.browsing_into().is_some())
}

/// What an app shows, as the bytes the drawing reads: the picture, three bytes for each pad's colour, a byte
/// for each button that is 1 when it is lit, a byte for each strip beside the screen that says how many of its
/// lights are lit, and last a byte that is 1 while the app waits for a file.
pub fn as_bytes(shown: &Shown, waits_for_a_file: bool) -> Vec<u8> {
    let lit = BUTTONS.iter().map(|button| u8::from(shown.lit_buttons.contains(button)));
    let lights = shown.pad_colours.iter().flatten().copied().chain(lit).chain(shown.level_lights);
    shown.picture.iter().copied().chain(lights).chain([u8::from(waits_for_a_file)]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_an_app_shows_is_written_as_the_picture_then_the_lights_then_whether_it_waits_for_a_file() {
        let mut shown = Shown { picture: vec![7; SCREEN_WIDTH * SCREEN_HEIGHT * 4], pad_colours: [[0; 3]; PADS], lit_buttons: vec!["PLAY"], level_lights: [2, 5] };
        shown.pad_colours[15] = [1, 2, 3];

        let bytes = as_bytes(&shown, true);

        let lights = &bytes[SCREEN_WIDTH * SCREEN_HEIGHT * 4..BYTES_SHOWN - 1];
        assert_eq!(bytes.len(), BYTES_SHOWN);
        assert_eq!(lights[15 * 3..16 * 3], [1, 2, 3]);
        assert_eq!(lights[PADS * 3..].iter().position(|is_lit| *is_lit == 1), BUTTONS.iter().position(|button| *button == "PLAY"));
        assert_eq!(bytes[BYTES_SHOWN - 1 - LEVEL_STRIPS..BYTES_SHOWN - 1], [2, 5], "the strips beside the screen come after the buttons");
        assert_eq!((bytes[BYTES_SHOWN - 1], as_bytes(&shown, false)[BYTES_SHOWN - 1]), (1, 0));
    }

    #[test]
    fn sound_that_would_hurt_is_never_passed_on_as_it_is() {
        assert_eq!([fit_to_hear(0.5), fit_to_hear(3.0), fit_to_hear(-3.0), fit_to_hear(f32::NAN), fit_to_hear(f32::INFINITY)], [0.5, 1.0, -1.0, 0.0, 0.0]);
    }

    #[test]
    fn a_line_that_is_not_something_touched_is_ignored() {
        assert_eq!(what_was_said(r#"{"control":"fader","value":0.5}"#), Some(Said::Touched(Input::fader_moved(0.5))));
        assert_eq!(what_was_said("not json"), None);
        assert_eq!(what_was_said(r#"{"control":"spaceship","value":1}"#), None);
    }

    #[test]
    fn a_line_says_which_file_the_musician_picked_or_that_they_picked_none() {
        assert_eq!(what_was_said(r#"{"picked":"/music/kick.wav"}"#), Some(Said::Picked(Some(PathBuf::from("/music/kick.wav")))));
        assert_eq!(what_was_said(r#"{"picked":null}"#), Some(Said::Picked(None)));
    }
}
