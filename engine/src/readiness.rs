//! What MP Forge needs on this computer besides itself, whether each thing is there, and how to get one that is not.
//!
//! Python runs Akai's build tools. The compiler those tools use is fetched by Akai's own build script, once;
//! `fetch_build_tools` has it do that ahead of the first build, so the first plugin does not wait on a download.
//! Nothing here installs a program on the computer itself: that is left to the musician, with the way to do it.

use std::path::PathBuf;
use std::time::Duration;

use crate::process::{RunError, command_for, is_installed, run_with_time_limit};
use crate::sdk::{PYTHON, Sdk};

const TIME_TO_ANSWER: Duration = Duration::from_secs(20);
const TIME_TO_FETCH: Duration = Duration::from_secs(15 * 60);
/// Akai's build script makes its compiler a small Python setup of its own, which needs these two parts of Python.
const PYTHON_CAN_SET_UP_THE_COMPILER: [&str; 2] = ["-c", "import venv, ensurepip"];
/// Has Akai's build script find its compiler, which fetches it when this computer has none.
const FETCH_THE_COMPILER: &str = "import sys; sys.path.insert(0, sys.argv[1]); import build; build.find_zig()";
const COMPILER: &str = "zig";
const COMPILER_SET_UP_BY_AKAI: &str = "zigenv";
const SENDS_TO_THE_MPC: &str = "ssh";
const CLAUDE_CODE: &str = "claude";

const GET_PYTHON: &str = if cfg!(target_os = "macos") {
    "Install Python 3 from python.org, then check again."
} else if cfg!(windows) {
    "Install Python 3 from the Microsoft Store, or from python.org with \"Add python.exe to PATH\" ticked, then check again."
} else {
    "Install python3 and python3-venv with your system's package manager, then check again."
};
const GET_THE_BUILD_TOOLS: &str = "Choose GET THE BUILD TOOLS. Akai's build script fetches its compiler once, about 50 MB.";
const GET_SSH: &str = if cfg!(windows) {
    "Add \"OpenSSH Client\" under Windows Settings, Optional features."
} else if cfg!(target_os = "macos") {
    "It comes with macOS. If it is missing, install Apple's command line tools."
} else {
    "Install openssh-client with your system's package manager."
};
const GET_CLAUDE_CODE: &str = "Install Claude Code from claude.com/claude-code and sign in to it. Not needed when you use a key or another service.";

/// One thing MP Forge needs, and whether this computer has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Need {
    pub name: &'static str,
    pub used_for: &'static str,
    pub is_there: bool,
    /// Without it no plugin can be made. Any other only switches off the one thing it is used for.
    pub is_required: bool,
    pub how_to_get: &'static str,
    /// MP Forge can fetch it itself when asked.
    pub can_be_fetched: bool,
}

pub fn needs_on_this_computer(sdk: &Sdk) -> Vec<Need> {
    vec![
        Need { name: "Python 3", used_for: "Runs Akai's tools that build and check a plugin.", is_there: python_can_set_up_the_compiler(), is_required: true, how_to_get: GET_PYTHON, can_be_fetched: false },
        Need { name: "Akai's build tools", used_for: "The compiler that turns a plugin's code into what the MPC runs.", is_there: has_the_compiler(sdk), is_required: true, how_to_get: GET_THE_BUILD_TOOLS, can_be_fetched: true },
        Need { name: "SSH", used_for: "Sends a plugin to your MPC over the network.", is_there: is_installed(SENDS_TO_THE_MPC), is_required: false, how_to_get: GET_SSH, can_be_fetched: false },
        Need { name: "Claude Code", used_for: "Lets Claude do the work through your own Claude sign-in, without a key.", is_there: is_installed(CLAUDE_CODE), is_required: false, how_to_get: GET_CLAUDE_CODE, can_be_fetched: false },
    ]
}

/// Has Akai's build script fetch its compiler now. Needs Python, and the network this once.
pub fn fetch_build_tools(sdk: &Sdk) -> Result<(), String> {
    let mut fetch = command_for(PYTHON);
    fetch.arg("-c").arg(FETCH_THE_COMPILER).arg(sdk.tools_folder());
    match run_with_time_limit(&mut fetch, TIME_TO_FETCH) {
        Ok(finished) if finished.succeeded() => Ok(()),
        Ok(finished) => Err(format!("The build tools could not be fetched: {}", last_line_of(&finished.output))),
        Err(RunError::TimedOut { .. }) => Err("Fetching the build tools took too long. Check the network and try again.".to_string()),
        Err(RunError::CouldNotStart(_)) => Err(format!("Python is needed to fetch the build tools. {GET_PYTHON}")),
    }
}

fn python_can_set_up_the_compiler() -> bool {
    let mut python = command_for(PYTHON);
    python.args(PYTHON_CAN_SET_UP_THE_COMPILER);
    run_with_time_limit(&mut python, TIME_TO_ANSWER).is_ok_and(|finished| finished.succeeded())
}

/// The compiler is there when the musician has it installed, or when Akai's build script has set it up before.
fn has_the_compiler(sdk: &Sdk) -> bool {
    is_installed(COMPILER) || python_set_up_by_akai(sdk).is_file()
}

fn python_set_up_by_akai(sdk: &Sdk) -> PathBuf {
    let within = if cfg!(windows) { "Scripts/python.exe" } else { "bin/python" };
    sdk.kept_tools_folder().join(COMPILER_SET_UP_BY_AKAI).join(within)
}

fn last_line_of(output: &str) -> &str {
    output.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("its tool said nothing.").trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn sdk_in(folder: &TemporaryFolder) -> Sdk {
        std::fs::create_dir_all(folder.0.join("tools")).unwrap();
        std::fs::write(folder.0.join("tools/build.py"), "").unwrap();
        Sdk::at(&folder.0).unwrap()
    }

    fn need_called<'needs>(needs: &'needs [Need], name: &str) -> &'needs Need {
        needs.iter().find(|need| need.name == name).unwrap()
    }

    #[test]
    fn only_python_and_the_build_tools_are_required_and_only_the_build_tools_can_be_fetched() {
        let folder = TemporaryFolder::create("readiness-needs");

        let needs = needs_on_this_computer(&sdk_in(&folder));

        assert_eq!(needs.iter().filter(|need| need.is_required).map(|need| need.name).collect::<Vec<_>>(), ["Python 3", "Akai's build tools"]);
        assert_eq!(needs.iter().filter(|need| need.can_be_fetched).map(|need| need.name).collect::<Vec<_>>(), ["Akai's build tools"]);
        assert!(need_called(&needs, "Python 3").is_there, "these tests are run with Python on the computer");
    }

    #[test]
    fn the_compiler_akais_build_script_set_up_before_counts_as_there() {
        let folder = TemporaryFolder::create("readiness-compiler");
        let sdk = sdk_in(&folder);
        let set_up = python_set_up_by_akai(&sdk);
        std::fs::create_dir_all(set_up.parent().unwrap()).unwrap();
        std::fs::write(&set_up, "").unwrap();

        assert!(has_the_compiler(&sdk));
    }

    #[test]
    fn what_went_wrong_is_the_last_thing_the_tool_said() {
        assert_eq!(last_line_of("Creating venv\n  pip install ziglang failed.\n\n"), "pip install ziglang failed.");
        assert_eq!(last_line_of(""), "its tool said nothing.");
    }
}
