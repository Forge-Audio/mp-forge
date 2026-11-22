//! A trial of a Sample app: it is played for a few seconds with nobody at it, to see that it stands up to it
//! and what its screen then shows.
//!
//! An app's code runs on this computer here, so a trial is held in a program of its own, as everything an app
//! does is. An app that crashes or hangs takes that program down and nothing else.
//!
//! The app is tried with the sound files the musician added to it, and with a small SD card that is the same
//! every time, so that an app which looks through a card has one to look through.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use resvg::tiny_skia::Pixmap;

use crate::app_sounds::sounds_of;
use crate::process::{RunError, command_for, run_with_time_limit};
use crate::sample_app::{BUTTONS, Input, PADS, RunningApp, SCREEN_HEIGHT, SCREEN_WIDTH, Shown, library_of};
use crate::sd_card::{SdCard, write_trial_card};

pub const TRY_SAMPLE_APP_COMMAND: &str = "try-sample-app";
/// Beside the built app: what is to be done to it in the trial, and the picture its screen ended on.
const TOUCHES_FILE: &str = "touches.json";
const PICTURE_FILE: &str = "screen.png";
const TRIAL_CARD_FOLDER: &str = "trial-card";

const FRAMES_A_SECOND: u32 = 44_100;
/// The device asks an app for its sound this many frames at a time.
const FRAMES_A_STRETCH: usize = 128;
const STRETCHES_A_PICTURE: usize = 11;
const STRETCHES_BETWEEN_TOUCHES: usize = 17;
const STRETCHES_AFTER_THE_LAST_TOUCH: usize = 172;
const KNOBS: i32 = 3;
const PLACES_A_KNOB_IS_PUT: [f32; 3] = [0.0, 1.0, 0.5];
const PADS_HIT_TOGETHER: [i32; 4] = [1, 6, 11, 16];
const CLICKS_THE_DIAL_IS_TURNED: i32 = 3;
const THE_DIAL_PRESSED: &str = "ENCODER";
/// What is played on a keyboard plugged in: a low note, middle C and a high one, the wheel most keyboards have
/// and the pedal that holds notes, and the pitch wheel to both ends and back to rest.
const NOTES_PLAYED: [u8; 3] = [36, 60, 96];
const CONTROLLERS_MOVED: [u8; 2] = [1, 64];
const PLACES_A_CONTROLLER_IS_PUT: [u8; 3] = [127, 0, 64];
const PLACES_THE_PITCH_WHEEL_IS_PUT: [f32; 3] = [1.0, -1.0, 0.0];
const MIDI_CHANNEL: u8 = 1;
const HARDEST_NOTE: u8 = 127;
/// A steady note comes in at the inputs, so that an app which works on sound coming in has some to work on.
const NOTE_COMING_IN: f64 = 220.0;
const LOUDNESS_COMING_IN: f64 = 0.5;

const LOUDEST_THE_DEVICE_PLAYS: f32 = 1.0;
/// The share of the time there is for a stretch of sound that an app may spend making it on this computer.
/// The device's processor is several times slower, so an app has to leave most of the time unused here.
const SHARE_OF_THE_TIME_THAT_IS_TOO_MUCH: f64 = 0.25;
const SHARE_OF_THE_TIME_WORTH_A_WARNING: f64 = 0.06;
const TIMES_BIGGER_THE_PICTURE_IS_SHOWN: usize = 3;
const BYTES_A_DOT: usize = 4;

// ---------------------------------------------------------------------------
// The trial itself, in the program that holds it
// ---------------------------------------------------------------------------

/// What was measured while an app was played.
#[derive(Debug, Default, Clone, PartialEq)]
struct Measured {
    loudest: f32,
    values_that_are_no_numbers: u64,
    seconds_spent_on_sound: f64,
    seconds_of_sound: f64,
}

impl Measured {
    fn hear(&mut self, sound: &[f32]) {
        self.values_that_are_no_numbers += sound.iter().filter(|value| !value.is_finite()).count() as u64;
        self.loudest = sound.iter().filter(|value| value.is_finite()).fold(self.loudest, |loudest, value| loudest.max(value.abs()));
    }

    fn written(&self) -> String {
        let share_of_the_time_used = if self.seconds_of_sound > 0.0 { self.seconds_spent_on_sound / self.seconds_of_sound } else { 0.0 };
        json!({ "loudest": self.loudest, "valuesThatAreNoNumbers": self.values_that_are_no_numbers, "shareOfTheTimeUsed": share_of_the_time_used }).to_string()
    }
}

