//! A Sample app made ready for the MPC Sample itself, where it runs with no computer.
//!
//! On a computer an app is a piece that MP Forge's player loads. The device has nothing to load it with, so
//! here the app's own code is built together with MP Forge's program that runs it there: one program that
//! needs nothing of the device's system but its files. The app's code is the same in both.
//!
//! That program goes into a folder for the boot menu SampleBoot, with the script that starts it and the
//! sound files the musician added to the app. The folder is the same whether it is written to a card in this
//! computer or sent to the device over MP Forge Link.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::app_sounds::MOST_SOUNDS_AN_APP_IS_GIVEN;
use crate::boot_menu::{HIDES_A_FOLDER_FROM_THE_MENU, MadeForTheCard, PlaceForAFolder, ShownInTheMenu};
use crate::link_program;
use crate::sample_app::{self, NotBuilt, compile_with};
use crate::sample_surface;
use crate::samples::SampleShelf;
use crate::sdk::Sdk;

/// What the built program is called in the app's folder.
pub const PROGRAM_NAME: &str = "app";
/// The script that starts the app, which is also how one of MP Forge's apps is told from any other folder.
pub const START_SCRIPT_FILE: &str = "start-app.sh";
/// What the start script is told when MP Forge Link starts it, so that it leaves the device to the link.
pub const STARTED_BY_THE_LINK: &str = "from-mp-forge-link";
/// Where in the app's folder the sound files the musician added to it are.
pub const SOUNDS_FOLDER: &str = "sounds";
/// What follows an app's name in the name of the folder it is written into before it takes its place.
pub const ENDS_AN_ARRIVING_FOLDER: &str = ".arriving";
/// What follows an app's name in the name its older folder has while the new one takes its place.
pub const ENDS_THE_FOLDER_BEFORE: &str = ".before";
/// Building an app with its program takes well under a minute. A build that takes this long has hung.
pub const TIME_ALLOWED_TO_BUILD: Duration = Duration::from_secs(300);

const START_SCRIPT: &str = include_str!("../helpers/sample/start-app.sh");
const HOST_CODE_FILE: &str = "mp_forge_sample_host.cpp";
/// MP Forge's program that runs an app on the device, as the code it is built from.
const HOST_CODE: [(&str, &str); 4] = [
    (HOST_CODE_FILE, include_str!("../helpers/sample/mp_forge_sample_host.cpp")),
    ("mp_forge_host_surface.h", include_str!("../helpers/sample/mp_forge_host_surface.h")),
    ("mp_forge_host_files.h", include_str!("../helpers/sample/mp_forge_host_files.h")),
    ("mp_forge_host_sound.h", include_str!("../helpers/sample/mp_forge_host_sound.h")),
];
const DEVICE_MAP_FILE: &str = "mp_forge_device_map.h";
const HOST_FOLDER: &str = "host";
const COMPILER_FLAGS: [&str; 5] = ["-std=c++17", "-O2", "-Wall", "-Wextra", "-pthread"];

/// What an app is built for, and where in the app's folder that build is kept.
struct BuiltFor {
    build_folder: &'static str,
    compiler_flags: &'static [&'static str],
}

/// The MPC Sample's processor, with everything the program needs built into it.
const THE_MPC_SAMPLE: BuiltFor = BuiltFor { build_folder: "build/device", compiler_flags: &["-target", "aarch64-linux-musl", "-static", "-s"] };
const THIS_COMPUTER: BuiltFor = BuiltFor { build_folder: "build/device-stand-in", compiler_flags: &[] };

/// Why an app was not made ready for the device.
#[derive(Debug)]
pub enum NotMade {
    NotBuilt(NotBuilt),
    /// A folder of the app's name is where the app would go, and MP Forge did not make it.
    NameIsTaken(PathBuf),
    /// The app is called what MP Forge's own program is called on the card.
    NameIsTheLinks,
    Io(io::Error),
}

impl fmt::Display for NotMade {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NotMade::NotBuilt(not_built) => write!(formatter, "{not_built}"),
            NotMade::NameIsTaken(folder) => write!(formatter, "There is already a folder at {} that MP Forge did not make. Rename the app, or move that folder away.", folder.display()),
            NotMade::NameIsTheLinks => write!(formatter, "On an MPC Sample's card, {} is the name of MP Forge's own program. Rename the app.", link_program::FOLDER_FOR_THE_CARD),
            NotMade::Io(cause) => write!(formatter, "The app's folder could not be written: {cause}"),
        }
    }
}

