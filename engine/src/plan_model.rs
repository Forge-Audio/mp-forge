//! A model reached with the musician's own ChatGPT plan, through OpenAI's Responses API.
//!
//! OpenAI lets a plan be used only this way: https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference
//! Nothing is stored with OpenAI between turns, so every request carries the whole conversation, and the
//! reply is always streamed. A turn only counts once OpenAI says it is complete.
//!
//! Such a model gets exactly the tools Claude gets and nothing more.

use std::io::{BufRead, BufReader};

use serde_json::{Value, json};

use crate::agent::{Brief, Entry, Illustration, Model, ModelError, ModelTurn, StopReason, ToolCall, ToolDefinition, ToolInput, ToolResult};
use crate::chat_model::{ChosenModel, PICTURE_NOT_SHOWN};
use crate::model_http::{self, HTTP_OK, is_worth_retrying, rejection};

const ASKED_AT: &str = "https://api.openai.com/v1/responses";
/// The plan takes tools grouped under one name, so MP Forge's tools go in a group of their own.
const TOOL_GROUP: &str = "mp_forge";
const ABOUT_THE_TOOL_GROUP: &str = "Everything MP Forge lets you do to the musician's plugin.";
/// Asked for so the model's own working can be handed back to it next turn, which nothing stored would allow.
const WORKING_KEPT_FOR_NEXT_TURN: &str = "reasoning.encrypted_content";
const HTTP_UNAUTHORIZED: u16 = 401;
const HTTP_TOO_MANY: u16 = 429;
const PLAN_LIMIT_REACHED: &str = "You have reached a usage limit on your ChatGPT plan, or the limit you set for MP Forge. See and change your limits at chatgpt.com/settings/usage.";
const PLAN_CANNOT_BE_USED: &str = "Your ChatGPT plan cannot be used here. OpenAI does not allow it for this account or workspace. Pick another service for now.";
const LIMIT_CODE: &str = "subscription_sharing_usage_limit_exceeded";
const NOT_ELIGIBLE_CODE: &str = "subscription_sharing_user_not_eligible";

/// The access token for the musician's plan. Whoever keeps the account renews it when it is near its end.
pub trait PlanAccess {
    /// A token that should work now.
    fn token(&mut self) -> Result<String, ModelError>;
    /// A new token, after OpenAI refused the last one.
    fn token_after_refusal(&mut self) -> Result<String, ModelError>;
}

pub struct PlanModel<'a> {
    http: ureq::Agent,
    address: String,
    access: Box<dyn PlanAccess + 'a>,
    chosen: ChosenModel,
}

impl<'a> PlanModel<'a> {
    pub fn new(access: impl PlanAccess + 'a, chosen: ChosenModel) -> Self {
        Self::asked_at(ASKED_AT, access, chosen)
    }

    /// The same model asked somewhere else, for tests that stand in for OpenAI.
    pub fn asked_at(address: &str, access: impl PlanAccess + 'a, chosen: ChosenModel) -> Self {
        Self { http: model_http::client(), address: address.to_string(), access: Box::new(access), chosen }
    }

    /// A token OpenAI refuses is renewed once. If the new one is refused too, that is the answer.
    fn open_event_stream(&mut self, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        let token = self.access.token()?;
        match self.asked_with(&token, body) {
            Err(ModelError::Rejected { status: HTTP_UNAUTHORIZED, .. }) => {
                let token = self.access.token_after_refusal()?;
                self.asked_with(&token, body)
            }
            outcome => outcome,
        }
    }

    /// A plan that has reached its limit is not asked again: OpenAI wants requests to stop until the musician acts.
    fn asked_with(&self, token: &str, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        model_http::with_retries_while(|| self.send(token, body), |refusal| is_worth_retrying(refusal) && !is_final_word_on_the_plan(refusal))
    }

