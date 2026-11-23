//! Claude as the workbench's AI, over Anthropic's Messages API.
//!
//! Anthropic publishes no SDK for Rust, so this speaks the HTTP API directly. The reply is streamed,
//! because a turn that writes a whole source file is long enough to time out otherwise.

use std::io::BufRead;
use std::process::Stdio;

use crate::process::command_for;

use serde_json::{Value, json};

use crate::agent::{Brief, Entry, Illustration, Model, ModelError, ModelTurn, StopReason, ToolCall, ToolDefinition, ToolInput, ToolOutput, ToolResult};
use crate::ai_settings::{AiSettings, ClaudeModel, Effort};
use crate::model_http::{self, rejection};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2026-10-21";
const KEY_VARIABLE: &str = "ANTHROPIC_API_KEY";
const SIGN_IN_TOOL: &str = "ant";
const PRINT_TOKEN: [&str; 3] = ["auth", "print-credentials", "--access-token"];
/// Required whenever a request is authorised by an account sign-in instead of an API key.
const SIGN_IN_BETA: &str = "oauth-2025-04-20";
const MODEL_IF_NOT_CHOSEN: ClaudeModel = ClaudeModel::Opus;
const EFFORT_IF_NOT_CHOSEN: Effort = Effort::High;
const LONGEST_REPLY_IN_TOKENS: u32 = 64_000;
const WEB_LOOKUPS_PER_REQUEST: u32 = 8;
const CLIENT_TOOL_REQUEST: &str = "tool_use";
/// A request for a tool Anthropic runs itself, such as a web search. The workbench only keeps it to send back.
const SERVER_TOOL_REQUEST: &str = "server_tool_use";
/// Lets Anthropic hand a declined request to another Claude model inside the same call.
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// How the workbench proves to Anthropic who is asking. Neither kind is ever written to a log or a file.
enum Credential {
    ApiKey(String),
    /// An Anthropic Console account signed in with Anthropic's `ant` command-line tool.
    AccountSignIn,
}

impl Credential {
    /// A key the musician gave the workbench wins, then an API key in the environment. Failing both, an `ant`
    /// sign-in is used if there is one.
    fn find(given_key: Option<String>) -> Result<Self, ModelError> {
        match given_key.or_else(key_from_environment) {
            Some(key) => Ok(Credential::ApiKey(key)),
            None => sign_in_token().map(|_| Credential::AccountSignIn),
        }
    }

    /// The sign-in's token is short-lived, so it is fetched fresh for every request.
    fn headers(&self, model: ClaudeModel) -> Result<Vec<(&'static str, String)>, ModelError> {
        match self {
            Credential::ApiKey(key) => Ok(key_headers(key, model)),
            Credential::AccountSignIn => Ok(sign_in_headers(&sign_in_token()?, model)),
        }
    }
}

fn key_from_environment() -> Option<String> {
    std::env::var(KEY_VARIABLE).ok().filter(|key| !key.trim().is_empty())
}

fn key_headers(key: &str, model: ClaudeModel) -> Vec<(&'static str, String)> {
    with_optional_features(("x-api-key", key.to_string()), &[], model)
}

fn sign_in_headers(token: &str, model: ClaudeModel) -> Vec<(&'static str, String)> {
    with_optional_features(("authorization", format!("Bearer {token}")), &[SIGN_IN_BETA], model)
}

/// Adds the header that names the optional features a request relies on, when it relies on any.
fn with_optional_features(who_is_asking: (&'static str, String), always: &[&'static str], model: ClaudeModel) -> Vec<(&'static str, String)> {
    let mut features = always.to_vec();
    if model.has_server_side_fallback() {
        features.push(FALLBACK_BETA);
    }
    let mut headers = vec![who_is_asking];
    if !features.is_empty() {
        headers.push(("anthropic-beta", features.join(",")));
    }
    headers
}

/// Asks `ant` for the signed-in account's current token. Read through a pipe, so it never touches disk here.
fn sign_in_token() -> Result<String, ModelError> {
    let printed = command_for(SIGN_IN_TOOL).args(PRINT_TOKEN).stdin(Stdio::null()).stderr(Stdio::null()).output();
    printed
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| usable_token(&String::from_utf8_lossy(&output.stdout)))
        .ok_or_else(no_credential)
}

