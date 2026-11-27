//! A plugin's project packed into one file to hand to somebody else, and a project taken in from such a file
//! or from a folder, such as one written by hand with Akai's SDK.
//!
//! Only what makes the plugin is carried: its manifest, its code, its screen and its look, its drawings, and
//! the artwork and sounds brought into it. What was built from it, its earlier versions and the conversation
//! with the AI stay behind.
//!
//! Taking a project in runs nothing from it. Every file is checked for where it goes and how big it is, the
//! manifest is written afresh from the few things a manifest may say, and nothing lands among the plugins until
//! the whole project has arrived. The code is somebody else's all the same. The safety check reads it before
//! building it, and that check is a tripwire, not a sandbox.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde_json::{Value, json};

use crate::art_shelf::is_art_file_name;
use crate::maker::is_usable_name;
use crate::origin::{ORIGIN_FILE, Origin};
use crate::plugin_name;
use crate::sample_app;
use crate::sample_knobs::{SIX_KNOBS_FILE, SixKnobs};

/// What a packed project's file name ends with.
pub const PROJECT_FILE_ENDING: &str = "mpforge";

const MANIFEST: &str = "plugin.json";
/// The words of the plugin's guide, the addresses it brings sound in from and the knobs it is played with on an
/// MPC Sample are among them, when it has them.
/// So are the licence it is shared under, by any of the names such a file goes by, and the note of where it
/// came from: both have to stay with somebody else's work wherever it goes.
const OTHER_FILES_AT_THE_TOP: [&str; 13] =
    ["ui.json", "README.md", "DESIGN.md", "pictures.json", "look.json", "guide.json", "online.json", SIX_KNOBS_FILE, ORIGIN_FILE, "LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"];
/// The names a project's licence goes by, each of them carried.
pub(crate) const LICENSE_FILES: [&str; 4] = ["LICENSE", "LICENSE.md", "LICENSE.txt", "COPYING"];
const CODE_FOLDER: &str = "src";
const CODE_FILE_ENDING: &str = "cpp";
const ARTWORK_FOLDER: &str = "artwork";
const MEGABYTE: u64 = 1024 * 1024;
/// Code, layouts and drawings are written by hand or by the AI, and are never this big.
const LARGEST_WRITTEN_FILE: u64 = 2 * MEGABYTE;
const MOST_FILES: usize = 400;
const MOST_BYTES: u64 = 160 * MEGABYTE;
/// A packed project is read no further than this many entries, however many it claims to hold.
const MOST_ENTRIES_READ: usize = 4000;
/// A repository holds far more than a plugin's project, and its plugin's files can come last of all.
const MOST_ENTRIES_READ_OF_A_REPOSITORY: usize = 400_000;

/// A packed project as it arrives: how far it is read, and what is said when it cannot be read.
struct Packed {
    most_entries: usize,
    unreadable: &'static str,
}

const PACKED_BY_MP_FORGE: Packed = Packed { most_entries: MOST_ENTRIES_READ, unreadable: "the file is not a plugin packed by MP Forge." };
const PACKED_BY_ITS_MAKERS: Packed = Packed {
    most_entries: MOST_ENTRIES_READ_OF_A_REPOSITORY,
    unreadable: "its source could not be read to the end. The connection may have dropped, or it is bigger than MP Forge fetches.",
};
const LONGEST_FILE_NAME: usize = 64;
const PUNCTUATION_IN_FILE_NAMES: &str = " -_.";
/// Where a project is put together until it is whole. It holds no manifest until the end, so it is never taken for a plugin.
const FOLDER_WHILE_ARRIVING: &str = ".arriving-plugin";
const LONGEST_NOTE_IN_A_MANIFEST: usize = 300;
const LONGEST_VERSION: usize = 20;
const USUAL_VERSION: &str = "1.0.0";
const MOST_CHANNELS: u64 = 2;

/// A folder of a project, with the kind of file carried from it and how big one may be.
struct Shelf {
    folder: &'static [&'static str],
    endings: &'static [&'static str],
    largest_file: u64,
}

const SHELVES: [Shelf; 4] = [
    Shelf { folder: &[CODE_FOLDER], endings: &[CODE_FILE_ENDING, "h", "hpp"], largest_file: LARGEST_WRITTEN_FILE },
    Shelf { folder: &["pictures"], endings: &["svg"], largest_file: LARGEST_WRITTEN_FILE },
    Shelf { folder: &[ARTWORK_FOLDER], endings: &["png"], largest_file: 12 * MEGABYTE },
    Shelf { folder: &["Content", "Samples"], endings: &["wav"], largest_file: 16 * MEGABYTE },
];

