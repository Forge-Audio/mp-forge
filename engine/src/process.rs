//! Runs an external command with a time limit, so a frozen tool or plugin can never hang the gate.
//!
//! A command that is stopped is ended together with everything it started in turn. Ending the command alone
//! would leave a compiler it started, or a plugin it was trying out, at work on files something else now holds.
//!
//! Every program MP Forge starts is started through `command_for`, which is where macOS, Windows and Linux differ
//! in how a program is found and shown.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// Lists every running process with the one that started it, a pair of numbers on each line.
const LIST_OF_WHAT_IS_RUNNING: (&str, [&str; 2]) = ("ps", ["-axo", "pid=,ppid="]);
const END_AT_ONCE: (&str, &str) = ("kill", "-KILL");
/// On Windows one program ends a process together with everything it started, given the process's number.
const END_WITH_ALL_IT_STARTED_ON_WINDOWS: (&str, [&str; 3]) = ("taskkill", ["/T", "/F", "/PID"]);
/// The endings Windows runs a program under, a program proper before a script.
const ENDINGS_WINDOWS_RUNS: [&str; 3] = ["exe", "cmd", "bat"];
/// Has the musician's own shell say which folders it looks for programs in, between marks nothing else prints.
const SHELL_SAYS_ITS_FOLDERS: [&str; 2] = ["-ilc", "printf '<<folders:%s>>' \"$PATH\""];
const FOLDERS_ARE_SAID_BETWEEN: (&str, &str) = ("<<folders:", ">>");
const LONGEST_WAIT_FOR_THE_SHELL: Duration = Duration::from_secs(5);

/// A command for a program named the way a person names it, such as `claude` or `python`. On Windows it is found
/// under whichever ending it was installed with, and it is started without a console window of its own.
pub fn command_for(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(as_this_system_names(program.as_ref()));
    command.env("PATH", searched_folders());
    without_a_window_of_its_own(&mut command);
    command
}

/// Whether this computer has the program among those it can start by name.
pub fn is_installed(program: &str) -> bool {
    let mut folders = env::split_paths(searched_folders());
    if cfg!(windows) { installed_among(folders, OsStr::new(program)).is_some() } else { folders.any(|folder| folder.join(program).is_file()) }
}

/// Where programs are looked for. An app opened from the desktop, and not from a terminal, is only told of the
/// system's own folders, so a program the musician installed themselves, such as `claude`, would not be found.
/// On macOS and Linux the folders their own shell looks in are asked for once and looked in as well.
fn searched_folders() -> &'static OsString {
    static SEARCHED: OnceLock<OsString> = OnceLock::new();
    SEARCHED.get_or_init(|| {
        let told_of = env::var_os("PATH").unwrap_or_default();
        let of_the_shell = if cfg!(unix) { folders_the_shell_looks_in() } else { None };
        of_the_shell.and_then(|of_the_shell| both_searched(&told_of, &of_the_shell)).unwrap_or(told_of)
    })
}

fn both_searched(first: &OsStr, then: &OsStr) -> Option<OsString> {
    env::join_paths(env::split_paths(first).chain(env::split_paths(then))).ok()
}

fn folders_the_shell_looks_in() -> Option<OsString> {
    let mut shell = Command::new(env::var_os("SHELL")?);
    shell.args(SHELL_SAYS_ITS_FOLDERS);
    let said = run_with_time_limit(&mut shell, LONGEST_WAIT_FOR_THE_SHELL).ok()?.output;
    folders_said_in(&said).map(OsString::from)
}

/// A shell prints greetings and warnings of its own as it starts, so only what stands between the marks is taken.
fn folders_said_in(said: &str) -> Option<&str> {
    let (opening, closing) = FOLDERS_ARE_SAID_BETWEEN;
    let (_, after_the_opening) = said.split_once(opening)?;
    let (folders, _) = after_the_opening.split_once(closing)?;
    Some(folders).filter(|folders| !folders.is_empty())
}

/// Windows only finds a program by its bare name when it ends in `.exe`. One installed as a script, as `claude.cmd`
/// is by some installers, has to be asked for by its whole name.
fn as_this_system_names(program: &OsStr) -> OsString {
    if !cfg!(windows) {
        return program.to_os_string();
    }
    let searched = env::var_os("PATH").unwrap_or_default();
    installed_among(env::split_paths(&searched), program).map_or_else(|| program.to_os_string(), PathBuf::into_os_string)
}

/// The program's file in the first of these folders to hold it. A program asked for by a path, or by a name
/// that has its ending already, is not looked for: it is run as named.
fn installed_among(folders: impl Iterator<Item = PathBuf>, program: &OsStr) -> Option<PathBuf> {
    let named = Path::new(program);
    let is_a_bare_name = named.components().count() == 1 && named.extension().is_none();
    let under_each_ending = |folder: PathBuf| ENDINGS_WINDOWS_RUNS.map(|ending| folder.join(named).with_extension(ending));
    folders.filter(|_| is_a_bare_name).flat_map(under_each_ending).find(|file| file.is_file())
}

