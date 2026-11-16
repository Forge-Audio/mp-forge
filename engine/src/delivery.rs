//! Sends a plugin to the musician's MPC over the network, through Akai's own remote tool.
//!
//! That tool only writes inside the `VST2 Plugins` folder of a drive on the MPC. The workbench asks it for three
//! things and no others: which drives the MPC has, put this plugin on one, and restart the MPC's app so the
//! plugin shows up. Whether a plugin is safe to send is decided by the gate, before this module is ever reached.
//!
//! A plugin only ever goes on a drive that can be pulled out: a USB drive or an SD card. Whatever a plugin does
//! to an MPC, pulling that drive out takes the plugin away again.

use std::ffi::OsString;
use std::fmt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use serde_json::Value;

use crate::process::{Finished, RunError, run_with_time_limit};
use crate::sdk::Sdk;

const TOOLS_FOLDER: &str = "tools";
const REMOTE_TOOL: &str = "tools/mpc_remote.py";
/// The workbench's own script that has Akai's tool list the MPC's drives in a form that can be read.
const DRIVES_SCRIPT: &str = include_str!("mpc_drives.py");
const DRIVES_ARE_LISTED_AFTER: &str = "@@DRIVES ";
const LOOKING_TIME_LIMIT: Duration = Duration::from_secs(60);
const ONLY_REMOVABLE: &str = "MP Forge only sends to a USB drive or SD card, because pulling that out always takes the plugin away again.";
/// Akai's tool reads the password from here, which keeps it out of the list of running programs.
const PASSWORD_VARIABLE: &str = "MPC_SSH_PASSWORD";
const SENDING_TIME_LIMIT: Duration = Duration::from_secs(5 * 60);
const RESTART_TIME_LIMIT: Duration = Duration::from_secs(2 * 60);
const LONGEST_ADDRESS: usize = 253;
const LONGEST_DRIVE_NAME: usize = 64;
const LINES_OF_THE_TOOLS_WORDS_SHOWN: usize = 12;

/// Where the musician's MPC is, and what it asks for before it lets anything in.
#[derive(Clone, PartialEq, Eq)]
pub struct Mpc {
    address: String,
    drive: Option<String>,
    password: Option<String>,
}

impl Mpc {
    /// An address is a name or a number on the network and nothing else, so it can never be read as an option.
    pub fn at(address: &str) -> Result<Self, DeliveryError> {
        let address = address.trim();
        if is_plain_address(address) { Ok(Self { address: address.to_string(), drive: None, password: None }) } else { Err(DeliveryError::NotAnAddress(address.to_string())) }
    }

    /// Which of the MPC's removable drives to use, by its number or name. Only needed when the MPC has several.
    pub fn using_drive(mut self, drive: Option<&str>) -> Result<Self, DeliveryError> {
        let drive = drive.map(str::trim).filter(|named| !named.is_empty());
        if let Some(unusable) = drive.filter(|named| !is_plain_drive_name(named)) {
            return Err(DeliveryError::NotADrive(unusable.to_string()));
        }
        self.drive = drive.map(str::to_string);
        Ok(self)
    }

    pub fn with_password(mut self, password: Option<String>) -> Self {
        self.password = password.filter(|given| !given.is_empty());
        self
    }

    pub fn address(&self) -> &str {
        &self.address
    }
}

fn is_plain_address(address: &str) -> bool {
    let plain = |character: char| character.is_ascii_alphanumeric() || character == '.' || character == '-';
    let starts_plainly = address.chars().next().is_some_and(|first| first.is_ascii_alphanumeric());
    starts_plainly && address.len() <= LONGEST_ADDRESS && address.chars().all(plain)
}

fn is_plain_drive_name(drive: &str) -> bool {
    !drive.starts_with('-') && drive.len() <= LONGEST_DRIVE_NAME && !drive.chars().any(char::is_control)
}