/// Plays the app built in `app_dir` as `touches_file` says, or through everything on the device when it says
/// nothing, with the musician's sound files for it to play. Writes the picture its screen ends on into
/// `picture_file`, and prints what was measured for `held_by` to read.
pub fn hold_and_print(app_dir: &Path, touches_file: &Path, picture_file: &Path) -> Result<(), String> {
    let touches = touches_in(touches_file)?;
    let library = library_of(app_dir);
    let card = write_trial_card(&library.with_file_name(TRIAL_CARD_FOLDER)).map_err(|error| format!("The card the app is tried with could not be made: {error}"))?;
    let mut app = RunningApp::load(&library, f64::from(FRAMES_A_SECOND), sounds_of(app_dir, FRAMES_A_SECOND))?;
    let measured = play(&mut app, &card, &touches);
    fs::write(picture_file, png_of(&app.show())?).map_err(|error| format!("The picture of the screen could not be kept: {error}"))?;
    println!("{}", measured.written());
    Ok(())
}

fn touches_in(touches_file: &Path) -> Result<Vec<Input>, String> {
    let written = fs::read_to_string(touches_file).unwrap_or_default();
    let asked: Vec<Value> = serde_json::from_str(&written).unwrap_or_default();
    if asked.is_empty() {
        return Ok(everything_on_the_device());
    }
    asked.iter().map(|touch| Input::said_by_the_screen(touch).ok_or_else(|| format!("{touch} is not something the device can do."))).collect()
}

/// Every control of the device used in turn: the knobs and the fader to both ends, every pad hit hard and let
/// go, a handful of pads at once, a keyboard plugged in and played, every button, and the dial both ways. The
/// dial is pressed once more at the end, so that a file browser a button opened, and the first press of the
/// dial took into a folder, loads a file.
fn everything_on_the_device() -> Vec<Input> {
    let all_pads = 1..=PADS as i32;
    let knobs = (1..=KNOBS).flat_map(|knob| PLACES_A_KNOB_IS_PUT.map(|stands| Input::knob_turned(knob, stands)));
    let fader = PLACES_A_KNOB_IS_PUT.map(Input::fader_moved);
    let pads = all_pads.flat_map(|pad| [Input::pad_hit(pad, 1.0), Input::pad_let_go(pad)]);
    let together = PADS_HIT_TOGETHER.map(|pad| Input::pad_hit(pad, 1.0)).into_iter().chain(PADS_HIT_TOGETHER.map(Input::pad_let_go));
    let buttons = BUTTONS.iter().flat_map(|button| [Input::button(button, true), Input::button(button, false)]).flatten();
    let dial = [Input::encoder_turned(CLICKS_THE_DIAL_IS_TURNED), Input::encoder_turned(-CLICKS_THE_DIAL_IS_TURNED)];
    let dial_pressed_again = [Input::button(THE_DIAL_PRESSED, true), Input::button(THE_DIAL_PRESSED, false)].into_iter().flatten();
    knobs.chain(fader).chain(pads).chain(together).chain(a_keyboard_played()).chain(buttons).chain(dial).chain(dial_pressed_again).collect()
}

fn a_keyboard_played() -> impl Iterator<Item = Input> {
    let notes = NOTES_PLAYED.into_iter().flat_map(|note| [Input::note_on(note, HARDEST_NOTE, MIDI_CHANNEL), Input::note_off(note, MIDI_CHANNEL)]);
    let together = NOTES_PLAYED.map(|note| Input::note_on(note, HARDEST_NOTE, MIDI_CHANNEL)).into_iter().chain(NOTES_PLAYED.map(|note| Input::note_off(note, MIDI_CHANNEL)));
    let controllers = CONTROLLERS_MOVED.into_iter().flat_map(|controller| PLACES_A_CONTROLLER_IS_PUT.map(|stands| Input::controller_moved(controller, stands, MIDI_CHANNEL)));
    let pitch_wheel = PLACES_THE_PITCH_WHEEL_IS_PUT.map(|bend| Input::pitch_bent(bend, MIDI_CHANNEL));
    notes.chain(together).chain(controllers).chain(pitch_wheel)
}

fn play(app: &mut RunningApp, card: &SdCard, touches: &[Input]) -> Measured {
    let mut measured = Measured::default();
    let (mut left, mut right) = ([0.0f32; FRAMES_A_STRETCH], [0.0f32; FRAMES_A_STRETCH]);
    let stretches = touches.len() * STRETCHES_BETWEEN_TOUCHES + STRETCHES_AFTER_THE_LAST_TOUCH;
    for stretch in 0..stretches {
        if stretch % STRETCHES_BETWEEN_TOUCHES == 0 && let Some(touch) = touches.get(stretch / STRETCHES_BETWEEN_TOUCHES) {
            app.tell(*touch);
        }
        let coming_in = sound_coming_in(stretch);
        let started = Instant::now();
        app.make_sound(&coming_in, &coming_in, &mut left, &mut right);
        measured.seconds_spent_on_sound += started.elapsed().as_secs_f64();
        measured.hear(&left);
        measured.hear(&right);
        if let Some(asked) = app.asked_of_the_card() {
            app.take_answer(card.answer(asked, FRAMES_A_SECOND));
        }
        if stretch % STRETCHES_A_PICTURE == 0 {
            app.show();
        }
    }
    measured.seconds_of_sound = (stretches * FRAMES_A_STRETCH) as f64 / f64::from(FRAMES_A_SECOND);
    measured
}

