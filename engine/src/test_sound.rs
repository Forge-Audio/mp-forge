//! A sound file of the musician's own, read so that an effect can be heard working on it in the live player
//! in place of the built-in beat.
//!
//! The file is only read. It is decoded here, in the player's own process, into plain numbers the plugin is
//! fed like any other sound. It is not copied into the plugin and never goes to the MPC. It can be a WAV
//! file or an MP3.

use std::fs;
use std::path::Path;

use crate::mp3::{self, Mp3Error};
use crate::samples::WavHeader;

const LARGEST_FILE_IN_BYTES: u64 = 96 * 1024 * 1024;
const MEGABYTE: u64 = 1024 * 1024;
const BITS_IN_A_BYTE: u16 = 8;
const LONGEST_MP3_IN_MINUTES: usize = 10;
/// The most frames an MP3 may decode to: its longest length at the highest rate an MP3 is recorded at.
const MOST_FRAMES_IN_AN_MP3: usize = LONGEST_MP3_IN_MINUTES * 60 * 48_000;

/// Reads a WAV or MP3 file as frames of left and right, at the rate the speakers play at.
pub fn read_for_playing(file: &Path, sample_rate: u32) -> Result<Vec<[f32; 2]>, String> {
    let size = fs::metadata(file).map_err(|_| "That sound file is no longer where it was. Choose it again.".to_string())?.len();
    if size > LARGEST_FILE_IN_BYTES {
        return Err(format!("That sound file is {} MB. The largest that can be played here is {} MB.", size / MEGABYTE, LARGEST_FILE_IN_BYTES / MEGABYTE));
    }
    let bytes = fs::read(file).map_err(|problem| format!("That sound file could not be read: {problem}."))?;
    if mp3::is_named_as_mp3(&file.to_string_lossy()) { sound_in_mp3(bytes, sample_rate) } else { sound_in(&bytes, sample_rate) }
}

fn sound_in_mp3(mp3: Vec<u8>, sample_rate: u32) -> Result<Vec<[f32; 2]>, String> {
    match mp3::decoded(mp3, MOST_FRAMES_IN_AN_MP3) {
        Ok(sound) => Ok(at_rate(&sound.frames, sound.sample_rate, sample_rate)),
        Err(Mp3Error::TooLong) => Err(format!("That MP3 is longer than {LONGEST_MP3_IN_MINUTES} minutes, the longest that can be played here.")),
        Err(Mp3Error::Unreadable) => Err("That MP3 file could not be read. Check that it plays on this computer.".to_string()),
    }
}

/// The sound a WAV file holds, as frames of left and right at `sample_rate`.
pub(crate) fn sound_in(wav: &[u8], sample_rate: u32) -> Result<Vec<[f32; 2]>, String> {
    let header = WavHeader::read(wav).map_err(|problem| problem.to_string())?;
    Ok(at_rate(&frames_of(&header, wav), header.sample_rate, sample_rate))
}

/// The sound as the file holds it: every frame as left and right, each between -1 and 1. Mono is put on both sides.
fn frames_of(header: &WavHeader, wav: &[u8]) -> Vec<[f32; 2]> {
    let bytes_in_a_number = usize::from(header.bits / BITS_IN_A_BYTE);
    let numbers: Vec<f32> = header.sound_in(wav).chunks_exact(bytes_in_a_number).map(|written| number_from(written, header.is_floating_point)).collect();
    let channels = usize::from(header.channels);
    numbers.chunks_exact(channels).take(header.frames).map(|frame| [frame[0], frame[channels - 1]]).collect()
}

/// One number of the sound. Whole numbers are written smallest byte first, and 8-bit ones count up from nought
/// instead of either side of it.
fn number_from(written: &[u8], is_floating_point: bool) -> f32 {
    match (written, is_floating_point) {
        (&[a, b, c, d], true) => f32::from_le_bytes([a, b, c, d]),
        (&[only], false) => (f32::from(only) - 128.0) / 128.0,
        (&[a, b], false) => f32::from(i16::from_le_bytes([a, b])) / 32_768.0,
        (&[a, b, c], false) => (i32::from_le_bytes([0, a, b, c]) >> 8) as f32 / 8_388_608.0,
        (&[a, b, c, d], false) => i32::from_le_bytes([a, b, c, d]) as f32 / 2_147_483_648.0,
        _ => 0.0,
    }
}

/// The sound at another rate, each new frame read off the line between the two old frames it falls between.
/// That is plenty for hearing an effect at work.
pub(crate) fn at_rate(frames: &[[f32; 2]], recorded_at: u32, played_at: u32) -> Vec<[f32; 2]> {
    if recorded_at == played_at || frames.is_empty() {
        return frames.to_vec();
    }
    let step = f64::from(recorded_at) / f64::from(played_at);
    let frames_out = (frames.len() as f64 / step).floor() as usize;
    (0..frames_out).map(|place| frame_at(frames, place as f64 * step)).collect()
}