impl From<NotBuilt> for NotMade {
    fn from(not_built: NotBuilt) -> Self {
        NotMade::NotBuilt(not_built)
    }
}

impl From<io::Error> for NotMade {
    fn from(cause: io::Error) -> Self {
        NotMade::Io(cause)
    }
}

// ---------------------------------------------------------------------------
// Building
// ---------------------------------------------------------------------------

/// Builds the app, with the program that runs it, for the MPC Sample, and returns the one program built.
pub fn build_for_the_mpc_sample(sdk: &Sdk, app_dir: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    build(sdk, app_dir, &THE_MPC_SAMPLE, limit)
}

/// Builds the same for this computer, where it stands in for the device with files in the place of the
/// device's screen, controls and sound. The computer has to be a Mac or a Linux computer.
pub fn build_for_this_computer(sdk: &Sdk, app_dir: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    build(sdk, app_dir, &THIS_COMPUTER, limit)
}

fn build(sdk: &Sdk, app_dir: &Path, built_for: &BuiltFor, limit: Duration) -> Result<PathBuf, NotBuilt> {
    sample_app::put_kit_into(app_dir)?;
    let code = sample_app::code_files_of(app_dir)?;
    if code.is_empty() {
        return Err(NotBuilt::NoCode);
    }
    let build_dir = app_dir.join(built_for.build_folder);
    let host_code = write_host_code_in(&build_dir.join(HOST_FOLDER))?;
    let program = build_dir.join(PROGRAM_NAME);
    let mut compiler = sdk.compiler().ok_or(NotBuilt::NoCompiler)?;
    compiler.arg("c++").args(COMPILER_FLAGS).args(built_for.compiler_flags).arg("-o").arg(&program).arg(host_code).args(&code);
    compile_with(&mut compiler, limit).map(|()| program)
}

/// Writes the code of MP Forge's program afresh, with the kit and with which number is which control, and
/// returns the file the compiler is handed.
fn write_host_code_in(folder: &Path) -> io::Result<PathBuf> {
    sample_app::write_kit_in(folder)?;
    fs::write(folder.join(DEVICE_MAP_FILE), sample_surface::as_code_for_the_device())?;
    for (file, code) in HOST_CODE {
        fs::write(folder.join(file), code)?;
    }
    Ok(folder.join(HOST_CODE_FILE))
}

// ---------------------------------------------------------------------------
// The folder for the boot menu
// ---------------------------------------------------------------------------

/// Writes the app's folder for a card the musician picked. On a card that has the boot menu it goes where
/// the boot menu looks. In any other folder it goes into the folder as it is, to be copied onto a card later.
pub fn write_for_the_card_picked(sdk: &Sdk, app_dir: &Path, picked: &Path, limit: Duration) -> Result<MadeForTheCard, NotMade> {
    let place = PlaceForAFolder::in_what_was_picked(picked);
    let folder = write_folder_for_the_boot_menu(sdk, app_dir, &place.into, limit)?;
    Ok(place.holding(folder))
}

/// Writes the app's folder for the boot menu inside `into`, with the app built for the MPC Sample in it, and
/// returns the folder. It is written beside its place first and moved there whole, so the folder the boot
/// menu shows is never one that is half written.
pub fn write_folder_for_the_boot_menu(sdk: &Sdk, app_dir: &Path, into: &Path, limit: Duration) -> Result<PathBuf, NotMade> {
    let name = name_on_the_device(app_dir)?;
    let folder = into.join(&name);
    require_free_or_an_app_of_ours(&folder)?;
    let built = build_for_the_mpc_sample(sdk, app_dir, limit)?;
    let arriving = beside(&folder, ENDS_AN_ARRIVING_FOLDER);
    let written = write_folder(&arriving, app_dir, &built, &name).and_then(|()| put_in_place(&arriving, &folder));
    if written.is_err() {
        let _ = fs::remove_dir_all(&arriving);
    }
    Ok(written.map(|()| folder)?)
}