    fn send(&self, token: &str, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        let request = self.http.post(&self.address).header("content-type", "application/json").header("authorization", format!("Bearer {token}"));
        let response = request.send(body).map_err(|error| ModelError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let mut reply = response.into_body();
        if status != HTTP_OK {
            return Err(refusal_of(status, &reply.read_to_string().unwrap_or_default()));
        }
        Ok(Box::new(BufReader::new(reply.into_reader())))
    }
}

impl Model for PlanModel<'_> {
    fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError> {
        let body = request_body(brief, &self.chosen).to_string();
        assemble_turn(self.open_event_stream(&body)?, brief.tools)
    }
}

// ---------------------------------------------------------------------------
// Why OpenAI said no
// ---------------------------------------------------------------------------

/// A refusal before any reply starts. What OpenAI says about the plan itself is put in plain words.
fn refusal_of(status: u16, reply: &str) -> ModelError {
    let said: Value = serde_json::from_str(reply).unwrap_or(Value::Null);
    about_the_plan(said["error"]["code"].as_str().unwrap_or_default()).unwrap_or_else(|| rejection(status, reply))
}

fn about_the_plan(code: &str) -> Option<ModelError> {
    match code {
        LIMIT_CODE => Some(ModelError::Rejected { status: HTTP_TOO_MANY, message: PLAN_LIMIT_REACHED.to_string() }),
        NOT_ELIGIBLE_CODE => Some(ModelError::Rejected { status: 403, message: PLAN_CANNOT_BE_USED.to_string() }),
        _ => None,
    }
}

fn is_final_word_on_the_plan(refusal: &ModelError) -> bool {
    matches!(refusal, ModelError::Rejected { message, .. } if message == PLAN_LIMIT_REACHED || message == PLAN_CANNOT_BE_USED)
}

// ---------------------------------------------------------------------------
// Building the request
// ---------------------------------------------------------------------------

fn request_body(brief: &Brief, chosen: &ChosenModel) -> Value {
    let conversation: Vec<Value> = brief.history.iter().flat_map(|entry| wire_items(entry, chosen.sees_pictures)).collect();
    let mut body = json!({
        "model": chosen.id,
        "instructions": brief.instructions,
        "input": conversation,
        "store": false,
        "stream": true,
        "include": [WORKING_KEPT_FOR_NEXT_TURN],
    });
    if !brief.tools.is_empty() {
        body["tools"] = json!([{ "type": "namespace", "name": TOOL_GROUP, "description": ABOUT_THE_TOOL_GROUP, "tools": brief.tools.iter().map(wire_tool).collect::<Vec<Value>>() }]);
    }
    body
}

fn wire_tool(tool: &ToolDefinition) -> Value {
    json!({ "type": "function", "name": tool.name, "description": tool.description, "parameters": tool.input_schema })
}

fn wire_items(entry: &Entry, sees_pictures: bool) -> Vec<Value> {
    match entry {
        Entry::User(text) => vec![json!({ "role": "user", "content": text })],
        Entry::Model(turn) => turn.replay.as_array().cloned().unwrap_or_default(),
        Entry::ToolResults(results) => results.iter().map(|result| wire_tool_result(result, sees_pictures)).collect(),
    }
}

/// A tool's answer can carry its pictures with it here, each after its caption.
fn wire_tool_result(result: &ToolResult, sees_pictures: bool) -> Value {
    let pictures = &result.output.illustrations;
    let output = match (pictures.is_empty(), sees_pictures) {
        (true, _) => json!(result.output.content),
        (false, false) => json!(format!("{}\n\n{PICTURE_NOT_SHOWN}", result.output.content)),
        (false, true) => {
            let words = json!({ "type": "input_text", "text": result.output.content });
            Value::Array(std::iter::once(words).chain(pictures.iter().flat_map(wire_illustration)).collect())
        }
    };
    json!({ "type": "function_call_output", "call_id": result.call_id, "output": output })
}

fn wire_illustration(illustration: &Illustration) -> [Value; 2] {
    let address = format!("data:image/png;base64,{}", illustration.png_as_base64());
    [json!({ "type": "input_text", "text": illustration.caption }), json!({ "type": "input_image", "image_url": address })]
}

// ---------------------------------------------------------------------------
// Reading the streamed reply
// ---------------------------------------------------------------------------

