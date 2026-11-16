//! Builds the app, and notes which commit it is built from, so that two copies of one version can be told apart.

use std::process::Command;

/// The commit's short id and the day it was made, as git prints them.
const SAYS_THE_COMMIT: [&str; 3] = ["log", "-1", "--format=%h of %cs"];
/// Where git keeps which commit is checked out. The note is made again whenever that changes.
const WHERE_GIT_KEEPS_THE_COMMIT: [&str; 2] = ["../.git/HEAD", "../.git/refs/heads"];

fn main() {
    note_the_commit();
    tauri_build::build()
}

/// A copy built where git is not at hand, or outside a checkout, goes without the note.
fn note_the_commit() {
    WHERE_GIT_KEEPS_THE_COMMIT.iter().for_each(|place| println!("cargo:rerun-if-changed={place}"));
    if let Some(commit) = commit_built_from() {
        println!("cargo:rustc-env=MP_FORGE_BUILT_FROM={commit}");
    }
}

fn commit_built_from() -> Option<String> {
    let said = Command::new("git").args(SAYS_THE_COMMIT).output().ok()?;
    let commit = String::from_utf8(said.stdout).ok()?.trim().to_string();
    (said.status.success() && !commit.is_empty()).then_some(commit)
}
