//! The only two places the AI can see: the plugin it is working on, and a read-only reference library.
//!
//! Every path the AI names is checked here. It can never leave these folders, and inside the plugin
//! it can only write the kinds of file a plugin is made of.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::attachments::is_attachment;
use crate::look::LOOK_FILE;
use crate::pictures::{DRAWING_EXTENSION, DRAWINGS_FOLDER, PICTURES_FILE};

const LARGEST_FILE_IN_BYTES: usize = 512 * 1024;
const PLUGIN_MANIFEST: &str = "plugin.json";
/// `DESIGN.md` is where the AI keeps its plan for the sound and for the screen's look, so both stay whole from one
/// request to the next.
const FILES_THE_AI_WRITES: [&str; 4] = ["ui.json", "README.md", "DESIGN.md", PICTURES_FILE];
const SOURCE_FOLDER: &str = "src";
const SOURCE_EXTENSIONS: [&str; 3] = ["cpp", "h", "hpp"];
const REFERENCE_FOLDERS: [&str; 6] = ["docs", "framework", "examples", "templates", "schemas", "ui-kit"];
const REFERENCE_EXTENSIONS: [&str; 5] = ["md", "h", "hpp", "cpp", "json"];
const BUILD_OUTPUT_FOLDER: &str = "build";
/// MP Forge's own guides. They are built into the program and read like any other file of the library, so the AI
/// is sent them only when its work calls for them.
const OWN_GUIDES: [OwnGuide; 1] = [OwnGuide { path: "mp-forge/pictures.md", text: include_str!("../../directives/reference/pictures.md") }];

struct OwnGuide {
    path: &'static str,
    text: &'static str,
}

#[derive(Debug)]
pub enum FileError {
    OutsideAllowedArea(String),
    NotAllowed(String),
    TooLarge { bytes: usize },
    Io(io::Error),
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileError::OutsideAllowedArea(path) => write!(formatter, "`{path}` is outside the area you can use."),
            FileError::NotAllowed(explanation) => write!(formatter, "{explanation}"),
            FileError::TooLarge { bytes } => {
                write!(formatter, "That is {bytes} bytes; the largest file allowed is {LARGEST_FILE_IN_BYTES}.")
            }
            FileError::Io(cause) => write!(formatter, "The file could not be used: {cause}"),
        }
    }
}

impl From<io::Error> for FileError {
    fn from(cause: io::Error) -> Self {
        FileError::Io(cause)
    }
}

const WINDOWS_MARK_OF_A_WHOLE_PATH: &str = r"\\?\";
const WINDOWS_MARK_OF_A_SHARED_FOLDER: &str = r"UNC\";

/// The one true path of a file or folder, written the way any program can read it.
///
/// Windows hands such a path back marked `\\?\`, and under that mark a forward slash is no longer a step
/// between folders. Akai's tools join their paths with forward slashes, so given a marked path they look for
/// `pictures/lamp.png` as one oddly named file and say it is missing. The mark is taken off again here.
pub(crate) fn true_path_of(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize().map(|marked| if cfg!(windows) { without_the_windows_mark(&marked) } else { marked })
}

fn without_the_windows_mark(path: &Path) -> PathBuf {
    let Some(marked) = path.to_str().and_then(|written| written.strip_prefix(WINDOWS_MARK_OF_A_WHOLE_PATH)) else {
        return path.to_path_buf();
    };
    match marked.strip_prefix(WINDOWS_MARK_OF_A_SHARED_FOLDER) {
        Some(shared_folder) => PathBuf::from(format!(r"\\{shared_folder}")),
        None => PathBuf::from(marked),
    }
}

/// The plugin the AI is working on.
pub struct PluginWorkspace {
    root: PathBuf,
}

