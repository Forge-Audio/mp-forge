//! Making a plugin from a description: create the starter, let the AI work, then judge the result.
//!
//! The verdict belongs to the workbench. Whatever the AI says when it stops, the gate is run again
//! here, and a plugin that does not pass is handed back to the AI or reported as not ready.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

use crate::agent::{Agent, AgentError, Event, Model, ModelError, Worker};
use crate::ai_settings::AiSettings;
use crate::arranged_screen::ArrangedPage;
use crate::art_shelf::{ArtError, ArtShelf};
use crate::attachments::{AttachmentError, AttachmentShelf};
use crate::claude_code::{ClaudeCode, ToolServer};
use crate::codex::{Codex, CodexChoices};
use crate::conversation;
use crate::delivery::{self, DeliveryError, Mpc};
use crate::gate::{Gate, GateReport};
use crate::history::{self, Version};
use crate::instructions;
use crate::kept_look::{self, NotKept};
use crate::look::{self, Look};
use crate::other_looks::note_on_other_looks;
use crate::plugin_name;
use crate::plugin_tools::PluginTools;
use crate::process::RunError;
use crate::samples::{Sample, SampleError, SampleShelf};
use crate::sdk::{PluginIdentity, Sdk, native_library_of};
use crate::stop_signal::StopSignal;
use crate::simulation::Session;
use crate::workspace::true_path_of;

const DEFAULT_REPAIR_ROUNDS: usize = 2;
const LONGEST_NAME: usize = 32;
const DEVICES_WINDOWS_NAMES: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];
/// `COM1` to `COM9` and `LPT1` to `LPT9` are kept too.
const PORTS_WINDOWS_NUMBERS: [&str; 2] = ["COM", "LPT"];
const LONGEST_VERSION_NOTE: usize = 80;
const STARTER_TIME_LIMIT: Duration = Duration::from_secs(60);
const PLUGIN_MANIFEST: &str = "plugin.json";
/// Where sound files wait for a plugin that has not been made yet. Its name keeps it out of the list of plugins.
const WAITING_SAMPLES_FOLDER: &str = ".waiting-samples";
/// Where pictures for the AI wait in the same way.
const WAITING_ATTACHMENTS_FOLDER: &str = ".waiting-attachments";
/// The workbench command that serves the AI's tools to an outside agent program.
pub const SERVE_TOOLS_COMMAND: &str = "serve-tools";

const PREVIEW_SOUND_FILE: &str = "build/preview.wav";
const PREVIEW_TIME_LIMIT: Duration = Duration::from_secs(60);

pub struct PluginRequest {
    pub identity: PluginIdentity,
    pub description: String,
}

/// Which AI does the work, and how it reaches the tools.
pub enum Connection<'a> {
    /// A model the workbench talks to directly. The workbench holds the tools itself.
    Model(&'a mut dyn Model),
    /// The user's own Claude Code, which reaches the tools by starting `tool_server_program`.
    ClaudeCode { tool_server_program: &'a Path, settings: AiSettings },
    /// The user's own Codex, signed in with their ChatGPT account, which reaches the tools the same way.
    Codex { tool_server_program: &'a Path, choices: CodexChoices },
}

/// How an attempt to send a plugin to an MPC ended. Either way the gate's report comes with it.
pub enum Sending {
    /// It is on the MPC, on the removable drive named here.
    Sent { report: GateReport, onto_drive: String },
    HeldBack(GateReport),
}

pub struct Delivery {
    pub plugin_dir: PathBuf,
    pub closing_words: String,
    pub report: GateReport,
}

impl Delivery {
    pub fn is_ready(&self) -> bool {
        self.report.may_be_sent()
    }
}

#[derive(Debug)]
pub enum MakeError {
    UnusableName(String),
    AlreadyExists(PathBuf),
    StarterNotCreated(String),
    NoSuchPlugin(String),
    NotBuiltYet(String),
    NotSignedIn(String),
    /// The musician stopped the work. Whatever had been changed by then is kept.
    Stopped,
    Agent { plugin_dir: PathBuf, error: AgentError },
    Sample(SampleError),
    Attachment(AttachmentError),
    Art(ArtError),
    /// What the musician arranged by hand cannot go on the screen, for these reasons.
    LookDoesNotFit(Vec<String>),
    NameTaken(String),
    /// The plugin could not be moved to the Trash, so it is still where it was.
    NotDeleted(String),
    NotSent(DeliveryError),
    Io(io::Error),
}

impl fmt::Display for MakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MakeError::UnusableName(name) => write!(
                formatter,
                "`{name}` cannot be used. Names start with a letter, use only letters, digits, spaces and hyphens, \
                 and are at most {LONGEST_NAME} characters."
            ),
            MakeError::AlreadyExists(folder) => write!(formatter, "There is already a plugin at {}.", folder.display()),
            MakeError::StarterNotCreated(cause) => write!(formatter, "The starter plugin could not be created: {cause}"),
            MakeError::NoSuchPlugin(name) => write!(formatter, "There is no plugin called `{name}`."),
            MakeError::NotBuiltYet(name) => write!(formatter, "`{name}` has no working build to play yet. Run the safety check first."),
            MakeError::NotSignedIn(how_to_fix) => write!(formatter, "{how_to_fix} Nothing was changed."),
            MakeError::Stopped => write!(formatter, "Stopped. Whatever was changed up to then has been kept, and GO BACK returns to the version before."),
            MakeError::Agent { plugin_dir, error } => {
                write!(formatter, "{error} The unfinished plugin is in {}.", plugin_dir.display())
            }
            MakeError::Sample(problem) => write!(formatter, "{problem}"),
            MakeError::Attachment(problem) => write!(formatter, "{problem}"),
            MakeError::Art(problem) => write!(formatter, "{problem}"),
            MakeError::LookDoesNotFit(problems) => write!(formatter, "{}", problems.join(" ")),
            MakeError::NameTaken(name) => write!(formatter, "There is already a plugin called `{name}`. Pick another name."),
            MakeError::NotDeleted(cause) => write!(formatter, "The plugin could not be moved to the Trash, so nothing was deleted: {cause}"),
            MakeError::NotSent(problem) => write!(formatter, "{problem}"),
            MakeError::Io(cause) => write!(formatter, "A file could not be used: {cause}"),
        }
    }
}

