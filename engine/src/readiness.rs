//! What MP Forge needs on this computer besides itself, whether each thing is there, and how to get one that is not.
//!
//! Python runs Akai's build tools, and the compiler those tools use is fetched by Akai's own build script. A
//! musician is not sent to get either: `fetch_what_is_missing` fetches a Python of MP Forge's own when the
//! computer has none that will do, and has Akai's script fetch its compiler. Both are kept in a folder of MP
//! Forge's. Nothing here installs a program on the computer itself.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use crate::own_python::OwnPython;
use crate::process::{RunError, command_for, is_installed, run_with_time_limit};
use crate::sdk::Sdk;

const TIME_TO_ANSWER: Duration = Duration::from_secs(20);
const TIME_TO_FETCH: Duration = Duration::from_secs(15 * 60);
/// Akai's build script makes its compiler a small Python setup of its own, which needs these two parts of Python.
const PYTHON_CAN_SET_UP_THE_COMPILER: [&str; 2] = ["-c", "import venv, ensurepip"];
/// Has Akai's build script find its compiler, which fetches it when this computer has none.
const FETCH_THE_COMPILER: &str = "import sys; sys.path.insert(0, sys.argv[1]); import build; build.find_zig()";
const PYTHON: &str = "Python 3";
const BUILD_TOOLS: &str = "Akai's build tools";
const COMPILER: &str = "zig";
const COMPILER_SET_UP_BY_AKAI: &str = "zigenv";
/// Asks the compiler Akai's build script set up which version it is. Only one that works can answer.
const COMPILER_SAYS_ITS_VERSION: [&str; 3] = ["-m", "ziglang", "version"];
const SENDS_TO_THE_MPC: &str = "ssh";
const CLAUDE_CODE: &str = "claude";

const FETCH_PYTHON: &str = "Choose GET WHAT IS MISSING. MP Forge fetches a Python of its own, about 30 MB, and installs nothing on your computer.";
/// For a kind of computer there is no ready-made Python for.
const INSTALL_PYTHON: &str = if cfg!(target_os = "macos") {
    "Install Python 3 from python.org, then check again."
} else if cfg!(windows) {
    "Install Python 3 from the Microsoft Store, or from python.org with \"Add python.exe to PATH\" ticked, then check again."
} else {
    "Install python3 and python3-venv with your system's package manager, then check again."
};
const GET_THE_BUILD_TOOLS: &str = "Choose GET WHAT IS MISSING. Akai's build script fetches its compiler once, about 50 MB.";
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
    let python_can_be_fetched = OwnPython::can_be_fetched();
    let get_python = if python_can_be_fetched { FETCH_PYTHON } else { INSTALL_PYTHON };
    vec![
        Need { name: PYTHON, used_for: "Runs Akai's tools that build and check a plugin.", is_there: python_can_set_up_the_compiler(sdk), is_required: true, how_to_get: get_python, can_be_fetched: python_can_be_fetched },
        Need { name: BUILD_TOOLS, used_for: "The compiler that turns a plugin's code into what the MPC runs.", is_there: has_a_working_compiler(sdk), is_required: true, how_to_get: GET_THE_BUILD_TOOLS, can_be_fetched: true },
        Need { name: "SSH", used_for: "Sends a plugin to your MPC over the network.", is_there: is_installed(SENDS_TO_THE_MPC), is_required: false, how_to_get: GET_SSH, can_be_fetched: false },
        Need { name: "Claude Code", used_for: "Lets Claude do the work through your own Claude sign-in, without a key.", is_there: is_installed(CLAUDE_CODE), is_required: false, how_to_get: GET_CLAUDE_CODE, can_be_fetched: false },
    ]
}

