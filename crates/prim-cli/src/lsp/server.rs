//! The prim LSP server state machine: an in-memory store of open documents
//! (keyed by URI) and the handlers for the requests and notifications prim
//! supports. Pure over its input messages — the stdio loop lives in
//! [`super::run`], so this is unit-testable without spawning a process.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::{Value, json};

use super::protocol::{
    DidChangeParams, DidCloseParams, DidOpenParams, FormattingParams, TextEdit,
    full_document_range, initialize_result,
};
use crate::editorconfig;

/// What the stdio loop should do with a handled message.
pub enum Reaction {
    /// Send this JSON-RPC response back to the client.
    Reply(Value),
    /// The message was a notification (or otherwise needs no reply).
    None,
    /// Stop the loop and exit the process with this code.
    Exit(i32),
}

/// The LSP server: the open-document store plus a cached `.editorconfig`
/// resolver reused across formatting requests.
#[derive(Default)]
pub struct Server {
    documents: HashMap<String, String>,
    resolver: editorconfig::Resolver,
    shutdown_requested: bool,
}

impl Server {
    /// A server with no open documents.
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the client has sent a `shutdown` request. The stdio loop reads
    /// this to pick the exit code when the stream ends without an `exit`.
    pub fn shutdown_requested(&self) -> bool {
        self.shutdown_requested
    }

    /// Handle one incoming JSON-RPC `message`, updating state and returning
    /// the [`Reaction`] the transport loop should carry out.
    pub fn handle(&mut self, message: &Value) -> Reaction {
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        match (method, id) {
            (Some("initialize"), Some(id)) => Reaction::Reply(success(id, initialize_result())),
            (Some("shutdown"), Some(id)) => {
                self.shutdown_requested = true;
                Reaction::Reply(success(id, Value::Null))
            }
            (Some("exit"), _) => Reaction::Exit(if self.shutdown_requested { 0 } else { 1 }),
            (Some("textDocument/didOpen"), _) => {
                self.did_open(params);
                Reaction::None
            }
            (Some("textDocument/didChange"), _) => {
                self.did_change(params);
                Reaction::None
            }
            (Some("textDocument/didClose"), _) => {
                self.did_close(params);
                Reaction::None
            }
            (Some("textDocument/formatting"), Some(id)) => {
                Reaction::Reply(success(id, self.formatting(params)))
            }
            // Any other request must be answered; notifications are ignored.
            (Some(_), Some(id)) => Reaction::Reply(method_not_found(id)),
            _ => Reaction::None,
        }
    }

    fn did_open(&mut self, params: Value) {
        if let Ok(params) = serde_json::from_value::<DidOpenParams>(params) {
            self.documents
                .insert(params.text_document.uri, params.text_document.text);
        }
    }

    fn did_change(&mut self, params: Value) {
        let Ok(params) = serde_json::from_value::<DidChangeParams>(params) else {
            return;
        };
        // Full sync (the only mode prim advertises): the last change carries
        // the entire new document, so prim never splices deltas.
        if let Some(change) = params.content_changes.into_iter().next_back() {
            self.documents.insert(params.text_document.uri, change.text);
        }
    }

    fn did_close(&mut self, params: Value) {
        if let Ok(params) = serde_json::from_value::<DidCloseParams>(params) {
            self.documents.remove(&params.text_document.uri);
        }
    }

    /// Format the requested document, returning the `TextEdit[]` result. An
    /// untracked document, a file type prim does not own, an already-formatted
    /// buffer, or a parse failure all yield no edits — prim never hands back
    /// edits that would corrupt or reflow content it cannot format.
    fn formatting(&mut self, params: Value) -> Value {
        let Ok(params) = serde_json::from_value::<FormattingParams>(params) else {
            return json!([]);
        };
        let Some(text) = self.documents.get(&params.text_document.uri) else {
            return json!([]);
        };
        let Some(path) = uri_to_path(&params.text_document.uri) else {
            return json!([]);
        };
        let Some(kind) = prim_fmt::classify(&path) else {
            return json!([]);
        };

        let style = self.resolver.resolve(&path);
        match prim_fmt::format(kind, text, &style) {
            Ok(formatted) if &formatted == text => json!([]),
            Ok(formatted) => {
                let edit = TextEdit {
                    range: full_document_range(text),
                    new_text: formatted,
                };
                serde_json::to_value([edit]).unwrap_or_else(|_| json!([]))
            }
            Err(_) => json!([]),
        }
    }
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn method_not_found(id: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": -32601, "message": "method not found" }
    })
}