fn assemble_turn(stream: impl BufRead, tools: &[ToolDefinition]) -> Result<ModelTurn, ModelError> {
    let mut assembler = TurnAssembler::default();
    for line in stream.lines() {
        let line = line.map_err(|error| ModelError::Unreachable(format!("the reply broke off: {error}")))?;
        if let Some(event) = event_in(&line) {
            assembler.take(&event);
        }
    }
    assembler.finish(tools)
}

/// Each event arrives as a `data: {...}` line that names its own kind. Every other line is framing.
fn event_in(line: &str) -> Option<Value> {
    serde_json::from_str(line.strip_prefix("data:")?.trim()).ok()
}

/// How a reply ended, when OpenAI said.
enum Ending {
    Complete,
    /// It stopped short, for the reason given.
    CutShort(String),
    Failed(ModelError),
}

#[derive(Default)]
struct TurnAssembler {
    /// Everything the model produced this turn, each piece as OpenAI finished it.
    items: Vec<Value>,
    ending: Option<Ending>,
    tokens_in_context: Option<u64>,
}

impl TurnAssembler {
    fn take(&mut self, event: &Value) {
        match event["type"].as_str().unwrap_or_default() {
            "response.output_item.done" => self.items.push(event["item"].clone()),
            "response.completed" => self.end_with(Ending::Complete, &event["response"]),
            "response.incomplete" => {
                let reason = event["response"]["incomplete_details"]["reason"].as_str().unwrap_or_default();
                self.end_with(Ending::CutShort(reason.to_string()), &event["response"]);
            }
            "response.failed" => self.ending = Some(Ending::Failed(failure_in(&event["response"]["error"]))),
            "error" => self.ending = Some(Ending::Failed(failure_in(if event["error"].is_object() { &event["error"] } else { event }))),
            _ => {}
        }
    }

    /// The count comes with the end of the reply: everything the model read for this turn and the turn it wrote.
    fn end_with(&mut self, ending: Ending, response: &Value) {
        self.ending = Some(ending);
        self.tokens_in_context = response["usage"]["total_tokens"].as_u64();
    }

    fn finish(self, tools: &[ToolDefinition]) -> Result<ModelTurn, ModelError> {
        let cut_short_because = match self.ending {
            Some(Ending::Failed(failure)) => return Err(failure),
            None => return Err(ModelError::Garbled("it ended without saying it was complete".to_string())),
            Some(Ending::CutShort(reason)) => Some(reason),
            Some(Ending::Complete) => None,
        };
        let tool_calls: Vec<ToolCall> = self.items.iter().filter(|item| item["type"] == "function_call").enumerate().map(|(place, item)| tool_call_in(item, place, tools)).collect();
        let stop = stop_reason_from(cut_short_because.as_deref(), was_refused(&self.items), !tool_calls.is_empty());
        Ok(ModelTurn { text: words_in(&self.items), tool_calls, stop, replay: replay_of(self.items), tokens_in_context: self.tokens_in_context })
    }
}

fn failure_in(error: &Value) -> ModelError {
    let code = error["code"].as_str().unwrap_or_default();
    let message = error["message"].as_str().unwrap_or("unknown error");
    about_the_plan(code).unwrap_or_else(|| ModelError::Unreachable(format!("the reply broke off: {message}")))
}

/// The parts of the model's messages that are of one kind, such as the words it wrote.
fn message_parts<'a>(items: &'a [Value], kind: &'a str) -> impl Iterator<Item = &'a Value> {
    let parts_of = |item: &'a Value| item["content"].as_array().map(Vec::as_slice).unwrap_or_default().iter();
    items.iter().filter(|item| item["type"] == "message").flat_map(parts_of).filter(move |part| part["type"] == kind)
}

fn words_in(items: &[Value]) -> String {
    let words: Vec<&str> = message_parts(items, "output_text").filter_map(|part| part["text"].as_str()).collect();
    words.join("\n").trim().to_string()
}

fn was_refused(items: &[Value]) -> bool {
    message_parts(items, "refusal").next().is_some()
}

