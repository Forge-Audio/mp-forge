//! The clock that moves a plugin's pictures on a real MPC.
//!
//! An MPC draws a picture again only when one of the plugin's controls changes. A picture written to play on
//! its own stood still on an MPC Key 37, twice, while pictures that follow a control kept up. So a plugin
//! with moving pictures keeps a control that is on no page and turns it round and round itself, telling the
//! MPC each time, and the pictures follow that control.
//!
//! This module knows the control's name, watches a built plugin to see whether the control really turns, and
//! keeps how long a turn takes, so the preview plays the pictures at the speed the MPC will.
//!
//! A picture may also follow some other control that is on no page, which the plugin moves when it has
//! reason to: lamps that light for the notes held, say. That one is watched too, but it is not a clock. It
//! may stand still while nothing is played, and its pictures are only played round and round in the preview
//! when it was seen to go round and round by itself.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};

use crate::hosted_plugin::HostedPlugin;
use crate::online::STAYS_OFFLINE;
use crate::pictures::{Motion, Picture};
use crate::process::{RunError, command_for, run_with_time_limit};
use crate::screen_layout::{ScreenLayout, SlotContent};

pub const CLOCK_CONTROL: &str = "Picture Clock";
pub const READ_CLOCK_COMMAND: &str = "read-clock";
/// The turn MP Forge's guide to pictures gives the clock. Used until a plugin's own clock has been watched.
pub const USUAL_TURN_SECONDS: f64 = 2.0;

const TURNS_KEPT_IN: &str = "build/picture_clocks.json";
const SAMPLE_RATE: u32 = 44_100;
const FRAMES_A_LOOK: usize = 64;
const WATCHED_FOR_SECONDS: f64 = 13.0;
const LONGEST_TURN_SECONDS: f64 = 6.0;
/// An MPC Key 37 showed 30 steps a second smoothly, and steps of 45, 60 and more looked no smoother.
/// The little room above 30 is for a clock whose steps do not divide its turn evenly.
const MOST_STEPS_A_SECOND: f64 = 32.0;
const FEWEST_STEPS_A_TURN: usize = 4;
const FALLS_BACK_TO_THE_START: f32 = 0.5;

/// What was seen of one control while the plugin ran with nobody touching it.
#[derive(Debug, Clone, PartialEq)]
pub struct Watched {
    /// How long it takes to go from its lowest to its highest and back, when it was seen to do so twice.
    pub turn_seconds: Option<f64>,
    pub steps_a_turn: usize,
    pub times_it_moved: usize,
    pub times_it_told_the_mpc: u64,
}

impl Watched {
    fn written(&self) -> String {
        json!({ "turnSeconds": self.turn_seconds, "stepsATurn": self.steps_a_turn, "timesItMoved": self.times_it_moved, "timesItToldTheMpc": self.times_it_told_the_mpc }).to_string()
    }

    fn read_from(line: &str) -> Option<Self> {
        let written: Value = serde_json::from_str(line).ok()?;
        let count = |field: &str| written[field].as_u64();
        Some(Self {
            turn_seconds: written["turnSeconds"].as_f64(),
            steps_a_turn: usize::try_from(count("stepsATurn")?).ok()?,
            times_it_moved: usize::try_from(count("timesItMoved")?).ok()?,
            times_it_told_the_mpc: count("timesItToldTheMpc")?,
        })
    }
}

/// The control that moves a picture with no knob of its own: the clock for a picture that plays on its own,
/// and whatever a picture follows that is on no page, which only the plugin itself can be moving.
pub fn control_the_plugin_turns_for(picture: &Picture, layout: &ScreenLayout) -> Option<String> {
    match &picture.motion {
        Motion::Still => None,
        Motion::Loops => Some(CLOCK_CONTROL.to_string()),
        Motion::Follows(control) => Some(control.clone()).filter(|control| !is_on_a_page(control, layout)),
    }
}

/// Every such control of a plugin, each named once.
pub fn controls_the_plugin_turns(pictures: &[Picture], layout: &ScreenLayout) -> Vec<String> {
    let mut controls: Vec<String> = pictures.iter().filter_map(|picture| control_the_plugin_turns_for(picture, layout)).collect();
    controls.sort();
    controls.dedup_by(|one, other| one.eq_ignore_ascii_case(other));
    controls
}

pub fn is_the_clock(control: &str) -> bool {
    control.eq_ignore_ascii_case(CLOCK_CONTROL)
}

pub fn is_on_a_page(control: &str, layout: &ScreenLayout) -> bool {
    let shows_it = |content: &SlotContent| matches!(content, SlotContent::Control(name) if name.eq_ignore_ascii_case(control));
    layout.pages.iter().any(|page| page.slots().iter().any(shows_it))
}

