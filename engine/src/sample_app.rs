//! A Sample app: a program for the MPC Sample, written with MP Forge's kit.
//!
//! Here an app is built for this computer and run, so that it can be seen and heard on the drawing of the
//! device: what the musician touches is passed to it, and its sound, its picture and its lights are taken back.
//! The app's own code is what will run on the device, unchanged.
//!
//! An app is somebody's program, loaded into the program that runs it. So it is run in a program of its own,
//! as a plugin is, and never inside MP Forge itself.

use std::ffi::{CStr, CString, c_char, c_void};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use libloading::Library;
use serde_json::Value;

use crate::app_sounds::{AppSound, MOST_NUMBERS_OF_SOUND_AN_APP_HOLDS, MOST_SOUNDS_AN_APP_IS_GIVEN};
use crate::process::{RunError, run_with_time_limit};
use crate::sd_card::{AskedOfTheCard, CardAnswer, CardEntry};
use crate::sdk::{PluginIdentity, Sdk};

/// What a Sample app's manifest says its kind is.
pub const KIND: &str = "sample-app";
pub const SCREEN_WIDTH: usize = 320;
pub const SCREEN_HEIGHT: usize = 240;
pub const PADS: usize = 16;
/// The device's buttons in the order the kit counts them, by the words the screen knows them by.
pub const BUTTONS: [&str; 24] = [
    "B1", "B2", "B3", "SAMPLE", "SEQ", "PAD FX", "KNOB FX", "SHIFT", "PAD BANK", "ERASE", "NOTE REPEAT", "CHOP", "MUTE", "LOOP", "16 LEVELS",
    "SAMPLE SELECT", "TAP TEMPO", "MINUS", "PLUS", "SAMPLE RECORD", "SEQ RECORD", "STOP", "PLAY", "ENCODER",
];

const MANIFEST: &str = "plugin.json";
const FIRST_VERSION: &str = "1.0.0";
const STARTER_FILE: &str = "App.cpp";
const STARTER: &str = include_str!("../helpers/sample/starter.cpp");
const NAME_IN_THE_STARTER: &str = "@NAME@";
const MARKS_THE_KIT_CAN_WRITE: [char; 5] = [' ', '-', '+', ':', '.'];
const KIT_FILE: &str = "mpc_sample_app.h";
const KIT: &str = include_str!("../helpers/sample/mpc_sample_app.h");
/// The kit an app has to have been built with. An app built with another is not run.
const KIT_NUMBER: i32 = 5;
const NO_ROOM_FOR_IT: &CStr = c"NO ROOM LEFT FOR IT";
const LONGEST_PATH_ON_THE_CARD: usize = 255;
const ASKED_FOR_A_FOLDER: i32 = 1;
const ASKED_FOR_A_FILE: i32 = 2;
const CARD_IS_MISSING: i32 = 0;
const CARD_IS_THERE: i32 = 1;
const CARD_IS_PICKED_ON_THE_COMPUTER: i32 = 2;
/// Where in the number an app is handed for a MIDI message it is said where the message came in.
const WHERE_MIDI_CAME_IN_STARTS_AT_BIT: u32 = 24;
const CODE_FOLDER: &str = "src";
const CODE_FILE_ENDING: &str = "cpp";
const BUILT_FOR_THIS_COMPUTER: &str = "build/native";
const BYTES_A_DOT: usize = 4;
const COLOURS_A_PAD: usize = 3;
pub(crate) const HARDEST_HIT: f32 = 127.0;
const COMPILER_FLAGS: [&str; 8] = ["-std=c++17", "-fPIC", "-shared", "-O2", "-fvisibility=hidden", "-fvisibility-inlines-hidden", "-Wall", "-Wextra"];
/// On Windows the compiler is told which way of calling functions to use, as Akai's own build does.
const WINDOWS_TARGET: &str = if cfg!(target_arch = "aarch64") { "aarch64-windows-gnu" } else { "x86_64-windows-gnu" };

// ---------------------------------------------------------------------------
// A new app
// ---------------------------------------------------------------------------

/// Whether what is in this folder is a Sample app, going by what its manifest says its kind is.
pub fn is_one(project_dir: &Path) -> bool {
    let manifest = fs::read_to_string(project_dir.join(MANIFEST)).ok().and_then(|written| serde_json::from_str::<Value>(&written).ok());
    manifest.is_some_and(|manifest| manifest["kind"] == KIND)
}