impl PluginWorkspace {
    pub fn open(root: &Path) -> io::Result<Self> {
        Ok(Self { root: true_path_of(root)? })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The files the AI may read, as paths relative to the plugin folder.
    pub fn list(&self) -> io::Result<Vec<String>> {
        let listed = relative_files_under(&self.root)?;
        Ok(listed.into_iter().filter(|path| may_read_in_plugin(Path::new(path))).collect())
    }

    pub fn read(&self, relative: &str) -> Result<String, FileError> {
        let path = contained_path(&self.root, relative)?;
        if !may_read_in_plugin(Path::new(relative)) {
            return Err(FileError::NotAllowed(format!("`{relative}` is not one of the plugin's own files.")));
        }
        Ok(fs::read_to_string(path)?)
    }

    /// One of the pictures the musician attached, as the bytes of its image file.
    pub fn read_attachment(&self, relative: &str) -> Result<Vec<u8>, FileError> {
        let path = contained_path(&self.root, relative)?;
        if !is_attachment(Path::new(relative)) {
            return Err(FileError::NotAllowed(format!("`{relative}` is not a picture the musician attached.")));
        }
        Ok(fs::read(path)?)
    }

    pub fn write(&self, relative: &str, contents: &str) -> Result<(), FileError> {
        if contents.len() > LARGEST_FILE_IN_BYTES {
            return Err(FileError::TooLarge { bytes: contents.len() });
        }
        if !may_write_in_plugin(Path::new(relative)) {
            return Err(FileError::NotAllowed(writing_rules()));
        }
        let path = contained_path(&self.root, relative)?;
        refuse_links(&path)?;
        if let Some(folder) = path.parent() {
            fs::create_dir_all(folder)?;
        }
        Ok(fs::write(path, contents)?)
    }
}

/// MP Forge's own guides, and Akai's documentation, framework headers and example plugins. Read-only.
pub struct ReferenceLibrary {
    root: PathBuf,
}

impl ReferenceLibrary {
    pub fn open(sdk_root: &Path) -> io::Result<Self> {
        Ok(Self { root: true_path_of(sdk_root)? })
    }

    pub fn list(&self) -> io::Result<Vec<String>> {
        let mut listed: Vec<String> = OWN_GUIDES.iter().map(|guide| guide.path.to_string()).collect();
        for folder in REFERENCE_FOLDERS {
            let within_folder = relative_files_under(&self.root.join(folder))?;
            listed.extend(within_folder.into_iter().map(|path| format!("{folder}/{path}")));
        }
        Ok(listed.into_iter().filter(|path| own_guide_at(path).is_some() || is_reference(Path::new(path))).collect())
    }

    pub fn read(&self, relative: &str) -> Result<String, FileError> {
        if let Some(guide) = own_guide_at(relative) {
            return Ok(guide.text.to_string());
        }
        let path = contained_path(&self.root, relative)?;
        if !is_reference(Path::new(relative)) {
            return Err(FileError::NotAllowed(format!("`{relative}` is not in the reference library.")));
        }
        Ok(fs::read_to_string(path)?)
    }
}

fn may_write_in_plugin(relative: &Path) -> bool {
    let named_file = FILES_THE_AI_WRITES.iter().any(|allowed| relative == Path::new(allowed));
    named_file || is_source_file(relative) || is_drawing(relative)
}

/// The AI may read the look, so it can work around what the musician did by hand, but never write the file.
/// It changes the look only through its styling tool, which checks every change.
fn may_read_in_plugin(relative: &Path) -> bool {
    let fixed_file = relative == Path::new(PLUGIN_MANIFEST) || relative == Path::new(LOOK_FILE);
    fixed_file || may_write_in_plugin(relative) || is_attachment(relative)
}

fn is_source_file(relative: &Path) -> bool {
    relative.starts_with(SOURCE_FOLDER) && has_extension(relative, &SOURCE_EXTENSIONS)
}

fn is_drawing(relative: &Path) -> bool {
    relative.parent() == Some(Path::new(DRAWINGS_FOLDER)) && has_extension(relative, &[DRAWING_EXTENSION])
}

fn own_guide_at(path: &str) -> Option<&'static OwnGuide> {
    OWN_GUIDES.iter().find(|guide| guide.path == path)
}

fn is_reference(relative: &Path) -> bool {
    let in_library = REFERENCE_FOLDERS.iter().any(|folder| relative.starts_with(folder));
    let is_build_output = relative.components().any(|part| part.as_os_str() == BUILD_OUTPUT_FOLDER);
    in_library && !is_build_output && has_extension(relative, &REFERENCE_EXTENSIONS)
}

fn has_extension(path: &Path, allowed: &[&str]) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| allowed.contains(&extension))
}

fn writing_rules() -> String {
    format!(
        "You can write {}, source files in {SOURCE_FOLDER}/ ending in .{}, and drawings in {DRAWINGS_FOLDER}/ ending in .{DRAWING_EXTENSION}. \
         {PLUGIN_MANIFEST} is fixed.",
        FILES_THE_AI_WRITES.join(", "),
        SOURCE_EXTENSIONS.join(", ."),
    )
}

/// Joins a path the AI named onto `root`, refusing anything that could land outside it.
fn contained_path(root: &Path, relative: &str) -> Result<PathBuf, FileError> {
    let outside = || FileError::OutsideAllowedArea(relative.to_string());
    let only_plain_names = Path::new(relative).components().all(|part| matches!(part, Component::Normal(_)));
    if relative.is_empty() || !only_plain_names {
        return Err(outside());
    }
    let path = root.join(relative);
    if true_path_of(nearest_existing_ancestor(&path))?.starts_with(root) { Ok(path) } else { Err(outside()) }
}