/// Runs the built plugin in silence and watches one of its controls. This runs the plugin's own code, so it
/// is only ever called from a program started for the purpose: see `watched_by`.
pub fn watch(library: &Path, control: usize) -> Result<Watched, String> {
    let mut plugin = HostedPlugin::load(library, SAMPLE_RATE)?;
    plugin.listen_for_moves_of(control);
    let silence = [[0.0_f32; 2]; FRAMES_A_LOOK];
    let mut heard = [[0.0_f32; 2]; FRAMES_A_LOOK];
    let seconds_a_look = FRAMES_A_LOOK as f64 / f64::from(SAMPLE_RATE);
    let looks = (WATCHED_FOR_SECONDS / seconds_a_look) as usize;
    let mut amounts = Vec::with_capacity(looks + 1);
    amounts.push(plugin.control_amount(control));
    for _ in 0..looks {
        plugin.render(&silence, &mut heard);
        amounts.push(plugin.control_amount(control));
    }
    Ok(seen_in(&amounts, seconds_a_look, plugin.times_told_it_moved()))
}

/// What a row of looks at the control, taken at even steps of time, shows of how it turns.
fn seen_in(amounts: &[f32], seconds_a_look: f64, times_it_told_the_mpc: u64) -> Watched {
    let looks_where = |changed: &dyn Fn(f32, f32) -> bool| -> Vec<usize> {
        amounts.windows(2).enumerate().filter(|(_, pair)| changed(pair[0], pair[1])).map(|(look, _)| look + 1).collect()
    };
    let moves = looks_where(&|before, after| after != before);
    let falls = looks_where(&|before, after| after < before - FALLS_BACK_TO_THE_START);
    let one_turn = match falls.as_slice() {
        [first, second, ..] => Some((*first, *second)),
        _ => None,
    };
    Watched {
        turn_seconds: one_turn.map(|(first, second)| (second - first) as f64 * seconds_a_look),
        steps_a_turn: one_turn.map_or(0, |(first, second)| moves.iter().filter(|look| **look > first && **look <= second).count()),
        times_it_moved: moves.len(),
        times_it_told_the_mpc,
    }
}

/// Prints what `watch` saw, for the program that asked through `watched_by`.
pub fn print_watched(library: &Path, control: &str) -> Result<(), String> {
    let control: usize = control.parse().map_err(|_| format!("`{control}` is not the number of a control."))?;
    println!("{}", watch(library, control)?.written());
    Ok(())
}

/// Has `program` watch the control in a process of its own, so that a plugin which crashes or hangs
/// takes down that process and nothing else.
pub fn watched_by(program: &Path, library: &Path, control: u32, limit: Duration) -> Result<Watched, String> {
    let mut command = command_for(program);
    command.arg(READ_CLOCK_COMMAND).arg(library).arg(control.to_string()).envs([STAYS_OFFLINE]);
    match run_with_time_limit(&mut command, limit) {
        Ok(finished) => finished.output.lines().rev().find_map(Watched::read_from).ok_or_else(|| could_not_watch(&finished.output)),
        Err(RunError::TimedOut { .. }) => Err("The plugin stopped answering while its moving pictures were being watched.".to_string()),
        Err(RunError::CouldNotStart(error)) => Err(format!("The plugin's moving pictures could not be watched: {error}")),
    }
}

fn could_not_watch(said: &str) -> String {
    format!("The plugin's moving pictures could not be watched: {}", said.lines().last().unwrap_or("the plugin stopped without a word"))
}

/// What is wrong with the way a control moves, in words the AI can act on. Nothing, when its pictures will move.
/// Only the clock has to turn with nothing played. Any other control may be waiting for notes.
pub fn problems_seen(control: &str, watched: &Watched) -> Vec<String> {
    let mut problems = moves_the_mpc_was_not_told_of(control, watched);
    match watched.turn_seconds {
        Some(seconds) => problems.extend(problems_with_the_turn(control, seconds, watched.steps_a_turn)),
        None if is_the_clock(control) => problems.push(why_the_clock_does_not_turn(watched)),
        None => {}
    }
    problems
}

fn moves_the_mpc_was_not_told_of(control: &str, watched: &Watched) -> Vec<String> {
    if watched.times_it_told_the_mpc >= watched.times_it_moved as u64 {
        return Vec::new();
    }
    vec![format!(
        "`{control}` moved {} times but the plugin told the MPC only {} times. An MPC draws a picture again only when it is told, \
         so call `notifyHostParameterChanged` every time the plugin moves it.",
        watched.times_it_moved, watched.times_it_told_the_mpc
    )]
}