/// Makes the folder of a new app under `plugins_dir`: who it is, the kit, and a small app that already runs,
/// for the real one to be written in the place of.
pub fn create_starter(plugins_dir: &Path, identity: &PluginIdentity) -> io::Result<PathBuf> {
    let app_dir = plugins_dir.join(&identity.name);
    let manifest = serde_json::json!({ "name": identity.name, "manufacturer": identity.maker, "version": FIRST_VERSION, "kind": KIND });
    put_kit_into(&app_dir)?;
    fs::write(app_dir.join(MANIFEST), format!("{manifest:#}\n"))?;
    fs::write(app_dir.join(CODE_FOLDER).join(STARTER_FILE), STARTER.replace(NAME_IN_THE_STARTER, &as_the_kit_writes(&identity.name)))?;
    Ok(app_dir)
}

/// A name in the letters the kit can write on the screen, which are also all safe inside a line of code.
fn as_the_kit_writes(name: &str) -> String {
    let can_be_written = |letter: &char| letter.is_ascii_alphanumeric() || MARKS_THE_KIT_CAN_WRITE.contains(letter);
    name.to_uppercase().chars().filter(can_be_written).collect()
}

// ---------------------------------------------------------------------------
// Building an app for this computer
// ---------------------------------------------------------------------------

/// Why an app was not built.
#[derive(Debug)]
pub enum NotBuilt {
    /// The compiler MP Forge builds with has not been fetched yet.
    NoCompiler,
    NoCode,
    /// The compiler stopped, and said this.
    DidNotCompile(String),
    Io(io::Error),
}

impl fmt::Display for NotBuilt {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NotBuilt::NoCompiler => write!(formatter, "The compiler has not been fetched yet. Check Settings, Build tools."),
            NotBuilt::NoCode => write!(formatter, "The app has no code in its `{CODE_FOLDER}` folder."),
            NotBuilt::DidNotCompile(said) => write!(formatter, "The app's code did not build:\n{said}"),
            NotBuilt::Io(cause) => write!(formatter, "The app could not be built: {cause}"),
        }
    }
}

impl From<io::Error> for NotBuilt {
    fn from(cause: io::Error) -> Self {
        NotBuilt::Io(cause)
    }
}

/// Writes MP Forge's kit into the app. It is written afresh before every build, so the kit an app is built with
/// is always MP Forge's own.
pub fn put_kit_into(app_dir: &Path) -> io::Result<()> {
    write_kit_in(&app_dir.join(CODE_FOLDER))
}

/// Writes the kit into a folder of code, which is made when it is not there yet.
pub(crate) fn write_kit_in(code_folder: &Path) -> io::Result<()> {
    fs::create_dir_all(code_folder)?;
    fs::write(code_folder.join(KIT_FILE), KIT)
}

/// Whether a file among an app's code is MP Forge's kit, exactly as MP Forge writes it.
pub fn is_the_kit(within_the_code: &Path, written: &[u8]) -> bool {
    within_the_code == Path::new(KIT_FILE) && written == KIT.as_bytes()
}

/// Builds the app for this computer and returns the library that was built.
pub fn build_for_this_computer(sdk: &Sdk, app_dir: &Path, limit: Duration) -> Result<PathBuf, NotBuilt> {
    put_kit_into(app_dir)?;
    let code = code_files_of(app_dir)?;
    if code.is_empty() {
        return Err(NotBuilt::NoCode);
    }
    let library = library_of(app_dir);
    fs::create_dir_all(app_dir.join(BUILT_FOR_THIS_COMPUTER))?;
    let mut compiler = sdk.compiler().ok_or(NotBuilt::NoCompiler)?;
    compiler.arg("c++").args(COMPILER_FLAGS).args(target_flags()).arg("-o").arg(&library).args(&code);
    compile_with(&mut compiler, limit).map(|()| library)
}

/// Runs the compiler on what it was told to build, for no longer than `limit`.
pub(crate) fn compile_with(compiler: &mut Command, limit: Duration) -> Result<(), NotBuilt> {
    match run_with_time_limit(compiler, limit) {
        Ok(finished) if finished.succeeded() => Ok(()),
        Ok(finished) => Err(NotBuilt::DidNotCompile(finished.output)),
        Err(RunError::CouldNotStart(cause)) => Err(NotBuilt::Io(cause)),
        Err(RunError::TimedOut { output, .. }) => Err(NotBuilt::DidNotCompile(format!("The compiler took too long.\n{output}"))),
    }
}

/// Where the app's library for this computer is, once it has been built.
pub fn library_of(app_dir: &Path) -> PathBuf {
    app_dir.join(BUILT_FOR_THIS_COMPUTER).join(format!("sample-app.{}", std::env::consts::DLL_EXTENSION))
}

