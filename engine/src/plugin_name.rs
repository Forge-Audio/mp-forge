//! Giving a plugin another name, and taking a plugin away.
//!
//! A plugin's name is its folder's name. It is also written in its manifest, in the names of the files the
//! manifest sends to the MPC, and in the name its own code reports, which Akai's validator compares with the
//! manifest. Every version kept for GO BACK is renamed the same way, so going back never brings the old name back.
//! The plugin's four-letter id stays as it was: to the MPC it is the same plugin under another name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "plugin.json";
const SOURCE_FOLDER: &str = "src";
const SOURCE_EXTENSIONS: [&str; 3] = ["cpp", "h", "hpp"];
const VERSIONS_FOLDER: &str = ".versions";
/// Everything in here was built under the old name, so it is thrown away and built again by the next check.
const BUILD_FOLDER: &str = "build";
/// What stands straight before the plugin's name, between quotes, where a file says what the plugin is called.
const BEFORE_THE_NAME_IN_THE_MANIFEST: [&str; 2] = ["\"name\"", ":"];
const BEFORE_THE_NAME_IN_THE_CODE: [&str; 2] = [".name", "="];

struct Renaming<'names> {
    old_name: &'names str,
    new_name: &'names str,
}

/// Renames the plugin in `plugins_dir`. The caller has already made sure the new name is a usable one that is free.
pub fn rename(plugins_dir: &Path, old_name: &str, new_name: &str) -> io::Result<()> {
    let plugin_dir = plugins_dir.join(old_name);
    let renaming = Renaming { old_name, new_name };
    for folder in folders_holding_its_name(&plugin_dir) {
        rename_inside(&folder, &renaming)?;
    }
    remove_if_present(&plugin_dir.join(BUILD_FOLDER))?;
    fs::rename(&plugin_dir, plugins_dir.join(new_name))
}

/// Moves the plugin's folder to where this computer keeps what was thrown away: the Trash on a Mac, the Recycle
/// Bin on Windows, the desktop's own trash on Linux. It can still be got back from there.
pub fn move_to_trash(plugin_dir: &Path) -> Result<(), String> {
    the_computers_trash().delete(plugin_dir).map_err(|refused| refused.to_string())
}

/// On a Mac the trash is asked for directly. The other way there goes through the Finder, which first asks the
/// musician to let the app control it.
fn the_computers_trash() -> trash::TrashContext {
    #[allow(unused_mut)]
    let mut the_trash = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    trash::macos::TrashContextExtMacos::set_delete_method(&mut the_trash, trash::macos::DeleteMethod::NsFileManager);
    the_trash
}

/// The plugin's own folder and each version of it kept for GO BACK.
fn folders_holding_its_name(plugin_dir: &Path) -> Vec<PathBuf> {
    let versions = fs::read_dir(plugin_dir.join(VERSIONS_FOLDER)).into_iter().flatten().flatten().map(|entry| entry.path());
    std::iter::once(plugin_dir.to_path_buf()).chain(versions.filter(|version| version.is_dir())).collect()
}

fn rename_inside(folder: &Path, renaming: &Renaming) -> io::Result<()> {
    rewrite(&folder.join(MANIFEST), |manifest| manifest_renamed(manifest, renaming))?;
    for source_file in source_files_in(&folder.join(SOURCE_FOLDER)) {
        rewrite(&source_file, |code| renaming.where_it_follows(BEFORE_THE_NAME_IN_THE_CODE, code))?;
    }
    Ok(())
}

/// The manifest with the plugin's name changed, and with it the names of the files the build makes for the MPC.
fn manifest_renamed(manifest: &str, renaming: &Renaming) -> String {
    let built_file = |name: &str| format!("/{}-arm", slug_of(name));
    let renamed = renaming.where_it_follows(BEFORE_THE_NAME_IN_THE_MANIFEST, manifest);
    renamed.replace(&built_file(renaming.old_name), &built_file(renaming.new_name))
}