fn frame_at(frames: &[[f32; 2]], place: f64) -> [f32; 2] {
    let before = frames[(place.floor() as usize).min(frames.len() - 1)];
    let after = frames[(place.floor() as usize + 1).min(frames.len() - 1)];
    let along = place.fract() as f32;
    [before[0] + (after[0] - before[0]) * along, before[1] + (after[1] - before[1]) * along]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp3::test_support::{frames_in, silent_mp3};
    use crate::samples::test_support::wav_with_kind;
    use crate::workspace::test_support::TemporaryFolder;

    const WHOLE_NUMBERS: u16 = 1;
    const FLOATING_POINT: u16 = 3;

    /// A WAV file holding the given bytes of sound.
    fn wav_holding(kind: u16, channels: u16, bits: u16, sound: &[u8]) -> Vec<u8> {
        let frames = sound.len() / usize::from(channels * bits / 8);
        let mut wav = wav_with_kind(kind, channels, 48_000, bits, frames as u32);
        let start = wav.len() - sound.len();
        wav[start..].copy_from_slice(sound);
        wav
    }

    #[test]
    fn sixteen_bit_stereo_is_read_as_left_and_right_between_minus_one_and_one() {
        let sound: Vec<u8> = [16_384i16, -16_384, 32_767, -32_768].iter().flat_map(|number| number.to_le_bytes()).collect();

        let frames = sound_in(&wav_holding(WHOLE_NUMBERS, 2, 16, &sound), 48_000).unwrap();

        assert_eq!(frames, [[0.5, -0.5], [32_767.0 / 32_768.0, -1.0]]);
    }

    #[test]
    fn an_mp3_file_is_read_for_playing_and_brought_to_the_speakers_rate() {
        let folder = TemporaryFolder::create("test-sound-mp3");
        let file = folder.0.join("My Loop.MP3");
        fs::write(&file, silent_mp3(2, 10)).unwrap();

        let at_its_own_rate = read_for_playing(&file, 44_100).unwrap();
        let at_twice_the_rate = read_for_playing(&file, 88_200).unwrap();

        assert_eq!(at_its_own_rate.len(), frames_in(10));
        assert_eq!(at_twice_the_rate.len(), 2 * frames_in(10));
    }

    #[test]
    fn a_file_named_as_an_mp3_that_cannot_be_read_as_one_is_said_to_be_so() {
        let folder = TemporaryFolder::create("test-sound-not-mp3");
        let file = folder.0.join("broken.mp3");
        fs::write(&file, b"this is not sound").unwrap();

        assert!(read_for_playing(&file, 44_100).unwrap_err().contains("MP3 file could not be read"));
    }

    #[test]
    fn mono_is_put_on_both_sides() {
        let sound: Vec<u8> = [16_384i16, -16_384].iter().flat_map(|number| number.to_le_bytes()).collect();

        assert_eq!(sound_in(&wav_holding(WHOLE_NUMBERS, 1, 16, &sound), 48_000).unwrap(), [[0.5, 0.5], [-0.5, -0.5]]);
    }

    #[test]
    fn every_size_of_number_a_wav_can_hold_is_read() {
        let eight_bit = wav_holding(WHOLE_NUMBERS, 1, 8, &[192, 0]);
        let twenty_four_bit = wav_holding(WHOLE_NUMBERS, 1, 24, &[0x00, 0x00, 0x40, 0x00, 0x00, 0xC0]);
        let thirty_two_bit = wav_holding(WHOLE_NUMBERS, 1, 32, &(i32::MIN / 2).to_le_bytes());
        let floating: Vec<u8> = [0.25f32, -0.75].iter().flat_map(|number| number.to_le_bytes()).collect();

        assert_eq!(sound_in(&eight_bit, 48_000).unwrap(), [[0.5, 0.5], [-1.0, -1.0]]);
        assert_eq!(sound_in(&twenty_four_bit, 48_000).unwrap(), [[0.5, 0.5], [-0.5, -0.5]]);
        assert_eq!(sound_in(&thirty_two_bit, 48_000).unwrap(), [[-0.5, -0.5]]);
        assert_eq!(sound_in(&wav_holding(FLOATING_POINT, 1, 32, &floating), 48_000).unwrap(), [[0.25, 0.25], [-0.75, -0.75]]);
    }

    #[test]
    fn sound_recorded_at_another_rate_is_brought_to_the_speakers_rate() {
        let sound: Vec<u8> = [0i16, 16_384, 0, -16_384].iter().flat_map(|number| number.to_le_bytes()).collect();

        let twice_as_fast = sound_in(&wav_holding(WHOLE_NUMBERS, 1, 16, &sound), 96_000).unwrap();
        let half_as_fast = sound_in(&wav_holding(WHOLE_NUMBERS, 1, 16, &sound), 24_000).unwrap();

        assert_eq!(twice_as_fast.iter().map(|frame| frame[0]).collect::<Vec<_>>(), [0.0, 0.25, 0.5, 0.25, 0.0, -0.25, -0.5, -0.5]);
        assert_eq!(half_as_fast, [[0.0, 0.0], [0.0, 0.0]]);
    }

    #[test]
    fn a_file_that_is_not_a_wav_is_refused_in_plain_words() {
        let refusal = sound_in(b"ID3 this is an mp3", 48_000).unwrap_err();

        assert!(refusal.contains("it is not a WAV file"), "{refusal}");
    }

    #[test]
    fn a_file_that_has_gone_is_said_to_have_gone() {
        let refusal = read_for_playing(Path::new("/nowhere/at/all.wav"), 48_000).unwrap_err();

        assert!(refusal.contains("no longer where it was"));
    }
}
