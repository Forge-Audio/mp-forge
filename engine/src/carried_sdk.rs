//! Akai's SDK as an installed copy of MP Forge carries it.
//!
//! An installer puts its files where they cannot be written to, and Akai's build script keeps its compiler inside
//! the SDK's own folder. So an installer carries the SDK as one archive, and MP Forge unpacks it among the
//! musician's own things the first time it is opened.
//!
//! The unpacked folder is named after the archive's fingerprint. A later MP Forge that carries a newer SDK so
//! unpacks afresh, and the older SDK is cleared away with the tools that were fetched for it.

use std::fs;
use std::path::{Path, PathBuf};

use crate::archive::Archive;

const CARRIED_AS: &str = "akai-sdk.tar.gz";
const FOLDER_IN_THE_ARCHIVE: &str = "akai-sdk";
/// Kept short: Akai's compiler has files deep inside the SDK's folder, and Windows refuses a path past 260 letters.
const UNPACKED_NAME_STARTS: &str = "sdk-";
const FINGERPRINT_LETTERS_IN_THE_NAME: usize = 12;

pub struct CarriedSdk {
    archive: PathBuf,
}

impl CarriedSdk {
    /// The SDK among the files an installer put on this computer. None when MP Forge was not installed by one,
    /// as when it is run from its source folder.
    pub fn among(installed_files: &Path) -> Option<Self> {
        let archive = installed_files.join(CARRIED_AS);
        archive.is_file().then_some(Self { archive })
    }

    /// The folder under `kept_in` that holds the SDK unpacked, which is unpacked there first unless it already is.
    pub fn unpacked_in(&self, kept_in: &Path) -> Result<PathBuf, String> {
        let archive = Archive { file: &self.archive, holds: FOLDER_IN_THE_ARCHIVE };
        let unpacked = kept_in.join(folder_name_for(&archive.fingerprint().map_err(|problem| could_not_unpack(&problem))?));
        if !unpacked.is_dir() {
            fs::create_dir_all(kept_in).and_then(|()| archive.unpack_as(&unpacked)).map_err(|problem| could_not_unpack(&problem))?;
            clear_away_older_than(&unpacked);
        }
        Ok(unpacked)
    }
}

fn folder_name_for(fingerprint: &str) -> String {
    format!("{UNPACKED_NAME_STARTS}{}", &fingerprint[..FINGERPRINT_LETTERS_IN_THE_NAME])
}

/// An older SDK that cannot be cleared away just now is left for the next time a newer one is unpacked.
fn clear_away_older_than(unpacked: &Path) {
    for older in other_unpacked_sdks_beside(unpacked) {
        let _ = fs::remove_dir_all(older);
    }
}

fn other_unpacked_sdks_beside(unpacked: &Path) -> Vec<PathBuf> {
    let beside = unpacked.parent().and_then(|kept_in| fs::read_dir(kept_in).ok());
    let folders = beside.into_iter().flatten().flatten().map(|entry| entry.path());
    folders.filter(|folder| folder != unpacked && is_an_unpacked_sdk(folder)).collect()
}

fn is_an_unpacked_sdk(folder: &Path) -> bool {
    let named_as_one = folder.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with(UNPACKED_NAME_STARTS));
    named_as_one && folder.is_dir()
}

