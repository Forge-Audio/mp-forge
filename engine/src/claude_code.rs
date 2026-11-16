//! Claude Code as the workbench's AI: the user's own Claude Code, signed in however they signed it in.
//!
//! Claude Code runs its own agent loop, so the workbench cannot hold the tools itself the way it does
//! for a plain model. Instead it starts a small tool server that offers the same tools, and
//! launches Claude Code with everything else switched off: its shell, its file tools, the user's
//! settings and skills, and every other tool server. Its two web tools, which only read, stay off
//! too unless the user has allowed web access. Those switches are Claude Code's own, so on this
//! route the lock-down depends on Claude Code honouring them.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::agent::{AgentError, Event, ModelError, Worker};
use crate::ai_settings::AiSettings;
use crate::mcp_server::SERVER_NAME;
use crate::plugin_tools::TOOL_NAMES;
use crate::process::{ErrorOutput, command_for, end_with_all_it_started, temporary_file_of_its_own};
use crate::stop_signal::StopSignal;

const PROGRAM: &str = "claude";
/// Claude Code's own tools for searching the web and reading a page. Neither can change anything.
const WEB_TOOLS: [&str; 2] = ["WebSearch", "WebFetch"];
const TIME_LIMIT: Duration = Duration::from_secs(45 * 60);
/// A single answer with no tools to use is a matter of seconds. Past this it is given up.
const TIME_LIMIT_FOR_ONE_ANSWER: Duration = Duration::from_secs(4 * 60);
/// Claude Code only takes a limit for tidying its own conversation within these bounds.
const SMALLEST_COMPACTION_LIMIT: u64 = 100_000;
const LARGEST_COMPACTION_LIMIT: u64 = 1_000_000;
/// What Claude Code counts as read for one request: what was sent fresh, and what was kept from before.
const TOKENS_READ: [&str; 3] = ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"];
const WATCH_INTERVAL: Duration = Duration::from_millis(500);
const HOW_TO_SET_UP: &str = "Install Claude Code, run `claude` once in a terminal and sign in with /login, then try again.";
const STOPPED_WITHOUT_FINISHING: &str = "Claude Code stopped without finishing.";
/// What a Claude Code from before one of the switches it is started with says of that switch.
const DOES_NOT_KNOW_A_SWITCH: &str = "unknown option";
const HOW_TO_UPDATE: &str = "Your Claude Code is older than MP Forge needs. Run `claude update` in a terminal, then try again.";
/// As much of what Claude Code said on its way out as a musician can read, or pass on to whoever helps them.
const LAST_LINES_PASSED_ON: usize = 3;
const LONGEST_PASSED_ON: usize = 400;

/// The command that starts the workbench's own tool server for one plugin.
pub struct ToolServer {
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

pub struct ClaudeCode<'a> {
    tool_server: ToolServer,
    plugin_dir: PathBuf,
    instructions: String,
    settings: AiSettings,
    /// How many tokens Claude Code lets its conversation reach before it tidies it. Unset, it decides for itself.
    compact_at: Option<u64>,
    session_id: Option<String>,
    stop: StopSignal,
    observer: Box<dyn FnMut(Event) + 'a>,
}

impl<'a> ClaudeCode<'a> {
    pub fn new(tool_server: ToolServer, plugin_dir: &Path, instructions: String) -> Self {
        Self { tool_server, plugin_dir: plugin_dir.to_path_buf(), instructions, settings: AiSettings::default(), compact_at: None, session_id: None, stop: StopSignal::default(), observer: Box::new(|_| {}) }
    }

    /// Without this, Claude Code uses whatever model and reasoning level the user has set in it.
    pub fn with_settings(mut self, settings: AiSettings) -> Self {
        self.settings = settings;
        self
    }

    pub fn compacting_at(mut self, tokens: Option<u64>) -> Self {
        self.compact_at = tokens;
        self
    }

    pub fn watched_by(mut self, observer: impl FnMut(Event) + 'a) -> Self {
        self.observer = Box::new(observer);
        self
    }

    /// Claude Code is shut down as soon as this signal asks for the work to stop.
    pub fn stopped_by(mut self, stop: StopSignal) -> Self {
        self.stop = stop;
        self
    }