/// A token is one unbroken word. Anything else means `ant` printed a message instead.
fn usable_token(printed: &str) -> Option<String> {
    let token = printed.trim();
    let is_one_word = !token.is_empty() && !token.contains(char::is_whitespace);
    is_one_word.then(|| token.to_string())
}

fn no_credential() -> ModelError {
    ModelError::NoCredential(format!("Sign in with `ant auth login`, or set {KEY_VARIABLE}, then try again."))
}

pub struct Claude {
    credential: Credential,
    http: ureq::Agent,
    choices: Choices,
}

/// What every request on this connection asks for.
#[derive(Clone, Copy)]
struct Choices {
    model: ClaudeModel,
    effort: Effort,
    web_access: bool,
}

impl Choices {
    fn from(settings: AiSettings) -> Self {
        Self {
            model: settings.model.unwrap_or(MODEL_IF_NOT_CHOSEN),
            effort: settings.effort.unwrap_or(EFFORT_IF_NOT_CHOSEN),
            web_access: settings.web_access,
        }
    }
}

impl Claude {
    pub fn from_environment(settings: AiSettings) -> Result<Self, ModelError> {
        Self::signed_in_with(None, settings)
    }

    /// Uses the key handed over when there is one. It is held in memory only, for as long as this work lasts.
    pub fn signed_in_with(key: Option<String>, settings: AiSettings) -> Result<Self, ModelError> {
        Ok(Self { credential: Credential::find(key)?, http: model_http::client(), choices: Choices::from(settings) })
    }

    fn open_event_stream(&self, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        model_http::with_retries(|| self.send(body))
    }

    fn send(&self, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        let mut request = self.http.post(API_URL).header("content-type", "application/json").header("anthropic-version", API_VERSION);
        for (name, value) in self.credential.headers(self.choices.model)? {
            request = request.header(name, value);
        }
        let reply = request.send(body).map_err(|error| ModelError::Unreachable(error.to_string()))?;
        model_http::event_stream_of(reply, rejection)
    }
}

impl Model for Claude {
    fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError> {
        let body = request_body(brief, self.choices).to_string();
        assemble_turn(self.open_event_stream(&body)?)
    }
}

// ---------------------------------------------------------------------------
// Building the request
// ---------------------------------------------------------------------------

fn request_body(brief: &Brief, choices: Choices) -> Value {
    let Choices { model, effort, web_access } = choices;
    let mut tools: Vec<Value> = brief.tools.iter().map(wire_tool).collect();
    if web_access {
        tools.extend(web_tools(model));
    }
    let mut body = json!({
        "model": model.id(),
        "max_tokens": LONGEST_REPLY_IN_TOKENS,
        "stream": true,
        "thinking": { "type": "adaptive" },
        "output_config": { "effort": effort.word() },
        "cache_control": { "type": "ephemeral" },
        "system": brief.instructions,
        "tools": tools,
        "messages": brief.history.iter().filter_map(wire_message).collect::<Vec<Value>>(),
    });
    if model.has_server_side_fallback() {
        body["fallbacks"] = json!("default");
    }
    body
}

/// Anthropic runs these itself and hands back what it found, so the workbench never touches the web.
/// Models without the newer, filtering kind get a plain search and no page reader.
fn web_tools(model: ClaudeModel) -> Vec<Value> {
    if !model.has_filtering_web_tools() {
        return vec![json!({ "type": "web_search_20250305", "name": "web_search", "max_uses": WEB_LOOKUPS_PER_REQUEST })];
    }
    vec![
        json!({ "type": "web_search_20260209", "name": "web_search", "max_uses": WEB_LOOKUPS_PER_REQUEST }),
        json!({ "type": "web_fetch_20260209", "name": "web_fetch", "max_uses": WEB_LOOKUPS_PER_REQUEST }),
    ]
}

/// Tool input is streamed as it is written, so the workbench checks it itself before running anything.
fn wire_tool(tool: &ToolDefinition) -> Value {
    json!({
        "name": tool.name,
        "description": tool.description,
        "input_schema": tool.input_schema,
        "eager_input_streaming": true,
    })
}