fn why_the_clock_does_not_turn(watched: &Watched) -> String {
    if watched.times_it_moved == 0 {
        return format!(
            "`{CLOCK_CONTROL}` never moved while the plugin ran, so the pictures that follow it would stand still on an MPC. \
             Turn it from `process()` the way `mp-forge/pictures.md` in the reference library shows under \"Pictures that play on their own\"."
        );
    }
    format!(
        "`{CLOCK_CONTROL}` did not go from its lowest to its highest and back to its lowest twice in {WATCHED_FOR_SECONDS} seconds. \
         One turn may take {LONGEST_TURN_SECONDS} seconds at most."
    )
}

fn problems_with_the_turn(control: &str, seconds: f64, steps: usize) -> Vec<String> {
    let steps_a_second = steps as f64 / seconds;
    let mut problems = Vec::new();
    if steps < FEWEST_STEPS_A_TURN {
        problems.push(format!("`{control}` goes round in only {steps} steps, so its pictures would jump. Give a turn {FEWEST_STEPS_A_TURN} steps or more."));
    }
    if steps_a_second > MOST_STEPS_A_SECOND {
        problems.push(format!(
            "`{control}` moves {steps_a_second:.0} times a second. An MPC's screen shows 30 pictures a second at most, \
             so anything faster only costs it work. Make the steps fewer."
        ));
    }
    problems
}

