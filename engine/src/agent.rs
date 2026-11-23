//! The agent loop: ask the model for its next turn, run the tools it asks for, and repeat.
//!
//! This module knows nothing about any particular AI provider and nothing about plugins.
//! A provider implements [`Model`]; whatever the model is allowed to do implements [`Toolbox`].
//! The model can only ever act through the toolbox it is given.

use std::fmt;
use std::time::Duration;

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use serde_json::{Value, json};

use crate::stop_signal::StopSignal;

/// How many times a model may answer one request before it is stopped. A new plugin takes a good many: Akai's
/// documents are read, the screen and the code are written, and the plugin is built, checked and mended. A model
/// that goes in circles spends the musician's money for as long as it is let.
const DEFAULT_TURN_LIMIT: usize = 250;
/// How long a service that says it is asked too often is left alone before it is asked again, when it names
/// no time itself: after its first refusal and after its second. Its third stands.
const USUAL_WAITS_WHEN_ASKED_TOO_OFTEN: [Duration; 2] = [Duration::from_secs(20), Duration::from_secs(60)];
/// A service that asks for longer than this is not waited for. The musician is told how long it asks for.
const LONGEST_WAIT_KEPT: Duration = Duration::from_secs(150);
const SECONDS_A_MINUTE: u64 = 60;
const MINUTES_AN_HOUR: u64 = 60;
/// Up to this long a wait is said in seconds, and up to this many minutes in minutes.
const LONGEST_SAID_IN_SECONDS: u64 = 90;
const MOST_MINUTES_SAID_IN_MINUTES: u64 = 90;
const CUT_OFF_BEFORE_COMPLETE: &str =
    "This request was cut off before it was complete, so nothing was done. Send it again in smaller pieces.";
const CONTINUE_AFTER_CUT_OFF: &str = "Your last message was cut off. Continue from where you stopped.";
/// When room is made, this many of the newest sets of tool results are left exactly as they were.
const RESULTS_LEFT_WHOLE: usize = 2;
const LONGEST_OLD_RESULT: usize = 400;
const DROPPED_TO_MAKE_ROOM: &str = "\n[The rest of this was dropped to make room. Use the tool again if you need it.]";

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolInput {
    Valid(Value),
    /// The model sent something that is not valid JSON. The tool is never run with it.
    Unparseable(String),
}