impl From<AttachmentError> for MakeError {
    fn from(problem: AttachmentError) -> Self {
        MakeError::Attachment(problem)
    }
}

impl From<DeliveryError> for MakeError {
    fn from(problem: DeliveryError) -> Self {
        MakeError::NotSent(problem)
    }
}

impl From<ArtError> for MakeError {
    fn from(problem: ArtError) -> Self {
        MakeError::Art(problem)
    }
}

impl From<SampleError> for MakeError {
    fn from(problem: SampleError) -> Self {
        MakeError::Sample(problem)
    }
}

impl From<io::Error> for MakeError {
    fn from(cause: io::Error) -> Self {
        MakeError::Io(cause)
    }
}

pub struct Maker {
    sdk: Sdk,
    gate: Gate,
    plugins_dir: PathBuf,
    repair_rounds: usize,
    /// How many tokens the AI's conversation may reach before older material is dropped from it.
    compact_at: Option<u64>,
    stop: StopSignal,
}

impl Maker {
    pub fn new(sdk: Sdk, plugins_dir: impl Into<PathBuf>) -> Self {
        Self { gate: Gate::new(sdk.clone()), sdk, plugins_dir: plugins_dir.into(), repair_rounds: DEFAULT_REPAIR_ROUNDS, compact_at: None, stop: StopSignal::default() }
    }

    pub fn with_repair_rounds(mut self, repair_rounds: usize) -> Self {
        self.repair_rounds = repair_rounds;
        self
    }

    /// Without a limit, each AI service decides for itself when its conversation is full.
    pub fn compacting_at(mut self, tokens: Option<u64>) -> Self {
        self.compact_at = tokens;
        self
    }

    /// The AI's work is given up as soon as this signal asks for it.
    pub fn stopped_by(mut self, stop: StopSignal) -> Self {
        self.stop = stop;
        self
    }

    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Makes a new plugin from a description.
    pub fn make(&self, request: &PluginRequest, connection: Connection, observer: impl FnMut(Event)) -> Result<Delivery, MakeError> {
        let plugin_dir = self.create_starter(&request.identity)?;
        let plugin_shelf = SampleShelf::of_plugin(&plugin_dir);
        self.waiting_shelf(&request.identity.name).move_all_to(&plugin_shelf)?;
        self.waiting_attachments(&request.identity.name).move_all_to(&AttachmentShelf::of_plugin(&plugin_dir))?;
        let first_message = opening_message(request) + &sound_files_note(&plugin_shelf.list()) + &note_on_other_looks(&self.plugins_dir, &request.identity.name);
        let outcome = self.work_and_judge(plugin_dir.clone(), &first_message, connection, observer);
        if matches!(outcome, Err(MakeError::NotSignedIn(_))) {
            discard_untouched_starter(&plugin_dir);
        }
        outcome
    }

