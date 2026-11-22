//! The SD card of the MPC Sample, as an app's file browser sees it: folders, and the WAV and MP3 files in them.
//!
//! An app never opens a file. It asks for a folder to be read or a file to be loaded, and whatever runs the app
//! answers from here. Where a folder stands in for the card, as when an app is tried, nothing outside that
//! folder can be reached by any path an app asks for. While an app is played on a computer there is no card
//! at all: the musician picks a file on the computer, and only that file is read.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::app_sounds::AppSound;
use crate::mp3::{self, Mp3Error, silent_mp3};
use crate::samples::{LARGEST_SAMPLE_IN_BYTES, sixteen_bit_wav};

/// As many things as MP Forge's kit holds of one folder.
pub const MOST_ENTRIES_OF_A_FOLDER: usize = 200;
/// The longest name the kit holds. A file or folder with a longer one is left out.
const LONGEST_NAME: usize = 64;
const WAV_ENDING: &str = ".wav";
const MEGABYTE: usize = 1024 * 1024;

/// Why a file was not loaded, in the few capitals the device's screen has room for.
const NO_CARD: &str = "NO SD CARD";
const NOT_FOUND: &str = "THAT FILE HAS GONE";
const TOO_LARGE: &str = "TOO LARGE: 16 MB AT MOST";
const NOT_A_READABLE_WAV: &str = "NOT A WAV THAT CAN BE READ";
const NOT_A_READABLE_MP3: &str = "NOT AN MP3 THAT CAN BE READ";
const TOO_LONG: &str = "TOO LONG FOR AN APP TO HOLD";
const _: () = assert!(LARGEST_SAMPLE_IN_BYTES / MEGABYTE == 16);

/// One thing in a folder of the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEntry {
    pub name: String,
    pub is_folder: bool,
}

/// What an app asked of the card: a folder to be read, or a file to be loaded as one of its samples. Both are
/// named by their path from the top of the card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskedOfTheCard {
    Folder(String),
    File { path: String, into_sample: usize },
}

/// What the card has to say to what was asked of it.
#[derive(Debug, Clone, PartialEq)]
pub enum CardAnswer {
    /// What the folder holds, or nothing when there is no card or no such folder.
    Folder(Option<Vec<CardEntry>>),
    /// There are no folders to look through: the musician picks a file on the computer instead.
    PickedOnTheComputer,
    File { into_sample: usize, loaded: Result<AppSound, &'static str> },
}

/// The card in the device, or what stands in for it: a folder, or files the musician picks on the computer.
#[derive(Debug, Clone, Default)]
pub struct SdCard {
    top: Option<PathBuf>,
    is_picked_on_the_computer: bool,
}

impl SdCard {
    pub fn none() -> Self {
        Self::default()
    }

    /// The card that `folder` stands in for. A folder that is not there is no card.
    pub fn standing_in(folder: &Path) -> Self {
        Self { top: folder.canonicalize().ok().filter(|top| top.is_dir()), is_picked_on_the_computer: false }
    }

    /// No card, on a computer where the musician picks a file whenever an app wants one from the card.
    pub fn picked_on_the_computer() -> Self {
        Self { top: None, is_picked_on_the_computer: true }
    }

    pub fn answer(&self, asked: AskedOfTheCard, frames_a_second: u32) -> CardAnswer {
        match asked {
            AskedOfTheCard::Folder(_) if self.is_picked_on_the_computer => CardAnswer::PickedOnTheComputer,
            AskedOfTheCard::Folder(path) => CardAnswer::Folder(self.list(&path)),
            AskedOfTheCard::File { path, into_sample } => CardAnswer::File { into_sample, loaded: self.load(&path, frames_a_second) },
        }
    }

    /// The folders and sound files of a folder, folders first, each in the order of their names.
    pub fn list(&self, folder: &str) -> Option<Vec<CardEntry>> {
        let listed = fs::read_dir(self.on_the_card(folder)?).ok()?;
        let mut entries: Vec<CardEntry> = listed.flatten().filter_map(|found| shown_of(&found.path())).collect();
        entries.sort_by_key(|entry| (!entry.is_folder, entry.name.to_lowercase()));
        entries.truncate(MOST_ENTRIES_OF_A_FOLDER);
        Some(entries)
    }

    /// One sound file of the card, read whole and brought to the rate the app runs at.
    pub fn load(&self, file: &str, frames_a_second: u32) -> Result<AppSound, &'static str> {
        let on_the_card = self.on_the_card(file).ok_or(if self.top.is_some() { NOT_FOUND } else { NO_CARD })?;
        read_as_sound(&on_the_card, frames_a_second)
    }