impl ToolInput {
    /// Reads what a model wrote as a tool's input. Strict on purpose: input that was cut short or malformed
    /// must never reach a tool. Nothing written at all means a tool that takes no input.
    pub fn read_from(written: &str) -> Self {
        if written.trim().is_empty() {
            return ToolInput::Valid(json!({}));
        }
        match serde_json::from_str::<Value>(written) {
            Ok(input) if input.is_object() => ToolInput::Valid(input),
            _ => ToolInput::Unparseable(written.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: ToolInput,
}

/// A picture a tool hands back for the model to look at, with words that say what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Illustration {
    pub caption: String,
    pub png: Vec<u8>,
}

impl Illustration {
    /// The picture as text, which is how every provider wants an image sent.
    pub fn png_as_base64(&self) -> String {
        BASE64_STANDARD.encode(&self.png)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
    pub illustrations: Vec<Illustration>,
    pub is_error: bool,
}

impl ToolOutput {
    pub fn success(content: impl Into<String>) -> Self {
        Self { content: content.into(), illustrations: Vec::new(), is_error: false }
    }

    pub fn failure(content: impl Into<String>) -> Self {
        Self { content: content.into(), illustrations: Vec::new(), is_error: true }
    }

    pub fn illustrated_with(self, illustrations: Vec<Illustration>) -> Self {
        Self { illustrations, ..self }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    pub call_id: String,
    pub output: ToolOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Finished,
    WantsTools,
    /// The provider paused a long run of its own tools, such as web lookups. Asking again carries on from there.
    Paused,
    RanOutOfRoom,
    Refused,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelTurn {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
    pub stop: StopReason,
    /// The provider's own record of this turn. It is sent back unchanged on later requests.
    pub replay: Value,
    /// How many tokens the conversation with the model held once this turn was made, when the provider said.
    pub tokens_in_context: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Entry {
    User(String),
    Model(ModelTurn),
    ToolResults(Vec<ToolResult>),
}

/// Everything a model needs to produce its next turn.
pub struct Brief<'a> {
    pub instructions: &'a str,
    pub tools: &'a [ToolDefinition],
    pub history: &'a [Entry],
}

#[derive(Debug)]
pub enum ModelError {
    NoCredential(String),
    Rejected { status: u16, message: String },
    /// The service is being asked more often than it allows, which passes by itself. It may say how long it
    /// asks to be left alone.
    AskedTooOften { message: String, asks_to_wait: Option<Duration> },
    Unreachable(String),
    Garbled(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::NoCredential(how_to_fix) => write!(formatter, "The AI is not signed in. {how_to_fix}"),
            ModelError::Rejected { status, message } => write!(formatter, "The AI service said no ({status}): {message}"),
            ModelError::AskedTooOften { message, asks_to_wait } => {
                let its_words = message.trim_end_matches('.');
                write!(formatter, "The AI service said no because it is being asked too often (429): {its_words}. {}", when_to_send_again(*asks_to_wait))
            }
            ModelError::Unreachable(cause) => write!(formatter, "The AI service could not be reached: {cause}"),
            ModelError::Garbled(cause) => write!(formatter, "The AI service sent a reply that could not be read: {cause}"),
        }
    }
}

fn when_to_send_again(asks_to_wait: Option<Duration>) -> String {
    match asks_to_wait {
        Some(wait) => format!("It asks to be left alone for {}: send your message again after that, or pick another model.", span_in_words(wait)),
        None => "Send your message again in a few minutes, or pick another model.".to_string(),
    }
}

/// What whoever watches is told while a service that is asked too often is left alone.
pub fn said_while_waiting(wait: Duration) -> String {
    format!("the AI service says it is being asked too often: asking again in {}", span_in_words(wait))
}

/// A length of time as it is said to the musician: in seconds while it is short, then in minutes, then in hours.
pub fn span_in_words(span: Duration) -> String {
    let seconds = span.as_secs().max(1);
    let minutes = seconds.div_ceil(SECONDS_A_MINUTE);
    match (seconds, minutes) {
        (1, _) => "1 second".to_string(),
        (..=LONGEST_SAID_IN_SECONDS, _) => format!("{seconds} seconds"),
        (_, ..=MOST_MINUTES_SAID_IN_MINUTES) => format!("about {minutes} minutes"),
        _ => format!("about {} hours", minutes.div_ceil(MINUTES_AN_HOUR)),
    }
}

pub trait Model {
    fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError>;
}

pub trait Toolbox {
    fn definitions(&self) -> Vec<ToolDefinition>;
    fn run(&mut self, tool_name: &str, input: &Value) -> ToolOutput;
}

/// Something that can be handed a message and left to work until it has closing words to give.
/// Sending a second message continues the same conversation.
pub trait Worker {
    fn work_on(&mut self, message: &str) -> Result<String, AgentError>;
}

/// What the agent is doing, for whoever is watching.
pub enum Event<'a> {
    ModelSaid(&'a str),
    ToolStarted { name: &'a str, input: &'a Value },
    ToolFinished { name: &'a str, succeeded: bool },
    /// How many tokens the conversation with the model now holds.
    ContextUsed { tokens: u64 },
    /// Older material was dropped from the conversation with the model to make room.
    ContextCompacted,
    /// The AI service said it is asked too often. It is left alone for this long and then asked again.
    WaitingToAskAgain { wait: Duration },
}

#[derive(Debug)]
pub enum AgentError {
    Model(ModelError),
    Refused,
    TooManyTurns(usize),
    /// The musician asked for the work to stop.
    Stopped,
}

impl fmt::Display for AgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::Model(error) => error.fmt(formatter),
            AgentError::Refused => write!(formatter, "The AI declined this request."),
            AgentError::TooManyTurns(limit) => write!(
                formatter,
                "The AI had not finished after {limit} rounds of work, which is where MP Forge stops it so that a model going in circles cannot run up your bill. What it has done so far is kept: say \"continue\" to let it carry on."
            ),
            AgentError::Stopped => write!(formatter, "Stopped."),
        }
    }
}

impl From<ModelError> for AgentError {
    fn from(error: ModelError) -> Self {
        AgentError::Model(error)
    }
}

pub struct Agent<'a> {
    model: &'a mut dyn Model,
    toolbox: &'a mut dyn Toolbox,
    instructions: &'a str,
    history: Vec<Entry>,
    turn_limit: usize,
    /// Once the conversation holds this many tokens, what the tools handed back long ago is dropped from it.
    compact_at: Option<u64>,
    stop: StopSignal,
    observer: Box<dyn FnMut(Event) + 'a>,
}

impl<'a> Agent<'a> {
    pub fn new(model: &'a mut dyn Model, toolbox: &'a mut dyn Toolbox, instructions: &'a str) -> Self {
        Self { model, toolbox, instructions, history: Vec::new(), turn_limit: DEFAULT_TURN_LIMIT, compact_at: None, stop: StopSignal::default(), observer: Box::new(|_| {}) }
    }

