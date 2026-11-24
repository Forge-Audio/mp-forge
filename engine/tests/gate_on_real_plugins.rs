//! Runs the gate on real plugins: Akai's examples must pass, and deliberately broken plugins must be blocked.
//!
//! These tests compile C++ and need Akai's SDK in `vendor/`. Run `scripts/check-sdk.sh` once to fetch it.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::json;
use workbench_engine::app_sending::{self, Doors, NotSent, Refusal};
use workbench_engine::app_sounds::{AppSound, sounds_of};
use workbench_engine::agent::{Brief, Entry, Model, ModelError, ModelTurn, StopReason, ToolOutput, Toolbox};
use workbench_engine::artwork::{built_control_number, draw_pictures};
use workbench_engine::beat_loop::beat_loop;
use workbench_engine::built_knobs::KNOB_FILE;
use workbench_engine::device_app;
use workbench_engine::device_link::DeviceLink;
use workbench_engine::gate::{
    APP_BUILD_CHECK, APP_SOURCE_CHECK, APP_TRIAL_CHECK,
    BUILD_CHECK, Gate, GateReport, MOVING_PICTURES_CHECK, Outcome, PICTURES_CHECK, SCREEN_CHECK, SOURCE_CHECK, TimeLimits, simulation_check_name,
};
use workbench_engine::hosted_plugin::{HostedPlugin, LARGEST_BLOCK, Note, moves_made_by_the_plugin};
use workbench_engine::library::built_parameters;
use workbench_engine::link_program;
use workbench_engine::maker::{Connection, Maker, PluginRequest, PutOnACard, SentToTheSample};
use workbench_engine::online::{ONLINE_FILE, ONLINE_IS_SWITCHED_OFF};
use workbench_engine::open_mpc_listing::{self, License, Listing};
use workbench_engine::origin::Origin;
use workbench_engine::live_player::Rig;
use workbench_engine::picture_clock::{CLOCK_CONTROL, turn_seconds_of};
use workbench_engine::picture_review::HOW_TO_READ_THE_PICTURES;
use workbench_engine::pictures::pictures_of;
use workbench_engine::plugin_tools::PluginTools;
use workbench_engine::open_mpc;
use workbench_engine::project_file::{self, pack, take_in};
use workbench_engine::sample_app::{self, Input, MidiCameIn, RunningApp, Shown, SoundIn};
use workbench_engine::sample_app_tools::SampleAppTools;
use workbench_engine::sample_surface::Lights;
use workbench_engine::samples::{SampleShelf, sixteen_bit_wav};
use workbench_engine::sd_card::{self, AskedOfTheCard, SdCard, write_trial_card};
use workbench_engine::sdk::{PluginIdentity, PluginKind, Sdk, built_screen_file, native_library_of};
use workbench_engine::simulation::Session;

const FREEZE_LIMIT: Duration = Duration::from_secs(5);

fn engine_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn sdk_dir() -> PathBuf {
    engine_dir().join("../vendor/mpc-plugin-builder-sdk")
}

fn akai_example(name: &str) -> PathBuf {
    sdk_dir().join("examples").join(name)
}

fn broken_plugin(name: &str) -> PathBuf {
    engine_dir().join("tests/fixtures").join(name)
}

fn gate() -> Gate {
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let limits = TimeLimits { simulation: FREEZE_LIMIT, ..TimeLimits::default() };
    Gate::new(sdk).with_limits(limits).hosting_plugins_with(env!("CARGO_BIN_EXE_workbench"))
}

fn everyday_check() -> String {
    simulation_check_name(&Session::EVERYDAY)
}

fn blocking_details(report: &GateReport, check_name: &str) -> String {
    match report.outcome_of(check_name) {
        Some(Outcome::Blocked(details)) => details.join("\n"),
        other => panic!("expected `{check_name}` to block, but it was {other:?}\nfull report: {report:#?}"),
    }
}

#[test]
fn akais_own_examples_are_let_through() {
    for example in ["SimpleSynth", "StereoDelay", "DrumSynth"] {
        let report = gate().inspect(&akai_example(example));

        assert!(report.may_be_sent(), "{example} was blocked: {report:#?}");
    }
}

#[test]
fn a_plugin_that_starts_a_program_is_blocked_before_any_of_its_code_runs() {
    let plugin = broken_plugin("Shell Effect");

    let report = gate().inspect(&plugin);

    assert!(blocking_details(&report, SOURCE_CHECK).contains("starts another program"));
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
    assert!(!plugin.join("build").exists(), "a blocked plugin must never be built");
    assert!(!report.may_be_sent());
}

#[test]
fn a_plugin_that_outputs_broken_audio_is_blocked_by_the_simulator() {
    let report = gate().inspect(&broken_plugin("Nan Effect"));

    assert_eq!(report.outcome_of(SOURCE_CHECK), Some(&Outcome::Passed));
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::Passed));
    assert!(blocking_details(&report, &everyday_check()).contains("NaN"));
    assert!(!report.may_be_sent());
}

#[test]
fn a_plugin_that_freezes_is_stopped_and_blocked() {
    let report = gate().inspect(&broken_plugin("Frozen Effect"));

    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::Passed));
    assert!(blocking_details(&report, &everyday_check()).contains("froze"));
    assert!(!report.may_be_sent());
}

/// Akai's delay example has one page, DELAY, whose bottom two rows are empty.
const SPINNING_REEL: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 278 107"><rect x="129" y="10" width="20" height="87" fill="white" transform="rotate({180 * t} 139 53)"/></svg>"#;
const REEL_AND_LAMP: &str = r#"{ "pictures": [
    { "name": "reel", "page": "DELAY", "where": { "row": 3, "column": 1 }, "motion": "loops", "frames": 6 },
    { "name": "lamp", "page": "DELAY", "where": { "row": 3, "column": 2 }, "motion": { "follows": "Feedback" }, "frames": 4 }
] }"#;

const LAMP_FOR_NOTES: &str = r#"{ "pictures": [
    { "name": "lamp", "page": "DELAY", "where": { "row": 3, "column": 2 }, "motion": { "follows": "Notes Held" }, "frames": 4 }
] }"#;
const CONTROL_FOR_NOTES_HELD: &str = r#"        {
            mpc::Param notesHeld = mpc::floatParam("notesheld", "Notes Held", 0.0f, 1.0f, 0.0f, "", 2);
            notesHeld.automatable = false;
            addParam(notesHeld);
        }"#;

const LAST_CONTROL_OF_THE_DELAY: &str = r#"        addParam(floatParam("mix",    "Mix", 0.0f, 100.0f, 35.0f, "%", 0));"#;
const START_OF_PROCESS: &str = "    void process(mpc::ProcessContext& ctx) override\n    {";
const CLOCK_IN_THE_GUIDE: &str = "## Pictures that play on their own";

/// The two pieces of C++ MP Forge's guide to pictures gives for the clock: how it is declared, and how it is turned.
fn clock_in_the_guide() -> (String, String) {
    let guide = fs::read_to_string(engine_dir().join("../directives/reference/pictures.md")).unwrap();
    let about_the_clock = guide.split(CLOCK_IN_THE_GUIDE).nth(1).expect("the guide says how pictures play on their own");
    let mut code = about_the_clock.split("```cpp\n").skip(1).map(|from_here| from_here.split("```").next().unwrap().to_string());
    (code.next().expect("how the clock is declared"), code.next().expect("how the clock is turned"))
}

/// On Windows, Git may hand Akai's files out with Windows line ends, and the passages looked for here have none.
fn with_line_ends_as_akai_wrote_them(source: &str) -> String {
    source.replace("\r\n", "\n")
}

/// A copy of Akai's delay example in a folder of its own, removed when the test ends.
struct CopiedExample(PathBuf);

impl CopiedExample {
    fn of_the_delay(label: &str) -> Self {
        Self::of("StereoDelay", label)
    }

    fn of(example: &str, label: &str) -> Self {
        let copy = std::env::temp_dir().join(format!("workbench-gate-{label}-{}", std::process::id()));
        let original = akai_example(example);
        fs::create_dir_all(copy.join("src")).unwrap();
        for file in ["plugin.json", "ui.json"] {
            fs::copy(original.join(file), copy.join(file)).unwrap();
        }
        for source in fs::read_dir(original.join("src")).unwrap() {
            let source = source.unwrap().path();
            fs::copy(&source, copy.join("src").join(source.file_name().unwrap())).unwrap();
        }
        Self(copy)
    }

    /// A reel that plays on its own and a lamp that follows a knob, in a plugin that turns the clock for the reel.
    fn with_pictures(self, reel_drawing: &str) -> Self {
        self.with_pictures_and_no_clock(reel_drawing).with_the_picture_clock()
    }

    fn with_pictures_and_no_clock(self, reel_drawing: &str) -> Self {
        fs::create_dir_all(self.0.join("pictures")).unwrap();
        fs::write(self.0.join("pictures.json"), REEL_AND_LAMP).unwrap();
        fs::write(self.0.join("pictures/reel.svg"), reel_drawing).unwrap();
        fs::write(self.0.join("pictures/lamp.svg"), SPINNING_REEL).unwrap();
        self
    }

    /// Gives Akai's delay the clock, written exactly as the AI's instructions give it.
    fn with_the_picture_clock(self) -> Self {
        let (declared, turned) = clock_in_the_guide();
        self.with_the_delay_changed(&[
            ("kMix, kNumParams", "kMix, kPictureClock, kNumParams".to_string()),
            (LAST_CONTROL_OF_THE_DELAY, format!("{LAST_CONTROL_OF_THE_DELAY}\n{declared}")),
            (START_OF_PROCESS, format!("{START_OF_PROCESS}\n        turnPictureClock(ctx);")),
            ("\nprivate:\n", format!("\n{turned}\nprivate:\n")),
        ])
    }

    /// A lamp that follows a control on no page, which the plugin would only move when notes are held.
    fn with_a_lamp_for_the_notes_held(self) -> Self {
        fs::create_dir_all(self.0.join("pictures")).unwrap();
        fs::write(self.0.join("pictures.json"), LAMP_FOR_NOTES).unwrap();
        fs::write(self.0.join("pictures/lamp.svg"), SPINNING_REEL).unwrap();
        self.with_the_delay_changed(&[
            ("kMix, kNumParams", "kMix, kNotesHeld, kNumParams".to_string()),
            (LAST_CONTROL_OF_THE_DELAY, format!("{LAST_CONTROL_OF_THE_DELAY}\n{CONTROL_FOR_NOTES_HELD}")),
        ])
    }

    fn with_the_delay_changed(self, changes: &[(&str, String)]) -> Self {
        let source_file = self.0.join("src/StereoDelay.cpp");
        let mut source = with_line_ends_as_akai_wrote_them(&fs::read_to_string(&source_file).unwrap());
        for (passage, replacement) in changes {
            assert_eq!(source.matches(passage).count(), 1, "Akai's delay no longer has exactly one `{passage}`");
            source = source.replacen(passage, replacement, 1);
        }
        fs::write(source_file, source).unwrap();
        self
    }

    /// Adds a source file to the copy. The build compiles every file in `src`.
    fn with_source(self, file_name: &str, code: &str) -> Self {
        fs::write(self.0.join("src").join(file_name), code).unwrap();
        self
    }
}

impl Drop for CopiedExample {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_plugin_with_pictures_is_let_through_with_them_on_its_built_screen() {
    let plugin = CopiedExample::of_the_delay("pictures").with_pictures(SPINNING_REEL);

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert_eq!(report.outcome_of(SCREEN_CHECK), Some(&Outcome::Passed));
    let screen_file = built_screen_file(&plugin.0).unwrap();
    let screen = fs::read_to_string(&screen_file).unwrap();
    assert!(screen.contains("\"image\": \"pictures/reel.png\"") && !screen.contains("\"animated\": true"), "{screen}");
    assert!(screen.contains("\"image\": \"pictures/lamp.png\""));
    assert!(screen_file.with_file_name("pictures").join("reel.png").is_file());
}

#[test]
fn a_picture_that_plays_on_its_own_follows_a_clock_that_is_seen_to_turn() {
    let plugin = CopiedExample::of_the_delay("clock").with_pictures(SPINNING_REEL);

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert_eq!(report.outcome_of(MOVING_PICTURES_CHECK), Some(&Outcome::Passed));
    let clock = built_control_number(&plugin.0, CLOCK_CONTROL).expect("the built delay has the clock");
    let screen = fs::read_to_string(built_screen_file(&plugin.0).unwrap()).unwrap();
    let reel = &screen[screen.find("pictures/reel.png").unwrap()..];
    assert!(reel.contains(&format!("\"handleName\": \"Parameter {clock}\"")), "{reel}");
    let reel_picture = pictures_of(&plugin.0).unwrap().into_iter().find(|picture| picture.name == "reel").unwrap();
    let one_play = turn_seconds_of(&plugin.0, &reel_picture).unwrap();
    assert!((one_play - 2.0).abs() < 0.05, "one play took {one_play} seconds");
    assert!(fs::read_to_string(plugin.0.join("build/picture_clocks.json")).unwrap().contains(CLOCK_CONTROL));
}

#[test]
fn a_picture_that_plays_on_its_own_without_a_clock_is_blocked() {
    let plugin = CopiedExample::of_the_delay("no-clock").with_pictures_and_no_clock(SPINNING_REEL);

    let report = gate().inspect(&plugin.0);

    assert!(blocking_details(&report, SCREEN_CHECK).contains("Picture `reel` plays on its own"));
    assert_eq!(report.outcome_of(MOVING_PICTURES_CHECK), Some(&Outcome::NotRun));
}

#[test]
fn a_clock_the_plugin_never_turns_is_blocked() {
    let plugin = CopiedExample::of_the_delay("still-clock")
        .with_pictures(SPINNING_REEL)
        .with_the_delay_changed(&[("\n        turnPictureClock(ctx);", String::new())]);

    let report = gate().inspect(&plugin.0);

    assert!(blocking_details(&report, MOVING_PICTURES_CHECK).contains("`Picture Clock` never moved"));
    assert!(!report.may_be_sent());
}

#[test]
fn a_picture_that_waits_for_notes_is_let_through_and_is_not_played_round_and_round() {
    let plugin = CopiedExample::of_the_delay("lamp-for-notes").with_a_lamp_for_the_notes_held();

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert_eq!(report.outcome_of(MOVING_PICTURES_CHECK), Some(&Outcome::Passed));
    let lamp = pictures_of(&plugin.0).unwrap().into_iter().find(|picture| picture.name == "lamp").unwrap();
    assert_eq!(turn_seconds_of(&plugin.0, &lamp), None);
}

#[test]
fn a_screen_the_ai_styled_with_its_tool_is_let_through_without_the_picture_it_hid() {
    let plugin = CopiedExample::of_the_delay("styled-by-the-ai").with_pictures(SPINNING_REEL);
    let gate = gate();
    let mut tools = PluginTools::open(&plugin.0, &sdk_dir(), &gate).unwrap();
    let hide_the_lamp = json!({ "change": "style", "page": "DELAY", "kind": "picture", "name": "lamp", "hidden": true });
    let backdrop = json!({ "change": "add_artwork", "page": "DELAY", "behind": true, "layer": { "kind": "block", "area": { "x": 0, "y": 0, "width": 1280, "height": 628 }, "colour": "#1e3a28" } });

    let reel_behind = json!({ "change": "style", "page": "DELAY", "kind": "picture", "name": "reel", "opacity": 40, "depth": "behind" });

    let styled = tools.run("style_screen", &json!({ "changes": [hide_the_lamp, backdrop, reel_behind] }));
    let report = gate.inspect(&plugin.0);

    assert!(!styled.is_error, "{}", styled.content);
    assert!(styled.content.contains("picture `lamp`: hidden"), "{}", styled.content);
    assert!(report.may_be_sent(), "{report:#?}");
    let screen_file = built_screen_file(&plugin.0).unwrap();
    let screen = fs::read_to_string(&screen_file).unwrap();
    assert!(screen.contains("\"image\": \"pictures/reel.png\"") && !screen.contains("pictures/lamp.png"), "{screen}");
    assert!(screen.contains("look-page-1.png") && screen_file.with_file_name("look-page-1.png").is_file());
    let place_of = |words: &str| screen.find(words).unwrap();
    assert!(place_of("look-page-1.png") < place_of("pictures/reel.png") && place_of("pictures/reel.png") < place_of("\"name\": \"Time\""), "{screen}");
    let kept_solid = fs::read(plugin.0.join("assets/pictures/reel.png")).unwrap();
    assert_ne!(fs::read(screen_file.with_file_name("pictures").join("reel.png")).unwrap(), kept_solid);
}

#[test]
fn a_picture_that_cannot_be_drawn_blocks_the_plugin_before_it_is_built() {
    let plugin = CopiedExample::of_the_delay("bad-picture").with_pictures("<svg><rect width=\"{spin}\"/></svg>");

    let report = gate().inspect(&plugin.0);

    assert!(blocking_details(&report, PICTURES_CHECK).contains("Picture `reel`"));
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
    assert!(!plugin.0.join("build").exists());
}

/// The Feedback knob, the fourth control of the DELAY page, moved to the empty bottom half and renamed.
const FEEDBACK_MOVED_AND_RENAMED: &str = r#"{ "pages": [{ "page": "DELAY", "items": [
    { "kind": "control", "name": "Feedback", "place": { "x": 500, "y": 400 }, "shownName": "ECHOES" }
]}]}"#;
const FEEDBACK_OFF_THE_PAGE: &str = r#"{ "pages": [{ "page": "DELAY", "items": [
    { "kind": "control", "name": "Feedback", "place": { "x": 1100, "y": 400 } }
]}]}"#;

