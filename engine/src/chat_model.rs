//! Any model that speaks the chat protocol OpenAI started and most services now share: OpenAI, Google, xAI,
//! Mistral, DeepSeek, everything behind OpenRouter, and the models Ollama runs on the musician's own computer.
//!
//! Such a model gets exactly the tools Claude gets and nothing more. The reply is streamed, because a turn that
//! writes a whole source file is long enough to time out otherwise.

use std::io::{BufRead, BufReader};

use serde_json::{Map, Value, json};

use crate::agent::{Brief, Entry, Illustration, Model, ModelError, ModelTurn, StopReason, ToolCall, ToolDefinition, ToolInput, ToolResult};
use crate::ai_settings::Effort;
use crate::model_http::{self, HTTP_OK, rejection};
use crate::providers::Provider;

const PICTURES_FOLLOW: &str = "The pictures that came with the tool results above:";
pub(crate) const PICTURE_NOT_SHOWN: &str = "(A picture came with this, but this model cannot look at pictures.)";

/// Which model to ask, and what is known of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenModel {
    pub id: String,
    pub sees_pictures: bool,
    /// How hard to think, when the musician chose and the model can be told.
    pub effort: Option<Effort>,
    /// Whether it may look things up on the web, which only some services sell.
    pub looks_things_up: bool,
}

pub struct ChatModel {
    http: ureq::Agent,
    address: String,
    /// Held in memory for as long as this work lasts. It is never written to a log or a file.
    /// Models on the musician's own computer have none.
    key: Option<String>,
    chosen: ChosenModel,
    asks_for_token_counts: bool,
}

impl ChatModel {
    /// A service is only ever sent the settings it has, whatever was chosen.
    pub fn new(provider: &Provider, key: Option<String>, chosen: ChosenModel) -> Self {
        let effort = chosen.effort.filter(|_| provider.takes_effort());
        let looks_things_up = chosen.looks_things_up && provider.sells_web_lookups();
        let chosen = ChosenModel { effort, looks_things_up, ..chosen };
        Self { http: model_http::client(), address: provider.chat_address(), key, chosen, asks_for_token_counts: provider.counts_tokens_only_when_asked }
    }

    fn open_event_stream(&self, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        model_http::with_retries(|| self.send(body))
    }