/// A tool is asked for by its own name. Should the group's name come in front of it, the tool is still found.
fn tool_call_in(item: &Value, place: usize, tools: &[ToolDefinition]) -> ToolCall {
    let asked_for = item["name"].as_str().unwrap_or_default();
    let known = tools.iter().map(|tool| tool.name).find(|name| asked_for == *name || asked_for.strip_suffix(name).is_some_and(|before| before.ends_with(['.', '/', ':', '_'])));
    let id = item["call_id"].as_str().map_or_else(|| format!("call-{place}"), str::to_string);
    ToolCall { id, name: known.unwrap_or(asked_for).to_string(), input: ToolInput::read_from(item["arguments"].as_str().unwrap_or("")) }
}

/// The turn as it must be sent back later, since OpenAI keeps none of it. Each piece goes back without the
/// name OpenAI gave it, which it could not look up. The model's own working goes back only where it came
/// sealed, and tool input that could not be read goes back as an empty object.
fn replay_of(items: Vec<Value>) -> Value {
    Value::Array(items.into_iter().filter(can_be_sent_back).map(as_sent_back).collect())
}

fn can_be_sent_back(item: &Value) -> bool {
    item["type"] != "reasoning" || item["encrypted_content"].is_string()
}

fn as_sent_back(mut item: Value) -> Value {
    if let Some(fields) = item.as_object_mut() {
        fields.remove("id");
        fields.remove("status");
    }
    let has_broken_input = item["type"] == "function_call" && matches!(ToolInput::read_from(item["arguments"].as_str().unwrap_or("")), ToolInput::Unparseable(_));
    if has_broken_input {
        item["arguments"] = json!("{}");
    }
    item
}