#[test]
fn a_plugin_the_musician_arranged_by_hand_is_let_through_with_its_arrangement_on_the_built_screen() {
    let plugin = CopiedExample::of_the_delay("arranged");
    fs::write(plugin.0.join("look.json"), FEEDBACK_MOVED_AND_RENAMED).unwrap();

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert_eq!(report.outcome_of(SCREEN_CHECK), Some(&Outcome::Passed));
    let screen = fs::read_to_string(built_screen_file(&plugin.0).unwrap()).unwrap();
    assert!(screen.contains("\"bounds\": \"500 400 278 107\""), "{screen}");
    assert!(screen.contains("\"value\": \"WithName[ECHOES]/Parameter "), "{screen}");
    assert!(!screen.contains("\"bounds\": \"983 45 278 107\""), "the knob should have left its slot");
    assert!(screen.contains("\"494 190 773 323\""), "the fourth column's box should surround Feedback and Mix");
}

const FEEDBACK_AND_LOW_CUT_SWAP_KNOBS: &str = r#"{ "pages": [{ "page": "DELAY", "knobs": [
    { "control": "Feedback", "knob": 9 }, { "control": "Low Cut", "knob": 16 }
]}]}"#;

#[test]
fn a_plugin_whose_knobs_the_musician_chose_is_let_through_with_them_in_its_built_knob_file() {
    let plugin = CopiedExample::of_the_delay("knobs-chosen");
    fs::write(plugin.0.join("look.json"), FEEDBACK_AND_LOW_CUT_SWAP_KNOBS).unwrap();

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert_eq!(report.outcome_of(SCREEN_CHECK), Some(&Outcome::Passed));
    let knob_file = built_screen_file(&plugin.0).unwrap().with_file_name(KNOB_FILE);
    let knobs: serde_json::Value = serde_json::from_str(&fs::read_to_string(knob_file).unwrap()).unwrap();
    let on_the_page = &knobs["Screen Mode Q-Links"]["map"][0]["Q-Links"];
    let turned_by = |knob: u32| on_the_page[format!("Q-Link {knob}")].as_u64().map(|number| number as u32);
    assert_eq!(turned_by(9), built_control_number(&plugin.0, "Feedback"));
    assert_eq!(turned_by(16), built_control_number(&plugin.0, "Low Cut"));
    assert_eq!(turned_by(13), built_control_number(&plugin.0, "Sync"));
}

const ONE_STREAM: &str = r#"{ "streams": [{ "name": "Jazz", "address": "http://stream.example.com/jazz.mp3" }] }"#;
const FIRST_INCLUDE_OF_THE_DELAY: &str = "#include \"mpc_plugin.h\"";
/// What a plugin's code writes to hear a stream: MP Forge's helper, started and opened, and its sound read.
const HEARS_THE_STREAM: &str = "        static mpforge::AudioStream fromTheInternet;\n        float heard[2][512];\n        fromTheInternet.start(ctx.sampleRate);\n        fromTheInternet.open(0);\n        fromTheInternet.read(heard[0], heard[1], ctx.numFrames < 512 ? ctx.numFrames : 512);";

impl CopiedExample {
    /// A delay that lists one stream and reads it through MP Forge's helper.
    fn of_the_delay_with_a_stream(label: &str) -> Self {
        let delay = Self::of_the_delay(label).with_the_delay_changed(&[
            (FIRST_INCLUDE_OF_THE_DELAY, format!("{FIRST_INCLUDE_OF_THE_DELAY}\n#include \"mp_forge_stream.h\"")),
            (START_OF_PROCESS, format!("{START_OF_PROCESS}\n{HEARS_THE_STREAM}")),
        ]);
        fs::write(delay.0.join(ONLINE_FILE), ONE_STREAM).unwrap();
        delay
    }
}

#[test]
fn a_plugin_that_reads_a_stream_through_mp_forges_helper_is_let_through_once_online_plugins_are_switched_on() {
    let plugin = CopiedExample::of_the_delay_with_a_stream("stream-on");

    let report = gate().with_online_plugins(true).inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    assert!(plugin.0.join("src/mp_forge_stream.cpp").is_file() && plugin.0.join("src/mp-forge-online/streams.inc").is_file());
    assert!(native_library_of(&plugin.0).is_some());
}

#[test]
fn the_same_plugin_is_blocked_before_it_is_built_while_online_plugins_are_switched_off() {
    let plugin = CopiedExample::of_the_delay_with_a_stream("stream-off");

    let report = gate().inspect(&plugin.0);

    assert_eq!(blocking_details(&report, SOURCE_CHECK), ONLINE_IS_SWITCHED_OFF);
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
    assert!(!plugin.0.join("src/mp_forge_stream.cpp").exists() && !plugin.0.join("build").exists());
}

#[test]
fn a_plugins_own_code_still_may_not_go_online_or_reach_into_the_helper_with_online_plugins_on() {
    let goes_online_itself = CopiedExample::of_the_delay_with_a_stream("stream-own-socket").with_source("Sneak.cpp", "#include <sys/socket.h>\nint sneak() { return socket(2, 1, 0); }\n");
    let reaches_in = CopiedExample::of_the_delay_with_a_stream("stream-reach-in")
        .with_source("Reach.cpp", "#include \"mp-forge-online/small_sockets.h\"\nint reach() { return mp_forge_net::socketThatDoesNotWait(1); }\n");
    let changes_the_helper = CopiedExample::of_the_delay_with_a_stream("stream-changed-helper").with_source("mp_forge_stream.cpp", "#include <sys/socket.h>\nint anywhere() { return socket(2, 1, 0); }\n");

    let online = gate().with_online_plugins(true);
    let (went_online, reached_in, changed) = (online.inspect(&goes_online_itself.0), online.inspect(&reaches_in.0), online.inspect(&changes_the_helper.0));

    assert!(blocking_details(&went_online, SOURCE_CHECK).contains("Sneak.cpp") && blocking_details(&went_online, SOURCE_CHECK).contains("uses the network"));
    assert!(blocking_details(&reached_in, SOURCE_CHECK).contains("reaches into MP Forge's stream helper"));
    assert_eq!(went_online.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
    assert!(changed.may_be_sent(), "a helper that was changed is written over with MP Forge's own, so the plugin is the honest one again: {changed:#?}");
    assert!(!fs::read_to_string(changes_the_helper.0.join("src/mp_forge_stream.cpp")).unwrap().contains("anywhere"));
}

/// A folder of plugins on its own, removed when the test ends.
struct PluginsFolder(PathBuf);

impl PluginsFolder {
    fn empty(label: &str) -> Self {
        let folder = std::env::temp_dir().join(format!("workbench-gate-plugins-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        Self(folder)
    }
}

impl Drop for PluginsFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn akais_delay_handed_over_as_a_file_and_taken_in_under_another_name_is_let_through() {
    let plugins = PluginsFolder::empty("handed-over");
    let packed = plugins.0.join("Stereo Delay.mpforge");

    let from_its_folder = take_in(&akai_example("StereoDelay"), &plugins.0, None).unwrap();
    pack(&plugins.0.join(&from_its_folder), &packed).unwrap();
    let from_the_file = take_in(&packed, &plugins.0, Some("Borrowed Delay")).unwrap();

    assert_eq!((from_its_folder.as_str(), from_the_file.as_str()), ("Stereo Delay", "Borrowed Delay"));
    assert!(!plugins.0.join("Stereo Delay/build").exists(), "what Akai's example had built should stay behind");
    let report = gate().inspect(&plugins.0.join("Borrowed Delay"));
    assert!(report.may_be_sent(), "{report:#?}");
}

#[test]
fn a_plugin_taken_in_that_starts_a_program_is_blocked_before_any_of_its_code_runs() {
    let plugins = PluginsFolder::empty("handed-over-harmful");

    let taken_in = take_in(&broken_plugin("Shell Effect"), &plugins.0, None).unwrap();

    let report = gate().inspect(&plugins.0.join(&taken_in));
    assert!(blocking_details(&report, SOURCE_CHECK).contains("starts another program"));
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
    assert!(!plugins.0.join(&taken_in).join("build").exists(), "a blocked plugin must never be built");
}

#[test]
fn an_arrangement_that_hangs_off_the_screen_blocks_the_plugin_before_it_is_built() {
    let plugin = CopiedExample::of_the_delay("arranged-badly");
    fs::write(plugin.0.join("look.json"), FEEDBACK_OFF_THE_PAGE).unwrap();

    let report = gate().inspect(&plugin.0);

    assert!(blocking_details(&report, PICTURES_CHECK).contains("`Feedback` hangs off the edge of the screen"));
    assert_eq!(report.outcome_of(BUILD_CHECK), Some(&Outcome::NotRun));
}

/// A fading panel behind the DELAY page, the Feedback knob made bigger with its name and value centred
/// underneath in the MPC's other fonts, and the Mix fader made longer.
const ARTWORK_AND_RESIZED_CONTROLS: &str = r##"{ "pages": [{ "page": "DELAY",
    "artwork": [
        { "kind": "block", "id": 1, "area": { "x": 0, "y": 0, "width": 1280, "height": 628 }, "colour": "#203040",
          "fade": { "colour": "#000000", "from": { "x": 0, "y": 0 }, "to": { "x": 0, "y": 100 } } },
        { "kind": "block", "id": 2, "shape": "ellipse", "area": { "x": 480, "y": 330, "width": 220, "height": 220 }, "colour": "#ff8800", "opacity": 40 }
    ],
    "items": [
        { "kind": "control", "name": "Feedback", "place": { "x": 500, "y": 340 }, "parts": {
            "art": { "x": 0, "y": 0, "width": 180, "height": 180 },
            "name": { "area": { "x": 0, "y": 180, "width": 180, "height": 34 }, "font": "Roboto", "bold": true, "height": 28, "colour": "#ff8800", "align": "centre" },
            "value": { "area": { "x": 0, "y": 214, "width": 180, "height": 34 }, "font": "Nexa", "height": 26, "colour": "#ffffff", "align": "centre" } } },
        { "kind": "control", "name": "Mix", "place": { "x": 800, "y": 400 }, "parts": {
            "art": { "x": 0, "y": 40, "width": 400, "height": 48 },
            "name": { "area": { "x": 0, "y": 0, "width": 200, "height": 34 }, "font": "Titillium Web", "height": 24, "colour": "#a0a0a0", "align": "left" },
            "value": { "area": { "x": 200, "y": 0, "width": 200, "height": 34 }, "font": "Titillium Web", "height": 26, "colour": "#ffffff", "align": "right", "hidden": true } } }
    ]
}]}"##;

#[test]
fn artwork_and_controls_resized_by_hand_are_let_through_with_everything_they_need_beside_the_screen() {
    let plugin = CopiedExample::of_the_delay("restyled");
    fs::write(plugin.0.join("look.json"), ARTWORK_AND_RESIZED_CONTROLS).unwrap();

    let report = gate().inspect(&plugin.0);

    assert!(report.may_be_sent(), "{report:#?}");
    let screen_file = built_screen_file(&plugin.0).unwrap();
    let screen = fs::read_to_string(&screen_file).unwrap();
    let size_beside_the_screen = |file: &str| resvg::tiny_skia::Pixmap::decode_png(&fs::read(screen_file.with_file_name(file)).unwrap()).map(|image| (image.width(), image.height())).unwrap();
    assert_eq!(size_beside_the_screen("look-page-1.png"), (1280, 628));
    assert_eq!(size_beside_the_screen("look-knob_uni-180x180-89.png"), (180, 180 * 89));
    assert_eq!(size_beside_the_screen("look-fader_h_uni-400x48-127.png"), (400, 48 * 127));
    assert!(screen.contains("\"bounds\": \"500 340 180 248\"") && screen.contains("\"bounds\": \"800 400 400 88\""), "{screen}");
    assert!(screen.contains("\"name\": \"Roboto\"") && screen.contains("\"name\": \"Nexa\"") && screen.contains("\"style\": \"Bold\""), "{screen}");
    assert!(screen.contains("\"image\": \"look-page-1.png\""), "{screen}");
}

fn run_the_check(tools: &mut PluginTools) -> ToolOutput {
    tools.run("check_plugin", &json!({}))
}