fn target_flags() -> Vec<&'static str> {
    if cfg!(windows) { vec!["-target", WINDOWS_TARGET] } else { Vec::new() }
}

/// The files of the app's own code, in the order of their names.
pub(crate) fn code_files_of(app_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut code: Vec<PathBuf> = fs::read_dir(app_dir.join(CODE_FOLDER))?.flatten().map(|entry| entry.path()).collect();
    code.retain(|file| file.extension().is_some_and(|ending| ending == CODE_FILE_ENDING));
    code.sort();
    Ok(code)
}

// ---------------------------------------------------------------------------
// What an app is told
// ---------------------------------------------------------------------------

/// The kinds of control, by the numbers the kit knows them by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Touched {
    Pad = 0,
    Knob = 1,
    Fader = 2,
    Encoder = 3,
    Button = 4,
    Midi = 5,
}

pub(crate) const NOTE_ON: u8 = 0x90;
pub(crate) const NOTE_OFF: u8 = 0x80;
pub(crate) const CONTROLLER: u8 = 0xB0;
const PITCH_BEND: u8 = 0xE0;
const MOST_IN_A_MIDI_BYTE: u8 = 127;
const PITCH_BEND_AT_REST: f32 = 8192.0;
const MOST_PITCH_BEND: f32 = 16_383.0;

/// One thing the musician did on the device or on something plugged into it, as the app is told it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    touched: Touched,
    number: i32,
    value: f32,
}

impl Input {
    /// A pad hit, as hard as `strength` says from just above 0 to 1. Pads are numbered from 1.
    pub fn pad_hit(pad: i32, strength: f32) -> Input {
        Input { touched: Touched::Pad, number: pad, value: strength.clamp(f32::MIN_POSITIVE, 1.0) }
    }

    pub fn pad_let_go(pad: i32) -> Input {
        Input { touched: Touched::Pad, number: pad, value: 0.0 }
    }

    pub fn knob_turned(knob: i32, stands: f32) -> Input {
        Input { touched: Touched::Knob, number: knob, value: stands.clamp(0.0, 1.0) }
    }

    pub fn fader_moved(stands: f32) -> Input {
        Input { touched: Touched::Fader, number: 0, value: stands.clamp(0.0, 1.0) }
    }

    pub fn encoder_turned(clicks: i32) -> Input {
        Input { touched: Touched::Encoder, number: clicks, value: 0.0 }
    }

    /// A button pressed or let go, by the words the screen knows it by. None for a button the device has not got.
    pub fn button(name: &str, is_pressed: bool) -> Option<Input> {
        let number = BUTTONS.iter().position(|button| *button == name)?;
        Some(Input { touched: Touched::Button, number: number as i32, value: if is_pressed { 1.0 } else { 0.0 } })
    }

    /// One MIDI message as it arrives, of three bytes. The app is handed them in one number, the first byte
    /// lowest. None for a message of another length, which is nothing an app is told of.
    pub fn midi(message: &[u8]) -> Option<Input> {
        let &[status, first, second] = message else { return None };
        Some(Input { touched: Touched::Midi, number: i32::from(status) | i32::from(first) << 8 | i32::from(second) << 16, value: 0.0 })
    }

    /// The same MIDI message, said to have come in at one of the device's sockets. Left unsaid, it stands in
    /// for both, as a computer's MIDI does.
    pub fn having_come_in(self, came_in: MidiCameIn) -> Input {
        Input { number: self.number | (came_in as i32) << WHERE_MIDI_CAME_IN_STARTS_AT_BIT, ..self }
    }

    /// A note played on something plugged in, as hard as `velocity` says from 1 to 127. Channels are numbered from 1.
    pub fn note_on(note: u8, velocity: u8, channel: u8) -> Input {
        Input::midi_of(NOTE_ON, channel, [note, velocity.clamp(1, MOST_IN_A_MIDI_BYTE)])
    }

    pub fn note_off(note: u8, channel: u8) -> Input {
        Input::midi_of(NOTE_OFF, channel, [note, 0])
    }

    /// A controller, such as a wheel or a knob of a keyboard, moved to where `stands` says from 0 to 127.
    pub fn controller_moved(controller: u8, stands: u8, channel: u8) -> Input {
        Input::midi_of(CONTROLLER, channel, [controller, stands])
    }

