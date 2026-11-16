//! Where the app keeps its things, and the rules about what may happen at the same time. The AI works on
//! up to five plugins at once, each a job of its own. Safety checks run side by side, one to a plugin, and
//! beside the AI's work on other plugins. A plugin with neither going on can be changed by hand.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::{Value, json};
use workbench_engine::library::{self, MovedPlugins};
use workbench_engine::maker::Maker;
use workbench_engine::sdk::Sdk;
use workbench_engine::stop_signal::StopSignal;

const HOME_VARIABLE: &str = "WORKBENCH_HOME";
/// While the app is run from its source folder, its home is the project folder beside it.
const HOME_DURING_DEVELOPMENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/..");
const SDK_FOLDER: &str = "vendor/mpc-plugin-builder-sdk";
const PLUGINS_FOLDER: &str = "plugins";
/// Each plugin being built has an AI, a compiler and a safety check of its own at work on this computer.
const MOST_BUILT_AT_ONCE: usize = 5;
const NO_ROOM_FOR_ANOTHER: &str = "The AI is already building five plugins. Build this one once one of them has finished.";
/// The note the app keeps, among its own settings, of the folder the musician chose for their plugins.
const PLACES_NOTE: &str = "places.json";
const FOLDER_ON_THE_NOTE: &str = "pluginsFolder";
const STAYS_WHILE_BUSY: &str = "Something is still being built, checked or changed. Choose the folder again once it has finished.";
const ALREADY_THERE: &str = "Your plugins are already kept in that folder.";
const NOT_INSIDE_ITSELF: &str = "Choose a folder that is not inside the one your plugins are in now.";

/// What the workbench is in the middle of.
#[derive(Default)]
struct Work {
    /// The plugins the AI is working on.
    ai_working_on: Vec<String>,
    /// The plugins having their safety check run, or being put back a version, which ends in a check.
    checking: Vec<String>,
    /// The plugins with a quick change by hand under way, once for each such change.
    changing_by_hand: Vec<String>,
}

impl Work {
    /// Why nothing new may be started on this plugin just now. None when it is free.
    fn what_holds(&self, plugin_name: &str) -> Option<String> {
        let named = |plugins: &[String]| plugins.iter().any(|held| held == plugin_name);
        if named(&self.ai_working_on) {
            return Some(format!("The AI is working on `{plugin_name}`. Try again once it has finished."));
        }
        if named(&self.checking) {
            return Some(format!("`{plugin_name}` is having its safety check run. Try again once it has finished."));
        }
        named(&self.changing_by_hand).then(|| format!("`{plugin_name}` is being changed by hand. Try again in a moment."))
    }

    fn is_idle(&self) -> bool {
        self.ai_working_on.is_empty() && self.checking.is_empty() && self.changing_by_hand.is_empty()
    }

    fn is_being_worked_on(&self, plugin_name: &str) -> bool {
        self.ai_working_on.iter().chain(&self.checking).any(|worked_on| worked_on == plugin_name)
    }
}

pub struct Workbench {
    /// Where Akai's SDK is, or why it is not on this computer.
    sdk_folder: Result<PathBuf, String>,
    /// Where the plugins are kept. The musician can choose another folder.
    plugins: Mutex<PathBuf>,
    /// Where the chosen folder is written down, so it is still the one when the app is next opened.
    places_note: Option<PathBuf>,
    work: Arc<Mutex<Work>>,
    /// The signal held by the AI's job on each plugin, so the musician can stop that job and no other.
    stops: Mutex<HashMap<String, StopSignal>>,
}