/// A caption gives its picture's name between backticks.
fn pictures_shown_by(output: &ToolOutput) -> Vec<&str> {
    output.illustrations.iter().filter_map(|illustration| illustration.caption.split('`').nth(1)).collect()
}

#[test]
fn the_check_shows_the_ai_each_drawing_once_and_again_only_after_a_redraw() {
    let plugin = CopiedExample::of_the_delay("seen").with_pictures(SPINNING_REEL).with_source("Unfinished.cpp", "this is not C++");
    let gate = gate();
    let mut tools = PluginTools::open(&plugin.0, &sdk_dir(), &gate).unwrap();

    let first = run_the_check(&mut tools);
    let with_nothing_changed = run_the_check(&mut tools);
    fs::write(plugin.0.join("pictures/reel.svg"), SPINNING_REEL.replace("white", "red")).unwrap();
    let after_a_redraw = run_the_check(&mut tools);

    assert_eq!(pictures_shown_by(&first), ["reel", "lamp"]);
    assert!(first.content.contains("BLOCK  Builds for every MPC") && first.content.ends_with(HOW_TO_READ_THE_PICTURES), "{}", first.content);
    assert!(with_nothing_changed.illustrations.is_empty());
    assert!(with_nothing_changed.content.ends_with("Fix the blocked check first."));
    assert_eq!(pictures_shown_by(&after_a_redraw), ["reel"]);
}

#[test]
fn a_check_that_stops_before_the_pictures_shows_none_left_from_last_time() {
    let starts_a_program = "#include <cstdlib>\nvoid tidy() { system(\"ls\"); }\n";
    let plugin = CopiedExample::of_the_delay("stale").with_pictures(SPINNING_REEL).with_source("Tidy.cpp", starts_a_program);
    draw_pictures(&plugin.0).unwrap();
    let gate = gate();
    let mut tools = PluginTools::open(&plugin.0, &sdk_dir(), &gate).unwrap();

    let output = run_the_check(&mut tools);

    assert!(output.content.contains("BLOCK  Code stays inside the plugin"), "{}", output.content);
    assert!(output.illustrations.is_empty());
}

/// A copy of one of Akai's examples, built and loaded the way the live player loads a plugin.
fn hosted_delay(plugin: &CopiedExample) -> HostedPlugin {
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let built = sdk.build(&plugin.0, Duration::from_secs(300)).unwrap();
    assert!(built.succeeded(), "{}", built.output);
    HostedPlugin::load(&native_library_of(&plugin.0).unwrap(), 44_100).unwrap()
}

fn text_the_build_reported(control: &serde_json::Value) -> String {
    let parts = [control["display"].as_str().unwrap_or_default().trim(), control["label"].as_str().unwrap_or_default().trim()];
    parts.iter().filter(|part| !part.is_empty()).copied().collect::<Vec<_>>().join(" ")
}

#[test]
fn the_live_player_reads_a_real_plugins_values_and_they_change_when_a_control_is_turned() {
    let plugin = CopiedExample::of_the_delay("player-values");
    let mut hosted = hosted_delay(&plugin);
    let reported = built_parameters(&plugin.0).unwrap();
    let controls = reported["params"].as_array().unwrap();

    let texts_at_rest: Vec<String> = (0..hosted.control_count()).map(|index| hosted.value_text(index)).collect();
    (0..hosted.control_count()).for_each(|index| hosted.turn_control(index, 1.0));
    let texts_turned_up: Vec<String> = (0..hosted.control_count()).map(|index| hosted.value_text(index)).collect();

    assert_eq!(hosted.control_count(), controls.len());
    assert!(!hosted.is_instrument());
    assert_eq!(texts_at_rest, controls.iter().map(text_the_build_reported).collect::<Vec<_>>());
    assert_ne!(texts_turned_up, texts_at_rest);
    assert_eq!(hosted.value_text(999), "");
}

#[test]
fn the_live_player_lists_a_real_plugins_presets_and_choosing_one_sets_its_controls() {
    let plugin = CopiedExample::of("SimpleSynth", "player-presets");
    let mut hosted = hosted_delay(&plugin);
    let where_controls_stand = |hosted: &HostedPlugin| (0..hosted.control_count()).map(|index| hosted.value_text(index)).collect::<Vec<String>>();
    let presets = hosted.preset_names();
    let last = presets.len() - 1;

    let on_the_first = where_controls_stand(&hosted);
    hosted.choose_preset(last);
    let on_the_last = where_controls_stand(&hosted);
    hosted.choose_preset(999);

    assert!(presets.len() >= 2 && presets.iter().all(|name| !name.is_empty()), "{presets:?}");
    assert_ne!(on_the_last, on_the_first);
    assert_eq!(hosted.preset_chosen(), last, "a preset the plugin does not have is ignored");
    assert!((0.0..=1.0).contains(&hosted.control_amount(0)));
}

#[test]
fn a_plugin_with_no_presets_of_its_own_lists_the_one_it_starts_on() {
    let plugin = CopiedExample::of_the_delay("player-no-presets");

    assert_eq!(hosted_delay(&plugin).preset_names(), ["Default"]);
}

#[test]
fn the_live_player_hears_a_real_delay_repeat_a_click() {
    let plugin = CopiedExample::of_the_delay("player-sound");
    let mut hosted = hosted_delay(&plugin);
    let mut click = vec![[0.0f32; 2]; LARGEST_BLOCK];
    click[0] = [0.8, 0.8];
    let silence = vec![[0.0f32; 2]; LARGEST_BLOCK];
    let mut heard = vec![[0.0f32; 2]; LARGEST_BLOCK];

    let mut loudest_after_the_click = 0.0f32;
    hosted.render(&click, &mut heard);
    for _ in 0..170 {
        hosted.render(&silence, &mut heard);
        assert!(heard.iter().flatten().all(|sample| sample.is_finite()));
        loudest_after_the_click = heard.iter().flatten().fold(loudest_after_the_click, |loudest, sample| loudest.max(sample.abs()));
    }

    assert!(loudest_after_the_click > 0.01, "the delay never repeated the click: {loudest_after_the_click}");
}

fn loudest_in(heard: &[[f32; 2]]) -> f32 {
    heard.iter().flatten().fold(0.0f32, |loudest, sample| loudest.max(sample.abs()))
}

#[test]
fn the_live_player_feeds_an_effect_its_beat_and_keeps_the_sound_within_full_scale() {
    let plugin = CopiedExample::of_the_delay("player-beat");
    let mut rig = Rig::around(hosted_delay(&plugin), 44_100);
    let mut heard = vec![[0.0f32; 2]; 44_100];

    rig.render(&mut heard);

    assert!(loudest_in(&heard) > 0.05, "the effect let no beat through");
    assert!(loudest_in(&heard) <= 1.0);
}

#[test]
fn a_real_effect_that_is_passed_by_lets_its_beat_through_untouched() {
    let plugin = CopiedExample::of_the_delay("player-passed-by");
    let mut rig = Rig::around(hosted_delay(&plugin), 44_100);
    let beat = beat_loop(44_100);
    let mut heard = vec![[0.0f32; 2]; 44_100];
    let listening = rig.listening();

    listening.set_volume(1.0);
    listening.hear_the_effect(false);
    rig.render(&mut heard);

    assert_eq!(heard[22_050..], beat[22_050..44_100], "once the change has been followed, only the beat is heard");
}

#[test]
fn an_effect_waiting_for_its_next_sound_is_fed_silence_and_not_the_beat() {
    let plugin = CopiedExample::of_the_delay("player-waiting");
    let mut rig = Rig::around(hosted_delay(&plugin), 44_100);
    let mut heard = vec![[0.0f32; 2]; 44_100];
    rig.listening().hear_the_effect(false);

    rig.wait_for_its_next_sound();
    rig.render(&mut heard);

    assert_eq!(loudest_in(&heard), 0.0);
}

#[test]
fn the_live_player_plays_an_instrument_with_the_notes_it_is_sent() {
    let plugin = CopiedExample::of("SimpleSynth", "player-notes");
    let mut rig = Rig::around(hosted_delay(&plugin), 44_100);
    let mut before_any_note = vec![[0.0f32; 2]; 4_410];
    let mut while_a_note_is_held = vec![[0.0f32; 2]; 22_050];

    rig.render(&mut before_any_note);
    rig.play(Note { pitch: 60, velocity: 100, pressed: true });
    rig.render(&mut while_a_note_is_held);

    assert!(loudest_in(&before_any_note) < 0.001, "an instrument made sound with no note held");
    assert!(loudest_in(&while_a_note_is_held) > 0.01, "the note was not heard");
}

#[test]
fn the_live_player_hears_of_a_control_the_plugin_moves_by_itself() {
    let plugin = CopiedExample::of_the_delay("player-moves").with_the_picture_clock();
    let mut rig = Rig::around(hosted_delay(&plugin), 44_100);
    let clock = built_control_number(&plugin.0, CLOCK_CONTROL).expect("the built delay has the clock") as usize;
    let mut one_second = vec![[0.0f32; 2]; 44_100];

    rig.render(&mut one_second);
    let moves = moves_made_by_the_plugin();

    let clock_moved_to = moves.iter().find(|(control, _)| *control == clock).map(|(_, amount)| *amount);
    assert!(clock_moved_to.is_some_and(|amount| amount > 0.3 && amount < 0.7), "half a turn in: {moves:?}");
}

fn captions_of(output: &ToolOutput) -> Vec<&str> {
    output.illustrations.iter().map(|illustration| illustration.caption.as_str()).collect()
}

#[test]
fn the_check_shows_the_ai_each_whole_page_once_the_plugin_builds() {
    let plugin = CopiedExample::of_the_delay("pages").with_pictures(SPINNING_REEL);
    let gate = gate();
    let mut tools = PluginTools::open(&plugin.0, &sdk_dir(), &gate).unwrap();

    let first = run_the_check(&mut tools);
    let with_nothing_changed = run_the_check(&mut tools);

    let shown = captions_of(&first);
    assert!(shown[0].starts_with("Page `DELAY` as the MPC lays it out"), "{shown:?}\n{}", first.content);
    assert!(shown[1].starts_with("Picture `reel`") && shown[2].starts_with("Picture `lamp`"), "{shown:?}");
    assert!(first.content.contains("Each page of the screen follows"));
    assert!(with_nothing_changed.illustrations.is_empty());
}

#[test]
fn akais_delay_is_got_ready_for_the_open_mpc_catalog_with_its_source_its_zip_and_its_entry() {
    let plugin = CopiedExample::of_the_delay("open-mpc");
    let into = PluginsFolder::empty("open-mpc-into");
    fs::create_dir_all(&into.0).unwrap();
    assert!(gate().inspect(&plugin.0).may_be_sent());
    let listing = Listing::given("somebody/stereo-delay", License::Mit, "A stereo delay that keeps time with the song.");

    let listed = open_mpc_listing::as_it_would_be_listed(&plugin.0).unwrap();
    let folder = open_mpc_listing::prepare(&plugin.0, &listing, &into.0).unwrap();
    let again = open_mpc_listing::prepare(&plugin.0, &listing, &into.0).unwrap();

    let written = |file: &str| fs::read_to_string(folder.join(file)).unwrap_or_else(|_| panic!("`{file}` was not written"));
    let readme = written("1 - put in your repository/README.md");
    let entry: serde_json::Value = serde_json::from_str(&written("3 - add to the catalog/stereo-delay.json")).unwrap();
    let zip = fs::read(folder.join("2 - attach to your release/Stereo-Delay-1.0.0-mpc-sdk.zip")).unwrap();
    let zip_as_text = String::from_utf8_lossy(&zip);
    assert_eq!((listed.id.as_str(), listed.version.as_str(), listed.guide_is_written), ("stereo-delay", "1.0.0", false));
    assert_eq!(folder, into.0.join("Stereo Delay for Open MPC"));
    assert_eq!(again, into.0.join("Stereo Delay for Open MPC 2"), "the first folder is never written over");
    assert!(readme.starts_with("# Stereo Delay\n\nAn effect for the Akai MPC, by MPC SDK Examples.\n\nA stereo delay that keeps time with the song."));
    assert!(readme.contains("![The DELAY page of the plugin's screen](docs/screen-1.png)") && readme.contains("| **Feedback** |"));
    assert!(folder.join("1 - put in your repository/docs/screen-1.png").is_file());
    assert!(written("1 - put in your repository/LICENSE").contains("MPC SDK Examples"));
    assert!(written("1 - put in your repository/src/StereoDelay.cpp").contains("class StereoDelay"));
    assert!(written("1 - put in your repository/plugin.json").contains("\"description\": \"A stereo delay that keeps time with the song.\""));
    assert_eq!(entry["repo"], "somebody/stereo-delay");
    assert_eq!(entry["kind"], "effect");
    assert_eq!(entry["screenshot"], "https://raw.githubusercontent.com/somebody/stereo-delay/v1.0.0/docs/screen-1.png");
    assert!(zip.starts_with(b"PK"));
    for carried in ["MPC SDK Examples - VST - Stereo Delay/plugin.json", "MPC SDK Examples - VST - Stereo Delay/VST/stereo-delay-arm32.so", "MPC SDK Examples - VST - Stereo Delay/Plugin Skins/TUI.json"] {
        assert!(zip_as_text.contains(carried), "the zip has no `{carried}`");
    }
    assert!(written("START HERE.md").contains("Give it the tag `v1.0.0`"));
}

#[test]
fn a_version_of_somebody_elses_plugin_is_listed_under_their_licence_with_their_notice_and_their_name() {
    let plugin = CopiedExample::of_the_delay("open-mpc-based-on");
    let into = PluginsFolder::empty("open-mpc-based-on-into");
    fs::create_dir_all(&into.0).unwrap();
    let original = Origin { name: "Stereo Delay".to_string(), author: "its makers".to_string(), source: "https://github.com/its-makers/stereo-delay".to_string(), license: "GPL-3.0-or-later".to_string() };
    original.write_into(&plugin.0).unwrap();
    fs::write(plugin.0.join("LICENSE"), "The licence, with its makers' notice.").unwrap();
    assert!(gate().inspect(&plugin.0).may_be_sent());
    let listed_under = |license: License| open_mpc_listing::prepare(&plugin.0, &Listing::given("somebody/stereo-delay", license, "A stereo delay, a little changed."), &into.0);

    let owed = open_mpc_listing::as_it_would_be_listed(&plugin.0).unwrap().owed.unwrap();
    let under_another = listed_under(License::Mit).unwrap_err().to_string();
    let folder = listed_under(License::Gpl3).unwrap();

    let written = |file: &str| fs::read_to_string(folder.join("1 - put in your repository").join(file)).unwrap_or_else(|_| panic!("`{file}` was not written"));
    assert_eq!(owed.license, Some(License::Gpl3));
    assert!(under_another.contains("So this plugin has to be listed under GPL-3.0-only."), "{under_another}");
    assert_eq!(written("LICENSE"), "The licence, with its makers' notice.");
    assert!(written("README.md").contains("Based on Stereo Delay by its makers (https://github.com/its-makers/stereo-delay), which is shared under GPL-3.0-or-later."));
    assert_eq!(Origin::of_plugin(&folder.join("1 - put in your repository")), Some(original));
}