#[derive(Debug)]
pub enum DeliveryError {
    NotAnAddress(String),
    NotADrive(String),
    ToolDidNotRun(String),
    /// Akai's tool ran and said no. These are its own closing words.
    Refused(String),
    NoRemovableDrive,
    /// The MPC has more than one drive that can be pulled out, and the musician has not said which to use.
    SeveralRemovableDrives(Vec<String>),
    /// The drive the musician named is built into the MPC.
    BuiltIn(String),
    NoSuchDrive(String),
}

impl fmt::Display for DeliveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeliveryError::NotAnAddress(address) => write!(formatter, "`{address}` is not an address an MPC can have. It looks like 192.168.1.50, and the MPC shows its own under its Wi-Fi settings."),
            DeliveryError::NotADrive(drive) => write!(formatter, "`{drive}` cannot be the name or number of a drive."),
            DeliveryError::ToolDidNotRun(cause) => write!(formatter, "Akai's tool for reaching the MPC could not be run: {cause}"),
            DeliveryError::Refused(words) => write!(formatter, "The MPC was not reached, or it said no. Akai's tool reported:\n\n{words}"),
            DeliveryError::NoRemovableDrive => write!(formatter, "The MPC has no USB drive or SD card in it that can be written to. {ONLY_REMOVABLE} Put one in and send again."),
            DeliveryError::SeveralRemovableDrives(names) => write!(formatter, "The MPC has several drives that can be pulled out: {}. Say which one to use under Your MPC in Settings.", names.join(", ")),
            DeliveryError::BuiltIn(drive) => write!(formatter, "`{drive}` is built into the MPC. {ONLY_REMOVABLE} Name a removable drive in Settings, or leave the drive empty."),
            DeliveryError::NoSuchDrive(drive) => write!(formatter, "The MPC has no drive called `{drive}`. Check the drive under Your MPC in Settings."),
        }
    }
}

/// One drive on the MPC, as Akai's tool describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Drive {
    name: String,
    /// Where the drive sits on the MPC, which is how Akai's tool is told to use this drive and no other.
    mount: String,
    is_built_in: bool,
    can_be_written_to: bool,
}

impl Drive {
    fn read_from(listed: &Value) -> Option<Self> {
        Some(Self {
            name: listed["name"].as_str()?.to_string(),
            mount: listed["mount"].as_str()?.to_string(),
            is_built_in: listed["internal"].as_bool()?,
            can_be_written_to: listed["writable"].as_bool()?,
        })
    }

    fn is_called(&self, wanted: &str) -> bool {
        self.name.eq_ignore_ascii_case(wanted) || self.mount.eq_ignore_ascii_case(wanted)
    }
}

/// What the workbench puts on an MPC and what it does afterwards. Looking at the drives is the only other thing.
enum Errand<'a> {
    /// Puts the built plugin on one drive of the MPC, in place of a copy already there.
    Install { plugin_dir: &'a Path, onto: &'a Drive },
    /// Restarts the MPC's app so it finds the plugin. Anything unsaved on the MPC is lost.
    RestartApp,
}

/// Puts a built plugin on a removable drive of the MPC, and says which drive. The caller has already had the
/// gate pass the plugin.
pub fn install(sdk: &Sdk, plugin_dir: &Path, mpc: &Mpc) -> Result<String, DeliveryError> {
    let drives = drives_of(sdk, mpc)?;
    let onto = removable_drive(&drives, mpc.drive.as_deref())?;
    run(sdk, mpc, &Errand::Install { plugin_dir, onto }, SENDING_TIME_LIMIT)?;
    Ok(onto.name.clone())
}

/// The drive to send to: the one the musician named, or the MPC's only removable drive.
/// A drive built into the MPC is never chosen, named or not.
fn removable_drive<'a>(drives: &'a [Drive], wanted: Option<&str>) -> Result<&'a Drive, DeliveryError> {
    match wanted {
        Some(wanted) => named_drive(drives, wanted).and_then(|drive| if drive.is_built_in { Err(DeliveryError::BuiltIn(drive.name.clone())) } else { Ok(drive) }),
        None => only_removable_drive(drives),
    }
}