fn wire_message(entry: &Entry) -> Option<Value> {
    match entry {
        Entry::User(text) => Some(json!({ "role": "user", "content": text })),
        Entry::Model(turn) if has_content(&turn.replay) => Some(json!({ "role": "assistant", "content": turn.replay })),
        Entry::Model(_) => None,
        Entry::ToolResults(results) => {
            Some(json!({ "role": "user", "content": results.iter().map(wire_tool_result).collect::<Vec<Value>>() }))
        }
    }
}

fn has_content(replay: &Value) -> bool {
    replay.as_array().is_some_and(|blocks| !blocks.is_empty())
}

fn wire_tool_result(result: &ToolResult) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": result.call_id,
        "content": wire_tool_content(&result.output),
        "is_error": result.output.is_error,
    })
}

/// Words alone are sent as plain text. With pictures, the words come first and each picture follows its caption.
fn wire_tool_content(output: &ToolOutput) -> Value {
    if output.illustrations.is_empty() {
        return json!(output.content);
    }
    let words = json!({ "type": "text", "text": output.content });
    Value::Array(std::iter::once(words).chain(output.illustrations.iter().flat_map(wire_illustration)).collect())
}

fn wire_illustration(illustration: &Illustration) -> [Value; 2] {
    let picture = json!({ "type": "base64", "media_type": "image/png", "data": illustration.png_as_base64() });
    [json!({ "type": "text", "text": illustration.caption }), json!({ "type": "image", "source": picture })]
}

// ---------------------------------------------------------------------------
// Reading the streamed reply
// ---------------------------------------------------------------------------

fn assemble_turn(stream: impl BufRead) -> Result<ModelTurn, ModelError> {
    let mut assembler = TurnAssembler::default();
    for line in stream.lines() {
        let line = line.map_err(|error| ModelError::Unreachable(format!("the reply broke off: {error}")))?;
        if let Some(event) = event_in(&line) {
            assembler.take(&event);
        }
    }
    assembler.finish()
}

/// Each event arrives as a `data: {...}` line. Every other line is framing.
fn event_in(line: &str) -> Option<Value> {
    serde_json::from_str(line.strip_prefix("data:")?.trim()).ok()
}

#[derive(Default)]
struct TurnAssembler {
    blocks: Vec<BlockInProgress>,
    stop_reason: Option<String>,
    failure: Option<String>,
    tokens_read: Option<u64>,
    tokens_written: Option<u64>,
}

/// What Anthropic counts as read for one request: what was sent fresh, and what it had kept from before.
const TOKENS_READ: [&str; 3] = ["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"];

struct BlockInProgress {
    block: Value,
    tool_input_so_far: String,
}

impl TurnAssembler {
    fn take(&mut self, event: &Value) {
        match event["type"].as_str() {
            Some("content_block_start") => self.start_block(&event["content_block"]),
            Some("content_block_delta") => self.extend_block(event),
            Some("message_start") => self.note_tokens(&event["message"]["usage"]),
            Some("message_delta") => {
                self.note_stop_reason(&event["delta"]["stop_reason"]);
                self.note_tokens(&event["usage"]);
            }
            Some("error") => self.failure = Some(event["error"]["message"].as_str().unwrap_or("unknown error").to_string()),
            _ => {}
        }
    }

    fn start_block(&mut self, block: &Value) {
        self.blocks.push(BlockInProgress { block: block.clone(), tool_input_so_far: String::new() });
    }

    fn extend_block(&mut self, event: &Value) {
        let position = event["index"].as_u64().unwrap_or(u64::MAX) as usize;
        if let Some(in_progress) = self.blocks.get_mut(position) {
            in_progress.extend_with(&event["delta"]);
        }
    }

    /// The count of what was read comes with the start of the reply, and the count of what was written with its end.
    fn note_tokens(&mut self, usage: &Value) {
        if !usage["input_tokens"].is_null() {
            self.tokens_read = Some(TOKENS_READ.iter().filter_map(|count| usage[*count].as_u64()).sum());
        }
        if let Some(written) = usage["output_tokens"].as_u64() {
            self.tokens_written = Some(written);
        }
    }

    /// What the conversation holds now: everything the model read for this turn, and the turn it wrote.
    fn tokens_in_context(&self) -> Option<u64> {
        self.tokens_read.map(|read| read + self.tokens_written.unwrap_or(0))
    }