/// The example Sample app that makes its sound in code, copied to a folder of its own.
fn pocket_pads_in(apps: &PluginsFolder) -> PathBuf {
    example_app_in(apps, "Pocket Pads", "src/PocketPads.cpp")
}

/// The example Sample app that plays the musician's sound files, copied to a folder of its own.
fn sample_pads_in(apps: &PluginsFolder) -> PathBuf {
    example_app_in(apps, "Sample Pads", "src/SamplePads.cpp")
}

fn sample_echo_in(apps: &PluginsFolder) -> PathBuf {
    example_app_in(apps, "Sample Echo", "src/SampleEcho.cpp")
}

/// An example app is copied before it is used, because building it writes into its folder.
fn example_app_in(apps: &PluginsFolder, name: &str, code_file: &str) -> PathBuf {
    let example = engine_dir().join("tests/fixtures").join(name);
    let copy = apps.0.join(name);
    fs::create_dir_all(copy.join("src")).unwrap();
    for file in ["plugin.json", code_file] {
        fs::copy(example.join(file), copy.join(file)).unwrap();
    }
    copy
}

fn loudest(sound: &[f32]) -> f32 {
    sound.iter().fold(0.0, |loudest, frame| frame.abs().max(loudest))
}

#[test]
fn a_sample_app_is_built_for_this_computer_and_answers_what_is_touched_with_sound_picture_and_lights() {
    const FRAMES: usize = 512;
    const TITLE_BAR: [u8; 4] = [181, 39, 31, 255];
    let apps = PluginsFolder::empty("sample-app");
    let app_dir = pocket_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let silence = [0.0f32; FRAMES];
    let (mut left, mut right) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);

    let library = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let before_a_hit = (loudest(&left), app.show());
    app.tell(Input::pad_hit(1, 1.0));
    app.tell(Input::button("PLAY", true).unwrap());
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let after_a_hit = (loudest(&left), app.show());

    assert_eq!(before_a_hit.0, 0.0, "nothing is heard until a pad is hit");
    assert!(after_a_hit.0 > 0.05 && after_a_hit.0 <= 1.0, "a hit pad is heard, at {}", after_a_hit.0);
    assert_eq!(left, right);
    assert_eq!(after_a_hit.1.picture.len(), sample_app::SCREEN_WIDTH * sample_app::SCREEN_HEIGHT * 4);
    assert_eq!(after_a_hit.1.picture[..4], TITLE_BAR, "the app draws its title bar across the top");
    assert_ne!(before_a_hit.1.pad_colours[0], after_a_hit.1.pad_colours[0], "the pad that was hit lights up");
    assert_eq!(before_a_hit.1.pad_colours[1], after_a_hit.1.pad_colours[1], "and no other pad does");
    assert_eq!((before_a_hit.1.lit_buttons, after_a_hit.1.lit_buttons), (Vec::<&str>::new(), vec!["PLAY"]));
    assert!(fs::read_to_string(app_dir.join("src/mpc_sample_app.h")).unwrap().contains("MP Forge's kit for an app that runs on the MPC Sample"));
}

#[test]
fn a_keyboard_plugged_in_plays_pocket_pads_note_by_note() {
    const FRAMES: usize = 512;
    let apps = PluginsFolder::empty("sample-app-midi");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &pocket_pads_in(&apps), Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let silence = [0.0f32; FRAMES];
    let (mut left, mut right) = ([0.0f32; FRAMES], [0.0f32; FRAMES]);
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();

    let at_rest = app.show();
    let listens_at = app.sound_in();
    app.tell(Input::note_off(36, 1));
    app.tell(Input::midi(&[0x90, 36, 0]).unwrap());
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let after_notes_let_go = loudest(&left);
    app.tell(Input::midi(&[0x90, 37, 127]).unwrap());
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let after_a_note = (loudest(&left), app.show());

    assert_eq!(after_notes_let_go, 0.0, "a note let go plays nothing, however a keyboard says it");
    assert!(after_a_note.0 > 0.05, "a note played is heard, at {}", after_a_note.0);
    assert_ne!(at_rest.pad_colours[1], after_a_note.1.pad_colours[1], "the second note up from the first pad's lights the second pad");
    assert_eq!(at_rest.pad_colours[0], after_a_note.1.pad_colours[0], "and no other pad");
    assert_eq!(listens_at, SoundIn::Off, "an app that never says where to listen hears nothing at its inputs");
}

#[test]
fn the_kits_page_lets_the_musician_choose_the_midi_an_app_is_played_by() {
    const FIRST_NOTE: u8 = 36;
    const ROWS_FROM_BOTH_SOCKETS_TO_OFF: i32 = 3;
    const ROWS_FROM_OFF_TO_USB: i32 = -2;
    const ROWS_FROM_USB_TO_CHANNEL_TWO: i32 = 5;
    let apps = PluginsFolder::empty("sample-app-midi-page");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &pocket_pads_in(&apps), Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let at_rest = app.show();
    let choose_the_row = |app: &mut RunningApp, rows_on: i32| {
        app.tell(Input::encoder_turned(rows_on));
        press(app, "ENCODER");
    };
    let note_for_pad = |pad: u8, channel: u8| Input::note_on(FIRST_NOTE + pad - 1, 100, channel);

    press(&mut app, "PAD BANK");
    let the_page = app.show();
    choose_the_row(&mut app, ROWS_FROM_BOTH_SOCKETS_TO_OFF);
    app.tell(note_for_pad(1, 1));
    choose_the_row(&mut app, ROWS_FROM_OFF_TO_USB);
    app.tell(note_for_pad(2, 1).having_come_in(MidiCameIn::AtTheSocket));
    app.tell(note_for_pad(3, 1).having_come_in(MidiCameIn::AtUsb));
    app.tell(note_for_pad(4, 1));
    choose_the_row(&mut app, ROWS_FROM_USB_TO_CHANNEL_TWO);
    app.tell(note_for_pad(5, 1));
    app.tell(note_for_pad(6, 2));
    press(&mut app, "B3");
    let closed = app.show();

    let lit_pads: Vec<usize> = (1..=sample_app::PADS).filter(|pad| closed.pad_colours[pad - 1] != at_rest.pad_colours[pad - 1]).collect();
    assert_ne!(the_page.picture, at_rest.picture, "the kit's page covers the app's screen");
    assert_eq!(the_page.lit_buttons, ["PAD BANK"]);
    assert_eq!(lit_pads, [3, 4, 6], "only MIDI from where the app listens, on the channel it listens on, plays it: a computer stands in for both sockets");
    assert_ne!(closed.picture, the_page.picture, "B3 closes the page");
    assert!(closed.lit_buttons.is_empty());
}

/// One second of what the app plays after pad 1 is hit, with its knobs and fader where `stands` puts them.
fn a_second_of_pad_one(app_library: &Path, stands: &[Input]) -> Vec<f32> {
    const FRAMES: usize = 44_100;
    let silence = vec![0.0f32; FRAMES];
    let (mut left, mut right) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    let mut app = RunningApp::load(app_library, 44_100.0, Vec::new()).unwrap();
    stands.iter().for_each(|input| app.tell(*input));
    app.tell(Input::pad_hit(1, 1.0));
    app.make_sound(&silence, &silence, &mut left, &mut right);
    left
}

fn times_it_crosses_silence(sound: &[f32]) -> usize {
    sound.windows(2).filter(|pair| pair[0] < 0.0 && pair[1] >= 0.0).count()
}

#[test]
fn the_knobs_and_the_fader_of_pocket_pads_change_what_is_heard() {
    let apps = PluginsFolder::empty("sample-app-knobs");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &pocket_pads_in(&apps), Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let heard_with = |stands: &[Input]| a_second_of_pad_one(&library, stands);
    let late = |sound: &[f32]| loudest(&sound[22_050..]);

    let as_it_starts = heard_with(&[]);
    let tuned_up = heard_with(&[Input::knob_turned(1, 1.0)]);
    let cut_short = heard_with(&[Input::knob_turned(2, 0.0)]);
    let (smooth, rough) = (heard_with(&[Input::knob_turned(3, 0.0)]), heard_with(&[Input::knob_turned(3, 1.0)]));
    let quieter = heard_with(&[Input::fader_moved(0.25)]);
    let another_wave = heard_with(&[Input::encoder_turned(2)]);

    assert!(times_it_crosses_silence(&tuned_up[..4410]) > times_it_crosses_silence(&as_it_starts[..4410]) * 3 / 2, "K1 tunes the pads up");
    assert!(late(&cut_short) < late(&as_it_starts) / 10.0, "K2 cuts a note short: {} against {}", late(&cut_short), late(&as_it_starts));
    assert!(loudest(&rough[4410..8820]) > loudest(&smooth[4410..8820]) * 2.0, "K3 adds grit, which is heard as louder and rougher");
    assert_ne!(another_wave[..2000], as_it_starts[..2000], "the dial picks another wave");
    assert!(loudest(&quieter) < loudest(&as_it_starts) / 2.0, "the fader turns it down");
}

#[test]
fn a_sample_app_whose_code_does_not_build_says_what_the_compiler_said() {
    let apps = PluginsFolder::empty("sample-app-broken");
    let app_dir = pocket_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    fs::write(app_dir.join("src/PocketPads.cpp"), "#include \"mpc_sample_app.h\"\nthis is not code\n").unwrap();

    let not_built = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_err().to_string();

    assert!(not_built.starts_with("The app's code did not build:") && not_built.contains("PocketPads.cpp"), "{not_built}");
}

/// Fetches a real plugin's source from the web, so it is only run by hand:
/// `cargo test --test gate_on_real_plugins -- --ignored a_real_plugin_from_the_web`.
#[test]
#[ignore]
fn a_real_plugin_from_the_web_is_taken_in_with_its_licence_and_passes_the_gate() {
    let plugins = PluginsFolder::empty("from-the-web");
    let original = Origin { name: "MPC Radio".to_string(), author: "Forge Audio".to_string(), source: "https://github.com/Forge-Audio/mpc-radio".to_string(), license: "MIT".to_string() };

    let packed_source = open_mpc::packed_source_of(&original.source).unwrap();
    let name = project_file::take_in_from(&original, packed_source, &plugins.0, None).unwrap();
    let report = gate().with_online_plugins(true).inspect(&plugins.0.join(&name));

    let taken_in = plugins.0.join(&name);
    assert_eq!(name, "MPC Radio");
    assert!(taken_in.join("LICENSE").is_file());
    assert_eq!(Origin::of_plugin(&taken_in), Some(original));
    assert!(report.may_be_sent(), "{report:?}");
}

#[test]
fn a_plugin_that_goes_online_is_listed_with_the_whole_stream_helper_and_says_so() {
    let plugin = CopiedExample::of_the_delay_with_a_stream("open-mpc-online");
    let into = PluginsFolder::empty("open-mpc-online-into");
    fs::create_dir_all(&into.0).unwrap();
    assert!(gate().with_online_plugins(true).inspect(&plugin.0).may_be_sent());
    let listing = Listing::given("somebody/stereo-delay", License::Gpl3, "A stereo delay with a radio in it.");

    let folder = open_mpc_listing::prepare(&plugin.0, &listing, &into.0).unwrap();

    let written = |file: &str| fs::read_to_string(folder.join(file)).unwrap_or_else(|_| panic!("`{file}` was not written"));
    let in_the_plugin = |file: &str| fs::read_to_string(plugin.0.join(file)).unwrap();
    for helper_file in ["src/mp_forge_stream.h", "src/mp_forge_stream.cpp", "src/mp-forge-online/small_sockets.h", "src/mp-forge-online/minimp3.h", "src/mp-forge-online/streams.inc"] {
        assert_eq!(written(&format!("1 - put in your repository/{helper_file}")), in_the_plugin(helper_file), "`{helper_file}`");
    }
    assert!(written("1 - put in your repository/README.md").contains("This plugin brings in sound from the internet"));
    assert!(written("1 - put in your repository/LICENSE").contains("GNU GENERAL PUBLIC LICENSE"));
    let steps = written("START HERE.md");
    assert!(steps.contains("**This plugin goes online.**") && steps.contains("http://stream.example.com/jazz.mp3"));
    assert!(steps.contains("press Fork, then Create fork") && steps.contains("In your copy, open the folder `catalog/plugins`"), "nobody can upload into a repository that is not theirs");
    assert!(steps.contains("SHA-256, which is `"));
}

// ---------------------------------------------------------------------------
// A Sample app is made, checked and looked at
// ---------------------------------------------------------------------------

/// A new Sample app as MP Forge starts one, with its starter's code changed by `rewritten`.
fn new_sample_app_in(apps: &PluginsFolder, rewritten: impl Fn(String) -> String) -> PathBuf {
    let identity = PluginIdentity { name: "Night Bus".to_string(), maker: "Workbench Tests".to_string(), kind: PluginKind::SampleApp };
    fs::create_dir_all(&apps.0).unwrap();
    let app_dir = sample_app::create_starter(&apps.0, &identity).unwrap();
    let code = app_dir.join("src/App.cpp");
    fs::write(&code, rewritten(fs::read_to_string(&code).unwrap())).unwrap();
    app_dir
}

fn as_it_is(code: String) -> String {
    code
}

#[test]
fn the_app_a_new_sample_app_starts_as_passes_its_safety_check() {
    let apps = PluginsFolder::empty("sample-starter");
    let app_dir = new_sample_app_in(&apps, as_it_is);

    let report = gate().inspect(&app_dir);

    assert!(report.may_be_sent(), "{report}");
    assert!(sample_app::is_one(&app_dir));
    assert!(fs::read_to_string(app_dir.join("src/App.cpp")).unwrap().contains("screen.write(\"NIGHT BUS\""));
    assert_eq!(report.checks.iter().map(|check| check.name.as_str()).collect::<Vec<_>>(), [APP_SOURCE_CHECK, APP_BUILD_CHECK, APP_TRIAL_CHECK]);
}

#[test]
fn pocket_pads_passes_the_safety_check_of_a_sample_app() {
    let apps = PluginsFolder::empty("sample-pocket-pads");

    let report = gate().inspect(&pocket_pads_in(&apps));

    assert!(report.may_be_sent(), "{report}");
}

#[test]
fn a_sample_app_that_starts_a_thread_is_stopped_before_it_is_built() {
    let apps = PluginsFolder::empty("sample-thread");
    let app_dir = new_sample_app_in(&apps, |code| code.replace("#include <cmath>", "#include <cmath>\n#include <thread>"));

    let report = gate().inspect(&app_dir);

    assert!(blocking_details(&report, APP_SOURCE_CHECK).contains("starts its own threads"), "{report}");
    assert_eq!(report.outcome_of(APP_BUILD_CHECK), Some(&Outcome::NotRun));
}