fn sound_coming_in(stretch: usize) -> [f32; FRAMES_A_STRETCH] {
    let frames_before = stretch * FRAMES_A_STRETCH;
    std::array::from_fn(|frame| {
        let seconds = (frames_before + frame) as f64 / f64::from(FRAMES_A_SECOND);
        ((std::f64::consts::TAU * NOTE_COMING_IN * seconds).sin() * LOUDNESS_COMING_IN) as f32
    })
}

/// The app's screen as a picture file, with every dot drawn several times over so that small letters can be read.
fn png_of(shown: &Shown) -> Result<Vec<u8>, String> {
    let times = TIMES_BIGGER_THE_PICTURE_IS_SHOWN;
    let could_not_be_made = || "The picture of the screen could not be made.".to_string();
    let mut picture = Pixmap::new((SCREEN_WIDTH * times) as u32, (SCREEN_HEIGHT * times) as u32).ok_or_else(could_not_be_made)?;
    for (place, dot) in picture.data_mut().chunks_exact_mut(BYTES_A_DOT).enumerate() {
        let (across, down) = (place % (SCREEN_WIDTH * times) / times, place / (SCREEN_WIDTH * times) / times);
        let from = (down * SCREEN_WIDTH + across) * BYTES_A_DOT;
        dot.copy_from_slice(&shown.picture[from..from + BYTES_A_DOT]);
    }
    picture.encode_png().map_err(|_| could_not_be_made())
}

// ---------------------------------------------------------------------------
// A trial, as whoever asked for it sees it
// ---------------------------------------------------------------------------

/// How an app stood up to being played, and the picture its screen ended on.
#[derive(Debug, Clone, PartialEq)]
pub struct Trial {
    pub loudest: f32,
    pub values_that_are_no_numbers: u64,
    pub share_of_the_time_used: f64,
    pub picture_as_png: Vec<u8>,
}

impl Trial {
    fn read_from(line: &str, picture_as_png: Vec<u8>) -> Option<Trial> {
        let said: Value = serde_json::from_str(line).ok()?;
        Some(Trial {
            loudest: said["loudest"].as_f64()? as f32,
            values_that_are_no_numbers: said["valuesThatAreNoNumbers"].as_u64()?,
            share_of_the_time_used: said["shareOfTheTimeUsed"].as_f64()?,
            picture_as_png,
        })
    }

    /// What has to be put right before the app is fit for the device, in words the AI can act on.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.values_that_are_no_numbers > 0 {
            problems.push(format!(
                "The sound held {} values that are not numbers. Keep whatever feeds a division, a filter or a square root from reaching a value it cannot take.",
                self.values_that_are_no_numbers
            ));
        }
        if self.loudest > LOUDEST_THE_DEVICE_PLAYS {
            problems.push(format!(
                "The sound reached {:.2}, and the device plays nothing above 1: it would be cut off harshly. Turn the mix down, or hold it under 1 gently.",
                self.loudest
            ));
        }
        if self.share_of_the_time_used > SHARE_OF_THE_TIME_THAT_IS_TOO_MUCH {
            problems.push(format!("Making the sound took {} of the time there is for it on this computer. {THE_DEVICE_IS_SLOWER}", self.time_used_in_words()));
        }
        problems
    }

    pub fn warnings(&self) -> Vec<String> {
        let is_heavy = self.share_of_the_time_used > SHARE_OF_THE_TIME_WORTH_A_WARNING && self.share_of_the_time_used <= SHARE_OF_THE_TIME_THAT_IS_TOO_MUCH;
        if is_heavy { vec![format!("Making the sound takes {} of the time there is for it on this computer. {THE_DEVICE_IS_SLOWER}", self.time_used_in_words())] } else { Vec::new() }
    }

    fn time_used_in_words(&self) -> String {
        format!("{:.0}%", self.share_of_the_time_used * 100.0)
    }
}

const THE_DEVICE_IS_SLOWER: &str = "The MPC Sample's processor is several times slower, so make the sound with less work: fewer voices, cheaper maths in the inner loop.";