/// Why a project was not taken in. Nothing of it is left among the plugins.
#[derive(Debug)]
pub enum NotTakenIn {
    /// What was picked is not a plugin's project, for this reason.
    NotAProject(String),
    /// There is already a plugin of this name. The musician can take the project in under another.
    NameTaken(String),
    UnusableName(String),
    /// Its source could not be fetched from where it is kept, for this reason.
    NotFetched(String),
    Io(io::Error),
}

impl fmt::Display for NotTakenIn {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotTakenIn::NotAProject(why) => write!(formatter, "This is not a plugin MP Forge can take in: {why}"),
            NotTakenIn::NameTaken(name) => write!(formatter, "There is already a plugin called `{name}`. Pick another name."),
            NotTakenIn::UnusableName(name) => write!(formatter, "`{name}` cannot be used as a plugin's name. Names start with a letter and use only letters, digits, spaces and hyphens."),
            NotTakenIn::NotFetched(why) => write!(formatter, "{why}"),
            NotTakenIn::Io(cause) => write!(formatter, "The plugin could not be taken in: {cause}"),
        }
    }
}

impl From<io::Error> for NotTakenIn {
    fn from(cause: io::Error) -> Self {
        NotTakenIn::Io(cause)
    }
}

fn not_a_project(why: impl Into<String>) -> NotTakenIn {
    NotTakenIn::NotAProject(why.into())
}

/// Packs the plugin's project into one file. A file already there is replaced.
pub fn pack(plugin_dir: &Path, into: &Path) -> io::Result<()> {
    let top_folder = PathBuf::from(plugin_dir.file_name().unwrap_or_default());
    let mut packed = tar::Builder::new(GzEncoder::new(File::create(into)?, Compression::default()));
    for carried in carried_files_in(plugin_dir) {
        packed.append_path_with_name(plugin_dir.join(&carried), top_folder.join(&carried))?;
    }
    packed.into_inner()?.finish()?;
    Ok(())
}

/// Takes a project in from a packed file or from a folder, as a new plugin in `plugins_dir`, and returns the
/// plugin's name. `named` gives it another name than its own. A plugin that is already there is never replaced.
pub fn take_in(from: &Path, plugins_dir: &Path, named: Option<&str>) -> Result<String, NotTakenIn> {
    let mut arriving = Arriving::begin_in(plugins_dir)?;
    if from.is_dir() { arriving.take_folder(from)? } else { arriving.take_packed(File::open(from)?, &PACKED_BY_MP_FORGE)? }
    arriving.land_among(plugins_dir, named)
}

/// Takes in somebody else's plugin from its source, packed as its makers keep it, and notes where it came from.
/// The source is read as it arrives and only a project's own files are written, so however much else the
/// makers keep beside their plugin, none of it lands here. The note replaces any the project brought along,
/// so it always names where this copy was fetched from.
pub fn take_in_from(origin: &Origin, packed_source: impl Read, plugins_dir: &Path, named: Option<&str>) -> Result<String, NotTakenIn> {
    let mut arriving = Arriving::begin_in(plugins_dir)?;
    arriving.take_packed(packed_source, &PACKED_BY_ITS_MAKERS)?;
    origin.write_into(&arriving.folder)?;
    arriving.land_among(plugins_dir, named)
}

/// Every file of the project in this folder that is carried, by its path inside the folder.
pub(crate) fn carried_files_in(project: &Path) -> Vec<PathBuf> {
    let at_the_top = std::iter::once(MANIFEST).chain(OTHER_FILES_AT_THE_TOP).map(PathBuf::from);
    let on_the_shelves = SHELVES.iter().flat_map(|shelf| files_on(shelf, project));
    let mut carried: Vec<PathBuf> = at_the_top.chain(on_the_shelves).filter(|file| is_a_plain_file(&project.join(file))).collect();
    carried.sort();
    carried
}

fn files_on(shelf: &Shelf, project: &Path) -> Vec<PathBuf> {
    let folder: PathBuf = shelf.folder.iter().collect();
    let entries = fs::read_dir(project.join(&folder)).into_iter().flatten().flatten();
    let inside = entries.map(|entry| folder.join(entry.file_name()));
    inside.filter(|file| largest_allowed(file).is_some()).collect()
}

/// A link is not followed: only a file that is really in the project is carried.
fn is_a_plain_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|found| found.is_file())
}

/// How big the file at this place in a project may be. None for a place nothing is carried from.
fn largest_allowed(inside_the_project: &Path) -> Option<u64> {
    let parts = plain_parts_of(inside_the_project)?;
    let (file_name, folder) = parts.split_last()?;
    if folder.is_empty() {
        let is_known = *file_name == MANIFEST || OTHER_FILES_AT_THE_TOP.contains(file_name);
        return is_known.then_some(LARGEST_WRITTEN_FILE);
    }
    let shelf = SHELVES.iter().find(|shelf| shelf.folder == folder)?;
    let ending = Path::new(file_name).extension()?.to_str()?;
    let is_carried = shelf.endings.contains(&ending) && (folder != [ARTWORK_FOLDER] || is_art_file_name(file_name));
    is_carried.then_some(shelf.largest_file)
}