    /// Without a limit the conversation grows until the provider's own limit.
    pub fn compacting_at(mut self, tokens: Option<u64>) -> Self {
        self.compact_at = tokens;
        self
    }

    pub fn watched_by(mut self, observer: impl FnMut(Event) + 'a) -> Self {
        self.observer = Box::new(observer);
        self
    }

    /// The agent gives up before its next step once this signal asks it to.
    pub fn stopped_by(mut self, stop: StopSignal) -> Self {
        self.stop = stop;
        self
    }

    pub fn with_turn_limit(mut self, turn_limit: usize) -> Self {
        self.turn_limit = turn_limit;
        self
    }

    pub fn history(&self) -> &[Entry] {
        &self.history
    }

    fn ask_model(&mut self) -> Result<ModelTurn, ModelError> {
        let tools = self.toolbox.definitions();
        let brief = Brief { instructions: self.instructions, tools: &tools, history: &self.history };
        let (observer, stop) = (&mut self.observer, &self.stop);
        let wait_out = |wait: Duration| {
            observer(Event::WaitingToAskAgain { wait });
            stop.lets_pass(wait)
        };
        let turn = turn_asked_patiently(&mut *self.model, &brief, wait_out)?;
        if !turn.text.trim().is_empty() {
            (self.observer)(Event::ModelSaid(&turn.text));
        }
        if let Some(tokens) = turn.tokens_in_context {
            (self.observer)(Event::ContextUsed { tokens });
            self.make_room_if_over_the_limit(tokens);
        }
        Ok(turn)
    }

    fn make_room_if_over_the_limit(&mut self, tokens: u64) {
        let is_over = self.compact_at.is_some_and(|limit| tokens >= limit);
        if is_over && compact(&mut self.history) {
            (self.observer)(Event::ContextCompacted);
        }
    }

    /// Records the turn and answers it. Returns the closing words once the model has finished.
    fn respond_to(&mut self, turn: ModelTurn) -> Result<Option<String>, AgentError> {
        let reply = match turn.stop {
            StopReason::Refused => return Err(AgentError::Refused),
            StopReason::Finished => return Ok(Some(self.record_final(turn))),
            StopReason::Paused => None,
            StopReason::WantsTools => Some(Entry::ToolResults(self.run_tools(&turn.tool_calls))),
            StopReason::RanOutOfRoom => Some(reply_to_cut_off(&turn.tool_calls)),
        };
        self.history.push(Entry::Model(turn));
        self.history.extend(reply);
        Ok(None)
    }

    fn record_final(&mut self, turn: ModelTurn) -> String {
        let closing_words = turn.text.clone();
        self.history.push(Entry::Model(turn));
        closing_words
    }

    fn run_tools(&mut self, calls: &[ToolCall]) -> Vec<ToolResult> {
        calls.iter().map(|call| ToolResult { call_id: call.id.clone(), output: self.run_tool(call) }).collect()
    }