impl Renaming<'_> {
    /// The text with the name changed only where it is given as the plugin's own. A control or a preset that
    /// shares the name keeps it, because the files that lay out the screen go on calling it that.
    fn where_it_follows(&self, lead_in: [&str; 2], text: &str) -> String {
        let (old_between_quotes, new_between_quotes) = (format!("\"{}\"", self.old_name), format!("\"{}\"", self.new_name));
        let mut renamed = String::with_capacity(text.len());
        let mut copied_up_to = 0;
        for (found_at, found) in text.match_indices(&old_between_quotes) {
            if ends_with_each_in_turn(&text[..found_at], lead_in) {
                renamed.push_str(&text[copied_up_to..found_at]);
                renamed.push_str(&new_between_quotes);
                copied_up_to = found_at + found.len();
            }
        }
        renamed + &text[copied_up_to..]
    }
}

/// Whether the text ends with these words one after the other, whatever the spacing between and after them.
fn ends_with_each_in_turn(text: &str, words: [&str; 2]) -> bool {
    words.iter().rev().try_fold(text, |left, word| left.trim_end().strip_suffix(word)).is_some()
}

/// The name as Akai's tools write it into file names: small letters and digits, with a hyphen for anything else.
fn slug_of(name: &str) -> String {
    name.to_ascii_lowercase().chars().map(|letter| if letter.is_ascii_alphanumeric() { letter } else { '-' }).collect()
}

fn source_files_in(folder: &Path) -> Vec<PathBuf> {
    let is_source = |path: &PathBuf| path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| SOURCE_EXTENSIONS.contains(&extension));
    fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(is_source).collect()
}

/// Writes a file again only when it would change. A file that is not there is left out.
fn rewrite(file: &Path, changed: impl Fn(&str) -> String) -> io::Result<()> {
    let Ok(before) = fs::read_to_string(file) else { return Ok(()) };
    let after = changed(&before);
    if after == before { Ok(()) } else { fs::write(file, after) }
}