/// The path as the names it goes through, when every one is a plain name. A path that climbs out of its folder,
/// starts at the top of a disk or holds a hidden or oddly named file has none.
fn plain_parts_of(path: &Path) -> Option<Vec<&str>> {
    path.components().map(|part| match part {
        Component::Normal(name) => name.to_str().filter(|name| is_a_plain_file_name(name)),
        _ => None,
    }).collect()
}

/// The path as it is written on every system, whatever this one puts between a folder and a file.
fn written_with_slashes(inside_the_project: &Path) -> String {
    plain_parts_of(inside_the_project).unwrap_or_default().join("/")
}

fn is_a_plain_file_name(name: &str) -> bool {
    let plain = name.chars().all(|letter| letter.is_ascii_alphanumeric() || PUNCTUATION_IN_FILE_NAMES.contains(letter));
    let starts_and_ends_well = !name.starts_with('.') && !name.ends_with(['.', ' ']);
    !name.is_empty() && name.len() <= LONGEST_FILE_NAME && plain && starts_and_ends_well
}

/// A project on its way in: the folder it is put together in, and how much has arrived.
struct Arriving {
    folder: PathBuf,
    files: usize,
    bytes: u64,
}

impl Arriving {
    fn begin_in(plugins_dir: &Path) -> io::Result<Self> {
        let folder = plugins_dir.join(FOLDER_WHILE_ARRIVING);
        remove_if_there(&folder)?;
        fs::create_dir_all(&folder)?;
        Ok(Arriving { folder, files: 0, bytes: 0 })
    }

    fn take_folder(&mut self, project: &Path) -> Result<(), NotTakenIn> {
        for carried in carried_files_in(project) {
            self.take(&carried, &mut File::open(project.join(&carried))?)?;
        }
        Ok(())
    }

    /// The project that arrived becomes a plugin among the others, under `named` or its own name. A plugin that
    /// is already there is never replaced.
    fn land_among(self, plugins_dir: &Path, named: Option<&str>) -> Result<String, NotTakenIn> {
        let own_name = self.write_manifest_afresh()?;
        let name = named.unwrap_or(&own_name);
        if !is_usable_name(name) {
            return Err(NotTakenIn::UnusableName(name.to_string()));
        }
        if plugins_dir.join(name).exists() {
            return Err(NotTakenIn::NameTaken(name.to_string()));
        }
        if name != own_name {
            plugin_name::rename_where_it_is_written(&self.folder, &own_name, name)?;
        }
        fs::rename(&self.folder, plugins_dir.join(name))?;
        Ok(name.to_string())
    }

    /// Reads a packed project. Its files sit in one folder, whose name is left behind.
    fn take_packed(&mut self, packed_project: impl Read, as_packed: &Packed) -> Result<(), NotTakenIn> {
        let unreadable = |_| not_a_project(as_packed.unreadable);
        let mut packed = tar::Archive::new(GzDecoder::new(packed_project));
        for entry in packed.entries().map_err(unreadable)?.take(as_packed.most_entries) {
            let mut entry = entry.map_err(unreadable)?;
            let path = entry.path().map_err(unreadable)?.into_owned();
            let inside_the_project: PathBuf = path.components().skip(1).collect();
            if entry.header().entry_type().is_file() && largest_allowed(&inside_the_project).is_some() {
                self.take(&inside_the_project, &mut entry)?;
            }
        }
        Ok(())
    }

    /// Writes one file of the project, unless it is bigger than its kind may be or the project has grown too big.
    fn take(&mut self, inside_the_project: &Path, contents: &mut dyn Read) -> Result<(), NotTakenIn> {
        let largest = largest_allowed(inside_the_project).ok_or_else(|| not_a_project("it holds a file that does not belong in a plugin."))?;
        let lands_at = self.folder.join(inside_the_project);
        if let Some(folder) = lands_at.parent() {
            fs::create_dir_all(folder)?;
        }
        let written = io::copy(&mut contents.take(largest + 1), &mut File::create(&lands_at)?)?;
        self.files += 1;
        self.bytes += written;
        if written > largest {
            return Err(not_a_project(format!("`{}` is bigger than such a file may be.", written_with_slashes(inside_the_project))));
        }
        if self.files > MOST_FILES || self.bytes > MOST_BYTES {
            return Err(not_a_project("it is bigger than a plugin may be."));
        }
        Ok(())
    }