    fn send(&self, body: &str) -> Result<Box<dyn BufRead>, ModelError> {
        let mut request = self.http.post(&self.address).header("content-type", "application/json");
        if let Some(key) = &self.key {
            request = request.header("authorization", format!("Bearer {key}"));
        }
        let response = request.send(body).map_err(|error| ModelError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let mut reply = response.into_body();
        if status != HTTP_OK {
            return Err(rejection(status, &reply.read_to_string().unwrap_or_default()));
        }
        Ok(Box::new(BufReader::new(reply.into_reader())))
    }
}

impl Model for ChatModel {
    fn next_turn(&mut self, brief: &Brief) -> Result<ModelTurn, ModelError> {
        let mut body = request_body(brief, &self.chosen);
        if self.asks_for_token_counts {
            body["stream_options"] = json!({ "include_usage": true });
        }
        let body = body.to_string();
        assemble_turn(self.open_event_stream(&body)?)
    }
}

// ---------------------------------------------------------------------------
// Building the request
// ---------------------------------------------------------------------------

fn request_body(brief: &Brief, chosen: &ChosenModel) -> Value {
    let instructions = json!({ "role": "system", "content": brief.instructions });
    let conversation = brief.history.iter().flat_map(|entry| wire_messages(entry, chosen.sees_pictures));
    let mut body = json!({
        "model": chosen.id,
        "stream": true,
        "messages": std::iter::once(instructions).chain(conversation).collect::<Vec<Value>>(),
        "tools": tools_given(brief, chosen),
    });
    if let Some(effort) = chosen.effort {
        body["reasoning"] = json!({ "effort": effort.word() });
    }
    body
}

fn tools_given(brief: &Brief, chosen: &ChosenModel) -> Vec<Value> {
    let web_search = chosen.looks_things_up.then(web_search_tool);
    brief.tools.iter().map(wire_tool).chain(web_search).collect()
}

/// OpenRouter's own tool for searching the web. OpenRouter runs each search and hands the model what it found,
/// so no request for it ever comes back here to be run.
fn web_search_tool() -> Value {
    json!({ "type": "openrouter:web_search" })
}

fn wire_tool(tool: &ToolDefinition) -> Value {
    json!({ "type": "function", "function": { "name": tool.name, "description": tool.description, "parameters": tool.input_schema } })
}

fn wire_messages(entry: &Entry, sees_pictures: bool) -> Vec<Value> {
    match entry {
        Entry::User(text) => vec![json!({ "role": "user", "content": text })],
        Entry::Model(turn) if turn.replay.is_object() => vec![turn.replay.clone()],
        Entry::Model(_) => Vec::new(),
        Entry::ToolResults(results) => wire_tool_results(results, sees_pictures),
    }
}

/// This protocol has no room for a picture inside a tool's answer. So the answers go first, as words, and the
/// pictures follow in a message of their own, each after its caption.
fn wire_tool_results(results: &[ToolResult], sees_pictures: bool) -> Vec<Value> {
    let mut messages: Vec<Value> = results.iter().map(|result| wire_tool_result(result, sees_pictures)).collect();
    let pictures: Vec<&Illustration> = results.iter().flat_map(|result| &result.output.illustrations).collect();
    if sees_pictures && !pictures.is_empty() {
        let introduction = json!({ "type": "text", "text": PICTURES_FOLLOW });
        let shown = std::iter::once(introduction).chain(pictures.into_iter().flat_map(wire_illustration));
        messages.push(json!({ "role": "user", "content": shown.collect::<Vec<Value>>() }));
    }
    messages
}

fn wire_tool_result(result: &ToolResult, sees_pictures: bool) -> Value {
    let has_unseen_pictures = !sees_pictures && !result.output.illustrations.is_empty();
    let words = if has_unseen_pictures { format!("{}\n\n{PICTURE_NOT_SHOWN}", result.output.content) } else { result.output.content.clone() };
    json!({ "role": "tool", "tool_call_id": result.call_id, "content": words })
}

fn wire_illustration(illustration: &Illustration) -> [Value; 2] {
    let address = format!("data:image/png;base64,{}", illustration.png_as_base64());
    [json!({ "type": "text", "text": illustration.caption }), json!({ "type": "image_url", "image_url": { "url": address } })]
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

/// Each piece arrives as a `data: {...}` line. Every other line, the closing `[DONE]` among them, is framing.
fn event_in(line: &str) -> Option<Value> {
    serde_json::from_str(line.strip_prefix("data:")?.trim()).ok()
}

#[derive(Default)]
struct TurnAssembler {
    text: String,
    tool_requests: Vec<Value>,
    finish_reason: Option<String>,
    failure: Option<String>,
    tokens_in_context: Option<u64>,
}

impl TurnAssembler {
    fn take(&mut self, event: &Value) {
        if !event["error"].is_null() {
            self.failure = Some(event["error"]["message"].as_str().unwrap_or("unknown error").to_string());
        }
        self.note_tokens(&event["usage"]);
        let choice = &event["choices"][0];
        self.text.push_str(choice["delta"]["content"].as_str().unwrap_or(""));
        for piece in choice["delta"]["tool_calls"].as_array().map(Vec::as_slice).unwrap_or_default() {
            self.extend_tool_request(piece);
        }
        if let Some(finish_reason) = choice["finish_reason"].as_str() {
            self.finish_reason = Some(finish_reason.to_string());
        }
    }

    /// The count comes with the last piece of the reply: everything the model read for this turn and the turn it wrote.
    fn note_tokens(&mut self, usage: &Value) {
        let read_and_written = || Some(usage["prompt_tokens"].as_u64()? + usage["completion_tokens"].as_u64().unwrap_or(0));
        if let Some(tokens) = usage["total_tokens"].as_u64().or_else(read_and_written) {
            self.tokens_in_context = Some(tokens);
        }
    }

    /// A tool request arrives in pieces that say which request they belong to by its place in the turn.
    fn extend_tool_request(&mut self, piece: &Value) {
        let place = piece["index"].as_u64().map_or_else(|| self.place_of_unnumbered(piece), |index| index as usize);
        while self.tool_requests.len() <= place {
            self.tool_requests.push(json!({ "type": "function", "function": { "name": "", "arguments": "" } }));
        }
        merge_piece(&mut self.tool_requests[place], piece);
    }

    /// A piece that carries no number belongs to the newest request, unless it brings an id of its own.
    fn place_of_unnumbered(&self, piece: &Value) -> usize {
        let starts_another = piece["id"].is_string() && self.tool_requests.last().is_some_and(|newest| newest["id"] != piece["id"]);
        if starts_another { self.tool_requests.len() } else { self.tool_requests.len().saturating_sub(1) }
    }

    fn finish(self) -> Result<ModelTurn, ModelError> {
        if let Some(failure) = self.failure {
            return Err(ModelError::Unreachable(format!("the reply broke off: {failure}")));
        }
        let tool_calls: Vec<ToolCall> = self.tool_requests.iter().enumerate().map(|(place, request)| tool_call_in(request, place)).collect();
        let stop = stop_reason_from(self.finish_reason.as_deref(), !tool_calls.is_empty())?;
        let text = self.text.trim().to_string();
        let replay = replay_of(&text, self.tool_requests, &tool_calls);
        Ok(ModelTurn { text, tool_calls, stop, replay, tokens_in_context: self.tokens_in_context })
    }
}

/// The name and the input grow as their pieces arrive. Anything else a service sends with a request, such as
/// its id, is kept as first given, because some services want it handed back unchanged.
fn merge_piece(request: &mut Value, piece: &Value) {
    let Some(fields) = piece.as_object() else { return };
    for (field, value) in fields.iter().filter(|(field, value)| field.as_str() != "index" && !value.is_null()) {
        if field == "function" {
            append_text(&mut request["function"], "name", &value["name"]);
            append_text(&mut request["function"], "arguments", &value["arguments"]);
        } else if request[field].is_null() {
            request[field] = value.clone();
        }
    }
}

fn append_text(holder: &mut Value, field: &str, addition: &Value) {
    let extended = format!("{}{}", holder[field].as_str().unwrap_or(""), addition.as_str().unwrap_or(""));
    holder[field] = Value::String(extended);
}

fn tool_call_in(request: &Value, place: usize) -> ToolCall {
    let text_of = |value: &Value| value.as_str().unwrap_or_default().to_string();
    let id = request["id"].as_str().map_or_else(|| format!("call-{place}"), str::to_string);
    ToolCall { id, name: text_of(&request["function"]["name"]), input: ToolInput::read_from(request["function"]["arguments"].as_str().unwrap_or("")) }
}

/// The turn as it must be sent back later. Input that could not be read is sent back as an empty object,
/// so the service is never handed its own broken words again.
fn replay_of(text: &str, mut tool_requests: Vec<Value>, tool_calls: &[ToolCall]) -> Value {
    for (request, call) in tool_requests.iter_mut().zip(tool_calls) {
        request["id"] = json!(call.id);
        if matches!(call.input, ToolInput::Unparseable(_)) {
            request["function"]["arguments"] = json!("{}");
        }
    }
    let mut message = Map::new();
    message.insert("role".to_string(), json!("assistant"));
    message.insert("content".to_string(), if text.is_empty() { Value::Null } else { json!(text) });
    if !tool_requests.is_empty() {
        message.insert("tool_calls".to_string(), Value::Array(tool_requests));
    }
    Value::Object(message)
}

/// Services do not agree on the word for "it asked for tools", so the requests themselves decide that.
fn stop_reason_from(stated: Option<&str>, asked_for_tools: bool) -> Result<StopReason, ModelError> {
    match stated {
        Some("length") => Ok(StopReason::RanOutOfRoom),
        Some("content_filter") => Ok(StopReason::Refused),
        Some(_) if asked_for_tools => Ok(StopReason::WantsTools),
        Some(_) => Ok(StopReason::Finished),
        None => Err(ModelError::Garbled("it ended without saying why".to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::ToolOutput;

    fn turn_from(lines: &[&str]) -> Result<ModelTurn, ModelError> {
        assemble_turn(lines.join("\n").as_bytes())
    }

    fn chosen(sees_pictures: bool) -> ChosenModel {
        ChosenModel { id: "acme/builder".to_string(), sees_pictures, effort: None, looks_things_up: false }
    }

    #[test]
    fn the_web_is_searched_only_when_chosen_and_only_through_a_service_that_sells_it() {
        let check = ToolDefinition { name: "check_plugin", description: "Runs the safety check.", input_schema: json!({ "type": "object" }) };
        let tools = [check];
        let brief = Brief { instructions: "Be careful.", tools: &tools, history: &[] };
        let looking_things_up = ChosenModel { looks_things_up: true, ..chosen(true) };
        let through = |provider: &str| ChatModel::new(crate::providers::provider_called(provider).unwrap(), None, looking_things_up.clone()).chosen;
        let kinds_of_tool = |chosen: &ChosenModel| request_body(&brief, chosen)["tools"].as_array().unwrap().iter().map(|tool| tool["type"].clone()).collect::<Vec<Value>>();

        assert_eq!(kinds_of_tool(&through("openrouter")), ["function", "openrouter:web_search"]);
        assert_eq!(kinds_of_tool(&through("openai")), ["function"]);
        assert_eq!(kinds_of_tool(&chosen(true)), ["function"]);
    }

    #[test]
    fn the_effort_chosen_goes_with_the_request_and_only_to_a_service_that_takes_it() {
        let brief = Brief { instructions: "Be careful.", tools: &[], history: &[] };
        let thinking_hard = ChosenModel { effort: Some(Effort::High), ..chosen(true) };
        let through = |provider: &str| ChatModel::new(crate::providers::provider_called(provider).unwrap(), None, thinking_hard.clone()).chosen;

        assert_eq!(request_body(&brief, &through("openrouter"))["reasoning"], json!({ "effort": "high" }));
        assert!(request_body(&brief, &through("openai")).get("reasoning").is_none());
        assert!(request_body(&brief, &chosen(true)).get("reasoning").is_none());
    }

    fn tool_result_with_a_picture() -> Entry {
        let picture = Illustration { caption: "Page MAIN".to_string(), png: vec![1, 2, 3] };
        Entry::ToolResults(vec![ToolResult { call_id: "call_1".to_string(), output: ToolOutput::success("Styled.").illustrated_with(vec![picture]) }])
    }

    #[test]
    fn words_arriving_in_pieces_are_joined() {
        let turn = turn_from(&[
            r#"data: {"choices":[{"delta":{"role":"assistant","content":"Here is "}}]}"#,
            ": keep-alive",
            r#"data: {"choices":[{"delta":{"content":"your plugin."},"finish_reason":"stop"}]}"#,
            "data: [DONE]",
        ])
        .unwrap();

        assert_eq!(turn.text, "Here is your plugin.");
        assert_eq!(turn.stop, StopReason::Finished);
        assert_eq!(turn.replay, json!({ "role": "assistant", "content": "Here is your plugin." }));
    }

    #[test]
    fn the_tokens_the_conversation_holds_come_with_the_last_piece_when_the_service_says() {
        let counted = turn_from(&[
            r#"data: {"choices":[{"delta":{"content":"Done."},"finish_reason":"stop"}]}"#,
            r#"data: {"choices":[],"usage":{"prompt_tokens":1200,"completion_tokens":30,"total_tokens":1230}}"#,
        ]);
        let not_counted = turn_from(&[r#"data: {"choices":[{"delta":{"content":"Done."},"finish_reason":"stop"}]}"#]);

        assert_eq!(counted.unwrap().tokens_in_context, Some(1230));
        assert_eq!(not_counted.unwrap().tokens_in_context, None);
    }

    #[test]
    fn a_tool_request_arriving_in_pieces_is_joined_and_read() {
        let turn = turn_from(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","type":"function","function":{"name":"read_plugin_file","arguments":""}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"path\":"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"ui.json\"}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ])
        .unwrap();

        assert_eq!(turn.stop, StopReason::WantsTools);
        assert_eq!(turn.tool_calls, [ToolCall { id: "call_1".to_string(), name: "read_plugin_file".to_string(), input: ToolInput::Valid(json!({ "path": "ui.json" })) }]);
        assert_eq!(turn.replay["tool_calls"][0]["function"]["arguments"], r#"{"path":"ui.json"}"#);
        assert!(turn.replay["content"].is_null());
    }

    #[test]
    fn two_tool_requests_in_one_turn_are_kept_apart() {
        let turn = turn_from(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"list_plugin_files","arguments":"{}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":1,"id":"b","function":{"name":"check_plugin","arguments":"{}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"stop"}]}"#,
        ])
        .unwrap();

        assert_eq!(turn.tool_calls.iter().map(|call| call.name.as_str()).collect::<Vec<_>>(), ["list_plugin_files", "check_plugin"]);
        assert_eq!(turn.stop, StopReason::WantsTools);
    }

    #[test]
    fn what_a_service_sends_along_with_a_request_is_handed_back_unchanged() {
        let turn = turn_from(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","extra_content":{"signature":"xyz"},"function":{"name":"check_plugin","arguments":"{}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ])
        .unwrap();

        assert_eq!(turn.replay["tool_calls"][0]["extra_content"], json!({ "signature": "xyz" }));
    }

    #[test]
    fn broken_tool_input_never_reaches_a_tool_and_is_sent_back_as_an_empty_object() {
        let turn = turn_from(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"a","function":{"name":"write_plugin_file","arguments":"{\"path\": \"src/"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"length"}]}"#,
        ])
        .unwrap();

        assert_eq!(turn.stop, StopReason::RanOutOfRoom);
        assert!(matches!(turn.tool_calls[0].input, ToolInput::Unparseable(_)));
        assert_eq!(turn.replay["tool_calls"][0]["function"]["arguments"], "{}");
    }

    #[test]
    fn a_reply_that_breaks_off_or_never_says_why_it_stopped_is_an_error() {
        let broke_off = turn_from(&[r#"data: {"choices":[{"delta":{"content":"Here"}}]}"#, r#"data: {"error":{"message":"the provider went away"}}"#]);
        let never_said = turn_from(&[r#"data: {"choices":[{"delta":{"content":"Here"}}]}"#]);

        assert!(matches!(broke_off, Err(ModelError::Unreachable(cause)) if cause.contains("the provider went away")));
        assert!(matches!(never_said, Err(ModelError::Garbled(_))));
    }

    #[test]
    fn a_refusal_is_told_apart_from_a_normal_finish() {
        let turn = turn_from(&[r#"data: {"choices":[{"delta":{},"finish_reason":"content_filter"}]}"#]).unwrap();

        assert_eq!(turn.stop, StopReason::Refused);
    }

    #[test]
    fn the_request_carries_the_instructions_the_tools_and_the_conversation_in_order() {
        let said = turn_from(&[
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"check_plugin","arguments":"{}"}}]}}]}"#,
            r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
        ])
        .unwrap();
        let answered = Entry::ToolResults(vec![ToolResult { call_id: "call_1".to_string(), output: ToolOutput::success("All checks passed.") }]);
        let history = [Entry::User("Make an echo.".to_string()), Entry::Model(said), answered];
        let tools = [ToolDefinition { name: "check_plugin", description: "Runs the safety check.", input_schema: json!({ "type": "object" }) }];

        let body = request_body(&Brief { instructions: "Be careful.", tools: &tools, history: &history }, &chosen(true));

        assert_eq!(body["model"], "acme/builder");
        assert_eq!(body["stream"], true);
        assert_eq!(body["tools"][0], json!({ "type": "function", "function": { "name": "check_plugin", "description": "Runs the safety check.", "parameters": { "type": "object" } } }));
        let roles: Vec<&str> = body["messages"].as_array().unwrap().iter().map(|message| message["role"].as_str().unwrap()).collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool"]);
        assert_eq!(body["messages"][3], json!({ "role": "tool", "tool_call_id": "call_1", "content": "All checks passed." }));
    }

    #[test]
    fn a_model_that_sees_pictures_gets_them_after_the_tool_answers_each_under_its_caption() {
        let messages = wire_messages(&tool_result_with_a_picture(), true);

        assert_eq!(messages[0], json!({ "role": "tool", "tool_call_id": "call_1", "content": "Styled." }));
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[1]["content"][1], json!({ "type": "text", "text": "Page MAIN" }));
        assert_eq!(messages[1]["content"][2], json!({ "type": "image_url", "image_url": { "url": "data:image/png;base64,AQID" } }));
    }

    #[test]
    fn a_model_that_cannot_see_pictures_is_told_there_was_one_and_sent_none() {
        let messages = wire_messages(&tool_result_with_a_picture(), false);

        assert_eq!(messages.len(), 1);
        assert!(messages[0]["content"].as_str().unwrap().ends_with(PICTURE_NOT_SHOWN));
    }
}