#[test]
fn a_sample_app_that_plays_louder_than_the_device_can_is_blocked() {
    let apps = PluginsFolder::empty("sample-loud");
    let app_dir = new_sample_app_in(&apps, |code| code.replace("kLoudness = 0.3f", "kLoudness = 3.0f"));

    let report = gate().inspect(&app_dir);

    assert!(blocking_details(&report, APP_TRIAL_CHECK).contains("the device plays nothing above 1"), "{report}");
}

#[test]
fn a_sample_app_that_crashes_when_a_pad_is_hit_is_blocked_and_takes_nothing_else_down() {
    let apps = PluginsFolder::empty("sample-crash");
    let crash_on_a_hit = "padLastHit_ = pad;\n        volatile int* nowhere = nullptr;\n        *nowhere = pad;";
    let app_dir = new_sample_app_in(&apps, |code| code.replace("padLastHit_ = pad;", crash_on_a_hit));

    let report = gate().inspect(&app_dir);

    assert!(blocking_details(&report, APP_TRIAL_CHECK).contains("The app stopped while it was being played"), "{report}");
}

#[test]
fn a_sample_app_whose_code_does_not_build_is_blocked_with_what_the_compiler_said() {
    let apps = PluginsFolder::empty("sample-unbuilt");
    let app_dir = new_sample_app_in(&apps, |code| code.replace("MPC_SAMPLE_APP(StarterApp)", "MPC_SAMPLE_APP(NoSuchApp)"));

    let report = gate().inspect(&app_dir);

    assert!(blocking_details(&report, APP_BUILD_CHECK).contains("NoSuchApp"), "{report}");
}

/// A tone at full level, as the 16 bit numbers of a WAV file with one side.
fn full_level_tone(frames: usize) -> Vec<i16> {
    (0..frames).map(|frame| ((frame as f32 * 0.05).sin() * f32::from(i16::MAX)) as i16).collect()
}

/// What Sample Pads plays in the half second after `pad` is hit, with these sound files to play.
fn heard_from_sample_pads(library: &Path, sounds: Vec<AppSound>, pad: i32) -> Vec<f32> {
    const FRAMES: usize = 22_050;
    let silence = vec![0.0f32; FRAMES];
    let (mut left, mut right) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    let mut app = RunningApp::load(library, 44_100.0, sounds).unwrap();
    app.tell(Input::fader_moved(1.0));
    app.tell(Input::pad_hit(pad, 1.0));
    app.make_sound(&silence, &silence, &mut left, &mut right);
    left
}

#[test]
fn a_sample_app_plays_the_sound_files_the_musician_added_and_is_silent_past_their_end() {
    const FRAMES_OF_THE_FILE: usize = 4_410;
    let apps = PluginsFolder::empty("sample-app-sounds");
    let app_dir = sample_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    SampleShelf::of_plugin(&app_dir).add("tone.wav", &sixteen_bit_wav(1, 44_100, &full_level_tone(FRAMES_OF_THE_FILE))).unwrap();
    let library = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let sounds = sounds_of(&app_dir, 44_100);

    let its_pad = heard_from_sample_pads(&library, sounds.clone(), 1);
    let a_pad_with_no_file = heard_from_sample_pads(&library, sounds.clone(), 2);
    let with_no_files_at_all = heard_from_sample_pads(&library, Vec::new(), 2);

    assert_eq!(sounds.iter().map(|sound| (sound.name.to_str().unwrap(), sound.frames())).collect::<Vec<_>>(), [("tone", FRAMES_OF_THE_FILE)]);
    assert!(loudest(&its_pad[..FRAMES_OF_THE_FILE]) > 0.5, "the pad plays its file, at {}", loudest(&its_pad[..FRAMES_OF_THE_FILE]));
    assert!(loudest(&its_pad) < 1.0, "and holds it under full");
    assert_eq!(loudest(&its_pad[FRAMES_OF_THE_FILE + 1..]), 0.0, "nothing is heard once the file has ended");
    assert_eq!(loudest(&a_pad_with_no_file), 0.0, "a pad with no file is silent");
    assert!(loudest(&with_no_files_at_all) > 0.05, "with no files added, a pad plays a tone of the app's own");
}

/// Presses a button of the device and lets it go.
fn press(app: &mut RunningApp, button: &str) {
    app.tell(Input::button(button, true).unwrap());
    app.tell(Input::button(button, false).unwrap());
}

#[test]
fn the_kits_browser_takes_a_file_from_the_card_onto_the_pad_that_was_hit() {
    const FRAMES: usize = 4_410;
    let apps = PluginsFolder::empty("sample-app-card");
    let app_dir = sample_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let card = write_trial_card(&apps.0.join("card")).unwrap();
    let silence = vec![0.0f32; FRAMES];
    let (mut left, mut right) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let asked_and_answered = |app: &mut RunningApp| {
        let asked = app.asked_of_the_card();
        asked.iter().for_each(|asked| app.take_answer(card.answer(asked.clone(), 44_100)));
        asked
    };
    app.tell(Input::fader_moved(1.0));

    app.tell(Input::pad_hit(3, 1.0));
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let before_a_file_is_loaded = app.show();
    press(&mut app, "SAMPLE SELECT");
    let on_opening = asked_and_answered(&mut app);
    let the_page = app.show();
    press(&mut app, "ENCODER");
    let on_going_into_the_first_folder = asked_and_answered(&mut app);
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let on_picking_the_second_file = asked_and_answered(&mut app);
    let nothing_more = asked_and_answered(&mut app);
    let after_a_file_is_loaded = app.show();
    left.fill(0.0);
    app.make_sound(&silence, &silence, &mut left, &mut right);
    let before_the_pad_is_hit_again = loudest(&left);
    app.tell(Input::pad_hit(3, 1.0));
    app.make_sound(&silence, &silence, &mut left, &mut right);

    assert_eq!(on_opening, Some(AskedOfTheCard::Folder(String::new())));
    assert_eq!(on_going_into_the_first_folder, Some(AskedOfTheCard::Folder("DRUMS".to_string())));
    assert_eq!(on_picking_the_second_file, Some(AskedOfTheCard::File { path: "DRUMS/snare.wav".to_string(), into_sample: 2 }));
    assert_eq!(nothing_more, None);
    assert_ne!(the_page.picture, before_a_file_is_loaded.picture, "the kit's page covers the app's screen");
    assert_eq!(the_page.lit_buttons, ["SAMPLE SELECT"]);
    assert_ne!(after_a_file_is_loaded.picture, the_page.picture, "the page closes once the file is loaded");
    assert_ne!(after_a_file_is_loaded.picture, before_a_file_is_loaded.picture, "and the pad now shows its file");
    assert!(before_the_pad_is_hit_again < 0.1, "only the tone the pad played while it had no file is still fading");
    assert!(loudest(&left) > 0.4, "the pad plays the file from the card, at {}", loudest(&left));
}

#[test]
fn on_a_computer_the_kits_browser_takes_the_file_the_musician_picked_there() {
    const FRAMES: usize = 4_410;
    const SAMPLE_OF_PAD_THREE: usize = 2;
    let apps = PluginsFolder::empty("sample-app-picked");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &sample_pads_in(&apps), Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    write_trial_card(&apps.0.join("music")).unwrap();
    let silence = vec![0.0f32; FRAMES];
    let (mut left, mut right) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let open_the_page_with = |app: &mut RunningApp, card: SdCard| {
        press(app, "SAMPLE SELECT");
        let asked = app.asked_of_the_card().unwrap();
        app.take_answer(card.answer(asked, 44_100));
        app.show().picture
    };
    app.tell(Input::fader_moved(1.0));
    app.tell(Input::pad_hit(3, 1.0));

    let page_with_no_card = open_the_page_with(&mut app, SdCard::none());
    press(&mut app, "B3");
    let before_the_page_opens = app.browsing_into();
    let page_on_a_computer = open_the_page_with(&mut app, SdCard::picked_on_the_computer());
    let while_the_musician_picks = app.browsing_into();
    app.close_browser();
    let once_they_pick_none = app.browsing_into();
    open_the_page_with(&mut app, SdCard::picked_on_the_computer());
    app.take_answer(sd_card::picked_on_the_computer(&apps.0.join("music/DRUMS/snare.wav"), SAMPLE_OF_PAD_THREE, 44_100));
    let once_they_pick_a_file = app.browsing_into();
    app.make_sound(&silence, &silence, &mut left, &mut right);
    app.tell(Input::pad_hit(3, 1.0));
    app.make_sound(&silence, &silence, &mut left, &mut right);

    assert_ne!(page_on_a_computer, page_with_no_card, "the page says a file is picked on the computer, where it would say there is no card");
    assert_eq!((before_the_page_opens, while_the_musician_picks), (None, Some(SAMPLE_OF_PAD_THREE)), "the open page says which sample the file is for");
    assert_eq!((once_they_pick_none, once_they_pick_a_file), (None, None), "the page closes either way");
    assert!(loudest(&left) > 0.4, "the pad plays the file that was picked, at {}", loudest(&left));
}

#[test]
fn the_kits_browser_takes_an_mp3_from_the_card_as_it_takes_a_wav() {
    let apps = PluginsFolder::empty("sample-app-card-mp3");
    let app_dir = sample_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let card = write_trial_card(&apps.0.join("card")).unwrap();
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let asked_and_answered = |app: &mut RunningApp| {
        let asked = app.asked_of_the_card();
        asked.iter().for_each(|asked| app.take_answer(card.answer(asked.clone(), 44_100)));
        asked
    };

    press(&mut app, "SAMPLE SELECT");
    asked_and_answered(&mut app);
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let on_going_into_the_second_folder = asked_and_answered(&mut app);
    let the_page = app.show();
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let on_picking_the_second_file = asked_and_answered(&mut app);
    let after_the_mp3_is_loaded = app.show();

    assert_eq!(on_going_into_the_second_folder, Some(AskedOfTheCard::Folder("LOOPS".to_string())));
    assert_eq!(on_picking_the_second_file, Some(AskedOfTheCard::File { path: "LOOPS/quiet.mp3".to_string(), into_sample: 0 }));
    assert_ne!(after_the_mp3_is_loaded.picture, the_page.picture, "the page closes once the MP3 is loaded");
    assert!(after_the_mp3_is_loaded.lit_buttons.is_empty(), "and SAMPLE SELECT is no longer lit");
}

#[test]
fn the_dial_and_its_buttons_are_the_apps_again_once_the_browser_is_closed() {
    let apps = PluginsFolder::empty("sample-app-card-closed");
    let app_dir = sample_pads_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let as_it_starts = app.show();

    press(&mut app, "SAMPLE SELECT");
    let asked = app.asked_of_the_card();
    app.take_answer(SdCard::none().answer(asked.clone().unwrap(), 44_100));
    let with_no_card = app.show();
    press(&mut app, "B3");
    let closed = app.show();

    assert_eq!(asked, Some(AskedOfTheCard::Folder(String::new())));
    assert_ne!(with_no_card.picture, as_it_starts.picture);
    assert_eq!(closed, as_it_starts);
}

#[test]
fn sample_pads_passes_the_safety_check_with_sound_files_and_with_none() {
    let apps = PluginsFolder::empty("sample-pads-check");
    let app_dir = sample_pads_in(&apps);

    let with_none = gate().inspect(&app_dir);
    for name in ["kick.wav", "snare.wav", "hat.wav"] {
        SampleShelf::of_plugin(&app_dir).add(name, &sixteen_bit_wav(1, 48_000, &full_level_tone(24_000))).unwrap();
    }
    let with_three = gate().inspect(&app_dir);

    assert!(with_none.may_be_sent(), "{with_none}");
    assert!(with_three.may_be_sent(), "{with_three}");
}

#[test]
fn sample_echo_passes_the_safety_check() {
    let apps = PluginsFolder::empty("sample-echo-check");

    let verdict = gate().inspect(&sample_echo_in(&apps));

    assert!(verdict.may_be_sent(), "{verdict}");
}

#[test]
fn the_kits_page_lets_the_musician_choose_where_sound_comes_in_from() {
    const FRAMES: usize = 4_410;
    let apps = PluginsFolder::empty("sample-app-sound-in");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, &sample_echo_in(&apps), Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let coming_in = vec![0.5f32; FRAMES];
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let heard_now = |app: &mut RunningApp| {
        let (mut left, mut right) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
        app.make_sound(&coming_in, &coming_in, &mut left, &mut right);
        loudest(&left)
    };

    let as_it_starts = (app.sound_in(), heard_now(&mut app), app.show());
    press(&mut app, "SAMPLE");
    let the_page = app.show();
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let on_the_microphone = (app.sound_in(), heard_now(&mut app));
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let on_usb = (app.sound_in(), heard_now(&mut app));
    app.tell(Input::encoder_turned(1));
    press(&mut app, "ENCODER");
    let turned_off = (app.sound_in(), heard_now(&mut app));
    press(&mut app, "B3");
    let closed = app.show();

    assert_eq!(as_it_starts.0, SoundIn::LineIn, "the app says as it starts that it listens at the jacks");
    assert!(as_it_starts.1 > 0.4, "and what comes in there is heard, at {}", as_it_starts.1);
    assert_ne!(the_page.picture, as_it_starts.2.picture, "the kit's page covers the app's screen");
    assert_eq!(the_page.lit_buttons, ["SAMPLE"]);
    assert_eq!(on_the_microphone.0, SoundIn::Mic);
    assert!(on_the_microphone.1 > 0.4, "what the microphone hears is heard, at {}", on_the_microphone.1);
    assert_eq!(on_usb.0, SoundIn::Usb);
    assert!(on_usb.1 > 0.4, "what comes in over USB is heard, at {}", on_usb.1);
    assert_eq!(turned_off, (SoundIn::Off, 0.0), "with its inputs off the app is handed silence");
    assert_ne!(closed.picture, the_page.picture, "B3 closes the page");
    assert!(closed.lit_buttons.is_empty());
}

const START_OF_A_PNG: [u8; 4] = [0x89, b'P', b'N', b'G'];

#[test]
fn the_ai_is_shown_the_screen_of_a_sample_app_after_the_touches_it_names() {
    let apps = PluginsFolder::empty("sample-look");
    let app_dir = pocket_pads_in(&apps);
    let gate = gate();
    let mut tools = SampleAppTools::open(&app_dir, &gate).unwrap();
    let touches = json!({ "touches": [{ "control": "pad", "number": 5, "value": 127 }, { "control": "button", "name": "PLAY", "value": 1 }] });

    let looked = tools.run("look_at_screen", &touches);
    let asked_the_impossible = tools.run("look_at_screen", &json!({ "touches": [{ "control": "wheel", "value": 1 }] }));

    assert!(!looked.is_error, "{}", looked.content);
    assert!(looked.illustrations[0].png.starts_with(&START_OF_A_PNG));
    assert!(looked.content.contains("Run check_plugin before you finish"));
    assert!(asked_the_impossible.is_error && asked_the_impossible.content.contains("is not something the device can do"), "{}", asked_the_impossible.content);
}