    /// Replaces the manifest that arrived with one that says only what a manifest may, and returns the plugin's name.
    fn write_manifest_afresh(&self) -> Result<String, NotTakenIn> {
        let arrived = fs::read_to_string(self.folder.join(MANIFEST)).map_err(|_| not_a_project("it has no plugin.json."))?;
        let arrived: Value = serde_json::from_str(&arrived).map_err(|_| not_a_project("its plugin.json cannot be read."))?;
        if !self.has_code() {
            return Err(not_a_project("it has no code. Only a plugin's project can be taken in, not a finished plugin built for the MPC."));
        }
        let manifest = manifest_afresh(&arrived)?;
        self.keep_the_knobs_named_in(&arrived)?;
        let written = serde_json::to_string_pretty(&manifest).expect("a manifest is always plain JSON");
        fs::write(self.folder.join(MANIFEST), written + "\n")?;
        Ok(manifest["name"].as_str().unwrap_or_default().to_string())
    }

    /// A plugin written by hand with Akai's SDK names the MPC Sample's knobs in its manifest. They are kept
    /// the way MP Forge keeps them, in a file of their own, unless the project brought that file along.
    fn keep_the_knobs_named_in(&self, arrived: &Value) -> io::Result<()> {
        let brought_its_own = self.folder.join(SIX_KNOBS_FILE).exists();
        match SixKnobs::named_in_a_manifest(arrived) {
            Some(named) if !brought_its_own && arrived["kind"] != sample_app::KIND => named.keep_with(&self.folder),
            _ => Ok(()),
        }
    }

    fn has_code(&self) -> bool {
        let code = fs::read_dir(self.folder.join(CODE_FOLDER)).into_iter().flatten().flatten();
        code.into_iter().any(|file| file.path().extension().is_some_and(|ending| ending == CODE_FILE_ENDING))
    }
}

impl Drop for Arriving {
    /// A project that was taken in has left this folder. One that was not leaves nothing behind.
    fn drop(&mut self) {
        let _ = remove_if_there(&self.folder);
    }
}

fn remove_if_there(folder: &Path) -> io::Result<()> {
    if folder.exists() { fs::remove_dir_all(folder) } else { Ok(()) }
}

/// A manifest holding only what one may say, each thing checked. An app for the MPC Sample stays one.
fn manifest_afresh(arrived: &Value) -> Result<Value, NotTakenIn> {
    if arrived["kind"] == sample_app::KIND { manifest_of_a_sample_app(arrived) } else { manifest_of_a_plugin(arrived) }
}

/// A Sample app says who it is and no more: nothing of it is built for the MPCs with a touch screen.
fn manifest_of_a_sample_app(arrived: &Value) -> Result<Value, NotTakenIn> {
    Ok(json!({
        "name": usable_name_in(arrived, "name")?,
        "manufacturer": usable_name_in(arrived, "manufacturer")?,
        "version": version_in(arrived),
        "kind": sample_app::KIND,
    }))
}

/// The files a plugin names for the MPC are worked out from its name, so a manifest cannot point the build
/// anywhere else.
fn manifest_of_a_plugin(arrived: &Value) -> Result<Value, NotTakenIn> {
    let name = usable_name_in(arrived, "name")?;
    let maker = usable_name_in(arrived, "manufacturer")?;
    let is_effect = arrived["kind"].as_str().is_some_and(|kind| kind.eq_ignore_ascii_case("effect"));
    let slug = plugin_name::slug_of(&name);
    Ok(json!({
        "name": name,
        "manufacturer": maker,
        "version": version_in(arrived),
        "format": "vst2",
        "kind": if is_effect { "effect" } else { "instrument" },
        "uid": id_in(arrived)?,
        "numInputs": channels_in(arrived, "numInputs").unwrap_or(if is_effect { MOST_CHANNELS } else { 0 }),
        "numOutputs": channels_in(arrived, "numOutputs").filter(|outputs| *outputs > 0).unwrap_or(MOST_CHANNELS),
        "category": note_in(arrived, "category"),
        "file32": format!("VST/{slug}-arm32.so"),
        "file64": format!("VST/{slug}-arm64.so"),
        "description": note_in(arrived, "description"),
        "author": note_in(arrived, "author"),
        "url": note_in(arrived, "url"),
        "license": note_in(arrived, "license"),
    }))
}

fn usable_name_in(manifest: &Value, field: &str) -> Result<String, NotTakenIn> {
    let name = manifest[field].as_str().filter(|name| is_usable_name(name));
    name.map(str::to_string).ok_or_else(|| not_a_project(format!("its plugin.json has no usable \"{field}\".")))
}

/// The four letters or digits the MPC knows the plugin by. The plugin's code states the same four.
fn id_in(manifest: &Value) -> Result<String, NotTakenIn> {
    let id = manifest["uid"].as_str().filter(|id| id.len() == 4 && id.chars().all(|letter| letter.is_ascii_alphanumeric()));
    id.map(str::to_string).ok_or_else(|| not_a_project("the \"uid\" in its plugin.json is not four letters or digits."))
}