    fn made_ready_for(&self, message: &str) -> Result<MadeReady, AgentError> {
        let mut ready = made_ready(&Launch { directory: &self.plugin_dir, message, instructions: &self.instructions, tool_server: Some(&self.tool_server), settings: self.settings })?;
        ready.command.args(compaction_arguments(self.compact_at));
        if let Some(session_id) = &self.session_id {
            ready.command.args(["--resume", session_id]);
        }
        Ok(ready)
    }
}

impl Worker for ClaudeCode<'_> {
    fn work_on(&mut self, message: &str) -> Result<String, AgentError> {
        let overseen = Overseen { limit: TIME_LIMIT, stop: &self.stop };
        let mut transcript = heard_from(self.made_ready_for(message)?, &overseen, &mut *self.observer)?;
        self.session_id = transcript.session_id.take().or(self.session_id.take());
        Ok(transcript.closing_words()?)
    }
}

/// Asks Claude Code one thing and returns its answer. It is given no tools at all for this: none of its own,
/// and none of the workbench's. So all it can do is answer.
pub fn answer_once(instructions: &str, message: &str, settings: AiSettings, stop: &StopSignal) -> Result<String, AgentError> {
    answer_once_looking_things_up(instructions, message, AiSettings { web_access: false, ..settings }, stop)
}

/// Asks Claude Code one thing and returns its answer. It keeps its two web tools, which only read, when the
/// settings allow them. It is given nothing else: no other tool of its own, and none of the workbench's.
pub fn answer_once_looking_things_up(instructions: &str, message: &str, settings: AiSettings, stop: &StopSignal) -> Result<String, AgentError> {
    let directory = std::env::temp_dir();
    let ready = made_ready(&Launch { directory: &directory, message, instructions, tool_server: None, settings })?;
    let overseen = Overseen { limit: TIME_LIMIT_FOR_ONE_ANSWER, stop };
    Ok(heard_from(ready, &overseen, &mut |_| {})?.closing_words()?)
}

/// What one run of Claude Code is started with.
struct Launch<'a> {
    directory: &'a Path,
    message: &'a str,
    instructions: &'a str,
    /// The workbench's own tools, when this run is to have them.
    tool_server: Option<&'a ToolServer>,
    settings: AiSettings,
}

/// How long a run may take, and the signal that ends it early.
struct Overseen<'a> {
    limit: Duration,
    stop: &'a StopSignal,
}

/// Claude Code about to be started: the command, its instructions in a file, and the request it is handed once
/// it runs. Neither is put on the command line: the instructions alone are longer than Windows lets one be.
struct MadeReady {
    command: Command,
    message: String,
    /// What it says when something goes wrong for it, which is the only word of why it stopped early.
    complaints: ErrorOutput,
    /// Kept for as long as Claude Code may read it, and cleared away with this.
    _instructions: InstructionsFile,
}

/// The instructions, written where Claude Code can read them for one run.
struct InstructionsFile(PathBuf);

impl InstructionsFile {
    fn holding(instructions: &str) -> io::Result<Self> {
        let file = temporary_file_of_its_own("md");
        fs::write(&file, instructions)?;
        Ok(Self(file))
    }
}

impl Drop for InstructionsFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Claude Code, made ready with everything switched off but what the launch names.
fn made_ready(launch: &Launch) -> Result<MadeReady, AgentError> {
    let instructions = InstructionsFile::holding(launch.instructions).map_err(|problem| ModelError::Unreachable(format!("Its instructions could not be written for Claude Code: {problem}")))?;
    let mut command = command_for(PROGRAM);
    command.current_dir(launch.directory).stdin(Stdio::piped()).stdout(Stdio::piped());
    let complaints = ErrorOutput::of(&mut command).map_err(|problem| ModelError::Unreachable(format!("Claude Code could not be listened to: {problem}")))?;
    command.args(["--print", "--output-format", "stream-json", "--verbose"]);
    command.arg("--system-prompt-file").arg(&instructions.0);
    command.args(lock_down_arguments(launch.tool_server, built_in_tools_for(launch.settings)));
    command.args(choice_arguments(launch.settings));
    Ok(MadeReady { command, message: launch.message.to_string(), complaints, _instructions: instructions })
}

