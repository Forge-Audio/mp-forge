//! An effect put on the card of an MPC Sample, where the device loads it as a Knob FX.
//!
//! The other MPCs are sent a plugin over the network by Akai's own tool. That tool does not work with the
//! MPC Sample, which takes plugins only from the `VST2 Plugins` folder of its card. So for the MPC Sample
//! alone MP Forge copies the finished plugin folder there itself, onto the card the musician picked.
//!
//! Nothing else on the card is touched. The folder is written beside `VST2 Plugins` first and moved in
//! whole, so the device never finds half a plugin. A folder already there is replaced only when it holds
//! this same plugin, and files are written as plain bytes, so none of the computer's own notes about a file
//! land on the card beside it.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::library::read_json;
use crate::sdk::built_plugin_folder;

/// The one folder of a card an MPC looks in for plugins.
pub const PLUGINS_FOLDER_ON_A_CARD: &str = "VST2 Plugins";

const MANIFEST: &str = "plugin.json";
const ID_IN_A_MANIFEST: &str = "uid";
/// A name no plugin folder has, and one the computers and the MPC leave out of what they list.
const STARTS_A_FOLDER_ON_ITS_WAY: &str = ".mp-forge-arriving ";
const STARTS_THE_FOLDER_BEFORE: &str = ".mp-forge-before ";

/// Where on the card the effect now is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnTheCard {
    pub folder: PathBuf,
    /// Whether an older copy of the same effect was there and has been replaced.
    pub replaced_an_older_copy: bool,
}

/// Why an effect was not put on the card. The card is as it was.
#[derive(Debug)]
pub enum NotPutOnTheCard {
    NotBuilt,
    /// A folder of the effect's name is on the card and holds some other plugin.
    NameIsTaken(PathBuf),
    Io(io::Error),
}

impl fmt::Display for NotPutOnTheCard {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NotPutOnTheCard::NotBuilt => write!(formatter, "The effect has no finished build to copy. Run the safety check first."),
            NotPutOnTheCard::NameIsTaken(folder) => write!(formatter, "There is already a folder at {} that holds another plugin. Move it away, or rename this effect.", folder.display()),
            NotPutOnTheCard::Io(cause) => write!(formatter, "The effect could not be written to the card: {cause}"),
        }
    }
}

impl From<io::Error> for NotPutOnTheCard {
    fn from(cause: io::Error) -> Self {
        NotPutOnTheCard::Io(cause)
    }
}

/// Copies the finished plugin folder of `plugin` into `VST2 Plugins` on the card the musician picked. They
/// may have picked that folder itself.
pub fn put_on_the_card(plugin: &Path, picked: &Path) -> Result<OnTheCard, NotPutOnTheCard> {
    let built = built_plugin_folder(plugin).ok_or(NotPutOnTheCard::NotBuilt)?;
    let card = card_of(picked);
    let folder = card.join(PLUGINS_FOLDER_ON_A_CARD).join(built.file_name().unwrap_or_default());
    let replaced_an_older_copy = folder.exists();
    if replaced_an_older_copy && !holds_the_same_plugin(&folder, &built) {
        return Err(NotPutOnTheCard::NameIsTaken(folder));
    }
    let arriving = beside_the_plugins_folder(&card, STARTS_A_FOLDER_ON_ITS_WAY, &folder);
    let written = write_as_plain_bytes(&built, &arriving).and_then(|()| put_in_place(&arriving, &folder, &card));
    if written.is_err() {
        let _ = remove_if_there(&arriving);
    }
    written?;
    Ok(OnTheCard { folder, replaced_an_older_copy })
}

fn card_of(picked: &Path) -> PathBuf {
    let picked_the_plugins_folder = picked.file_name().is_some_and(|name| name == PLUGINS_FOLDER_ON_A_CARD);
    if picked_the_plugins_folder { picked.parent().unwrap_or(picked).to_path_buf() } else { picked.to_path_buf() }
}