fn could_not_unpack(problem: &dyn std::fmt::Display) -> String {
    format!("Akai's SDK, which comes with MP Forge, could not be unpacked on this computer: {problem}. Check that the disk is not full, then open MP Forge again.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::test_support;
    use crate::sdk::Sdk;
    use crate::workspace::test_support::TemporaryFolder;

    const BUILD_SCRIPT_IN_THE_ARCHIVE: &str = "akai-sdk/tools/build.py";

    /// What an installer puts on a computer: the SDK as an archive, here holding only its build script.
    fn installed_files_with(build_script: &[u8], folder: &TemporaryFolder) -> PathBuf {
        let installed = folder.0.join("installed");
        fs::create_dir_all(&installed).unwrap();
        test_support::pack(&installed.join(CARRIED_AS), &[(Path::new(BUILD_SCRIPT_IN_THE_ARCHIVE), build_script)]);
        installed
    }

    #[test]
    fn mp_forge_run_from_its_source_folder_carries_no_sdk() {
        let folder = TemporaryFolder::create("carried-sdk-none");

        assert!(CarriedSdk::among(&folder.0).is_none());
    }

    #[test]
    fn the_carried_sdk_is_unpacked_where_it_can_be_written_to() {
        let folder = TemporaryFolder::create("carried-sdk-unpacked");
        let carried = CarriedSdk::among(&installed_files_with(b"build", &folder)).unwrap();

        let unpacked = carried.unpacked_in(&folder.0.join("kept")).unwrap();

        assert!(Sdk::at(&unpacked).is_ok());
        assert!(unpacked.starts_with(folder.0.join("kept")));
    }

    #[test]
    fn an_sdk_unpacked_before_is_used_as_it_is_with_the_tools_fetched_for_it() {
        let folder = TemporaryFolder::create("carried-sdk-again");
        let carried = CarriedSdk::among(&installed_files_with(b"build", &folder)).unwrap();
        let unpacked = carried.unpacked_in(&folder.0.join("kept")).unwrap();
        let fetched_tools = unpacked.join(".sdk-tools");
        fs::create_dir_all(&fetched_tools).unwrap();

        let unpacked_again = carried.unpacked_in(&folder.0.join("kept")).unwrap();

        assert_eq!(unpacked_again, unpacked);
        assert!(fetched_tools.is_dir());
    }

    #[test]
    fn a_newer_sdk_is_unpacked_afresh_and_the_older_one_cleared_away() {
        let folder = TemporaryFolder::create("carried-sdk-newer");
        let kept = folder.0.join("kept");
        let older = CarriedSdk::among(&installed_files_with(b"older build", &folder)).unwrap().unpacked_in(&kept).unwrap();
        let something_else_kept_there = kept.join("settings");
        fs::create_dir_all(&something_else_kept_there).unwrap();

        let newer = CarriedSdk::among(&installed_files_with(b"newer build", &folder)).unwrap().unpacked_in(&kept).unwrap();

        assert_ne!(newer, older);
        assert!(Sdk::at(&newer).is_ok());
        assert!(!older.exists());
        assert!(something_else_kept_there.is_dir());
    }

    /// Run by hand once `scripts/pack-sdk-for-installers.sh` has packed it: `cargo test -- --ignored the_real_sdk`.
    #[test]
    #[ignore = "needs Akai's SDK packed for the installers"]
    fn the_real_sdk_packed_for_the_installers_unpacks_with_all_its_parts() {
        let folder = TemporaryFolder::create("carried-sdk-real");
        let packed_by_the_script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/carried");

        let unpacked = CarriedSdk::among(&packed_by_the_script).unwrap().unpacked_in(&folder.0).unwrap();

        assert!(Sdk::at(&unpacked).is_ok());
        for part in ["framework", "templates/effect/plugin.json", "templates/instrument/src/{{CLASS}}.cpp", "ui-kit", "docs", "LICENSE"] {
            assert!(unpacked.join(part).exists(), "{part} did not arrive");
        }
        assert_eq!(fs::read_dir(&folder.0).unwrap().count(), 1, "something was left beside the unpacked SDK");
    }

    #[test]
    fn an_sdk_that_cannot_be_unpacked_is_explained_to_the_musician() {
        let folder = TemporaryFolder::create("carried-sdk-broken");
        let installed = folder.0.join("installed");
        fs::create_dir_all(&installed).unwrap();
        fs::write(installed.join(CARRIED_AS), "not an archive").unwrap();

        let problem = CarriedSdk::among(&installed).unwrap().unpacked_in(&folder.0.join("kept")).unwrap_err();

        assert!(problem.starts_with("Akai's SDK, which comes with MP Forge, could not be unpacked"), "{problem}");
    }
}
