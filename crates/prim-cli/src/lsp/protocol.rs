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

/// LSP `DiagnosticSeverity::Error`. prim's error-tier findings map here.
pub const SEVERITY_ERROR: u8 = 1;
/// LSP `DiagnosticSeverity::Warning`. prim's warning-tier findings map here.
pub const SEVERITY_WARNING: u8 = 2;

/// One `textDocument/publishDiagnostics` diagnostic (story G5's follow-up,
/// issue #83): prim's own hygiene (B1) and Markdown content (G2) findings,
/// reused as-is and reprojected onto LSP's range/severity shape.
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: u8,
    pub code: String,
    pub source: &'static str,
    pub message: String,
}

/// Convert a 1-indexed `(line, column)` position — `column` counting **chars**
/// from the start of the line, the convention `prim_fmt::line_col` and
/// `prim_fmt::MdDiagnostic` both use — into an LSP [`Position`]: 0-indexed
/// line, **UTF-16** code-unit character offset. A `line` past the end of
/// `text` (should not happen, but diagnostics must never panic) yields an
/// empty line's worth of characters.
pub fn position_from_line_col(text: &str, line: usize, column: usize) -> Position {
    let line_text = text.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let character = line_text
        .chars()
        .take(column.saturating_sub(1))
        .map(char::len_utf16)
        .sum::<usize>() as u32;
    Position {
        line: line.saturating_sub(1) as u32,
        character,
    }
}

/// A one-character [`Range`] starting at `(line, column)` — prim's own
/// diagnostics carry only a point position, but LSP diagnostics require a
/// range, so this widens the point by one character (standard practice for
/// point diagnostics; clients clamp a range that runs past end-of-line).
pub fn point_range(text: &str, line: usize, column: usize) -> Range {
    let start = position_from_line_col(text, line, column);
    let end = Position {
        line: start.line,
        character: start.character + 1,
    };
    Range { start, end }
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

    #[test]
    fn position_from_line_col_converts_1_indexed_chars_on_the_first_line() {
        // "héllo": h=1, é=2, l=3 (1-indexed char columns).
        assert_eq!(
            position_from_line_col("héllo\n", 1, 3),
            Position {
                line: 0,
                character: 2, // "h" (1 unit) + "é" (1 unit)
            }
        );
    }

    #[test]
    fn position_from_line_col_finds_a_later_line() {
        assert_eq!(
            position_from_line_col("abc\ndéf\n", 2, 2),
            Position {
                line: 1,
                character: 1, // "d" is 1 UTF-16 unit
            }
        );
    }

    #[test]
    fn position_from_line_col_counts_a_surrogate_pair_as_two_units() {
        // "a𝄞b": a=1, 𝄞=2 (surrogate pair), b=3 (1-indexed char columns).
        assert_eq!(
            position_from_line_col("a𝄞b\n", 1, 3),
            Position {
                line: 0,
                character: 3, // "a" (1) + "𝄞" (2 units)
            }
        );
    }

    #[test]
    fn point_range_spans_exactly_one_character() {
        assert_eq!(
            point_range("abc\n", 1, 2),
            Range {
                start: Position {
                    line: 0,
                    character: 1
                },
                end: Position {
                    line: 0,
                    character: 2
                },
            }
        );
    }
}