    fn note_stop_reason(&mut self, stop_reason: &Value) {
        if let Some(stop_reason) = stop_reason.as_str() {
            self.stop_reason = Some(stop_reason.to_string());
        }
    }

    fn finish(self) -> Result<ModelTurn, ModelError> {
        if let Some(failure) = self.failure {
            return Err(ModelError::Unreachable(format!("the reply broke off: {failure}")));
        }
        let stop = stop_reason_from(self.stop_reason.as_deref())?;
        let tokens_in_context = self.tokens_in_context();
        let mut tool_calls = Vec::new();
        let finished: Vec<Value> = self.blocks.into_iter().map(|block| block.finish(&mut tool_calls)).collect();
        Ok(ModelTurn { text: visible_text_of(&finished), tool_calls, stop, replay: Value::Array(finished), tokens_in_context })
    }
}

impl BlockInProgress {
    fn extend_with(&mut self, delta: &Value) {
        match delta["type"].as_str() {
            Some("text_delta") => append_text(&mut self.block, "text", &delta["text"]),
            Some("thinking_delta") => append_text(&mut self.block, "thinking", &delta["thinking"]),
            Some("signature_delta") => append_text(&mut self.block, "signature", &delta["signature"]),
            Some("input_json_delta") => self.tool_input_so_far.push_str(delta["partial_json"].as_str().unwrap_or("")),
            Some("citations_delta") => append_citation(&mut self.block, &delta["citation"]),
            _ => {}
        }
    }

    /// Returns the block as it must be sent back later, and records the tool request it holds, if any.
    fn finish(mut self, tool_calls: &mut Vec<ToolCall>) -> Value {
        let kind = text_of(&self.block["type"]);
        if kind != CLIENT_TOOL_REQUEST && kind != SERVER_TOOL_REQUEST {
            return self.block;
        }
        let input = ToolInput::read_from(&self.tool_input_so_far);
        self.block["input"] = match &input {
            ToolInput::Valid(input) => input.clone(),
            ToolInput::Unparseable(_) => json!({}),
        };
        if kind == CLIENT_TOOL_REQUEST {
            tool_calls.push(ToolCall { id: text_of(&self.block["id"]), name: text_of(&self.block["name"]), input });
        }
        self.block
    }
}

fn stop_reason_from(stated: Option<&str>) -> Result<StopReason, ModelError> {
    match stated {
        Some("end_turn" | "stop_sequence") => Ok(StopReason::Finished),
        Some("tool_use") => Ok(StopReason::WantsTools),
        Some("pause_turn") => Ok(StopReason::Paused),
        Some("max_tokens" | "model_context_window_exceeded") => Ok(StopReason::RanOutOfRoom),
        Some("refusal") => Ok(StopReason::Refused),
        Some(other) => Err(ModelError::Garbled(format!("it stopped for an unexpected reason: {other}"))),
        None => Err(ModelError::Garbled("it ended without saying why".to_string())),
    }
}

fn visible_text_of(blocks: &[Value]) -> String {
    let texts: Vec<&str> = blocks.iter().filter(|block| block["type"] == "text").filter_map(|block| block["text"].as_str()).collect();
    texts.join("\n\n").trim().to_string()
}

fn append_text(block: &mut Value, field: &str, addition: &Value) {
    let extended = format!("{}{}", block[field].as_str().unwrap_or(""), addition.as_str().unwrap_or(""));
    block[field] = Value::String(extended);
}

/// Where a sentence came from arrives separately from the sentence, and has to go back with it.
fn append_citation(block: &mut Value, citation: &Value) {
    match block["citations"].as_array_mut() {
        Some(citations) => citations.push(citation.clone()),
        None => block["citations"] = json!([citation]),
    }
}