    /// Changes a plugin that already exists. Its files are kept as a version first, so the change can be undone.
    pub fn revise(&self, plugin_name: &str, change: &str, connection: Connection, observer: impl FnMut(Event)) -> Result<Delivery, MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        history::keep_version(&plugin_dir, &format!("the change \u{201c}{}\u{201d}", opening_words_of(change)))?;
        let first_message = revision_message(plugin_name, change) + &sound_files_note(&SampleShelf::of_plugin(&plugin_dir).list());
        self.work_and_judge(plugin_dir, &first_message, connection, observer)
    }

    /// Keeps a sound file for the plugin to play. For a plugin that is not made yet, the file waits until it is.
    pub fn add_sample(&self, plugin_name: &str, file_name: &str, bytes: &[u8]) -> Result<Sample, MakeError> {
        Ok(self.sample_shelf(plugin_name)?.add(file_name, bytes)?)
    }

    pub fn remove_sample(&self, plugin_name: &str, file_name: &str) -> Result<(), MakeError> {
        Ok(self.sample_shelf(plugin_name)?.remove(file_name)?)
    }

    pub fn samples(&self, plugin_name: &str) -> Result<Vec<Sample>, MakeError> {
        Ok(self.sample_shelf(plugin_name)?.list())
    }

    /// Keeps a picture for the AI to look at and returns the name it was kept under. For a plugin that is
    /// not made yet, the picture waits until it is.
    pub fn add_attachment(&self, plugin_name: &str, png: &[u8]) -> Result<String, MakeError> {
        let shelf = match self.existing_plugin(plugin_name) {
            Ok(plugin_dir) => AttachmentShelf::of_plugin(&plugin_dir),
            Err(MakeError::NoSuchPlugin(_)) => self.waiting_attachments(plugin_name),
            Err(other) => return Err(other),
        };
        Ok(shelf.add(png)?)
    }

    fn waiting_attachments(&self, plugin_name: &str) -> AttachmentShelf {
        AttachmentShelf::at(self.plugins_dir.join(WAITING_ATTACHMENTS_FOLDER).join(plugin_name))
    }

    fn sample_shelf(&self, plugin_name: &str) -> Result<SampleShelf, MakeError> {
        match self.existing_plugin(plugin_name) {
            Ok(plugin_dir) => Ok(SampleShelf::of_plugin(&plugin_dir)),
            Err(MakeError::NoSuchPlugin(_)) => Ok(self.waiting_shelf(plugin_name)),
            Err(other) => Err(other),
        }
    }

    fn waiting_shelf(&self, plugin_name: &str) -> SampleShelf {
        SampleShelf::at(self.plugins_dir.join(WAITING_SAMPLES_FOLDER).join(plugin_name))
    }

    /// Plays the built plugin in the simulator and returns the sound file it wrote.
    pub fn record_preview(&self, plugin_name: &str) -> Result<PathBuf, MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        let library = native_library_of(&plugin_dir).ok_or_else(|| MakeError::NotBuiltYet(plugin_name.to_string()))?;
        let sound_file = plugin_dir.join(PREVIEW_SOUND_FILE);
        let recorded = self.sdk.record(&library, &Session::PREVIEW, &sound_file, PREVIEW_TIME_LIMIT);
        match recorded {
            Ok(finished) if finished.succeeded() && sound_file.is_file() => Ok(sound_file),
            _ => Err(MakeError::NotBuiltYet(plugin_name.to_string())),
        }
    }

    /// What has been said about the plugin, oldest first, as the screen last kept it.
    pub fn conversation(&self, plugin_name: &str) -> Result<Vec<Value>, MakeError> {
        Ok(conversation::kept(&self.existing_plugin(plugin_name)?))
    }

    pub fn keep_conversation(&self, plugin_name: &str, messages: &[Value]) -> Result<(), MakeError> {
        Ok(conversation::keep(&self.existing_plugin(plugin_name)?, messages)?)
    }

    /// The last few messages about the plugin, worded for the AI to read with a new request.
    pub fn earlier_talk(&self, plugin_name: &str) -> Result<String, MakeError> {
        Ok(conversation::earlier_talk(&self.existing_plugin(plugin_name)?))
    }

    /// Every version of the plugin that has been kept, oldest first.
    pub fn versions(&self, plugin_name: &str) -> Result<Vec<Version>, MakeError> {
        Ok(history::versions(&self.existing_plugin(plugin_name)?))
    }

    /// Puts the plugin back as one kept version holds it. Returns false when there is no such version.
    pub fn go_to_version(&self, plugin_name: &str, number: usize) -> Result<bool, MakeError> {
        Ok(history::go_to(&self.existing_plugin(plugin_name)?, number)?)
    }

    /// Throws one kept version away for good. Returns false when there is no version with that number.
    pub fn forget_version(&self, plugin_name: &str, number: usize) -> Result<bool, MakeError> {
        Ok(history::forget_version(&self.existing_plugin(plugin_name)?, number)?)
    }

    /// Folds the plugin's oldest versions away so at most `most` are left. Returns how many went.
    pub fn fold_old_versions(&self, plugin_name: &str, most: usize) -> Result<usize, MakeError> {
        Ok(history::keep_only_newest(&self.existing_plugin(plugin_name)?, most)?)
    }

    /// Sends the plugin to a removable drive of an MPC, but only once the gate has passed it as it stands right now.
    /// A plugin the gate stops is held back, and the report says where it was stopped.
    pub fn send_to_mpc(&self, plugin_name: &str, mpc: &Mpc) -> Result<Sending, MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        let report = self.gate.inspect(&plugin_dir);
        if !report.may_be_sent() {
            return Ok(Sending::HeldBack(report));
        }
        let onto_drive = delivery::install(&self.sdk, &plugin_dir, mpc)?;
        Ok(Sending::Sent { report, onto_drive })
    }

    /// Restarts the MPC's app so it finds a plugin just sent. Anything unsaved on the MPC is lost, so this is
    /// only for when the musician has just said yes to it.
    pub fn restart_mpc_app(&self, mpc: &Mpc) -> Result<(), MakeError> {
        Ok(delivery::restart_app(&self.sdk, mpc)?)
    }

    /// Gets ready for the musician to edit the look by hand. Pictures no look uses any more are cleared
    /// away, and a version is kept, so going back undoes everything done in the visit.
    pub fn begin_editing(&self, plugin_name: &str) -> Result<(), MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        let look = look::look_of(&plugin_dir).unwrap_or_default();
        ArtShelf::of_plugin(&plugin_dir).keep_only(&look.art_files())?;
        history::keep_version(&plugin_dir, "the look was edited by hand")?;
        Ok(())
    }

    /// Keeps a picture the musician brought in to use as artwork, and returns the name it was kept under.
    pub fn add_artwork(&self, plugin_name: &str, png: &[u8]) -> Result<String, MakeError> {
        Ok(ArtShelf::of_plugin(&self.existing_plugin(plugin_name)?).add(png)?)
    }

    /// Keeps what the musician did by hand, once it is known to fit the screen. Controls they put on a
    /// page or took off one since the look before are written into the screen's layout as well.
    /// Returns the screen as it is now arranged. A look that does not fit is refused and nothing is kept.
    pub fn keep_look(&self, plugin_name: &str, look: &Look) -> Result<Vec<ArrangedPage>, MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        kept_look::keep_if_it_fits(&plugin_dir, look).map_err(|not_kept| match not_kept {
            NotKept::DoesNotFit(problems) => MakeError::LookDoesNotFit(problems),
            NotKept::CouldNotBeWritten(error) => MakeError::from(error),
        })
    }

    /// Gives a plugin another name. Its build is thrown away, so it needs its safety check again.
    pub fn rename_plugin(&self, plugin_name: &str, new_name: &str) -> Result<(), MakeError> {
        self.existing_plugin(plugin_name)?;
        require_usable_name(new_name)?;
        let only_its_capitals_change = new_name.eq_ignore_ascii_case(plugin_name);
        if self.plugins_dir.join(new_name).exists() && !only_its_capitals_change {
            return Err(MakeError::NameTaken(new_name.to_string()));
        }
        Ok(plugin_name::rename(&self.plugins_dir, plugin_name, new_name)?)
    }

    /// Moves a plugin's folder to the Trash on this computer. Nothing on an MPC is touched.
    pub fn delete_plugin(&self, plugin_name: &str) -> Result<(), MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        plugin_name::move_to_trash(&plugin_dir).map_err(MakeError::NotDeleted)
    }

    /// One of the pictures the musician attached, for the screen to show beside the message it came with.
    pub fn attachment(&self, plugin_name: &str, file_name: &str) -> Result<Option<Vec<u8>>, MakeError> {
        Ok(AttachmentShelf::of_plugin(&self.existing_plugin(plugin_name)?).read(file_name))
    }

    /// The plugin as built for this computer, which is what the live player loads.
    pub fn built_library(&self, plugin_name: &str) -> Result<PathBuf, MakeError> {
        let plugin_dir = self.existing_plugin(plugin_name)?;
        native_library_of(&plugin_dir).ok_or_else(|| MakeError::NotBuiltYet(plugin_name.to_string()))
    }

    pub fn inspect(&self, plugin_name: &str) -> Result<GateReport, MakeError> {
        Ok(self.gate.inspect(&self.existing_plugin(plugin_name)?))
    }

    fn existing_plugin(&self, plugin_name: &str) -> Result<PathBuf, MakeError> {
        require_usable_name(plugin_name)?;
        let plugin_dir = self.plugins_dir.join(plugin_name);
        if plugin_dir.join(PLUGIN_MANIFEST).is_file() { Ok(plugin_dir) } else { Err(MakeError::NoSuchPlugin(plugin_name.to_string())) }
    }

    fn work_and_judge(&self, plugin_dir: PathBuf, first_message: &str, connection: Connection, observer: impl FnMut(Event)) -> Result<Delivery, MakeError> {
        let instructions = instructions::for_making_plugins();
        match connection {
            Connection::Model(model) => {
                let mut tools = PluginTools::open(&plugin_dir, self.sdk.root(), &self.gate)?;
                let mut agent = Agent::new(model, &mut tools, &instructions).compacting_at(self.compact_at).stopped_by(self.stop.clone()).watched_by(observer);
                self.deliver(plugin_dir, &mut agent, first_message)
            }
            Connection::ClaudeCode { tool_server_program, settings } => {
                let tool_server = self.tool_server_for(tool_server_program, &plugin_dir)?;
                let claude_code = ClaudeCode::new(tool_server, &plugin_dir, instructions).with_settings(settings).compacting_at(self.compact_at);
                let mut claude_code = claude_code.stopped_by(self.stop.clone()).watched_by(observer);
                self.deliver(plugin_dir, &mut claude_code, first_message)
            }
            Connection::Codex { tool_server_program, choices } => {
                let tool_server = self.tool_server_for(tool_server_program, &plugin_dir)?;
                let mut codex = Codex::new(tool_server, &plugin_dir, instructions).with_choices(choices).stopped_by(self.stop.clone()).watched_by(observer);
                self.deliver(plugin_dir, &mut codex, first_message)
            }
        }
    }

    /// An agent program starts the tool server from the plugin's folder, so every path it is given is absolute.
    fn tool_server_for(&self, tool_server_program: &Path, plugin_dir: &Path) -> io::Result<ToolServer> {
        let absolute = |path: &Path| true_path_of(path).map(|path| path.to_string_lossy().into_owned());
        let arguments = vec![SERVE_TOOLS_COMMAND.to_string(), absolute(plugin_dir)?, "--sdk".to_string(), absolute(self.sdk.root())?];
        Ok(ToolServer { program: tool_server_program.to_path_buf(), arguments })
    }

    fn deliver(&self, plugin_dir: PathBuf, worker: &mut dyn Worker, first_message: &str) -> Result<Delivery, MakeError> {
        match self.work_until_it_passes(worker, &plugin_dir, first_message) {
            Ok((closing_words, report)) => Ok(Delivery { plugin_dir, closing_words, report }),
            Err(AgentError::Model(not_signed_in @ ModelError::NoCredential(_))) => Err(MakeError::NotSignedIn(not_signed_in.to_string())),
            Err(AgentError::Stopped) => Err(MakeError::Stopped),
            Err(error) => Err(MakeError::Agent { plugin_dir, error }),
        }
    }

    fn work_until_it_passes(&self, worker: &mut dyn Worker, plugin_dir: &Path, first_message: &str) -> Result<(String, GateReport), AgentError> {
        let mut closing_words = worker.work_on(first_message)?;
        let mut report = self.gate.inspect(plugin_dir);
        for _ in 0..self.repair_rounds {
            if report.may_be_sent() {
                break;
            }
            if self.stop.is_asked() {
                return Err(AgentError::Stopped);
            }
            closing_words = worker.work_on(&repair_message(&report))?;
            report = self.gate.inspect(plugin_dir);
        }
        Ok((closing_words, report))
    }

    fn create_starter(&self, identity: &PluginIdentity) -> Result<PathBuf, MakeError> {
        require_usable_name(&identity.name)?;
        require_usable_name(&identity.maker)?;
        let plugin_dir = self.plugins_dir.join(&identity.name);
        if plugin_dir.exists() {
            return Err(MakeError::AlreadyExists(plugin_dir));
        }
        fs::create_dir_all(&self.plugins_dir)?;
        let created = self.sdk.create_plugin(identity, &self.plugins_dir, STARTER_TIME_LIMIT).map_err(describe_run_error)?;
        if created.succeeded() && plugin_dir.join(PLUGIN_MANIFEST).is_file() {
            Ok(plugin_dir)
        } else {
            Err(MakeError::StarterNotCreated(created.output.trim().to_string()))
        }
    }
}