/// Hands Claude Code its request on a thread of its own, then closes its input so it knows that is all.
/// It is not waited on here: a program that has not begun reading yet must not hold up the one that listens to it.
fn hand_over(message: String, input: Option<ChildStdin>) {
    thread::spawn(move || {
        if let Some(mut input) = input {
            let _ = input.write_all(message.as_bytes());
        }
    });
}

/// Runs Claude Code to its end and returns what it reported on the way.
fn heard_from(mut ready: MadeReady, overseen: &Overseen, observer: &mut dyn FnMut(Event)) -> Result<Transcript, AgentError> {
    let mut child = ready.command.spawn().map_err(|_| ModelError::NoCredential(HOW_TO_SET_UP.to_string()))?;
    hand_over(std::mem::take(&mut ready.message), child.stdin.take());
    let output = child.stdout.take().expect("stdout was piped");
    let watchdog = Watchdog::guarding(child, overseen.limit, overseen.stop.clone());
    let mut transcript = Transcript::default();
    for line in BufReader::new(output).lines().map_while(Result::ok) {
        if let Ok(event) = serde_json::from_str::<Value>(&line) {
            transcript.take(&event, observer);
        }
    }
    watchdog.stand_down();
    transcript.complaints = ready.complaints.said_so_far();
    if overseen.stop.is_asked() { Err(AgentError::Stopped) } else { Ok(transcript) }
}

/// The only tools of Claude Code's own that it keeps: the web tools when the user allows them, otherwise none.
fn built_in_tools_for(settings: AiSettings) -> &'static [&'static str] {
    if settings.web_access { &WEB_TOOLS } else { &[] }
}

/// Everything that confines Claude Code to the workbench's own tools, when it is given them, and the built-in
/// tools named here. Given neither, it has no tools at all.
fn lock_down_arguments(tool_server: Option<&ToolServer>, built_in_tools: &[&str]) -> Vec<String> {
    let workbench_tools = TOOL_NAMES.iter().filter(|_| tool_server.is_some()).map(|tool| format!("mcp__{SERVER_NAME}__{tool}"));
    let allowed_tools: Vec<String> = workbench_tools.chain(built_in_tools.iter().map(|tool| tool.to_string())).collect();
    vec![
        "--tools".to_string(),
        built_in_tools.join(","),
        "--restricted".to_string(),
        "--disable-slash-commands".to_string(),
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        tool_server_config(tool_server).to_string(),
        "--allowedTools".to_string(),
        allowed_tools.join(","),
        "--permission-prompts".to_string(),
        "none".to_string(),
    ]
}

/// The limit at which Claude Code tidies its conversation, kept within what it takes.
fn compaction_arguments(compact_at: Option<u64>) -> Vec<String> {
    let within_bounds = |tokens: u64| tokens.clamp(SMALLEST_COMPACTION_LIMIT, LARGEST_COMPACTION_LIMIT);
    compact_at.map(|tokens| vec!["--autocompact".to_string(), within_bounds(tokens).to_string()]).unwrap_or_default()
}

/// The model and reasoning level, when the user picked them.
fn choice_arguments(settings: AiSettings) -> Vec<String> {
    let model = settings.model.map(|model| ["--model", model.id()]);
    let effort = settings.effort.map(|effort| ["--effort", effort.word()]);
    [model, effort].into_iter().flatten().flatten().map(str::to_string).collect()
}

fn tool_server_config(tool_server: Option<&ToolServer>) -> Value {
    match tool_server {
        Some(server) => json!({ "mcpServers": { SERVER_NAME: { "command": server.program, "args": server.arguments } } }),
        None => json!({ "mcpServers": {} }),
    }
}

/// What Claude Code reported while it worked, read from its stream of JSON events.
#[derive(Default)]
struct Transcript {
    session_id: Option<String>,
    result: Option<Result<String, String>>,
    /// What Claude Code said on its error output, where it explains itself when it gives up before it begins.
    complaints: String,
}