/// A drive is named by its name, by where it sits, or by its number in the order Akai's tool lists them.
fn named_drive<'a>(drives: &'a [Drive], wanted: &str) -> Result<&'a Drive, DeliveryError> {
    let by_number = wanted.parse::<usize>().ok().and_then(|number| drives.get(number.wrapping_sub(1)));
    by_number.or_else(|| drives.iter().find(|drive| drive.is_called(wanted))).ok_or_else(|| DeliveryError::NoSuchDrive(wanted.to_string()))
}

fn only_removable_drive(drives: &[Drive]) -> Result<&Drive, DeliveryError> {
    let usable: Vec<&Drive> = drives.iter().filter(|drive| !drive.is_built_in && drive.can_be_written_to).collect();
    match usable.as_slice() {
        [] => Err(DeliveryError::NoRemovableDrive),
        [only] => Ok(only),
        several => Err(DeliveryError::SeveralRemovableDrives(several.iter().map(|drive| drive.name.clone()).collect())),
    }
}

/// Has Akai's tool reach the MPC and say which drives it has. Nothing on the MPC is changed.
fn drives_of(sdk: &Sdk, mpc: &Mpc) -> Result<Vec<Drive>, DeliveryError> {
    let mut command = sdk.python();
    command.arg("-c").arg(DRIVES_SCRIPT).arg(sdk.root().join(TOOLS_FOLDER)).arg(&mpc.address);
    let said = finished(&mut command, mpc, LOOKING_TIME_LIMIT)?;
    drives_listed_in(&said).ok_or_else(|| DeliveryError::ToolDidNotRun("its list of the MPC's drives could not be read.".to_string()))
}

fn drives_listed_in(said: &str) -> Option<Vec<Drive>> {
    let listed = said.lines().rev().find_map(|line| line.strip_prefix(DRIVES_ARE_LISTED_AFTER))?;
    serde_json::from_str::<Vec<Value>>(listed).ok()?.iter().map(Drive::read_from).collect()
}

/// Restarts the MPC's app. Only ever done when the musician asked for it, for this one time.
pub fn restart_app(sdk: &Sdk, mpc: &Mpc) -> Result<(), DeliveryError> {
    run(sdk, mpc, &Errand::RestartApp, RESTART_TIME_LIMIT)
}

fn run(sdk: &Sdk, mpc: &Mpc, errand: &Errand, limit: Duration) -> Result<(), DeliveryError> {
    let mut command = sdk.python();
    command.arg(sdk.root().join(REMOTE_TOOL)).args(arguments_for(mpc, errand));
    finished(&mut command, mpc, limit).map(|_said| ())
}

/// Runs Akai's tool to its end and returns what it said. The MPC's password reaches it out of sight.
fn finished(command: &mut Command, mpc: &Mpc, limit: Duration) -> Result<String, DeliveryError> {
    if let Some(password) = &mpc.password {
        command.env(PASSWORD_VARIABLE, password);
    }
    match run_with_time_limit(command, limit) {
        Ok(finished) if finished.succeeded() => Ok(finished.output),
        Ok(Finished { output, .. }) => Err(DeliveryError::Refused(closing_words_of(&output))),
        Err(RunError::CouldNotStart(cause)) => Err(DeliveryError::ToolDidNotRun(cause.to_string())),
        Err(RunError::TimedOut { limit, .. }) => Err(DeliveryError::Refused(format!("It had not finished after {} seconds.", limit.as_secs()))),
    }
}

/// Akai's tool is told never to stop and ask, because nobody is there to answer it. A plugin is sent to the one
/// drive already chosen, named by where it sits, so the tool can never settle on another.
fn arguments_for(mpc: &Mpc, errand: &Errand) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["--address", &mpc.address, "--yes", "--no-color"].map(OsString::from).to_vec();
    match errand {
        Errand::Install { plugin_dir, onto } => {
            arguments.extend(["--drive", &onto.mount, "update"].map(OsString::from));
            arguments.push(plugin_dir.as_os_str().to_os_string());
        }
        Errand::RestartApp => arguments.push(OsString::from("restart")),
    }
    arguments
}