fn text_of(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTHING_TO_SAY: Brief = Brief { instructions: "", tools: &[], history: &[] };

    fn choices(model: ClaudeModel, effort: Effort) -> Choices {
        Choices { model, effort, web_access: false }
    }

    fn default_request(brief: &Brief) -> Value {
        request_body(brief, Choices::from(AiSettings::default()))
    }

    fn tool_types_with_the_web_open(model: ClaudeModel) -> Vec<String> {
        let body = request_body(&NOTHING_TO_SAY, Choices { web_access: true, ..choices(model, Effort::High) });
        body["tools"].as_array().unwrap().iter().map(|tool| text_of(&tool["type"])).collect()
    }

    fn turn_from(events: &[Value]) -> Result<ModelTurn, ModelError> {
        let stream: String = events.iter().map(|event| format!("event: {}\ndata: {event}\n\n", event["type"])).collect();
        assemble_turn(stream.as_bytes())
    }

    fn block_start(position: usize, block: Value) -> Value {
        json!({ "type": "content_block_start", "index": position, "content_block": block })
    }

    fn block_delta(position: usize, delta: Value) -> Value {
        json!({ "type": "content_block_delta", "index": position, "delta": delta })
    }

    fn stopped_because(stop_reason: &str) -> Value {
        json!({ "type": "message_delta", "delta": { "stop_reason": stop_reason }, "usage": { "output_tokens": 1 } })
    }

    fn write_request(input_fragments: &[&str]) -> Vec<Value> {
        let mut events = vec![block_start(0, json!({ "type": "tool_use", "id": "toolu_1", "name": "write_plugin_file", "input": {} }))];
        let fragment = |text: &&str| block_delta(0, json!({ "type": "input_json_delta", "partial_json": text }));
        events.extend(input_fragments.iter().map(fragment));
        events
    }

    #[test]
    fn the_tokens_the_conversation_holds_are_what_was_read_and_what_was_written() {
        let read = json!({ "type": "message_start", "message": { "usage": { "input_tokens": 20, "cache_read_input_tokens": 9_000, "cache_creation_input_tokens": 480, "output_tokens": 1 } } });
        let written = json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" }, "usage": { "output_tokens": 500 } });

        let counted = turn_from(&[read, written]).unwrap();
        let not_counted = turn_from(&[json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" } })]).unwrap();

        assert_eq!(counted.tokens_in_context, Some(10_000));
        assert_eq!(not_counted.tokens_in_context, None);
    }

    #[test]
    fn text_arriving_in_pieces_is_joined() {
        let turn = turn_from(&[
            block_start(0, json!({ "type": "text", "text": "" })),
            block_delta(0, json!({ "type": "text_delta", "text": "Here is " })),
            block_delta(0, json!({ "type": "text_delta", "text": "your delay." })),
            stopped_because("end_turn"),
        ])
        .unwrap();

        assert_eq!(turn.text, "Here is your delay.");
        assert_eq!(turn.stop, StopReason::Finished);
        assert!(turn.tool_calls.is_empty());
    }

    #[test]
    fn a_tool_request_arriving_in_pieces_is_joined_and_parsed() {
        let mut events = write_request(&["{\"path\": \"src/A.cpp\", ", "\"contents\": \"// a\"}"]);
        events.push(stopped_because("tool_use"));

        let turn = turn_from(&events).unwrap();

        let expected_input = json!({ "path": "src/A.cpp", "contents": "// a" });
        assert_eq!(turn.stop, StopReason::WantsTools);
        assert_eq!(
            turn.tool_calls,
            [ToolCall { id: "toolu_1".to_string(), name: "write_plugin_file".to_string(), input: ToolInput::Valid(expected_input.clone()) }]
        );
        assert_eq!(turn.replay[0]["input"], expected_input);
    }

    #[test]
    fn a_tool_request_with_no_input_gets_an_empty_object() {
        let mut events = write_request(&[]);
        events.push(stopped_because("tool_use"));

        let turn = turn_from(&events).unwrap();

        assert_eq!(turn.tool_calls[0].input, ToolInput::Valid(json!({})));
    }

    #[test]
    fn broken_tool_input_is_flagged_and_replayed_as_an_empty_object() {
        let mut events = write_request(&["{\"path\": \"src/A.cpp\", \"contents\": \"// unfinished"]);
        events.push(stopped_because("tool_use"));

        let turn = turn_from(&events).unwrap();

        assert!(matches!(turn.tool_calls[0].input, ToolInput::Unparseable(_)));
        assert_eq!(turn.replay[0]["input"], json!({}));
    }

    #[test]
    fn thinking_is_kept_exactly_so_it_can_be_sent_back() {
        let turn = turn_from(&[
            block_start(0, json!({ "type": "thinking", "thinking": "", "signature": "" })),
            block_delta(0, json!({ "type": "signature_delta", "signature": "abc" })),
            block_delta(0, json!({ "type": "signature_delta", "signature": "123" })),
            block_start(1, json!({ "type": "text", "text": "" })),
            block_delta(1, json!({ "type": "text_delta", "text": "Done." })),
            stopped_because("end_turn"),
        ])
        .unwrap();

        assert_eq!(turn.replay[0], json!({ "type": "thinking", "thinking": "", "signature": "abc123" }));
        assert_eq!(turn.text, "Done.");
    }

    #[test]
    fn the_length_limit_and_a_refusal_are_told_apart_from_a_normal_finish() {
        assert_eq!(turn_from(&[stopped_because("max_tokens")]).unwrap().stop, StopReason::RanOutOfRoom);
        assert_eq!(turn_from(&[stopped_because("refusal")]).unwrap().stop, StopReason::Refused);
    }

    #[test]
    fn a_reply_that_breaks_off_or_never_says_why_it_stopped_is_an_error() {
        let broke_off = turn_from(&[json!({ "type": "error", "error": { "type": "overloaded_error", "message": "Overloaded" } })]);
        let never_stopped = turn_from(&[block_start(0, json!({ "type": "text", "text": "" }))]);

        assert!(matches!(broke_off, Err(ModelError::Unreachable(_))));
        assert!(matches!(never_stopped, Err(ModelError::Garbled(_))));
    }

    #[test]
    fn the_request_carries_the_conversation_in_order() {
        let earlier_turn = ModelTurn {
            text: String::new(),
            tool_calls: Vec::new(),
            stop: StopReason::WantsTools,
            replay: json!([{ "type": "tool_use", "id": "toolu_1", "name": "check_plugin", "input": {} }]),
            tokens_in_context: None,
        };
        let result = ToolResult { call_id: "toolu_1".to_string(), output: ToolOutput::failure("blocked") };
        let history = [Entry::User("a tape delay".to_string()), Entry::Model(earlier_turn.clone()), Entry::ToolResults(vec![result])];

        let body = default_request(&Brief { instructions: "be careful", tools: &[], history: &history });

        assert_eq!(body["system"], "be careful");
        assert_eq!(body["messages"][0], json!({ "role": "user", "content": "a tape delay" }));
        assert_eq!(body["messages"][1], json!({ "role": "assistant", "content": earlier_turn.replay }));
        assert_eq!(
            body["messages"][2]["content"][0],
            json!({ "type": "tool_result", "tool_use_id": "toolu_1", "content": "blocked", "is_error": true })
        );
    }

    #[test]
    fn a_tool_result_with_pictures_sends_each_one_after_its_caption() {
        let sketch = Illustration { caption: "a dot".to_string(), png: b"png".to_vec() };
        let result = ToolResult { call_id: "toolu_2".to_string(), output: ToolOutput::success("drawn").illustrated_with(vec![sketch]) };

        let sent = wire_tool_result(&result);

        let expected = json!([
            { "type": "text", "text": "drawn" },
            { "type": "text", "text": "a dot" },
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "cG5n" } },
        ]);
        assert_eq!(sent["content"], expected);
    }

    #[test]
    fn the_request_asks_for_a_streamed_reply_from_the_default_model() {
        let tool = ToolDefinition { name: "check_plugin", description: "Runs the check", input_schema: json!({ "type": "object" }) };

        let body = default_request(&Brief { instructions: "", tools: &[tool], history: &[] });

        assert_eq!(body["model"], "claude-opus-5-5");
        assert_eq!(body["output_config"]["effort"], "high");
        assert_eq!(body["stream"], true);
        assert_eq!(body["fallbacks"], "default");
        assert_eq!(body["tools"][0]["eager_input_streaming"], true);
        assert_eq!(body["tools"][0]["name"], "check_plugin");
    }

    #[test]
    fn an_empty_model_turn_is_left_out_of_the_request() {
        let silent_turn = ModelTurn { text: String::new(), tool_calls: Vec::new(), stop: StopReason::Finished, replay: json!([]), tokens_in_context: None };
        let history = [Entry::User("one".to_string()), Entry::Model(silent_turn), Entry::User("two".to_string())];

        let body = default_request(&Brief { instructions: "", tools: &[], history: &history });

        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn an_api_key_and_an_account_sign_in_are_presented_differently() {
        let with_key = key_headers("sk-test", ClaudeModel::Opus);
        let with_sign_in = sign_in_headers("token-abc", ClaudeModel::Opus);

        assert!(with_key.contains(&("x-api-key", "sk-test".to_string())));
        assert!(with_sign_in.contains(&("authorization", "Bearer token-abc".to_string())));
        assert!(with_sign_in.contains(&("anthropic-beta", format!("{SIGN_IN_BETA},{FALLBACK_BETA}"))));
        assert!(!with_sign_in.iter().any(|(name, _)| *name == "x-api-key"));
    }

    #[test]
    fn the_chosen_model_and_reasoning_are_what_the_request_asks_for() {
        let body = request_body(&NOTHING_TO_SAY, choices(ClaudeModel::Fable, Effort::Max));

        assert_eq!(body["model"], "claude-fable-5-2");
        assert_eq!(body["output_config"]["effort"], "max");
        assert_eq!(body["fallbacks"], "default");
    }

    #[test]
    fn the_web_tools_are_offered_only_when_the_web_is_open() {
        assert_eq!(default_request(&NOTHING_TO_SAY)["tools"], json!([]));
        assert_eq!(tool_types_with_the_web_open(ClaudeModel::Opus), ["web_search_20260209", "web_fetch_20260209"]);
        assert_eq!(tool_types_with_the_web_open(ClaudeModel::Haiku), ["web_search_20250305"]);
    }

    #[test]
    fn a_web_lookup_is_kept_to_send_back_but_never_run_by_the_workbench() {
        let turn = turn_from(&[
            block_start(0, json!({ "type": "server_tool_use", "id": "srvtoolu_1", "name": "web_search", "input": {} })),
            block_delta(0, json!({ "type": "input_json_delta", "partial_json": "{\"query\": \"tape wow" })),
            block_delta(0, json!({ "type": "input_json_delta", "partial_json": " and flutter\"}" })),
            block_start(1, json!({ "type": "web_search_tool_result", "tool_use_id": "srvtoolu_1", "content": [] })),
            block_start(2, json!({ "type": "text", "text": "" })),
            block_delta(2, json!({ "type": "text_delta", "text": "Flutter is fast." })),
            block_delta(2, json!({ "type": "citations_delta", "citation": { "type": "web_search_result_location", "url": "https://example.com" } })),
            stopped_because("end_turn"),
        ])
        .unwrap();

        assert!(turn.tool_calls.is_empty());
        assert_eq!(turn.text, "Flutter is fast.");
        assert_eq!(turn.replay[0]["input"], json!({ "query": "tape wow and flutter" }));
        assert_eq!(turn.replay[1]["type"], "web_search_tool_result");
        assert_eq!(turn.replay[2]["citations"][0]["url"], "https://example.com");
    }

    #[test]
    fn a_paused_run_of_web_lookups_is_told_apart_from_a_finish() {
        assert_eq!(turn_from(&[stopped_because("pause_turn")]).unwrap().stop, StopReason::Paused);
    }

    #[test]
    fn a_model_without_server_side_fallback_is_not_asked_for_one() {
        let body = request_body(&NOTHING_TO_SAY, choices(ClaudeModel::Haiku, Effort::Low));

        assert_eq!(body["model"], "claude-haiku-5-5");
        assert!(body.get("fallbacks").is_none());
        assert_eq!(key_headers("sk-test", ClaudeModel::Haiku), [("x-api-key", "sk-test".to_string())]);
        assert!(sign_in_headers("token-abc", ClaudeModel::Haiku).contains(&("anthropic-beta", SIGN_IN_BETA.to_string())));
    }

    #[test]
    fn only_a_single_unbroken_word_is_accepted_as_a_sign_in_token() {
        assert_eq!(usable_token("token-abc\n"), Some("token-abc".to_string()));
        assert_eq!(usable_token(""), None);
        assert_eq!(usable_token("Buildfile: build.xml does not exist!"), None);
        assert_eq!(usable_token("{\n  \"access_token\": \"x\"\n}"), None);
    }

}