    /// Where a path asked for lies, when it lies on the card: a path that leaves the card, by its own steps or
    /// through a link, lies nowhere.
    fn on_the_card(&self, path: &str) -> Option<PathBuf> {
        let top = self.top.as_ref()?;
        let steps = Path::new(path).components().map(|step| match step {
            Component::Normal(name) => Some(name),
            _ => None,
        });
        let asked = steps.collect::<Option<PathBuf>>()?;
        top.join(asked).canonicalize().ok().filter(|found| found.starts_with(top))
    }
}

/// The file the musician picked on the computer, as the answer an app gets when it loads one from the card.
pub fn picked_on_the_computer(file: &Path, into_sample: usize, frames_a_second: u32) -> CardAnswer {
    CardAnswer::File { into_sample, loaded: read_as_sound(file, frames_a_second) }
}

/// One sound file, read whole and brought to the rate the app runs at, when it is no larger than an app is given.
fn read_as_sound(file: &Path, frames_a_second: u32) -> Result<AppSound, &'static str> {
    let size = fs::metadata(file).map_err(|_| NOT_FOUND)?.len();
    if size > LARGEST_SAMPLE_IN_BYTES as u64 {
        return Err(TOO_LARGE);
    }
    let read = fs::read(file).map_err(|_| NOT_FOUND)?;
    sound_of(&name_of(file).ok_or(NOT_FOUND)?, read, frames_a_second)
}

/// The sound a file of the card holds, under its name without its ending. A file named as an MP3 is decoded as
/// one, and any other is read as a WAV.
fn sound_of(file_name: &str, read: Vec<u8>, frames_a_second: u32) -> Result<AppSound, &'static str> {
    let name = without_ending(file_name);
    if mp3::is_named_as_mp3(file_name) {
        AppSound::from_mp3(name, read, frames_a_second).map_err(why_no_mp3)
    } else {
        AppSound::from_wav(name, &read, frames_a_second).map_err(|_| NOT_A_READABLE_WAV)
    }
}

fn why_no_mp3(problem: Mp3Error) -> &'static str {
    match problem {
        Mp3Error::TooLong => TOO_LONG,
        Mp3Error::Unreadable => NOT_A_READABLE_MP3,
    }
}

/// What a browser shows of something on the card. Hidden files, files that are not WAV or MP3 sound, and names
/// too long for the kit are left out.
fn shown_of(on_the_card: &Path) -> Option<CardEntry> {
    let name = name_of(on_the_card)?;
    let is_folder = on_the_card.is_dir();
    let is_shown = !name.starts_with('.') && name.len() <= LONGEST_NAME && (is_folder || is_named_as_sound(&name));
    is_shown.then_some(CardEntry { name, is_folder })
}

fn name_of(on_the_card: &Path) -> Option<String> {
    Some(on_the_card.file_name()?.to_str()?.to_string())
}

fn is_named_as_sound(name: &str) -> bool {
    is_named_as_wav(name) || mp3::is_named_as_mp3(name)
}

fn is_named_as_wav(name: &str) -> bool {
    let ending_starts_at = name.len().saturating_sub(WAV_ENDING.len());
    name.is_char_boundary(ending_starts_at) && name[ending_starts_at..].eq_ignore_ascii_case(WAV_ENDING)
}

fn without_ending(file_name: &str) -> &str {
    file_name.rsplit_once('.').map_or(file_name, |(before_the_ending, _)| before_the_ending)
}

// ---------------------------------------------------------------------------
// The card an app is tried with
// ---------------------------------------------------------------------------

const FRAMES_A_SECOND_ON_THE_TRIAL_CARD: u32 = 44_100;
const FILES_OF_THE_TRIAL_CARD: [(&str, f32, f32); 4] =
    [("DRUMS/kick.wav", 60.0, 0.25), ("DRUMS/snare.wav", 190.0, 0.15), ("LOOPS/bass loop.wav", 55.0, 1.0), ("tone.wav", 440.0, 0.5)];
const LOUDNESS_ON_THE_TRIAL_CARD: f32 = 0.6;
/// No MP3 can be made here but one of silence, which is enough for an app to be handed a file of that kind.
const MP3_OF_THE_TRIAL_CARD: &str = "LOOPS/quiet.mp3";
const PIECES_OF_THE_TRIAL_MP3: usize = 20;