    fn run_tool(&mut self, call: &ToolCall) -> ToolOutput {
        let input = match &call.input {
            ToolInput::Valid(input) => input,
            ToolInput::Unparseable(raw) => return ToolOutput::failure(json!({ "INVALID_JSON": raw }).to_string()),
        };
        (self.observer)(Event::ToolStarted { name: &call.name, input });
        let output = self.toolbox.run(&call.name, input);
        (self.observer)(Event::ToolFinished { name: &call.name, succeeded: !output.is_error });
        output
    }
}

impl Agent<'_> {
    /// A wait that the musician cut short ends in having stopped, not in the refusal that was being waited out.
    fn stopped_or(&self, refusal: ModelError) -> AgentError {
        if self.stop.is_asked() { AgentError::Stopped } else { AgentError::Model(refusal) }
    }
}

/// Asks the model for its next turn. A service that says it is asked too often is left alone for a while and
/// asked again, for as long as `wait_out` says each wait passed whole.
fn turn_asked_patiently(model: &mut dyn Model, brief: &Brief, mut wait_out: impl FnMut(Duration) -> bool) -> Result<ModelTurn, ModelError> {
    let mut times_waited = 0;
    loop {
        let refusal = match model.next_turn(brief) {
            Err(refusal) => refusal,
            turn => return turn,
        };
        match wait_before_asking_again(&refusal, times_waited) {
            Some(wait) if wait_out(wait) => times_waited += 1,
            _ => return Err(refusal),
        }
    }
}

/// How long to leave a service alone before asking it again, when that is worth doing: as long as it asks,
/// or as long as usual when it names no time. Nothing once it has been waited for often enough, when it asks
/// for longer than is kept, and for every other kind of refusal.
fn wait_before_asking_again(refusal: &ModelError, times_waited: usize) -> Option<Duration> {
    let ModelError::AskedTooOften { asks_to_wait, .. } = refusal else { return None };
    let usual = USUAL_WAITS_WHEN_ASKED_TOO_OFTEN.get(times_waited)?;
    Some(asks_to_wait.unwrap_or(*usual)).filter(|wait| *wait <= LONGEST_WAIT_KEPT)
}

impl Worker for Agent<'_> {
    /// Lets the model work until it stops asking for tools.
    fn work_on(&mut self, message: &str) -> Result<String, AgentError> {
        self.history.push(Entry::User(message.to_string()));
        for _ in 0..self.turn_limit {
            if self.stop.is_asked() {
                return Err(AgentError::Stopped);
            }
            let turn = self.ask_model().map_err(|refusal| self.stopped_or(refusal))?;
            if let Some(closing_words) = self.respond_to(turn)? {
                return Ok(closing_words);
            }
        }
        Err(AgentError::TooManyTurns(self.turn_limit))
    }
}

/// Drops what tools handed back long ago from the conversation: their words are cut short and their pictures
/// let go. The newest results are left whole, and so is every turn of the model's, so the conversation still
/// reads in order. The model can ask any tool again for what it no longer has. Says whether anything was dropped.
fn compact(history: &mut [Entry]) -> bool {
    let results_at: Vec<usize> = history.iter().enumerate().filter(|(_, entry)| matches!(entry, Entry::ToolResults(_))).map(|(place, _)| place).collect();
    let old_ones = &results_at[..results_at.len().saturating_sub(RESULTS_LEFT_WHOLE)];
    let mut dropped_anything = false;
    for place in old_ones {
        if let Entry::ToolResults(results) = &mut history[*place] {
            dropped_anything |= results.iter_mut().map(|result| cut_short(&mut result.output)).fold(false, |any, cut| any || cut);
        }
    }
    dropped_anything
}

/// Says whether there was anything to cut.
fn cut_short(output: &mut ToolOutput) -> bool {
    let is_long = output.content.chars().count() > LONGEST_OLD_RESULT;
    if !is_long && output.illustrations.is_empty() {
        return false;
    }
    if is_long {
        let opening: String = output.content.chars().take(LONGEST_OLD_RESULT).collect();
        output.content = format!("{opening}{DROPPED_TO_MAKE_ROOM}");
    }
    output.illustrations.clear();
    true
}

