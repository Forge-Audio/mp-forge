//! The safety gate: every check a plugin must pass before it may be sent to an MPC.
//!
//! Checks run in order and stop at the first one that blocks, because a plugin's code is only
//! run on this computer (by the build and the simulator) after its source has been scanned.

use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artwork;
use crate::built_screen::SizedArt;
use crate::picture_clock::{self, CLOCK_CONTROL};
use crate::pictures::pictures_of;
use crate::process::{Finished, RunError};
use crate::sample_app;
use crate::sample_trial;
use crate::library;
use crate::screen_layout::{self, ScreenLayout};
use crate::sdk::{Sdk, native_library_of};
use crate::simulation::{Session, SimulationReport};
use crate::online;
use crate::source_scan::{Violation, scan_plugin};

pub const SOURCE_CHECK: &str = "Code stays inside the plugin";
pub const PICTURES_CHECK: &str = "Pictures can be drawn";
pub const BUILD_CHECK: &str = "Builds for every MPC";
pub const SCREEN_CHECK: &str = "Screen loads with its pictures";
pub const MOVING_PICTURES_CHECK: &str = "Moving pictures move";
pub const APP_SOURCE_CHECK: &str = "Code stays inside the app";
pub const APP_BUILD_CHECK: &str = "Builds";
pub const APP_TRIAL_CHECK: &str = "Stands up to being played";

const BUILD_LOG_LINES_SHOWN: usize = 15;
const NO_COPY_TO_TEST: &str = "The build left no copy of the plugin to test.";
const VALIDATOR_ERROR_MARK: &str = "ERROR";

/// Eight notes at once, from low to high: the most an instrument is likely to be asked to play.
const FULL_CHORD: &str = "36,48,55,60,64,67,72,76";

const SESSIONS: [Session; 7] = [
    Session::EVERYDAY,
    Session { name: "a full chord", notes: FULL_CHORD, seconds: 4, seed: 6, ..Session::EVERYDAY },
    Session { name: "a playing song", transport_playing: true, seed: 77, ..Session::EVERYDAY },
    Session { name: "the smallest buffer", block_size: 32, seed: 2, ..Session::EVERYDAY },
    Session { name: "the largest buffer", block_size: 512, seed: 3, ..Session::EVERYDAY },
    Session { name: "a 48 kHz project", sample_rate: 48_000, seed: 4, ..Session::EVERYDAY },
    Session { name: "a long session", seconds: 30, seed: 5, ..Session::EVERYDAY },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    PassedWithWarnings(Vec<String>),
    Blocked(Vec<String>),
    /// An earlier check blocked, so this one was never started.
    NotRun,
}

impl Outcome {
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Passed => "PASS",
            Outcome::PassedWithWarnings(_) => "WARN",
            Outcome::Blocked(_) => "BLOCK",
            Outcome::NotRun => "SKIP",
        }
    }

    pub fn details(&self) -> &[String] {
        match self {
            Outcome::PassedWithWarnings(details) | Outcome::Blocked(details) => details,
            Outcome::Passed | Outcome::NotRun => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub outcome: Outcome,
}

impl fmt::Display for Check {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "  {:<6} {}", self.outcome.label(), self.name)?;
        for line in self.outcome.details().iter().flat_map(|detail| detail.lines()) {
            writeln!(formatter, "           {line}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct GateReport {
    pub checks: Vec<Check>,
}

impl fmt::Display for GateReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Safety check")?;
        for check in &self.checks {
            check.fmt(formatter)?;
        }
        write!(formatter, "{}", self.verdict())
    }
}

impl GateReport {
    pub fn verdict(&self) -> &'static str {
        if self.may_be_sent() {
            "Verdict: safe to send to an MPC."
        } else {
            "Verdict: NOT safe to send. Fix the blocked check first."
        }
    }

    pub fn may_be_sent(&self) -> bool {
        let passed = |check: &Check| matches!(check.outcome, Outcome::Passed | Outcome::PassedWithWarnings(_));
        !self.checks.is_empty() && self.checks.iter().all(passed)
    }

    pub fn outcome_of(&self, check_name: &str) -> Option<&Outcome> {
        self.checks.iter().find(|check| check.name == check_name).map(|check| &check.outcome)
    }

    fn is_blocked(&self) -> bool {
        self.checks.iter().any(|check| matches!(check.outcome, Outcome::Blocked(_)))
    }

    fn record(&mut self, name: impl Into<String>, check: impl FnOnce() -> Outcome) {
        let outcome = if self.is_blocked() { Outcome::NotRun } else { check() };
        self.checks.push(Check { name: name.into(), outcome });
    }
}