    /// The pitch wheel, from -1 to 1 with 0 where it rests.
    pub fn pitch_bent(bend: f32, channel: u8) -> Input {
        let written = (PITCH_BEND_AT_REST + bend.clamp(-1.0, 1.0) * PITCH_BEND_AT_REST).min(MOST_PITCH_BEND) as u16;
        Input::midi_of(PITCH_BEND, channel, [(written & 0x7F) as u8, (written >> 7) as u8])
    }

    fn midi_of(kind: u8, channel: u8, data: [u8; 2]) -> Input {
        let status = kind | (channel.clamp(1, 16) - 1);
        let within_a_midi_byte = data.map(|byte| byte.min(MOST_IN_A_MIDI_BYTE));
        Input { touched: Touched::Midi, number: i32::from(status) | i32::from(within_a_midi_byte[0]) << 8 | i32::from(within_a_midi_byte[1]) << 16, value: 0.0 }
    }

    /// What the screen said was done, in the screen's own words: which `control`, its `number` or `name`, and
    /// a `value`. MIDI is said as a `note` or a `controller` with a `value` up to 127, or a `bend` from -1 to
    /// 1, each on `channel` 1 when none is named. None for anything that is not something the device can do.
    pub fn said_by_the_screen(said: &Value) -> Option<Input> {
        let value = said["value"].as_f64()? as f32;
        let number = || said["number"].as_i64().and_then(|number| i32::try_from(number).ok());
        let midi_number = || number().and_then(|number| u8::try_from(number).ok()).filter(|number| *number <= MOST_IN_A_MIDI_BYTE);
        let channel = said["channel"].as_u64().map_or(1, |channel| channel.clamp(1, 16) as u8);
        match said["control"].as_str()? {
            "note" if value > 0.0 => Some(Input::note_on(midi_number()?, value as u8, channel)),
            "note" => Some(Input::note_off(midi_number()?, channel)),
            "controller" => Some(Input::controller_moved(midi_number()?, value.clamp(0.0, 127.0) as u8, channel)),
            "bend" => Some(Input::pitch_bent(value, channel)),
            "pad" if value > 0.0 => Some(Input::pad_hit(number()?, value / HARDEST_HIT)),
            "pad" => Some(Input::pad_let_go(number()?)),
            "knob" => Some(Input::knob_turned(number()?, value)),
            "fader" => Some(Input::fader_moved(value)),
            "encoder" => Some(Input::encoder_turned(value as i32)),
            "button" => Input::button(said["name"].as_str()?, value > 0.0),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// An app, running
// ---------------------------------------------------------------------------

type Make = unsafe extern "C" fn(f64, *const SoundGiven, i32) -> *mut c_void;
type Free = unsafe extern "C" fn(*mut c_void);
type Told = unsafe extern "C" fn(*mut c_void, i32, i32, f32);
type Sound = unsafe extern "C" fn(*mut c_void, *const f32, *const f32, *mut f32, *mut f32, i32);
type Show = unsafe extern "C" fn(*mut c_void, *mut u8, *mut u8, *mut u8);
type KitNumber = unsafe extern "C" fn() -> i32;
type Asked = unsafe extern "C" fn(*mut c_void, *mut c_char, *mut i32) -> i32;
type Listed = unsafe extern "C" fn(*mut c_void, *const EntryGiven, i32, i32);
type Loaded = unsafe extern "C" fn(*mut c_void, i32, *const SoundGiven, *const c_char);
type SoundInChosen = unsafe extern "C" fn(*mut c_void) -> i32;
type BrowsingInto = unsafe extern "C" fn(*mut c_void) -> i32;
type CloseBrowser = unsafe extern "C" fn(*mut c_void);

/// Where a MIDI message came in: at the device's USB-C socket or its MIDI IN socket. A computer's MIDI stands
/// in for both. The numbers are the kit's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MidiCameIn {
    AtEither = 0,
    AtUsb = 1,
    AtTheSocket = 2,
}

/// Where an app has its inputs listen: the jacks at the back of the device, its built-in microphone, its USB-C
/// socket, or nowhere. The
/// numbers are the kit's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundIn {
    LineIn = 0,
    Usb = 1,
    Off = 2,
    Mic = 3,
}

/// One sound file as the kit takes it: where its name and its sound are, for as long as the app runs.
#[repr(C)]
struct SoundGiven {
    name: *const c_char,
    left: *const f32,
    right: *const f32,
    frames: i32,
}

impl SoundGiven {
    fn of(sound: &AppSound) -> SoundGiven {
        SoundGiven { name: sound.name.as_ptr(), left: sound.left.as_ptr(), right: sound.right_side().as_ptr(), frames: sound.frames() as i32 }
    }
}

/// One thing in a folder of the card as the kit takes it.
#[repr(C)]
struct EntryGiven {
    name: *const c_char,
    is_folder: i32,
}

/// What an app shows: the picture on its screen, four bytes a dot, and the lights it sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub picture: Vec<u8>,
    pub pad_colours: [[u8; COLOURS_A_PAD]; PADS],
    pub lit_buttons: Vec<&'static str>,
}