/// Writes the small card every app is tried with into `folder`: two folders and a handful of short sounds. So
/// an app that looks through a card has one to look through in the safety check, and the same one every time.
pub fn write_trial_card(folder: &Path) -> std::io::Result<SdCard> {
    for (path, pitch, seconds) in FILES_OF_THE_TRIAL_CARD {
        write_onto_the_card(&folder.join(path), &sixteen_bit_wav(1, FRAMES_A_SECOND_ON_THE_TRIAL_CARD, &fading_tone(pitch, seconds)))?;
    }
    write_onto_the_card(&folder.join(MP3_OF_THE_TRIAL_CARD), &silent_mp3(2, PIECES_OF_THE_TRIAL_MP3))?;
    Ok(SdCard::standing_in(folder))
}

fn write_onto_the_card(file: &Path, sound: &[u8]) -> std::io::Result<()> {
    file.parent().map_or(Ok(()), fs::create_dir_all)?;
    fs::write(file, sound)
}

fn fading_tone(pitch: f32, seconds: f32) -> Vec<i16> {
    let frames = (seconds * FRAMES_A_SECOND_ON_THE_TRIAL_CARD as f32) as usize;
    let at = |frame: usize| {
        let turned = std::f32::consts::TAU * pitch * frame as f32 / FRAMES_A_SECOND_ON_THE_TRIAL_CARD as f32;
        let faded = 1.0 - frame as f32 / frames as f32;
        (turned.sin() * faded * LOUDNESS_ON_THE_TRIAL_CARD * f32::from(i16::MAX)) as i16
    };
    (0..frames).map(at).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp3::test_support::frames_in;
    use crate::samples::test_support::wav_of;
    use crate::workspace::test_support::TemporaryFolder;

    fn card_with(files: &[&str]) -> (TemporaryFolder, SdCard) {
        let folder = TemporaryFolder::create("sd-card");
        for file in files {
            let path = folder.0.join("card").join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, wav_of(1, 44_100, 16, 100)).unwrap();
        }
        fs::create_dir_all(folder.0.join("card")).unwrap();
        let card = SdCard::standing_in(&folder.0.join("card"));
        (folder, card)
    }

    fn names(entries: &[CardEntry]) -> Vec<(&str, bool)> {
        entries.iter().map(|entry| (entry.name.as_str(), entry.is_folder)).collect()
    }

    #[test]
    fn a_folder_shows_its_folders_first_and_then_its_sound_files_by_name() {
        let (_folder, card) = card_with(&["snare.wav", "Kick.WAV", "Loops/a.wav", "drums/b.wav", "notes.txt", ".hidden.wav", "._kick.wav", "Break.MP3", ".mp3", "wav"]);

        let top = card.list("").unwrap();
        let loops = card.list("Loops").unwrap();

        assert_eq!(names(&top), [("drums", true), ("Loops", true), ("Break.MP3", false), ("Kick.WAV", false), ("snare.wav", false)]);
        assert_eq!(names(&loops), [("a.wav", false)]);
    }

    #[test]
    fn a_file_is_loaded_under_its_name_without_its_ending_at_the_rate_asked_for() {
        let (_folder, card) = card_with(&["drums/Kick 1.wav"]);

        let loaded = card.load("drums/Kick 1.wav", 88_200).unwrap();

        assert_eq!((loaded.name.to_str().unwrap(), loaded.frames()), ("Kick 1", 200));
    }

    #[test]
    fn an_mp3_is_loaded_as_the_sound_it_decodes_to() {
        let (folder, card) = card_with(&[]);
        fs::write(folder.0.join("card/My Break.MP3"), silent_mp3(2, 10)).unwrap();

        let loaded = card.load("My Break.MP3", 44_100).unwrap();

        assert_eq!((loaded.name.to_str().unwrap(), loaded.frames()), ("My Break", frames_in(10)));
        assert_eq!(loaded.right.as_ref().map(Vec::len), Some(frames_in(10)));
    }

    #[test]
    fn a_file_named_as_an_mp3_that_is_not_one_says_so() {
        let (folder, card) = card_with(&[]);
        fs::write(folder.0.join("card/words.mp3"), b"these are just words").unwrap();
        fs::write(folder.0.join("card/really a wav.mp3"), wav_of(1, 44_100, 16, 100)).unwrap();

        assert_eq!(card.load("words.mp3", 44_100), Err(NOT_A_READABLE_MP3));
        assert_eq!(card.load("really a wav.mp3", 44_100), Err(NOT_A_READABLE_MP3));
    }

    #[test]
    fn an_mp3_holding_more_than_an_app_has_room_for_is_named_as_too_long() {
        assert_eq!(why_no_mp3(Mp3Error::TooLong), TOO_LONG);
    }

    #[test]
    fn nothing_outside_the_card_can_be_listed_or_loaded() {
        let (folder, card) = card_with(&["kick.wav"]);
        fs::write(folder.0.join("secret.wav"), wav_of(1, 44_100, 16, 100)).unwrap();
        let outside = folder.0.join("secret.wav");

        assert_eq!(card.list(".."), None);
        assert_eq!(card.load("../secret.wav", 44_100), Err(NOT_FOUND));
        assert_eq!(card.load(outside.to_str().unwrap(), 44_100), Err(NOT_FOUND));
        assert_eq!(card.load("./kick.wav", 44_100), Err(NOT_FOUND));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_off_the_card_is_not_followed() {
        let (folder, card) = card_with(&["kick.wav"]);
        fs::write(folder.0.join("secret.wav"), wav_of(1, 44_100, 16, 100)).unwrap();
        std::os::unix::fs::symlink(folder.0.join("secret.wav"), folder.0.join("card/link.wav")).unwrap();

        assert_eq!(card.load("link.wav", 44_100), Err(NOT_FOUND));
    }

    #[test]
    fn with_no_card_nothing_is_listed_and_nothing_loads() {
        let no_card = SdCard::none();
        let gone = SdCard::standing_in(Path::new("/nowhere/at/all"));

        assert_eq!((no_card.list(""), gone.list("")), (None, None));
        assert_eq!(no_card.load("kick.wav", 44_100), Err(NO_CARD));
    }

    #[test]
    fn a_file_that_is_not_sound_or_is_too_large_says_so_in_a_few_capitals() {
        let (folder, card) = card_with(&["kick.wav"]);
        fs::write(folder.0.join("card/words.wav"), b"these are just words").unwrap();
        fs::write(folder.0.join("card/huge.wav"), vec![0u8; LARGEST_SAMPLE_IN_BYTES + 1]).unwrap();

        assert_eq!(card.load("words.wav", 44_100), Err(NOT_A_READABLE_WAV));
        assert_eq!(card.load("huge.wav", 44_100), Err(TOO_LARGE));
        assert_eq!(card.load("gone.wav", 44_100), Err(NOT_FOUND));
    }

    #[test]
    fn what_is_asked_of_the_card_is_answered_in_kind() {
        let (_folder, card) = card_with(&["kick.wav"]);

        let folder = card.answer(AskedOfTheCard::Folder(String::new()), 44_100);
        let file = card.answer(AskedOfTheCard::File { path: "kick.wav".to_string(), into_sample: 3 }, 44_100);

        assert_eq!(folder, CardAnswer::Folder(Some(vec![CardEntry { name: "kick.wav".to_string(), is_folder: false }])));
        assert!(matches!(file, CardAnswer::File { into_sample: 3, loaded: Ok(_) }));
    }

    #[test]
    fn on_a_computer_no_folder_is_listed_and_the_file_the_musician_picked_is_what_loads() {
        let (folder, _card) = card_with(&["Kick 1.wav"]);
        let on_the_computer = SdCard::picked_on_the_computer();

        let folder_asked_for = on_the_computer.answer(AskedOfTheCard::Folder(String::new()), 44_100);
        let file_asked_for_by_path = on_the_computer.answer(AskedOfTheCard::File { path: "Kick 1.wav".to_string(), into_sample: 0 }, 44_100);
        let picked = picked_on_the_computer(&folder.0.join("card/Kick 1.wav"), 5, 44_100);
        let gone = picked_on_the_computer(&folder.0.join("card/gone.wav"), 5, 44_100);

        assert_eq!(folder_asked_for, CardAnswer::PickedOnTheComputer);
        assert_eq!(file_asked_for_by_path, CardAnswer::File { into_sample: 0, loaded: Err(NO_CARD) });
        assert!(matches!(picked, CardAnswer::File { into_sample: 5, loaded: Ok(sound) } if sound.name.to_str() == Ok("Kick 1")));
        assert_eq!(gone, CardAnswer::File { into_sample: 5, loaded: Err(NOT_FOUND) });
    }

    #[test]
    fn the_card_an_app_is_tried_with_has_folders_to_go_into_and_files_to_load() {
        let folder = TemporaryFolder::create("trial-card");

        let card = write_trial_card(&folder.0).unwrap();

        assert_eq!(names(&card.list("").unwrap()), [("DRUMS", true), ("LOOPS", true), ("tone.wav", false)]);
        assert_eq!(names(&card.list("DRUMS").unwrap()), [("kick.wav", false), ("snare.wav", false)]);
        assert_eq!(names(&card.list("LOOPS").unwrap()), [("bass loop.wav", false), ("quiet.mp3", false)]);
        assert_eq!(card.load("DRUMS/kick.wav", 44_100).unwrap().frames(), 11_025);
        assert_eq!(card.load("LOOPS/quiet.mp3", 44_100).unwrap().frames(), frames_in(PIECES_OF_THE_TRIAL_MP3));
    }
}
