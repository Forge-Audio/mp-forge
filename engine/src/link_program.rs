//! MP Forge Link: the program for the MPC Sample's end of the link, built from MP Forge's own code.
//!
//! It is built with the compiler that builds Sample apps, for the device's processor and needing nothing of
//! the device's system but its files and its network. Built for this computer instead, it stands in for the
//! device, so both ends of the link can be tried with no device at hand.
//!
//! For the device it is put in a folder for the boot menu SampleBoot, which belongs on the card in
//! `autorun/mods`. The musician picks the card, and the folder goes there when the boot menu is on it, or
//! else into whatever folder they picked, for them to copy. The files at the top of `autorun` that start the
//! boot menu are SampleBoot's own: none of them is ever written here.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use crate::boot_menu::{MadeForTheCard, WHERE_THE_BOOT_MENU_LOOKS};
use crate::boot_menu::{PlaceForAFolder, ShownInTheMenu};
use crate::app_sending::PORT_FOR_APPS;
use crate::device_link::PORT_OF_THE_PROGRAM;
use crate::sample_app::{NotBuilt, compile_with, write_kit_in};
use crate::sample_surface;
use crate::sdk::Sdk;

/// What the built program is called, on the device and here.
pub const PROGRAM_NAME: &str = "mp-forge-link";
/// The folder that goes onto the card, named as the boot menu shows it.
pub const FOLDER_FOR_THE_CARD: &str = "MP Forge Link";
/// Building the program takes well under a minute. A build that takes this long has hung.
pub const TIME_ALLOWED_TO_BUILD: Duration = Duration::from_secs(300);

const START_SCRIPT_FILE: &str = "start-mp-forge-link.sh";
const START_SCRIPT: &str = include_str!("../helpers/sample/start-mp-forge-link.sh");
const PORT_IN_THE_START_SCRIPT: &str = "@PORT@";
const PORT_FOR_APPS_IN_THE_START_SCRIPT: &str = "@PORT_FOR_APPS@";

const CODE_FILE: &str = "mp_forge_link.cpp";
/// The program as the code it is built from: the carrying, the taking of apps, and the opening of the controls.
const CODE: [(&str, &str); 3] = [
    (CODE_FILE, include_str!("../helpers/sample/mp_forge_link.cpp")),
    ("mp_forge_link_apps.h", include_str!("../helpers/sample/mp_forge_link_apps.h")),
    sample_surface::CODE_THAT_OPENS_THE_CONTROLS,
];
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

/// Writes the link's folder for a card the musician picked. On a card that has the boot menu, it goes where
/// the boot menu looks, whether they picked the card itself or that place on it. In any other folder it goes
/// into the folder as it is, to be copied onto a card later.
pub fn write_for_the_card_picked(sdk: &Sdk, picked: &Path, limit: Duration) -> Result<MadeForTheCard, NotBuilt> {
    let place = PlaceForAFolder::in_what_was_picked(picked);
    let folder = write_folder_for_the_boot_menu(sdk, &place.into, limit)?;
    Ok(place.holding(folder))
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
    ShownInTheMenu { name: FOLDER_FOR_THE_CARD, start_script_file: START_SCRIPT_FILE, start_script: &start_script() }.write_into(folder)
}

/// The script with the ports the computer reaches the program at: one to link, and one to hand over an app.
fn start_script() -> String {
    let with_the_port_for_apps = START_SCRIPT.replace(PORT_FOR_APPS_IN_THE_START_SCRIPT, &PORT_FOR_APPS.to_string());
    with_the_port_for_apps.replace("\r\n", "\n").replace(PORT_IN_THE_START_SCRIPT, &PORT_OF_THE_PROGRAM.to_string())
}

/// The program draws its own screen with the kit, so the kit is written beside its code.
fn build(sdk: &Sdk, build_dir: &Path, built_for: &[&str], limit: Duration) -> Result<PathBuf, NotBuilt> {
    write_kit_in(build_dir)?;
    for (file, code) in CODE {
        fs::write(build_dir.join(file), code)?;
    }
    let program = build_dir.join(PROGRAM_NAME);
    let mut compiler = sdk.compiler().ok_or(NotBuilt::NoCompiler)?;
    compiler.arg("c++").args(COMPILER_FLAGS).args(built_for).arg("-o").arg(&program).arg(build_dir.join(CODE_FILE));
    compile_with(&mut compiler, limit).map(|()| program)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of this test's own that is gone again when the test is over.
    struct Scratch(PathBuf);

    impl Scratch {
        fn named(name: &str) -> Self {
            let folder = std::env::temp_dir().join(format!("mp-forge-link-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&folder);
            fs::create_dir_all(&folder).unwrap();
            Self(folder)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_folder_holds_what_the_boot_menu_needs_to_show_and_start_the_link() {
        let scratch = Scratch::named("folder");
        let built = scratch.0.join("built-program");
        fs::write(&built, b"the program").unwrap();
        let folder = scratch.0.join(FOLDER_FOR_THE_CARD);

        write_folder(&folder, &built).unwrap();

        assert_eq!(fs::read(folder.join(PROGRAM_NAME)).unwrap(), b"the program");
        assert_eq!(fs::read_to_string(folder.join("sampleboot.txt")).unwrap(), "MP Forge Link");
        assert_eq!(fs::read_to_string(folder.join("autorun.json")).unwrap(), r#"{"scripts":[{"path":"start-mp-forge-link.sh"}]}"#);
        assert_eq!(fs::read_to_string(folder.join(START_SCRIPT_FILE)).unwrap(), start_script());
    }

    #[test]
    fn the_start_script_tells_the_program_the_port_the_computer_reaches_it_at() {
        let script = start_script();

        assert!(script.contains(&format!("\"/dev/snd/midiC${{CARD}}D0\" {PORT_OF_THE_PROGRAM} ")), "{script}");
        assert!(!script.contains('@') && !script.contains('\r'));
    }

    #[test]
    fn the_start_script_has_apps_go_where_the_boot_menu_looks_and_never_onto_the_devices_own_storage() {
        let script = start_script();

        assert!(script.contains(&format!(" {PORT_FOR_APPS} \"$(dirname -- \"$HERE\")\" / 2>\"$SAID\"")), "{script}");
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