/// Remembers how long each self-turning control takes to go round, beside the plugin's build.
pub fn keep_turns(plugin: &Path, turns: &BTreeMap<String, f64>) -> io::Result<()> {
    let kept_in = plugin.join(TURNS_KEPT_IN);
    if turns.is_empty() {
        return match fs::remove_file(kept_in) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    fs::write(kept_in, serde_json::to_string(turns).map_err(io::Error::other)?)
}

/// How long one play of this picture takes on an MPC, when the plugin plays it round and round by itself.
/// A clock that has not been watched yet is taken to turn as the instructions say. Any other control of the
/// plugin's own is only taken to go round when it was seen to.
pub fn turn_seconds_of(plugin: &Path, picture: &Picture) -> Option<f64> {
    let control = control_the_plugin_turns_for(picture, &ScreenLayout::of_plugin(plugin))?;
    let kept: BTreeMap<String, f64> = fs::read_to_string(plugin.join(TURNS_KEPT_IN)).ok().and_then(|kept| serde_json::from_str(&kept).ok()).unwrap_or_default();
    let watched = kept.iter().find(|(name, _)| name.eq_ignore_ascii_case(&control)).map(|(_, seconds)| *seconds);
    watched.or_else(|| is_the_clock(&control).then_some(USUAL_TURN_SECONDS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pictures::test_support::looping_picture;
    use crate::screen_layout::SlotBlock;
    use crate::workspace::test_support::TemporaryFolder;

    const A_SLOT: SlotBlock = SlotBlock { top_row: 3, left_column: 0, rows: 1, columns: 1 };
    const TEN_LOOKS_A_SECOND: f64 = 0.1;

    fn layout_showing(control: &str) -> ScreenLayout {
        ScreenLayout::written_in(&format!(r#"{{ "pages": [{{ "name": "MAIN", "controls": ["{control}"] }}] }}"#))
    }

    fn following(control: &str) -> Picture {
        Picture { motion: Motion::Follows(control.to_string()), ..looping_picture("lamp", A_SLOT) }
    }

    /// A control that climbs in four steps, one step every second look, and then starts again.
    fn four_steps_round_and_round(turns: usize) -> Vec<f32> {
        let one_turn = [0.0, 0.0, 0.33, 0.33, 0.66, 0.66, 1.0, 1.0];
        one_turn.iter().copied().cycle().take(one_turn.len() * turns + 1).collect()
    }

    #[test]
    fn a_picture_that_plays_on_its_own_is_moved_by_the_clock() {
        let layout = layout_showing("Drive");

        assert_eq!(control_the_plugin_turns_for(&looping_picture("reels", A_SLOT), &layout), Some(CLOCK_CONTROL.to_string()));
    }

    #[test]
    fn a_picture_that_follows_a_knob_on_a_page_is_moved_by_the_musician_not_the_plugin() {
        let layout = layout_showing("Drive");

        assert_eq!(control_the_plugin_turns_for(&following("drive"), &layout), None);
        assert_eq!(control_the_plugin_turns_for(&following("Motion"), &layout), Some("Motion".to_string()));
    }

    #[test]
    fn each_control_the_plugin_turns_is_named_once() {
        let pictures = [looping_picture("reels", A_SLOT), looping_picture("spring", A_SLOT), following("Motion"), following("Drive")];

        assert_eq!(controls_the_plugin_turns(&pictures, &layout_showing("Drive")), ["Motion", CLOCK_CONTROL]);
    }

    #[test]
    fn a_turn_is_timed_from_one_fall_back_to_the_start_to_the_next() {
        let watched = seen_in(&four_steps_round_and_round(3), TEN_LOOKS_A_SECOND, 12);

        assert_eq!(watched.steps_a_turn, 4);
        assert!((watched.turn_seconds.unwrap() - 0.8).abs() < 1e-9);
        assert_eq!((watched.times_it_moved, watched.times_it_told_the_mpc), (12, 12));
        assert_eq!(problems_seen(CLOCK_CONTROL, &watched), Vec::<String>::new());
    }

    #[test]
    fn what_was_watched_is_read_back_as_it_was_written() {
        let watched = seen_in(&four_steps_round_and_round(3), TEN_LOOKS_A_SECOND, 12);

        assert_eq!(Watched::read_from(&watched.written()), Some(watched));
        assert_eq!(Watched::read_from("the plugin said something else"), None);
    }

    #[test]
    fn a_clock_that_stands_still_is_a_problem() {
        let problems = problems_seen(CLOCK_CONTROL, &seen_in(&[0.0; 40], TEN_LOOKS_A_SECOND, 0));

        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("never moved"), "{problems:?}");
    }

    #[test]
    fn a_clock_that_turns_without_telling_the_mpc_is_a_problem() {
        let problems = problems_seen(CLOCK_CONTROL, &seen_in(&four_steps_round_and_round(3), TEN_LOOKS_A_SECOND, 0));

        assert!(problems[0].contains("told the MPC only 0 times"), "{problems:?}");
    }

    #[test]
    fn a_clock_that_never_comes_round_or_steps_too_fast_is_a_problem() {
        let never_back: Vec<f32> = (0..60).map(|look| look as f32 / 60.0).collect();
        let too_fast = seen_in(&four_steps_round_and_round(3), 0.01, 12);

        assert!(problems_seen(CLOCK_CONTROL, &seen_in(&never_back, TEN_LOOKS_A_SECOND, 59))[0].contains("back to its lowest twice"));
        assert!(problems_seen(CLOCK_CONTROL, &too_fast)[0].contains("times a second"));
    }

    #[test]
    fn a_control_that_waits_for_notes_may_stand_still_but_must_tell_the_mpc_when_it_moves() {
        let lit_once: Vec<f32> = [0.0, 0.0, 0.5, 0.5, 0.0, 0.0].to_vec();

        assert_eq!(problems_seen("Life", &seen_in(&[0.0; 40], TEN_LOOKS_A_SECOND, 0)), Vec::<String>::new());
        assert_eq!(problems_seen("Life", &seen_in(&lit_once, TEN_LOOKS_A_SECOND, 2)), Vec::<String>::new());
        assert!(problems_seen("Life", &seen_in(&lit_once, TEN_LOOKS_A_SECOND, 1))[0].contains("told the MPC only 1 times"));
    }

    #[test]
    fn a_picture_following_a_control_that_waits_for_notes_is_not_played_round_and_round() {
        let plugin = TemporaryFolder::create("picture-lamps");
        fs::create_dir_all(plugin.0.join("build")).unwrap();
        fs::write(plugin.0.join("ui.json"), r#"{ "pages": [{ "name": "MAIN", "controls": ["Drive"] }] }"#).unwrap();

        assert_eq!(turn_seconds_of(&plugin.0, &following("Life")), None);
        keep_turns(&plugin.0, &BTreeMap::from([("Life".to_string(), 1.6)])).unwrap();

        assert_eq!(turn_seconds_of(&plugin.0, &following("Life")), Some(1.6));
    }

    #[test]
    fn a_watched_turn_is_remembered_for_the_preview_and_the_usual_one_stands_in_until_then() {
        let plugin = TemporaryFolder::create("picture-clock");
        fs::create_dir_all(plugin.0.join("build")).unwrap();
        fs::write(plugin.0.join("ui.json"), r#"{ "pages": [{ "name": "MAIN", "controls": ["Drive"] }] }"#).unwrap();
        let reels = looping_picture("reels", A_SLOT);

        assert_eq!(turn_seconds_of(&plugin.0, &reels), Some(USUAL_TURN_SECONDS));
        keep_turns(&plugin.0, &BTreeMap::from([(CLOCK_CONTROL.to_string(), 1.6)])).unwrap();

        assert_eq!(turn_seconds_of(&plugin.0, &reels), Some(1.6));
        assert_eq!(turn_seconds_of(&plugin.0, &following("Drive")), None);
    }
}