/// A built app, loaded and started. Whoever holds it calls it from one place at a time, as the kit promises
/// the app.
pub struct RunningApp {
    made: *mut c_void,
    told: Told,
    sound: Sound,
    show: Show,
    free: Free,
    asked: Asked,
    listed: Listed,
    loaded: Loaded,
    sound_in: SoundInChosen,
    browsing_into: BrowsingInto,
    close_browser: CloseBrowser,
    /// Handed to an app that has its inputs off, in place of whatever came in.
    silence: Vec<f32>,
    /// The app reads its samples where they lie here, each at its number, so they are kept for as long as it
    /// runs or until another is loaded in their place.
    sounds: Vec<Option<AppSound>>,
    /// The app's code stays loaded for as long as the app runs.
    _library: Library,
}

/// The app is only ever reached through `&mut self`, so it is called from one thread at a time.
unsafe impl Send for RunningApp {}

impl RunningApp {
    /// Loads a built app and starts it, with the musician's sound files for it to play.
    pub fn load(library_file: &Path, frames_a_second: f64, sounds: Vec<AppSound>) -> Result<Self, String> {
        let not_an_app = |error: libloading::Error| format!("That is not a Sample app built with MP Forge's kit: {error}");
        let library = unsafe { Library::new(library_file) }.map_err(|error| format!("The app could not be loaded: {error}"))?;
        let function = |name: &[u8]| unsafe { library.get::<*const c_void>(name).map(|found| *found) }.map_err(not_an_app);
        let kit_number: KitNumber = unsafe { std::mem::transmute(function(b"mp_forge_sample_kit")?) };
        if unsafe { kit_number() } != KIT_NUMBER {
            return Err("The app was built with another version of MP Forge's kit. Build it again.".to_string());
        }
        let make: Make = unsafe { std::mem::transmute(function(b"mp_forge_sample_make")?) };
        let given: Vec<SoundGiven> = sounds.iter().map(SoundGiven::of).collect();
        Ok(RunningApp {
            told: unsafe { std::mem::transmute::<*const c_void, Told>(function(b"mp_forge_sample_told")?) },
            sound: unsafe { std::mem::transmute::<*const c_void, Sound>(function(b"mp_forge_sample_sound")?) },
            show: unsafe { std::mem::transmute::<*const c_void, Show>(function(b"mp_forge_sample_show")?) },
            free: unsafe { std::mem::transmute::<*const c_void, Free>(function(b"mp_forge_sample_free")?) },
            asked: unsafe { std::mem::transmute::<*const c_void, Asked>(function(b"mp_forge_sample_asked")?) },
            listed: unsafe { std::mem::transmute::<*const c_void, Listed>(function(b"mp_forge_sample_listed")?) },
            loaded: unsafe { std::mem::transmute::<*const c_void, Loaded>(function(b"mp_forge_sample_loaded")?) },
            sound_in: unsafe { std::mem::transmute::<*const c_void, SoundInChosen>(function(b"mp_forge_sample_sound_in")?) },
            browsing_into: unsafe { std::mem::transmute::<*const c_void, BrowsingInto>(function(b"mp_forge_sample_browsing_into")?) },
            close_browser: unsafe { std::mem::transmute::<*const c_void, CloseBrowser>(function(b"mp_forge_sample_close_browser")?) },
            silence: Vec::new(),
            made: unsafe { make(frames_a_second, given.as_ptr(), given.len() as i32) },
            sounds: at_their_numbers(sounds),
            _library: library,
        })
    }

    pub fn tell(&mut self, input: Input) {
        unsafe { (self.told)(self.made, input.touched as i32, input.number, input.value) }
    }

