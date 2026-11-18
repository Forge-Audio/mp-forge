//! Runs the gate on real plugins: Akai's examples must pass, and deliberately broken plugins must be blocked.
//!
//! These tests compile C++ and need Akai's SDK in `vendor/`. Run `scripts/check-sdk.sh` once to fetch it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;
use workbench_engine::agent::{ToolOutput, Toolbox};
use workbench_engine::artwork::{built_control_number, draw_pictures};
use workbench_engine::beat_loop::beat_loop;
use workbench_engine::built_knobs::KNOB_FILE;
use workbench_engine::gate::{
    BUILD_CHECK, Gate, GateReport, MOVING_PICTURES_CHECK, Outcome, PICTURES_CHECK, SCREEN_CHECK, SOURCE_CHECK, TimeLimits, simulation_check_name,
};
use workbench_engine::hosted_plugin::{HostedPlugin, LARGEST_BLOCK, Note, moves_made_by_the_plugin};
use workbench_engine::library::built_parameters;
use workbench_engine::online::{ONLINE_FILE, ONLINE_IS_SWITCHED_OFF};
use workbench_engine::open_mpc_listing::{self, License, Listing};
use workbench_engine::live_player::Rig;
use workbench_engine::picture_clock::{CLOCK_CONTROL, turn_seconds_of};
use workbench_engine::picture_review::HOW_TO_READ_THE_PICTURES;
use workbench_engine::pictures::pictures_of;
use workbench_engine::plugin_tools::PluginTools;
use workbench_engine::project_file::{pack, take_in};
use workbench_engine::sdk::{Sdk, built_screen_file, native_library_of};
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
