//! MP3 files, decoded into plain sound.
//!
//! An MPC plugin reads WAV files only, so an MP3 never goes into a plugin as it is. It is decoded here, and
//! either played as it is to test an effect or written out again as a WAV file for a plugin to carry.

use std::io::Cursor;

use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{CODEC_TYPE_NULL, Decoder, DecoderOptions};
use symphonia::core::errors::Error;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::samples::sixteen_bit_wav;

const MP3_ENDING: &str = ".mp3";
const LOUDEST_SIXTEEN_BIT: f32 = 32_767.0;

/// The sound an MP3 file held: every frame as left and right, each between -1 and 1. Mono is on both sides.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub frames: Vec<[f32; 2]>,
    pub channels: u16,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mp3Error {
    /// It is not an MP3 file, or there is no sound in it that can be read.
    Unreadable,
    /// It holds more sound than was asked for at most.
    TooLong,
}

pub fn is_named_as_mp3(file_name: &str) -> bool {
    let ending_starts_at = file_name.len().saturating_sub(MP3_ENDING.len());
    file_name.is_char_boundary(ending_starts_at) && file_name[ending_starts_at..].eq_ignore_ascii_case(MP3_ENDING)
}

/// Decodes a whole MP3 file. One that turns out to hold more than `most_frames` is given up on, so a small
/// file can never unpack into more sound than there is room for.
pub fn decoded(mp3: Vec<u8>, most_frames: usize) -> Result<Decoded, Mp3Error> {
    let mut reader = reader_of(mp3)?;
    let track = reader.tracks().iter().find(|track| track.codec_params.codec != CODEC_TYPE_NULL).ok_or(Mp3Error::Unreadable)?;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|_| Mp3Error::Unreadable)?;
    let mut sound = Decoded { frames: Vec::new(), channels: 0, sample_rate: 0 };
    while let Some(piece) = next_piece_of(reader.as_mut(), decoder.as_mut()) {
        sound.take_in(piece);
        if sound.frames.len() > most_frames {
            return Err(Mp3Error::TooLong);
        }
    }
    if sound.frames.is_empty() { Err(Mp3Error::Unreadable) } else { Ok(sound) }
}

/// An MP3 file has a little silence added at each end by the way it is made. Where the file says how much,
/// that is left out, so a loop still loops cleanly.
fn reader_of(mp3: Vec<u8>) -> Result<Box<dyn FormatReader>, Mp3Error> {
    let stream = MediaSourceStream::new(Box::new(Cursor::new(mp3)), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let without_the_padding = FormatOptions { enable_gapless: true, ..FormatOptions::default() };
    let found = symphonia::default::get_probe().format(&hint, stream, &without_the_padding, &MetadataOptions::default());
    found.map(|found| found.format).map_err(|_| Mp3Error::Unreadable)
}

/// The next stretch of sound in the file. A damaged stretch is skipped, as a music player would skip it.
/// None when the file has ended.
fn next_piece_of<'decoder>(reader: &mut dyn FormatReader, decoder: &'decoder mut dyn Decoder) -> Option<AudioBufferRef<'decoder>> {
    loop {
        let packet = reader.next_packet().ok()?;
        match decoder.decode(&packet) {
            Err(Error::DecodeError(_)) => continue,
            // The borrow checker cannot see that a skipped stretch has let go of the decoder, so the sound
            // is asked for again here rather than handed on from the line above.
            Ok(_) => return Some(decoder.last_decoded()),
            Err(_) => return None,
        }
    }
}

/// What starts every piece of a 44.1 kHz, 128 kbit MP3 file. The last byte says stereo or mono.
const STEREO_PIECE: [u8; 4] = [0xFF, 0xFB, 0x90, 0x00];
const MONO_PIECE: [u8; 4] = [0xFF, 0xFB, 0x90, 0xC0];
const BYTES_IN_A_PIECE: usize = 417;

/// A real MP3 file of silence, built by hand: a piece with nothing after its start decodes as silence. Each
/// piece is a fortieth of a second or so.
pub fn silent_mp3(channels: u16, pieces: usize) -> Vec<u8> {
    let start = if channels == 1 { MONO_PIECE } else { STEREO_PIECE };
    let mut piece = start.to_vec();
    piece.resize(BYTES_IN_A_PIECE, 0);
    piece.repeat(pieces)
}

