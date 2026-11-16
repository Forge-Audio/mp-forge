//! A two-bar beat made from scratch: kick, snare, hats and a plucked bass line.
//!
//! An effect needs sound to work on. This gives the live player something with low, middle and high
//! in it, at the tempo the player tells the plugin, so tempo-synced effects land on the beat.

use std::f32::consts::TAU;

use crate::hosted_plugin::TEMPO;

const STEPS: usize = 32;
const STEPS_IN_A_BEAT: f64 = 4.0;
const LOUDEST: f32 = 0.7;
const KICK_STEPS: [usize; 8] = [0, 6, 8, 14, 16, 22, 24, 27];
const SNARE_STEPS: [usize; 4] = [4, 12, 20, 28];
/// Each bass note: the step it starts on and its pitch in hertz.
const BASS_NOTES: [(usize, f32); 8] = [(0, 55.0), (3, 55.0), (6, 65.4), (10, 49.0), (16, 55.0), (19, 55.0), (22, 73.4), (26, 65.4)];

/// One drum or note: how long it sounds, how loud on each side, and its wave at a moment after it starts.
struct Hit {
    seconds: f32,
    level: [f32; 2],
    wave: fn(f32, f32, &mut Hiss) -> f32,
}

const KICK: Hit = Hit { seconds: 0.30, level: [0.9, 0.9], wave: kick };
const SNARE: Hit = Hit { seconds: 0.20, level: [0.5, 0.5], wave: snare };
const HAT: Hit = Hit { seconds: 0.05, level: [0.10, 0.16], wave: hat };
const BASS: Hit = Hit { seconds: 0.35, level: [0.30, 0.30], wave: pluck };

/// A repeatable source of hiss, so the loop is the same every time it is made.
struct Hiss(u32);

impl Hiss {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / 8_388_608.0 - 1.0
    }
}

/// The loop as stereo frames at this sample rate. It joins up when played round and round.
pub fn beat_loop(sample_rate: u32) -> Vec<[f32; 2]> {
    let seconds = STEPS as f64 / STEPS_IN_A_BEAT * 60.0 / TEMPO;
    let length = (f64::from(sample_rate) * seconds).round() as usize;
    let mut track = Track { frames: vec![[0.0; 2]; length], sample_rate: sample_rate as f32, hiss: Hiss(7) };
    KICK_STEPS.iter().for_each(|step| track.add(&KICK, *step, 0.0));
    SNARE_STEPS.iter().for_each(|step| track.add(&SNARE, *step, 0.0));
    (0..STEPS).step_by(2).for_each(|step| track.add(&HAT, step, 0.0));
    BASS_NOTES.iter().for_each(|(step, pitch)| track.add(&BASS, *step, *pitch));
    track.no_louder_than(LOUDEST)
}

struct Track {
    frames: Vec<[f32; 2]>,
    sample_rate: f32,
    hiss: Hiss,
}

impl Track {
    /// Mixes one hit in at a step. A hit that runs past the end carries on from the start, as it would in a loop.
    fn add(&mut self, hit: &Hit, step: usize, pitch: f32) {
        let start = step * self.frames.len() / STEPS;
        let length = (hit.seconds * self.sample_rate) as usize;
        for offset in 0..length {
            let sample = (hit.wave)(offset as f32 / self.sample_rate, pitch, &mut self.hiss);
            let frame = (start + offset) % self.frames.len();
            self.frames[frame][0] += sample * hit.level[0];
            self.frames[frame][1] += sample * hit.level[1];
        }
    }

    fn no_louder_than(mut self, loudest: f32) -> Vec<[f32; 2]> {
        let peak = self.frames.iter().flatten().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        let gain = if peak > 0.0 { loudest / peak } else { 1.0 };
        self.frames.iter_mut().flatten().for_each(|sample| *sample *= gain);
        self.frames
    }
}

/// A low thump whose pitch drops quickly.
fn kick(seconds: f32, _pitch: f32, _hiss: &mut Hiss) -> f32 {
    let turns = 50.0 * seconds + (110.0 / 30.0) * (1.0 - (-30.0 * seconds).exp());
    (TAU * turns).sin() * (-8.0 * seconds).exp()
}

/// A burst of hiss over a short tone.
fn snare(seconds: f32, _pitch: f32, hiss: &mut Hiss) -> f32 {
    hiss.next() * 0.6 * (-22.0 * seconds).exp() + (TAU * 190.0 * seconds).sin() * 0.5 * (-30.0 * seconds).exp()
}

/// A very short, thin tick: the difference of two hiss values keeps only the high end.
fn hat(seconds: f32, _pitch: f32, hiss: &mut Hiss) -> f32 {
    (hiss.next() - hiss.next()) * (-70.0 * seconds).exp()
}

/// A note with a few overtones that dies away, like a plucked string.
fn pluck(seconds: f32, pitch: f32, _hiss: &mut Hiss) -> f32 {
    let overtones: f32 = (1..=4).map(|overtone| (TAU * pitch * overtone as f32 * seconds).sin() / overtone as f32).sum();
    overtones * (-6.0 * seconds).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_loop_is_two_bars_long_and_safe_to_play() {
        let beat = beat_loop(44_100);

        let peak = beat.iter().flatten().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert_eq!(beat.len(), 4 * 44_100);
        assert!(beat.iter().flatten().all(|sample| sample.is_finite()));
        assert!((peak - LOUDEST).abs() < 0.001, "{peak}");
    }

    #[test]
    fn the_loop_opens_on_a_kick_and_is_the_same_every_time() {
        let beat = beat_loop(48_000);

        let loudest_at_the_start = beat[..2_400].iter().flatten().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert!(loudest_at_the_start > 0.3, "{loudest_at_the_start}");
        assert_eq!(beat, beat_loop(48_000));
    }
}