impl Workbench {
    /// MP Forge run from its source folder, with Akai's SDK and the plugins in the project folder.
    pub fn at_home() -> Self {
        let home = env::var_os(HOME_VARIABLE).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(HOME_DURING_DEVELOPMENT));
        Self::keeping(Ok(home.join(SDK_FOLDER)), home.join(PLUGINS_FOLDER))
    }

    /// An installed MP Forge, with Akai's SDK where it was unpacked and the plugins in a folder of the musician's.
    pub fn installed(sdk_folder: Result<PathBuf, String>, plugins: PathBuf) -> Self {
        Self::keeping(sdk_folder, plugins)
    }

    fn keeping(sdk_folder: Result<PathBuf, String>, plugins: PathBuf) -> Self {
        Self { sdk_folder, plugins: Mutex::new(plugins), places_note: None, work: Arc::default(), stops: Mutex::default() }
    }

    /// The same workbench, keeping its plugins in the folder noted among the app's settings, if one was chosen,
    /// and noting there whichever folder is chosen from now on.
    pub fn remembering_places_in(self, settings_dir: &Path) -> Self {
        let note = settings_dir.join(PLACES_NOTE);
        let plugins = folder_noted_on(&note).map_or(self.plugins, Mutex::new);
        Self { plugins, places_note: Some(note), ..self }
    }

    pub fn plugins_dir(&self) -> PathBuf {
        self.plugins_folder().clone()
    }

    /// The folder of one plugin. Anything but the plain name of one folder could lead out of the plugins folder,
    /// so it is answered the way a name no plugin has is.
    pub fn plugin_dir(&self, name: &str) -> Result<PathBuf, String> {
        if names_one_folder(name) { Ok(self.plugins_dir().join(name)) } else { Err(format!("There is no plugin called `{name}`.")) }
    }

    /// Keeps the plugins in another folder from now on, moving them there first when they are to come along.
    /// Nothing may be under way while they move, and nothing can start until they have.
    pub fn keep_plugins_in(&self, folder: &Path, bring_along: bool) -> Result<MovedPlugins, String> {
        let work = work_of(&self.work);
        if !work.is_idle() {
            return Err(STAYS_WHILE_BUSY.to_string());
        }
        let mut plugins = self.plugins_folder();
        refuse_unusable_folder(&plugins, folder)?;
        fs::create_dir_all(folder).map_err(|problem| format!("That folder cannot be used: {problem}."))?;
        let moved = if bring_along { moved_between(&plugins, folder)? } else { MovedPlugins::default() };
        self.note_folder(folder)?;
        *plugins = folder.to_path_buf();
        Ok(moved)
    }

    fn note_folder(&self, folder: &Path) -> Result<(), String> {
        let Some(note) = &self.places_note else { return Ok(()) };
        let written = json!({ FOLDER_ON_THE_NOTE: folder }).to_string();
        let kept = note.parent().map_or(Ok(()), fs::create_dir_all).and_then(|()| fs::write(note, written));
        kept.map_err(|problem| format!("The choice of folder could not be remembered: {problem}."))
    }

    fn plugins_folder(&self) -> MutexGuard<'_, PathBuf> {
        self.plugins.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Akai's SDK, which comes with MP Forge.
    pub fn sdk(&self) -> Result<Sdk, String> {
        Sdk::at(self.sdk_folder.clone()?).map_err(|not_found| not_found.to_string())
    }

    pub fn maker(&self) -> Result<Maker, String> {
        Ok(Maker::new(self.sdk()?, self.plugins_dir()))
    }

    /// A fresh signal for the job about to start on a plugin. Asking to stop reaches that job and no later one.
    pub fn stop_signal_for_job_on(&self, plugin_name: &str) -> StopSignal {
        let fresh = StopSignal::default();
        self.held_signals().insert(plugin_name.to_string(), fresh.clone());
        fresh
    }

    pub fn ask_job_to_stop_on(&self, plugin_name: &str) {
        if let Some(signal) = self.held_signals().get(plugin_name) {
            signal.ask_to_stop();
        }
    }

    fn held_signals(&self) -> MutexGuard<'_, HashMap<String, StopSignal>> {
        self.stops.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Claims a plugin for the AI to work on. The AI works on a few plugins at once, and never on one that
    /// is being worked on, checked or changed by hand already. Each claim is given up when the returned value
    /// is dropped.
    pub fn claim_for_ai_work(&self, plugin_name: &str) -> Result<Claim, String> {
        let mut work = work_of(&self.work);
        refuse_if_held(&work, plugin_name)?;
        if work.ai_working_on.len() >= MOST_BUILT_AT_ONCE {
            return Err(NO_ROOM_FOR_ANOTHER.to_string());
        }
        work.ai_working_on.push(plugin_name.to_string());
        Ok(self.claim_holding(Held::AiWork(plugin_name.to_string())))
    }

    /// Claims a plugin for its safety check. Other plugins can be checked at the same time, and the AI can
    /// be at work on another. Nothing else may be going on with this one.
    pub fn claim_to_check(&self, plugin_name: &str) -> Result<Claim, String> {
        let mut work = work_of(&self.work);
        refuse_if_held(&work, plugin_name)?;
        work.checking.push(plugin_name.to_string());
        Ok(self.claim_holding(Held::Check(plugin_name.to_string())))
    }

    /// Claims a plugin for a quick change by hand, such as keeping its look. Work on another plugin does not
    /// stand in the way. The AI's work or a safety check on this one does.
    pub fn claim_to_change_by_hand(&self, plugin_name: &str) -> Result<Claim, String> {
        let mut work = work_of(&self.work);
        if work.is_being_worked_on(plugin_name) {
            return Err(work.what_holds(plugin_name).unwrap_or_default());
        }
        work.changing_by_hand.push(plugin_name.to_string());
        Ok(self.claim_holding(Held::ChangeByHand(plugin_name.to_string())))
    }

    fn claim_holding(&self, held: Held) -> Claim {
        Claim { work: Arc::clone(&self.work), held }
    }
}

