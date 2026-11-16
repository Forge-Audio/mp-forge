//! Sound files the musician adds for a plugin to play: one-shots, loops, wavetables, impulse responses.
//!
//! They are kept in the plugin's `Content/Samples` folder, which travels to the MPC with the plugin.
//! Only WAV files the plugin framework can load are kept, and each one is checked first. An MP3 the musician
//! adds is decoded and kept as the WAV file it comes to.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;
use std::path::{Path, PathBuf};

use crate::mp3::{self, Mp3Error};

const SAMPLES_FOLDER_IN_PLUGIN: &str = "Content/Samples";
/// How plugin code names the samples folder when it asks the framework for a file.
const SAMPLES_FOLDER_IN_CODE: &str = "Samples";
const WAV_ENDING: &str = ".wav";
const LONGEST_FILE_NAME: usize = 64;
const LARGEST_SAMPLE_IN_BYTES: usize = 16 * 1024 * 1024;
const MOST_BYTES_ON_A_SHELF: u64 = 64 * 1024 * 1024;
const MOST_SAMPLES_ON_A_SHELF: usize = 64;

const WHOLE_NUMBER_SOUND: u16 = 1;
const FLOATING_POINT_SOUND: u16 = 3;
/// A wrapper format whose real kind is written further into the header.
const EXTENSIBLE_SOUND: u16 = 0xFFFE;
const BITS_OF_WHOLE_NUMBER_SOUND: [u16; 4] = [8, 16, 24, 32];
const BITS_OF_FLOATING_POINT_SOUND: u16 = 32;
const MOST_CHANNELS: u16 = 2;
const BYTES_IN_A_SIXTEEN_BIT_NUMBER: usize = 2;
const WAV_HEADER_LENGTH: usize = 44;
/// A WAV file opens with `RIFF`, its length and `WAVE`. Its chunks follow.
const WAV_OPENING_LENGTH: u64 = 12;
/// A chunk is labelled with its four-letter name and its length.
const CHUNK_LABEL_LENGTH: u64 = 8;
/// As much of the chunk that says what kind of sound a file holds as is ever looked at.
const MOST_OF_THE_FORMAT_READ: u64 = 40;
/// A real file has a handful of chunks. One made of thousands of empty ones is not worth walking to its end.
const MOST_CHUNKS_LOOKED_AT: usize = 1024;

/// One sound file kept for a plugin, with what its header says about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub file_name: String,
    pub seconds: f32,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Sample {
    /// The path plugin code gives the framework to load this file.
    pub fn path_in_code(&self) -> String {
        format!("{SAMPLES_FOLDER_IN_CODE}/{}", self.file_name)
    }
}

impl fmt::Display for Sample {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let width = if self.channels == 1 { "mono" } else { "stereo" };
        write!(formatter, "{} ({:.1} s, {width}, {} Hz)", self.path_in_code(), self.seconds, self.sample_rate)
    }
}

#[derive(Debug)]
pub enum SampleError {
    UnusableName(String),
    NotASupportedWav(&'static str),
    UnreadableMp3,
    TooLongAsWav,
    TooLarge { bytes: usize },
    ShelfFull,
    Io(io::Error),
}

impl fmt::Display for SampleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SampleError::UnusableName(name) => write!(formatter, "`{name}` cannot be added. Sound files must be WAV or MP3 files, with a name ending in .wav or .mp3."),
            SampleError::UnreadableMp3 => write!(formatter, "That MP3 file could not be read. Check that it plays on this computer, or convert it to WAV first."),
            SampleError::TooLongAsWav => write!(
                formatter,
                "That MP3 is too long. The MPC plays WAV files, and as one it comes to more than the {} MB a plugin's sound file can be.",
                LARGEST_SAMPLE_IN_BYTES / MEGABYTE
            ),
            SampleError::NotASupportedWav(why) => write!(
                formatter,
                "That file cannot be used: {why}. The MPC plays mono or stereo WAV files, 8 to 32 bit, or 32 bit float."
            ),
            SampleError::TooLarge { bytes } => write!(
                formatter,
                "That file is {} MB. The largest sound file a plugin can carry is {} MB.",
                bytes / MEGABYTE,
                LARGEST_SAMPLE_IN_BYTES / MEGABYTE
            ),
            SampleError::ShelfFull => write!(
                formatter,
                "This plugin already carries as much sound as it can: {MOST_SAMPLES_ON_A_SHELF} files or {} MB. Remove one first.",
                MOST_BYTES_ON_A_SHELF / MEGABYTE as u64
            ),
            SampleError::Io(cause) => write!(formatter, "The sound file could not be kept: {cause}"),
        }
    }
}

