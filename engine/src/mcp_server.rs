//! Offers a toolbox to an outside agent program, such as Claude Code, over the Model Context Protocol.
//!
//! The protocol is JSON-RPC, one message per line, on standard input and output. Only the handful of
//! messages needed to list and call tools are answered.

use std::io::{self, BufRead, Write};

use serde_json::{Value, json};

use crate::agent::{Illustration, ToolDefinition, ToolOutput, Toolbox};

pub const SERVER_NAME: &str = "workbench";
const PROTOCOL_VERSION_IF_UNSTATED: &str = "2025-06-18";
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

struct Refusal {
    code: i64,
    message: &'static str,
}

pub fn serve(toolbox: &mut dyn Toolbox, requests: impl BufRead, mut replies: impl Write) -> io::Result<()> {
    for line in requests.lines() {
        let Ok(message) = serde_json::from_str::<Value>(&line?) else { continue };
        if let Some(reply) = reply_to(toolbox, &message) {
            writeln!(replies, "{reply}")?;
            replies.flush()?;
        }
    }
    Ok(())
}

/// Only requests are answered. A request has both a method and an id; a notification has no id.
fn reply_to(toolbox: &mut dyn Toolbox, message: &Value) -> Option<Value> {
    let id = message.get("id")?;
    let method = message["method"].as_str()?;
    Some(match result_of(toolbox, method, &message["params"]) {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(refusal) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": refusal.code, "message": refusal.message } }),
    })
}

fn result_of(toolbox: &mut dyn Toolbox, method: &str, params: &Value) -> Result<Value, Refusal> {
    match method {
        "initialize" => Ok(introduction(params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": toolbox.definitions().iter().map(wire_tool).collect::<Vec<Value>>() })),
        "tools/call" => call_tool(toolbox, params),
        _ => Err(Refusal { code: METHOD_NOT_FOUND, message: "This server only lists and calls tools." }),
    }
}

fn introduction(params: &Value) -> Value {
    let protocol_version = params["protocolVersion"].as_str().unwrap_or(PROTOCOL_VERSION_IF_UNSTATED);
    json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
    })
}

fn wire_tool(tool: &ToolDefinition) -> Value {
    json!({ "name": tool.name, "description": tool.description, "inputSchema": tool.input_schema })
}

fn call_tool(toolbox: &mut dyn Toolbox, params: &Value) -> Result<Value, Refusal> {
    let name = params["name"].as_str().ok_or(Refusal { code: INVALID_PARAMS, message: "A tool call needs the tool's name." })?;
    let no_arguments = json!({});
    let arguments = params.get("arguments").filter(|arguments| arguments.is_object()).unwrap_or(&no_arguments);
    let output = toolbox.run(name, arguments);
    Ok(json!({ "content": wire_content(&output), "isError": output.is_error }))
}

/// The tool's words first, then each picture after its caption.
fn wire_content(output: &ToolOutput) -> Vec<Value> {
    let words = json!({ "type": "text", "text": output.content });
    std::iter::once(words).chain(output.illustrations.iter().flat_map(wire_illustration)).collect()
}

fn wire_illustration(illustration: &Illustration) -> [Value; 2] {
    [
        json!({ "type": "text", "text": illustration.caption }),
        json!({ "type": "image", "data": illustration.png_as_base64(), "mimeType": "image/png" }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoToolbox;

    impl Toolbox for EchoToolbox {
        fn definitions(&self) -> Vec<ToolDefinition> {
            vec![ToolDefinition { name: "echo", description: "Says it back", input_schema: json!({ "type": "object" }) }]
        }

        fn run(&mut self, tool_name: &str, input: &Value) -> ToolOutput {
            match tool_name {
                "echo" => ToolOutput::success(input["words"].as_str().unwrap_or("nothing").to_string()),
                "sketch" => ToolOutput::success("drawn").illustrated_with(vec![Illustration { caption: "a dot".to_string(), png: b"png".to_vec() }]),
                _ => ToolOutput::failure("no such tool"),
            }
        }
    }

    fn reply(message: Value) -> Option<Value> {
        reply_to(&mut EchoToolbox, &message)
    }

    #[test]
    fn the_handshake_agrees_to_the_callers_protocol_version() {
        let answer = reply(json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2099-01-01" } })).unwrap();

        assert_eq!(answer["id"], 1);
        assert_eq!(answer["result"]["protocolVersion"], "2099-01-01");
        assert_eq!(answer["result"]["serverInfo"]["name"], SERVER_NAME);
    }

    #[test]
    fn the_tools_are_listed_with_their_input_shape() {
        let answer = reply(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();

        assert_eq!(answer["result"]["tools"], json!([{ "name": "echo", "description": "Says it back", "inputSchema": { "type": "object" } }]));
    }

    #[test]
    fn a_tool_call_runs_the_tool_and_returns_its_words() {
        let call = json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "echo", "arguments": { "words": "hello" } } });

        let answer = reply(call).unwrap();

        assert_eq!(answer["result"], json!({ "content": [{ "type": "text", "text": "hello" }], "isError": false }));
    }

    #[test]
    fn a_picture_from_a_tool_follows_its_words_and_its_caption() {
        let call = json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "sketch" } });

        let answer = reply(call).unwrap();

        let expected = json!([
            { "type": "text", "text": "drawn" },
            { "type": "text", "text": "a dot" },
            { "type": "image", "data": "cG5n", "mimeType": "image/png" },
        ]);
        assert_eq!(answer["result"]["content"], expected);
    }

    #[test]
    fn a_failed_tool_is_reported_as_a_tool_error_not_a_protocol_error() {
        let call = json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "run_shell" } });

        let answer = reply(call).unwrap();

        assert_eq!(answer["result"]["isError"], true);
        assert!(answer.get("error").is_none());
    }

    #[test]
    fn notifications_get_no_reply_and_unknown_requests_are_refused() {
        let notification = reply(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        let unknown = reply(json!({ "jsonrpc": "2.0", "id": 5, "method": "resources/list" })).unwrap();

        assert!(notification.is_none());
        assert_eq!(unknown["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn requests_are_answered_one_line_each_and_junk_is_skipped() {
        let requests = "not json\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n";
        let mut replies = Vec::new();

        serve(&mut EchoToolbox, requests.as_bytes(), &mut replies).unwrap();

        let written = String::from_utf8(replies).unwrap();
        assert_eq!(written.lines().count(), 2);
        assert!(written.lines().all(|line| serde_json::from_str::<Value>(line).unwrap()["result"] == json!({})));
    }
}