    /// Has the app make its next stretch of sound: as many frames as the shortest of the four is long. What
    /// came in reaches an app that listens at its inputs, and an app that has them off is handed silence.
    pub fn make_sound(&mut self, in_left: &[f32], in_right: &[f32], out_left: &mut [f32], out_right: &mut [f32]) {
        let frames = in_left.len().min(in_right.len()).min(out_left.len()).min(out_right.len());
        let (in_left, in_right) = if self.sound_in() == SoundIn::Off { self.silence_for(frames) } else { (in_left.as_ptr(), in_right.as_ptr()) };
        unsafe { (self.sound)(self.made, in_left, in_right, out_left.as_mut_ptr(), out_right.as_mut_ptr(), frames as i32) }
    }

    /// Where the app has its inputs listen. It starts as nowhere, and the app or the musician can change it.
    pub fn sound_in(&self) -> SoundIn {
        match unsafe { (self.sound_in)(self.made) } {
            1 => SoundIn::Usb,
            2 => SoundIn::Off,
            3 => SoundIn::Mic,
            _ => SoundIn::LineIn,
        }
    }

    fn silence_for(&mut self, frames: usize) -> (*const f32, *const f32) {
        self.silence.resize(frames.max(self.silence.len()), 0.0);
        (self.silence.as_ptr(), self.silence.as_ptr())
    }

    pub fn show(&mut self) -> Shown {
        let mut picture = vec![0u8; SCREEN_WIDTH * SCREEN_HEIGHT * BYTES_A_DOT];
        let mut pad_colours = [[0u8; COLOURS_A_PAD]; PADS];
        let mut lit = [0u8; BUTTONS.len()];
        unsafe { (self.show)(self.made, picture.as_mut_ptr(), pad_colours.as_mut_ptr().cast(), lit.as_mut_ptr()) };
        let lit_buttons = BUTTONS.iter().zip(lit).filter(|(_, is_lit)| *is_lit != 0).map(|(button, _)| *button).collect();
        Shown { picture, pad_colours, lit_buttons }
    }
}

/// What the card is asked for and answers. An app asks and goes on running: whoever holds it takes what was
/// asked between two calls, fetches it with the app left free to play, and hands the answer over.
impl RunningApp {
    pub fn asked_of_the_card(&mut self) -> Option<AskedOfTheCard> {
        let mut path = vec![0 as c_char; LONGEST_PATH_ON_THE_CARD + 1];
        let mut into_sample = 0i32;
        let asked = unsafe { (self.asked)(self.made, path.as_mut_ptr(), &mut into_sample) };
        let path = unsafe { CStr::from_ptr(path.as_ptr()) }.to_string_lossy().into_owned();
        match asked {
            ASKED_FOR_A_FOLDER => Some(AskedOfTheCard::Folder(path)),
            ASKED_FOR_A_FILE => Some(AskedOfTheCard::File { path, into_sample: usize::try_from(into_sample).unwrap_or(usize::MAX) }),
            _ => None,
        }
    }

    /// The sample the kit's own page is open to load a file as, or none when the page is not open.
    pub fn browsing_into(&self) -> Option<usize> {
        usize::try_from(unsafe { (self.browsing_into)(self.made) }).ok()
    }

    /// Closes the kit's own page, as when the musician decides against picking a file.
    pub fn close_browser(&mut self) {
        unsafe { (self.close_browser)(self.made) }
    }

    pub fn take_answer(&mut self, answer: CardAnswer) {
        match answer {
            CardAnswer::Folder(Some(found)) => self.hand_over_folder(&found, CARD_IS_THERE),
            CardAnswer::Folder(None) => self.hand_over_folder(&[], CARD_IS_MISSING),
            CardAnswer::PickedOnTheComputer => self.hand_over_folder(&[], CARD_IS_PICKED_ON_THE_COMPUTER),
            CardAnswer::File { into_sample, loaded: Ok(sound) } if self.has_room_for(&sound, into_sample) => self.hand_over_sound(sound, into_sample),
            CardAnswer::File { loaded: Ok(_), .. } => self.say_why_not(NO_ROOM_FOR_IT),
            CardAnswer::File { loaded: Err(why_not), .. } => self.say_why_not(&CString::new(why_not).unwrap_or_default()),
        }
    }

    fn hand_over_folder(&mut self, found: &[CardEntry], card_is: i32) {
        let names: Vec<CString> = found.iter().map(|entry| CString::new(entry.name.as_str()).unwrap_or_default()).collect();
        let given: Vec<EntryGiven> = names.iter().zip(found).map(|(name, entry)| EntryGiven { name: name.as_ptr(), is_folder: i32::from(entry.is_folder) }).collect();
        unsafe { (self.listed)(self.made, given.as_ptr(), given.len() as i32, card_is) }
    }