const MEGABYTE: usize = 1024 * 1024;

impl From<io::Error> for SampleError {
    fn from(cause: io::Error) -> Self {
        SampleError::Io(cause)
    }
}

/// The folder that holds one plugin's sound files.
pub struct SampleShelf {
    folder: PathBuf,
}

impl SampleShelf {
    pub fn of_plugin(plugin_dir: &Path) -> Self {
        Self::at(plugin_dir.join(SAMPLES_FOLDER_IN_PLUGIN))
    }

    pub fn at(folder: impl Into<PathBuf>) -> Self {
        Self { folder: folder.into() }
    }

    /// Checks the file and keeps it. A file already kept under the same name is replaced. An MP3 is kept as
    /// the WAV file it decodes to, under the same name with the other ending, because that is all an MPC reads.
    pub fn add(&self, given_name: &str, bytes: &[u8]) -> Result<Sample, SampleError> {
        if mp3::is_named_as_mp3(given_name) {
            return self.add_wav(&named_as_wav(given_name), &wav_made_from(bytes)?);
        }
        self.add_wav(given_name, bytes)
    }

    fn add_wav(&self, given_name: &str, bytes: &[u8]) -> Result<Sample, SampleError> {
        let file_name = tidy_file_name(given_name)?;
        if bytes.len() > LARGEST_SAMPLE_IN_BYTES {
            return Err(SampleError::TooLarge { bytes: bytes.len() });
        }
        let sound = WavHeader::read(bytes)?;
        self.require_room_for(&file_name, bytes.len())?;
        fs::create_dir_all(&self.folder)?;
        fs::write(self.folder.join(&file_name), bytes)?;
        Ok(sound.describing(file_name))
    }

    pub fn remove(&self, given_name: &str) -> Result<(), SampleError> {
        let file_name = tidy_file_name(given_name)?;
        match fs::remove_file(self.folder.join(file_name)) {
            Err(cause) if cause.kind() != io::ErrorKind::NotFound => Err(cause.into()),
            _ => Ok(()),
        }
    }

    /// Every usable sound file on the shelf, by name.
    pub fn list(&self) -> Vec<Sample> {
        let Ok(entries) = fs::read_dir(&self.folder) else { return Vec::new() };
        let mut samples: Vec<Sample> = entries.filter_map(|entry| sample_at(&entry.ok()?.path())).collect();
        samples.sort_by(|one, other| one.file_name.cmp(&other.file_name));
        samples
    }

    /// Moves every file to another shelf and leaves this one gone.
    pub fn move_all_to(&self, other: &SampleShelf) -> io::Result<()> {
        if !self.folder.is_dir() {
            return Ok(());
        }
        fs::create_dir_all(&other.folder)?;
        for sample in self.list() {
            fs::rename(self.folder.join(&sample.file_name), other.folder.join(&sample.file_name))?;
        }
        fs::remove_dir_all(&self.folder)
    }