/// A turn that hit the length limit may hold half-written tool requests. None of them are run.
fn reply_to_cut_off(calls: &[ToolCall]) -> Entry {
    if calls.is_empty() {
        return Entry::User(CONTINUE_AFTER_CUT_OFF.to_string());
    }
    let not_done = |call: &ToolCall| ToolResult { call_id: call.id.clone(), output: ToolOutput::failure(CUT_OFF_BEFORE_COMPLETE) };
    Entry::ToolResults(calls.iter().map(not_done).collect())
}

/// Stand-ins for a real model and real tools, shared by the tests of every module that drives an agent.
#[cfg(test)]
pub mod test_support {
    use std::collections::VecDeque;

    use super::*;

    /// A model that plays back turns written in advance.
    #[derive(Default)]
    pub struct ScriptedModel {
        refusals: VecDeque<ModelError>,
        turns: VecDeque<ModelTurn>,
        pub histories_seen: Vec<Vec<Entry>>,
    }

    impl ScriptedModel {
        pub fn playing(turns: impl IntoIterator<Item = ModelTurn>) -> Self {
            Self { turns: turns.into_iter().collect(), ..Self::default() }
        }

        /// Its service says no this many times, in these words, before the first turn is played.
        pub fn refusing_first(mut self, refusals: impl IntoIterator<Item = ModelError>) -> Self {
            self.refusals = refusals.into_iter().collect();
            self
        }
    }

    impl Model for ScriptedModel {
        fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError> {
            self.histories_seen.push(brief.history.to_vec());
            if let Some(refusal) = self.refusals.pop_front() {
                return Err(refusal);
            }
            self.turns.pop_front().ok_or_else(|| ModelError::Garbled("the script ran out of turns".to_string()))
        }
    }

    pub fn says(text: &str) -> ModelTurn {
        ModelTurn { text: text.to_string(), tool_calls: Vec::new(), stop: StopReason::Finished, replay: json!([]), tokens_in_context: None }
    }

