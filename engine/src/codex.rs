//! Codex as the workbench's AI: the musician's own Codex, signed in with their ChatGPT account.
//!
//! Like Claude Code, Codex runs its own agent loop, so the workbench hands it a small tool server with the
//! same tools and switches off everything else it can: its shell, its picture viewer, its helpers, its
//! connectors and plugins, and the musician's own Codex settings. Codex has no switch for its file-patching
//! tool, so it runs in Codex's read-only mode, where every patch is refused. Those switches are Codex's own.
//! As a second lock the workbench reads everything Codex reports, and stops it the moment it reports running
//! a command or changing a file by itself.
//!
//! Each message starts Codex afresh and leaves nothing in the musician's Codex history. The plugin's files
//! carry the work from one message to the next.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::{Value, json};

use crate::agent::{AgentError, Event, ModelError, Worker};
use crate::ai_settings::Effort;
use crate::claude_code::{ToolServer, Watchdog};
use crate::process::command_for;
use crate::mcp_server::SERVER_NAME;
use crate::providers::OfferedModel;
use crate::stop_signal::StopSignal;

const PROGRAM: &str = "codex";
const TIME_LIMIT: Duration = Duration::from_secs(45 * 60);
/// Everything of Codex's own that can be switched off by name. None of it is needed to make a plugin.
const SWITCHED_OFF: [&str; 8] = ["shell_tool", "view_image", "multi_agent", "image_generation", "apps", "plugins", "sleep_tool", "hooks"];
/// What Codex reports when it has used a tool of its own on the computer.
const RAN_A_COMMAND: &str = "command_execution";
const CHANGED_A_FILE: &str = "file_change";
const WORKBENCH_TOOL_CALL: &str = "mcp_tool_call";
const HOW_TO_SET_UP: &str = "Install Codex, run `codex login` in a terminal and sign in with your ChatGPT account, then try again.";
const STOPPED_FOR_SAFETY: &str = "Codex used a tool of its own on this computer, which MP Forge does not allow, so it was stopped at once. \
                                  Run the safety check before trusting the plugin, or pick another model.";
const ONLY_THE_WORKBENCH_TOOLS: &str = "\n\nYou are running inside MP Forge. Your shell and your file-patching tool are switched off here and \
                                        will refuse every request, so never try them. Do all of your work with MP Forge's own tools.";

/// Whether the musician's Codex is ready to work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    SignedIn,
    NotSignedIn,
    NotInstalled,
}

/// Asks Codex whether it is signed in. Nothing is sent anywhere and nothing is shown of the account.
pub fn sign_in() -> SignIn {
    let asked = command_for(PROGRAM).args(["login", "status"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    match asked {
        Ok(status) if status.success() => SignIn::SignedIn,
        Ok(_) => SignIn::NotSignedIn,
        Err(_) => SignIn::NotInstalled,
    }
}

/// The models Codex offers, as Codex itself lists them.
pub fn models_offered() -> Result<Vec<OfferedModel>, ModelError> {
    let listed = command_for(PROGRAM).args(["debug", "models"]).stdin(Stdio::null()).stderr(Stdio::null()).output();
    let listed = listed.map_err(|_| ModelError::NoCredential(HOW_TO_SET_UP.to_string()))?;
    let catalog = serde_json::from_slice::<Value>(&listed.stdout).map_err(|error| ModelError::Garbled(format!("Codex's list of models: {error}")))?;
    Ok(models_in(&catalog))
}

/// Only the models Codex shows in its own list are offered. The rest are its internal ones.
fn models_in(catalog: &Value) -> Vec<OfferedModel> {
    let listed = catalog["models"].as_array().map(Vec::as_slice).unwrap_or_default();
    listed.iter().filter(|model| model["visibility"] == "list").filter_map(offered).collect()
}

fn offered(model: &Value) -> Option<OfferedModel> {
    let id = model["slug"].as_str()?;
    let sees_pictures = model["input_modalities"].as_array().is_some_and(|taken_in| taken_in.iter().any(|kind| kind == "image"));
    Some(OfferedModel { id: id.to_string(), name: model["display_name"].as_str().unwrap_or(id).to_string(), sees_pictures, context_window: model["context_window"].as_u64(), efforts: Vec::new() })
}

/// What the musician picked for Codex. A model left unset is Codex's own choice for their plan.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodexChoices {
    pub model: Option<String>,
    pub effort: Option<Effort>,
    /// Whether Codex may search the web. Searching is all it can do there.
    pub web_access: bool,
}

pub struct Codex<'a> {
    tool_server: ToolServer,
    plugin_dir: PathBuf,
    instructions: String,
    choices: CodexChoices,
    /// What was asked first, so a later message can say what the work is about.
    first_message: Option<String>,
    stop: StopSignal,
    observer: Box<dyn FnMut(Event) + 'a>,
}