    fn require_room_for(&self, file_name: &str, bytes: usize) -> Result<(), SampleError> {
        let others: Vec<PathBuf> = self.list().iter().filter(|kept| kept.file_name != file_name).map(|kept| self.folder.join(&kept.file_name)).collect();
        let bytes_kept: u64 = others.iter().filter_map(|path| fs::metadata(path).ok()).map(|file| file.len()).sum();
        let has_room = others.len() < MOST_SAMPLES_ON_A_SHELF && bytes_kept + bytes as u64 <= MOST_BYTES_ON_A_SHELF;
        if has_room { Ok(()) } else { Err(SampleError::ShelfFull) }
    }
}

fn sample_at(path: &Path) -> Option<Sample> {
    let file_name = path.file_name()?.to_str()?.to_string();
    if tidy_file_name(&file_name).ok()? != file_name {
        return None;
    }
    Some(WavHeader::read_from(&mut File::open(path).ok()?).ok()?.describing(file_name))
}

fn named_as_wav(mp3_name: &str) -> String {
    format!("{}{WAV_ENDING}", &mp3_name[..mp3_name.len() - WAV_ENDING.len()])
}

/// The WAV file an MP3 decodes to. One that would come out larger than a plugin's sound file can be is refused,
/// and the decoding stops as soon as that is known.
fn wav_made_from(mp3_bytes: &[u8]) -> Result<Vec<u8>, SampleError> {
    if mp3_bytes.len() > LARGEST_SAMPLE_IN_BYTES {
        return Err(SampleError::TooLarge { bytes: mp3_bytes.len() });
    }
    let most_frames = LARGEST_SAMPLE_IN_BYTES / BYTES_IN_A_SIXTEEN_BIT_NUMBER;
    let wav = match mp3::decoded(mp3_bytes.to_vec(), most_frames) {
        Ok(sound) => sound.as_wav(),
        Err(Mp3Error::TooLong) => return Err(SampleError::TooLongAsWav),
        Err(Mp3Error::Unreadable) => return Err(SampleError::UnreadableMp3),
    };
    if wav.len() > LARGEST_SAMPLE_IN_BYTES { Err(SampleError::TooLongAsWav) } else { Ok(wav) }
}

/// A WAV file holding these 16 bit numbers, left then right for each frame when there are two channels.
pub(crate) fn sixteen_bit_wav(channels: u16, sample_rate: u32, numbers: &[i16]) -> Vec<u8> {
    let bytes_in_a_frame = channels * BYTES_IN_A_SIXTEEN_BIT_NUMBER as u16;
    let sound_length = (numbers.len() * BYTES_IN_A_SIXTEEN_BIT_NUMBER) as u32;
    let mut wav = Vec::with_capacity(WAV_HEADER_LENGTH + sound_length as usize);
    wav.extend(b"RIFF");
    wav.extend((WAV_HEADER_LENGTH as u32 - 8 + sound_length).to_le_bytes());
    wav.extend(b"WAVEfmt ");
    wav.extend(16u32.to_le_bytes());
    wav.extend(WHOLE_NUMBER_SOUND.to_le_bytes());
    wav.extend(channels.to_le_bytes());
    wav.extend(sample_rate.to_le_bytes());
    wav.extend((sample_rate * u32::from(bytes_in_a_frame)).to_le_bytes());
    wav.extend(bytes_in_a_frame.to_le_bytes());
    wav.extend(16u16.to_le_bytes());
    wav.extend(b"data");
    wav.extend(sound_length.to_le_bytes());
    wav.extend(numbers.iter().flat_map(|number| number.to_le_bytes()));
    wav
}

/// Plugin code names these files in plain text, so a name keeps only letters, digits, spaces, dots, hyphens and underscores.
fn tidy_file_name(given_name: &str) -> Result<String, SampleError> {
    let unusable = || SampleError::UnusableName(given_name.to_string());
    let last_part = given_name.rsplit(['/', '\\']).next().unwrap_or_default().trim();
    let plain = |character: char| character.is_ascii_alphanumeric() || matches!(character, ' ' | '.' | '-' | '_');
    let tidied: String = last_part.chars().map(|character| if plain(character) { character } else { '_' }).collect();
    let stem_length = tidied.len().checked_sub(WAV_ENDING.len()).filter(|length| *length > 0).ok_or_else(unusable)?;
    let is_wav = tidied[stem_length..].eq_ignore_ascii_case(WAV_ENDING);
    let usable = is_wav && !tidied.starts_with('.') && tidied.len() <= LONGEST_FILE_NAME;
    if usable { Ok(tidied) } else { Err(unusable()) }
}

