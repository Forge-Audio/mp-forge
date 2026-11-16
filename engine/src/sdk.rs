//! The pinned copy of Akai's MPC Plugin SDK: where it lives and how its tools are run.

use std::env::consts::EXE_SUFFIX;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::built_screen::SizedArt;
use crate::own_python::OwnPython;
use crate::process::{Finished, RunError, command_for, run_with_time_limit};
use crate::simulation::Session;

/// What the computer calls a Python that was installed on it.
const PYTHON_OF_THE_COMPUTER: &str = if cfg!(windows) { "python" } else { "python3" };
/// Has Python read and write its words as UTF-8, as it does on a Mac and on Linux. Left to itself on Windows, a
/// Python that is not printing to a console writes in the computer's old code page, and stops with an error at
/// the first letter that page lacks, such as the arrow in the notes Akai's tool prints after an install.
const WRITES_EVERY_LETTER: [(&str, &str); 2] = [("PYTHONUTF8", "1"), ("PYTHONIOENCODING", "utf-8")];
const BUILD_SCRIPT: &str = "tools/build.py";
const NEW_PLUGIN_SCRIPT: &str = "tools/new_plugin.py";
const VALIDATE_SCRIPT: &str = "tools/validate.py";
const TOOLS_FOLDER: &str = "tools";
const KEPT_TOOLS_FOLDER: &str = ".sdk-tools";
const BUILD_FOLDER: &str = "build";
const BUILT_PARAMETERS: &str = "build/params.json";
const SCREEN_LAYOUT: &str = "ui.json";
/// Where resized kit images are kept between builds, because drawing one takes seconds.
const SIZED_ART_CACHE: &str = "build/look-art";
/// Akai's tools name folders by the way to them from where they were started, and on Windows there is no such
/// way between two drives. Run through this script, a tool names a folder on another drive in full instead.
const RUNS_A_TOOL_ACROSS_DRIVES: &str = include_str!("akai_tool_across_drives.py");
/// The workbench's own script that draws kit images at other sizes with Akai's drawing code.
const SIZED_ART_SCRIPT: &str = include_str!("sized_kit_art.py");
const SCREEN_FILE_IN_BUILT_PLUGIN: &str = "Plugin Skins/TUI.json";
const NATIVE_LIBRARY_EXTENSIONS: [&str; 3] = ["dylib", "so", "dll"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginKind {
    Effect,
    Instrument,
}

impl PluginKind {
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "effect" => Some(PluginKind::Effect),
            "instrument" => Some(PluginKind::Instrument),
            _ => None,
        }
    }

    pub fn as_word(self) -> &'static str {
        match self {
            PluginKind::Effect => "effect",
            PluginKind::Instrument => "instrument",
        }
    }
}

/// Who a plugin is. Fixed when the plugin is created, because projects saved on an MPC refer to it.
#[derive(Debug, Clone)]
pub struct PluginIdentity {
    pub name: String,
    pub maker: String,
    pub kind: PluginKind,
}

#[derive(Clone)]
pub struct Sdk {
    root: PathBuf,
}

#[derive(Debug)]
pub struct SdkNotFound(pub PathBuf);

impl fmt::Display for SdkNotFound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Akai's SDK was not found at {}. Run scripts/check-sdk.sh to fetch it.",
            self.0.display()
        )
    }
}