    pub fn calls(tool_name: &str, input: Value) -> ModelTurn {
        let call = ToolCall { id: format!("call-{tool_name}"), name: tool_name.to_string(), input: ToolInput::Valid(input) };
        ModelTurn { text: String::new(), tool_calls: vec![call], stop: StopReason::WantsTools, replay: json!([]), tokens_in_context: None }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{ScriptedModel, calls, says};
    use super::*;

    #[derive(Default)]
    struct RecordingToolbox {
        runs: Vec<(String, Value)>,
    }

    impl Toolbox for RecordingToolbox {
        fn definitions(&self) -> Vec<ToolDefinition> {
            Vec::new()
        }

        fn run(&mut self, tool_name: &str, input: &Value) -> ToolOutput {
            self.runs.push((tool_name.to_string(), input.clone()));
            ToolOutput::success("done")
        }
    }

    fn turn_with(stop: StopReason, tool_calls: Vec<ToolCall>) -> ModelTurn {
        ModelTurn { text: String::new(), tool_calls, stop, replay: json!([]), tokens_in_context: None }
    }

    fn call_with(input: ToolInput) -> ToolCall {
        ToolCall { id: "call-1".to_string(), name: "write".to_string(), input }
    }

    fn last_tool_results(history: &[Entry]) -> &[ToolResult] {
        match history.last() {
            Some(Entry::ToolResults(results)) => results,
            other => panic!("expected tool results at the end of the history, found {other:?}"),
        }
    }

    #[test]
    fn runs_the_requested_tool_then_returns_the_closing_words() {
        let mut model = ScriptedModel::playing([calls("write", json!({"path": "a"})), says("All done.")]);
        let mut toolbox = RecordingToolbox::default();

        let closing_words = Agent::new(&mut model, &mut toolbox, "be helpful").work_on("make a thing").unwrap();

        assert_eq!(closing_words, "All done.");
        assert_eq!(toolbox.runs, [("write".to_string(), json!({"path": "a"}))]);
    }

    #[test]
    fn the_model_sees_the_tool_result_on_its_next_turn() {
        let mut model = ScriptedModel::playing([calls("write", json!({})), says("ok")]);
        let mut toolbox = RecordingToolbox::default();

        Agent::new(&mut model, &mut toolbox, "").work_on("go").unwrap();

        let results = last_tool_results(&model.histories_seen[1]);
        assert_eq!(results, [ToolResult { call_id: "call-write".to_string(), output: ToolOutput::success("done") }]);
    }

    /// A toolbox whose tool hands back a long reading and a picture, as reading a file or styling a screen does.
    struct ReadingToolbox;

    impl Toolbox for ReadingToolbox {
        fn definitions(&self) -> Vec<ToolDefinition> {
            Vec::new()
        }

        fn run(&mut self, _tool_name: &str, _input: &Value) -> ToolOutput {
            let picture = Illustration { caption: "the page".to_string(), png: vec![1, 2, 3] };
            ToolOutput::success("x".repeat(LONGEST_OLD_RESULT * 3)).illustrated_with(vec![picture])
        }
    }

    fn reads_holding(tokens: u64) -> ModelTurn {
        ModelTurn { tokens_in_context: Some(tokens), ..calls("read", json!({})) }
    }

    fn results_in(history: &[Entry]) -> Vec<&ToolOutput> {
        history.iter().filter_map(|entry| if let Entry::ToolResults(results) = entry { Some(&results[0].output) } else { None }).collect()
    }

    #[test]
    fn past_the_limit_old_tool_results_are_cut_short_and_the_newest_are_left_whole() {
        let mut model = ScriptedModel::playing([reads_holding(10), reads_holding(20), reads_holding(30), reads_holding(900), says("done")]);
        let mut events = Vec::new();

        Agent::new(&mut model, &mut ReadingToolbox, "")
            .compacting_at(Some(500))
            .watched_by(|event| events.push(match event {
                Event::ContextUsed { tokens } => format!("holds {tokens}"),
                Event::ContextCompacted => "compacted".to_string(),
                _ => String::new(),
            }))
            .work_on("go")
            .unwrap();

        let seen_last = results_in(model.histories_seen.last().unwrap());
        assert_eq!(seen_last.len(), 4);
        assert!(seen_last[0].content.ends_with(DROPPED_TO_MAKE_ROOM) && seen_last[0].illustrations.is_empty());
        assert_eq!(seen_last[0].content.chars().count(), LONGEST_OLD_RESULT + DROPPED_TO_MAKE_ROOM.chars().count());
        assert_eq!(seen_last[1].content.chars().count(), LONGEST_OLD_RESULT * 3, "the newest sets of results before the limit was passed stay whole");
        assert_eq!(seen_last[3].illustrations.len(), 1);
        events.retain(|event| !event.is_empty());
        assert_eq!(events, ["holds 10", "holds 20", "holds 30", "holds 900", "compacted"]);
    }

    #[test]
    fn without_a_limit_nothing_is_ever_dropped() {
        let mut model = ScriptedModel::playing([reads_holding(900_000), reads_holding(900_000), reads_holding(900_000), says("done")]);

        Agent::new(&mut model, &mut ReadingToolbox, "").work_on("go").unwrap();

        assert!(results_in(model.histories_seen.last().unwrap()).iter().all(|output| output.illustrations.len() == 1));
    }

    #[test]
    fn input_that_is_not_valid_json_is_reported_back_and_never_run() {
        let broken = call_with(ToolInput::Unparseable("{\"path\": ".to_string()));
        let mut model = ScriptedModel::playing([turn_with(StopReason::WantsTools, vec![broken]), says("ok")]);
        let mut toolbox = RecordingToolbox::default();

        Agent::new(&mut model, &mut toolbox, "").work_on("go").unwrap();

        assert!(toolbox.runs.is_empty());
        let results = last_tool_results(&model.histories_seen[1]);
        assert!(results[0].output.is_error);
        assert!(results[0].output.content.contains("INVALID_JSON"));
    }

    #[test]
    fn a_tool_request_cut_off_by_the_length_limit_is_never_run() {
        let half_written = call_with(ToolInput::Valid(json!({"path": "src/a.cpp"})));
        let mut model = ScriptedModel::playing([turn_with(StopReason::RanOutOfRoom, vec![half_written]), says("ok")]);
        let mut toolbox = RecordingToolbox::default();

        Agent::new(&mut model, &mut toolbox, "").work_on("go").unwrap();

        assert!(toolbox.runs.is_empty());
        assert_eq!(last_tool_results(&model.histories_seen[1])[0].output, ToolOutput::failure(CUT_OFF_BEFORE_COMPLETE));
    }

    #[test]
    fn a_refusal_stops_the_work_without_running_tools() {
        let call = call_with(ToolInput::Valid(json!({})));
        let mut model = ScriptedModel::playing([turn_with(StopReason::Refused, vec![call])]);
        let mut toolbox = RecordingToolbox::default();

        let result = Agent::new(&mut model, &mut toolbox, "").work_on("go");

        assert!(matches!(result, Err(AgentError::Refused)));
        assert!(toolbox.runs.is_empty());
    }

    #[test]
    fn a_paused_turn_is_kept_and_the_model_is_simply_asked_again() {
        let mut model = ScriptedModel::playing([turn_with(StopReason::Paused, Vec::new()), says("done")]);
        let mut toolbox = RecordingToolbox::default();
        let mut agent = Agent::new(&mut model, &mut toolbox, "");

        let closing_words = agent.work_on("go").unwrap();

        assert_eq!(closing_words, "done");
        assert!(matches!(agent.history(), [Entry::User(_), Entry::Model(_), Entry::Model(_)]));
    }

    #[test]
    fn a_model_that_never_finishes_is_stopped_at_the_turn_limit() {
        let endless = std::iter::repeat_with(|| calls("write", json!({}))).take(10);
        let mut model = ScriptedModel::playing(endless);
        let mut toolbox = RecordingToolbox::default();

        let result = Agent::new(&mut model, &mut toolbox, "").with_turn_limit(3).work_on("go");

        assert!(matches!(result, Err(AgentError::TooManyTurns(3))));
        assert_eq!(toolbox.runs.len(), 3);
    }

    fn asked_too_often(asks_to_wait: Option<Duration>) -> ModelError {
        ModelError::AskedTooOften { message: "slow down".to_string(), asks_to_wait }
    }

    const A_MOMENT: Option<Duration> = Some(Duration::from_millis(1));

    #[test]
    fn a_service_that_is_asked_too_often_is_left_alone_as_long_as_it_asks_and_then_asked_again() {
        let mut model = ScriptedModel::playing([says("All done.")]).refusing_first([asked_too_often(A_MOMENT), asked_too_often(A_MOMENT)]);
        let mut toolbox = RecordingToolbox::default();
        let mut waits_told = Vec::new();
        let noting_the_waits = |event: Event| {
            if let Event::WaitingToAskAgain { wait } = event {
                waits_told.push(wait);
            }
        };

        let closing_words = Agent::new(&mut model, &mut toolbox, "").watched_by(noting_the_waits).work_on("go").unwrap();

        assert_eq!(closing_words, "All done.");
        assert_eq!(waits_told, [A_MOMENT.unwrap(), A_MOMENT.unwrap()]);
    }

    #[test]
    fn a_service_that_still_says_no_after_its_waits_has_its_refusal_stand() {
        let mut model = ScriptedModel::playing([says("never reached")]).refusing_first([A_MOMENT, A_MOMENT, A_MOMENT].map(asked_too_often));
        let mut toolbox = RecordingToolbox::default();

        let result = Agent::new(&mut model, &mut toolbox, "").work_on("go");

        assert!(matches!(result, Err(AgentError::Model(ModelError::AskedTooOften { .. }))));
        assert_eq!(model.histories_seen.len(), 3);
    }

    #[test]
    fn a_wait_the_musician_cuts_short_ends_in_having_stopped() {
        let mut model = ScriptedModel::playing([says("never reached")]).refusing_first([asked_too_often(Some(Duration::from_secs(100)))]);
        let mut toolbox = RecordingToolbox::default();
        let stop = StopSignal::default();
        let asks_to_stop_once_it_waits = |event: Event| {
            if matches!(event, Event::WaitingToAskAgain { .. }) {
                stop.ask_to_stop();
            }
        };

        let result = Agent::new(&mut model, &mut toolbox, "").stopped_by(stop.clone()).watched_by(asks_to_stop_once_it_waits).work_on("go");

        assert!(matches!(result, Err(AgentError::Stopped)));
    }

    #[test]
    fn how_long_a_service_is_left_alone_is_what_it_asks_or_else_the_usual_and_only_a_few_times() {
        let names_no_time = asked_too_often(None);
        let asks_for_longer_than_is_kept = asked_too_often(Some(LONGEST_WAIT_KEPT + Duration::from_secs(1)));
        let another_refusal = ModelError::Rejected { status: 401, message: "no".to_string() };

        assert_eq!(wait_before_asking_again(&names_no_time, 0), Some(USUAL_WAITS_WHEN_ASKED_TOO_OFTEN[0]));
        assert_eq!(wait_before_asking_again(&names_no_time, 1), Some(USUAL_WAITS_WHEN_ASKED_TOO_OFTEN[1]));
        assert_eq!(wait_before_asking_again(&names_no_time, 2), None);
        assert_eq!(wait_before_asking_again(&asked_too_often(Some(Duration::from_secs(45))), 0), Some(Duration::from_secs(45)));
        assert_eq!(wait_before_asking_again(&asks_for_longer_than_is_kept, 0), None);
        assert_eq!(wait_before_asking_again(&another_refusal, 0), None);
    }

    #[test]
    fn a_service_asked_too_often_is_said_in_plain_words_with_how_long_it_asks_to_be_left_alone() {
        let naming_no_time = ModelError::AskedTooOften { message: "Provider returned error. Chutes said: rate limited.".to_string(), asks_to_wait: None }.to_string();
        let asking_for_an_hour = asked_too_often(Some(Duration::from_secs(3600))).to_string();

        assert_eq!(naming_no_time, "The AI service said no because it is being asked too often (429): Provider returned error. Chutes said: rate limited. Send your message again in a few minutes, or pick another model.");
        assert!(asking_for_an_hour.ends_with("slow down. It asks to be left alone for about 60 minutes: send your message again after that, or pick another model."), "{asking_for_an_hour}");
    }

    #[test]
    fn a_length_of_time_is_said_in_seconds_then_minutes_then_hours() {
        let said = |seconds: u64| span_in_words(Duration::from_secs(seconds));

        assert_eq!([said(0), said(1), said(20), said(90)], ["1 second", "1 second", "20 seconds", "90 seconds"]);
        assert_eq!([said(91), said(120), said(5400)], ["about 2 minutes", "about 2 minutes", "about 90 minutes"]);
        assert_eq!([said(5401), said(86_400)], ["about 2 hours", "about 24 hours"]);
    }

    #[test]
    fn a_model_stopped_at_the_limit_is_told_how_many_rounds_it_had_and_how_to_carry_on() {
        let said = AgentError::TooManyTurns(DEFAULT_TURN_LIMIT).to_string();

        assert!(said.starts_with("The AI had not finished after 250 rounds of work"), "{said}");
        assert!(said.ends_with("say \"continue\" to let it carry on."), "{said}");
    }

    #[test]
    fn an_agent_asked_to_stop_gives_up_before_its_next_step() {
        let mut model = ScriptedModel::playing([calls("write", json!({})), says("never reached")]);
        let mut toolbox = RecordingToolbox::default();
        let stop = StopSignal::default();
        let asks_to_stop_after_the_first_step = |event: Event| {
            if matches!(event, Event::ToolFinished { .. }) {
                stop.ask_to_stop();
            }
        };

        let result = Agent::new(&mut model, &mut toolbox, "").stopped_by(stop.clone()).watched_by(asks_to_stop_after_the_first_step).work_on("go");

        assert!(matches!(result, Err(AgentError::Stopped)));
        assert_eq!(toolbox.runs.len(), 1);
    }

    #[test]
    fn a_second_message_continues_the_same_conversation() {
        let mut model = ScriptedModel::playing([says("first"), says("second")]);
        let mut toolbox = RecordingToolbox::default();
        let mut agent = Agent::new(&mut model, &mut toolbox, "");

        agent.work_on("one").unwrap();
        agent.work_on("two").unwrap();

        assert_eq!(agent.history().len(), 4);
    }
}