/// What a WAV file says about the sound it holds. Read the same way the plugin framework reads it.
pub(crate) struct WavHeader {
    pub(crate) channels: u16,
    pub(crate) sample_rate: u32,
    pub(crate) frames: usize,
    /// How one number of the sound is written: its size in bits, and whether it is floating point.
    pub(crate) bits: u16,
    pub(crate) is_floating_point: bool,
}

impl WavHeader {
    pub(crate) fn read(bytes: &[u8]) -> Result<Self, SampleError> {
        Self::read_from(&mut Cursor::new(bytes))
    }

    /// Reads what a WAV file says of its sound without reading the sound, so a long file costs no more than a short one.
    pub(crate) fn read_from(wav: &mut (impl Read + Seek)) -> Result<Self, SampleError> {
        let opening = bytes_at::<{ WAV_OPENING_LENGTH as usize }>(wav, 0).unwrap_or_default();
        if &opening[0..4] != b"RIFF" || &opening[8..12] != b"WAVE" {
            return Err(SampleError::NotASupportedWav("it is not a WAV file"));
        }
        let chunks = chunks_of(wav);
        let format = chunk_named(b"fmt ", &chunks).map(|chunk| start_of(chunk, wav)).filter(|body| body.len() >= 16);
        let format = format.ok_or(SampleError::NotASupportedWav("it does not say what kind of sound it holds"))?;
        let sound = chunk_named(b"data", &chunks).ok_or(SampleError::NotASupportedWav("it holds no sound"))?;
        Self::of(&format, sound.length())
    }

    fn of(format: &[u8], bytes_of_sound: usize) -> Result<Self, SampleError> {
        let (channels, sample_rate, bits) = (u16_at(format, 2), u32_at(format, 4), u16_at(format, 14));
        if !is_supported_kind(kind_of_sound(format), bits) {
            return Err(SampleError::NotASupportedWav("it is compressed, or uses a bit depth the MPC cannot read"));
        }
        if channels == 0 || channels > MOST_CHANNELS || sample_rate == 0 {
            return Err(SampleError::NotASupportedWav("it must be mono or stereo"));
        }
        let frames = bytes_of_sound / (usize::from(bits / 8) * usize::from(channels));
        let is_floating_point = kind_of_sound(format) == FLOATING_POINT_SOUND;
        if frames == 0 { Err(SampleError::NotASupportedWav("it holds no sound")) } else { Ok(Self { channels, sample_rate, frames, bits, is_floating_point }) }
    }

    /// The bytes of the sound itself, in a file this header was read from.
    pub(crate) fn sound_in<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        let chunks = chunks_of(&mut Cursor::new(bytes));
        chunk_named(b"data", &chunks).map(|chunk| &bytes[chunk.body.start as usize..chunk.body.end as usize]).unwrap_or_default()
    }

    fn describing(&self, file_name: String) -> Sample {
        Sample { file_name, seconds: self.frames as f32 / self.sample_rate as f32, channels: self.channels, sample_rate: self.sample_rate }
    }
}

fn kind_of_sound(format: &[u8]) -> u16 {
    let stated = u16_at(format, 0);
    if stated == EXTENSIBLE_SOUND && format.len() >= 26 { u16_at(format, 24) } else { stated }
}

fn is_supported_kind(kind: u16, bits: u16) -> bool {
    match kind {
        WHOLE_NUMBER_SOUND => BITS_OF_WHOLE_NUMBER_SOUND.contains(&bits),
        FLOATING_POINT_SOUND => bits == BITS_OF_FLOATING_POINT_SOUND,
        _ => false,
    }
}