fn version_in(manifest: &Value) -> String {
    let fits = |version: &&str| !version.is_empty() && version.len() <= LONGEST_VERSION && version.chars().all(|letter| letter.is_ascii_alphanumeric() || ".-".contains(letter));
    manifest["version"].as_str().filter(fits).unwrap_or(USUAL_VERSION).to_string()
}

fn channels_in(manifest: &Value, field: &str) -> Option<u64> {
    manifest[field].as_u64().filter(|channels| *channels <= MOST_CHANNELS)
}

/// Free words from a manifest, on one line and no longer than a note may be.
fn note_in(manifest: &Value, field: &str) -> String {
    let words = manifest[field].as_str().unwrap_or_default();
    words.chars().filter(|letter| !letter.is_control()).take(LONGEST_NOTE_IN_A_MANIFEST).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    const CODE: &str = "// Dusty Tape\ninfo.name = \"Dusty Tape\";\ninfo.uniqueId = MPC_FOURCC('W', 'F', 'm', 'o');\n";
    const TINY_PNG: &[u8] = b"\x89PNG\r\n\x1a\n not really a picture";

    fn manifest_of(name: &str) -> String {
        json!({ "name": name, "manufacturer": "Forge Tests", "version": "1.0.0", "format": "vst2", "kind": "effect", "uid": "WFmo",
                "numInputs": 2, "numOutputs": 2, "category": "Delay", "file32": "VST/dusty-tape-arm32.so", "file64": "VST/dusty-tape-arm64.so",
                "description": "A worn tape echo.", "author": "Forge Tests", "url": "", "license": "MIT" }).to_string()
    }

    fn write(folder: &Path, inside: &str, contents: impl AsRef<[u8]>) {
        let file = folder.join(inside);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, contents).unwrap();
    }

    /// A folder of plugins holding Dusty Tape, with everything a plugin gathers while it is worked on.
    fn plugins_with_dusty_tape(label: &str) -> (TemporaryFolder, PathBuf) {
        let plugins = TemporaryFolder::create(label);
        let plugin = plugins.0.join("Dusty Tape");
        write(&plugin, MANIFEST, manifest_of("Dusty Tape"));
        write(&plugin, "src/DustyTape.cpp", CODE);
        write(&plugin, "ui.json", r#"{ "pages": [] }"#);
        write(&plugin, "look.json", r#"{ "pages": [] }"#);
        write(&plugin, "pictures/reels.svg", "<svg/>");
        write(&plugin, "artwork/art-3.png", TINY_PNG);
        write(&plugin, "Content/Samples/thump.wav", "RIFF");
        write(&plugin, "build/native/dusty-tape.dylib", "built code");
        write(&plugin, ".versions/1/plugin.json", manifest_of("Dusty Tape"));
        write(&plugin, ".workbench/conversation.json", "[\"what I said to the AI\"]");
        write(&plugin, "attachments/picture-1.png", TINY_PNG);
        (plugins, plugin)
    }

    fn files_in(folder: &Path) -> Vec<String> {
        fn listed(folder: &Path, top: &Path, found: &mut Vec<String>) {
            for entry in fs::read_dir(folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() { listed(&path, top, found) } else { found.push(path.strip_prefix(top).unwrap().to_string_lossy().replace('\\', "/")) }
            }
        }
        let mut found = Vec::new();
        listed(folder, folder, &mut found);
        found.sort();
        found
    }

    fn packed_with(entries: impl FnOnce(&mut tar::Builder<GzEncoder<File>>), folder: &TemporaryFolder) -> PathBuf {
        let file = folder.0.join("handed-over.mpforge");
        let mut packed = tar::Builder::new(GzEncoder::new(File::create(&file).unwrap(), Compression::fast()));
        entries(&mut packed);
        packed.into_inner().unwrap().finish().unwrap();
        file
    }

    /// Adds a file under exactly this path, even one no honest packer would write.
    fn add(packed: &mut tar::Builder<GzEncoder<File>>, path: &str, contents: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.as_gnu_mut().unwrap().name[..path.len()].copy_from_slice(path.as_bytes());
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_entry_type(tar::EntryType::Regular);
        header.set_cksum();
        packed.append(&header, contents).unwrap();
    }

    fn why_refused(refusal: Result<String, NotTakenIn>) -> String {
        refusal.unwrap_err().to_string()
    }

    #[test]
    fn a_packed_plugin_is_taken_in_with_what_makes_it_and_nothing_else() {
        let (plugins, plugin) = plugins_with_dusty_tape("project-round-trip");
        let elsewhere = TemporaryFolder::create("project-round-trip-elsewhere");
        let packed = plugins.0.join("Dusty Tape.mpforge");

        pack(&plugin, &packed).unwrap();
        let name = take_in(&packed, &elsewhere.0, None).unwrap();

        assert_eq!(name, "Dusty Tape");
        assert_eq!(files_in(&elsewhere.0.join("Dusty Tape")), [
            "Content/Samples/thump.wav", "artwork/art-3.png", "look.json", "pictures/reels.svg", "plugin.json", "src/DustyTape.cpp", "ui.json",
        ]);
        assert_eq!(fs::read_to_string(elsewhere.0.join("Dusty Tape/src/DustyTape.cpp")).unwrap(), CODE);
        assert!(!elsewhere.0.join(FOLDER_WHILE_ARRIVING).exists());
    }

    #[test]
    fn an_app_for_the_mpc_sample_is_taken_in_as_one() {
        let (apps, elsewhere) = (TemporaryFolder::create("project-sample-app"), TemporaryFolder::create("project-sample-app-elsewhere"));
        let (app, packed) = (apps.0.join("Night Bus"), apps.0.join("Night Bus.mpforge"));
        let said_of_it = json!({ "name": "Night Bus", "manufacturer": "Forge Tests", "version": "1.2.0", "kind": sample_app::KIND, "file64": "VST/elsewhere.so" });
        write(&app, "plugin.json", said_of_it.to_string());
        write(&app, "src/NightBus.cpp", "// Night Bus\n");
        write(&app, "Content/Samples/kick.wav", "RIFF");

        pack(&app, &packed).unwrap();
        let name = take_in(&packed, &elsewhere.0, None).unwrap();

        let taken_in = elsewhere.0.join(&name);
        let manifest: Value = serde_json::from_str(&fs::read_to_string(taken_in.join("plugin.json")).unwrap()).unwrap();
        assert!(sample_app::is_one(&taken_in));
        assert_eq!(manifest, json!({ "name": "Night Bus", "manufacturer": "Forge Tests", "version": "1.2.0", "kind": "sample-app" }), "it says who it is and no more");
        assert_eq!(files_in(&taken_in), ["Content/Samples/kick.wav", "plugin.json", "src/NightBus.cpp"]);
    }

    #[test]
    fn somebody_elses_plugin_arrives_with_its_licence_and_a_note_of_where_it_came_from() {
        let (plugins, plugin) = plugins_with_dusty_tape("project-with-origin");
        let elsewhere = TemporaryFolder::create("project-with-origin-elsewhere");
        let packed = plugins.0.join("as-its-makers-keep-it.tar.gz");
        let origin = Origin { name: "Dusty Tape".to_string(), author: "somebody".to_string(), source: "https://github.com/somebody/dusty-tape".to_string(), license: "GPL-3.0-only".to_string() };
        fs::write(plugin.join("LICENSE"), "The licence its makers chose.").unwrap();
        fs::write(plugin.join(ORIGIN_FILE), "a note from further back").unwrap();

        pack(&plugin, &packed).unwrap();
        let name = take_in_from(&origin, File::open(&packed).unwrap(), &elsewhere.0, None).unwrap();

        let taken_in = elsewhere.0.join(name);
        assert_eq!(fs::read_to_string(taken_in.join("LICENSE")).unwrap(), "The licence its makers chose.");
        assert_eq!(Origin::of_plugin(&taken_in), Some(origin));
    }

    #[test]
    fn a_project_folder_is_taken_in_the_same_way() {
        let (_plugins, plugin) = plugins_with_dusty_tape("project-from-folder");
        let elsewhere = TemporaryFolder::create("project-from-folder-elsewhere");

        take_in(&plugin, &elsewhere.0, None).unwrap();

        let taken = files_in(&elsewhere.0.join("Dusty Tape"));
        assert!(taken.contains(&"src/DustyTape.cpp".to_string()) && taken.contains(&"plugin.json".to_string()));
        assert!(!taken.iter().any(|file| file.starts_with("build") || file.starts_with('.') || file.starts_with("attachments")), "{taken:?}");
    }

    #[test]
    fn a_plugin_already_there_is_never_replaced_and_the_project_can_come_in_under_another_name() {
        let (plugins, plugin) = plugins_with_dusty_tape("project-name-taken");
        let packed = plugins.0.join("Dusty Tape.mpforge");
        pack(&plugin, &packed).unwrap();
        fs::write(plugin.join("ui.json"), "mine, changed since").unwrap();

        let refused = take_in(&packed, &plugins.0, None);
        let renamed = take_in(&packed, &plugins.0, Some("Dusty Tape Two")).unwrap();

        assert!(matches!(refused, Err(NotTakenIn::NameTaken(name)) if name == "Dusty Tape"));
        assert_eq!(fs::read_to_string(plugin.join("ui.json")).unwrap(), "mine, changed since");
        let taken = plugins.0.join(&renamed);
        assert!(fs::read_to_string(taken.join(MANIFEST)).unwrap().contains("\"name\": \"Dusty Tape Two\""));
        assert!(fs::read_to_string(taken.join(MANIFEST)).unwrap().contains("VST/dusty-tape-two-arm64.so"));
        assert!(fs::read_to_string(taken.join("src/DustyTape.cpp")).unwrap().contains("info.name = \"Dusty Tape Two\";"));
        assert!(matches!(take_in(&packed, &plugins.0, Some("../Sneaky")), Err(NotTakenIn::UnusableName(_))));
    }

    #[test]
    fn files_that_would_land_outside_the_plugin_are_left_out() {
        let folder = TemporaryFolder::create("project-climbing-out");
        let packed = packed_with(|packed| {
            add(packed, "Dusty Tape/plugin.json", manifest_of("Dusty Tape").as_bytes());
            add(packed, "Dusty Tape/src/DustyTape.cpp", CODE.as_bytes());
            add(packed, "Dusty Tape/src/../../escaped.cpp", b"outside");
            add(packed, "Dusty Tape/build/native/dusty-tape.dylib", b"somebody else's built code");
            add(packed, "Dusty Tape/.workbench/conversation.json", b"[]");
            add(packed, "Dusty Tape/src/run-me.sh", b"#!/bin/sh");
            add(packed, "Dusty Tape/artwork/photo.png", TINY_PNG);
        }, &folder);
        let plugins = folder.0.join("plugins");

        take_in(&packed, &plugins, None).unwrap();

        assert_eq!(files_in(&plugins), ["Dusty Tape/plugin.json", "Dusty Tape/src/DustyTape.cpp"]);
        assert!(!folder.0.join("escaped.cpp").exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_never_followed_out_of_the_project() {
        let (_plugins, plugin) = plugins_with_dusty_tape("project-link");
        let secret = TemporaryFolder::create("project-link-secret");
        fs::write(secret.0.join("secret.cpp"), "not the project's").unwrap();
        std::os::unix::fs::symlink(secret.0.join("secret.cpp"), plugin.join("src/Linked.cpp")).unwrap();
        let elsewhere = TemporaryFolder::create("project-link-elsewhere");

        take_in(&plugin, &elsewhere.0, None).unwrap();

        assert!(!elsewhere.0.join("Dusty Tape/src/Linked.cpp").exists());
    }

    #[test]
    fn the_manifest_is_written_afresh_so_it_cannot_point_the_build_anywhere() {
        let folder = TemporaryFolder::create("project-manifest");
        let sly = json!({ "name": "Dusty Tape", "manufacturer": "Forge Tests", "kind": "Effect", "uid": "WFmo", "numOutputs": 0,
                          "file32": "../../../outside.so", "file64": "/tmp/outside.so", "file": "x.so", "runAfterBuild": "curl example.com",
                          "description": "two\nlines", "version": "1.0; rm -rf" });
        write(&folder.0.join("project"), MANIFEST, sly.to_string());
        write(&folder.0.join("project"), "src/DustyTape.cpp", CODE);

        take_in(&folder.0.join("project"), &folder.0.join("plugins"), None).unwrap();

        let manifest: Value = serde_json::from_str(&fs::read_to_string(folder.0.join("plugins/Dusty Tape/plugin.json")).unwrap()).unwrap();
        assert_eq!((&manifest["file32"], &manifest["file64"]), (&json!("VST/dusty-tape-arm32.so"), &json!("VST/dusty-tape-arm64.so")));
        assert_eq!((&manifest["kind"], &manifest["numInputs"], &manifest["numOutputs"]), (&json!("effect"), &json!(2), &json!(2)));
        assert_eq!((&manifest["description"], &manifest["version"]), (&json!("twolines"), &json!(USUAL_VERSION)));
        assert!(manifest.get("runAfterBuild").is_none() && manifest.get("file").is_none());
    }

    #[test]
    fn the_knobs_a_handwritten_plugin_names_for_the_mpc_sample_are_kept_in_a_file_of_their_own() {
        let folder = TemporaryFolder::create("project-six-knobs");
        let mut by_hand: Value = serde_json::from_str(&manifest_of("Dusty Tape")).unwrap();
        by_hand["shortName"] = json!("Dusty");
        by_hand["knobs"] = json!([{ "parameter": 2, "label": "Wear" }, {}, { "parameter": 1, "label": "Mix" }]);
        write(&folder.0.join("project"), MANIFEST, by_hand.to_string());
        write(&folder.0.join("project"), "src/DustyTape.cpp", CODE);

        take_in(&folder.0.join("project"), &folder.0.join("plugins"), None).unwrap();

        let taken_in = folder.0.join("plugins/Dusty Tape");
        let manifest: Value = serde_json::from_str(&fs::read_to_string(taken_in.join(MANIFEST)).unwrap()).unwrap();
        let kept: Value = serde_json::from_str(&fs::read_to_string(taken_in.join(SIX_KNOBS_FILE)).unwrap()).unwrap();
        assert!(manifest.get("knobs").is_none() && manifest.get("shortName").is_none(), "the manifest says only what the build needs");
        assert_eq!(kept, json!({ "shortName": "Dusty", "knobs": [{ "parameter": 2, "label": "Wear" }, {}, { "parameter": 1, "label": "Mix" }], "shiftKnobs": [{}, {}, {}] }));
    }

    #[test]
    fn the_knobs_a_project_brings_along_win_over_what_its_manifest_names() {
        let folder = TemporaryFolder::create("project-six-knobs-own");
        let mut by_hand: Value = serde_json::from_str(&manifest_of("Dusty Tape")).unwrap();
        by_hand["knobs"] = json!([{ "parameter": 2, "label": "Wear" }]);
        write(&folder.0.join("project"), MANIFEST, by_hand.to_string());
        write(&folder.0.join("project"), "src/DustyTape.cpp", CODE);
        write(&folder.0.join("project"), SIX_KNOBS_FILE, r#"{ "knobs": [{ "control": "Wear" }] }"#);

        take_in(&folder.0.join("project"), &folder.0.join("plugins"), None).unwrap();

        assert_eq!(fs::read_to_string(folder.0.join("plugins/Dusty Tape").join(SIX_KNOBS_FILE)).unwrap(), r#"{ "knobs": [{ "control": "Wear" }] }"#);
    }

    #[test]
    fn what_is_not_a_plugins_project_is_refused_and_leaves_nothing_behind() {
        let folder = TemporaryFolder::create("project-refused");
        let plugins = folder.0.join("plugins");
        let (no_manifest, no_code, bad_id, built_only) = (folder.0.join("a"), folder.0.join("b"), folder.0.join("c"), folder.0.join("d"));
        write(&no_manifest, "src/DustyTape.cpp", CODE);
        write(&no_code, MANIFEST, manifest_of("Dusty Tape"));
        write(&bad_id, MANIFEST, manifest_of("Dusty Tape").replace("WFmo", "W/mo"));
        write(&bad_id, "src/DustyTape.cpp", CODE);
        write(&built_only, MANIFEST, manifest_of("Dusty Tape"));
        write(&built_only, "VST/dusty-tape-arm64.so", "a finished plugin");
        fs::write(folder.0.join("notes.txt"), "not packed by MP Forge").unwrap();

        assert!(why_refused(take_in(&no_manifest, &plugins, None)).contains("it has no plugin.json"));
        assert!(why_refused(take_in(&no_code, &plugins, None)).contains("it has no code"));
        assert!(why_refused(take_in(&bad_id, &plugins, None)).contains("not four letters or digits"));
        assert!(why_refused(take_in(&built_only, &plugins, None)).contains("not a finished plugin built for the MPC"));
        assert!(why_refused(take_in(&folder.0.join("notes.txt"), &plugins, None)).contains("not a plugin packed by MP Forge"));
        assert_eq!(files_in(&plugins), Vec::<String>::new());
    }

    #[test]
    fn a_file_bigger_than_its_kind_may_be_stops_the_whole_project() {
        let folder = TemporaryFolder::create("project-too-big");
        let far_too_much_code = vec![b'/'; LARGEST_WRITTEN_FILE as usize + 1];
        let packed = packed_with(|packed| {
            add(packed, "Dusty Tape/plugin.json", manifest_of("Dusty Tape").as_bytes());
            add(packed, "Dusty Tape/src/DustyTape.cpp", &far_too_much_code);
        }, &folder);
        let plugins = folder.0.join("plugins");

        assert!(why_refused(take_in(&packed, &plugins, None)).contains("`src/DustyTape.cpp` is bigger than such a file may be"));
        assert_eq!(files_in(&plugins), Vec::<String>::new());
    }

    #[test]
    fn only_plain_names_in_known_places_are_carried() {
        let carried = |path: &str| largest_allowed(Path::new(path)).is_some();

        assert!(carried("plugin.json") && carried("src/DustyTape.cpp") && carried("src/voice.hpp") && carried("Content/Samples/kick 1.wav"));
        assert!(carried("artwork/art-12.png") && carried("pictures/reels.svg"));
        assert!(!carried("../plugin.json") && !carried("/etc/passwd") && !carried("src/.hidden.cpp") && !carried("src/deep/voice.cpp"));
        assert!(!carried("Content/Samples/kick.exe") && !carried("Content/other.wav") && !carried("notes.txt") && !carried("src/a\u{202e}b.cpp"));
    }
}
