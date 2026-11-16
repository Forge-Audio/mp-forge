//! Pictures the musician shows the AI: screenshots, sketches, photos of gear.
//!
//! They are kept as PNG files in the plugin's `attachments` folder. The AI can look at them but never
//! write there, and nothing in the folder is built into the plugin.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::Pixmap;

pub const ATTACHMENTS_FOLDER: &str = "attachments";
const FILE_NAME_START: &str = "picture-";
const FILE_NAME_END: &str = ".png";
const LARGEST_PICTURE_IN_BYTES: usize = 4 * 1024 * 1024;
/// The screen shrinks a picture to this before handing it over, so anything longer did not come from the screen.
const LONGEST_SIDE: u32 = 1600;
/// Older pictures make way for new ones, so the folder never grows without end.
const MOST_PICTURES_KEPT: usize = 24;

#[derive(Debug)]
pub enum AttachmentError {
    NotAPicture,
    TooLarge,
    Io(io::Error),
}

impl fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AttachmentError::NotAPicture => write!(formatter, "That is not a picture MP Forge can read."),
            AttachmentError::TooLarge => write!(formatter, "That picture is too large to show to the AI."),
            AttachmentError::Io(cause) => write!(formatter, "The picture could not be kept: {cause}"),
        }
    }
}

impl From<io::Error> for AttachmentError {
    fn from(cause: io::Error) -> Self {
        AttachmentError::Io(cause)
    }
}

/// The folder that holds the pictures shown for one plugin.
pub struct AttachmentShelf {
    folder: PathBuf,
}

impl AttachmentShelf {
    pub fn of_plugin(plugin_dir: &Path) -> Self {
        Self::at(plugin_dir.join(ATTACHMENTS_FOLDER))
    }

    pub fn at(folder: impl Into<PathBuf>) -> Self {
        Self { folder: folder.into() }
    }

    /// Checks the picture and keeps it under the next free number. Returns the file name it was given.
    pub fn add(&self, png: &[u8]) -> Result<String, AttachmentError> {
        require_a_modest_picture(png)?;
        let file_name = file_name_numbered(self.highest_number() + 1);
        fs::create_dir_all(&self.folder)?;
        fs::write(self.folder.join(&file_name), png)?;
        self.remove_the_oldest_beyond(MOST_PICTURES_KEPT)?;
        Ok(file_name)
    }

    /// The pictures on the shelf, oldest first.
    pub fn list(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.folder) else { return Vec::new() };
        let mut numbers: Vec<u32> = entries.filter_map(|entry| number_in(&entry.ok()?.file_name().to_string_lossy())).collect();
        numbers.sort_unstable();
        numbers.into_iter().map(file_name_numbered).collect()
    }

    /// The image file of one picture on the shelf. A name the shelf did not give out gets nothing.
    pub fn read(&self, file_name: &str) -> Option<Vec<u8>> {
        if is_attachment_name(file_name) { fs::read(self.folder.join(file_name)).ok() } else { None }
    }

    /// Moves every picture to another shelf, keeping its name, and leaves this one gone.
    pub fn move_all_to(&self, other: &AttachmentShelf) -> io::Result<()> {
        if !self.folder.is_dir() {
            return Ok(());
        }
        fs::create_dir_all(&other.folder)?;
        for file_name in self.list() {
            fs::rename(self.folder.join(&file_name), other.folder.join(&file_name))?;
        }
        fs::remove_dir_all(&self.folder)
    }

    fn highest_number(&self) -> u32 {
        self.list().last().and_then(|file_name| number_in(file_name)).unwrap_or(0)
    }

    fn remove_the_oldest_beyond(&self, most_kept: usize) -> io::Result<()> {
        let kept = self.list();
        let too_many = kept.len().saturating_sub(most_kept);
        kept.iter().take(too_many).try_for_each(|file_name| fs::remove_file(self.folder.join(file_name)))
    }
}

/// Whether a path inside the plugin names one of the musician's pictures.
pub fn is_attachment(relative: &Path) -> bool {
    let file_name = relative.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    relative.parent() == Some(Path::new(ATTACHMENTS_FOLDER)) && is_attachment_name(file_name)
}

pub fn is_attachment_name(file_name: &str) -> bool {
    number_in(file_name).is_some()
}

/// Tells the AI which pictures came with the musician's message. Says nothing when there are none.
pub fn note_about(file_names: &[String]) -> String {
    let listed: Vec<String> = file_names.iter().filter(|name| is_attachment_name(name)).map(|name| format!("- {ATTACHMENTS_FOLDER}/{name}")).collect();
    if listed.is_empty() {
        return String::new();
    }
    format!(
        "\n\nThe musician attached pictures to this message. Look at each one with read_plugin_file before you do anything else:\n\n{}",
        listed.join("\n")
    )
}

