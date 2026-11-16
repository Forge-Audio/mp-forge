//! Keeps a copy of a plugin's files before they are changed, so no change is ever irreversible.
//!
//! Versions live in a hidden folder inside the plugin, numbered in the order they were kept. The AI can neither
//! see nor write there. A version stays until the musician throws it away, or asks for only the newest few to be
//! kept, so they can step back to any of them and forward again.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const VERSIONS_FOLDER: &str = ".versions";
const FILES_KEPT: [&str; 6] = ["plugin.json", "ui.json", "README.md", "DESIGN.md", "pictures.json", "look.json"];
const FOLDERS_KEPT: [&str; 3] = ["src", "pictures", "artwork"];
/// Says when a version was kept and what was about to happen to the plugin. It is no part of the plugin.
const NOTE_FILE: &str = "version.json";

/// One kept version, as the musician is told of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub number: usize,
    /// Seconds since 1970. Zero when it cannot be told.
    pub kept_at: u64,
    /// What was about to happen when it was kept, worded to follow "Kept before". Empty when nobody said.
    pub kept_before: String,
    /// Whether the plugin's files are, right now, exactly the ones this version holds.
    pub is_how_it_is_now: bool,
    /// The room its files take on this computer.
    pub size_in_bytes: u64,
}

/// Keeps the plugin's files as they are now and returns the number of the version that holds them.
/// `about_to` says what is about to happen to the plugin, worded to follow "Kept before".
/// Files that some version already holds exactly are not kept a second time.
pub fn keep_version(plugin_dir: &Path, about_to: &str) -> io::Result<usize> {
    if let Some(already_kept) = version_holding_what_is_there_now(plugin_dir) {
        return Ok(already_kept);
    }
    let number = numbers_kept(plugin_dir).last().map_or(1, |newest| newest + 1);
    let version_dir = folder_of(plugin_dir, number);
    fs::create_dir_all(&version_dir)?;
    copy_kept_things(plugin_dir, &version_dir)?;
    fs::write(version_dir.join(NOTE_FILE), json!({ "keptAt": seconds_since_1970(SystemTime::now()), "before": about_to }).to_string())?;
    Ok(number)
}

/// Every kept version, oldest first.
pub fn versions(plugin_dir: &Path) -> Vec<Version> {
    numbers_kept(plugin_dir).into_iter().map(|number| version(plugin_dir, number)).collect()
}

/// Throws one version away for good. The plugin as it stands is not touched, and the other versions keep
/// their numbers. Returns false when there is no version with that number.
pub fn forget_version(plugin_dir: &Path, number: usize) -> io::Result<bool> {
    let version_dir = folder_of(plugin_dir, number);
    if number == 0 || !version_dir.is_dir() {
        return Ok(false);
    }
    fs::remove_dir_all(version_dir)?;
    Ok(true)
}

/// Folds the oldest versions away so that at most `most` are left, and returns how many went.
/// The oldest one left then stands for everything before it.
pub fn keep_only_newest(plugin_dir: &Path, most: usize) -> io::Result<usize> {
    let kept = numbers_kept(plugin_dir);
    let too_old = &kept[..kept.len().saturating_sub(most.max(1))];
    for number in too_old {
        fs::remove_dir_all(folder_of(plugin_dir, *number))?;
    }
    Ok(too_old.len())
}

/// Puts the plugin's files back as one version holds them. What was there is kept as a version first, so the
/// musician can step forward again. Returns false when there is no version with that number.
pub fn go_to(plugin_dir: &Path, number: usize) -> io::Result<bool> {
    let version_dir = folder_of(plugin_dir, number);
    if number == 0 || !version_dir.is_dir() {
        return Ok(false);
    }
    keep_version(plugin_dir, &format!("going back to version {number}"))?;
    for file in FILES_KEPT {
        put_back_file(&version_dir.join(file), &plugin_dir.join(file))?;
    }
    for folder in FOLDERS_KEPT {
        put_back_folder(&version_dir.join(folder), &plugin_dir.join(folder))?;
    }
    Ok(true)
}

pub fn versions_kept(plugin_dir: &Path) -> usize {
    numbers_kept(plugin_dir).len()
}

