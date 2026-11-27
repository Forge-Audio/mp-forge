//! The user's plugins on disk: which ones exist, and what each one's screen looks like.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::Value;

use crate::history::copy_folder;

const PLUGIN_MANIFEST: &str = "plugin.json";
const SCREEN_LAYOUT: &str = "ui.json";
const BUILT_PARAMETERS: &str = "build/params.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSummary {
    pub name: String,
    pub maker: String,
    pub kind: String,
}

/// Every plugin in the folder, by name. Folders without a readable manifest are not plugins.
pub fn list_plugins(plugins_dir: &Path) -> Vec<PluginSummary> {
    let Ok(entries) = fs::read_dir(plugins_dir) else { return Vec::new() };
    let mut plugins: Vec<PluginSummary> = entries.filter_map(|entry| summary_of(&entry.ok()?.path())).collect();
    plugins.sort_by(|one, other| one.name.cmp(&other.name));
    plugins
}

/// What became of the plugins when they were moved to another folder.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MovedPlugins {
    pub moved: Vec<String>,
    /// Left where they were, because the new folder already holds something of that name.
    pub left_behind: Vec<String>,
}

/// Moves every plugin into another folder, with its versions and its conversation. Nothing that is already in
/// the new folder is ever replaced, and a plugin that cannot be moved whole stays where it was.
pub fn move_plugins(from: &Path, to: &Path) -> io::Result<MovedPlugins> {
    fs::create_dir_all(to)?;
    let mut outcome = MovedPlugins::default();
    for folder_name in plugin_folders_in(from) {
        if to.join(&folder_name).exists() {
            outcome.left_behind.push(folder_name);
        } else {
            move_folder(&from.join(&folder_name), &to.join(&folder_name))?;
            outcome.moved.push(folder_name);
        }
    }
    Ok(outcome)
}

fn plugin_folders_in(plugins_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(plugins_dir) else { return Vec::new() };
    let is_plugin = |entry: &fs::DirEntry| summary_of(&entry.path()).is_some();
    let mut names: Vec<String> = entries.flatten().filter(is_plugin).map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// A folder is moved in one step where it can be. Onto another drive it is copied whole first, and the
/// original is taken away only once the copy is complete.
fn move_folder(from: &Path, to: &Path) -> io::Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if let Err(problem) = copy_folder(from, to) {
        let _ = fs::remove_dir_all(to);
        return Err(problem);
    }
    fs::remove_dir_all(from)
}

pub fn summary_of(plugin_dir: &Path) -> Option<PluginSummary> {
    let manifest = read_json(&plugin_dir.join(PLUGIN_MANIFEST))?;
    let text = |field: &str| manifest[field].as_str().map(str::to_string);
    Some(PluginSummary { name: text("name")?, maker: text("manufacturer")?, kind: text("kind")? })
}

/// The pages and controls the plugin shows on the MPC's touch screen.
pub fn screen_layout(plugin_dir: &Path) -> Option<Value> {
    read_json(&plugin_dir.join(SCREEN_LAYOUT))
}

/// What the built plugin reports about its own controls. Present only after a successful build.
pub fn built_parameters(plugin_dir: &Path) -> Option<Value> {
    read_json(&plugin_dir.join(BUILT_PARAMETERS))
}

pub(crate) fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn add_plugin(plugins: &TemporaryFolder, name: &str, kind: &str) {
        let folder = plugins.0.join(name);
        fs::create_dir_all(&folder).unwrap();
        let manifest = format!(r#"{{"name": "{name}", "manufacturer": "Tests", "kind": "{kind}"}}"#);
        fs::write(folder.join(PLUGIN_MANIFEST), manifest).unwrap();
    }

    #[test]
    fn plugins_move_to_another_folder_with_everything_in_them() {
        let old_place = TemporaryFolder::create("library-move-from");
        let new_place = TemporaryFolder::create("library-move-to");
        add_plugin(&old_place, "Dusty Tape", "effect");
        fs::create_dir_all(old_place.0.join("Dusty Tape/.versions/1")).unwrap();
        fs::write(old_place.0.join("Dusty Tape/.versions/1/ui.json"), "{}").unwrap();
        fs::create_dir_all(old_place.0.join("not a plugin")).unwrap();

        let outcome = move_plugins(&old_place.0, &new_place.0.join("my plugins")).unwrap();

        assert_eq!(outcome, MovedPlugins { moved: vec!["Dusty Tape".to_string()], left_behind: Vec::new() });
        assert!(new_place.0.join("my plugins/Dusty Tape/.versions/1/ui.json").is_file());
        assert!(!old_place.0.join("Dusty Tape").exists());
        assert!(old_place.0.join("not a plugin").is_dir());
    }

    #[test]
    fn a_plugin_is_left_behind_rather_than_replace_one_of_the_same_name() {
        let old_place = TemporaryFolder::create("library-keep-from");
        let new_place = TemporaryFolder::create("library-keep-to");
        add_plugin(&old_place, "Dusty Tape", "effect");
        add_plugin(&old_place, "Glass Pad", "instrument");
        add_plugin(&new_place, "Dusty Tape", "instrument");

        let outcome = move_plugins(&old_place.0, &new_place.0).unwrap();

        assert_eq!(outcome, MovedPlugins { moved: vec!["Glass Pad".to_string()], left_behind: vec!["Dusty Tape".to_string()] });
        assert!(old_place.0.join("Dusty Tape").is_dir());
        assert_eq!(summary_of(&new_place.0.join("Dusty Tape")).unwrap().kind, "instrument");
    }

    #[test]
    fn plugins_are_listed_by_name_and_other_folders_are_ignored() {
        let plugins = TemporaryFolder::create("library");
        add_plugin(&plugins, "Glass Pad", "instrument");
        add_plugin(&plugins, "Dusty Tape", "effect");
        fs::create_dir_all(plugins.0.join("not a plugin")).unwrap();

        let listed = list_plugins(&plugins.0);

        let names: Vec<&str> = listed.iter().map(|plugin| plugin.name.as_str()).collect();
        assert_eq!(names, ["Dusty Tape", "Glass Pad"]);
        assert_eq!(listed[1].kind, "instrument");
    }

    #[test]
    fn a_missing_plugins_folder_is_an_empty_library() {
        assert_eq!(list_plugins(Path::new("/no/such/folder")), []);
    }

    #[test]
    fn the_screen_layout_is_read_when_present() {
        let plugins = TemporaryFolder::create("library-screen");
        add_plugin(&plugins, "Dusty Tape", "effect");
        let plugin_dir = plugins.0.join("Dusty Tape");
        fs::write(plugin_dir.join(SCREEN_LAYOUT), r#"{"kit": "cyan", "pages": []}"#).unwrap();

        assert_eq!(screen_layout(&plugin_dir).unwrap()["kit"], "cyan");
        assert!(built_parameters(&plugin_dir).is_none());
    }
}