impl<'a> Codex<'a> {
    pub fn new(tool_server: ToolServer, plugin_dir: &Path, instructions: String) -> Self {
        Self { tool_server, plugin_dir: plugin_dir.to_path_buf(), instructions, choices: CodexChoices::default(), first_message: None, stop: StopSignal::default(), observer: Box::new(|_| {}) }
    }

    pub fn with_choices(mut self, choices: CodexChoices) -> Self {
        self.choices = choices;
        self
    }

    pub fn watched_by(mut self, observer: impl FnMut(Event) + 'a) -> Self {
        self.observer = Box::new(observer);
        self
    }

    /// Codex is shut down as soon as this signal asks for the work to stop.
    pub fn stopped_by(mut self, stop: StopSignal) -> Self {
        self.stop = stop;
        self
    }

    fn command(&self, message: &str) -> Command {
        let mut command = command_for(PROGRAM);
        command.current_dir(&self.plugin_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        command.arg("exec").args(lock_down_arguments(&self.tool_server, &self.plugin_dir));
        command.args(setting("developer_instructions", &json!(format!("{}{ONLY_THE_WORKBENCH_TOOLS}", self.instructions))));
        command.args(choice_arguments(&self.choices));
        command.arg(message);
        command
    }

    /// Codex starts afresh each time, so a later message is sent with what was asked first.
    fn worded_with_what_came_before(&self, message: &str) -> String {
        match &self.first_message {
            None => message.to_string(),
            Some(first) => format!("Earlier you were asked for this, and you have already worked on the plugin's files:\n\n{first}\n\nNow:\n\n{message}"),
        }
    }
}

impl Worker for Codex<'_> {
    fn work_on(&mut self, message: &str) -> Result<String, AgentError> {
        let worded = self.worded_with_what_came_before(message);
        self.first_message.get_or_insert_with(|| message.to_string());
        let mut child = self.command(&worded).spawn().map_err(|_| ModelError::NoCredential(HOW_TO_SET_UP.to_string()))?;
        let output = child.stdout.take().expect("stdout was piped");
        let watchdog = Watchdog::guarding(child, TIME_LIMIT, self.stop.clone());
        let mut transcript = Transcript::default();
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                transcript.take(&event, &mut *self.observer);
            }
            if transcript.used_a_tool_of_its_own {
                break;
            }
        }
        if transcript.used_a_tool_of_its_own { watchdog.end_now() } else { watchdog.stand_down() }
        if self.stop.is_asked() { Err(AgentError::Stopped) } else { Ok(transcript.closing_words()?) }
    }
}

/// One of Codex's settings, given for this run only. The value is written as JSON, which Codex reads the same way.
fn setting(name: &str, value: &Value) -> [String; 2] {
    ["-c".to_string(), format!("{name}={value}")]
}

