//! The sound files the musician added to a Sample app, read whole for the app to play.
//!
//! An app never opens a file. Whatever runs it reads the files here, from the same shelf a plugin keeps its
//! sound files on, brings each to the rate the app runs at, and hands them over before the app starts.

use std::ffi::CString;
use std::path::Path;

use crate::mp3::{self, Mp3Error};
use crate::samples::{Sample, SampleShelf, WavHeader};
use crate::test_sound;

/// As many sound files as MP Forge's kit holds for an app.
pub const MOST_SOUNDS_AN_APP_IS_GIVEN: usize = 64;
/// As much sound as an app holds at once, counted in numbers: about twelve minutes with two sides. The device
/// has little memory, and an app that asked for more would be stopped by it.
pub const MOST_NUMBERS_OF_SOUND_AN_APP_HOLDS: usize = 64 * 1024 * 1024;
/// An MP3 is small for the sound it holds, so it is given up on once it has unpacked into all an app holds.
const MOST_FRAMES_IN_AN_MP3: usize = MOST_NUMBERS_OF_SOUND_AN_APP_HOLDS / 2;

/// One sound file as an app is handed it: its name without its ending, and its sound at the app's own rate.
/// A file with one side only has no sound of its own for the right.
#[derive(Debug, Clone, PartialEq)]
pub struct AppSound {
    pub name: CString,
    pub left: Vec<f32>,
    pub right: Option<Vec<f32>>,
}

impl AppSound {
    /// The sound a WAV file holds, at the rate the app runs at, under the name the app is to find it by.
    pub fn from_wav(name: &str, wav: &[u8], frames_a_second: u32) -> Result<AppSound, String> {
        let has_two_sides = WavHeader::read(wav).map_err(|problem| problem.to_string())?.channels > 1;
        let frames = test_sound::sound_in(wav, frames_a_second)?;
        AppSound::from_frames(name, &frames, has_two_sides).ok_or_else(|| "Its name cannot be used.".to_string())
    }

    /// The sound an MP3 file holds, at the rate the app runs at. One holding more than an app has room for is
    /// given up on.
    pub fn from_mp3(name: &str, mp3: Vec<u8>, frames_a_second: u32) -> Result<AppSound, Mp3Error> {
        let sound = mp3::decoded(mp3, MOST_FRAMES_IN_AN_MP3)?;
        let frames = test_sound::at_rate(&sound.frames, sound.sample_rate, frames_a_second);
        AppSound::from_frames(name, &frames, sound.channels > 1).ok_or(Mp3Error::Unreadable)
    }

    /// Nothing when the name cannot be handed to an app.
    fn from_frames(name: &str, frames: &[[f32; 2]], has_two_sides: bool) -> Option<AppSound> {
        let side = |side: usize| frames.iter().map(|frame| frame[side]).collect::<Vec<f32>>();
        Some(AppSound { name: CString::new(name).ok()?, left: side(0), right: has_two_sides.then(|| side(1)) })
    }

    /// How many numbers of sound it holds in memory, which is what an app's room for sound is counted in.
    pub fn numbers_held(&self) -> usize {
        self.left.len() + self.right.as_ref().map_or(0, Vec::len)
    }

    pub fn frames(&self) -> usize {
        self.left.len()
    }

    /// The right side's sound, which for a file with one side only is its left.
    pub fn right_side(&self) -> &[f32] {
        self.right.as_deref().unwrap_or(&self.left)
    }
}

/// Every sound file of the app in `app_dir`, in the order of their names. A file that can no longer be read is
/// left out, so the app is handed the others.
pub fn sounds_of(app_dir: &Path, frames_a_second: u32) -> Vec<AppSound> {
    let shelf = SampleShelf::of_plugin(app_dir);
    let kept = shelf.list();
    kept.iter().take(MOST_SOUNDS_AN_APP_IS_GIVEN).filter_map(|sample| read_from(&shelf, sample, frames_a_second)).collect()
}

fn read_from(shelf: &SampleShelf, sample: &Sample, frames_a_second: u32) -> Option<AppSound> {
    AppSound::from_wav(sample.name_without_ending(), &shelf.read(sample).ok()?, frames_a_second).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp3::test_support::{frames_in, silent_mp3};
    use crate::samples::test_support::wav_of;
    use crate::workspace::test_support::TemporaryFolder;

    fn app_with(files: &[(&str, Vec<u8>)]) -> TemporaryFolder {
        let app = TemporaryFolder::create("app-sounds");
        for (name, wav) in files {
            SampleShelf::of_plugin(&app.0).add(name, wav).unwrap();
        }
        app
    }

    #[test]
    fn an_app_with_no_sound_files_is_given_none() {
        assert!(sounds_of(&app_with(&[]).0, 44_100).is_empty());
    }

    #[test]
    fn sound_files_are_handed_over_by_name_in_the_order_of_their_names() {
        let app = app_with(&[("snare.wav", wav_of(2, 44_100, 16, 30)), ("Kick 1.WAV", wav_of(1, 44_100, 16, 20))]);

        let sounds = sounds_of(&app.0, 44_100);

        let names: Vec<&str> = sounds.iter().map(|sound| sound.name.to_str().unwrap()).collect();
        assert_eq!(names, ["Kick 1", "snare"]);
        assert_eq!((sounds[0].frames(), sounds[1].frames()), (20, 30));
    }

    #[test]
    fn a_file_with_one_side_has_the_same_sound_left_and_right() {
        let app = app_with(&[("mono.wav", wav_of(1, 44_100, 16, 10)), ("stereo.wav", wav_of(2, 44_100, 16, 10))]);

        let sounds = sounds_of(&app.0, 44_100);

        assert_eq!(sounds[0].right, None);
        assert_eq!(sounds[0].right_side().as_ptr(), sounds[0].left.as_ptr());
        assert_eq!(sounds[1].right.as_ref().map(Vec::len), Some(10));
    }

    #[test]
    fn an_mp3_is_read_with_its_sides_and_brought_to_the_rate_the_app_runs_at() {
        let stereo = AppSound::from_mp3("loop", silent_mp3(2, 10), 88_200).unwrap();
        let mono = AppSound::from_mp3("hit", silent_mp3(1, 10), 44_100).unwrap();

        assert_eq!((stereo.name.to_str().unwrap(), stereo.frames()), ("loop", frames_in(10) * 2));
        assert_eq!(stereo.right.as_ref().map(Vec::len), Some(frames_in(10) * 2));
        assert_eq!((mono.frames(), &mono.right), (frames_in(10), &None));
    }

    #[test]
    fn a_file_that_is_no_mp3_is_not_read_as_one() {
        assert_eq!(AppSound::from_mp3("words", b"these are just words".to_vec(), 44_100), Err(Mp3Error::Unreadable));
    }

    #[test]
    fn a_sound_recorded_at_another_rate_is_brought_to_the_rate_the_app_runs_at() {
        let app = app_with(&[("slow.wav", wav_of(1, 22_050, 16, 100))]);

        assert_eq!(sounds_of(&app.0, 44_100)[0].frames(), 200);
    }
}