/// The app's name, which is also its folder's on the card and the name the boot menu shows.
fn name_on_the_device(app_dir: &Path) -> Result<String, NotMade> {
    let name = app_dir.file_name().and_then(|name| name.to_str()).ok_or_else(|| io::Error::other("the app's folder has no name"))?;
    if name.eq_ignore_ascii_case(link_program::FOLDER_FOR_THE_CARD) { Err(NotMade::NameIsTheLinks) } else { Ok(name.to_string()) }
}

fn require_free_or_an_app_of_ours(folder: &Path) -> Result<(), NotMade> {
    let is_free_or_ours = !folder.exists() || folder.join(START_SCRIPT_FILE).is_file();
    if is_free_or_ours { Ok(()) } else { Err(NotMade::NameIsTaken(folder.to_path_buf())) }
}

/// A folder next to `folder` that the boot menu leaves out of its list.
fn beside(folder: &Path, ending: &str) -> PathBuf {
    let name = folder.file_name().unwrap_or_default().to_string_lossy();
    folder.with_file_name(format!("{HIDES_A_FOLDER_FROM_THE_MENU}{name}{ending}"))
}

fn write_folder(folder: &Path, app_dir: &Path, built: &Path, name: &str) -> io::Result<()> {
    remove_if_there(folder)?;
    fs::create_dir_all(folder)?;
    fs::copy(built, folder.join(PROGRAM_NAME))?;
    ShownInTheMenu { name, start_script_file: START_SCRIPT_FILE, start_script: &start_script() }.write_into(folder)?;
    copy_sounds_of(app_dir, &folder.join(SOUNDS_FOLDER))
}

fn start_script() -> String {
    START_SCRIPT.replace("@PROGRAM@", PROGRAM_NAME).replace("@SOUNDS@", SOUNDS_FOLDER).replace("@STARTED_BY_THE_LINK@", STARTED_BY_THE_LINK)
}

/// The sound files the musician added to the app go along as they are, as many as an app is given.
fn copy_sounds_of(app_dir: &Path, into: &Path) -> io::Result<()> {
    let shelf = SampleShelf::of_plugin(app_dir);
    fs::create_dir_all(into)?;
    for sound in shelf.list().iter().take(MOST_SOUNDS_AN_APP_IS_GIVEN) {
        fs::write(into.join(&sound.file_name), shelf.read(sound)?)?;
    }
    Ok(())
}

/// Moves the folder just written to where it belongs. An older folder of the same app steps aside first and
/// is thrown away only once the new one is in its place.
fn put_in_place(arriving: &Path, folder: &Path) -> io::Result<()> {
    let before = beside(folder, ENDS_THE_FOLDER_BEFORE);
    remove_if_there(&before)?;
    if folder.exists() {
        fs::rename(folder, &before)?;
    }
    fs::rename(arriving, folder)?;
    remove_if_there(&before)
}