/// Has `program` hold a trial of the app built in `app_dir`, in a process of its own. `touches` is what is done
/// to it, in the words the screen uses, and an empty list has everything on the device used in turn.
pub fn held_by(program: &Path, app_dir: &Path, touches: &Value, limit: Duration) -> Result<Trial, String> {
    let library = library_of(app_dir);
    let beside_the_app = |file: &str| library.with_file_name(file);
    fs::write(beside_the_app(TOUCHES_FILE), touches.to_string()).map_err(|error| format!("The app could not be tried: {error}"))?;
    let _ = fs::remove_file(beside_the_app(PICTURE_FILE));
    let mut command = command_for(program);
    command.arg(TRY_SAMPLE_APP_COMMAND).arg(app_dir).arg(beside_the_app(TOUCHES_FILE)).arg(beside_the_app(PICTURE_FILE));
    match run_with_time_limit(&mut command, limit) {
        Ok(finished) => {
            let picture = fs::read(beside_the_app(PICTURE_FILE)).unwrap_or_default();
            finished.output.lines().rev().find_map(|line| Trial::read_from(line, picture.clone())).ok_or_else(|| stopped_while_played(&finished.output))
        }
        Err(RunError::TimedOut { .. }) => Err("The app stopped answering while it was being played. Something in it waits, or never comes back.".to_string()),
        Err(RunError::CouldNotStart(error)) => Err(format!("The app could not be tried: {error}")),
    }
}

/// The picture the app's screen ended on in its last trial, when that trial got as far as its end.
pub fn picture_of_the_last_trial(app_dir: &Path) -> Option<Vec<u8>> {
    fs::read(library_of(app_dir).with_file_name(PICTURE_FILE)).ok()
}

fn stopped_while_played(said: &str) -> String {
    format!("The app stopped while it was being played: {}", said.lines().last().unwrap_or("it crashed without a word"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trial_that_went_well() -> Trial {
        Trial { loudest: 0.4, values_that_are_no_numbers: 0, share_of_the_time_used: 0.01, picture_as_png: Vec::new() }
    }

    #[test]
    fn an_app_that_plays_cleanly_and_lightly_has_nothing_against_it() {
        let trial = trial_that_went_well();

        assert!(trial.problems().is_empty() && trial.warnings().is_empty());
    }

    #[test]
    fn sound_that_is_no_number_or_too_loud_or_too_much_work_is_a_problem() {
        let no_numbers = Trial { values_that_are_no_numbers: 12, ..trial_that_went_well() };
        let too_loud = Trial { loudest: 1.8, ..trial_that_went_well() };
        let too_heavy = Trial { share_of_the_time_used: 0.4, ..trial_that_went_well() };

        assert!(no_numbers.problems()[0].contains("12 values that are not numbers"));
        assert!(too_loud.problems()[0].contains("reached 1.80"));
        assert!(too_heavy.problems()[0].contains("40% of the time"));
    }

    #[test]
    fn an_app_that_is_heavy_but_not_too_heavy_is_only_warned() {
        let heavy = Trial { share_of_the_time_used: 0.1, ..trial_that_went_well() };

        assert!(heavy.problems().is_empty());
        assert!(heavy.warnings()[0].contains("10% of the time"));
    }

    #[test]
    fn everything_on_the_device_is_used_in_a_trial_nobody_gave_touches_for() {
        let touches = everything_on_the_device();

        let every_pad_is_hit = (1..=PADS as i32).all(|pad| touches.contains(&Input::pad_hit(pad, 1.0)));
        let every_button_is_pressed = BUTTONS.iter().all(|button| touches.contains(&Input::button(button, true).unwrap()));

        assert!(every_pad_is_hit && every_button_is_pressed);
        assert!(touches.contains(&Input::knob_turned(3, 1.0)) && touches.contains(&Input::fader_moved(0.0)) && touches.contains(&Input::encoder_turned(-3)));
        assert!(touches.contains(&Input::note_on(60, 127, 1)) && touches.contains(&Input::note_off(60, 1)), "a keyboard is played");
        assert!(touches.contains(&Input::controller_moved(64, 127, 1)) && touches.contains(&Input::pitch_bent(-1.0, 1)));
    }

    #[test]
    fn what_was_measured_is_read_back_as_it_was_written() {
        let measured = Measured { loudest: 0.5, values_that_are_no_numbers: 2, seconds_spent_on_sound: 0.5, seconds_of_sound: 2.0 };

        let trial = Trial::read_from(&measured.written(), vec![1, 2, 3]).unwrap();

        assert_eq!(trial, Trial { loudest: 0.5, values_that_are_no_numbers: 2, share_of_the_time_used: 0.25, picture_as_png: vec![1, 2, 3] });
        assert_eq!(Trial::read_from("the app said something of its own", Vec::new()), None);
    }
}