impl Sdk {
    pub fn at(root: impl Into<PathBuf>) -> Result<Self, SdkNotFound> {
        let root = root.into();
        if root.join(BUILD_SCRIPT).is_file() { Ok(Self { root }) } else { Err(SdkNotFound(root)) }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Akai's own scripts.
    pub(crate) fn tools_folder(&self) -> PathBuf {
        self.root.join(TOOLS_FOLDER)
    }

    /// Where Akai's build script keeps what it fetched or built for itself: its compiler and its tryout program.
    pub(crate) fn kept_tools_folder(&self) -> PathBuf {
        self.root.join(KEPT_TOOLS_FOLDER)
    }

    /// The Python that runs Akai's tools: MP Forge's own once it has fetched one, and the computer's until then.
    pub(crate) fn python(&self) -> Command {
        let own = self.own_python();
        let mut python = if own.is_there() { command_for(own.program()) } else { command_for(PYTHON_OF_THE_COMPUTER) };
        python.envs(WRITES_EVERY_LETTER);
        python
    }

    pub(crate) fn own_python(&self) -> OwnPython {
        OwnPython::kept_in(&self.kept_tools_folder())
    }

    /// Creates a working starter plugin from Akai's template, in its own folder under `plugins_dir`.
    pub fn create_plugin(&self, identity: &PluginIdentity, plugins_dir: &Path, limit: Duration) -> Result<Finished, RunError> {
        let mut command = self.tool_that_names_folders(NEW_PLUGIN_SCRIPT);
        command.arg(&identity.name);
        command.arg("--vendor").arg(&identity.maker);
        command.arg("--kind").arg(identity.kind.as_word());
        command.arg("--dir").arg(plugins_dir);
        run_with_time_limit(&mut command, limit)
    }

    /// Builds the plugin for both MPC chip types and for this computer, then validates the result.
    pub fn build(&self, plugin: &Path, limit: Duration) -> Result<Finished, RunError> {
        let mut command = self.tool_that_names_folders(BUILD_SCRIPT);
        command.arg(plugin);
        run_with_time_limit(&mut command, limit)
    }

    /// One of Akai's tools that names folders in what it says, started so that the plugins may be kept on
    /// another drive than the SDK.
    fn tool_that_names_folders(&self, script: &str) -> Command {
        let mut command = self.python();
        command.arg("-c").arg(RUNS_A_TOOL_ACROSS_DRIVES).arg(self.root.join(script));
        command
    }

    /// Checks the finished plugin folder the build left behind, as Akai's own validator sees it.
    pub fn validate(&self, plugin: &Path, limit: Duration) -> Result<Finished, RunError> {
        let mut command = self.python();
        command.arg(self.root.join(VALIDATE_SCRIPT)).arg(plugin);
        run_with_time_limit(&mut command, limit)
    }

    /// Draws images of the control kit again at other sizes, beside the screen file the build made.
    /// Akai's own drawing code does the drawing, in the colours the plugin's layout asks for.
    pub fn draw_sized_art(&self, plugin: &Path, art: &[SizedArt], limit: Duration) -> Result<Finished, RunError> {
        let beside_the_screen = built_screen_file(plugin).and_then(|screen_file| screen_file.parent().map(Path::to_path_buf));
        let mut command = self.python();
        command.arg("-c").arg(SIZED_ART_SCRIPT).arg(self.root.join(TOOLS_FOLDER));
        command.arg(plugin.join(BUILT_PARAMETERS)).arg(plugin.join(SCREEN_LAYOUT));
        command.arg(beside_the_screen.unwrap_or_else(|| plugin.join(BUILD_FOLDER))).arg(plugin.join(SIZED_ART_CACHE));
        command.args(art.iter().map(|sized| format!("{}:{}:{}:{}:{}", sized.kit_image, sized.width, sized.height, sized.frames, sized.file_name())));
        run_with_time_limit(&mut command, limit)
    }

    /// Plays the plugin the way an MPC would, under the given conditions.
    pub fn simulate(&self, library: &Path, session: &Session, limit: Duration) -> Result<Finished, RunError> {
        run_with_time_limit(&mut self.simulator_command(library, session), limit)
    }

    /// The same, and also writes what the plugin played to a sound file.
    pub fn record(&self, library: &Path, session: &Session, sound_file: &Path, limit: Duration) -> Result<Finished, RunError> {
        let mut command = self.simulator_command(library, session);
        command.arg("--wav").arg(sound_file);
        run_with_time_limit(&mut command, limit)
    }

    fn simulator_command(&self, library: &Path, session: &Session) -> Command {
        let mut command = command_for(self.host_simulator());
        command.arg(library).arg("test").args(session.simulator_arguments());
        command
    }

    fn host_simulator(&self) -> PathBuf {
        self.kept_tools_folder().join(format!("host_sim{EXE_SUFFIX}"))
    }
}

/// The copy of the plugin built for this computer, which is what the simulator loads.
pub fn native_library_of(plugin: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(plugin.join("build/native")).ok()?;
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| is_native_library(path))
}

/// One of the images the build put beside the screen file. Only a plain PNG file name is looked up.
pub fn built_skin_image(plugin: &Path, file_name: &str) -> Option<Vec<u8>> {
    let plain = |character: char| character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_';
    let stem = file_name.strip_suffix(".png")?;
    if stem.is_empty() || !stem.chars().all(plain) {
        return None;
    }
    fs::read(built_screen_file(plugin)?.with_file_name(file_name)).ok()
}

/// The screen file inside the finished plugin folder, which the build names after the maker and the plugin.
pub fn built_screen_file(plugin: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(plugin.join(BUILD_FOLDER)).ok()?;
    entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path().join(SCREEN_FILE_IN_BUILT_PLUGIN)))
        .find(|screen_file| screen_file.is_file())
}

fn is_native_library(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| NATIVE_LIBRARY_EXTENSIONS.contains(&extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::test_support::TemporaryFolder;

    #[test]
    fn only_a_plainly_named_image_beside_the_built_screen_is_handed_over() {
        let plugin = TemporaryFolder::create("skin-image");
        let skins = plugin.0.join("build/Tests - VST - Reels/Plugin Skins");
        fs::create_dir_all(&skins).unwrap();
        fs::write(skins.join("TUI.json"), "{}").unwrap();
        fs::write(skins.join("knob_uni.png"), "knob").unwrap();
        fs::write(plugin.0.join("build/secret.png"), "secret").unwrap();

        assert_eq!(built_skin_image(&plugin.0, "knob_uni.png"), Some(b"knob".to_vec()));
        assert_eq!(built_skin_image(&plugin.0, "../secret.png"), None);
        assert_eq!(built_skin_image(&plugin.0, "TUI.json"), None);
        assert_eq!(built_skin_image(&plugin.0, "fader_h_uni.png"), None);
    }
}