/// Fetches whichever of Python and Akai's build tools this computer lacks, so that the musician never has to.
/// Each is named to `tell_of_fetching` as its fetching begins. Needs the network, and takes a minute or two.
/// Asked for twice at once, the second waits for the first and then finds nothing left to fetch.
pub fn fetch_what_is_missing(sdk: &Sdk, tell_of_fetching: &dyn Fn(&'static str)) -> Result<(), String> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _only_this_one = ONE_AT_A_TIME.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !python_can_set_up_the_compiler(sdk) {
        tell_of_fetching(PYTHON);
        sdk.own_python().fetch()?;
    }
    if has_a_working_compiler(sdk) {
        return Ok(());
    }
    tell_of_fetching(BUILD_TOOLS);
    fetch_build_tools(sdk)
}

/// Has Akai's build script fetch its compiler now. Needs Python, and the network this once.
fn fetch_build_tools(sdk: &Sdk) -> Result<(), String> {
    clear_away_a_compiler_that_stopped_working(sdk);
    let mut fetch = sdk.python();
    fetch.arg("-c").arg(FETCH_THE_COMPILER).arg(sdk.tools_folder());
    match run_with_time_limit(&mut fetch, TIME_TO_FETCH) {
        Ok(finished) if finished.succeeded() => Ok(()),
        Ok(finished) => Err(format!("The build tools could not be fetched: {}", last_line_of(&finished.output))),
        Err(RunError::TimedOut { .. }) => Err("Fetching the build tools took too long. Check the network and try again.".to_string()),
        Err(RunError::CouldNotStart(_)) => Err("Python is needed to fetch the build tools, and it could not be started.".to_string()),
    }
}

/// Akai's build script keeps a compiler it finds set up, working or not. One left behind by a Python that has
/// since gone from the computer is removed, so the script sets it up afresh.
fn clear_away_a_compiler_that_stopped_working(sdk: &Sdk) {
    let set_up = sdk.kept_tools_folder().join(COMPILER_SET_UP_BY_AKAI);
    if set_up.exists() && !compiler_set_up_by_akai_works(sdk) {
        let _ = fs::remove_dir_all(set_up);
    }
}

fn python_can_set_up_the_compiler(sdk: &Sdk) -> bool {
    let mut python = sdk.python();
    python.args(PYTHON_CAN_SET_UP_THE_COMPILER);
    answers(&mut python)
}

/// The compiler is there when the musician has it installed, or when the one Akai's build script set up works.
fn has_a_working_compiler(sdk: &Sdk) -> bool {
    is_installed(COMPILER) || compiler_set_up_by_akai_works(sdk)
}

fn compiler_set_up_by_akai_works(sdk: &Sdk) -> bool {
    let its_python = python_set_up_by_akai(sdk);
    if !its_python.is_file() {
        return false;
    }
    let mut compiler = command_for(its_python);
    compiler.args(COMPILER_SAYS_ITS_VERSION);
    answers(&mut compiler)
}