/// A plugin is known by its four letters: two folders that state the same four hold the same plugin.
fn holds_the_same_plugin(on_the_card: &Path, built: &Path) -> bool {
    let id_of = |folder: &Path| read_json(&folder.join(MANIFEST)).and_then(|manifest: Value| manifest[ID_IN_A_MANIFEST].as_str().map(str::to_string));
    id_of(on_the_card).is_some() && id_of(on_the_card) == id_of(built)
}

/// A folder at the top of the card, next to `VST2 Plugins` and never inside it.
fn beside_the_plugins_folder(card: &Path, starts_with: &str, folder: &Path) -> PathBuf {
    card.join(format!("{starts_with}{}", folder.file_name().unwrap_or_default().to_string_lossy()))
}

fn write_as_plain_bytes(from: &Path, into: &Path) -> io::Result<()> {
    remove_if_there(into)?;
    fs::create_dir_all(into)?;
    copy_each_file(from, into)
}

fn copy_each_file(from: &Path, into: &Path) -> io::Result<()> {
    for entry in fs::read_dir(from)? {
        let found = entry?.path();
        let lands_at = into.join(found.file_name().unwrap_or_default());
        if found.is_dir() {
            fs::create_dir_all(&lands_at)?;
            copy_each_file(&found, &lands_at)?;
        } else {
            fs::write(lands_at, fs::read(found)?)?;
        }
    }
    Ok(())
}

/// Moves the folder just written into `VST2 Plugins`. An older copy steps out beside it first and is thrown
/// away only once the new one is in its place.
fn put_in_place(arriving: &Path, folder: &Path, card: &Path) -> io::Result<()> {
    let before = beside_the_plugins_folder(card, STARTS_THE_FOLDER_BEFORE, folder);
    remove_if_there(&before)?;
    fs::create_dir_all(card.join(PLUGINS_FOLDER_ON_A_CARD))?;
    if folder.exists() {
        fs::rename(folder, &before)?;
    }
    fs::rename(arriving, folder)?;
    remove_if_there(&before)
}