/// Everything that confines Codex to the workbench's own tools. It reports as lines of JSON, keeps nothing of
/// the run, ignores the musician's own Codex settings and instructions, and may write nothing by itself.
fn lock_down_arguments(tool_server: &ToolServer, plugin_dir: &Path) -> Vec<String> {
    let server = |field: &str| format!("mcp_servers.{SERVER_NAME}.{field}");
    let mut arguments: Vec<String> = ["--json", "--ephemeral", "--skip-git-repo-check", "--ignore-user-config", "--sandbox", "read-only"].map(str::to_string).to_vec();
    arguments.extend(["--cd".to_string(), plugin_dir.to_string_lossy().into_owned()]);
    arguments.extend(SWITCHED_OFF.iter().flat_map(|feature| ["--disable".to_string(), feature.to_string()]));
    arguments.extend(setting("project_doc_max_bytes", &json!(0)));
    arguments.extend(setting("tools.experimental_request_user_input.enabled", &json!(false)));
    arguments.extend(setting(&server("command"), &json!(tool_server.program)));
    arguments.extend(setting(&server("args"), &json!(tool_server.arguments)));
    arguments.extend(setting(&server("default_tools_approval_mode"), &json!("approve")));
    arguments
}

/// The model, the reasoning level and whether the web may be searched.
fn choice_arguments(choices: &CodexChoices) -> Vec<String> {
    let model = choices.model.iter().flat_map(|model| ["--model".to_string(), model.clone()]);
    let effort = choices.effort.iter().flat_map(|effort| setting("model_reasoning_effort", &json!(effort.word())));
    let web = setting("web_search", &json!(if choices.web_access { "live" } else { "disabled" }));
    model.chain(effort).chain(web).collect()
}

/// What Codex reported while it worked, read from its lines of JSON.
#[derive(Default)]
struct Transcript {
    last_words: Option<String>,
    failure: Option<String>,
    used_a_tool_of_its_own: bool,
}

impl Transcript {
    fn take(&mut self, event: &Value, observer: &mut dyn FnMut(Event)) {
        let item = &event["item"];
        match event["type"].as_str() {
            Some("item.started") => self.note_started(item, observer),
            Some("item.completed") => self.note_completed(item, observer),
            Some("turn.failed") => self.failure = Some(words_of(&event["error"]["message"])),
            Some("error") => self.failure = Some(words_of(&event["message"])),
            _ => {}
        }
    }

    fn note_started(&mut self, item: &Value, observer: &mut dyn FnMut(Event)) {
        match item["type"].as_str() {
            Some(RAN_A_COMMAND) => self.used_a_tool_of_its_own = true,
            Some(WORKBENCH_TOOL_CALL) => observer(Event::ToolStarted { name: item["tool"].as_str().unwrap_or_default(), input: &item["arguments"] }),
            _ => {}
        }
    }

    /// A patch Codex's read-only mode refused changed nothing. One that went through is a file changed behind the gate.
    fn note_completed(&mut self, item: &Value, observer: &mut dyn FnMut(Event)) {
        match item["type"].as_str() {
            Some(RAN_A_COMMAND) => self.used_a_tool_of_its_own = true,
            Some(CHANGED_A_FILE) if item["status"] == "completed" => self.used_a_tool_of_its_own = true,
            Some(WORKBENCH_TOOL_CALL) => observer(Event::ToolFinished { name: item["tool"].as_str().unwrap_or_default(), succeeded: item["status"] == "completed" }),
            Some("agent_message") => self.note_said(item["text"].as_str().unwrap_or_default(), observer),
            _ => {}
        }
    }

    fn note_said(&mut self, said: &str, observer: &mut dyn FnMut(Event)) {
        if said.trim().is_empty() {
            return;
        }
        observer(Event::ModelSaid(said));
        self.last_words = Some(said.trim().to_string());
    }

    fn closing_words(self) -> Result<String, ModelError> {
        if self.used_a_tool_of_its_own {
            return Err(ModelError::Unreachable(STOPPED_FOR_SAFETY.to_string()));
        }
        match (self.failure, self.last_words) {
            (Some(problem), _) if is_about_signing_in(&problem) => Err(ModelError::NoCredential(format!("{problem}. {HOW_TO_SET_UP}"))),
            (Some(problem), _) => Err(ModelError::Unreachable(format!("Codex stopped: {problem}"))),
            (None, Some(closing_words)) => Ok(closing_words),
            (None, None) => Err(ModelError::Unreachable(format!("Codex stopped without finishing. If it is not signed in: {HOW_TO_SET_UP}"))),
        }
    }
}