fn stop_reason_from(cut_short_because: Option<&str>, refused: bool, asked_for_tools: bool) -> StopReason {
    match cut_short_because {
        Some("content_filter") => StopReason::Refused,
        Some(_) => StopReason::RanOutOfRoom,
        None if refused => StopReason::Refused,
        None if asked_for_tools => StopReason::WantsTools,
        None => StopReason::Finished,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;
    use crate::agent::ToolOutput;

    const TOOLS: [&str; 2] = ["read_plugin_file", "check_plugin"];

    fn tools() -> Vec<ToolDefinition> {
        TOOLS.iter().map(|name| ToolDefinition { name, description: "Does one thing.", input_schema: json!({ "type": "object" }) }).collect()
    }

    fn turn_from(lines: &[&str]) -> Result<ModelTurn, ModelError> {
        assemble_turn(lines.join("\n").as_bytes(), &tools())
    }

    fn chosen(sees_pictures: bool) -> ChosenModel {
        ChosenModel { id: "made-up-large".to_string(), sees_pictures, effort: None, looks_things_up: false }
    }

    const WORDS_ITEM: &str = r#"data: {"type":"response.output_item.done","item":{"id":"msg_1","type":"message","role":"assistant","status":"completed","content":[{"type":"output_text","text":"Here is your plugin."}]}}"#;
    const COMPLETED: &str = r#"data: {"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":1200,"output_tokens":30,"total_tokens":1230}}}"#;

    #[test]
    fn a_reply_counts_only_once_openai_says_it_is_complete() {
        let complete = turn_from(&["event: response.output_text.delta", r#"data: {"type":"response.output_text.delta","delta":"Here"}"#, WORDS_ITEM, COMPLETED]).unwrap();
        let broke_off = turn_from(&[WORDS_ITEM]);

        assert_eq!((complete.text.as_str(), complete.stop, complete.tokens_in_context), ("Here is your plugin.", StopReason::Finished, Some(1230)));
        assert!(matches!(broke_off, Err(ModelError::Garbled(_))));
    }

    #[test]
    fn a_tool_request_is_read_whether_or_not_the_group_is_named_in_front_of_it() {
        let turn = turn_from(&[
            r#"data: {"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"read_plugin_file","namespace":"mp_forge","arguments":"{\"path\":\"ui.json\"}","status":"completed"}}"#,
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","call_id":"call_2","name":"mp_forge.check_plugin","arguments":""}}"#,
            COMPLETED,
        ])
        .unwrap();

        assert_eq!(turn.stop, StopReason::WantsTools);
        assert_eq!(turn.tool_calls[0], ToolCall { id: "call_1".to_string(), name: "read_plugin_file".to_string(), input: ToolInput::Valid(json!({ "path": "ui.json" })) });
        assert_eq!((turn.tool_calls[1].id.as_str(), turn.tool_calls[1].name.as_str()), ("call_2", "check_plugin"));
    }

    #[test]
    fn the_turn_goes_back_without_openais_names_and_with_its_working_only_where_sealed() {
        let turn = turn_from(&[
            r#"data: {"type":"response.output_item.done","item":{"id":"rs_1","type":"reasoning","summary":[],"encrypted_content":"sealed"}}"#,
            r#"data: {"type":"response.output_item.done","item":{"id":"rs_2","type":"reasoning","summary":[]}}"#,
            r#"data: {"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"check_plugin","arguments":"{}","status":"completed"}}"#,
            COMPLETED,
        ])
        .unwrap();

        assert_eq!(turn.replay, json!([{ "type": "reasoning", "summary": [], "encrypted_content": "sealed" }, { "type": "function_call", "call_id": "call_1", "name": "check_plugin", "arguments": "{}" }]));
    }

    #[test]
    fn broken_tool_input_never_reaches_a_tool_and_is_sent_back_as_an_empty_object() {
        let turn = turn_from(&[
            r#"data: {"type":"response.output_item.done","item":{"type":"function_call","call_id":"call_1","name":"read_plugin_file","arguments":"{\"path\": \"src/","status":"incomplete"}}"#,
            r#"data: {"type":"response.incomplete","response":{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}}"#,
        ])
        .unwrap();

        assert_eq!(turn.stop, StopReason::RanOutOfRoom);
        assert!(matches!(turn.tool_calls[0].input, ToolInput::Unparseable(_)));
        assert_eq!(turn.replay[0]["arguments"], "{}");
    }

    #[test]
    fn a_refusal_is_told_apart_from_a_normal_finish() {
        let refused = turn_from(&[r#"data: {"type":"response.output_item.done","item":{"type":"message","content":[{"type":"refusal","refusal":"No."}]}}"#, COMPLETED]).unwrap();
        let filtered = turn_from(&[r#"data: {"type":"response.incomplete","response":{"incomplete_details":{"reason":"content_filter"}}}"#]).unwrap();

        assert_eq!((refused.stop, filtered.stop), (StopReason::Refused, StopReason::Refused));
    }

    #[test]
    fn a_plan_that_has_reached_its_limit_is_said_in_plain_words_and_not_asked_again() {
        let mid_reply = turn_from(&[WORDS_ITEM, r#"data: {"type":"response.failed","response":{"error":{"code":"subscription_sharing_usage_limit_exceeded","message":"limit"}}}"#]);
        let before_reply = refusal_of(429, r#"{"error":{"code":"subscription_sharing_usage_limit_exceeded","message":"limit"}}"#);
        let busy = refusal_of(429, r#"{"error":{"code":"rate_limit_exceeded","message":"slow down"}}"#);
        let other_failure = turn_from(&[r#"data: {"type":"error","code":"server_error","message":"it fell over"}"#]);

        assert!(matches!(&mid_reply, Err(ModelError::Rejected { message, .. }) if message == PLAN_LIMIT_REACHED));
        assert!(is_final_word_on_the_plan(&before_reply) && !is_final_word_on_the_plan(&busy));
        assert!(matches!(busy, ModelError::Rejected { status: 429, message } if message == "slow down"));
        assert!(matches!(other_failure, Err(ModelError::Unreachable(cause)) if cause.contains("it fell over")));
    }

    #[test]
    fn the_request_is_the_whole_conversation_with_nothing_stored_and_the_tools_in_one_group() {
        let said = turn_from(&[r#"data: {"type":"response.output_item.done","item":{"id":"fc_1","type":"function_call","call_id":"call_1","name":"check_plugin","arguments":"{}"}}"#, COMPLETED]).unwrap();
        let answered = Entry::ToolResults(vec![ToolResult { call_id: "call_1".to_string(), output: ToolOutput::success("All checks passed.") }]);
        let history = [Entry::User("Make an echo.".to_string()), Entry::Model(said), answered];

        let body = request_body(&Brief { instructions: "Be careful.", tools: &tools(), history: &history }, &chosen(true));

        assert_eq!((&body["model"], &body["instructions"], &body["store"], &body["stream"]), (&json!("made-up-large"), &json!("Be careful."), &json!(false), &json!(true)));
        assert_eq!(body["include"], json!(["reasoning.encrypted_content"]));
        assert_eq!((&body["tools"][0]["type"], &body["tools"][0]["name"]), (&json!("namespace"), &json!("mp_forge")));
        assert_eq!(body["tools"][0]["tools"][1], json!({ "type": "function", "name": "check_plugin", "description": "Does one thing.", "parameters": { "type": "object" } }));
        assert_eq!(body["input"], json!([
            { "role": "user", "content": "Make an echo." },
            { "type": "function_call", "call_id": "call_1", "name": "check_plugin", "arguments": "{}" },
            { "type": "function_call_output", "call_id": "call_1", "output": "All checks passed." },
        ]));
        assert!(body.get("temperature").is_none() && body.get("max_output_tokens").is_none() && body.get("previous_response_id").is_none());
    }

    #[test]
    fn a_request_with_no_tools_sends_no_empty_group() {
        let body = request_body(&Brief { instructions: "Ask three questions.", tools: &[], history: &[] }, &chosen(true));

        assert!(body.get("tools").is_none());
    }

    #[test]
    fn a_tools_pictures_go_with_its_answer_for_a_model_that_can_look_and_are_named_for_one_that_cannot() {
        let picture = Illustration { caption: "Page MAIN".to_string(), png: vec![1, 2, 3] };
        let result = ToolResult { call_id: "call_1".to_string(), output: ToolOutput::success("Styled.").illustrated_with(vec![picture]) };

        let seen = wire_tool_result(&result, true);
        let unseen = wire_tool_result(&result, false);

        assert_eq!(seen["output"], json!([{ "type": "input_text", "text": "Styled." }, { "type": "input_text", "text": "Page MAIN" }, { "type": "input_image", "image_url": "data:image/png;base64,AQID" }]));
        assert!(unseen["output"].as_str().unwrap().ends_with(PICTURE_NOT_SHOWN));
    }

    /// Access that hands out one token, then another once the first has been refused.
    struct TwoTokens {
        renewals: usize,
    }

    impl PlanAccess for &mut TwoTokens {
        fn token(&mut self) -> Result<String, ModelError> {
            Ok("old-token".to_string())
        }

        fn token_after_refusal(&mut self) -> Result<String, ModelError> {
            self.renewals += 1;
            Ok("new-token".to_string())
        }
    }

    /// Stands in for OpenAI: refuses the old token, and answers the new one with a complete reply.
    fn openai_that_refuses_the_old_token() -> String {
        let service = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = format!("http://127.0.0.1:{}/responses", service.local_addr().unwrap().port());
        thread::spawn(move || {
            for mut asker in service.incoming().flatten() {
                let mut heard = [0u8; 8192];
                let read = asker.read(&mut heard).unwrap_or(0);
                let has_the_new_token = String::from_utf8_lossy(&heard[..read]).to_lowercase().contains("bearer new-token");
                let reply = format!("{WORDS_ITEM}\n\n{COMPLETED}\n\n");
                let _ = if has_the_new_token {
                    write!(asker, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len())
                } else {
                    write!(asker, "HTTP/1.1 401 Unauthorized\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{{\"detail\":\"expired\"}}")
                };
            }
        });
        address
    }

    #[test]
    fn a_token_openai_refuses_is_renewed_once_and_the_turn_goes_on() {
        let mut access = TwoTokens { renewals: 0 };
        let address = openai_that_refuses_the_old_token();

        let turn = PlanModel::asked_at(&address, &mut access, chosen(true)).next_turn(&Brief { instructions: "Be careful.", tools: &tools(), history: &[Entry::User("Hello.".to_string())] });

        assert_eq!(turn.unwrap().text, "Here is your plugin.");
        assert_eq!(access.renewals, 1);
    }
}