fn file_name_numbered(number: u32) -> String {
    format!("{FILE_NAME_START}{number}{FILE_NAME_END}")
}

/// The number in a name the shelf gave out. Any other name is not one of its pictures.
fn number_in(file_name: &str) -> Option<u32> {
    let digits = file_name.strip_prefix(FILE_NAME_START)?.strip_suffix(FILE_NAME_END)?;
    let written_plainly = !digits.is_empty() && !digits.starts_with('0') && digits.bytes().all(|byte| byte.is_ascii_digit());
    if written_plainly { digits.parse().ok() } else { None }
}

fn require_a_modest_picture(png: &[u8]) -> Result<(), AttachmentError> {
    if png.len() > LARGEST_PICTURE_IN_BYTES {
        return Err(AttachmentError::TooLarge);
    }
    let picture = Pixmap::decode_png(png).map_err(|_| AttachmentError::NotAPicture)?;
    if picture.width().max(picture.height()) > LONGEST_SIDE { Err(AttachmentError::TooLarge) } else { Ok(()) }
}

#[cfg(test)]
pub mod test_support {
    use resvg::tiny_skia::{Color, Pixmap};

    /// A plain grey PNG of the size asked for.
    pub fn png_of(width: u32, height: u32) -> Vec<u8> {
        let mut picture = Pixmap::new(width, height).unwrap();
        picture.fill(Color::from_rgba8(90, 90, 90, 255));
        picture.encode_png().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::png_of;
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    fn shelf_in(folder: &TemporaryFolder) -> AttachmentShelf {
        AttachmentShelf::of_plugin(&folder.0)
    }

    #[test]
    fn each_picture_is_kept_under_the_next_number() {
        let folder = TemporaryFolder::create("attachments");
        let shelf = shelf_in(&folder);

        let first = shelf.add(&png_of(40, 30)).unwrap();
        let second = shelf.add(&png_of(40, 30)).unwrap();

        assert_eq!([first.as_str(), second.as_str()], ["picture-1.png", "picture-2.png"]);
        assert_eq!(shelf.list(), ["picture-1.png", "picture-2.png"]);
        assert!(shelf.read("picture-2.png").is_some() && shelf.read("../plugin.json").is_none());
        assert!(folder.0.join("attachments/picture-2.png").is_file());
    }

    #[test]
    fn something_that_is_not_a_modest_png_is_refused_and_nothing_is_kept() {
        let folder = TemporaryFolder::create("attachments-refused");
        let shelf = shelf_in(&folder);

        assert!(matches!(shelf.add(b"#!/bin/sh\nrm -rf /"), Err(AttachmentError::NotAPicture)));
        assert!(matches!(shelf.add(&png_of(LONGEST_SIDE + 1, 10)), Err(AttachmentError::TooLarge)));
        assert!(shelf.list().is_empty());
    }

    #[test]
    fn the_oldest_pictures_make_way_once_the_shelf_is_full() {
        let folder = TemporaryFolder::create("attachments-full");
        let shelf = shelf_in(&folder);
        let small = png_of(4, 4);

        for _ in 0..MOST_PICTURES_KEPT + 2 {
            shelf.add(&small).unwrap();
        }

        let kept = shelf.list();
        assert_eq!(kept.len(), MOST_PICTURES_KEPT);
        assert_eq!(kept.first().map(String::as_str), Some("picture-3.png"));
        assert_eq!(kept.last().map(String::as_str), Some("picture-26.png"));
    }

    #[test]
    fn pictures_waiting_for_a_new_plugin_move_in_under_the_same_names() {
        let waiting_folder = TemporaryFolder::create("attachments-waiting");
        let plugin = TemporaryFolder::create("attachments-plugin");
        let waiting = AttachmentShelf::at(waiting_folder.0.join("Dusty Tape"));
        waiting.add(&png_of(4, 4)).unwrap();

        waiting.move_all_to(&shelf_in(&plugin)).unwrap();

        assert_eq!(shelf_in(&plugin).list(), ["picture-1.png"]);
        assert!(waiting.list().is_empty());
    }

    #[test]
    fn only_names_the_shelf_gave_out_count_as_its_pictures() {
        assert!(is_attachment(Path::new("attachments/picture-12.png")));
        assert!(!is_attachment(Path::new("attachments/picture-012.png")));
        assert!(!is_attachment(Path::new("attachments/notes.txt")));
        assert!(!is_attachment(Path::new("attachments/deeper/picture-1.png")));
        assert!(!is_attachment(Path::new("src/picture-1.png")));
    }

    #[test]
    fn the_note_lists_the_pictures_and_says_nothing_when_there_are_none() {
        let note = note_about(&["picture-3.png".to_string(), "../plugin.json".to_string()]);

        assert!(note.contains("- attachments/picture-3.png"));
        assert!(!note.contains("plugin.json"));
        assert_eq!(note_about(&[]), "");
    }
}
