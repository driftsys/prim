//! Behavioural test: `prim lsp` drives a real LSP session over stdio —
//! `initialize` → `didOpen` → `textDocument/formatting` → `shutdown`/`exit`
//! (story D1).

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

/// Frame one JSON-RPC message with its `Content-Length` header.
fn frame(message: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(message).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

/// Parse a stream of `Content-Length`-framed JSON-RPC messages.
fn parse_messages(mut bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    while let Some(split) = find_header_end(bytes) {
        let (header, rest) = bytes.split_at(split);
        let body_start = &rest[4..]; // skip the "\r\n\r\n"
        let length: usize = std::str::from_utf8(header)
            .unwrap()
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        messages.push(serde_json::from_slice(&body_start[..length]).unwrap());
        bytes = &body_start[length..];
    }
    messages
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes.windows(4).position(|window| window == b"\r\n\r\n")
}

/// Drive one full session: write every `messages` frame to `prim lsp`'s stdin,
/// close it, and return the parsed responses plus the exit code.
fn run_session(messages: &[Value]) -> (Vec<Value>, i32) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_prim"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn prim lsp");

    {
        let mut stdin = child.stdin.take().unwrap();
        for message in messages {
            stdin.write_all(&frame(message)).unwrap();
        }
        // Dropping stdin closes it, signalling end of input.
    }

    let mut stdout = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    let status = child.wait().unwrap();

    (parse_messages(&stdout), status.code().unwrap())
}

#[test]
fn formats_a_json_buffer_over_a_full_lsp_session() {
    let uri = "file:///tmp/prim-lsp-behaviour.json";
    let (responses, code) = run_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({"jsonrpc": "2.0", "method": "initialized", "params": {}}),
        json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {
                "uri": uri, "languageId": "json", "version": 1,
                "text": "{\"a\":1,\"b\":[1,2]}\n"
            }}
        }),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
            "params": {"textDocument": {"uri": uri}, "options": {"tabSize": 2, "insertSpaces": true}}
        }),
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}),
        json!({"jsonrpc": "2.0", "method": "exit"}),
    ]);

    assert_eq!(code, 0, "clean shutdown/exit must return 0");

    let initialize = find_response(&responses, 1);
    assert_eq!(initialize["result"]["capabilities"]["textDocumentSync"], 1);
    assert_eq!(
        initialize["result"]["capabilities"]["documentFormattingProvider"],
        true
    );

    let formatting = find_response(&responses, 2);
    let edits = formatting["result"].as_array().expect("edits array");
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0]["newText"], "{ \"a\": 1, \"b\": [1, 2] }\n");
    // Whole-document replace: origin to one-past-the-single-newline.
    assert_eq!(
        edits[0]["range"]["start"],
        json!({"line": 0, "character": 0})
    );
    assert_eq!(edits[0]["range"]["end"], json!({"line": 1, "character": 0}));

    assert_eq!(find_response(&responses, 3)["result"], Value::Null);
}

#[test]
fn already_formatted_buffer_yields_no_edits() {
    let uri = "file:///tmp/prim-lsp-clean.json";
    let (responses, code) = run_session(&[
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "languageId": "json", "version": 1, "text": "{ \"a\": 1 }\n"}}
        }),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
            "params": {"textDocument": {"uri": uri}}
        }),
        json!({"jsonrpc": "2.0", "id": 3, "method": "shutdown"}),
        json!({"jsonrpc": "2.0", "method": "exit"}),
    ]);

    assert_eq!(code, 0);
    assert_eq!(
        find_response(&responses, 2)["result"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
}

fn find_response(responses: &[Value], id: i64) -> &Value {
    responses
        .iter()
        .find(|response| response["id"] == id)
        .unwrap_or_else(|| panic!("no response with id {id}"))
}
