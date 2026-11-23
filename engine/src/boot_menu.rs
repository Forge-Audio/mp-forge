//! The boot menu SampleBoot, as far as MP Forge has to know it: where on a card it looks for what it can
//! start, and what it reads in a folder there.
//!
//! The musician puts the boot menu on their card themselves. MP Forge only ever writes a folder of its own
//! where the boot menu looks. The files at the top of `autorun` that start the boot menu are SampleBoot's
//! own: none of them is ever written here, and neither is a script called `autorun.sh`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

/// Where on a card the boot menu looks for what it can start, from the card's top folder down.
pub const WHERE_THE_BOOT_MENU_LOOKS: [&str; 2] = ["autorun", "mods"];
/// The boot menu leaves a folder out of its list when its name starts with this.
pub const HIDES_A_FOLDER_FROM_THE_MENU: char = '_';

/// The file the boot menu reads to learn which script starts what is in a folder.
const NAMES_THE_START_SCRIPT_FILE: &str = "autorun.json";
/// The file the boot menu reads the name it shows from.
const NAME_IN_THE_MENU_FILE: &str = "sampleboot.txt";

/// A folder as it was written for a card, and whether it lies where the boot menu will find it.
#[derive(Debug, PartialEq, Eq)]
pub struct MadeForTheCard {
    pub folder: PathBuf,
    pub is_where_the_boot_menu_looks: bool,
}

/// Where a folder for the boot menu goes, in what the musician picked.
#[derive(Debug, PartialEq, Eq)]
pub struct PlaceForAFolder {
    pub into: PathBuf,
    pub is_where_the_boot_menu_looks: bool,
}

impl PlaceForAFolder {
    /// On a card that has the boot menu, where the boot menu looks, whether the musician picked the card
    /// itself or that place on it. In any other folder, the folder as it is, to be copied onto a card later.
    pub fn in_what_was_picked(picked: &Path) -> PlaceForAFolder {
        let on_the_card = where_the_boot_menu_looks_in(picked);
        PlaceForAFolder { is_where_the_boot_menu_looks: on_the_card.is_some(), into: on_the_card.unwrap_or_else(|| picked.to_path_buf()) }
    }

    pub fn holding(&self, folder: PathBuf) -> MadeForTheCard {
        MadeForTheCard { folder, is_where_the_boot_menu_looks: self.is_where_the_boot_menu_looks }
    }
}

fn where_the_boot_menu_looks_in(picked: &Path) -> Option<PathBuf> {
    let further_in: PathBuf = WHERE_THE_BOOT_MENU_LOOKS.iter().collect();
    let on_the_card_picked = picked.join(&further_in);
    if on_the_card_picked.is_dir() {
        return Some(on_the_card_picked);
    }
    picked.ends_with(&further_in).then(|| picked.to_path_buf())
}

/// What the boot menu reads in a folder: the name it shows, and the script it starts.
pub struct ShownInTheMenu<'a> {
    pub name: &'a str,
    pub start_script_file: &'a str,
    pub start_script: &'a str,
}

impl ShownInTheMenu<'_> {
    /// Writes the start script and the two files the boot menu reads into `folder`. The script is written
    /// with lines that end the way the device's shell expects, whichever computer MP Forge was built on.
    pub fn write_into(&self, folder: &Path) -> io::Result<()> {
        fs::write(folder.join(self.start_script_file), self.start_script.replace("\r\n", "\n"))?;
        fs::write(folder.join(NAMES_THE_START_SCRIPT_FILE), json!({ "scripts": [{ "path": self.start_script_file }] }).to_string())?;
        fs::write(folder.join(NAME_IN_THE_MENU_FILE), self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    #[test]
    fn on_a_card_with_the_boot_menu_a_folder_goes_where_the_boot_menu_looks() {
        let card = TemporaryFolder::create("card-with-the-boot-menu");
        let its_place = card.0.join("autorun").join("mods");
        fs::create_dir_all(&its_place).unwrap();

        let found_from_the_card = PlaceForAFolder::in_what_was_picked(&card.0);
        let found_from_the_place = PlaceForAFolder::in_what_was_picked(&its_place);

        assert_eq!(found_from_the_card, PlaceForAFolder { into: its_place.clone(), is_where_the_boot_menu_looks: true });
        assert_eq!(found_from_the_place, found_from_the_card);
    }

    #[test]
    fn in_any_other_folder_it_goes_into_the_folder_as_it_is() {
        let any_folder = TemporaryFolder::create("any-folder");
        let only_half_the_way = any_folder.0.join("autorun");
        fs::create_dir_all(&only_half_the_way).unwrap();

        assert_eq!(PlaceForAFolder::in_what_was_picked(&any_folder.0), PlaceForAFolder { into: any_folder.0.clone(), is_where_the_boot_menu_looks: false });
        assert!(!PlaceForAFolder::in_what_was_picked(&only_half_the_way).is_where_the_boot_menu_looks);
    }

    #[test]
    fn the_menu_is_told_the_name_to_show_and_the_script_to_start() {
        let folder = TemporaryFolder::create("shown-in-the-menu");

        ShownInTheMenu { name: "Night Bus", start_script_file: "start-app.sh", start_script: "#!/bin/sh\r\necho hello\r\n" }.write_into(&folder.0).unwrap();

        assert_eq!(fs::read_to_string(folder.0.join(NAME_IN_THE_MENU_FILE)).unwrap(), "Night Bus");
        assert_eq!(fs::read_to_string(folder.0.join(NAMES_THE_START_SCRIPT_FILE)).unwrap(), r#"{"scripts":[{"path":"start-app.sh"}]}"#);
        assert_eq!(fs::read_to_string(folder.0.join("start-app.sh")).unwrap(), "#!/bin/sh\necho hello\n");
    }
}