/// The tool draws boxes and progress as it goes. Its last few lines of words, without the boxes round them,
/// say what went wrong.
fn closing_words_of(output: &str) -> String {
    let is_box_edge = |character: char| character.is_whitespace() || ('\u{2500}'..='\u{257f}').contains(&character);
    let worded: Vec<&str> = output.lines().map(|line| line.trim_matches(is_box_edge)).filter(|line| line.chars().any(char::is_alphanumeric)).collect();
    let from = worded.len().saturating_sub(LINES_OF_THE_TOOLS_WORDS_SHOWN);
    worded[from..].join("\n")
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    /// A stand-in for Akai's remote tool. Asked for the drives, it lists the ones given here. Run as a tool, it
    /// writes down what it was asked and the password it was handed. `unreachable.local` is never reached.
    fn stand_in_tool(drives: &str) -> String {
        format!(
            r#"import os, sys
class SshError(Exception): pass
class Drive:
    def __init__(self, name, internal, writable=True):
        self.name, self.mount, self.internal, self.writable = name, "/media/" + name, internal, writable
class Remote:
    def __init__(self, host, user, port, password): self.host, self.password = host, password
    def close(self): pass
def connect(remote):
    if remote.host == "unreachable.local": raise SshError("no route")
def explain(error, host, port, has_password):
    print("  ----------")
    print("\u256d\u2500 Could not reach " + host + " \u2500\u2500\u256e")
    print("\u2502 Is developer mode on?    \u2502")
def discover_drives(remote): return [{drives}]
if __name__ == "__main__":
    asked = " ".join(sys.argv[1:]) + "|" + os.environ.get("MPC_SSH_PASSWORD", "")
    open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "asked.txt"), "w").write(asked)
"#
        )
    }

    const ONE_CARD: &str = r#"Drive("az01-internal-sd", True), Drive("SD CARD", False)"#;

    fn sdk_whose_mpc_has(drives: &str) -> (TemporaryFolder, Sdk) {
        let folder = TemporaryFolder::create("delivery");
        fs::create_dir_all(folder.0.join("tools")).unwrap();
        fs::write(folder.0.join("tools/build.py"), "").unwrap();
        fs::write(folder.0.join(REMOTE_TOOL), stand_in_tool(drives)).unwrap();
        let sdk = Sdk::at(&folder.0).unwrap();
        (folder, sdk)
    }

    fn what_was_asked(sdk_folder: &TemporaryFolder) -> Option<String> {
        fs::read_to_string(sdk_folder.0.join("tools/asked.txt")).ok()
    }

    fn mpc() -> Mpc {
        Mpc::at("192.168.1.50").unwrap()
    }

    fn sending_to(drives: &str, mpc: &Mpc) -> (Result<String, DeliveryError>, Option<String>) {
        let (folder, sdk) = sdk_whose_mpc_has(drives);
        let outcome = install(&sdk, Path::new("/plugins/Dusty Tape"), mpc);
        (outcome, what_was_asked(&folder))
    }

    #[test]
    fn a_plugin_goes_on_the_mpcs_one_removable_drive_in_place_of_the_copy_already_there() {
        let (outcome, asked) = sending_to(ONE_CARD, &Mpc::at(" 192.168.1.50 ").unwrap());

        assert_eq!(outcome.unwrap(), "SD CARD");
        assert_eq!(asked.unwrap(), "--address 192.168.1.50 --yes --no-color --drive /media/SD CARD update /plugins/Dusty Tape|");
    }

    #[test]
    fn nothing_is_sent_to_an_mpc_with_only_its_built_in_storage() {
        let (outcome, asked) = sending_to(r#"Drive("az01-internal-sd", True), Drive("nvme", True)"#, &mpc());

        assert!(matches!(outcome, Err(DeliveryError::NoRemovableDrive)));
        assert_eq!(asked, None);
    }

    #[test]
    fn a_removable_drive_that_cannot_be_written_to_is_not_chosen() {
        let (outcome, asked) = sending_to(r#"Drive("LOCKED CARD", False, writable=False)"#, &mpc());

        assert!(matches!(outcome, Err(DeliveryError::NoRemovableDrive)));
        assert_eq!(asked, None);
    }

    #[test]
    fn with_several_removable_drives_the_musician_has_to_say_which() {
        let two_drives = r#"Drive("az01-internal-sd", True), Drive("SD CARD", False), Drive("USB STICK", False)"#;

        let (unnamed, asked_unnamed) = sending_to(two_drives, &mpc());
        let (named, asked_named) = sending_to(two_drives, &mpc().using_drive(Some("usb stick")).unwrap());
        let (numbered, _) = sending_to(two_drives, &mpc().using_drive(Some("2")).unwrap());

        assert!(matches!(unnamed, Err(DeliveryError::SeveralRemovableDrives(names)) if names == ["SD CARD", "USB STICK"]));
        assert_eq!(asked_unnamed, None);
        assert_eq!(named.unwrap(), "USB STICK");
        assert!(asked_named.unwrap().contains("--drive /media/USB STICK update"));
        assert_eq!(numbered.unwrap(), "SD CARD");
    }

    #[test]
    fn a_built_in_drive_is_refused_even_when_the_musician_names_it() {
        let (by_name, asked) = sending_to(ONE_CARD, &mpc().using_drive(Some("az01-internal-sd")).unwrap());
        let (by_number, _) = sending_to(ONE_CARD, &mpc().using_drive(Some("1")).unwrap());

        assert!(matches!(by_name, Err(DeliveryError::BuiltIn(drive)) if drive == "az01-internal-sd"));
        assert!(matches!(by_number, Err(DeliveryError::BuiltIn(_))));
        assert_eq!(asked, None);
    }

    #[test]
    fn a_drive_the_mpc_does_not_have_is_named_in_the_refusal() {
        let (outcome, asked) = sending_to(ONE_CARD, &mpc().using_drive(Some("FLOPPY")).unwrap());

        assert!(matches!(outcome, Err(DeliveryError::NoSuchDrive(drive)) if drive == "FLOPPY"));
        assert_eq!(asked, None);
    }

    #[test]
    fn the_password_is_handed_over_out_of_sight_and_a_restart_names_no_drive() {
        let (folder, sdk) = sdk_whose_mpc_has(ONE_CARD);
        let mpc = Mpc::at("mpc.local").unwrap().with_password(Some("opensesame".to_string()));

        restart_app(&sdk, &mpc).unwrap();

        assert_eq!(what_was_asked(&folder).unwrap(), "--address mpc.local --yes --no-color restart|opensesame");
    }

    #[test]
    fn what_akais_tool_said_is_passed_on_when_the_mpc_cannot_be_reached() {
        let (outcome, asked) = sending_to(ONE_CARD, &Mpc::at("unreachable.local").unwrap());

        assert!(matches!(&outcome, Err(DeliveryError::Refused(words)) if words == "Could not reach unreachable.local\nIs developer mode on?"), "{outcome:?}");
        assert_eq!(asked, None);
    }

    #[test]
    fn an_address_that_could_be_read_as_an_option_or_a_command_is_refused() {
        for unusable in ["", "-oProxyCommand=sh", "192.168.1.50; rm -rf /", "a b", "$(reboot)"] {
            assert!(matches!(Mpc::at(unusable), Err(DeliveryError::NotAnAddress(_))), "{unusable}");
        }
        assert!(matches!(mpc().using_drive(Some("--password")), Err(DeliveryError::NotADrive(_))));
    }

    #[test]
    fn the_only_errands_are_installing_and_restarting() {
        let card = Drive { name: "SD CARD".to_string(), mount: "/media/SD CARD".to_string(), is_built_in: false, can_be_written_to: true };
        let install = arguments_for(&mpc(), &Errand::Install { plugin_dir: Path::new("p"), onto: &card });
        let restart = arguments_for(&mpc(), &Errand::RestartApp);

        assert_eq!(install[4..], ["--drive", "/media/SD CARD", "update", "p"]);
        assert_eq!(restart[4..], ["restart"]);
    }
}
