//! A Python of MP Forge's own, for a computer that has none it can use.
//!
//! Akai's tools are run by Python, and a musician should not have to install it. So MP Forge fetches one of the
//! standalone builds of Python that Astral publishes. Such a build needs nothing installed and changes nothing on
//! the computer: it is unpacked into a folder MP Forge keeps, and is only ever run from there.
//!
//! The build for each kind of computer is written down here with its fingerprint. A download that does not have
//! that fingerprint is thrown away unopened.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::archive::Archive;

const PUBLISHED_AT: &str = "https://github.com/astral-sh/python-build-standalone/releases/download/20261110";
const KEPT_AS: &str = "mp-forge-python";
const FOLDER_IN_THE_ARCHIVE: &str = "python";
const PROGRAM: &str = if cfg!(windows) { "python.exe" } else { "bin/python3" };
const LARGEST_ARCHIVE: u64 = 200 * 1024 * 1024;
const TIME_TO_DOWNLOAD: Duration = Duration::from_secs(15 * 60);
const HTTP_OK: u16 = 200;

/// The build of Python for one kind of computer.
struct Build {
    system: &'static str,
    chip: &'static str,
    file_name: &'static str,
    fingerprint: &'static str,
}

const BUILDS: [Build; 6] = [
    Build { system: "macos", chip: "aarch64", file_name: "cpython-3.12.16+20261110-aarch64-apple-darwin-install_only_stripped.tar.gz", fingerprint: "2f11b8aa3726bdbd87dd9eb76568412faddf4f47e26913fa45d6a2e424972acd" },
    Build { system: "macos", chip: "x86_64", file_name: "cpython-3.12.16+20261110-x86_64-apple-darwin-install_only_stripped.tar.gz", fingerprint: "b78a4c479f1964ec484880bee567568266f68da102269fb93a05fe8716b45f91" },
    Build { system: "windows", chip: "x86_64", file_name: "cpython-3.12.16+20261110-x86_64-pc-windows-msvc-install_only_stripped.tar.gz", fingerprint: "355ee61b8207b91574491b1c6dc9b0ebb4408fc56bfafe498f3ef439314b52dc" },
    Build { system: "windows", chip: "aarch64", file_name: "cpython-3.12.16+20261110-aarch64-pc-windows-msvc-install_only_stripped.tar.gz", fingerprint: "c1b334c7b1db8f962b37a333d8362cab0ba7ebda1a7cce917fae56a1e0c8cbe7" },
    Build { system: "linux", chip: "x86_64", file_name: "cpython-3.12.16+20261110-x86_64-unknown-linux-gnu-install_only_stripped.tar.gz", fingerprint: "6a9e62dad033790aef722dbd370e0ce76ce17b73627ba3ae679f537049350b39" },
    Build { system: "linux", chip: "aarch64", file_name: "cpython-3.12.16+20261110-aarch64-unknown-linux-gnu-install_only_stripped.tar.gz", fingerprint: "b6a41defec4975d94b4ff1950236e7383e5c02ba40ea48c9ec33c66087a35aab" },
];

impl Build {
    fn for_a_computer(system: &str, chip: &str) -> Option<&'static Build> {
        BUILDS.iter().find(|build| build.system == system && build.chip == chip)
    }

    fn for_this_computer() -> Option<&'static Build> {
        Self::for_a_computer(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// A plus sign in an address stands for a space unless it is written out this way.
    fn address(&self) -> String {
        format!("{PUBLISHED_AT}/{}", self.file_name.replace('+', "%2B"))
    }
}

/// Where MP Forge keeps its own Python, whether or not it has fetched one yet.
pub struct OwnPython {
    kept_in: PathBuf,
}

impl OwnPython {
    pub fn kept_in(folder: &Path) -> Self {
        Self { kept_in: folder.to_path_buf() }
    }

    pub fn program(&self) -> PathBuf {
        self.folder().join(PROGRAM)
    }

    pub fn is_there(&self) -> bool {
        self.program().is_file()
    }

    /// Whether Astral publishes a build for this kind of computer.
    pub fn can_be_fetched() -> bool {
        Build::for_this_computer().is_some()
    }

    /// Downloads the build for this computer, checks it and unpacks it. Needs the network this once.
    pub fn fetch(&self) -> Result<(), String> {
        let build = Build::for_this_computer().ok_or("MP Forge has no Python of its own for this kind of computer.")?;
        fs::create_dir_all(&self.kept_in).map_err(|problem| could_not_keep(&problem))?;
        let archive = self.kept_in.join(build.file_name);
        let fetched = download(&build.address(), &archive).and_then(|()| self.unpack_if_genuine(&archived(&archive), build.fingerprint));
        let _ = fs::remove_file(&archive);
        fetched
    }

    /// Unpacks the archive in place of whatever Python was kept before, once its fingerprint is the one expected.
    fn unpack_if_genuine(&self, archive: &Archive, fingerprint: &str) -> Result<(), String> {
        let found = archive.fingerprint().map_err(|problem| could_not_keep(&problem))?;
        if found != fingerprint {
            return Err("The Python that arrived is not the one MP Forge asked for, so it was thrown away. Try again.".to_string());
        }
        archive.unpack_as(&self.folder()).map_err(|problem| could_not_keep(&problem))
    }

