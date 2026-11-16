//! The archives MP Forge unpacks: each holds one folder, packed as a gzipped tar.
//!
//! A folder unpacked from one is only ever there whole. It is unpacked beside where it belongs and moved into
//! place once every file has arrived, so stopping half way leaves nothing that looks finished.

use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use ring::digest::{Context, SHA256};

const ENDING_WHILE_ARRIVING: &str = ".arriving";

pub(crate) struct Archive<'a> {
    pub file: &'a Path,
    /// The name of the one folder the archive holds.
    pub holds: &'a str,
}

impl Archive<'_> {
    /// The SHA-256 of the archive, written the way such fingerprints are published.
    pub(crate) fn fingerprint(&self) -> io::Result<String> {
        let mut reading = File::open(self.file)?;
        let mut fingerprint = Context::new(&SHA256);
        let mut piece = [0u8; 64 * 1024];
        loop {
            let read = reading.read(&mut piece)?;
            if read == 0 {
                return Ok(fingerprint.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect());
            }
            fingerprint.update(&piece[..read]);
        }
    }

    /// Unpacks the folder the archive holds as `folder`, in place of whatever was there before.
    pub(crate) fn unpack_as(&self, folder: &Path) -> io::Result<()> {
        let arriving = while_arriving(folder);
        remove_if_there(&arriving)?;
        tar::Archive::new(GzDecoder::new(File::open(self.file)?)).unpack(&arriving)?;
        remove_if_there(folder)?;
        fs::rename(arriving.join(self.holds), folder)?;
        remove_if_there(&arriving)
    }
}

/// Where a folder is unpacked until it is whole: beside where it belongs, under a name that says so.
pub(crate) fn while_arriving(folder: &Path) -> PathBuf {
    let mut name = OsString::from(folder.as_os_str());
    name.push(ENDING_WHILE_ARRIVING);
    PathBuf::from(name)
}

fn remove_if_there(folder: &Path) -> io::Result<()> {
    if folder.exists() { fs::remove_dir_all(folder) } else { Ok(()) }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::fs::File;
    use std::path::Path;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    /// Packs an archive holding these files, each named by its path inside the archive.
    pub(crate) fn pack(archive: &Path, files: &[(&Path, &[u8])]) {
        let mut packed = tar::Builder::new(GzEncoder::new(File::create(archive).unwrap(), Compression::fast()));
        for (path, contents) in files {
            let mut heading = tar::Header::new_gnu();
            heading.set_size(contents.len() as u64);
            heading.set_mode(0o755);
            packed.append_data(&mut heading, path, *contents).unwrap();
        }
        packed.into_inner().unwrap().finish().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    #[test]
    fn the_fingerprint_is_the_one_the_standard_gives_for_its_own_example() {
        let folder = TemporaryFolder::create("archive-fingerprint");
        let file = folder.0.join("abc");
        fs::write(&file, "abc").unwrap();

        let fingerprint = Archive { file: &file, holds: "" }.fingerprint().unwrap();

        assert_eq!(fingerprint, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn the_folder_an_archive_holds_takes_the_place_of_the_one_there_before() {
        let folder = TemporaryFolder::create("archive-unpack");
        let file = folder.0.join("packed.tar.gz");
        test_support::pack(&file, &[(Path::new("held/inside/note.txt"), b"arrived")]);
        let unpacked = folder.0.join("unpacked");
        fs::create_dir_all(unpacked.join("left-from-before")).unwrap();

        Archive { file: &file, holds: "held" }.unpack_as(&unpacked).unwrap();

        assert_eq!(fs::read(unpacked.join("inside/note.txt")).unwrap(), b"arrived");
        assert!(!unpacked.join("left-from-before").exists());
        assert!(!while_arriving(&unpacked).exists());
    }

    #[test]
    fn an_archive_without_the_folder_it_should_hold_puts_nothing_in_place() {
        let folder = TemporaryFolder::create("archive-wrong-folder");
        let file = folder.0.join("packed.tar.gz");
        test_support::pack(&file, &[(Path::new("something-else/note.txt"), b"arrived")]);
        let unpacked = folder.0.join("unpacked");

        assert!(Archive { file: &file, holds: "held" }.unpack_as(&unpacked).is_err());
        assert!(!unpacked.exists());
    }
}