/// Whether the name is the plain name of one folder: no step up, no step down and no place of its own to start from.
fn names_one_folder(name: &str) -> bool {
    let mut steps = Path::new(name).components();
    matches!((steps.next(), steps.next()), (Some(Component::Normal(only)), None) if only == OsStr::new(name))
}

fn folder_noted_on(note: &Path) -> Option<PathBuf> {
    let written: Value = serde_json::from_str(&fs::read_to_string(note).ok()?).ok()?;
    written[FOLDER_ON_THE_NOTE].as_str().map(PathBuf::from)
}

/// A folder is compared as the computer knows it, so two spellings of one place count as the same. A folder
/// that is not there yet is known by the nearest folder above it that is.
fn as_the_computer_knows_it(folder: &Path) -> PathBuf {
    let known = folder.ancestors().find_map(|above| Some((above, above.canonicalize().ok()?)));
    let Some((above, as_known)) = known else { return folder.to_path_buf() };
    folder.strip_prefix(above).map_or_else(|_| folder.to_path_buf(), |not_there_yet| as_known.join(not_there_yet))
}

fn refuse_unusable_folder(current: &Path, chosen: &Path) -> Result<(), String> {
    let (current, chosen) = (as_the_computer_knows_it(current), as_the_computer_knows_it(chosen));
    if !chosen.is_absolute() {
        return Err("Choose a folder by its whole path.".to_string());
    }
    if chosen == current {
        return Err(ALREADY_THERE.to_string());
    }
    if chosen.starts_with(&current) { Err(NOT_INSIDE_ITSELF.to_string()) } else { Ok(()) }
}

/// A move that stops part way loses nothing: what has moved is in the new folder and the rest is where it was.
fn moved_between(current: &Path, chosen: &Path) -> Result<MovedPlugins, String> {
    library::move_plugins(current, chosen).map_err(|problem| {
        format!("Not every plugin could be moved: {problem}. Nothing was lost. Those already moved are in {}, the rest are still in {}, and that is where MP Forge still looks. Choosing the folder again moves the rest.", chosen.display(), current.display())
    })
}

fn refuse_if_held(work: &Work, plugin_name: &str) -> Result<(), String> {
    work.what_holds(plugin_name).map_or(Ok(()), Err)
}

fn work_of(work: &Mutex<Work>) -> MutexGuard<'_, Work> {
    work.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

enum Held {
    AiWork(String),
    Check(String),
    ChangeByHand(String),
}

pub struct Claim {
    work: Arc<Mutex<Work>>,
    held: Held,
}

