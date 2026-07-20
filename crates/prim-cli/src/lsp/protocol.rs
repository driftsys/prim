//! The subset of the Language Server Protocol prim's formatting server
//! speaks: request/notification parameter types, the `initialize` result
//! advertising Full document sync plus whole-document formatting, and the
//! UTF-16 position math LSP `TextEdit`s require.

use serde::{Deserialize, Serialize};

/// An LSP text position. `character` is a **UTF-16** code-unit offset within
/// `line` (LSP's default `PositionEncodingKind`), not a byte or scalar offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A half-open `[start, end)` range of [`Position`]s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// A single text replacement the client applies to the buffer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: Range,
    #[serde(rename = "newText")]
    pub new_text: String,
}

/// The document a `textDocument/didOpen` notification carries.
#[derive(Clone, Debug, Deserialize)]
pub struct TextDocumentItem {
    pub uri: String,
    pub text: String,
}

/// Identifies an already-open document by URI.
#[derive(Clone, Debug, Deserialize)]
pub struct TextDocumentIdentifier {
    pub uri: String,
}

/// `textDocument/didOpen` parameters.
#[derive(Clone, Debug, Deserialize)]
pub struct DidOpenParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentItem,
}

/// One content change. Under Full sync (the only mode prim advertises) the
/// client omits `range` and sends the whole new document in `text`.
#[derive(Clone, Debug, Deserialize)]
pub struct ContentChange {
    pub text: String,
}

/// `textDocument/didChange` parameters.
#[derive(Clone, Debug, Deserialize)]
pub struct DidChangeParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
    #[serde(rename = "contentChanges")]
    pub content_changes: Vec<ContentChange>,
}

/// `textDocument/didClose` parameters.
#[derive(Clone, Debug, Deserialize)]
pub struct DidCloseParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
}

/// `textDocument/formatting` parameters. prim honors `.editorconfig`, not the
/// client-supplied `options`, so only the document identity is read.
#[derive(Clone, Debug, Deserialize)]
pub struct FormattingParams {
    #[serde(rename = "textDocument")]
    pub text_document: TextDocumentIdentifier,
}

/// The `initialize` result: prim advertises Full document sync (kind `1`) so
/// it never has to splice incremental deltas, and whole-document formatting.
pub fn initialize_result() -> serde_json::Value {
    serde_json::json!({
        "capabilities": {
            "textDocumentSync": 1,
            "documentFormattingProvider": true
        },
        "serverInfo": {
            "name": "prim",
            "version": env!("CARGO_PKG_VERSION")
        }
    })
}

/// The [`Range`] covering the whole of `text`, from `(0, 0)` to its end — used
/// to replace an entire document with its formatted form in one [`TextEdit`].
pub fn full_document_range(text: &str) -> Range {
    Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: end_position(text),
    }
}

/// The LSP [`Position`] one past the last character of `text`: the line index
/// is the number of line feeds, and the character is the **UTF-16** length of
/// the final line, so multi-byte content maps to the offsets clients expect.
fn end_position(text: &str) -> Position {
    let line = text.matches('\n').count() as u32;
    let last_line = match text.rfind('\n') {
        Some(index) => &text[index + 1..],
        None => text,
    };
    let character = last_line.chars().map(char::len_utf16).sum::<usize>() as u32;
    Position { line, character }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn end_position_of_empty_text_is_origin() {
        assert_eq!(
            end_position(""),
            Position {
                line: 0,
                character: 0
            }
        );
    }

    #[test]
    fn end_position_counts_trailing_newline_as_an_empty_final_line() {
        assert_eq!(
            end_position("a\nb\n"),
            Position {
                line: 2,
                character: 0
            }
        );
    }

    #[test]
    fn end_position_without_trailing_newline_points_past_last_char() {
        assert_eq!(
            end_position("a\nbc"),
            Position {
                line: 1,
                character: 2
            }
        );
    }

    #[test]
    fn end_position_character_counts_utf16_code_units() {
        // "é" is one UTF-16 unit; "𝄞" (U+1D11E) is a surrogate pair, two units.
        assert_eq!(
            end_position("é𝄞"),
            Position {
                line: 0,
                character: 3
            }
        );
    }

    #[test]
    fn full_document_range_spans_origin_to_end() {
        assert_eq!(
            full_document_range("a\nb\n"),
            Range {
                start: Position {
                    line: 0,
                    character: 0
                },
                end: Position {
                    line: 2,
                    character: 0
                }
            }
        );
    }
}