/// The number of every version there is, oldest first. Numbers of versions thrown away are not used again
/// while a newer one is kept, so a higher number is always a later version.
fn numbers_kept(plugin_dir: &Path) -> Vec<usize> {
    let Ok(entries) = fs::read_dir(plugin_dir.join(VERSIONS_FOLDER)) else { return Vec::new() };
    let is_a_version = |entry: &fs::DirEntry| entry.path().is_dir();
    let mut numbers: Vec<usize> = entries.flatten().filter(is_a_version).filter_map(|entry| entry.file_name().to_str()?.parse().ok()).collect();
    numbers.sort_unstable();
    numbers
}

fn folder_of(plugin_dir: &Path, number: usize) -> PathBuf {
    plugin_dir.join(VERSIONS_FOLDER).join(number.to_string())
}

fn version(plugin_dir: &Path, number: usize) -> Version {
    let version_dir = folder_of(plugin_dir, number);
    let note = fs::read_to_string(version_dir.join(NOTE_FILE)).ok().and_then(|written| serde_json::from_str::<Value>(&written).ok()).unwrap_or_default();
    Version {
        number,
        kept_at: note["keptAt"].as_u64().unwrap_or_else(|| when_made(&version_dir)),
        kept_before: note["before"].as_str().unwrap_or_default().to_string(),
        is_how_it_is_now: holds_what_is_there_now(&version_dir, plugin_dir),
        size_in_bytes: files_under(&version_dir).values().map(|contents| contents.len() as u64).sum(),
    }
}

/// A version kept before versions carried a note is dated by its folder.
fn when_made(version_dir: &Path) -> u64 {
    fs::metadata(version_dir).and_then(|details| details.modified()).map(seconds_since_1970).unwrap_or(0)
}

fn seconds_since_1970(moment: SystemTime) -> u64 {
    moment.duration_since(UNIX_EPOCH).map(|since| since.as_secs()).unwrap_or(0)
}

fn version_holding_what_is_there_now(plugin_dir: &Path) -> Option<usize> {
    numbers_kept(plugin_dir).into_iter().rev().find(|number| holds_what_is_there_now(&folder_of(plugin_dir, *number), plugin_dir))
}

fn holds_what_is_there_now(version_dir: &Path, plugin_dir: &Path) -> bool {
    let same_file = |file: &&str| fs::read(version_dir.join(file)).ok() == fs::read(plugin_dir.join(file)).ok();
    let same_folder = |folder: &&str| files_under(&version_dir.join(folder)) == files_under(&plugin_dir.join(folder));
    version_dir.is_dir() && FILES_KEPT.iter().all(same_file) && FOLDERS_KEPT.iter().all(same_folder)
}

/// Every file in a folder and the folders inside it, by where it sits, with what it holds.
fn files_under(folder: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut found = BTreeMap::new();
    gather_files(folder, Path::new(""), &mut found);
    found
}

fn gather_files(folder: &Path, inside: &Path, found: &mut BTreeMap<PathBuf, Vec<u8>>) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for path in entries.flatten().map(|entry| entry.path()) {
        let place = inside.join(path.file_name().unwrap_or_default());
        if path.is_dir() {
            gather_files(&path, &place, found);
        } else if let Ok(contents) = fs::read(&path) {
            found.insert(place, contents);
        }
    }
}

fn copy_kept_things(plugin_dir: &Path, version_dir: &Path) -> io::Result<()> {
    for file in FILES_KEPT {
        copy_if_present(&plugin_dir.join(file), &version_dir.join(file))?;
    }
    for folder in FOLDERS_KEPT {
        copy_folder(&plugin_dir.join(folder), &version_dir.join(folder))?;
    }
    Ok(())
}

/// A file the version did not have was added since, so going back takes it away again.
fn put_back_file(kept: &Path, current: &Path) -> io::Result<()> {
    if kept.is_file() {
        fs::copy(kept, current)?;
    } else if current.is_file() {
        fs::remove_file(current)?;
    }
    Ok(())
}

fn put_back_folder(kept: &Path, current: &Path) -> io::Result<()> {
    if current.is_dir() {
        fs::remove_dir_all(current)?;
    }
    copy_folder(kept, current)
}

fn copy_if_present(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_file() {
        fs::copy(from, to)?;
    }
    Ok(())
}