impl Transcript {
    fn take(&mut self, event: &Value, observer: &mut dyn FnMut(Event)) {
        if let Some(session_id) = event["session_id"].as_str() {
            self.session_id = Some(session_id.to_string());
        }
        match event["type"].as_str() {
            Some("assistant") if event["error"].is_null() => {
                narrate(&event["message"]["content"], observer);
                tell_of_context(&event["message"]["usage"], observer);
            }
            Some("system") if event["subtype"] == "compact_boundary" => observer(Event::ContextCompacted),
            Some("result") => self.result = Some(result_in(event)),
            _ => {}
        }
    }

    fn closing_words(self) -> Result<String, ModelError> {
        match self.result {
            Some(Ok(closing_words)) => Ok(closing_words),
            Some(Err(problem)) if problem.contains("authenticate") => Err(ModelError::NoCredential(format!("{problem}. {HOW_TO_SET_UP}"))),
            Some(Err(problem)) => Err(ModelError::Unreachable(format!("Claude Code stopped: {problem}"))),
            None => Err(ModelError::Unreachable(why_it_stopped_early(&self.complaints))),
        }
    }
}

/// Why Claude Code ended with no result, in its own last words, with what to do when the cause is known.
fn why_it_stopped_early(complaints: &str) -> String {
    let last_words = last_words_of(complaints);
    if last_words.is_empty() {
        return STOPPED_WITHOUT_FINISHING.to_string();
    }
    let what_to_do = if complaints.contains(DOES_NOT_KNOW_A_SWITCH) { format!(" {HOW_TO_UPDATE}") } else { String::new() };
    format!("{STOPPED_WITHOUT_FINISHING}{what_to_do} It said: {last_words}")
}

fn last_words_of(complaints: &str) -> String {
    let lines: Vec<&str> = complaints.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let last_lines = lines[lines.len().saturating_sub(LAST_LINES_PASSED_ON)..].join(" ");
    last_lines.chars().take(LONGEST_PASSED_ON).collect()
}

fn result_in(event: &Value) -> Result<String, String> {
    let said = event["result"].as_str().unwrap_or_default().trim().to_string();
    let failed = event["is_error"].as_bool().unwrap_or(false) || event["subtype"] != "success";
    if failed { Err(if said.is_empty() { event["subtype"].to_string() } else { said }) } else { Ok(said) }
}

fn narrate(content: &Value, observer: &mut dyn FnMut(Event)) {
    for block in content.as_array().into_iter().flatten() {
        match block["type"].as_str() {
            Some("text") => observer(Event::ModelSaid(block["text"].as_str().unwrap_or_default())),
            Some("tool_use") => observer(Event::ToolStarted { name: plain_tool_name(&block["name"]), input: &block["input"] }),
            _ => {}
        }
    }
}

/// What the conversation holds after a turn: everything Claude read for it, and the turn it wrote.
fn tell_of_context(usage: &Value, observer: &mut dyn FnMut(Event)) {
    if usage["input_tokens"].is_null() {
        return;
    }
    let read: u64 = TOKENS_READ.iter().filter_map(|count| usage[*count].as_u64()).sum();
    observer(Event::ContextUsed { tokens: read + usage["output_tokens"].as_u64().unwrap_or(0) });
}

/// Claude Code calls the tools `mcp__workbench__write_plugin_file`; the workbench calls them `write_plugin_file`.
fn plain_tool_name(name: &Value) -> &str {
    let name = name.as_str().unwrap_or_default();
    name.rsplit_once("__").map_or(name, |(_, plain)| plain)
}

/// Stops an agent program if it is still running when the time limit passes, or as soon as the musician asks.
pub(crate) struct Watchdog {
    finished: Arc<AtomicBool>,
    child: Arc<Mutex<Child>>,
}

impl Watchdog {
    pub(crate) fn guarding(child: Child, limit: Duration, stop: StopSignal) -> Self {
        let watchdog = Self { finished: Arc::new(AtomicBool::new(false)), child: Arc::new(Mutex::new(child)) };
        let (finished, child) = (Arc::clone(&watchdog.finished), Arc::clone(&watchdog.child));
        thread::spawn(move || {
            let deadline = Instant::now() + limit;
            while !finished.load(Ordering::Relaxed) && Instant::now() < deadline && !stop.is_asked() {
                thread::sleep(WATCH_INTERVAL);
            }
            let _ = child.lock().map(|mut child| end_unless(&finished, &mut child));
        });
        watchdog
    }