fn remove_if_there(folder: &Path) -> io::Result<()> {
    if folder.exists() { fs::remove_dir_all(folder) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::samples::test_support::wav_of;
    use crate::workspace::test_support::TemporaryFolder;

    /// An app called Night Bus with two sound files, a program that stands for the built one, and a card.
    struct Scene {
        folder: TemporaryFolder,
        app_dir: PathBuf,
        built: PathBuf,
        card: PathBuf,
    }

    impl Scene {
        fn set(label: &str) -> Scene {
            let folder = TemporaryFolder::create(label);
            let (app_dir, built, card) = (folder.0.join("Night Bus"), folder.0.join("built"), folder.0.join("card"));
            let shelf = SampleShelf::of_plugin(&app_dir);
            shelf.add("snare.wav", &wav_of(2, 44_100, 16, 30)).unwrap();
            shelf.add("kick.wav", &wav_of(1, 44_100, 16, 20)).unwrap();
            fs::write(&built, b"the program").unwrap();
            fs::create_dir_all(&card).unwrap();
            Scene { folder, app_dir, built, card }
        }

        fn written(&self) -> PathBuf {
            let folder = self.card.join("Night Bus");
            let arriving = beside(&folder, ENDS_AN_ARRIVING_FOLDER);
            write_folder(&arriving, &self.app_dir, &self.built, "Night Bus").and_then(|()| put_in_place(&arriving, &folder)).unwrap();
            folder
        }
    }

    fn names_in(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder).unwrap().flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn the_folder_holds_the_program_what_the_boot_menu_reads_and_the_apps_sounds() {
        let scene = Scene::set("device-app-folder");

        let folder = scene.written();

        assert_eq!(names_in(&folder), ["app", "autorun.json", "sampleboot.txt", "sounds", "start-app.sh"], "nothing that starts the boot menu itself is among them");
        assert_eq!(fs::read(folder.join("app")).unwrap(), b"the program");
        assert_eq!(fs::read_to_string(folder.join("sampleboot.txt")).unwrap(), "Night Bus");
        assert_eq!(fs::read_to_string(folder.join("autorun.json")).unwrap(), r#"{"scripts":[{"path":"start-app.sh"}]}"#);
        assert_eq!(names_in(&folder.join("sounds")), ["kick.wav", "snare.wav"]);
        assert_eq!(fs::read(folder.join("sounds/kick.wav")).unwrap(), wav_of(1, 44_100, 16, 20));
    }

    #[test]
    fn a_newer_folder_takes_the_place_of_the_apps_older_one_and_leaves_nothing_beside_it() {
        let scene = Scene::set("device-app-again");
        let folder = scene.written();
        fs::write(folder.join("last-run.txt"), "notes of an older run").unwrap();
        SampleShelf::of_plugin(&scene.app_dir).remove("snare.wav").unwrap();

        scene.written();

        assert_eq!(names_in(&scene.card), ["Night Bus"]);
        assert_eq!(names_in(&folder.join("sounds")), ["kick.wav"], "a sound taken out of the app is gone from the card too");
        assert!(!folder.join("last-run.txt").exists());
    }

    #[test]
    fn a_folder_of_that_name_that_mp_forge_did_not_make_is_left_alone() {
        let scene = Scene::set("device-app-name-taken");
        let somebody_elses = scene.card.join("Night Bus");
        fs::create_dir_all(&somebody_elses).unwrap();
        fs::write(somebody_elses.join("autorun.sh"), "#!/bin/sh").unwrap();

        let refused = require_free_or_an_app_of_ours(&somebody_elses);

        assert!(matches!(refused, Err(NotMade::NameIsTaken(folder)) if folder == somebody_elses));
        assert!(require_free_or_an_app_of_ours(&scene.card.join("Another App")).is_ok());
        assert!(require_free_or_an_app_of_ours(&scene.written()).is_ok(), "an app of MP Forge's own may be written again");
        drop(scene.folder);
    }

    #[test]
    fn an_app_cannot_take_the_name_of_mp_forges_own_program_on_the_card() {
        assert!(matches!(name_on_the_device(Path::new("/apps/mp forge link")), Err(NotMade::NameIsTheLinks)));
        assert_eq!(name_on_the_device(Path::new("/apps/Night Bus")).unwrap(), "Night Bus");
    }

    #[test]
    fn a_folder_still_arriving_is_one_the_boot_menu_does_not_list() {
        let arriving = beside(Path::new("/card/autorun/mods/Night Bus"), ENDS_AN_ARRIVING_FOLDER);

        assert_eq!(arriving, Path::new("/card/autorun/mods/_Night Bus.arriving"));
    }

    #[test]
    fn the_start_script_hands_the_device_back_to_akais_app_unless_the_link_started_it() {
        let script = start_script();

        assert!(script.contains("[ \"$1\" = from-mp-forge-link ] || trap 'systemctl start ac50' EXIT"), "{script}");
        assert!(script.contains("\"$HERE/sounds\" \"$CARD\" 2>>\"$SAID\""), "{script}");
        assert!(!script.contains('@') && !script.contains('\r'), "{script}");
    }

    #[test]
    fn the_start_script_keeps_off_the_devices_own_storage() {
        let script = start_script();

        assert!(script.contains("PROGRAM=/run/mp-forge-app"), "the program is run from memory");
        assert!(script.lines().filter(|line| line.contains('>')).all(|line| line.contains("\"$SAID\"")), "it writes only its own note of a run");
    }

    #[cfg(unix)]
    #[test]
    fn the_start_script_is_one_a_shell_can_read() {
        let folder = TemporaryFolder::create("device-app-start-script");
        let file = folder.0.join(START_SCRIPT_FILE);
        fs::write(&file, start_script()).unwrap();

        let read = std::process::Command::new("sh").arg("-n").arg(&file).output().unwrap();

        assert!(read.status.success(), "{}", String::from_utf8_lossy(&read.stderr));
    }
}