/// Convert a `file://` URI to a filesystem path, percent-decoding the path.
/// Non-`file` URIs and undecodable paths yield `None`. Windows drive-letter
/// URIs (`file:///C:/…`) are not yet handled — prim targets Unix hosts first.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Skip an optional authority ("host") so both `file:///path` (empty
    // authority) and `file://host/path` resolve to the absolute path.
    let index = rest.find('/')?;
    let path = &rest[index..];
    Some(PathBuf::from(percent_decode(path)?))
}

/// Decode `%XX` escapes in a URI path component back to raw bytes, then to a
/// UTF-8 string. A malformed escape is left literal; invalid UTF-8 is `None`.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = (bytes[index + 1] as char).to_digit(16);
            let lo = (bytes[index + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_to_path_decodes_a_file_uri() {
        assert_eq!(
            uri_to_path("file:///tmp/my%20doc.md"),
            Some(PathBuf::from("/tmp/my doc.md"))
        );
    }

    #[test]
    fn uri_to_path_skips_an_authority() {
        assert_eq!(
            uri_to_path("file://host/tmp/a.json"),
            Some(PathBuf::from("/tmp/a.json"))
        );
    }

    #[test]
    fn uri_to_path_rejects_non_file_schemes() {
        assert!(uri_to_path("http://example.com/a.md").is_none());
    }

    #[test]
    fn initialize_reports_full_sync_and_formatting() {
        let mut server = Server::new();
        let reaction = server.handle(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
        }));
        let Reaction::Reply(reply) = reaction else {
            panic!("initialize must reply");
        };
        let caps = &reply["result"]["capabilities"];
        assert_eq!(caps["textDocumentSync"], 1);
        assert_eq!(caps["documentFormattingProvider"], true);
    }

    #[test]
    fn shutdown_then_exit_is_a_clean_zero() {
        let mut server = Server::new();
        let _ = server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "shutdown"}));
        match server.handle(&json!({"jsonrpc": "2.0", "method": "exit"})) {
            Reaction::Exit(code) => assert_eq!(code, 0),
            _ => panic!("exit must stop the loop"),
        }
    }

    #[test]
    fn exit_without_shutdown_is_a_nonzero_code() {
        let mut server = Server::new();
        match server.handle(&json!({"jsonrpc": "2.0", "method": "exit"})) {
            Reaction::Exit(code) => assert_eq!(code, 1),
            _ => panic!("exit must stop the loop"),
        }
    }

    #[test]
    fn formatting_a_dirty_json_buffer_returns_a_whole_document_edit() {
        let mut server = Server::new();
        let uri = "file:///tmp/prim-lsp-test.json";
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "text": "{\"a\":1}\n" } }
        }));
        let reaction = server.handle(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
            "params": { "textDocument": { "uri": uri } }
        }));
        let Reaction::Reply(reply) = reaction else {
            panic!("formatting must reply");
        };
        let edits = reply["result"].as_array().expect("edits array");
        assert_eq!(edits.len(), 1);
        assert!(edits[0]["newText"].as_str().unwrap().contains("\"a\": 1"));
    }

    #[test]
    fn formatting_an_unowned_file_type_returns_no_edits() {
        let mut server = Server::new();
        let uri = "file:///tmp/prim-lsp-test.rs";
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "text": "fn  main(){}\n" } }
        }));
        let reaction = server.handle(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "textDocument/formatting",
            "params": { "textDocument": { "uri": uri } }
        }));
        let Reaction::Reply(reply) = reaction else {
            panic!("formatting must reply");
        };
        assert_eq!(reply["result"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn didchange_replaces_the_tracked_buffer() {
        let mut server = Server::new();
        let uri = "file:///tmp/prim-lsp-change.json";
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "text": "{}\n" } }
        }));
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": {
                "textDocument": { "uri": uri },
                "contentChanges": [{ "text": "{\"b\":2}\n" }]
            }
        }));
        assert_eq!(server.documents[uri], "{\"b\":2}\n");
    }

    #[test]
    fn didclose_forgets_the_document() {
        let mut server = Server::new();
        let uri = "file:///tmp/prim-lsp-close.json";
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didOpen",
            "params": { "textDocument": { "uri": uri, "text": "{}\n" } }
        }));
        server.handle(&json!({
            "jsonrpc": "2.0", "method": "textDocument/didClose",
            "params": { "textDocument": { "uri": uri } }
        }));
        assert!(!server.documents.contains_key(uri));
    }

    #[test]
    fn unknown_request_gets_a_method_not_found_error() {
        let mut server = Server::new();
        let reaction = server.handle(&json!({
            "jsonrpc": "2.0", "id": 9, "method": "textDocument/hover", "params": {}
        }));
        let Reaction::Reply(reply) = reaction else {
            panic!("a request must be answered");
        };
        assert_eq!(reply["error"]["code"], -32601);
    }
}