pub(crate) fn copy_folder(from: &Path, to: &Path) -> io::Result<()> {
    if !from.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let path = entry?.path();
        let destination = to.join(path.file_name().unwrap_or_default());
        if path.is_dir() { copy_folder(&path, &destination)? } else { copy_if_present(&path, &destination)? }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn plugin_with_source(contents: &str) -> TemporaryFolder {
        let plugin = TemporaryFolder::create("history");
        fs::create_dir_all(plugin.0.join("src/dsp")).unwrap();
        fs::write(plugin.0.join("plugin.json"), "{}").unwrap();
        fs::write(plugin.0.join("src/Echo.cpp"), contents).unwrap();
        fs::write(plugin.0.join("src/dsp/tape.h"), "// tape").unwrap();
        plugin
    }

    fn rewrite_source(plugin: &TemporaryFolder, contents: &str) {
        fs::write(plugin.0.join("src/Echo.cpp"), contents).unwrap();
    }

    fn source_of(plugin: &TemporaryFolder) -> String {
        fs::read_to_string(plugin.0.join("src/Echo.cpp")).unwrap()
    }

    #[test]
    fn a_kept_version_holds_the_files_as_they_were() {
        let plugin = plugin_with_source("// first");

        let number = keep_version(&plugin.0, "a change").unwrap();
        rewrite_source(&plugin, "// second");

        let version_dir = folder_of(&plugin.0, number);
        assert_eq!(fs::read_to_string(version_dir.join("src/Echo.cpp")).unwrap(), "// first");
        assert_eq!(fs::read_to_string(version_dir.join("src/dsp/tape.h")).unwrap(), "// tape");
        assert!(version_dir.join("plugin.json").is_file());
    }

    #[test]
    fn a_kept_version_holds_the_pictures_too() {
        let plugin = plugin_with_source("// first");
        fs::create_dir_all(plugin.0.join("pictures")).unwrap();
        fs::write(plugin.0.join("pictures.json"), "{}").unwrap();
        fs::write(plugin.0.join("pictures/reels.svg"), "<svg/>").unwrap();

        let version_dir = folder_of(&plugin.0, keep_version(&plugin.0, "a change").unwrap());

        assert!(version_dir.join("pictures.json").is_file());
        assert_eq!(fs::read_to_string(version_dir.join("pictures/reels.svg")).unwrap(), "<svg/>");
    }

    #[test]
    fn each_change_gets_the_next_number_and_says_what_it_was_kept_before() {
        let plugin = plugin_with_source("// a");

        keep_version(&plugin.0, "the change \u{201c}more wobble\u{201d}").unwrap();
        rewrite_source(&plugin, "// b");
        keep_version(&plugin.0, "the look was edited by hand").unwrap();

        let kept = versions(&plugin.0);
        assert_eq!(kept.iter().map(|version| version.number).collect::<Vec<_>>(), [1, 2]);
        assert_eq!(kept[0].kept_before, "the change \u{201c}more wobble\u{201d}");
        assert_eq!(kept[1].kept_before, "the look was edited by hand");
        assert!(kept.iter().all(|version| version.kept_at > 0));
    }

    #[test]
    fn files_that_a_version_already_holds_are_not_kept_twice() {
        let plugin = plugin_with_source("// a");

        let first = keep_version(&plugin.0, "a change").unwrap();
        let again = keep_version(&plugin.0, "another change").unwrap();

        assert_eq!((first, again), (1, 1));
        assert_eq!(versions_kept(&plugin.0), 1);
    }

    #[test]
    fn the_version_that_matches_the_plugin_as_it_stands_is_marked() {
        let plugin = plugin_with_source("// a");
        keep_version(&plugin.0, "a change").unwrap();
        rewrite_source(&plugin, "// b");
        keep_version(&plugin.0, "a change").unwrap();
        rewrite_source(&plugin, "// c");

        assert!(versions(&plugin.0).iter().all(|version| !version.is_how_it_is_now));
        rewrite_source(&plugin, "// a");

        let marked: Vec<usize> = versions(&plugin.0).iter().filter(|version| version.is_how_it_is_now).map(|version| version.number).collect();
        assert_eq!(marked, [1]);
    }

    #[test]
    fn going_to_a_version_restores_it_and_removes_what_was_added_since() {
        let plugin = plugin_with_source("// first");
        keep_version(&plugin.0, "a change").unwrap();
        rewrite_source(&plugin, "// second");
        fs::write(plugin.0.join("src/Extra.cpp"), "// added later").unwrap();
        fs::write(plugin.0.join("pictures.json"), "{}").unwrap();

        let went = go_to(&plugin.0, 1).unwrap();

        assert!(went);
        assert_eq!(source_of(&plugin), "// first");
        assert_eq!(fs::read_to_string(plugin.0.join("src/dsp/tape.h")).unwrap(), "// tape");
        assert!(!plugin.0.join("src/Extra.cpp").exists());
        assert!(!plugin.0.join("pictures.json").exists());
    }

    #[test]
    fn going_back_keeps_what_was_there_so_the_musician_can_step_forward_again() {
        let plugin = plugin_with_source("// first");
        keep_version(&plugin.0, "a change").unwrap();
        rewrite_source(&plugin, "// second");

        go_to(&plugin.0, 1).unwrap();
        let kept = versions(&plugin.0);
        go_to(&plugin.0, 2).unwrap();

        assert_eq!(kept.len(), 2);
        assert_eq!(kept[1].kept_before, "going back to version 1");
        assert!(kept[0].is_how_it_is_now);
        assert_eq!(source_of(&plugin), "// second");
        assert_eq!(versions_kept(&plugin.0), 2);
    }

    #[test]
    fn going_back_restores_what_the_musician_had_arranged() {
        let plugin = plugin_with_source("// first");
        fs::write(plugin.0.join("look.json"), "before editing").unwrap();
        keep_version(&plugin.0, "the look was edited by hand").unwrap();
        fs::write(plugin.0.join("look.json"), "after editing").unwrap();

        go_to(&plugin.0, 1).unwrap();

        assert_eq!(fs::read_to_string(plugin.0.join("look.json")).unwrap(), "before editing");
    }

    /// Keeps three versions, of sources "// 1" to "// 3", and leaves the plugin at "// 4".
    fn plugin_with_three_versions() -> TemporaryFolder {
        let plugin = plugin_with_source("// 1");
        for next in 2..=4 {
            keep_version(&plugin.0, "a change").unwrap();
            rewrite_source(&plugin, &format!("// {next}"));
        }
        plugin
    }

    fn numbers_of(plugin: &TemporaryFolder) -> Vec<usize> {
        versions(&plugin.0).iter().map(|version| version.number).collect()
    }

    #[test]
    fn a_version_thrown_away_is_gone_and_the_others_keep_their_numbers() {
        let plugin = plugin_with_three_versions();

        assert!(forget_version(&plugin.0, 2).unwrap());

        assert_eq!(numbers_of(&plugin), [1, 3]);
        assert_eq!(source_of(&plugin), "// 4", "the plugin as it stands is not touched");
        assert!(!forget_version(&plugin.0, 2).unwrap());
        assert!(!forget_version(&plugin.0, 0).unwrap());
    }

    #[test]
    fn the_next_version_is_numbered_after_the_newest_one_left() {
        let plugin = plugin_with_three_versions();
        forget_version(&plugin.0, 1).unwrap();

        assert_eq!(keep_version(&plugin.0, "a change").unwrap(), 4);
        assert_eq!(numbers_of(&plugin), [2, 3, 4]);
        assert!(go_to(&plugin.0, 2).unwrap());
        assert_eq!(source_of(&plugin), "// 2");
    }

    #[test]
    fn folding_keeps_only_the_newest_versions() {
        let plugin = plugin_with_three_versions();

        assert_eq!(keep_only_newest(&plugin.0, 2).unwrap(), 1);
        assert_eq!(numbers_of(&plugin), [2, 3]);
        assert_eq!(keep_only_newest(&plugin.0, 2).unwrap(), 0);
        assert_eq!(keep_only_newest(&plugin.0, 0).unwrap(), 1, "one version is always left to go back to");
        assert_eq!(numbers_of(&plugin), [3]);
    }

    #[test]
    fn a_version_says_how_much_room_its_files_take() {
        let plugin = plugin_with_source("// first");
        keep_version(&plugin.0, "a change").unwrap();

        let kept_files = "{}".len() + "// first".len() + "// tape".len();

        assert!(versions(&plugin.0)[0].size_in_bytes >= kept_files as u64);
    }

    #[test]
    fn there_is_no_version_to_go_to_before_the_first_change() {
        let plugin = plugin_with_source("// first");

        assert!(!go_to(&plugin.0, 1).unwrap());
        assert!(!go_to(&plugin.0, 0).unwrap());
        assert_eq!(source_of(&plugin), "// first");
        assert!(versions(&plugin.0).is_empty());
    }

    #[test]
    fn a_version_kept_before_versions_carried_a_note_is_still_listed() {
        let plugin = plugin_with_source("// first");
        fs::create_dir_all(plugin.0.join(".versions/1/src")).unwrap();
        fs::write(plugin.0.join(".versions/1/src/Echo.cpp"), "// older").unwrap();

        let kept = versions(&plugin.0);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].kept_before, "");
        assert!(kept[0].kept_at > 0);
    }
}