/// Without a sign-in the AI never started, so the folder holds only the starter and the name can be reused.
fn discard_untouched_starter(plugin_dir: &Path) {
    let _ = fs::remove_dir_all(plugin_dir);
}

/// Names become folder names and appear on the MPC's screen, so they are kept plain.
pub fn is_usable_name(name: &str) -> bool {
    let starts_with_letter = name.chars().next().is_some_and(|first| first.is_ascii_alphabetic());
    let tidy = name == name.trim() && name.len() <= LONGEST_NAME;
    starts_with_letter && tidy && name.chars().all(fits_in_a_name) && !is_kept_by_windows(name)
}

/// Windows keeps a few names for its own devices, in any capitals, and no folder there can have one. They are
/// refused on every computer, so a plugin made on one can always be moved to another.
fn is_kept_by_windows(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    let is_a_numbered_port = PORTS_WINDOWS_NUMBERS.iter().any(|port| name.strip_prefix(port).is_some_and(is_one_digit_above_zero));
    DEVICES_WINDOWS_NAMES.contains(&name.as_str()) || is_a_numbered_port
}

fn is_one_digit_above_zero(written: &str) -> bool {
    matches!(written.as_bytes(), [b'1'..=b'9'])
}

fn fits_in_a_name(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == ' ' || character == '-'
}

