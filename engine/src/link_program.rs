//! MP Forge Link: the program for the MPC Sample's end of the link, built from MP Forge's own code.
//!
//! It is built with the compiler that builds Sample apps, for the device's processor and needing nothing of
//! the device's system but its files and its network. Built for this computer instead, it stands in for the
//! device, so both ends of the link can be tried with no device at hand.
//!
//! For the device it is put in a folder for the boot menu SampleBoot, which the musician copies onto their
//! card themselves, into `autorun/mods`. The files at the top of `autorun` that start the boot menu are
//! SampleBoot's own: none of them is ever written here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;

use crate::device_link::PORT_OF_THE_PROGRAM;
use crate::sample_app::{NotBuilt, compile_with, write_kit_in};
use crate::sdk::Sdk;

/// What the built program is called, on the device and here.
pub const PROGRAM_NAME: &str = "mp-forge-link";
/// The folder that goes onto the card, named as the boot menu shows it.
pub const FOLDER_FOR_THE_CARD: &str = "MP Forge Link";

const START_SCRIPT_FILE: &str = "start-mp-forge-link.sh";
const START_SCRIPT: &str = include_str!("../helpers/sample/start-mp-forge-link.sh");
const PORT_IN_THE_START_SCRIPT: &str = "@PORT@";
/// The file the boot menu reads to learn which script starts what is in a folder.
const NAMES_THE_START_SCRIPT_FILE: &str = "autorun.json";
/// The file the boot menu reads the name it shows from.
const NAME_IN_THE_MENU_FILE: &str = "sampleboot.txt";

const CODE_FILE: &str = "mp_forge_link.cpp";
const CODE: &str = include_str!("../helpers/sample/mp_forge_link.cpp");
const COMPILER_FLAGS: [&str; 4] = ["-std=c++17", "-O2", "-Wall", "-Wextra"];
/// The MPC Sample's processor, with everything the program needs built into it.
const FOR_THE_MPC_SAMPLE: [&str; 4] = ["-target", "aarch64-linux-musl", "-static", "-s"];

/// Builds the program for the MPC Sample, in `build_dir`, and returns it.
pub fn build_for_the_mpc_sample(sdk: &Sdk, build_dir: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    build(sdk, build_dir, &FOR_THE_MPC_SAMPLE, limit)
}

/// Builds the program for this computer, which has to be one with files and sockets like the device's: a Mac
/// or a Linux computer.
pub fn build_for_this_computer(sdk: &Sdk, build_dir: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    build(sdk, build_dir, &[], limit)
}

/// Writes the folder for the boot menu inside `into`, with the program built for the MPC Sample in it, and
/// returns the folder.
pub fn write_folder_for_the_boot_menu(sdk: &Sdk, into: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    let folder = into.join(FOLDER_FOR_THE_CARD);
    let built_in = std::env::temp_dir().join(format!("mp-forge-link-build-{}", std::process::id()));
    let written = build_for_the_mpc_sample(sdk, &built_in, limit).and_then(|built| Ok(write_folder(&folder, &built)?));
    let _ = fs::remove_dir_all(&built_in);
    written.map(|()| folder)
}

fn write_folder(folder: &Path, built: &Path) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    fs::copy(built, folder.join(PROGRAM_NAME))?;
    fs::write(folder.join(START_SCRIPT_FILE), start_script())?;
    fs::write(folder.join(NAMES_THE_START_SCRIPT_FILE), json!({ "scripts": [{ "path": START_SCRIPT_FILE }] }).to_string())?;
    fs::write(folder.join(NAME_IN_THE_MENU_FILE), FOLDER_FOR_THE_CARD)
}

/// The script as the device's shell reads it: with the port the computer reaches the program at, and with
/// lines that end the way that shell expects, whichever computer MP Forge itself was built on.
fn start_script() -> String {
    START_SCRIPT.replace("\r\n", "\n").replace(PORT_IN_THE_START_SCRIPT, &PORT_OF_THE_PROGRAM.to_string())
}

/// The program draws its own screen with the kit, so the kit is written beside its code.
fn build(sdk: &Sdk, build_dir: &Path, built_for: &[&str], limit: Duration) -> Result<PathBuf, NotBuilt> {
    write_kit_in(build_dir)?;
    fs::write(build_dir.join(CODE_FILE), CODE)?;
    let program = build_dir.join(PROGRAM_NAME);
    let mut compiler = sdk.compiler().ok_or(NotBuilt::NoCompiler)?;
    compiler.arg("c++").args(COMPILER_FLAGS).args(built_for).arg("-o").arg(&program).arg(build_dir.join(CODE_FILE));
    compile_with(&mut compiler, limit).map(|()| program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_script_tells_the_program_the_port_the_computer_reaches_it_at() {
        let script = start_script();

        assert!(script.contains(&format!("\"/dev/snd/midiC${{CARD}}D0\" {PORT_OF_THE_PROGRAM} ")), "{script}");
        assert!(!script.contains(PORT_IN_THE_START_SCRIPT) && !script.contains('\r'));
    }

    #[test]
    fn the_start_script_hands_the_device_back_to_akais_app_and_keeps_off_its_own_storage() {
        let script = start_script();

        assert!(script.contains("trap 'systemctl start ac50' EXIT"));
        assert!(script.lines().filter(|line| line.contains('>')).all(|line| line.contains("\"$SAID\"")), "it writes only its own note of what went wrong");
    }

    #[cfg(unix)]
    #[test]
    fn the_start_script_is_one_a_shell_can_read() {
        let file = std::env::temp_dir().join(format!("mp-forge-link-start-{}.sh", std::process::id()));
        fs::write(&file, start_script()).unwrap();

        let read = std::process::Command::new("sh").arg("-n").arg(&file).output().unwrap();
        let _ = fs::remove_file(&file);

        assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    }
}