fn words_of(message: &Value) -> String {
    message.as_str().unwrap_or("unknown error").trim().to_string()
}

fn is_about_signing_in(problem: &str) -> bool {
    let problem = problem.to_lowercase();
    ["not logged in", "login", "unauthorized", "401"].iter().any(|sign| problem.contains(sign))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool_server() -> ToolServer {
        ToolServer { program: PathBuf::from("/apps/workbench"), arguments: vec!["serve-tools".to_string(), "/plugins/Dusty Tape".to_string()] }
    }

    fn lock_down() -> Vec<String> {
        lock_down_arguments(&tool_server(), Path::new("/plugins/Dusty Tape"))
    }

    fn heard(events: &[Value]) -> (Transcript, Vec<String>) {
        let mut transcript = Transcript::default();
        let mut narration = Vec::new();
        let mut observer = |event: Event| match event {
            Event::ModelSaid(text) => narration.push(format!("said: {text}")),
            Event::ToolStarted { name, input } => narration.push(format!("tool: {name} {input}")),
            Event::ToolFinished { name, succeeded } => narration.push(format!("done: {name} {succeeded}")),
            _ => {}
        };
        for event in events {
            transcript.take(event, &mut observer);
        }
        (transcript, narration)
    }

    fn item(moment: &str, item: Value) -> Value {
        json!({ "type": format!("item.{moment}"), "item": item })
    }

    #[test]
    fn codex_is_given_no_shell_and_nothing_else_of_its_own_that_can_be_switched_off() {
        let arguments = lock_down();

        let is_switched_off = |feature: &str| arguments.windows(2).any(|pair| pair[0] == "--disable" && pair[1] == feature);
        assert!(SWITCHED_OFF.iter().all(|feature| is_switched_off(feature)));
        assert!(is_switched_off("shell_tool"));
        assert!(arguments.windows(2).any(|pair| pair[0] == "--sandbox" && pair[1] == "read-only"));
        assert!(!arguments.iter().any(|argument| argument.contains("danger") || argument.contains("bypass") || argument.contains("workspace-write")));
    }

    #[test]
    fn codex_ignores_the_musicians_own_settings_and_instructions_and_keeps_nothing_of_the_run() {
        let arguments = lock_down();

        assert!(arguments.contains(&"--ignore-user-config".to_string()) && arguments.contains(&"--ephemeral".to_string()));
        assert!(arguments.contains(&"project_doc_max_bytes=0".to_string()));
    }

    #[test]
    fn the_workbench_tool_server_is_the_only_one_and_its_tools_need_no_approval() {
        let arguments = lock_down();

        assert!(arguments.contains(&r#"mcp_servers.workbench.command="/apps/workbench""#.to_string()));
        assert!(arguments.contains(&r#"mcp_servers.workbench.args=["serve-tools","/plugins/Dusty Tape"]"#.to_string()));
        assert!(arguments.contains(&r#"mcp_servers.workbench.default_tools_approval_mode="approve""#.to_string()));
        assert_eq!(arguments.iter().filter(|argument| argument.starts_with("mcp_servers.")).count(), 3);
    }

    #[test]
    fn the_model_and_reasoning_level_are_passed_on_and_the_web_is_closed_unless_opened() {
        let chosen = CodexChoices { model: Some("gpt-6.1-sol".to_string()), effort: Some(Effort::High), web_access: true };

        assert_eq!(choice_arguments(&chosen), ["--model", "gpt-6.1-sol", "-c", r#"model_reasoning_effort="high""#, "-c", r#"web_search="live""#]);
        assert_eq!(choice_arguments(&CodexChoices::default()), ["-c", r#"web_search="disabled""#]);
    }

    #[test]
    fn instructions_with_quotes_and_new_lines_are_written_as_one_setting() {
        let [flag, written] = setting("developer_instructions", &json!("Say \"hi\".\nThen stop."));

        assert_eq!(flag, "-c");
        assert_eq!(written, r#"developer_instructions="Say \"hi\".\nThen stop.""#);
    }

    #[test]
    fn what_codex_says_and_which_workbench_tools_it_uses_are_narrated() {
        let (transcript, narration) = heard(&[
            json!({ "type": "thread.started", "thread_id": "t-1" }),
            item("started", json!({ "type": "mcp_tool_call", "server": "workbench", "tool": "write_plugin_file", "arguments": { "path": "src/A.cpp" }, "status": "in_progress" })),
            item("completed", json!({ "type": "mcp_tool_call", "server": "workbench", "tool": "write_plugin_file", "status": "completed" })),
            item("completed", json!({ "type": "mcp_tool_call", "server": "workbench", "tool": "check_plugin", "status": "failed" })),
            item("completed", json!({ "type": "agent_message", "text": "Here is your delay." })),
            json!({ "type": "turn.completed", "usage": { "input_tokens": 9, "output_tokens": 3 } }),
        ]);

        assert_eq!(narration, ["tool: write_plugin_file {\"path\":\"src/A.cpp\"}", "done: write_plugin_file true", "done: check_plugin false", "said: Here is your delay."]);
        assert_eq!(transcript.closing_words().unwrap(), "Here is your delay.");
    }

    #[test]
    fn a_command_codex_ran_by_itself_stops_the_work_and_is_said_plainly() {
        let ran = item("started", json!({ "type": "command_execution", "command": "ls", "status": "in_progress" }));

        let (transcript, _) = heard(&[ran, item("completed", json!({ "type": "agent_message", "text": "Done." }))]);

        assert!(transcript.used_a_tool_of_its_own);
        assert!(matches!(transcript.closing_words(), Err(ModelError::Unreachable(message)) if message.contains("does not allow")));
    }

    #[test]
    fn a_file_codex_changed_by_itself_stops_the_work_but_a_refused_patch_does_not() {
        let refused = item("completed", json!({ "type": "file_change", "changes": [{ "path": "src/A.cpp", "kind": "update" }], "status": "failed" }));
        let went_through = item("completed", json!({ "type": "file_change", "changes": [{ "path": "src/A.cpp", "kind": "update" }], "status": "completed" }));

        assert!(!heard(&[refused]).0.used_a_tool_of_its_own);
        assert!(heard(&[went_through]).0.used_a_tool_of_its_own);
    }

    #[test]
    fn a_failed_turn_is_reported_and_one_about_signing_in_says_how_to_fix_it() {
        let failed = |message: &str| heard(&[json!({ "type": "turn.failed", "error": { "message": message } })]).0.closing_words();

        assert!(matches!(failed("the model is overloaded"), Err(ModelError::Unreachable(message)) if message.contains("overloaded")));
        assert!(matches!(failed("Not logged in"), Err(ModelError::NoCredential(message)) if message.contains("codex login")));
        assert!(matches!(heard(&[]).0.closing_words(), Err(ModelError::Unreachable(_))));
    }

    #[test]
    fn a_later_message_is_sent_with_what_was_asked_first() {
        let mut codex = Codex::new(tool_server(), Path::new("/plugins/Dusty Tape"), String::new());

        let first = codex.worded_with_what_came_before("make a tape echo");
        codex.first_message = Some("make a tape echo".to_string());
        let later = codex.worded_with_what_came_before("the check did not pass");

        assert_eq!(first, "make a tape echo");
        assert!(later.contains("make a tape echo") && later.ends_with("Now:\n\nthe check did not pass"));
    }

    #[test]
    fn only_the_models_codex_lists_for_picking_are_offered() {
        let catalog = json!({ "models": [
            { "slug": "gpt-6.1-sol", "display_name": "GPT-6.1-Sol", "visibility": "list", "context_window": 272_000, "input_modalities": ["text", "image"] },
            { "slug": "codex-auto-review", "display_name": "Codex Auto Review", "visibility": "hide", "context_window": 272_000 },
        ] });

        let offered = models_in(&catalog);

        assert_eq!(offered, [OfferedModel { id: "gpt-6.1-sol".to_string(), name: "GPT-6.1-Sol".to_string(), sees_pictures: true, context_window: Some(272_000), efforts: Vec::new() }]);
    }
}
