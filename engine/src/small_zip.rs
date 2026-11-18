//! A zip file, written here because the one place MP Forge hands one over needs so little of the format.
//!
//! The Open MPC catalog takes a built plugin as a zip. Every file is squeezed the usual way and carries the same
//! day, so packing the same files twice gives the same zip, byte for byte.

use std::io::{self, Write};

use flate2::write::DeflateEncoder;
use flate2::{Compression, Crc};

const FILE_BEGINS: u32 = 0x0403_4b50;
const FILE_IS_LISTED: u32 = 0x0201_4b50;
const LIST_ENDS: u32 = 0x0605_4b50;
const WRITTEN_FOR_VERSION: u16 = 20;
const NAMES_ARE_UTF8: u16 = 0x0800;
const SQUEEZED: u16 = 8;
const MIDNIGHT: u16 = 0;
/// 1 January 2026, the way a zip writes a day.
const THE_SAME_DAY: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1;
const NOTHING: u16 = 0;

/// A zip being put together in memory. Files are added by the name they have inside it, with `/` between folders.
#[derive(Default)]
pub struct Zip {
    files: Vec<u8>,
    list: Vec<u8>,
    how_many: u16,
}

impl Zip {
    pub fn add(&mut self, name: &str, contents: &[u8]) -> io::Result<()> {
        let file = Squeezed::of(name, contents)?;
        let begins_at = as_a_zip_writes(self.files.len())?;
        self.how_many = self.how_many.checked_add(1).ok_or_else(too_much)?;
        self.files.extend(file.heading());
        self.files.extend(&file.squeezed);
        self.list.extend(file.line_in_the_list(begins_at));
        Ok(())
    }

    /// The finished zip.
    pub fn finish(self) -> io::Result<Vec<u8>> {
        let list_begins_at = as_a_zip_writes(self.files.len())?;
        let list_is_long = as_a_zip_writes(self.list.len())?;
        let mut zip = self.files;
        zip.extend(self.list);
        zip.extend(LIST_ENDS.to_le_bytes());
        zip.extend(shorts(&[NOTHING, NOTHING, self.how_many, self.how_many]));
        zip.extend(longs(&[list_is_long, list_begins_at]));
        zip.extend(NOTHING.to_le_bytes());
        Ok(zip)
    }
}

/// One file as it sits in the zip.
struct Squeezed<'a> {
    name: &'a str,
    squeezed: Vec<u8>,
    check: u32,
    length: u32,
}

impl<'a> Squeezed<'a> {
    fn of(name: &'a str, contents: &[u8]) -> io::Result<Self> {
        let mut squeezer = DeflateEncoder::new(Vec::new(), Compression::default());
        squeezer.write_all(contents)?;
        let mut check = Crc::new();
        check.update(contents);
        u16::try_from(name.len()).map_err(|_| too_much())?;
        Ok(Self { name, squeezed: squeezer.finish()?, check: check.sum(), length: as_a_zip_writes(contents.len())? })
    }

    /// What stands in front of the file itself.
    fn heading(&self) -> Vec<u8> {
        [&FILE_BEGINS.to_le_bytes()[..], &self.described(), self.name.as_bytes()].concat()
    }

    /// The file's line in the list at the end of the zip, which says where the file begins.
    fn line_in_the_list(&self, begins_at: u32) -> Vec<u8> {
        let about_the_file = shorts(&[NOTHING, NOTHING, NOTHING]);
        let where_it_is = longs(&[u32::from(NOTHING), begins_at]);
        let made_by = WRITTEN_FOR_VERSION.to_le_bytes();
        [&FILE_IS_LISTED.to_le_bytes()[..], &made_by, &self.described(), &about_the_file, &where_it_is, self.name.as_bytes()].concat()
    }

    /// What the heading and the line in the list both say about the file.
    fn described(&self) -> Vec<u8> {
        let squeezed_length = self.squeezed.len() as u32;
        let how_it_is_kept = shorts(&[WRITTEN_FOR_VERSION, NAMES_ARE_UTF8, SQUEEZED, MIDNIGHT, THE_SAME_DAY]);
        let sizes = longs(&[self.check, squeezed_length, self.length]);
        let names = shorts(&[self.name.len() as u16, NOTHING]);
        [how_it_is_kept, sizes, names].concat()
    }
}

fn shorts(numbers: &[u16]) -> Vec<u8> {
    numbers.iter().flat_map(|number| number.to_le_bytes()).collect()
}

fn longs(numbers: &[u32]) -> Vec<u8> {
    numbers.iter().flat_map(|number| number.to_le_bytes()).collect()
}

/// A length or a place in the zip, which this kind of zip writes in four bytes.
fn as_a_zip_writes(length: usize) -> io::Result<u32> {
    u32::try_from(length).map_err(|_| too_much())
}

fn too_much() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "That is more than a zip file of this kind can hold.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    fn long_at(zip: &[u8], place: usize) -> u32 {
        u32::from_le_bytes(zip[place..place + 4].try_into().unwrap())
    }

    fn short_at(zip: &[u8], place: usize) -> u16 {
        u16::from_le_bytes(zip[place..place + 2].try_into().unwrap())
    }

    #[test]
    fn a_file_can_be_read_back_out_of_the_zip_from_its_line_in_the_list() {
        let mut zip = Zip::default();
        zip.add("A Folder/first.txt", b"one one one one one one").unwrap();
        zip.add("A Folder/second.txt", b"two").unwrap();
        let zip = zip.finish().unwrap();

        let end = zip.len() - 22;
        let list_begins_at = long_at(&zip, end + 16) as usize;
        let second_line = list_begins_at + 46 + "A Folder/first.txt".len();
        let second_begins_at = long_at(&zip, second_line + 42) as usize;
        let squeezed_length = long_at(&zip, second_begins_at + 18) as usize;
        let name_length = short_at(&zip, second_begins_at + 26) as usize;
        let squeezed_from = second_begins_at + 30 + name_length;
        let mut read_back = String::new();
        DeflateDecoder::new(&zip[squeezed_from..squeezed_from + squeezed_length]).read_to_string(&mut read_back).unwrap();

        assert_eq!(long_at(&zip, end), LIST_ENDS);
        assert_eq!(short_at(&zip, end + 10), 2, "two files are listed");
        assert_eq!(long_at(&zip, second_line), FILE_IS_LISTED);
        assert_eq!(&zip[second_begins_at + 30..squeezed_from], b"A Folder/second.txt");
        assert_eq!(read_back, "two");
    }

    #[test]
    fn the_same_files_give_the_same_zip() {
        let packed = || {
            let mut zip = Zip::default();
            zip.add("plugin.json", b"{}").unwrap();
            zip.finish().unwrap()
        };

        assert_eq!(packed(), packed());
    }
}