#[test]
fn the_ais_check_of_a_sample_app_reports_each_step_and_shows_the_screen_it_ended_on() {
    let apps = PluginsFolder::empty("sample-check");
    let app_dir = pocket_pads_in(&apps);
    let gate = gate();
    let mut tools = SampleAppTools::open(&app_dir, &gate).unwrap();

    let checked = tools.run("check_plugin", &json!({}));
    let guide = tools.run("read_reference", &json!({ "path": "mp-forge/mpc-sample.md" }));
    let akais_library = tools.run("read_reference", &json!({ "path": "docs/overview.md" }));
    let names: Vec<&str> = tools.definitions().iter().map(|tool| tool.name).collect();

    assert!(checked.content.contains("PASS   Stands up to being played") && checked.content.contains("Verdict: safe"), "{}", checked.content);
    assert!(checked.illustrations[0].png.starts_with(&START_OF_A_PNG));
    assert!(guide.content.contains("# The MPC Sample, and how an app for it is written"));
    assert!(akais_library.is_error, "a Sample app is not shown Akai's library for plugins");
    assert!(!names.contains(&"style_screen") && !names.contains(&"list_fonts") && names.contains(&"look_at_screen"));
}

/// A stand-in for the AI that has nothing to add to the starter, and keeps what it was told and given.
#[derive(Default)]
struct ModelThatIsContent {
    instructions: String,
    first_message: String,
    tools: Vec<&'static str>,
}

impl Model for ModelThatIsContent {
    fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError> {
        self.instructions = brief.instructions.to_string();
        self.tools = brief.tools.iter().map(|tool| tool.name).collect();
        if let Some(Entry::User(message)) = brief.history.first() {
            self.first_message = message.clone();
        }
        Ok(ModelTurn { text: "It is a pad synth.".to_string(), tool_calls: Vec::new(), stop: StopReason::Finished, replay: json!([]), tokens_in_context: None })
    }
}

#[test]
fn a_new_sample_app_is_made_by_the_ai_with_the_rules_and_tools_of_a_sample_app_and_delivered_once_it_passes() {
    let apps = PluginsFolder::empty("sample-made");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let identity = PluginIdentity { name: "Night Bus".to_string(), maker: "Workbench Tests".to_string(), kind: PluginKind::SampleApp };
    let request = PluginRequest { identity, description: "sixteen dusty chords".to_string() };
    let mut model = ModelThatIsContent::default();

    let delivery = Maker::new(sdk, &apps.0).hosting_plugins_with(env!("CARGO_BIN_EXE_workbench")).make(&request, Connection::Model(&mut model), |_| {}).unwrap();

    assert!(delivery.is_ready(), "{}", delivery.report);
    assert_eq!(delivery.closing_words, "It is a pad synth.");
    assert!(model.instructions.starts_with("# Making an app for the MPC Sample in MP Forge"));
    assert!(model.first_message.contains("\"Night Bus\"") && model.first_message.contains("It is an app for the MPC Sample") && model.first_message.contains("sixteen dusty chords"));
    assert!(model.tools.contains(&"look_at_screen") && !model.tools.contains(&"style_screen"));
    assert!(sample_app::library_of(&delivery.plugin_dir).is_file());
}

/// MP Forge's program for the MPC Sample, built for this computer and run here in the device's place: a file
/// for its screen, and a pipe for what its controls send.
#[cfg(unix)]
struct StandInDevice {
    program: std::process::Child,
    screen: PathBuf,
    controls: PathBuf,
    port: u16,
    port_for_apps: u16,
    /// The folder that stands in for where the boot menu looks on the device's card.
    folder_for_apps: PathBuf,
    _folder: PluginsFolder,
}

/// How a stand-in device is started: as one that only links, as one that also takes apps onto its card, or
/// as one whose folder for apps turns out to lie on the device's own storage.
#[cfg(unix)]
#[derive(Clone, Copy, PartialEq)]
enum Takes {
    NoApps,
    Apps,
    AppsButIsNotOnACard,
}

#[cfg(unix)]
impl StandInDevice {
    const BYTES_A_SCREEN: usize = sample_app::SCREEN_WIDTH * sample_app::SCREEN_HEIGHT * 4;

    fn started(label: &str) -> Self {
        Self::started_as_one_that(Takes::NoApps, label)
    }