fn answers(asked: &mut Command) -> bool {
    run_with_time_limit(asked, TIME_TO_ANSWER).is_ok_and(|finished| finished.succeeded())
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

    fn akais_real_sdk() -> Sdk {
        Sdk::at(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../vendor/mpc-plugin-builder-sdk")).unwrap_or_else(|not_found| panic!("{not_found}"))
    }

    /// Leaves what a compiler would leave if the Python it was set up with had since gone: a file that cannot run.
    fn with_a_compiler_that_cannot_run(sdk: &Sdk) -> PathBuf {
        let set_up = python_set_up_by_akai(sdk);
        std::fs::create_dir_all(set_up.parent().unwrap()).unwrap();
        std::fs::write(&set_up, "").unwrap();
        sdk.kept_tools_folder().join(COMPILER_SET_UP_BY_AKAI)
    }

    #[test]
    fn python_and_the_build_tools_are_all_that_is_required_and_both_can_be_fetched() {
        let folder = TemporaryFolder::create("readiness-needs");

        let needs = needs_on_this_computer(&sdk_in(&folder));

        assert_eq!(needs.iter().filter(|need| need.is_required).map(|need| need.name).collect::<Vec<_>>(), ["Python 3", "Akai's build tools"]);
        assert_eq!(needs.iter().filter(|need| need.can_be_fetched).map(|need| need.name).collect::<Vec<_>>(), ["Python 3", "Akai's build tools"]);
        assert!(need_called(&needs, "Python 3").is_there, "these tests are run with Python on the computer");
    }

    #[test]
    fn the_compiler_akais_build_script_set_up_counts_as_there_when_it_works() {
        assert!(compiler_set_up_by_akai_works(&akais_real_sdk()), "these tests are run after Akai's build tools were set up");
    }

    #[test]
    fn a_compiler_that_stopped_working_does_not_count_and_is_cleared_away_before_it_is_fetched_again() {
        let folder = TemporaryFolder::create("readiness-broken-compiler");
        let sdk = sdk_in(&folder);
        let set_up = with_a_compiler_that_cannot_run(&sdk);
        assert!(!compiler_set_up_by_akai_works(&sdk));

        clear_away_a_compiler_that_stopped_working(&sdk);

        assert!(!set_up.exists());
    }

    #[test]
    fn a_computer_that_has_everything_is_fetched_nothing() {
        let fetched = std::cell::RefCell::new(Vec::new());

        fetch_what_is_missing(&akais_real_sdk(), &|name| fetched.borrow_mut().push(name)).unwrap();

        assert!(fetched.borrow().is_empty(), "{fetched:?}");
    }

    /// Akai's SDK as it is before anything has been fetched for it: none of what its build script keeps.
    fn akais_sdk_with_nothing_fetched(folder: &TemporaryFolder) -> Sdk {
        let real = akais_real_sdk();
        for part in ["tools", "framework", "examples", "templates", "ui-kit", "schemas"] {
            crate::history::copy_folder(&real.root().join(part), &folder.0.join(part)).unwrap();
        }
        Sdk::at(&folder.0).unwrap()
    }

    /// Downloads Python and Akai's compiler, about 100 MB, so it is run by hand:
    /// `cargo test -- --ignored a_computer_with_nothing`.
    #[test]
    #[ignore = "downloads Python and Akai's compiler from the network"]
    fn a_computer_with_nothing_is_made_ready_by_mp_forges_own_python_and_then_builds_a_plugin() {
        let folder = TemporaryFolder::create("readiness-from-nothing");
        let sdk = akais_sdk_with_nothing_fetched(&folder);
        let fetched = std::cell::RefCell::new(Vec::new());
        sdk.own_python().fetch().unwrap();

        fetch_what_is_missing(&sdk, &|name| fetched.borrow_mut().push(name)).unwrap();
        let built = sdk.build(&folder.0.join("examples/StereoDelay"), TIME_TO_FETCH).unwrap();

        assert_eq!(*fetched.borrow(), [BUILD_TOOLS], "MP Forge's own Python was there, so only the compiler was fetched");
        assert!(compiler_set_up_by_akai_works(&sdk));
        assert!(built.succeeded(), "{}", built.output);
    }

    #[test]
    fn a_compiler_that_works_is_left_where_it_is() {
        let sdk = akais_real_sdk();

        clear_away_a_compiler_that_stopped_working(&sdk);

        assert!(compiler_set_up_by_akai_works(&sdk));
    }

    #[test]
    fn mp_forges_own_python_is_used_in_place_of_the_computers_once_it_is_there() {
        let folder = TemporaryFolder::create("readiness-own-python");
        let sdk = sdk_in(&folder);
        let own = sdk.own_python().program();
        assert_ne!(sdk.python().get_program(), own.as_os_str());
        std::fs::create_dir_all(own.parent().unwrap()).unwrap();
        std::fs::write(&own, "").unwrap();

        assert_eq!(sdk.python().get_program(), own.as_os_str());
    }

    #[test]
    fn what_went_wrong_is_the_last_thing_the_tool_said() {
        assert_eq!(last_line_of("Creating venv\n  pip install ziglang failed.\n\n"), "pip install ziglang failed.");
        assert_eq!(last_line_of(""), "its tool said nothing.");
    }
}