/// A name somebody else wrote, made into one a plugin can have: plain characters only, starting at its first
/// letter and no longer than `longest`. Nothing when no such name is left.
pub fn name_made_usable(written: &str, longest: usize) -> Option<String> {
    let plain: String = written.chars().map(|character| if fits_in_a_name(character) { character } else { ' ' }).collect();
    let on_one_line = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let from_first_letter = on_one_line.trim_start_matches(|character: char| !character.is_ascii_alphabetic());
    let name: String = from_first_letter.chars().take(longest).collect();
    Some(name.trim_end().to_string()).filter(|name| is_usable_name(name))
}

fn require_usable_name(name: &str) -> Result<(), MakeError> {
    if is_usable_name(name) { Ok(()) } else { Err(MakeError::UnusableName(name.to_string())) }
}

fn describe_run_error(error: RunError) -> MakeError {
    let cause = match error {
        RunError::CouldNotStart(cause) => cause.to_string(),
        RunError::TimedOut { limit, .. } => format!("it had not finished after {} seconds", limit.as_secs()),
    };
    MakeError::StarterNotCreated(cause)
}

fn opening_message(request: &PluginRequest) -> String {
    let PluginIdentity { name, maker, kind } = &request.identity;
    format!(
        "The plugin is called \"{name}\", made by \"{maker}\". It is an {kind}.\n\nWhat the musician asked for:\n\n{description}",
        kind = kind.as_word(),
        description = request.description.trim(),
    )
}

/// The first line of what was asked, cut short, to remember a version by.
fn opening_words_of(change: &str) -> String {
    let first_line = change.trim().lines().next().unwrap_or_default().trim();
    let shortened: String = first_line.chars().take(LONGEST_VERSION_NOTE).collect();
    if shortened.len() < first_line.len() { format!("{shortened}\u{2026}") } else { shortened }
}

fn revision_message(plugin_name: &str, change: &str) -> String {
    format!(
        "The musician wants a change to \"{plugin_name}\", a plugin that already exists:\n\n{change}\n\n\
         Read the plugin's current files before you change anything. Keep everything they did not ask to change. \
         Projects saved on an MPC depend on the existing controls, so keep each one's id and position, and add \
         any new control after the last one.",
        change = change.trim(),
    )
}

/// Tells the AI which sound files the musician has added. Says nothing when there are none.
fn sound_files_note(samples: &[Sample]) -> String {
    if samples.is_empty() {
        return String::new();
    }
    let listed: Vec<String> = samples.iter().map(|sample| format!("- {sample}")).collect();
    format!("\n\nSound files the musician has added for this plugin to use:\n\n{}", listed.join("\n"))
}