    fn started_as_one_that(takes: Takes, label: &str) -> Self {
        let folder = PluginsFolder::empty(label);
        let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
        let built = link_program::build_for_this_computer(&sdk, &folder.0, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
        let (screen, controls, folder_for_apps) = (folder.0.join("screen"), folder.0.join("controls"), folder.0.join("card/autorun/mods"));
        fs::write(&screen, vec![0u8; Self::BYTES_A_SCREEN]).unwrap();
        fs::create_dir_all(&folder_for_apps).unwrap();
        assert!(std::process::Command::new("mkfifo").arg(&controls).status().unwrap().success());
        let [port, port_for_apps] = [(); 2].map(|()| std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port());
        let mut started = std::process::Command::new(built);
        started.args([&screen, &controls]).arg(port.to_string());
        if takes != Takes::NoApps {
            started.arg(port_for_apps.to_string()).arg(&folder_for_apps);
        }
        if takes == Takes::AppsButIsNotOnACard {
            started.arg(&folder.0);
        }
        Self { program: started.spawn().unwrap(), screen, controls, port, port_for_apps, folder_for_apps, _folder: folder }
    }

    fn doors(&self) -> Doors {
        Doors::at(self.address(), self.port_for_apps)
    }

    /// Whether the screen asks the musician to let the computer put apps on the card. Only that picture has
    /// writing this far up.
    fn asks_the_musician(&self) -> bool {
        const INK_ON_THE_SCREEN: [u8; 4] = [242, 235, 236, 0];
        const ROWS_OF_THE_QUESTION: std::ops::Range<usize> = 40..50;
        let bytes_a_row = sample_app::SCREEN_WIDTH * 4;
        let shown = fs::read(&self.screen).unwrap();
        shown[ROWS_OF_THE_QUESTION.start * bytes_a_row..ROWS_OF_THE_QUESTION.end * bytes_a_row].chunks_exact(4).any(|dot| dot == INK_ON_THE_SCREEN)
    }

    /// Waits until the musician is asked on the screen, then answers with what the controls send. A musician
    /// who is never asked sends nothing: with no question on the screen, a push of the dial would end the link,
    /// and on a slow computer the question can be a long while coming.
    fn answer_once_asked(&self, sent_by_the_controls: &[u8], is_over: &AtomicBool) {
        while !self.asks_the_musician() {
            if is_over.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.touch(sent_by_the_controls);
    }

    /// What `send` comes to, with the musician answering on the device as soon as they are asked.
    fn with_the_musician_answering<Sent: Send>(&self, sent_by_the_controls: &[u8], send: impl FnOnce() -> Sent + Send) -> Sent {
        let is_over = AtomicBool::new(false);
        std::thread::scope(|both| {
            both.spawn(|| self.answer_once_asked(sent_by_the_controls, &is_over));
            let sent = both.spawn(send).join();
            is_over.store(true, Ordering::Relaxed);
            sent.unwrap()
        })
    }

    /// Doors nothing waits at.
    fn doors_of_nothing(&self) -> Doors {
        let nothing_waits_here = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        Doors::at(nothing_waits_here, nothing_waits_here.port())
    }

    fn names_where_the_boot_menu_looks(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(&self.folder_for_apps).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    fn address(&self) -> std::net::SocketAddr {
        std::net::SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    fn touch(&self, sent_by_the_controls: &[u8]) {
        fs::OpenOptions::new().write(true).open(&self.controls).unwrap().write_all(sent_by_the_controls).unwrap();
    }

    /// The first dot the screen shows once it shows this one, or whatever it shows after ten seconds.
    fn first_dot_once_it_is(&self, awaited: [u8; 4]) -> Vec<u8> {
        let first_dot = || fs::read(&self.screen).unwrap()[..4].to_vec();
        let gives_up = std::time::Instant::now() + Duration::from_secs(10);
        while first_dot() != awaited && std::time::Instant::now() < gives_up {
            std::thread::sleep(Duration::from_millis(50));
        }
        first_dot()
    }

    fn has_ended_within(&mut self, patience: Duration) -> bool {
        let gives_up = std::time::Instant::now() + patience;
        while std::time::Instant::now() < gives_up {
            if self.program.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

#[cfg(unix)]
impl Drop for StandInDevice {
    fn drop(&mut self) {
        let _ = self.program.kill();
        let _ = self.program.wait();
    }
}

#[cfg(unix)]
#[test]
fn a_linked_device_shows_the_apps_picture_and_plays_the_app_with_its_own_controls() {
    const A_DOT: [u8; 4] = [10, 20, 30, 255];
    const THE_DOT_ON_THE_SCREEN: [u8; 4] = [30, 20, 10, 0];
    const PAD_ONE_HIT_THEN_PLAY_PRESSED: [u8; 6] = [0x99, 37, 127, 0x90, 82, 127];
    let device = StandInDevice::started("device-link");
    let (tell, told) = std::sync::mpsc::channel();

    let link = DeviceLink::to(device.address(), move |touched| {
        let _ = tell.send(touched);
    });
    link.show(&Shown { picture: A_DOT.repeat(sample_app::SCREEN_WIDTH * sample_app::SCREEN_HEIGHT), pad_colours: [[0; 3]; sample_app::PADS], lit_buttons: vec!["PLAY"] });
    let shown = device.first_dot_once_it_is(THE_DOT_ON_THE_SCREEN);
    device.touch(&PAD_ONE_HIT_THEN_PLAY_PRESSED);
    let touched: Vec<Input> = (0..2).map(|_| told.recv_timeout(Duration::from_secs(10)).unwrap()).collect();

    assert_eq!(shown, THE_DOT_ON_THE_SCREEN, "the picture reaches the screen in the screen's own order of colours");
    assert_eq!(touched, [Input::pad_hit(1, 1.0), Input::button("PLAY", true).unwrap()]);
}

#[cfg(unix)]
#[test]
fn a_device_nobody_is_linked_to_says_that_it_waits_and_ends_when_its_dial_is_pushed() {
    const THE_BAR_ACROSS_THE_TOP: [u8; 4] = [31, 39, 181, 0];
    const A_PAD_HIT: [u8; 3] = [0x99, 37, 127];
    const THE_DIAL_PUSHED: [u8; 3] = [0x90, 111, 127];
    let mut device = StandInDevice::started("device-link-waiting");

    let shown = device.first_dot_once_it_is(THE_BAR_ACROSS_THE_TOP);
    device.touch(&A_PAD_HIT);
    let ended_by_a_pad = device.has_ended_within(Duration::from_millis(500));
    device.touch(&THE_DIAL_PUSHED);
    let ended_by_the_dial = device.has_ended_within(Duration::from_secs(10));

    assert_eq!(shown, THE_BAR_ACROSS_THE_TOP);
    assert!(!ended_by_a_pad && ended_by_the_dial);
}

#[test]
fn the_folder_for_the_boot_menu_holds_mp_forge_link_built_for_the_mpc_samples_processor() {
    const STARTS_A_LINUX_PROGRAM: &[u8] = b"\x7FELF";
    const WHERE_THE_PROCESSOR_IS_SAID: usize = 18;
    const SIXTY_FOUR_BIT_ARM: u8 = 183;
    let folder = PluginsFolder::empty("device-link-for-the-device");
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));

    let written = link_program::write_folder_for_the_boot_menu(&sdk, &folder.0, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));

    let program = fs::read(written.join("mp-forge-link")).unwrap();
    let mut files: Vec<String> = fs::read_dir(&written).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!(written, folder.0.join("MP Forge Link"));
    assert!(program.starts_with(STARTS_A_LINUX_PROGRAM));
    assert_eq!(program[WHERE_THE_PROCESSOR_IS_SAID], SIXTY_FOUR_BIT_ARM);
    assert!(!String::from_utf8_lossy(&program).contains("ld-musl"), "it asks for no library of the device's");
    assert_eq!(files, ["autorun.json", "mp-forge-link", "sampleboot.txt", "start-mp-forge-link.sh"], "nothing that starts the boot menu itself is among them");
    assert_eq!(fs::read_to_string(written.join("autorun.json")).unwrap(), r#"{"scripts":[{"path":"start-mp-forge-link.sh"}]}"#);
    assert_eq!(fs::read_to_string(written.join("sampleboot.txt")).unwrap(), "MP Forge Link");
}

/// What stands in for the port the MPC Sample's controls speak on: one place a program both reads what the
/// controls send and writes what sets their lights. Here the test is the controls.
#[cfg(unix)]
struct StandInControls {
    this_end: fs::File,
    /// Kept open so that the port stays as it was set up for as long as the test runs.
    _the_programs_end: fs::File,
    port: PathBuf,
}

#[cfg(unix)]
impl StandInControls {
    fn opened() -> Self {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::OpenOptionsExt;
        unsafe {
            let this_end = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            assert!(this_end >= 0 && libc::grantpt(this_end) == 0 && libc::unlockpt(this_end) == 0);
            let port = PathBuf::from(std::ffi::CStr::from_ptr(libc::ptsname(this_end)).to_str().unwrap());
            let the_programs_end = fs::OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY).open(&port).unwrap();
            let mut passes_bytes_as_they_are: libc::termios = std::mem::zeroed();
            assert_eq!(libc::tcgetattr(the_programs_end.as_raw_fd(), &mut passes_bytes_as_they_are), 0);
            libc::cfmakeraw(&mut passes_bytes_as_they_are);
            assert_eq!(libc::tcsetattr(the_programs_end.as_raw_fd(), libc::TCSANOW, &passes_bytes_as_they_are), 0);
            assert_eq!(libc::fcntl(this_end, libc::F_SETFL, libc::O_NONBLOCK), 0);
            Self { this_end: fs::File::from_raw_fd(this_end), _the_programs_end: the_programs_end, port }
        }
    }

    fn send(&mut self, sent_by_the_controls: &[u8]) {
        self.this_end.write_all(sent_by_the_controls).unwrap();
    }

    /// Everything the program has sent to the controls since the last time, once that ends with `awaited`,
    /// or whatever it has sent after ten seconds.
    fn sent_to_them_once_it_ends_with(&mut self, awaited: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let gives_up = std::time::Instant::now() + Duration::from_secs(10);
        let mut sent = Vec::new();
        while !sent.ends_with(awaited) && std::time::Instant::now() < gives_up {
            let mut arrived = [0u8; 1024];
            match self.this_end.read(&mut arrived) {
                Ok(count) => sent.extend(&arrived[..count]),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        sent
    }
}

/// An app built with MP Forge's program that runs it on the device, run here in the device's place: a file
/// for its screen, a file its sound is written to, and a folder for its card.
#[cfg(unix)]
struct StandInSample {
    program: std::process::Child,
    controls: StandInControls,
    screen: PathBuf,
    sound: PathBuf,
    notes: PathBuf,
}

#[cfg(unix)]
impl StandInSample {
    const BYTES_A_SCREEN: usize = sample_app::SCREEN_WIDTH * sample_app::SCREEN_HEIGHT * 4;
    const LOUDEST: f32 = 2_147_483_647.0;

    /// Starts the app in `app_dir` with the sound files in `sounds` and the card in `card`.
    fn running(app_dir: &Path, sounds: &Path, card: &Path) -> Self {
        let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
        let built = device_app::build_for_this_computer(&sdk, app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
        let controls = StandInControls::opened();
        let [screen, sound, notes] = ["screen", "sound", "notes"].map(|file| app_dir.join(file));
        fs::write(&screen, vec![0u8; Self::BYTES_A_SCREEN]).unwrap();
        fs::write(&sound, []).unwrap();
        let places = [screen.as_path(), controls.port.as_path(), sound.as_path(), Path::new("/dev/null"), sounds, card];
        let program = std::process::Command::new(built).args(places).stderr(fs::File::create(&notes).unwrap()).spawn().unwrap();
        Self { program, controls, screen, sound, notes }
    }

    /// The dot at the top left of the screen once it is this one, or whatever it is after ten seconds.
    fn first_dot_once_it_is(&self, awaited: [u8; 4]) -> Vec<u8> {
        let first_dot = || fs::read(&self.screen).unwrap()[..4].to_vec();
        let gives_up = std::time::Instant::now() + Duration::from_secs(10);
        while first_dot() != awaited && std::time::Instant::now() < gives_up {
            std::thread::sleep(Duration::from_millis(50));
        }
        first_dot()
    }

    /// How loud the loudest sound is that the app makes in the next stretch of time, from 0 to 1.
    fn loudest_in_the_next(&self, stretch: Duration) -> f32 {
        let heard_before = fs::metadata(&self.sound).unwrap().len() as usize;
        std::thread::sleep(stretch);
        let heard = fs::read(&self.sound).unwrap();
        let numbers = heard[heard_before - heard_before % 4..].chunks_exact(4).map(|number| i32::from_le_bytes([number[0], number[1], number[2], number[3]]));
        numbers.fold(0.0, |loudest, number| (number as f32 / Self::LOUDEST).abs().max(loudest))
    }

    fn has_ended_within(&mut self, patience: Duration) -> bool {
        let gives_up = std::time::Instant::now() + patience;
        while std::time::Instant::now() < gives_up {
            if self.program.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    fn notes(&self) -> String {
        fs::read_to_string(&self.notes).unwrap()
    }
}

#[cfg(unix)]
impl Drop for StandInSample {
    fn drop(&mut self) {
        let _ = self.program.kill();
        let _ = self.program.wait();
    }
}

#[cfg(unix)]
const ASKS_WHERE_THE_KNOBS_STAND: [u8; 8] = [0xF0, 0x47, 0x7F, 0x5C, 0x42, 0x00, 0x00, 0xF7];
#[cfg(unix)]
const PAD_ONE_HIT: [u8; 3] = [0x99, 37, 127];
#[cfg(unix)]
const PLAY_PRESSED: [u8; 3] = [0x90, 82, 127];
#[cfg(unix)]
const SHIFT_AND_STOP_HELD: [u8; 6] = [0x90, 49, 127, 0x90, 81, 127];
#[cfg(unix)]
const MAIN_VOLUME_ALL_THE_WAY_UP: [u8; 3] = [0xB0, 19, 127];
#[cfg(unix)]
const LAST_LIGHT_SET: [u8; 3] = [0xB0, 82, 0];

/// What Pocket Pads shows on a computer before anything is touched, and once PLAY is pressed.
#[cfg(unix)]
fn shown_by_pocket_pads_on_a_computer(app_dir: &Path) -> (Shown, Shown) {
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let library = sample_app::build_for_this_computer(&sdk, app_dir, Duration::from_secs(300)).unwrap_or_else(|not_built| panic!("{not_built}"));
    let mut app = RunningApp::load(&library, 44_100.0, Vec::new()).unwrap();
    let at_rest = app.show();
    app.tell(Input::button("PLAY", true).unwrap());
    (at_rest, app.show())
}

#[cfg(unix)]
#[test]
fn on_the_device_an_app_asks_the_knobs_where_they_stand_and_shows_its_picture_and_lights_as_on_a_computer() {
    const THE_TITLE_BAR_ON_THE_SCREEN: [u8; 4] = [31, 39, 181, 0];
    let apps = PluginsFolder::empty("device-app-shows");
    let app_dir = pocket_pads_in(&apps);
    let (at_rest, with_play_pressed) = shown_by_pocket_pads_on_a_computer(&app_dir);
    let lights_at_rest = Lights::of(&at_rest).bytes_to_set_from(None);
    let lights_that_change = Lights::of(&with_play_pressed).bytes_to_set_from(Some(&Lights::of(&at_rest)));

    let mut device = StandInSample::running(&app_dir, &apps.0.join("no-sounds"), &apps.0.join("no-card"));
    let sent_at_the_start = device.controls.sent_to_them_once_it_ends_with(&LAST_LIGHT_SET);
    let shown = device.first_dot_once_it_is(THE_TITLE_BAR_ON_THE_SCREEN);
    device.controls.send(&PLAY_PRESSED);
    let sent_once_play_is_pressed = device.controls.sent_to_them_once_it_ends_with(&lights_that_change);

    assert_eq!(sent_at_the_start, [&ASKS_WHERE_THE_KNOBS_STAND[..], &lights_at_rest].concat(), "the knobs are asked first, then every light is set as a computer sets it");
    assert_eq!(shown, THE_TITLE_BAR_ON_THE_SCREEN, "the picture reaches the screen in the screen's own order of colours");
    assert_eq!(sent_once_play_is_pressed, lights_that_change, "after that only a light that changed is set");
    assert_eq!(lights_that_change, [0xB0, 82, 2]);
}

#[cfg(unix)]
#[test]
fn on_the_device_an_app_is_heard_as_loud_as_the_main_volume_says_and_is_left_by_holding_shift_and_stop() {
    const ONLY_SHIFT_HELD: [u8; 3] = [0x90, 49, 127];
    const STOP_HELD_TOO: [u8; 3] = [0x90, 81, 127];
    let apps = PluginsFolder::empty("device-app-sound");
    let mut device = StandInSample::running(&pocket_pads_in(&apps), &apps.0.join("no-sounds"), &apps.0.join("no-card"));
    device.controls.sent_to_them_once_it_ends_with(&LAST_LIGHT_SET);

    let before_a_hit = device.loudest_in_the_next(Duration::from_millis(200));
    device.controls.send(&PAD_ONE_HIT);
    let at_the_volume_it_starts_with = device.loudest_in_the_next(Duration::from_millis(300));
    device.controls.send(&[MAIN_VOLUME_ALL_THE_WAY_UP, PAD_ONE_HIT].concat());
    let at_full_volume = device.loudest_in_the_next(Duration::from_millis(300));
    device.controls.send(&ONLY_SHIFT_HELD);
    let left_by_shift_alone = device.has_ended_within(Duration::from_millis(1500));
    device.controls.send(&STOP_HELD_TOO);
    let left_by_both = device.has_ended_within(Duration::from_secs(10));

    assert_eq!(before_a_hit, 0.0, "nothing is heard until a pad is hit");
    assert!(at_the_volume_it_starts_with > 0.01, "a hit pad is heard, at {at_the_volume_it_starts_with}");
    assert!(at_full_volume > at_the_volume_it_starts_with * 2.0 && at_full_volume <= 1.0, "{at_the_volume_it_starts_with} then {at_full_volume}");
    assert!(!left_by_shift_alone && left_by_both, "{}", device.notes());
    assert!(device.controls.sent_to_them_once_it_ends_with(&LAST_LIGHT_SET).ends_with(&LAST_LIGHT_SET), "every light is put out on the way out");
}

#[cfg(unix)]
#[test]
fn on_the_device_an_app_plays_the_sound_files_added_to_it_and_loads_more_from_the_card_itself() {
    const FADER_ALL_THE_WAY_UP: [u8; 3] = [0xB0, 20, 127];
    const PAD_THREE_HIT: [u8; 6] = [0x89, 42, 0, 0x99, 42, 127];
    const SAMPLE_SELECT_PRESSED: [u8; 3] = [0x90, 36, 127];
    const DIAL_TURNED_TO_THE_THIRD_ROW: [u8; 3] = [0xB0, 100, 2];
    const DIAL_PUSHED: [u8; 3] = [0x90, 111, 127];
    const TIME_TO_READ_THE_CARD: Duration = Duration::from_millis(400);
    let apps = PluginsFolder::empty("device-app-files");
    let app_dir = sample_pads_in(&apps);
    let its_sounds = apps.0.join("sounds");
    SampleShelf::at(&its_sounds).add("kick.wav", &loud_tone(0.2)).unwrap();
    write_trial_card(&apps.0.join("card")).unwrap();
    let mut device = StandInSample::running(&app_dir, &its_sounds, &apps.0.join("card"));
    device.controls.sent_to_them_once_it_ends_with(&LAST_LIGHT_SET);
    device.controls.send(&[MAIN_VOLUME_ALL_THE_WAY_UP, FADER_ALL_THE_WAY_UP].concat());

    device.controls.send(&PAD_ONE_HIT);
    let the_sound_added_to_the_app = device.loudest_in_the_next(Duration::from_millis(300));
    device.controls.send(&PAD_THREE_HIT);
    let a_pad_with_no_sound = device.loudest_in_the_next(Duration::from_millis(600));
    for touched_on_the_cards_page in [SAMPLE_SELECT_PRESSED, DIAL_TURNED_TO_THE_THIRD_ROW, DIAL_PUSHED] {
        device.controls.send(&touched_on_the_cards_page);
        std::thread::sleep(TIME_TO_READ_THE_CARD);
    }
    device.controls.send(&PAD_THREE_HIT);
    let the_file_loaded_from_the_card = device.loudest_in_the_next(Duration::from_millis(300));

    assert!(the_sound_added_to_the_app > 0.3, "the pad plays the sound added to the app, at {the_sound_added_to_the_app}");
    assert_eq!(a_pad_with_no_sound, 0.0);
    assert!(the_file_loaded_from_the_card > 0.3, "the pad plays tone.wav from the card, at {the_file_loaded_from_the_card}\n{}", device.notes());
}

/// A WAV file of a steady tone at most of full loudness.
#[cfg(unix)]
fn loud_tone(seconds: f32) -> Vec<u8> {
    let frames = (seconds * 44_100.0) as usize;
    let numbers: Vec<i16> = (0..frames).map(|frame| ((frame as f32 * 0.05).sin() * 0.8 * f32::from(i16::MAX)) as i16).collect();
    sixteen_bit_wav(1, 44_100, &numbers)
}

#[cfg(unix)]
#[test]
fn on_the_device_an_app_that_stops_answering_is_left_so_the_device_is_never_stuck_in_it() {
    const AN_APP_THAT_HANGS_WHEN_A_PAD_IS_HIT: &str = r#"
        #include "mpc_sample_app.h"
        class Hangs : public sample::App {
        public:
            void padHit(int, float) override { for (;;) stuck_ = stuck_ + 1; }
            void makeSound(sample::Sound& sound) override { sound.silence(); }
            void draw(sample::Screen& screen, sample::Lights&) override { screen.clear({0, 0, 0}); }
        private:
            volatile int stuck_ = 0;
        };
        MPC_SAMPLE_APP(Hangs)
    "#;
    let apps = PluginsFolder::empty("device-app-hangs");
    let app_dir = apps.0.join("Hangs");
    fs::create_dir_all(app_dir.join("src")).unwrap();
    fs::write(app_dir.join("src/Hangs.cpp"), AN_APP_THAT_HANGS_WHEN_A_PAD_IS_HIT).unwrap();
    let mut device = StandInSample::running(&app_dir, &apps.0.join("no-sounds"), &apps.0.join("no-card"));
    device.controls.sent_to_them_once_it_ends_with(&LAST_LIGHT_SET);

    let left_while_it_answers = device.has_ended_within(Duration::from_secs(1));
    device.controls.send(&[PAD_ONE_HIT.as_slice(), &SHIFT_AND_STOP_HELD].concat());
    let left_once_it_hangs = device.has_ended_within(Duration::from_secs(15));

    assert!(!left_while_it_answers && left_once_it_hangs);
    assert!(device.notes().contains("The app stopped answering, so it was left."), "{}", device.notes());
}

#[test]
fn an_app_for_the_device_is_one_program_for_its_processor_in_a_folder_the_boot_menu_can_start() {
    const STARTS_A_LINUX_PROGRAM: &[u8] = b"\x7FELF";
    const WHERE_THE_PROCESSOR_IS_SAID: usize = 18;
    const SIXTY_FOUR_BIT_ARM: u8 = 183;
    let apps = PluginsFolder::empty("device-app-for-the-device");
    let app_dir = sample_pads_in(&apps);
    SampleShelf::of_plugin(&app_dir).add("kick.wav", &sixteen_bit_wav(1, 44_100, &[0, 1_000, 0, -1_000])).unwrap();
    let where_the_boot_menu_looks = apps.0.join("card/autorun/mods");
    fs::create_dir_all(&where_the_boot_menu_looks).unwrap();
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));

    let made = device_app::write_for_the_card_picked(&sdk, &app_dir, &apps.0.join("card"), Duration::from_secs(300)).unwrap_or_else(|not_made| panic!("{not_made}"));

    let program = fs::read(made.folder.join("app")).unwrap();
    let mut files: Vec<String> = fs::read_dir(&made.folder).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    files.sort();
    assert_eq!((made.folder.as_path(), made.is_where_the_boot_menu_looks), (where_the_boot_menu_looks.join("Sample Pads").as_path(), true));
    assert!(program.starts_with(STARTS_A_LINUX_PROGRAM));
    assert_eq!(program[WHERE_THE_PROCESSOR_IS_SAID], SIXTY_FOUR_BIT_ARM);
    assert!(!String::from_utf8_lossy(&program).contains("ld-musl"), "it asks for no library of the device's");
    assert_eq!(files, ["app", "autorun.json", "sampleboot.txt", "sounds", "start-app.sh"], "nothing that starts the boot menu itself is among them");
    assert_eq!(fs::read_to_string(made.folder.join("autorun.json")).unwrap(), r#"{"scripts":[{"path":"start-app.sh"}]}"#);
    assert_eq!(fs::read_to_string(made.folder.join("sampleboot.txt")).unwrap(), "Sample Pads");
    assert!(made.folder.join("sounds/kick.wav").is_file());
    assert_eq!(fs::read_dir(&where_the_boot_menu_looks).unwrap().count(), 1, "nothing is left beside the app's folder");
}

#[cfg(unix)]
const THE_DIAL_PUSHED: [u8; 3] = [0x90, 111, 127];
#[cfg(unix)]
const THE_WAITING_PICTURES_BAR: [u8; 4] = [31, 39, 181, 0];

/// The folder of an app called Night Bus as it goes to a device, with a start script that only notes what it
/// was told, in the place of one that starts a program built for the device.
#[cfg(unix)]
fn folder_of_night_bus_in(holder: &PluginsFolder) -> PathBuf {
    const NOTES_WHAT_IT_WAS_TOLD: &str = "#!/bin/sh\necho \"$1\" > \"$(dirname -- \"$0\")/was-started.txt\"\n";
    let folder = holder.0.join("Night Bus");
    fs::create_dir_all(folder.join("sounds")).unwrap();
    fs::write(folder.join("app"), vec![7u8; 200_000]).unwrap();
    fs::write(folder.join("start-app.sh"), NOTES_WHAT_IT_WAS_TOLD).unwrap();
    fs::write(folder.join("sampleboot.txt"), "Night Bus").unwrap();
    fs::write(folder.join("sounds/kick.wav"), sixteen_bit_wav(1, 44_100, &[0, 1_000, 0, -1_000])).unwrap();
    folder
}

/// What the stand-in start script of an app noted it was told, once it has run, or nothing after ten seconds.
#[cfg(unix)]
fn what_the_start_script_was_told_in(on_the_card: &Path) -> String {
    let gives_up = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        match fs::read_to_string(on_the_card.join("was-started.txt")) {
            Ok(told) if !told.is_empty() => return told,
            _ if std::time::Instant::now() > gives_up => return String::new(),
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// A computer at a stand-in device's door for apps, asking one thing at a time by its bytes.
#[cfg(unix)]
struct AtTheDoorForApps(std::net::TcpStream);

#[cfg(unix)]
impl AtTheDoorForApps {
    fn of(device: &StandInDevice) -> Self {
        use std::io::Read;
        let mut link = std::net::TcpStream::connect(("127.0.0.1", device.port_for_apps)).unwrap();
        let mut greeting = [0u8; 21];
        link.read_exact(&mut greeting).unwrap();
        assert_eq!(&greeting, b"MP FORGE LINK APPS 1\n");
        link.write_all(b"MP FORGE SENDS AN APP\n").unwrap();
        Self(link)
    }

    /// The link's answer to what is asked.
    fn ask(&mut self, asked: &[u8]) -> char {
        use std::io::Read;
        let mut answer = [0u8; 1];
        self.0.write_all(asked).unwrap();
        self.0.read_exact(&mut answer).unwrap();
        char::from(answer[0])
    }
}

#[cfg(unix)]
#[test]
fn an_app_sent_over_the_link_lands_where_the_boot_menu_looks_and_runs_until_it_is_left() {
    let holder = PluginsFolder::empty("app-sent-by-link");
    let folder = folder_of_night_bus_in(&holder);
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-takes-an-app");
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);

    let sent = device.with_the_musician_answering(&THE_DIAL_PUSHED, || app_sending::send_and_run(&device.doors(), &folder));
    let on_the_card = device.folder_for_apps.join("Night Bus");
    let told_when_first_run = what_the_start_script_was_told_in(&on_the_card);
    let sent_again_without_being_asked = app_sending::send_and_run(&device.doors(), &folder);
    let told_when_run_again = what_the_start_script_was_told_in(&on_the_card);

    assert!(sent.is_ok(), "{sent:?}");
    assert_eq!(device.names_where_the_boot_menu_looks(), ["Night Bus"], "nothing is left beside the app's folder");
    assert_eq!(fs::read(on_the_card.join("app")).unwrap(), fs::read(folder.join("app")).unwrap());
    assert_eq!(fs::read(on_the_card.join("sounds/kick.wav")).unwrap(), fs::read(folder.join("sounds/kick.wav")).unwrap());
    assert_eq!(told_when_first_run, "from-mp-forge-link\n", "the app's script is told the link started it");
    assert_eq!(told_when_run_again, told_when_first_run);
    assert!(sent_again_without_being_asked.is_ok(), "the musician is asked once each time the link starts: {sent_again_without_being_asked:?}");
    assert_eq!(device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR), THE_WAITING_PICTURES_BAR, "the link is back once the app is left");
}

#[cfg(unix)]
#[test]
fn an_app_is_kept_off_the_card_when_the_musician_says_no_on_the_device() {
    const PLAY_PRESSED: [u8; 3] = [0x90, 82, 127];
    let holder = PluginsFolder::empty("app-not-allowed");
    let folder = folder_of_night_bus_in(&holder);
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-says-no");
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);

    let sent = device.with_the_musician_answering(&PLAY_PRESSED, || app_sending::send_and_run(&device.doors(), &folder));

    assert!(matches!(sent, Err(NotSent::Refused(Refusal::NotAllowed))), "{sent:?}");
    assert_eq!(device.names_where_the_boot_menu_looks(), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn a_sending_that_breaks_off_leaves_no_half_an_app_on_the_card() {
    const A_FILE_OF_A_THOUSAND_BYTES: &[u8] = b"F\x03\x00app\xe8\x03\x00\x00\x00\x00\x00\x00";
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-sending-breaks-off");
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);
    let mut computer = AtTheDoorForApps::of(&device);

    let begun = device.with_the_musician_answering(&THE_DIAL_PUSHED, || computer.ask(b"B\x09Night Bus"));
    let while_it_arrives = device.names_where_the_boot_menu_looks();
    computer.0.write_all(&[A_FILE_OF_A_THOUSAND_BYTES, b"only this much"].concat()).unwrap();
    drop(computer);
    let gives_up = std::time::Instant::now() + Duration::from_secs(10);
    while !device.names_where_the_boot_menu_looks().is_empty() && std::time::Instant::now() < gives_up {
        std::thread::sleep(Duration::from_millis(50));
    }

    assert_eq!(begun, 'K');
    assert_eq!(while_it_arrives, ["_Night Bus.arriving"], "an app arrives in a folder the boot menu does not list");
    assert_eq!(device.names_where_the_boot_menu_looks(), Vec::<String>::new());
}

#[cfg(unix)]
#[test]
fn the_link_writes_nowhere_but_inside_its_folder_for_apps() {
    const A_FILE_THAT_WOULD_LEAVE_THE_FOLDER: &[u8] = b"F\x0d\x00../escape.txt\x00\x00\x00\x00\xc5\x9d\x1c\x81";
    const A_FILE_THAT_NAMES_THE_CARDS_TOP: &[u8] = b"F\x0b\x00/escape.txt\x00\x00\x00\x00\xc5\x9d\x1c\x81";
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-keeps-to-its-folder");
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);
    let beside_the_folder = || fs::read_dir(device.folder_for_apps.parent().unwrap()).unwrap().count();
    let beside_it_before = beside_the_folder();

    let a_name_that_leaves = AtTheDoorForApps::of(&device).ask(b"B\x09../Escape");
    let a_name_with_a_way_in_it = AtTheDoorForApps::of(&device).ask(b"B\x07mods/In");
    let a_name_the_menu_hides = AtTheDoorForApps::of(&device).ask(b"B\x07_Hidden");
    let mut computer = AtTheDoorForApps::of(&device);
    let begun = device.with_the_musician_answering(&THE_DIAL_PUSHED, || computer.ask(b"B\x09Night Bus"));
    let a_path_that_leaves = computer.ask(A_FILE_THAT_WOULD_LEAVE_THE_FOLDER);
    let mut computer = AtTheDoorForApps::of(&device);
    let begun_again = computer.ask(b"B\x09Night Bus");
    let a_path_from_the_top = computer.ask(A_FILE_THAT_NAMES_THE_CARDS_TOP);
    let run_what_is_not_there = AtTheDoorForApps::of(&device).ask(b"R\x09Night Bus");

    assert_eq!([a_name_that_leaves, a_name_with_a_way_in_it, a_name_the_menu_hides], ['B', 'B', 'B']);
    assert_eq!([begun, a_path_that_leaves, begun_again, a_path_from_the_top], ['K', 'B', 'K', 'B']);
    assert_eq!(run_what_is_not_there, 'S');
    assert_eq!(beside_the_folder(), beside_it_before, "nothing was written beside the folder for apps");
    assert!(!device.folder_for_apps.join("Night Bus").exists(), "an app that did not arrive whole has no folder");
}

#[cfg(unix)]
#[test]
fn a_folder_on_the_card_that_mp_forge_did_not_make_is_never_written_over_or_run() {
    let holder = PluginsFolder::empty("app-name-taken");
    let folder = folder_of_night_bus_in(&holder);
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-name-taken");
    let somebody_elses = device.folder_for_apps.join("Night Bus");
    fs::create_dir_all(&somebody_elses).unwrap();
    fs::write(somebody_elses.join("autorun.sh"), "#!/bin/sh\n").unwrap();
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);

    let sent = app_sending::send_and_run(&device.doors(), &folder);
    let asked_to_run_it = AtTheDoorForApps::of(&device).ask(b"R\x09Night Bus");

    assert!(matches!(sent, Err(NotSent::Refused(Refusal::NameIsTaken))), "{sent:?}");
    assert_eq!(asked_to_run_it, 'T');
    assert_eq!(fs::read_dir(&somebody_elses).unwrap().count(), 1, "the folder is as it was");
}

#[cfg(unix)]
#[test]
fn a_link_that_is_not_on_a_card_takes_no_apps_and_one_from_before_apps_is_told_from_no_link() {
    let holder = PluginsFolder::empty("app-no-card");
    let folder = folder_of_night_bus_in(&holder);
    let not_on_a_card = StandInDevice::started_as_one_that(Takes::AppsButIsNotOnACard, "device-not-on-a-card");
    let from_before_apps = StandInDevice::started("device-from-before-apps");
    not_on_a_card.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);
    from_before_apps.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);

    let sent_to_the_devices_own_storage = app_sending::send_and_run(&not_on_a_card.doors(), &folder);
    let sent_to_an_older_link = app_sending::send_and_run(&from_before_apps.doors(), &folder);
    drop(from_before_apps);
    let sent_to_nothing = app_sending::send_and_run(&not_on_a_card.doors_of_nothing(), &folder);

    assert!(matches!(sent_to_the_devices_own_storage, Err(NotSent::Refused(Refusal::NotOnACard))), "{sent_to_the_devices_own_storage:?}");
    assert_eq!(not_on_a_card.names_where_the_boot_menu_looks(), Vec::<String>::new());
    assert!(matches!(sent_to_an_older_link, Err(NotSent::OlderLink)), "{sent_to_an_older_link:?}");
    assert!(matches!(sent_to_nothing, Err(NotSent::NoDeviceLinked)), "{sent_to_nothing:?}");
}

/// Pocket Pads with a line added that starts another program, which no app may do.
#[cfg(unix)]
fn pocket_pads_that_start_a_program_in(apps: &PluginsFolder) -> &'static str {
    const NAME: &str = "Shell Pads";
    let (safe, changed) = (pocket_pads_in(apps), apps.0.join(NAME));
    fs::create_dir_all(changed.join("src")).unwrap();
    fs::copy(safe.join("plugin.json"), changed.join("plugin.json")).unwrap();
    let code = fs::read_to_string(safe.join("src/PocketPads.cpp")).unwrap();
    fs::write(changed.join("src/PocketPads.cpp"), format!("#include <cstdlib>\nstatic int started = system(\"true\");\n{code}")).unwrap();
    NAME
}

#[cfg(unix)]
#[test]
fn an_app_that_passes_the_safety_check_is_built_for_the_device_sent_over_the_link_and_run_there() {
    let apps = PluginsFolder::empty("maker-sends-an-app");
    pocket_pads_in(&apps);
    let unsafe_app = pocket_pads_that_start_a_program_in(&apps);
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let maker = Maker::new(sdk, &apps.0).hosting_plugins_with(env!("CARGO_BIN_EXE_workbench"));
    let device = StandInDevice::started_as_one_that(Takes::Apps, "device-takes-a-real-app");
    device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR);

    let sent = device.with_the_musician_answering(&THE_DIAL_PUSHED, || maker.send_sample_app_by_link("Pocket Pads", &device.doors()));
    let held_back = maker.send_sample_app_by_link(unsafe_app, &device.doors());
    let with_no_device = maker.send_sample_app_by_link("Pocket Pads", &device.doors_of_nothing());

    let on_the_card = device.folder_for_apps.join("Pocket Pads");
    assert!(matches!(sent, Ok(SentToTheSample::Sent(_))), "{:?}", sent.err().map(|not_sent| not_sent.to_string()));
    assert!(matches!(held_back, Ok(SentToTheSample::HeldBack(report)) if !report.may_be_sent()));
    assert!(with_no_device.is_err_and(|not_sent| not_sent.to_string().starts_with("No MPC Sample is linked.")));
    assert_eq!(device.names_where_the_boot_menu_looks(), ["Pocket Pads"], "the app the check stopped is not on the card");
    assert_eq!(fs::read(on_the_card.join("app")).unwrap(), fs::read(apps.0.join("Pocket Pads/build/device/app")).unwrap());
    assert!(on_the_card.join("start-app.sh").is_file() && on_the_card.join("autorun.json").is_file());
    assert_eq!(device.first_dot_once_it_is(THE_WAITING_PICTURES_BAR), THE_WAITING_PICTURES_BAR, "the link is back once the app's script has ended");
}

#[test]
fn an_app_goes_onto_a_picked_card_only_once_it_has_passed_the_safety_check() {
    let apps = PluginsFolder::empty("maker-writes-an-app-to-a-card");
    pocket_pads_in(&apps);
    let where_the_boot_menu_looks = apps.0.join("card/autorun/mods");
    fs::create_dir_all(&where_the_boot_menu_looks).unwrap();
    let sdk = Sdk::at(sdk_dir()).unwrap_or_else(|not_found| panic!("{not_found}"));
    let maker = Maker::new(sdk, &apps.0).hosting_plugins_with(env!("CARGO_BIN_EXE_workbench"));
    fs::create_dir_all(apps.0.join("Touch Plugin")).unwrap();
    fs::write(apps.0.join("Touch Plugin/plugin.json"), r#"{ "name": "Touch Plugin" }"#).unwrap();

    let written = maker.put_sample_app_on_card("Pocket Pads", &apps.0.join("card"));
    let a_plugin_for_other_mpcs = maker.put_sample_app_on_card("Touch Plugin", &apps.0.join("card"));

    let Ok(PutOnACard::Written { report, made }) = written else { panic!("{:?}", written.err().map(|not_written| not_written.to_string())) };
    assert!(report.may_be_sent() && made.is_where_the_boot_menu_looks);
    assert_eq!(made.folder, where_the_boot_menu_looks.join("Pocket Pads"));
    assert!(made.folder.join("app").is_file());
    assert!(a_plugin_for_other_mpcs.is_err_and(|refused| refused.to_string().contains("Only an app made for the MPC Sample")));
}