    /// The sound that was at the number is let go only once the app has been handed the new one.
    fn hand_over_sound(&mut self, sound: AppSound, into_sample: usize) {
        unsafe { (self.loaded)(self.made, into_sample as i32, &SoundGiven::of(&sound), std::ptr::null()) };
        self.sounds[into_sample] = Some(sound);
    }

    fn say_why_not(&mut self, why_not: &CStr) {
        unsafe { (self.loaded)(self.made, 0, std::ptr::null(), why_not.as_ptr()) }
    }

    /// Whether the app would still hold no more sound than the device has memory for, with this one at that number.
    fn has_room_for(&self, sound: &AppSound, into_sample: usize) -> bool {
        let held_elsewhere = |(number, held): (usize, &Option<AppSound>)| if number == into_sample { 0 } else { held.as_ref().map_or(0, AppSound::numbers_held) };
        let held: usize = self.sounds.iter().enumerate().map(held_elsewhere).sum();
        into_sample < self.sounds.len() && held + sound.numbers_held() <= MOST_NUMBERS_OF_SOUND_AN_APP_HOLDS
    }
}

/// The sounds an app starts with, each at the number the app finds it at, with room for the rest of the numbers.
fn at_their_numbers(sounds: Vec<AppSound>) -> Vec<Option<AppSound>> {
    let mut numbered: Vec<Option<AppSound>> = sounds.into_iter().take(MOST_SOUNDS_AN_APP_IS_GIVEN).map(Some).collect();
    numbered.resize(MOST_SOUNDS_AN_APP_IS_GIVEN, None);
    numbered
}