fn repair_message(report: &GateReport) -> String {
    format!("MP Forge ran the safety check on what you delivered, and it did not pass:\n\n{report}\n\nFix the plugin and check it again.")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::agent::Entry;
    use crate::agent::test_support::{ScriptedModel, calls, says};
    use crate::samples::test_support::wav_of;
    use crate::look::ItemKind;
    use crate::look::test_support::{look_of_main_page, moved};
    use crate::sdk::PluginKind;
    use crate::workspace::test_support::TemporaryFolder;

    const CODE_THAT_STOPS_THE_APP: &str = "#include <cstdlib>\nvoid giveUp() { std::abort(); }\n";
    const HARMLESS_CODE: &str = "// nothing here any more\n";

    fn akai_sdk() -> Sdk {
        let sdk_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../vendor/mpc-plugin-builder-sdk");
        Sdk::at(sdk_dir).unwrap_or_else(|not_found| panic!("{not_found}"))
    }

    fn request_for(name: &str) -> PluginRequest {
        let identity = PluginIdentity { name: name.to_string(), maker: "Workbench Tests".to_string(), kind: PluginKind::Effect };
        PluginRequest { identity, description: "a gentle tone control".to_string() }
    }

    fn writes_extra_source(contents: &str) -> crate::agent::ModelTurn {
        calls("write_plugin_file", json!({ "path": "src/Extra.cpp", "contents": contents }))
    }

    fn user_messages(history: &[Entry]) -> Vec<&str> {
        history.iter().filter_map(|entry| if let Entry::User(text) = entry { Some(text.as_str()) } else { None }).collect()
    }

    #[test]
    fn sound_files_added_before_the_plugin_exists_wait_and_then_move_in() {
        let folder = TemporaryFolder::create("waiting-samples");
        let maker = Maker::new(akai_sdk(), &folder.0);
        maker.add_sample("Tone Shaper", "kick.wav", &wav_of(1, 44_100, 16, 4_410)).unwrap();
        let mut model = ScriptedModel::playing([says("Here is your tone control.")]);

        let delivery = maker.make(&request_for("Tone Shaper"), Connection::Model(&mut model), |_| {}).unwrap();

        assert!(delivery.plugin_dir.join("Content/Samples/kick.wav").is_file());
        assert_eq!(maker.samples("Tone Shaper").unwrap()[0].file_name, "kick.wav");
        assert!(crate::library::list_plugins(&folder.0).iter().all(|plugin| plugin.name == "Tone Shaper"));
        assert!(user_messages(&model.histories_seen[0]).iter().any(|message| message.contains("Samples/kick.wav (0.1 s, mono, 44100 Hz)")));
    }

    #[test]
    fn a_plugin_that_passes_the_gate_is_delivered_ready() {
        let plugins = TemporaryFolder::create("maker-ready");
        let mut model = ScriptedModel::playing([calls("list_plugin_files", json!({})), says("Here is your tone control.")]);

        let delivery = Maker::new(akai_sdk(), &plugins.0).make(&request_for("Soft Tone"), Connection::Model(&mut model), |_| {}).unwrap();

        assert!(delivery.is_ready(), "{}", delivery.report);
        assert_eq!(delivery.closing_words, "Here is your tone control.");
        assert!(delivery.plugin_dir.join("build").is_dir());
    }

    #[test]
    fn the_request_reaches_the_model_with_the_plugins_identity() {
        let plugins = TemporaryFolder::create("maker-brief");
        let mut model = ScriptedModel::playing([says("Done.")]);

        Maker::new(akai_sdk(), &plugins.0).make(&request_for("Brief Check"), Connection::Model(&mut model), |_| {}).unwrap();

        let first_message = user_messages(&model.histories_seen[0])[0];
        assert!(first_message.contains("\"Brief Check\""));
        assert!(first_message.contains("a gentle tone control"));
    }

    #[test]
    fn a_new_plugin_is_told_how_the_plugins_there_are_already_look() {
        let plugins = TemporaryFolder::create("maker-other-looks");
        let earlier = plugins.0.join("Dusty Tape");
        fs::create_dir_all(&earlier).unwrap();
        fs::write(earlier.join(PLUGIN_MANIFEST), r#"{ "name": "Dusty Tape", "manufacturer": "Workbench Tests", "kind": "effect" }"#).unwrap();
        fs::write(earlier.join("DESIGN.md"), "## The look\nThe front of a worn tape machine.\n").unwrap();
        let mut model = ScriptedModel::playing([says("Done.")]);

        Maker::new(akai_sdk(), &plugins.0).make(&request_for("Brief Check"), Connection::Model(&mut model), |_| {}).unwrap();

        let first_message = user_messages(&model.histories_seen[0])[0];
        assert!(first_message.contains("- Dusty Tape: The front of a worn tape machine."), "{first_message}");
    }

    #[test]
    fn a_plugin_the_gate_blocks_is_sent_back_to_the_model_until_it_passes() {
        let plugins = TemporaryFolder::create("maker-repair");
        let mut model = ScriptedModel::playing([
            writes_extra_source(CODE_THAT_STOPS_THE_APP),
            says("All finished!"),
            writes_extra_source(HARMLESS_CODE),
            says("Fixed it."),
        ]);

        let delivery = Maker::new(akai_sdk(), &plugins.0).make(&request_for("Needs Repair"), Connection::Model(&mut model), |_| {}).unwrap();

        let told_to_fix = user_messages(model.histories_seen.last().unwrap())[1];
        assert!(told_to_fix.contains("did not pass"));
        assert!(told_to_fix.contains("can stop the whole MPC app"));
        assert!(delivery.is_ready(), "{}", delivery.report);
        assert_eq!(delivery.closing_words, "Fixed it.");
    }

    #[test]
    fn a_model_that_claims_success_without_fixing_anything_is_not_believed() {
        let plugins = TemporaryFolder::create("maker-unfixed");
        let mut model = ScriptedModel::playing([writes_extra_source(CODE_THAT_STOPS_THE_APP), says("It works!"), says("It really works!")]);
        let maker = Maker::new(akai_sdk(), &plugins.0).with_repair_rounds(1);

        let delivery = maker.make(&request_for("Never Fixed"), Connection::Model(&mut model), |_| {}).unwrap();

        assert!(!delivery.is_ready());
        assert_eq!(delivery.closing_words, "It really works!");
    }

    struct SignedOut;

    impl Model for SignedOut {
        fn next_turn(&mut self, _: &crate::agent::Brief) -> Result<crate::agent::ModelTurn, ModelError> {
            Err(ModelError::NoCredential("Sign in first.".to_string()))
        }
    }

    fn maker_with_plugin(plugins: &TemporaryFolder, name: &str) -> Maker {
        let maker = Maker::new(akai_sdk(), &plugins.0);
        let mut model = ScriptedModel::playing([says("Made.")]);
        maker.make(&request_for(name), Connection::Model(&mut model), |_| {}).unwrap();
        maker
    }

    #[test]
    fn changing_a_plugin_keeps_the_old_version_and_passes_on_what_to_change() {
        let plugins = TemporaryFolder::create("maker-revise");
        let maker = maker_with_plugin(&plugins, "Change Me");
        let mut model = ScriptedModel::playing([writes_extra_source(HARMLESS_CODE), says("Changed.")]);

        let delivery = maker.revise("Change Me", "make it darker", Connection::Model(&mut model), |_| {}).unwrap();

        assert!(user_messages(&model.histories_seen[0])[0].contains("make it darker"));
        assert!(delivery.is_ready(), "{}", delivery.report);
        assert!(delivery.plugin_dir.join(".versions/1/src/ChangeMe.cpp").is_file());
        assert!(!delivery.plugin_dir.join(".versions/1/src/Extra.cpp").exists());
    }

    #[test]
    fn a_missing_sign_in_never_deletes_a_plugin_that_already_existed() {
        let plugins = TemporaryFolder::create("maker-revise-signed-out");
        let maker = maker_with_plugin(&plugins, "Keep Me");

        let result = maker.revise("Keep Me", "make it darker", Connection::Model(&mut SignedOut), |_| {});

        assert!(matches!(result, Err(MakeError::NotSignedIn(_))));
        assert!(plugins.0.join("Keep Me/plugin.json").is_file());
    }

    #[test]
    fn a_plugin_that_does_not_exist_cannot_be_changed_or_played() {
        let plugins = TemporaryFolder::create("maker-missing");
        let maker = Maker::new(akai_sdk(), &plugins.0);
        let mut model = ScriptedModel::playing([]);

        let changed = maker.revise("Not There", "anything", Connection::Model(&mut model), |_| {});

        assert!(matches!(changed, Err(MakeError::NoSuchPlugin(_))));
        assert!(matches!(maker.record_preview("Not There"), Err(MakeError::NoSuchPlugin(_))));
    }

    fn maker_with_unbuilt_plugin(plugins: &TemporaryFolder, name: &str) -> Maker {
        let plugin_dir = plugins.0.join(name);
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::write(plugin_dir.join(PLUGIN_MANIFEST), "{}").unwrap();
        fs::write(plugin_dir.join("ui.json"), r#"{ "pages": [{ "name": "MAIN", "controls": ["Time"] }] }"#).unwrap();
        Maker::new(akai_sdk(), &plugins.0)
    }

    #[test]
    fn a_plugin_the_gate_stops_is_never_handed_to_akais_tool() {
        let plugins = TemporaryFolder::create("maker-held-back");
        let maker = maker_with_unbuilt_plugin(&plugins, "Not Ready");
        let an_address_nothing_answers_at = Mpc::at("192.0.2.1").unwrap();

        let outcome = maker.send_to_mpc("Not Ready", &an_address_nothing_answers_at).unwrap();

        assert!(matches!(outcome, Sending::HeldBack(report) if !report.may_be_sent()));
    }

    fn look_that_moves_time_to(x: u32) -> Look {
        look_of_main_page(vec![moved(ItemKind::Control, "Time", x, 300)])
    }

    #[test]
    fn a_look_that_fits_is_kept_and_the_arranged_screen_comes_back() {
        let plugins = TemporaryFolder::create("maker-look");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");

        let pages = maker.keep_look("Arrange Me", &look_that_moves_time_to(600)).unwrap();

        assert_eq!((pages[0].items[0].area.x, pages[0].items[0].area.y), (600, 300));
        assert_eq!(look::look_of(&plugins.0.join("Arrange Me")), Ok(look_that_moves_time_to(600)));
    }

    #[test]
    fn a_look_that_hangs_off_the_screen_is_refused_and_the_old_one_stays() {
        let plugins = TemporaryFolder::create("maker-look-refused");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");
        maker.keep_look("Arrange Me", &look_that_moves_time_to(600)).unwrap();

        let refusal = maker.keep_look("Arrange Me", &look_that_moves_time_to(1100)).unwrap_err();

        assert!(refusal.to_string().contains("`Time` hangs off the edge of the screen"), "{refusal}");
        assert_eq!(look::look_of(&plugins.0.join("Arrange Me")), Ok(look_that_moves_time_to(600)));
    }

    #[test]
    fn going_back_after_editing_returns_to_the_look_from_before() {
        let plugins = TemporaryFolder::create("maker-look-back");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");
        maker.keep_look("Arrange Me", &look_that_moves_time_to(600)).unwrap();

        maker.begin_editing("Arrange Me").unwrap();
        maker.keep_look("Arrange Me", &look_that_moves_time_to(700)).unwrap();
        maker.keep_look("Arrange Me", &look_that_moves_time_to(800)).unwrap();
        maker.go_to_version("Arrange Me", 1).unwrap();

        assert_eq!(look::look_of(&plugins.0.join("Arrange Me")), Ok(look_that_moves_time_to(600)));
    }

    fn look_written(look: &str) -> Look {
        look::look_written_in(look).unwrap()
    }

    #[test]
    fn a_control_put_on_a_page_by_hand_is_written_into_the_layout_and_undoing_takes_it_off() {
        let plugins = TemporaryFolder::create("maker-look-added");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");
        let layout = || fs::read_to_string(plugins.0.join("Arrange Me/ui.json")).unwrap();
        let with_mix = look_written(r#"{ "pages": [{ "page": "MAIN", "added": [{ "control": "Mix", "widget": "fader" }] }] }"#);

        let pages = maker.keep_look("Arrange Me", &with_mix).unwrap();
        let layout_with_mix = layout();
        let undone = maker.keep_look("Arrange Me", &Look::default()).unwrap();

        assert_eq!(pages[0].items.iter().map(|item| item.name.as_str()).collect::<Vec<_>>(), ["Time", "Mix"]);
        assert!(layout_with_mix.contains("\"Mix\"") && layout_with_mix.contains("\"fader\""), "{layout_with_mix}");
        assert_eq!(undone[0].items.len(), 1);
        assert!(!layout().contains("Mix"));
    }

    #[test]
    fn a_look_that_is_refused_changes_neither_the_look_nor_the_layout() {
        let plugins = TemporaryFolder::create("maker-look-added-refused");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");
        let layout_before = fs::read_to_string(plugins.0.join("Arrange Me/ui.json")).unwrap();
        let off_the_page = look_written(
            r#"{ "pages": [{ "page": "MAIN", "items": [{ "kind": "control", "name": "Time", "place": { "x": 1100, "y": 45 } }], "added": [{ "control": "Mix", "widget": "knob" }] }] }"#,
        );

        assert!(maker.keep_look("Arrange Me", &off_the_page).is_err());
        assert_eq!(fs::read_to_string(plugins.0.join("Arrange Me/ui.json")).unwrap(), layout_before);
        assert_eq!(look::look_of(&plugins.0.join("Arrange Me")), Ok(Look::default()));
    }

    #[test]
    fn pictures_brought_in_as_artwork_are_kept_until_no_look_uses_them() {
        let plugins = TemporaryFolder::create("maker-artwork");
        let maker = maker_with_unbuilt_plugin(&plugins, "Arrange Me");
        let shelf = ArtShelf::of_plugin(&plugins.0.join("Arrange Me"));
        let used = maker.add_artwork("Arrange Me", &crate::attachments::test_support::png_of(8, 8)).unwrap();
        let unused = maker.add_artwork("Arrange Me", &crate::attachments::test_support::png_of(8, 8)).unwrap();
        let showing_the_first = format!(r#"{{ "pages": [{{ "page": "MAIN", "artwork": [{{ "kind": "picture", "id": 1, "file": "{used}", "area": {{ "x": 0, "y": 0, "width": 8, "height": 8 }} }}] }}] }}"#);
        maker.keep_look("Arrange Me", &look_written(&showing_the_first)).unwrap();

        maker.begin_editing("Arrange Me").unwrap();

        assert_eq!((used.as_str(), unused.as_str()), ("art-1.png", "art-2.png"));
        assert_eq!(shelf.list(), ["art-1.png"]);
        assert!(matches!(maker.add_artwork("Arrange Me", b"not a picture"), Err(MakeError::Art(ArtError::NotAPicture))));
    }

    #[test]
    fn a_built_plugin_can_be_recorded_for_listening() {
        let plugins = TemporaryFolder::create("maker-preview");
        let maker = maker_with_plugin(&plugins, "Hear Me");

        let sound_file = maker.record_preview("Hear Me").unwrap();

        assert!(fs::metadata(sound_file).unwrap().len() > 100_000);
    }

    #[test]
    fn a_missing_sign_in_leaves_nothing_behind_so_the_name_can_be_used_again() {
        let plugins = TemporaryFolder::create("maker-signed-out");

        let result = Maker::new(akai_sdk(), &plugins.0).make(&request_for("Try Again"), Connection::Model(&mut SignedOut), |_| {});

        assert!(matches!(result, Err(MakeError::NotSignedIn(message)) if message.contains("Sign in first.")));
        assert!(!plugins.0.join("Try Again").exists());
    }

    #[test]
    fn a_name_windows_keeps_for_itself_is_refused_on_every_computer() {
        for kept in ["CON", "nul", "Aux", "PRN", "COM1", "lpt9"] {
            assert!(!is_usable_name(kept), "`{kept}` was taken for a usable name");
        }
        for free in ["Console", "COM", "COM0", "COM10", "Null Space", "LPT A"] {
            assert!(is_usable_name(free), "`{free}` was refused");
        }
    }

    #[test]
    fn names_that_could_escape_the_plugins_folder_are_refused() {
        let plugins = TemporaryFolder::create("maker-names");
        let maker = Maker::new(akai_sdk(), &plugins.0);

        for bad_name in ["../Escape", "", " Padded", "semi;colon", "9 Lives", "A".repeat(LONGEST_NAME + 1).as_str()] {
            let mut model = ScriptedModel::playing([]);
            let result = maker.make(&request_for(bad_name), Connection::Model(&mut model), |_| {});
            assert!(matches!(result, Err(MakeError::UnusableName(_))), "`{bad_name}` should be refused");
        }
    }

    #[test]
    fn a_name_somebody_else_wrote_is_made_into_one_a_plugin_can_have() {
        assert_eq!(name_made_usable("  \"Kepler's 3rd: Orbit!\" ", 24).as_deref(), Some("Kepler s 3rd Orbit"));
        assert_eq!(name_made_usable("42 Days of Rain", 24).as_deref(), Some("Days of Rain"));
        assert_eq!(name_made_usable("A Very Long Name That Goes On And On", 24).as_deref(), Some("A Very Long Name That Go"));
        assert_eq!(name_made_usable("???", 24), None);
    }

    #[test]
    fn an_existing_plugin_is_never_overwritten() {
        let plugins = TemporaryFolder::create("maker-existing");
        fs::create_dir_all(plugins.0.join("Taken Name")).unwrap();
        let mut model = ScriptedModel::playing([]);

        let result = Maker::new(akai_sdk(), &plugins.0).make(&request_for("Taken Name"), Connection::Model(&mut model), |_| {});

        assert!(matches!(result, Err(MakeError::AlreadyExists(_))));
    }
}