    fn folder(&self) -> PathBuf {
        self.kept_in.join(KEPT_AS)
    }
}

/// A downloaded file, as the archive of Python it should be.
fn archived(file: &Path) -> Archive<'_> {
    Archive { file, holds: FOLDER_IN_THE_ARCHIVE }
}

fn download(address: &str, into: &Path) -> Result<(), String> {
    let client: ureq::Agent = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(TIME_TO_DOWNLOAD)).build().into();
    let response = client.get(address).call().map_err(|problem| could_not_reach(&problem))?;
    if response.status().as_u16() != HTTP_OK {
        return Err(could_not_reach(&format!("the download page answered {}", response.status().as_u16())));
    }
    let mut arriving = response.into_body().into_with_config().limit(LARGEST_ARCHIVE).reader();
    let mut kept = File::create(into).map_err(|problem| could_not_keep(&problem))?;
    io::copy(&mut arriving, &mut kept).map(|_bytes| ()).map_err(|problem| could_not_reach(&problem))
}

fn could_not_reach(problem: &dyn std::fmt::Display) -> String {
    format!("Python could not be downloaded ({problem}). Check the network and try again.")
}

fn could_not_keep(problem: &dyn std::fmt::Display) -> String {
    format!("Python could not be put in place on this computer: {problem}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{test_support, while_arriving};
    use crate::process::{command_for, run_with_time_limit};
    use crate::workspace::test_support::TemporaryFolder;

    const SAID_BY_THE_STAND_IN: &[u8] = b"a stand-in for Python";

    /// An archive laid out as Astral's are: a `python` folder holding the program.
    fn archive_in(folder: &Path) -> PathBuf {
        let archive = folder.join("python.tar.gz");
        test_support::pack(&archive, &[(&Path::new(FOLDER_IN_THE_ARCHIVE).join(PROGRAM), SAID_BY_THE_STAND_IN)]);
        archive
    }

    #[test]
    fn every_kind_of_computer_mp_forge_runs_on_has_a_build_and_no_other_has() {
        for (system, chip) in [("macos", "aarch64"), ("macos", "x86_64"), ("windows", "x86_64"), ("linux", "x86_64"), ("linux", "aarch64")] {
            assert!(Build::for_a_computer(system, chip).is_some_and(|build| build.fingerprint.len() == 64), "{system} on {chip}");
        }
        assert!(Build::for_a_computer("freebsd", "x86_64").is_none());
    }

    #[test]
    fn the_plus_sign_in_a_builds_name_is_written_out_in_its_address() {
        let address = Build::for_a_computer("linux", "x86_64").unwrap().address();

        assert!(address.starts_with("https://github.com/astral-sh/python-build-standalone/releases/download/"));
        assert!(address.contains("cpython-3.12.16%2B20261110-x86_64-unknown-linux-gnu"), "{address}");
    }

    #[test]
    fn an_archive_with_the_expected_fingerprint_is_unpacked_where_python_is_kept() {
        let folder = TemporaryFolder::create("own-python-genuine");
        let archive = archive_in(&folder.0);
        let python = OwnPython::kept_in(&folder.0);
        assert!(!python.is_there());

        python.unpack_if_genuine(&archived(&archive), &archived(&archive).fingerprint().unwrap()).unwrap();

        assert!(python.is_there());
        assert_eq!(fs::read(python.program()).unwrap(), SAID_BY_THE_STAND_IN);
        assert!(!while_arriving(&python.folder()).exists());
    }

    #[test]
    fn an_archive_with_another_fingerprint_is_never_unpacked() {
        let folder = TemporaryFolder::create("own-python-not-genuine");
        let archive = archive_in(&folder.0);
        let python = OwnPython::kept_in(&folder.0);

        let refused = python.unpack_if_genuine(&archived(&archive), &"0".repeat(64)).unwrap_err();

        assert!(refused.contains("not the one MP Forge asked for"), "{refused}");
        assert!(!python.is_there());
    }

    #[test]
    fn a_python_fetched_again_takes_the_place_of_the_one_kept_before() {
        let folder = TemporaryFolder::create("own-python-again");
        let archive = archive_in(&folder.0);
        let python = OwnPython::kept_in(&folder.0);
        let left_from_before = python.folder().join("left-from-before");
        fs::create_dir_all(&left_from_before).unwrap();

        python.unpack_if_genuine(&archived(&archive), &archived(&archive).fingerprint().unwrap()).unwrap();

        assert!(python.is_there());
        assert!(!left_from_before.exists());
    }

    /// Downloads about 25 MB from GitHub, so it is run by hand: `cargo test -- --ignored the_real_python`.
    #[test]
    #[ignore = "downloads Python from the network"]
    fn the_real_python_for_this_computer_arrives_runs_and_can_set_up_a_compiler() {
        let folder = TemporaryFolder::create("own-python-real");
        let python = OwnPython::kept_in(&folder.0);

        python.fetch().unwrap();

        let mut asked = command_for(python.program());
        asked.args(["-c", "import venv, ensurepip, sys; print(sys.version)"]);
        let said = run_with_time_limit(&mut asked, Duration::from_secs(60)).unwrap();
        assert!(said.succeeded() && said.output.starts_with("3.12."), "{}", said.output);
    }
}