/// A link inside the plugin could point anywhere, so the AI never writes through one.
fn refuse_links(path: &Path) -> Result<(), FileError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(FileError::NotAllowed("That file is a link to somewhere else, so it cannot be written.".to_string()))
        }
        _ => Ok(()),
    }
}

fn nearest_existing_ancestor(path: &Path) -> &Path {
    path.ancestors().find(|ancestor| ancestor.exists()).unwrap_or(path)
}

fn relative_files_under(folder: &Path) -> io::Result<Vec<String>> {
    let mut files = Vec::new();
    collect_files(folder, folder, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(base: &Path, folder: &Path, files: &mut Vec<String>) -> io::Result<()> {
    if !folder.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(base, &path, files)?;
        } else if let Ok(relative) = path.strip_prefix(base) {
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

#[cfg(test)]
pub mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A folder that exists for one test and is removed afterwards.
    pub struct TemporaryFolder(pub PathBuf);

    impl TemporaryFolder {
        pub fn create(label: &str) -> Self {
            static NEXT_FOLDER: AtomicU64 = AtomicU64::new(0);
            let unique = format!("workbench-{label}-{}-{}", std::process::id(), NEXT_FOLDER.fetch_add(1, Ordering::Relaxed));
            let path = std::env::temp_dir().join(unique);
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TemporaryFolder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TemporaryFolder;
    use super::*;

    #[test]
    fn the_mark_windows_puts_on_a_whole_path_is_taken_off() {
        assert_eq!(without_the_windows_mark(Path::new(r"\\?\C:\Users\me\plugins\Dusty Tape")), PathBuf::from(r"C:\Users\me\plugins\Dusty Tape"));
        assert_eq!(without_the_windows_mark(Path::new(r"\\?\UNC\studio\shared\plugins")), PathBuf::from(r"\\studio\shared\plugins"));
    }

    #[test]
    fn a_path_without_the_mark_is_left_as_it_is() {
        assert_eq!(without_the_windows_mark(Path::new(r"D:\plugins")), PathBuf::from(r"D:\plugins"));
        assert_eq!(without_the_windows_mark(Path::new("/home/me/plugins")), PathBuf::from("/home/me/plugins"));
    }

    #[test]
    fn the_true_path_of_a_folder_is_one_any_program_can_read() {
        let folder = TemporaryFolder::create("true-path");

        let true_path = true_path_of(&folder.0).unwrap();

        assert!(!true_path.to_string_lossy().starts_with(WINDOWS_MARK_OF_A_WHOLE_PATH), "{}", true_path.display());
        assert!(true_path.is_dir());
    }

    fn plugin_with_manifest() -> (TemporaryFolder, PluginWorkspace) {
        let folder = TemporaryFolder::create("plugin");
        fs::write(folder.0.join(PLUGIN_MANIFEST), "{}").unwrap();
        let workspace = PluginWorkspace::open(&folder.0).unwrap();
        (folder, workspace)
    }

    #[test]
    fn source_files_and_the_screen_layout_can_be_written_and_read_back() {
        let (_folder, plugin) = plugin_with_manifest();

        plugin.write("src/Delay.cpp", "// delay").unwrap();
        plugin.write("ui.json", "{}").unwrap();

        assert_eq!(plugin.read("src/Delay.cpp").unwrap(), "// delay");
        assert_eq!(plugin.list().unwrap(), ["plugin.json", "src/Delay.cpp", "ui.json"]);
    }

    #[test]
    fn pictures_and_their_drawings_can_be_written_but_never_the_drawn_images() {
        let (_folder, plugin) = plugin_with_manifest();

        plugin.write("pictures.json", "{}").unwrap();
        plugin.write("pictures/reels.svg", "<svg/>").unwrap();

        assert_eq!(plugin.list().unwrap(), ["pictures.json", "pictures/reels.svg", "plugin.json"]);
        for forbidden in ["pictures/reels.png", "pictures/deep/reels.svg", "assets/pictures/reels.png", "skins/TUI.json", "reels.svg"] {
            assert!(matches!(plugin.write(forbidden, "x"), Err(FileError::NotAllowed(_))), "`{forbidden}`");
        }
    }

    #[test]
    fn what_the_musician_arranged_by_hand_cannot_be_overwritten() {
        let (_folder, plugin) = plugin_with_manifest();

        assert!(matches!(plugin.write("look.json", "{\"pages\": []}"), Err(FileError::NotAllowed(_))));
    }

    #[test]
    fn the_ai_can_read_the_musicians_look_but_none_of_their_artwork() {
        let (folder, plugin) = plugin_with_manifest();
        fs::write(folder.0.join("look.json"), "{\"pages\": []}").unwrap();
        fs::create_dir_all(folder.0.join("artwork")).unwrap();
        fs::write(folder.0.join("artwork/art-1.png"), "png").unwrap();

        assert_eq!(plugin.read("look.json").unwrap(), "{\"pages\": []}");
        assert!(matches!(plugin.read("artwork/art-1.png"), Err(FileError::NotAllowed(_))));
        assert!(matches!(plugin.write("artwork/art-1.png", "x"), Err(FileError::NotAllowed(_))));
    }

    #[test]
    fn the_ai_keeps_its_plan_for_the_look_in_its_own_notes() {
        let plugin_dir = TemporaryFolder::create("design-notes");
        let plugin = PluginWorkspace::open(&plugin_dir.0).unwrap();

        plugin.write("DESIGN.md", "A worn tape machine.").unwrap();

        assert_eq!(plugin.read("DESIGN.md").unwrap(), "A worn tape machine.");
        assert!(plugin.write("NOTES.md", "anything").is_err());
    }

    #[test]
    fn the_manifest_can_be_read_but_not_changed() {
        let (_folder, plugin) = plugin_with_manifest();

        assert_eq!(plugin.read(PLUGIN_MANIFEST).unwrap(), "{}");
        assert!(matches!(plugin.write(PLUGIN_MANIFEST, "{\"uid\":\"evil\"}"), Err(FileError::NotAllowed(_))));
    }

    #[test]
    fn paths_that_climb_out_of_the_plugin_are_refused() {
        let (_folder, plugin) = plugin_with_manifest();

        for escape in ["../outside.cpp", "src/../../outside.cpp", "/etc/passwd", ""] {
            assert!(plugin.write(escape, "x").is_err(), "writing `{escape}` should be refused");
            assert!(plugin.read(escape).is_err(), "reading `{escape}` should be refused");
        }
    }

    #[test]
    fn only_plugin_file_types_can_be_written() {
        let (_folder, plugin) = plugin_with_manifest();

        for forbidden in ["autorun.sh", "src/run.sh", "build/native/x.dylib", "Content/a.wav", "src/notes.txt"] {
            assert!(matches!(plugin.write(forbidden, "x"), Err(FileError::NotAllowed(_))), "`{forbidden}`");
        }
    }

    #[test]
    fn oversized_files_are_refused() {
        let (_folder, plugin) = plugin_with_manifest();
        let huge = "x".repeat(LARGEST_FILE_IN_BYTES + 1);

        assert!(matches!(plugin.write("src/Big.cpp", &huge), Err(FileError::TooLarge { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_out_of_the_plugin_cannot_be_followed() {
        let (folder, plugin) = plugin_with_manifest();
        let elsewhere = TemporaryFolder::create("elsewhere");
        std::os::unix::fs::symlink(&elsewhere.0, folder.0.join(SOURCE_FOLDER)).unwrap();

        let result = plugin.write("src/Escape.cpp", "x");

        assert!(matches!(result, Err(FileError::OutsideAllowedArea(_))));
        assert!(!elsewhere.0.join("Escape.cpp").exists());
    }

    #[test]
    fn the_reference_library_offers_documents_but_not_tools_or_build_output() {
        let sdk = TemporaryFolder::create("sdk");
        for file in ["docs/overview.md", "framework/mpc_plugin.h", "tools/mpc_firmware.py", "examples/A/build/x.json"] {
            let path = sdk.0.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "text").unwrap();
        }
        let library = ReferenceLibrary::open(&sdk.0).unwrap();

        assert_eq!(library.list().unwrap(), ["mp-forge/pictures.md", "docs/overview.md", "framework/mpc_plugin.h"]);
        assert_eq!(library.read("docs/overview.md").unwrap(), "text");
        assert!(library.read("tools/mpc_firmware.py").is_err());
        assert!(library.read("../anything.md").is_err());
    }

    #[test]
    fn the_reference_library_holds_mp_forges_own_guide_to_pictures_with_the_picture_clock() {
        let sdk = TemporaryFolder::create("sdk-own-guides");
        let library = ReferenceLibrary::open(&sdk.0).unwrap();

        let guide = library.read("mp-forge/pictures.md").unwrap();

        assert!(guide.contains("{ \"pictures\": ["));
        assert!(guide.contains("void turnPictureClock(const mpc::ProcessContext& ctx)"));
        assert!(guide.contains("a knob's dial is the circle centred at (x + 53, y + 53)"));
        assert!(library.read("mp-forge/another.md").is_err());
    }
}