pub struct TimeLimits {
    pub build: Duration,
    pub simulation: Duration,
}

impl Default for TimeLimits {
    fn default() -> Self {
        Self { build: Duration::from_secs(300), simulation: Duration::from_secs(60) }
    }
}

pub struct Gate {
    sdk: Sdk,
    limits: TimeLimits,
    /// The program started to run a plugin's own code, so that it never runs inside this one.
    plugin_host: PathBuf,
    /// Whether the musician has switched online plugins on. Without that, a plugin that goes online is blocked.
    online_is_switched_on: bool,
}

impl Gate {
    /// Plugins are hosted by another run of the program that made the gate, which answers `read-clock`.
    pub fn new(sdk: Sdk) -> Self {
        Self { sdk, limits: TimeLimits::default(), plugin_host: env::current_exe().unwrap_or_default(), online_is_switched_on: false }
    }

    pub fn hosting_plugins_with(mut self, program: impl Into<PathBuf>) -> Self {
        self.plugin_host = program.into();
        self
    }

    pub fn with_online_plugins(mut self, switched_on: bool) -> Self {
        self.online_is_switched_on = switched_on;
        self
    }

    pub fn with_limits(mut self, limits: TimeLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn inspect(&self, plugin: &Path) -> GateReport {
        if sample_app::is_one(plugin) {
            return self.inspect_sample_app(plugin);
        }
        let mut report = GateReport::default();
        report.record(SOURCE_CHECK, || check_source(plugin, self.online_is_switched_on));
        report.record(PICTURES_CHECK, || check_pictures(plugin));
        report.record(BUILD_CHECK, || self.check_build(plugin));
        report.record(SCREEN_CHECK, || self.check_screen(plugin));
        report.record(MOVING_PICTURES_CHECK, || self.check_moving_pictures(plugin));
        for session in &SESSIONS {
            report.record(simulation_check_name(session), || self.check_session(plugin, session));
        }
        report
    }

    /// A Sample app has no pages, pictures or simulator of Akai's. Its code is read, then built for this
    /// computer, then played through everything on the device by a program of its own.
    fn inspect_sample_app(&self, app: &Path) -> GateReport {
        let mut report = GateReport::default();
        report.record(APP_SOURCE_CHECK, || check_source_of_app(app));
        report.record(APP_BUILD_CHECK, || self.check_app_builds(app));
        report.record(APP_TRIAL_CHECK, || self.check_app_stands_up(app));
        report
    }

    /// Plays a Sample app as `touches` say, once its code has been read and built, and says how it went. Why it
    /// was not played, otherwise.
    pub fn try_sample_app(&self, app: &Path, touches: &serde_json::Value) -> Result<sample_trial::Trial, Vec<String>> {
        let unless_blocked = |outcome: Outcome| if let Outcome::Blocked(why) = outcome { Err(why) } else { Ok(()) };
        unless_blocked(check_source_of_app(app))?;
        unless_blocked(self.check_app_builds(app))?;
        sample_trial::held_by(&self.plugin_host, app, touches, self.limits.simulation).map_err(|stopped| vec![stopped])
    }

    fn check_app_builds(&self, app: &Path) -> Outcome {
        match sample_app::build_for_this_computer(&self.sdk, app, self.limits.build) {
            Ok(_) => Outcome::Passed,
            Err(not_built) => Outcome::Blocked(vec![last_lines(&not_built.to_string(), BUILD_LOG_LINES_SHOWN)]),
        }
    }

    fn check_app_stands_up(&self, app: &Path) -> Outcome {
        let with_everything_on_the_device = serde_json::json!([]);
        match sample_trial::held_by(&self.plugin_host, app, &with_everything_on_the_device, self.limits.simulation) {
            Ok(trial) if !trial.problems().is_empty() => Outcome::Blocked(trial.problems()),
            Ok(trial) if !trial.warnings().is_empty() => Outcome::PassedWithWarnings(trial.warnings()),
            Ok(_) => Outcome::Passed,
            Err(stopped) => Outcome::Blocked(vec![stopped]),
        }
    }

    fn check_build(&self, plugin: &Path) -> Outcome {
        match self.sdk.build(plugin, self.limits.build) {
            Ok(finished) if finished.succeeded() => Outcome::Passed,
            Ok(finished) => Outcome::Blocked(vec![last_lines(&finished.output, BUILD_LOG_LINES_SHOWN)]),
            Err(error) => Outcome::Blocked(vec![describe_run_error("The build", &error)]),
        }
    }

    /// The build makes the screen without pictures and without what the musician arranged by hand.
    /// Those are applied afterwards, so the finished folder is checked again whenever they changed it.
    fn check_screen(&self, plugin: &Path) -> Outcome {
        match artwork::finish_built_screen(plugin, &|sized_art| self.draw_sized_art(plugin, sized_art)) {
            Ok(false) => Outcome::Passed,
            Ok(true) => self.check_finished_folder(plugin),
            Err(problems) => Outcome::Blocked(problems),
        }
    }

    /// The knobs and faders the musician resized are drawn again by Akai's own drawing code.
    fn draw_sized_art(&self, plugin: &Path, sized_art: &[SizedArt]) -> Result<(), String> {
        match self.sdk.draw_sized_art(plugin, sized_art, self.limits.build) {
            Ok(finished) if finished.succeeded() => Ok(()),
            Ok(finished) => Err(format!("The resized knobs could not be drawn: {}", last_lines(&finished.output, BUILD_LOG_LINES_SHOWN))),
            Err(error) => Err(describe_run_error("Drawing the resized knobs", &error)),
        }
    }

    fn check_finished_folder(&self, plugin: &Path) -> Outcome {
        match self.sdk.validate(plugin, self.limits.build) {
            Ok(finished) if finished.succeeded() => Outcome::Passed,
            Ok(finished) => Outcome::Blocked(validator_errors(&finished.output)),
            Err(error) => Outcome::Blocked(vec![describe_run_error("The check of the finished plugin", &error)]),
        }
    }

    /// A picture that plays on its own only moves on an MPC while the plugin keeps turning a control for it.
    /// The built plugin is run and watched, and how long a turn takes is kept for the preview. A control the
    /// plugin moves for some other reason, such as notes held, is watched too but may stand still.
    fn check_moving_pictures(&self, plugin: &Path) -> Outcome {
        let pictures = pictures_of(plugin).unwrap_or_default();
        let layout = ScreenLayout::of_plugin(plugin);
        let turned_by_the_plugin = picture_clock::controls_the_plugin_turns(&pictures, &layout);
        let watched: Vec<_> = turned_by_the_plugin.iter().map(|control| (control, self.watch_turning(plugin, control))).collect();
        let mut problems = clock_shown_as_a_knob(&turned_by_the_plugin, &layout);
        problems.extend(watched.iter().flat_map(|(control, seen)| match seen {
            Ok(seen) => picture_clock::problems_seen(control, seen),
            Err(could_not_watch) => vec![could_not_watch.clone()],
        }));
        let turns = watched.iter().filter_map(|(control, seen)| Some(((*control).clone(), seen.as_ref().ok()?.turn_seconds?))).collect();
        if let Err(error) = picture_clock::keep_turns(plugin, &turns) {
            problems.push(format!("How fast the pictures move could not be written down: {error}"));
        }
        if problems.is_empty() { Outcome::Passed } else { Outcome::Blocked(problems) }
    }

    fn watch_turning(&self, plugin: &Path, control: &str) -> Result<picture_clock::Watched, String> {
        let library = native_library_of(plugin).ok_or_else(|| NO_COPY_TO_TEST.to_string())?;
        let number = artwork::built_control_number(plugin, control).ok_or_else(|| format!("The built plugin has no control called `{control}`."))?;
        picture_clock::watched_by(&self.plugin_host, &library, number, self.limits.simulation)
    }

    fn check_session(&self, plugin: &Path, session: &Session) -> Outcome {
        let Some(library) = native_library_of(plugin) else {
            return Outcome::Blocked(vec![NO_COPY_TO_TEST.to_string()]);
        };
        match self.sdk.simulate(&library, session, self.limits.simulation) {
            Ok(finished) => outcome_of_simulation(&finished),
            Err(RunError::TimedOut { limit, .. }) => Outcome::Blocked(vec![describe_freeze(limit)]),
            Err(error) => Outcome::Blocked(vec![describe_run_error("The simulator", &error)]),
        }
    }
}

pub fn simulation_check_name(session: &Session) -> String {
    format!("Survives {}", session.name)
}

/// A plugin that lists streams is first given MP Forge's stream helper afresh, which is the one part of its
/// code allowed online. Everything else in its code is read as it always was.
fn check_source(plugin: &Path, online_is_switched_on: bool) -> Outcome {
    if let Err(problems) = online::make_ready(plugin, online_is_switched_on) {
        return Outcome::Blocked(problems);
    }
    outcome_of_reading_the_code(plugin)
}

/// An app's code is read with MP Forge's kit written into it afresh, so the kit that is read is the kit that
/// is built with.
fn check_source_of_app(app: &Path) -> Outcome {
    match sample_app::put_kit_into(app) {
        Ok(()) => outcome_of_reading_the_code(app),
        Err(error) => Outcome::Blocked(vec![format!("MP Forge's kit could not be written into the app: {error}")]),
    }
}

fn outcome_of_reading_the_code(plugin: &Path) -> Outcome {
    match scan_plugin(plugin) {
        Ok(violations) if violations.is_empty() => Outcome::Passed,
        Ok(violations) => Outcome::Blocked(violations.iter().map(Violation::describe).collect()),
        Err(error) => Outcome::Blocked(vec![format!("The source could not be read: {error}")]),
    }
}

/// The clock is for the plugin to turn. On a page it would be a knob spinning by itself under the musician's hand.
fn clock_shown_as_a_knob(turned_by_the_plugin: &[String], layout: &ScreenLayout) -> Vec<String> {
    let on_a_page = turned_by_the_plugin.iter().any(|control| control == CLOCK_CONTROL) && picture_clock::is_on_a_page(CLOCK_CONTROL, layout);
    if on_a_page { vec![format!("`{CLOCK_CONTROL}` is turned by the plugin itself, so it must not be on a page in ui.json. Take it off.")] } else { Vec::new() }
}

fn check_pictures(plugin: &Path) -> Outcome {
    let not_followed = not_followed_in_the_layout_of(plugin);
    if !not_followed.is_empty() {
        return Outcome::Blocked(not_followed);
    }
    match artwork::draw_pictures(plugin) {
        Ok(_) => Outcome::Passed,
        Err(problems) => Outcome::Blocked(problems),
    }
}

/// A screen is only let through as MP Forge shows it, so what its layout asks of Akai's generator beyond that stops it.
fn not_followed_in_the_layout_of(plugin: &Path) -> Vec<String> {
    library::screen_layout(plugin).map(|layout| screen_layout::asked_for_but_not_followed_here(&layout)).unwrap_or_default()
}

/// The lines Akai's validator marked as errors, or all it said when it marked none.
fn validator_errors(output: &str) -> Vec<String> {
    let marked: Vec<String> = output.lines().filter_map(|line| line.strip_prefix(VALIDATOR_ERROR_MARK)).map(|line| line.trim().to_string()).collect();
    if marked.is_empty() { vec![last_lines(output, BUILD_LOG_LINES_SHOWN)] } else { marked }
}

fn outcome_of_simulation(finished: &Finished) -> Outcome {
    let report = SimulationReport::parse(&finished.output);
    let mut failures = report.failures();
    if failures.is_empty() && !finished.succeeded() {
        failures.push(describe_unexplained_stop(finished.exit_code));
    }
    if !failures.is_empty() {
        return Outcome::Blocked(failures);
    }
    match report.warnings() {
        warnings if warnings.is_empty() => Outcome::Passed,
        warnings => Outcome::PassedWithWarnings(warnings),
    }
}

fn describe_unexplained_stop(exit_code: Option<i32>) -> String {
    match exit_code {
        Some(code) => format!("The simulator stopped with exit code {code} and gave no reason."),
        None => "The plugin crashed the simulator.".to_string(),
    }
}

fn describe_freeze(limit: Duration) -> String {
    format!("The plugin froze: the simulator had not finished after {} seconds.", limit.as_secs())
}

fn describe_run_error(activity: &str, error: &RunError) -> String {
    match error {
        RunError::CouldNotStart(cause) => format!("{activity} could not be started: {cause}"),
        RunError::TimedOut { limit, .. } => format!("{activity} had not finished after {} seconds.", limit.as_secs()),
    }
}

fn last_lines(text: &str, count: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(count)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finished(exit_code: Option<i32>, output: &str) -> Finished {
        Finished { exit_code, output: output.to_string() }
    }

    #[test]
    fn a_clean_simulation_passes() {
        let outcome = outcome_of_simulation(&finished(Some(0), "PASS  no NaN/Inf anywhere\n"));

        assert_eq!(outcome, Outcome::Passed);
    }

    #[test]
    fn warnings_pass_but_are_kept() {
        let outcome = outcome_of_simulation(&finished(Some(0), "WARN  clipping in the main pass\n"));

        assert_eq!(outcome, Outcome::PassedWithWarnings(vec!["clipping in the main pass".to_string()]));
    }

    #[test]
    fn a_reported_failure_blocks() {
        let outcome = outcome_of_simulation(&finished(Some(1), "FAIL  NaN in the output\n"));

        assert_eq!(outcome, Outcome::Blocked(vec!["NaN in the output".to_string()]));
    }

    #[test]
    fn a_failing_exit_code_blocks_even_when_the_report_looks_clean() {
        let outcome = outcome_of_simulation(&finished(Some(1), "PASS  everything\n"));

        assert!(matches!(outcome, Outcome::Blocked(_)));
    }

    #[test]
    fn a_simulator_killed_by_the_plugin_blocks() {
        let outcome = outcome_of_simulation(&finished(None, ""));

        assert_eq!(outcome, Outcome::Blocked(vec!["The plugin crashed the simulator.".to_string()]));
    }

    #[test]
    fn checks_after_a_block_are_not_run() {
        let mut report = GateReport::default();
        report.record("first", || Outcome::Blocked(vec!["no".to_string()]));
        report.record("second", || panic!("must not run after a block"));

        assert_eq!(report.outcome_of("second"), Some(&Outcome::NotRun));
        assert!(!report.may_be_sent());
    }

    #[test]
    fn an_empty_report_never_lets_a_plugin_through() {
        assert!(!GateReport::default().may_be_sent());
    }

    #[test]
    fn the_written_report_lists_each_check_with_its_details_and_a_verdict() {
        let mut report = GateReport::default();
        report.record("Builds", || Outcome::Passed);
        report.record("Survives", || Outcome::Blocked(vec!["NaN in the output".to_string()]));

        let written = report.to_string();

        assert!(written.contains("  PASS   Builds\n"));
        assert!(written.contains("  BLOCK  Survives\n           NaN in the output\n"));
        assert!(written.ends_with("Verdict: NOT safe to send. Fix the blocked check first."));
    }

    #[test]
    fn only_the_validators_errors_are_passed_on() {
        let output = "OK     folder name\nERROR  TUI.json: references image pictures/reel.png which is not in Plugin Skins/\n1 error, 0 warnings\n";

        assert_eq!(validator_errors(output), ["TUI.json: references image pictures/reel.png which is not in Plugin Skins/"]);
        assert_eq!(validator_errors("it fell over"), ["it fell over"]);
    }

    #[test]
    fn only_the_end_of_a_long_build_log_is_shown() {
        assert_eq!(last_lines("one\ntwo\nthree", 2), "two\nthree");
        assert_eq!(last_lines("one", 5), "one");
    }
}