    pub(crate) fn stand_down(self) {
        self.finished.store(true, Ordering::Relaxed);
        let _ = self.child.lock().map(|mut child| child.wait());
    }

    /// Ends the program at once, whether or not it has finished by itself, and waits for it to be gone.
    pub(crate) fn end_now(self) {
        self.finished.store(true, Ordering::Relaxed);
        let _ = self.child.lock().map(|mut child| end_with_all_it_started(&mut child));
    }
}

/// The program is ended with its tool server and whatever build or tryout that server has running. Whether it has
/// finished is asked while the program is held, so one that was let go meanwhile is never ended.
fn end_unless(finished: &AtomicBool, child: &mut Child) {
    if !finished.load(Ordering::Relaxed) {
        end_with_all_it_started(child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asks the real Claude Code one small thing, which spends a little of the musician's usage, so it is only
    /// run by hand: `cargo test -- --ignored the_real_claude_code`.
    #[test]
    #[ignore = "asks the real Claude Code a question"]
    fn the_real_claude_code_reads_its_instructions_from_a_file_and_its_request_from_its_input() {
        let instructions = "Whatever you are asked, answer with the one word the request names, in capitals, and nothing else.";

        let answer = answer_once(instructions, "The word is forge.", AiSettings::default(), &StopSignal::default()).unwrap();

        assert!(answer.contains("FORGE"), "{answer}");
    }

    #[test]
    fn the_instructions_are_in_a_file_only_for_as_long_as_one_run_may_read_them() {
        let ready = made_ready(&Launch { directory: Path::new("."), message: "make it warmer", instructions: "Be brief.", tool_server: None, settings: AiSettings::default() }).unwrap();
        let file = ready._instructions.0.clone();
        let on_the_command_line: Vec<String> = ready.command.get_args().map(|argument| argument.to_string_lossy().into_owned()).collect();

        assert_eq!(fs::read_to_string(&file).unwrap(), "Be brief.");
        assert!(on_the_command_line.windows(2).any(|pair| pair[0] == "--system-prompt-file" && Path::new(&pair[1]) == file));
        assert!(!on_the_command_line.iter().any(|argument| argument == "make it warmer" || argument == "Be brief."));
        drop(ready);
        assert!(!file.exists());
    }

    fn tool_server() -> ToolServer {
        ToolServer { program: PathBuf::from("/apps/workbench"), arguments: vec!["serve-tools".to_string(), "/plugins/Dusty Tape".to_string()] }
    }

    fn heard(events: &[Value]) -> (Transcript, Vec<String>) {
        let mut transcript = Transcript::default();
        let mut narration = Vec::new();
        let mut observer = |event: Event| match event {
            Event::ModelSaid(text) => narration.push(format!("said: {text}")),
            Event::ToolStarted { name, input } => narration.push(format!("tool: {name} {input}")),
            Event::ContextUsed { tokens } => narration.push(format!("context: {tokens}")),
            Event::ContextCompacted => narration.push("compacted".to_string()),
            Event::ToolFinished { .. } => {}
        };
        for event in events {
            transcript.take(event, &mut observer);
        }
        (transcript, narration)
    }

    fn finished_with(result: &str, is_error: bool) -> Value {
        json!({ "type": "result", "subtype": "success", "is_error": is_error, "result": result, "session_id": "session-1" })
    }

    #[test]
    fn claude_code_gets_no_tools_of_its_own_and_only_the_workbench_server() {
        let arguments = lock_down_arguments(Some(&tool_server()), built_in_tools_for(AiSettings::default()));

        let value_after = |flag: &str| arguments[arguments.iter().position(|argument| argument == flag).unwrap() + 1].clone();
        assert_eq!(value_after("--tools"), "");
        assert_eq!(value_after("--permission-prompts"), "none");
        assert!(arguments.contains(&"--restricted".to_string()));
        assert!(arguments.contains(&"--strict-mcp-config".to_string()));
        assert!(arguments.contains(&"--disable-slash-commands".to_string()));
        assert!(!arguments.iter().any(|argument| argument.contains("skip-permissions") || argument == "bypassPermissions"));
    }

    #[test]
    fn exactly_the_six_workbench_tools_are_allowed() {
        let arguments = lock_down_arguments(Some(&tool_server()), built_in_tools_for(AiSettings::default()));

        let allowed = &arguments[arguments.iter().position(|argument| argument == "--allowedTools").unwrap() + 1];
        let allowed: Vec<&str> = allowed.split(',').collect();
        assert_eq!(allowed.len(), TOOL_NAMES.len());
        assert!(allowed.iter().all(|tool| tool.starts_with("mcp__workbench__")));
        assert!(allowed.contains(&"mcp__workbench__check_plugin"));
    }

    #[test]
    fn allowing_web_access_adds_the_two_web_tools_and_nothing_else() {
        let online = AiSettings::default().with_web_access(true);

        let arguments = lock_down_arguments(Some(&tool_server()), built_in_tools_for(online));

        let value_after = |flag: &str| arguments[arguments.iter().position(|argument| argument == flag).unwrap() + 1].clone();
        assert_eq!(value_after("--tools"), "WebSearch,WebFetch");
        assert_eq!(value_after("--allowedTools").split(',').count(), TOOL_NAMES.len() + WEB_TOOLS.len());
        assert!(value_after("--allowedTools").ends_with("WebSearch,WebFetch"));
        assert!(arguments.contains(&"--restricted".to_string()));
    }

    #[test]
    fn asked_for_one_answer_claude_code_gets_no_tools_and_no_tool_server_at_all() {
        let arguments = lock_down_arguments(None, &[]);

        let value_after = |flag: &str| arguments[arguments.iter().position(|argument| argument == flag).unwrap() + 1].clone();
        assert_eq!(value_after("--tools"), "");
        assert_eq!(value_after("--allowedTools"), "");
        assert_eq!(value_after("--mcp-config"), r#"{"mcpServers":{}}"#);
        assert!(arguments.contains(&"--strict-mcp-config".to_string()) && arguments.contains(&"--restricted".to_string()));
    }

    #[test]
    fn a_compaction_limit_is_passed_on_within_what_claude_code_takes() {
        assert!(compaction_arguments(None).is_empty());
        assert_eq!(compaction_arguments(Some(200_000)), ["--autocompact", "200000"]);
        assert_eq!(compaction_arguments(Some(40_000)), ["--autocompact", "100000"]);
        assert_eq!(compaction_arguments(Some(5_000_000)), ["--autocompact", "1000000"]);
    }

    #[test]
    fn the_context_is_told_after_each_turn_and_so_is_a_compaction() {
        let turn = json!({ "type": "assistant", "message": { "content": [], "usage": { "input_tokens": 12, "cache_read_input_tokens": 40_000, "cache_creation_input_tokens": 3_000, "output_tokens": 500 } } });
        let tidied = json!({ "type": "system", "subtype": "compact_boundary", "compact_metadata": { "trigger": "auto", "pre_tokens": 200_000 } });

        let (_, narration) = heard(&[turn, tidied]);

        assert_eq!(narration, ["context: 43512", "compacted"]);
    }

    #[test]
    fn a_chosen_model_and_reasoning_level_are_passed_on_and_unset_ones_are_left_out() {
        let chosen = AiSettings::from_words(Some("sonnet"), Some("low")).unwrap();
        let only_effort = AiSettings::from_words(None, Some("max")).unwrap();

        assert_eq!(choice_arguments(chosen), ["--model", "claude-sonnet-5-5", "--effort", "low"]);
        assert_eq!(choice_arguments(only_effort), ["--effort", "max"]);
        assert!(choice_arguments(AiSettings::default()).is_empty());
    }

    #[test]
    fn the_tool_server_is_started_with_the_workbench_program() {
        let config = tool_server_config(Some(&tool_server()));

        assert_eq!(config["mcpServers"]["workbench"]["command"], "/apps/workbench");
        assert_eq!(config["mcpServers"]["workbench"]["args"], json!(["serve-tools", "/plugins/Dusty Tape"]));
    }

    #[test]
    fn the_closing_words_and_session_come_from_the_result_event() {
        let (transcript, _) = heard(&[json!({ "type": "system", "subtype": "init", "session_id": "session-1" }), finished_with("Here is your delay.", false)]);

        assert_eq!(transcript.session_id.as_deref(), Some("session-1"));
        assert_eq!(transcript.closing_words().unwrap(), "Here is your delay.");
    }

    #[test]
    fn what_claude_says_and_which_tools_it_uses_are_narrated_in_plain_names() {
        let working = json!({ "type": "assistant", "message": { "content": [
            { "type": "text", "text": "Writing the delay." },
            { "type": "tool_use", "name": "mcp__workbench__write_plugin_file", "input": { "path": "src/A.cpp" } },
        ] } });

        let (_, narration) = heard(&[working]);

        assert_eq!(narration, ["said: Writing the delay.", "tool: write_plugin_file {\"path\":\"src/A.cpp\"}"]);
    }

    #[test]
    fn an_expired_sign_in_is_reported_with_how_to_fix_it() {
        let problem = "Failed to authenticate: OAuth session expired and could not be refreshed";
        let error_notice = json!({ "type": "assistant", "error": "authentication_failed", "message": { "content": [{ "type": "text", "text": problem }] } });

        let (transcript, narration) = heard(&[error_notice, finished_with(problem, true)]);

        assert!(narration.is_empty());
        assert!(matches!(transcript.closing_words(), Err(ModelError::NoCredential(message)) if message.contains("/login")));
    }

    #[test]
    fn stopping_without_a_result_is_an_error() {
        let (transcript, _) = heard(&[json!({ "type": "system", "subtype": "init", "session_id": "session-1" })]);

        assert!(matches!(transcript.closing_words(), Err(ModelError::Unreachable(why)) if why == STOPPED_WITHOUT_FINISHING));
    }

    fn stopped_early_saying(complaints: &str) -> String {
        let transcript = Transcript { complaints: complaints.to_string(), ..Transcript::default() };
        match transcript.closing_words() {
            Err(ModelError::Unreachable(why)) => why,
            other => panic!("expected it to be unreachable, got {other:?}"),
        }
    }

    #[test]
    fn stopping_early_is_reported_with_the_last_of_what_claude_code_said() {
        let why = stopped_early_saying("starting up\n\nfirst\nsecond\n  third  \nError: the tool server could not be started\n");

        assert_eq!(why, "Claude Code stopped without finishing. It said: second third Error: the tool server could not be started");
    }

    #[test]
    fn a_claude_code_too_old_for_its_switches_is_reported_with_how_to_update_it() {
        let why = stopped_early_saying("error: unknown option '--restricted'\n");

        assert!(why.contains("claude update") && why.ends_with("It said: error: unknown option '--restricted'"), "{why}");
    }

    #[test]
    fn only_so_much_of_what_claude_code_said_is_passed_on() {
        let why = stopped_early_saying(&"x".repeat(5_000));

        assert_eq!(why.chars().filter(|letter| *letter == 'x').count(), LONGEST_PASSED_ON);
    }

    /// A stand-in for Claude Code that complains and gives up before it reports anything.
    #[cfg(unix)]
    #[test]
    fn what_a_program_says_as_it_gives_up_is_heard() {
        let mut command = Command::new("sh");
        command.args(["-c", "echo 'error: no such thing' >&2; exit 1"]).stdin(Stdio::piped()).stdout(Stdio::piped());
        let complaints = ErrorOutput::of(&mut command).unwrap();
        let ready = MadeReady { command, message: String::new(), complaints, _instructions: InstructionsFile::holding("").unwrap() };
        let overseen = Overseen { limit: Duration::from_secs(5), stop: &StopSignal::default() };

        let transcript = heard_from(ready, &overseen, &mut |_| {}).unwrap();

        assert_eq!(transcript.closing_words().unwrap_err().to_string(), "The AI service could not be reached: Claude Code stopped without finishing. It said: error: no such thing");
    }
}
