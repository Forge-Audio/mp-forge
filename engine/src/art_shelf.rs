//! Pictures the musician brought into the editor to use as artwork on a plugin's screen.
//!
//! They are kept as PNG files in the plugin's `artwork` folder, each under a name the shelf gives out.
//! The AI can neither read nor write there. A picture is never changed once kept: painting over one or
//! undoing a change keeps another, and those no look uses any more are cleared away between visits.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use resvg::tiny_skia::Pixmap;

use crate::layers::LONGEST_SIDE;

pub const ART_FOLDER: &str = "artwork";
const FILE_NAME_START: &str = "art-";
const FILE_NAME_END: &str = ".png";
const LARGEST_PICTURE_IN_BYTES: usize = 12 * 1024 * 1024;
const MOST_PICTURES_KEPT: usize = 400;

#[derive(Debug)]
pub enum ArtError {
    NotAPicture,
    TooLarge,
    ShelfIsFull,
    Io(io::Error),
}

impl fmt::Display for ArtError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtError::NotAPicture => write!(formatter, "That is not a picture MP Forge can read."),
            ArtError::TooLarge => write!(formatter, "That picture is too large to use on the MPC's screen."),
            ArtError::ShelfIsFull => write!(formatter, "This plugin holds as many pictures as it can. Press DONE, then open the editor again to make room."),
            ArtError::Io(cause) => write!(formatter, "The picture could not be kept: {cause}"),
        }
    }
}

impl From<io::Error> for ArtError {
    fn from(cause: io::Error) -> Self {
        ArtError::Io(cause)
    }
}

pub struct ArtShelf {
    folder: PathBuf,
}

impl ArtShelf {
    pub fn of_plugin(plugin_dir: &Path) -> Self {
        Self { folder: plugin_dir.join(ART_FOLDER) }
    }

    /// Checks the picture and keeps it under the next free number. Returns the file name it was given.
    pub fn add(&self, png: &[u8]) -> Result<String, ArtError> {
        require_a_picture_that_fits(png)?;
        let numbers = self.numbers();
        if numbers.len() >= MOST_PICTURES_KEPT {
            return Err(ArtError::ShelfIsFull);
        }
        let file_name = file_name_numbered(numbers.last().copied().unwrap_or(0) + 1);
        fs::create_dir_all(&self.folder)?;
        fs::write(self.folder.join(&file_name), png)?;
        Ok(file_name)
    }

    /// The image file of one picture on the shelf. A name the shelf did not give out gets nothing.
    pub fn read(&self, file_name: &str) -> Option<Vec<u8>> {
        if is_art_file_name(file_name) { fs::read(self.folder.join(file_name)).ok() } else { None }
    }

    /// The picture ready to draw, or nothing when the shelf has no such picture.
    pub fn decoded(&self, file_name: &str) -> Option<Pixmap> {
        self.read(file_name).and_then(|png| Pixmap::decode_png(&png).ok())
    }

    pub fn list(&self) -> Vec<String> {
        self.numbers().into_iter().map(file_name_numbered).collect()
    }

    /// Clears away every picture that is not among those still wanted.
    pub fn keep_only(&self, wanted: &[&str]) -> io::Result<()> {
        let unwanted = self.list().into_iter().filter(|file_name| !wanted.contains(&file_name.as_str()));
        unwanted.into_iter().try_for_each(|file_name| fs::remove_file(self.folder.join(file_name)))
    }

    /// The numbers of the pictures on the shelf, lowest first.
    fn numbers(&self) -> Vec<u32> {
        let Ok(entries) = fs::read_dir(&self.folder) else { return Vec::new() };
        let mut numbers: Vec<u32> = entries.filter_map(|entry| number_in(&entry.ok()?.file_name().to_string_lossy())).collect();
        numbers.sort_unstable();
        numbers
    }
}

pub fn is_art_file_name(file_name: &str) -> bool {
    number_in(file_name).is_some()
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

fn require_a_picture_that_fits(png: &[u8]) -> Result<(), ArtError> {
    if png.len() > LARGEST_PICTURE_IN_BYTES {
        return Err(ArtError::TooLarge);
    }
    let picture = Pixmap::decode_png(png).map_err(|_| ArtError::NotAPicture)?;
    if picture.width().max(picture.height()) > LONGEST_SIDE { Err(ArtError::TooLarge) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachments::test_support::png_of;
    use crate::workspace::test_support::TemporaryFolder;

    #[test]
    fn each_picture_is_kept_under_the_next_number_and_read_back_by_it() {
        let plugin = TemporaryFolder::create("art-shelf");
        let shelf = ArtShelf::of_plugin(&plugin.0);

        let first = shelf.add(&png_of(40, 30)).unwrap();
        let second = shelf.add(&png_of(8, 6)).unwrap();

        assert_eq!((first.as_str(), second.as_str()), ("art-1.png", "art-2.png"));
        assert!(plugin.0.join("artwork/art-2.png").is_file());
        assert_eq!(shelf.decoded("art-2.png").map(|picture| (picture.width(), picture.height())), Some((8, 6)));
        assert!(shelf.read("../plugin.json").is_none() && shelf.read("art-9.png").is_none());
    }

    #[test]
    fn something_that_is_not_a_picture_that_fits_is_refused() {
        let plugin = TemporaryFolder::create("art-shelf-refused");
        let shelf = ArtShelf::of_plugin(&plugin.0);

        assert!(matches!(shelf.add(b"<svg/>"), Err(ArtError::NotAPicture)));
        assert!(matches!(shelf.add(&png_of(LONGEST_SIDE + 1, 4)), Err(ArtError::TooLarge)));
        assert!(shelf.list().is_empty());
    }

    #[test]
    fn pictures_no_longer_wanted_are_cleared_and_their_numbers_are_not_reused_while_later_ones_stay() {
        let plugin = TemporaryFolder::create("art-shelf-cleared");
        let shelf = ArtShelf::of_plugin(&plugin.0);
        for _ in 0..3 {
            shelf.add(&png_of(4, 4)).unwrap();
        }

        shelf.keep_only(&["art-3.png"]).unwrap();

        assert_eq!(shelf.list(), ["art-3.png"]);
        assert_eq!(shelf.add(&png_of(4, 4)).unwrap(), "art-4.png");
    }

    #[test]
    fn only_names_the_shelf_gave_out_count_as_its_pictures() {
        assert!(is_art_file_name("art-12.png"));
        assert!(!is_art_file_name("art-012.png") && !is_art_file_name("art-.png") && !is_art_file_name("picture-1.png") && !is_art_file_name("art-1.png/.."));
    }
}