fn remove_if_there(folder: &Path) -> io::Result<()> {
    if folder.exists() { fs::remove_dir_all(folder) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const BUILT_FOLDER: &str = "Forge Tests - VST - Dusty Tape";

    /// An effect called Dusty Tape as the build leaves it, and an empty card.
    struct Scene {
        folder: TemporaryFolder,
        plugin: PathBuf,
        card: PathBuf,
    }

    impl Scene {
        fn set(label: &str) -> Scene {
            let folder = TemporaryFolder::create(label);
            let (plugin, card) = (folder.0.join("Dusty Tape"), folder.0.join("card"));
            let built = plugin.join("build").join(BUILT_FOLDER);
            fs::create_dir_all(built.join("VST")).unwrap();
            fs::create_dir_all(&card).unwrap();
            fs::write(built.join(MANIFEST), r#"{ "name": "Dusty Tape", "uid": "WFmo" }"#).unwrap();
            fs::write(built.join("VST/dusty-tape-arm64.so"), b"the effect").unwrap();
            fs::write(plugin.join("build/params.json"), "{}").unwrap();
            Scene { folder, plugin, card }
        }

        fn on_the_card(&self) -> PathBuf {
            self.card.join(PLUGINS_FOLDER_ON_A_CARD).join(BUILT_FOLDER)
        }

        fn everything_on_the_card(&self) -> Vec<String> {
            let mut found = Vec::new();
            list_under(&self.card, &self.card, &mut found);
            found.sort();
            found
        }
    }

    fn list_under(folder: &Path, top: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(folder).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() { list_under(&path, top, found) } else { found.push(path.strip_prefix(top).unwrap().to_string_lossy().replace('\\', "/")) }
        }
    }

    #[test]
    fn the_card_ends_up_holding_exactly_the_built_plugin_and_nothing_beside_it() {
        let scene = Scene::set("card-plugin-written");

        let put = put_on_the_card(&scene.plugin, &scene.card).unwrap();

        assert_eq!(put, OnTheCard { folder: scene.on_the_card(), replaced_an_older_copy: false });
        assert_eq!(scene.everything_on_the_card(), [format!("VST2 Plugins/{BUILT_FOLDER}/VST/dusty-tape-arm64.so"), format!("VST2 Plugins/{BUILT_FOLDER}/plugin.json")]);
        assert_eq!(fs::read(scene.on_the_card().join("VST/dusty-tape-arm64.so")).unwrap(), b"the effect");
    }

    #[test]
    fn what_else_is_on_the_card_is_left_as_it_was() {
        let scene = Scene::set("card-plugin-others");
        let others = scene.card.join(PLUGINS_FOLDER_ON_A_CARD).join("Somebody - VST - Other");
        fs::create_dir_all(&others).unwrap();
        fs::write(others.join(MANIFEST), r#"{ "uid": "Othr" }"#).unwrap();
        fs::write(scene.card.join("my-beat.wav"), "RIFF").unwrap();

        put_on_the_card(&scene.plugin, &scene.card).unwrap();

        assert_eq!(fs::read_to_string(others.join(MANIFEST)).unwrap(), r#"{ "uid": "Othr" }"#);
        assert_eq!(fs::read_to_string(scene.card.join("my-beat.wav")).unwrap(), "RIFF");
        assert_eq!(scene.everything_on_the_card().len(), 4);
    }

    #[test]
    fn an_older_copy_of_the_same_effect_is_replaced_whole() {
        let scene = Scene::set("card-plugin-replaced");
        put_on_the_card(&scene.plugin, &scene.card).unwrap();
        fs::write(scene.on_the_card().join("left-from-before.txt"), "old").unwrap();
        fs::write(scene.plugin.join("build").join(BUILT_FOLDER).join("VST/dusty-tape-arm64.so"), b"the effect, improved").unwrap();

        let put = put_on_the_card(&scene.plugin, &scene.card).unwrap();

        assert!(put.replaced_an_older_copy);
        assert_eq!(fs::read(scene.on_the_card().join("VST/dusty-tape-arm64.so")).unwrap(), b"the effect, improved");
        assert_eq!(scene.everything_on_the_card().len(), 2, "{:?}", scene.everything_on_the_card());
    }

    #[test]
    fn somebody_elses_folder_of_the_same_name_is_never_replaced() {
        let scene = Scene::set("card-plugin-taken");
        fs::create_dir_all(scene.on_the_card()).unwrap();
        fs::write(scene.on_the_card().join(MANIFEST), r#"{ "name": "Dusty Tape", "uid": "Else" }"#).unwrap();

        let refused = put_on_the_card(&scene.plugin, &scene.card);

        assert!(matches!(refused, Err(NotPutOnTheCard::NameIsTaken(folder)) if folder == scene.on_the_card()));
        assert_eq!(scene.everything_on_the_card(), [format!("VST2 Plugins/{BUILT_FOLDER}/plugin.json")]);
    }

    #[test]
    fn picking_the_plugins_folder_itself_puts_the_effect_in_the_same_place() {
        let scene = Scene::set("card-plugin-picked-inside");
        fs::create_dir_all(scene.card.join(PLUGINS_FOLDER_ON_A_CARD)).unwrap();

        let put = put_on_the_card(&scene.plugin, &scene.card.join(PLUGINS_FOLDER_ON_A_CARD)).unwrap();

        assert_eq!(put.folder, scene.on_the_card());
    }

    #[test]
    fn an_effect_that_was_never_built_is_not_put_on_the_card() {
        let scene = Scene::set("card-plugin-not-built");
        fs::remove_dir_all(scene.plugin.join("build")).unwrap();

        assert!(matches!(put_on_the_card(&scene.plugin, &scene.card), Err(NotPutOnTheCard::NotBuilt)));
        assert!(scene.everything_on_the_card().is_empty() && scene.folder.0.is_dir());
    }
}