impl Drop for Claim {
    fn drop(&mut self) {
        let mut work = work_of(&self.work);
        match &self.held {
            Held::AiWork(plugin_name) => forget_once(&mut work.ai_working_on, plugin_name),
            Held::Check(plugin_name) => forget_once(&mut work.checking, plugin_name),
            Held::ChangeByHand(plugin_name) => forget_once(&mut work.changing_by_hand, plugin_name),
        }
    }
}

/// Takes one mention of a plugin out of a list that may hold it several times.
fn forget_once(plugins: &mut Vec<String>, plugin_name: &str) {
    if let Some(position) = plugins.iter().position(|held| held == plugin_name) {
        plugins.swap_remove(position);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_plain_name_of_one_folder_is_taken_for_a_plugin() {
        let workbench = Workbench::at_home();

        assert_eq!(workbench.plugin_dir("Dusty Tape"), Ok(workbench.plugins_dir().join("Dusty Tape")));
        for sly in ["", ".", "..", "../Dusty Tape", "Dusty Tape/..", "Dusty Tape/src", "/etc", "Dusty Tape/"] {
            assert!(workbench.plugin_dir(sly).is_err(), "`{sly}` was taken for a plugin");
        }
    }

    #[test]
    fn the_ai_works_on_five_plugins_at_once_and_no_more() {
        let workbench = Workbench::at_home();
        let five = ["Dusty Tape", "Glass Pad", "Moon Verb", "Brick Wall", "Sea Organ"];

        let mut at_work: Vec<Claim> = five.iter().map(|plugin| workbench.claim_for_ai_work(plugin).unwrap()).collect();
        assert!(workbench.claim_for_ai_work("One Too Many").is_err_and(|why| why == NO_ROOM_FOR_ANOTHER));
        at_work.pop();

        assert!(workbench.claim_for_ai_work("One Too Many").is_ok());
    }

    #[test]
    fn the_ai_never_works_twice_on_one_plugin() {
        let workbench = Workbench::at_home();

        let first = workbench.claim_for_ai_work("Dusty Tape").unwrap();
        assert!(workbench.claim_for_ai_work("Dusty Tape").is_err_and(|why| why.contains("working on `Dusty Tape`")));
        drop(first);

        assert!(workbench.claim_for_ai_work("Dusty Tape").is_ok());
    }

    #[test]
    fn safety_checks_run_side_by_side_and_beside_the_ais_work_on_another_plugin() {
        let workbench = Workbench::at_home();
        let _building = workbench.claim_for_ai_work("Dusty Tape").unwrap();

        let checking_one = workbench.claim_to_check("Glass Pad");
        let checking_another = workbench.claim_to_check("Moon Verb");

        assert!(checking_one.is_ok() && checking_another.is_ok());
    }

    #[test]
    fn a_plugin_has_one_thing_going_on_at_a_time() {
        let workbench = Workbench::at_home();
        let building = workbench.claim_for_ai_work("Dusty Tape").unwrap();
        let checking = workbench.claim_to_check("Glass Pad").unwrap();

        assert!(workbench.claim_to_check("Dusty Tape").is_err());
        assert!(workbench.claim_to_check("Glass Pad").is_err());
        assert!(workbench.claim_to_change_by_hand("Dusty Tape").is_err());
        assert!(workbench.claim_to_change_by_hand("Glass Pad").is_err());
        drop((building, checking));

        assert!(workbench.claim_to_check("Dusty Tape").is_ok());
        assert!(workbench.claim_for_ai_work("Glass Pad").is_ok());
    }

    #[test]
    fn another_plugin_can_be_changed_by_hand_while_one_is_worked_on() {
        let workbench = Workbench::at_home();
        let _building = workbench.claim_for_ai_work("Dusty Tape").unwrap();

        assert!(workbench.claim_to_change_by_hand("Glass Pad").is_ok());
    }

    #[test]
    fn work_on_a_plugin_waits_for_every_change_by_hand_to_it_to_finish() {
        let workbench = Workbench::at_home();
        let one_change = workbench.claim_to_change_by_hand("Glass Pad").unwrap();
        let another_change = workbench.claim_to_change_by_hand("Glass Pad").unwrap();

        drop(one_change);
        assert!(workbench.claim_to_check("Glass Pad").is_err());
        assert!(workbench.claim_for_ai_work("Glass Pad").is_err());
        drop(another_change);

        assert!(workbench.claim_to_check("Glass Pad").is_ok());
    }

    #[test]
    fn an_installed_copy_says_why_akais_sdk_is_not_there() {
        let workbench = Workbench::installed(Err("The disk is full.".to_string()), PathBuf::from("plugins"));

        assert_eq!(workbench.sdk().err(), Some("The disk is full.".to_string()));
        assert_eq!(workbench.plugins_dir(), PathBuf::from("plugins"));
    }

    fn scratch_folder(label: &str) -> PathBuf {
        let folder = env::temp_dir().join(format!("workbench-places-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    fn workbench_keeping_plugins_under(root: &Path) -> Workbench {
        Workbench { plugins: Mutex::new(root.join("plugins")), ..Workbench::at_home() }.remembering_places_in(&root.join("settings"))
    }

    fn add_plugin(plugins: &Path, name: &str) {
        fs::create_dir_all(plugins.join(name)).unwrap();
        fs::write(plugins.join(name).join("plugin.json"), format!(r#"{{"name": "{name}", "manufacturer": "Tests", "kind": "effect"}}"#)).unwrap();
    }

    #[test]
    fn the_chosen_folder_takes_the_plugins_along_and_is_still_chosen_next_time() {
        let root = scratch_folder("move");
        add_plugin(&root.join("plugins"), "Dusty Tape");
        let workbench = workbench_keeping_plugins_under(&root);

        let moved = workbench.keep_plugins_in(&root.join("chosen"), true).unwrap();

        assert_eq!(moved.moved, ["Dusty Tape"]);
        assert_eq!(workbench.plugins_dir(), root.join("chosen"));
        assert!(root.join("chosen/Dusty Tape/plugin.json").is_file());
        assert_eq!(workbench_keeping_plugins_under(&root).plugins_dir(), root.join("chosen"));
    }

    #[test]
    fn a_folder_can_be_chosen_without_moving_anything_into_it() {
        let root = scratch_folder("fresh");
        add_plugin(&root.join("plugins"), "Dusty Tape");
        let workbench = workbench_keeping_plugins_under(&root);

        let moved = workbench.keep_plugins_in(&root.join("chosen"), false).unwrap();

        assert!(moved.moved.is_empty());
        assert!(root.join("plugins/Dusty Tape").is_dir());
        assert_eq!(workbench.plugins_dir(), root.join("chosen"));
    }

    #[test]
    fn the_folder_stays_while_anything_is_under_way_and_never_moves_inside_itself() {
        let root = scratch_folder("refused");
        add_plugin(&root.join("plugins"), "Dusty Tape");
        let workbench = workbench_keeping_plugins_under(&root);

        assert_eq!(workbench.keep_plugins_in(&root.join("plugins"), true).unwrap_err(), ALREADY_THERE);
        assert_eq!(workbench.keep_plugins_in(&root.join("plugins/Dusty Tape/inside"), true).unwrap_err(), NOT_INSIDE_ITSELF);
        let checking = workbench.claim_to_check("Dusty Tape").unwrap();
        assert_eq!(workbench.keep_plugins_in(&root.join("chosen"), true).unwrap_err(), STAYS_WHILE_BUSY);
        drop(checking);

        assert_eq!(workbench.plugins_dir(), root.join("plugins"));
    }

    #[test]
    fn asking_to_stop_reaches_the_job_on_that_plugin_and_neither_another_plugins_nor_its_next() {
        let workbench = Workbench::at_home();
        let running = workbench.stop_signal_for_job_on("Dusty Tape");
        let beside_it = workbench.stop_signal_for_job_on("Glass Pad");

        workbench.ask_job_to_stop_on("Dusty Tape");
        let next = workbench.stop_signal_for_job_on("Dusty Tape");

        assert!(running.is_asked());
        assert!(!beside_it.is_asked() && !next.is_asked());
    }
}