impl Drop for RunningApp {
    fn drop(&mut self) {
        unsafe { (self.free)(self.made) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn what_the_screen_says_was_touched_becomes_what_the_app_is_told() {
        let said = |said: Value| Input::said_by_the_screen(&said);

        assert_eq!(said(json!({ "control": "pad", "number": 5, "value": 127 })), Some(Input::pad_hit(5, 1.0)));
        assert_eq!(said(json!({ "control": "pad", "number": 5, "value": 0 })), Some(Input::pad_let_go(5)));
        assert_eq!(said(json!({ "control": "knob", "number": 2, "value": 0.25 })), Some(Input::knob_turned(2, 0.25)));
        assert_eq!(said(json!({ "control": "fader", "value": 3.0 })), Some(Input::fader_moved(1.0)));
        assert_eq!(said(json!({ "control": "encoder", "value": -2 })), Some(Input::encoder_turned(-2)));
        assert_eq!(said(json!({ "control": "button", "name": "PLAY", "value": 1 })), Input::button("PLAY", true));
    }

    #[test]
    fn midi_is_handed_over_as_its_three_bytes_in_one_number() {
        assert_eq!(Input::midi(&[0x90, 60, 100]), Some(Input { touched: Touched::Midi, number: 0x90 | 60 << 8 | 100 << 16, value: 0.0 }));
        assert_eq!(Input::note_on(60, 100, 1), Input::midi(&[0x90, 60, 100]).unwrap());
        assert_eq!(Input::note_on(60, 0, 16), Input::midi(&[0x9F, 60, 1]).unwrap(), "a note on is never as soft as a note let go");
        assert_eq!(Input::note_off(60, 2), Input::midi(&[0x81, 60, 0]).unwrap());
        assert_eq!(Input::controller_moved(1, 200, 1), Input::midi(&[0xB0, 1, 127]).unwrap());
        assert_eq!([Input::pitch_bent(0.0, 1), Input::pitch_bent(-1.0, 1), Input::pitch_bent(9.0, 1)], [[0xE0, 0, 64], [0xE0, 0, 0], [0xE0, 127, 127]].map(|bytes| Input::midi(&bytes).unwrap()));
    }

    #[test]
    fn a_midi_message_that_is_not_three_bytes_long_is_nothing_an_app_is_told_of() {
        assert_eq!(Input::midi(&[0xC0, 5]), None);
        assert_eq!(Input::midi(&[0xF8]), None);
        assert_eq!(Input::midi(&[0xF0, 1, 2, 0xF7]), None);
    }

    #[test]
    fn midi_is_said_by_the_screen_as_notes_controllers_and_bends() {
        let said = |said: Value| Input::said_by_the_screen(&said);

        assert_eq!(said(json!({ "control": "note", "number": 60, "value": 100 })), Some(Input::note_on(60, 100, 1)));
        assert_eq!(said(json!({ "control": "note", "number": 60, "value": 0, "channel": 10 })), Some(Input::note_off(60, 10)));
        assert_eq!(said(json!({ "control": "controller", "number": 1, "value": 64 })), Some(Input::controller_moved(1, 64, 1)));
        assert_eq!(said(json!({ "control": "bend", "value": -1 })), Some(Input::pitch_bent(-1.0, 1)));
        assert_eq!(said(json!({ "control": "note", "number": 128, "value": 100 })), None);
    }

    #[test]
    fn the_kit_reads_midi_as_it_is_handed_over() {
        assert!(KIT.contains("enum class Touched : int32_t { Pad = 0, Knob = 1, Fader = 2, Encoder = 3, Button = 4, Midi = 5 };"));
        assert!(KIT.contains("enum class Midi : int32_t { NoteOff = 0x80, NoteOn = 0x90, Controller = 0xB0, PitchBend = 0xE0 };"));
    }

    #[test]
    fn where_midi_came_in_is_said_above_its_three_bytes_as_the_kit_reads_it() {
        let played = Input::note_on(60, 100, 1);

        assert!(KIT.contains("enum class MidiCameIn : int32_t { AtEither = 0, AtUsb = 1, AtTheSocket = 2 };"));
        assert!(KIT.contains(&format!("static_cast<MidiCameIn>(message >> {WHERE_MIDI_CAME_IN_STARTS_AT_BIT})")));
        assert_eq!(played.having_come_in(MidiCameIn::AtEither), played);
        assert_eq!(played.having_come_in(MidiCameIn::AtTheSocket).number, played.number | 2 << 24);
    }

    #[test]
    fn what_the_device_cannot_do_is_not_passed_on() {
        let said = |said: Value| Input::said_by_the_screen(&said);

        assert_eq!(said(json!({ "control": "button", "name": "SELF DESTRUCT", "value": 1 })), None);
        assert_eq!(said(json!({ "control": "wheel", "value": 1 })), None);
        assert_eq!(said(json!({ "control": "pad", "value": 100 })), None);
        assert_eq!(said(json!({ "control": "knob", "number": 1 })), None);
    }

    #[test]
    fn the_buttons_are_counted_as_the_kit_counts_them() {
        assert!(KIT.contains("B1, B2, B3, Sample, Seq, PadFx, KnobFx, Shift, PadBank, Erase, NoteRepeat, Chop, Mute, Loop, SixteenLevels,"));
        assert!(KIT.contains("SampleSelect, TapTempo, Minus, Plus, SampleRecord, SeqRecord, Stop, Play, Encoder,"));
        assert_eq!((BUTTONS[0], BUTTONS[14], BUTTONS[22], BUTTONS[23]), ("B1", "16 LEVELS", "PLAY", "ENCODER"));
        assert!(KIT.contains(&format!("int32_t mp_forge_sample_kit() {{ return {KIT_NUMBER}; }}")));
    }

    #[test]
    fn where_sound_comes_in_from_is_numbered_as_the_kit_numbers_it() {
        assert!(KIT.contains("enum class SoundIn : int32_t { LineIn = 0, Usb = 1, Off = 2, Mic = 3 };"));
        assert_eq!([SoundIn::LineIn as i32, SoundIn::Usb as i32, SoundIn::Off as i32, SoundIn::Mic as i32], [0, 1, 2, 3]);
    }

    #[test]
    fn the_kit_holds_as_many_sound_files_as_an_app_is_given() {
        assert!(KIT.contains(&format!("constexpr int kMostSamples = {};", crate::app_sounds::MOST_SOUNDS_AN_APP_IS_GIVEN)));
    }

    #[test]
    fn the_kit_holds_as_much_of_the_card_as_it_is_handed() {
        assert!(KIT.contains(&format!("constexpr int kMostCardEntries = {};", crate::sd_card::MOST_ENTRIES_OF_A_FOLDER)));
        assert!(KIT.contains(&format!("constexpr int kLongestCardPath = {LONGEST_PATH_ON_THE_CARD};")));
        assert!(KIT.contains(&format!("enum class Asked : int32_t {{ Nothing = 0, Folder = {ASKED_FOR_A_FOLDER}, File = {ASKED_FOR_A_FILE} }};")));
        assert!(KIT.contains(&format!(
            "enum class CardIs : int32_t {{ Missing = {CARD_IS_MISSING}, There = {CARD_IS_THERE}, PickedOnTheComputer = {CARD_IS_PICKED_ON_THE_COMPUTER} }};"
        )));
        assert!(KIT.contains("return browser_.isOpen() ? browser_.intoSample() : -1;"), "a number below 0 is no sample");
    }
}