/// An app with a window of its own that starts a program without one is given a black console window for it,
/// unless Windows is told not to.
#[cfg(windows)]
fn without_a_window_of_its_own(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const NO_CONSOLE_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(NO_CONSOLE_WINDOW);
}

#[cfg(not(windows))]
fn without_a_window_of_its_own(_command: &mut Command) {}

/// A command that is neither given anything nor heard from: only whether it ran matters.
fn unheard(program: &str) -> Command {
    let mut command = command_for(program);
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    command
}

#[derive(Debug)]
pub struct Finished {
    /// `None` when the command was ended by a signal instead of exiting.
    pub exit_code: Option<i32>,
    pub output: String,
}

impl Finished {
    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }
}

#[derive(Debug)]
pub enum RunError {
    CouldNotStart(io::Error),
    TimedOut { limit: Duration, output: String },
}

pub fn run_with_time_limit(command: &mut Command, limit: Duration) -> Result<Finished, RunError> {
    let capture = OutputCapture::create().map_err(RunError::CouldNotStart)?;
    let mut child = capture.spawn(command).map_err(RunError::CouldNotStart)?;
    let ending = wait_until(&mut child, Instant::now() + limit);
    let output = capture.read_and_discard();
    match ending {
        Ending::Exited(exit_code) => Ok(Finished { exit_code, output }),
        Ending::Stopped => Err(RunError::TimedOut { limit, output }),
    }
}

enum Ending {
    Exited(Option<i32>),
    Stopped,
}

fn wait_until(child: &mut Child, deadline: Instant) -> Ending {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ending::Exited(status.code()),
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            _ => {
                end_with_all_it_started(child);
                return Ending::Stopped;
            }
        }
    }
}

/// Ends a command that is still running, and everything it started in turn, and waits for the command to be gone.
pub(crate) fn end_with_all_it_started(child: &mut Child) {
    if has_ended(child) {
        return;
    }
    if cfg!(windows) { end_with_its_tree_on_windows(child) } else { end_with_what_it_started(child) }
    let _ = child.wait();
}

fn end_with_its_tree_on_windows(child: &mut Child) {
    let (program, arguments) = END_WITH_ALL_IT_STARTED_ON_WINDOWS;
    let _ = unheard(program).args(arguments).arg(child.id().to_string()).status();
    let _ = child.kill();
}

/// What it started is looked up first: once a command is gone, what it started no longer says whose it was.
fn end_with_what_it_started(child: &mut Child) {
    let started = started_by(child.id(), &everything_running());
    let _ = child.kill();
    end_each(&started);
}

fn has_ended(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(Some(_)))
}

struct Running {
    id: u32,
    started_by: u32,
}

/// Everything the process started and everything those started in turn, the nearest first.
fn started_by(process_id: u32, running: &[Running]) -> Vec<u32> {
    let mut family = vec![process_id];
    let mut looked_at = 0;
    while let Some(&parent) = family.get(looked_at) {
        family.extend(running.iter().filter(|process| process.started_by == parent).map(|process| process.id));
        looked_at += 1;
    }
    family.split_off(1)
}

fn everything_running() -> Vec<Running> {
    let (program, arguments) = LIST_OF_WHAT_IS_RUNNING;
    let listed = command_for(program).args(arguments).stdin(Stdio::null()).stderr(Stdio::null()).output();
    listed.map(|list| String::from_utf8_lossy(&list.stdout).lines().filter_map(running_on).collect()).unwrap_or_default()
}

fn running_on(line: &str) -> Option<Running> {
    let mut numbers = line.split_whitespace().map(str::parse::<u32>);
    Some(Running { id: numbers.next()?.ok()?, started_by: numbers.next()?.ok()? })
}

fn end_each(process_ids: &[u32]) {
    if process_ids.is_empty() {
        return;
    }
    let (program, at_once) = END_AT_ONCE;
    let _ = unheard(program).arg(at_once).args(process_ids.iter().map(u32::to_string)).status();
}

/// Collects a command's output in a temporary file. A pipe would fill up and stall a chatty command.
struct OutputCapture {
    path: PathBuf,
    file: File,
}

impl OutputCapture {
    fn create() -> io::Result<Self> {
        let path = temporary_file_of_its_own("log");
        let file = File::create(&path)?;
        Ok(Self { path, file })
    }

    fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        command
            .stdin(Stdio::null())
            .stdout(self.file.try_clone()?)
            .stderr(self.file.try_clone()?)
            .spawn()
    }

    fn read_and_discard(self) -> String {
        let bytes = fs::read(&self.path).unwrap_or_default();
        let _ = fs::remove_file(&self.path);
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// A place among the computer's temporary files that nothing else uses, for a file with this ending.
pub(crate) fn temporary_file_of_its_own(ending: &str) -> PathBuf {
    static NEXT_FILE: AtomicU64 = AtomicU64::new(0);
    let number = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("workbench-{}-{number}.{ending}", std::process::id()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("sh");
        command.arg("-c").arg(script);
        command
    }

    #[test]
    fn captures_output_and_exit_code() {
        let finished = run_with_time_limit(&mut shell("echo hello; exit 3"), Duration::from_secs(5)).unwrap();

        assert_eq!(finished.exit_code, Some(3));
        assert_eq!(finished.output.trim(), "hello");
        assert!(!finished.succeeded());
    }

    #[test]
    fn stops_a_command_that_runs_past_its_limit() {
        let started = Instant::now();

        let result = run_with_time_limit(&mut shell("echo started; sleep 30"), Duration::from_millis(300));

        assert!(started.elapsed() < Duration::from_secs(5));
        match result {
            Err(RunError::TimedOut { output, .. }) => assert_eq!(output.trim(), "started"),
            other => panic!("expected a timeout, got {other:?}"),
        }
    }

    fn is_running(process_id: &str) -> bool {
        Command::new("kill").args(["-0", process_id]).stderr(Stdio::null()).status().is_ok_and(|asked| asked.success())
    }

    fn is_gone_within(process_id: &str, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;
        while is_running(process_id) && Instant::now() < deadline {
            thread::sleep(POLL_INTERVAL);
        }
        !is_running(process_id)
    }

    #[test]
    fn a_command_stopped_at_its_limit_takes_what_it_started_with_it() {
        let says_what_it_started_then_waits = "sleep 30 & echo $!; wait";

        let result = run_with_time_limit(&mut shell(says_what_it_started_then_waits), Duration::from_millis(300));

        let Err(RunError::TimedOut { output, .. }) = result else { panic!("expected a timeout, got {result:?}") };
        assert!(is_gone_within(output.trim(), Duration::from_secs(5)), "the sleep it started (process {}) is still running", output.trim());
    }

    #[test]
    fn what_a_process_started_is_found_however_far_down_and_nothing_else_is() {
        let running = [(10, 1), (11, 10), (12, 11), (13, 10), (20, 1), (21, 20)].map(|(id, started_by)| Running { id, started_by });

        assert_eq!(started_by(10, &running), [11, 13, 12]);
        assert!(started_by(12, &running).is_empty());
    }

    #[test]
    fn a_line_of_the_list_that_is_not_two_numbers_is_left_out() {
        assert!(running_on("  412   1").is_some_and(|process| process.id == 412 && process.started_by == 1));
        assert!(running_on("PID PPID").is_none() && running_on("412").is_none());
    }

    #[test]
    fn on_windows_a_program_is_found_under_the_ending_it_was_installed_with() {
        let folders = ["empty", "scripts", "programs"].map(|name| std::env::temp_dir().join(format!("workbench-found-{}-{name}", std::process::id())));
        folders.iter().for_each(|folder| fs::create_dir_all(folder).unwrap());
        fs::write(folders[1].join("claude.cmd"), "").unwrap();
        fs::write(folders[2].join("claude.exe"), "").unwrap();
        fs::write(folders[2].join("claude.cmd"), "").unwrap();
        let found = |from: usize, program: &str| installed_among(folders[from..].iter().cloned(), OsStr::new(program));

        assert_eq!(found(0, "claude"), Some(folders[1].join("claude.cmd")));
        assert_eq!(found(2, "claude"), Some(folders[2].join("claude.exe")));
        assert_eq!(found(0, "codex"), None);
        assert_eq!(found(0, "claude.cmd"), None);
        assert_eq!(found(0, "scripts/claude"), None);
        folders.iter().for_each(|folder| fs::remove_dir_all(folder).unwrap());
    }

    #[test]
    fn the_shells_folders_are_read_from_between_the_marks_whatever_else_it_prints() {
        assert_eq!(folders_said_in("Welcome back!\n<<folders:/usr/bin:/opt/homebrew/bin>>\nbye"), Some("/usr/bin:/opt/homebrew/bin"));
        assert_eq!(folders_said_in("no marks here"), None);
        assert_eq!(folders_said_in("<<folders:>>"), None);
    }

    #[test]
    fn a_program_this_computer_has_is_found_by_name_and_one_it_lacks_is_not() {
        assert!(is_installed("sh"));
        assert!(!is_installed("no-such-workbench-command"));
    }

    #[test]
    fn the_folders_the_musicians_shell_looks_in_are_looked_in_too() {
        let of_the_shell = folders_the_shell_looks_in().expect("this computer's shell says where it looks");

        assert!(env::split_paths(searched_folders()).any(|folder| env::split_paths(&of_the_shell).any(|of_the_shell| of_the_shell == folder)));
    }

    #[test]
    fn reports_a_command_that_does_not_exist() {
        let result = run_with_time_limit(&mut Command::new("no-such-workbench-command"), Duration::from_secs(1));

        assert!(matches!(result, Err(RunError::CouldNotStart(_))));
    }
}