impl Decoded {
    fn take_in(&mut self, piece: AudioBufferRef<'_>) {
        let said = *piece.spec();
        let channels = said.channels.count().max(1);
        let mut numbers = SampleBuffer::<f32>::new(piece.capacity() as u64, said);
        numbers.copy_interleaved_ref(piece);
        self.channels = channels.min(2) as u16;
        self.sample_rate = said.rate;
        self.frames.extend(numbers.samples().chunks_exact(channels).map(|frame| [frame[0], frame[channels.min(2) - 1]]));
    }

    /// The sound as a 16 bit WAV file, which is what a plugin on the MPC reads. Mono stays mono.
    pub fn as_wav(&self) -> Vec<u8> {
        let in_sixteen_bits = |number: f32| (number.clamp(-1.0, 1.0) * LOUDEST_SIXTEEN_BIT).round() as i16;
        let sides_kept = usize::from(self.channels);
        let numbers: Vec<i16> = self.frames.iter().flat_map(|frame| &frame[..sides_kept]).copied().map(in_sixteen_bits).collect();
        sixteen_bit_wav(self.channels, self.sample_rate, &numbers)
    }
}

#[cfg(test)]
pub mod test_support {
    pub use super::silent_mp3;

    const FRAMES_IN_A_PIECE: usize = 1_152;

    pub fn frames_in(pieces: usize) -> usize {
        pieces * FRAMES_IN_A_PIECE
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{frames_in, silent_mp3};
    use super::*;
    use crate::samples::WavHeader;

    const PLENTY: usize = 1_000_000;

    #[test]
    fn a_file_is_known_as_an_mp3_by_how_its_name_ends() {
        assert!(is_named_as_mp3("loop.mp3"));
        assert!(is_named_as_mp3("My Loop.MP3"));
        assert!(!is_named_as_mp3("loop.wav"));
        assert!(!is_named_as_mp3("mp3"));
        assert!(!is_named_as_mp3("caf\u{e9}"));
    }

    #[test]
    fn an_mp3_is_decoded_with_its_rate_its_sides_and_all_of_its_sound() {
        let stereo = decoded(silent_mp3(2, 10), PLENTY).unwrap();
        let mono = decoded(silent_mp3(1, 10), PLENTY).unwrap();

        assert_eq!((stereo.channels, stereo.sample_rate), (2, 44_100));
        assert_eq!((mono.channels, mono.sample_rate), (1, 44_100));
        assert_eq!(stereo.frames.len(), frames_in(10));
        assert!(stereo.frames.iter().flatten().all(|number| *number == 0.0));
    }

    #[test]
    fn a_file_that_is_not_an_mp3_is_said_to_be_unreadable() {
        assert_eq!(decoded(b"RIFF....WAVEfmt this is no mp3 at all".to_vec(), PLENTY), Err(Mp3Error::Unreadable));
        assert_eq!(decoded(Vec::new(), PLENTY), Err(Mp3Error::Unreadable));
    }

    #[test]
    fn an_mp3_holding_more_sound_than_there_is_room_for_is_given_up_on() {
        assert_eq!(decoded(silent_mp3(2, 10), frames_in(3)), Err(Mp3Error::TooLong));
    }

    #[test]
    fn decoded_sound_is_written_as_a_sixteen_bit_wav_the_mpc_can_read() {
        let sound = Decoded { frames: vec![[0.5, -0.5], [2.0, -2.0]], channels: 2, sample_rate: 44_100 };

        let wav = sound.as_wav();
        let header = WavHeader::read(&wav).unwrap();
        let numbers: Vec<i16> = header.sound_in(&wav).chunks_exact(2).map(|written| i16::from_le_bytes([written[0], written[1]])).collect();

        assert_eq!((header.channels, header.sample_rate, header.bits, header.frames), (2, 44_100, 16, 2));
        assert_eq!(numbers, [16_384, -16_384, 32_767, -32_767]);
    }

    #[test]
    fn mono_sound_is_written_as_a_mono_wav() {
        let sound = Decoded { frames: vec![[0.25, 0.25]; 3], channels: 1, sample_rate: 22_050 };

        let header = WavHeader::read(&sound.as_wav()).unwrap();

        assert_eq!((header.channels, header.frames), (1, 3));
    }
}