/// One named chunk of a WAV file, and where its bytes lie in the file.
struct Chunk {
    name: [u8; 4],
    body: Range<u64>,
}

impl Chunk {
    fn length(&self) -> usize {
        (self.body.end - self.body.start) as usize
    }
}

/// A WAV file is a run of named chunks, each one a four-letter name, a length, then that many bytes. Only the
/// names and lengths are read. A chunk that says it runs past the end of the file is taken to end there.
fn chunks_of(wav: &mut (impl Read + Seek)) -> Vec<Chunk> {
    let file_length = wav.seek(SeekFrom::End(0)).unwrap_or(0);
    let mut chunks = Vec::new();
    let mut position = WAV_OPENING_LENGTH;
    while position + CHUNK_LABEL_LENGTH <= file_length && chunks.len() < MOST_CHUNKS_LOOKED_AT {
        let Some(label) = bytes_at::<{ CHUNK_LABEL_LENGTH as usize }>(wav, position) else { break };
        let stated_length = u64::from(u32_at(&label, 4));
        let body_start = position + CHUNK_LABEL_LENGTH;
        let body = body_start..(body_start + stated_length).min(file_length);
        position = body.end + stated_length % 2;
        chunks.push(Chunk { name: [label[0], label[1], label[2], label[3]], body });
    }
    chunks
}

fn chunk_named<'chunks>(name: &[u8; 4], chunks: &'chunks [Chunk]) -> Option<&'chunks Chunk> {
    chunks.iter().find(|chunk| &chunk.name == name)
}

/// The first bytes of a chunk, as many as are ever looked at. Empty when they cannot be read.
fn start_of(chunk: &Chunk, wav: &mut (impl Read + Seek)) -> Vec<u8> {
    let mut start = Vec::new();
    let wanted = (chunk.length() as u64).min(MOST_OF_THE_FORMAT_READ);
    let read = wav.seek(SeekFrom::Start(chunk.body.start)).and_then(|_| wav.by_ref().take(wanted).read_to_end(&mut start));
    if read.is_ok() { start } else { Vec::new() }
}

fn bytes_at<const LENGTH: usize>(wav: &mut (impl Read + Seek), position: u64) -> Option<[u8; LENGTH]> {
    let mut bytes = [0; LENGTH];
    wav.seek(SeekFrom::Start(position)).ok()?;
    wav.read_exact(&mut bytes).ok()?;
    Some(bytes)
}

fn u16_at(bytes: &[u8], position: usize) -> u16 {
    u16::from_le_bytes([bytes[position], bytes[position + 1]])
}

fn u32_at(bytes: &[u8], position: usize) -> u32 {
    u32::from_le_bytes([bytes[position], bytes[position + 1], bytes[position + 2], bytes[position + 3]])
}

#[cfg(test)]
pub mod test_support {
    /// A short WAV file of silence, built by hand.
    pub fn wav_of(channels: u16, sample_rate: u32, bits: u16, frames: u32) -> Vec<u8> {
        wav_with_kind(1, channels, sample_rate, bits, frames)
    }