fn remove_if_present(folder: &Path) -> io::Result<()> {
    match fs::remove_dir_all(folder) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const MANIFEST_OF_DUSTY_TAPE: &str = r#"{ "name": "Dusty Tape", "uid": "WFmo", "file32": "VST/dusty-tape-arm32.so", "file64": "VST/dusty-tape-arm64.so", "description": "Dusty Tape: an echo." }"#;
    const CODE_OF_DUSTY_TAPE: &str = "// Dusty Tape\ninfo.name = \"Dusty Tape\";\ninfo.uniqueId = MPC_FOURCC('W', 'F', 'm', 'o');";

    fn plugin_called_dusty_tape(plugins: &TemporaryFolder) -> PathBuf {
        let plugin = plugins.0.join("Dusty Tape");
        for folder in [plugin.clone(), plugin.join(".versions/1")] {
            fs::create_dir_all(folder.join("src")).unwrap();
            fs::write(folder.join("plugin.json"), MANIFEST_OF_DUSTY_TAPE).unwrap();
            fs::write(folder.join("src/DustyTape.cpp"), CODE_OF_DUSTY_TAPE).unwrap();
        }
        fs::create_dir_all(plugin.join("build/native")).unwrap();
        plugin
    }

    #[test]
    fn a_renamed_plugin_has_its_new_name_in_its_folder_its_manifest_and_its_code_and_keeps_its_id() {
        let plugins = TemporaryFolder::create("rename-plugin");
        plugin_called_dusty_tape(&plugins);

        rename(&plugins.0, "Dusty Tape", "Worn Reel 2").unwrap();

        let renamed = plugins.0.join("Worn Reel 2");
        let manifest = fs::read_to_string(renamed.join("plugin.json")).unwrap();
        let code = fs::read_to_string(renamed.join("src/DustyTape.cpp")).unwrap();
        assert!(!plugins.0.join("Dusty Tape").exists());
        assert!(manifest.contains(r#""name": "Worn Reel 2""#) && manifest.contains("VST/worn-reel-2-arm32.so") && manifest.contains("VST/worn-reel-2-arm64.so"));
        assert!(manifest.contains(r#""uid": "WFmo""#) && manifest.contains("Dusty Tape: an echo."));
        assert!(code.contains("info.name = \"Worn Reel 2\";") && code.contains("MPC_FOURCC('W', 'F', 'm', 'o')"));
    }

    #[test]
    fn a_control_or_a_preset_that_shares_the_plugins_name_keeps_it() {
        let plugins = TemporaryFolder::create("rename-shared-name");
        let plugin = plugin_called_dusty_tape(&plugins);
        let code_with_a_control_of_that_name = format!("{CODE_OF_DUSTY_TAPE}\nparams.add(\"Dusty Tape\", 0.0f, 1.0f);\npreset.displayName = \"Dusty Tape\";");
        fs::write(plugin.join("src/DustyTape.cpp"), code_with_a_control_of_that_name).unwrap();
        fs::write(plugin.join("plugin.json"), r#"{ "name": "Dusty Tape", "author": "Dusty Tape", "file32": "VST/dusty-tape-arm32.so" }"#).unwrap();

        rename(&plugins.0, "Dusty Tape", "Worn Reel").unwrap();

        let code = fs::read_to_string(plugins.0.join("Worn Reel/src/DustyTape.cpp")).unwrap();
        let manifest = fs::read_to_string(plugins.0.join("Worn Reel/plugin.json")).unwrap();
        assert!(code.contains("info.name = \"Worn Reel\";"), "{code}");
        assert!(code.contains("params.add(\"Dusty Tape\", 0.0f, 1.0f);") && code.contains("preset.displayName = \"Dusty Tape\";"), "{code}");
        assert_eq!(manifest, r#"{ "name": "Worn Reel", "author": "Dusty Tape", "file32": "VST/worn-reel-arm32.so" }"#);
    }

    #[test]
    fn the_name_is_found_however_the_code_spaces_it() {
        let renaming = Renaming { old_name: "Dusty Tape", new_name: "Worn Reel" };

        assert_eq!(renaming.where_it_follows(BEFORE_THE_NAME_IN_THE_CODE, "i.name=\"Dusty Tape\";"), "i.name=\"Worn Reel\";");
        assert_eq!(renaming.where_it_follows(BEFORE_THE_NAME_IN_THE_CODE, "info.name         = \"Dusty Tape\";"), "info.name         = \"Worn Reel\";");
        assert_eq!(renaming.where_it_follows(BEFORE_THE_NAME_IN_THE_MANIFEST, "\"name\" :\n\"Dusty Tape\""), "\"name\" :\n\"Worn Reel\"");
    }

    #[test]
    fn the_versions_kept_for_going_back_are_renamed_too_and_the_old_build_is_thrown_away() {
        let plugins = TemporaryFolder::create("rename-versions");
        plugin_called_dusty_tape(&plugins);

        rename(&plugins.0, "Dusty Tape", "Worn Reel").unwrap();

        let renamed = plugins.0.join("Worn Reel");
        let kept_manifest = fs::read_to_string(renamed.join(".versions/1/plugin.json")).unwrap();
        let kept_code = fs::read_to_string(renamed.join(".versions/1/src/DustyTape.cpp")).unwrap();
        assert!(kept_manifest.contains(r#""name": "Worn Reel""#) && kept_code.contains("\"Worn Reel\""));
        assert!(!renamed.join("build").exists());
    }

    #[test]
    #[ignore = "puts a made-up plugin in this computer's real trash"]
    fn a_deleted_plugin_leaves_its_folder_for_the_computers_trash() {
        let plugins = TemporaryFolder::create("delete-plugin");
        let plugin = plugins.0.join(format!("Thrown Away By A Test {}", std::process::id()));
        fs::create_dir_all(plugin.join("src")).unwrap();
        fs::write(plugin.join("plugin.json"), MANIFEST_OF_DUSTY_TAPE).unwrap();

        move_to_trash(&plugin).unwrap();

        assert!(!plugin.exists());
    }

    #[test]
    fn a_folder_that_is_not_there_cannot_be_thrown_away() {
        let plugins = TemporaryFolder::create("delete-nothing");

        assert!(move_to_trash(&plugins.0.join("Never Made")).is_err());
    }
}