    pub fn wav_with_kind(kind: u16, channels: u16, sample_rate: u32, bits: u16, frames: u32) -> Vec<u8> {
        let bytes_per_frame = u32::from(channels) * u32::from(bits / 8);
        let sound_length = frames * bytes_per_frame;
        let mut wav = Vec::new();
        wav.extend(b"RIFF");
        wav.extend((36 + sound_length).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(kind.to_le_bytes());
        wav.extend(channels.to_le_bytes());
        wav.extend(sample_rate.to_le_bytes());
        wav.extend((sample_rate * bytes_per_frame).to_le_bytes());
        wav.extend((bytes_per_frame as u16).to_le_bytes());
        wav.extend(bits.to_le_bytes());
        wav.extend(b"data");
        wav.extend(sound_length.to_le_bytes());
        wav.resize(wav.len() + sound_length as usize, 0);
        wav
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{wav_of, wav_with_kind};
    use super::*;
    use crate::mp3::test_support::{frames_in, silent_mp3};
    use crate::workspace::test_support::TemporaryFolder;

    const MP3_KIND: u16 = 0x55;

    fn empty_shelf() -> (TemporaryFolder, SampleShelf) {
        let folder = TemporaryFolder::create("samples");
        let shelf = SampleShelf::of_plugin(&folder.0);
        (folder, shelf)
    }

    #[test]
    fn an_mp3_is_kept_as_the_wav_file_it_decodes_to() {
        let (folder, shelf) = empty_shelf();

        let kept = shelf.add("My Loop.MP3", &silent_mp3(2, 10)).unwrap();

        assert_eq!(kept.file_name, "My Loop.wav");
        assert_eq!((kept.channels, kept.sample_rate), (2, 44_100));
        assert_eq!(kept.seconds, frames_in(10) as f32 / 44_100.0);
        assert!(WavHeader::read(&fs::read(folder.0.join("Content/Samples/My Loop.wav")).unwrap()).is_ok());
        assert!(!folder.0.join("Content/Samples/My Loop.MP3").exists());
    }

    #[test]
    fn a_file_named_as_an_mp3_that_is_not_one_is_refused_and_nothing_is_kept() {
        let (_folder, shelf) = empty_shelf();

        let refused = shelf.add("loop.mp3", &wav_of(2, 44_100, 16, 100)).unwrap_err();

        assert!(matches!(refused, SampleError::UnreadableMp3));
        assert!(shelf.list().is_empty());
    }

    #[test]
    fn an_mp3_that_would_be_too_large_as_a_wav_is_refused() {
        let (_folder, shelf) = empty_shelf();
        let longer_than_fits = LARGEST_SAMPLE_IN_BYTES / (2 * BYTES_IN_A_SIXTEEN_BIT_NUMBER) / frames_in(1) + 2;

        let refused = shelf.add("long.mp3", &silent_mp3(2, longer_than_fits)).unwrap_err();

        assert!(matches!(refused, SampleError::TooLongAsWav), "{refused}");
    }

    #[test]
    fn a_wav_file_is_kept_in_the_plugins_content_and_described() {
        let (folder, shelf) = empty_shelf();

        let kept = shelf.add("kick.wav", &wav_of(2, 44_100, 16, 22_050)).unwrap();

        assert_eq!(kept, Sample { file_name: "kick.wav".to_string(), seconds: 0.5, channels: 2, sample_rate: 44_100 });
        assert_eq!(kept.to_string(), "Samples/kick.wav (0.5 s, stereo, 44100 Hz)");
        assert!(folder.0.join("Content/Samples/kick.wav").is_file());
        assert_eq!(shelf.list(), [kept]);
    }

    /// A file that counts how many of its bytes were read.
    struct Counted<Wav> {
        wav: Wav,
        bytes_read: usize,
    }

    impl<Wav: Read> Read for Counted<Wav> {
        fn read(&mut self, into: &mut [u8]) -> io::Result<usize> {
            let read = self.wav.read(into)?;
            self.bytes_read += read;
            Ok(read)
        }
    }

    impl<Wav: Seek> Seek for Counted<Wav> {
        fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
            self.wav.seek(to)
        }
    }

    #[test]
    fn a_long_sound_is_described_from_its_first_few_bytes_without_reading_the_sound() {
        let ten_seconds = wav_of(2, 44_100, 16, 441_000);
        let mut counted = Counted { wav: Cursor::new(&ten_seconds), bytes_read: 0 };

        let header = WavHeader::read_from(&mut counted).unwrap();

        assert_eq!((header.channels, header.sample_rate, header.frames), (2, 44_100, 441_000));
        assert!(counted.bytes_read < 100, "{} bytes were read", counted.bytes_read);
    }

    #[test]
    fn a_sound_that_says_it_is_longer_than_its_file_is_taken_to_end_with_the_file() {
        let mut cut_short = wav_of(1, 44_100, 16, 1_000);
        cut_short.truncate(WAV_HEADER_LENGTH + 500);

        let header = WavHeader::read(&cut_short).unwrap();

        assert_eq!(header.frames, 250);
        assert_eq!(header.sound_in(&cut_short).len(), 500);
    }

    #[test]
    fn a_name_is_stripped_of_its_folders_and_of_anything_code_could_trip_on() {
        let (_folder, shelf) = empty_shelf();

        let kept = shelf.add("../../My \"Best\" Kick (1).WAV", &wav_of(1, 48_000, 24, 10)).unwrap();

        assert_eq!(kept.file_name, "My _Best_ Kick _1_.WAV");
    }

    #[test]
    fn files_that_are_not_sound_the_mpc_can_be_given_are_refused() {
        let (_folder, shelf) = empty_shelf();
        let sound = wav_of(1, 44_100, 16, 10);

        assert!(matches!(shelf.add("loop.aiff", &sound), Err(SampleError::UnusableName(_))));
        assert!(matches!(shelf.add(".wav", &sound), Err(SampleError::UnusableName(_))));
        assert!(matches!(shelf.add(".mp3", &silent_mp3(2, 2)), Err(SampleError::UnusableName(_))));
        assert!(matches!(shelf.add("notes.wav", b"these are just words"), Err(SampleError::NotASupportedWav(_))));
        assert!(shelf.list().is_empty());
    }

    #[test]
    fn sound_the_mpc_cannot_read_is_refused() {
        let (_folder, shelf) = empty_shelf();

        let compressed = shelf.add("a.wav", &wav_with_kind(MP3_KIND, 2, 44_100, 16, 10));
        let surround = shelf.add("b.wav", &wav_of(6, 48_000, 16, 10));
        let silent = shelf.add("c.wav", &wav_of(1, 44_100, 16, 0));

        assert!(matches!(compressed, Err(SampleError::NotASupportedWav(_))));
        assert!(matches!(surround, Err(SampleError::NotASupportedWav(_))));
        assert!(matches!(silent, Err(SampleError::NotASupportedWav(_))));
    }

    #[test]
    fn float_sound_is_accepted_at_32_bits() {
        let (_folder, shelf) = empty_shelf();

        assert!(shelf.add("pad.wav", &wav_with_kind(FLOATING_POINT_SOUND, 2, 44_100, 32, 10)).is_ok());
    }

    #[test]
    fn a_file_over_the_size_limit_is_refused_before_it_is_read() {
        let (_folder, shelf) = empty_shelf();

        let refused = shelf.add("huge.wav", &vec![0; LARGEST_SAMPLE_IN_BYTES + 1]);

        assert!(matches!(refused, Err(SampleError::TooLarge { .. })));
    }

    #[test]
    fn a_removed_file_is_gone_and_removing_it_twice_is_harmless() {
        let (_folder, shelf) = empty_shelf();
        shelf.add("kick.wav", &wav_of(1, 44_100, 16, 10)).unwrap();

        shelf.remove("kick.wav").unwrap();
        shelf.remove("kick.wav").unwrap();

        assert!(shelf.list().is_empty());
    }

    #[test]
    fn waiting_files_move_onto_the_plugins_shelf() {
        let (folder, plugin_shelf) = empty_shelf();
        let waiting = SampleShelf::at(folder.0.join("waiting"));
        waiting.add("snare.wav", &wav_of(1, 44_100, 16, 10)).unwrap();

        waiting.move_all_to(&plugin_shelf).unwrap();

        assert_eq!(plugin_shelf.list()[0].file_name, "snare.wav");
        assert!(!folder.0.join("waiting").exists());
    }
}
