//! Structured, positioned lint findings for the whitespace-hygiene pass
//! (story B1). Each check has a stable code and a 1-indexed `line:column`,
//! computed with [`crate::line_col`] (AD-0008/spike #42).
//!
//! Scope (per B1's AC): these checks cover the orphan/un-owned-text allowlist
//! (the same set A1's BOM strip covers) — BOM, mixed line endings, trailing
//! whitespace, tab/space indentation, and a missing final newline. Structured
//! formats (JSON/JSONC/TOML/YAML/Markdown) keep `prim lint`'s coarser
//! format-drift finding; their own content diagnostics are future stories
//! (G2/D2).

use crate::position::line_col;
use crate::style::{Indent, Style};

/// One lint finding: a stable code, a human message, and its 1-indexed
/// position in the source that was scanned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// A stable, namespaced code (e.g. `hygiene::bom`) — safe to match on.
    pub code: &'static str,
    /// 1-indexed line number.
    pub line: usize,
    /// 1-indexed column (counts chars, not bytes; see [`crate::line_col`]).
    pub column: usize,
    /// Human-readable description of the finding.
    pub message: String,
}

/// One physical line of `body`, split on `\n`/`\r\n`/`\r`.
struct Line<'a> {
    /// Byte offset of the line's first content char.
    start: usize,
    /// The line's content, terminator excluded.
    content: &'a str,
    /// Byte offset where the terminator (if any) begins.
    terminator_offset: usize,
    /// The terminator sequence, or `None` for an unterminated final line.
    terminator: Option<&'static str>,
}

/// Split `body` into [`Line`]s. Splitting on the single-byte ASCII markers
/// `\n`/`\r` is safe on a `&str`: neither byte value ever occurs inside a
/// multi-byte UTF-8 continuation sequence, so every split point is a char
/// boundary.
fn lines_with_terminators(body: &str) -> Vec<Line<'_>> {
    let bytes = body.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                lines.push(Line {
                    start,
                    content: &body[start..index],
                    terminator_offset: index,
                    terminator: Some("\n"),
                });
                index += 1;
                start = index;
            }
            b'\r' => {
                let terminator = if bytes.get(index + 1) == Some(&b'\n') {
                    "\r\n"
                } else {
                    "\r"
                };
                lines.push(Line {
                    start,
                    content: &body[start..index],
                    terminator_offset: index,
                    terminator: Some(terminator),
                });
                index += terminator.len();
                start = index;
            }
            _ => index += 1,
        }
    }
    if start < bytes.len() {
        lines.push(Line {
            start,
            content: &body[start..],
            terminator_offset: bytes.len(),
            terminator: None,
        });
    }
    lines
}

/// Human name for a line-ending sequence, for diagnostic messages.
fn eol_name(terminator: &str) -> &'static str {
    match terminator {
        "\n" => "LF",
        "\r\n" => "CRLF",
        "\r" => "CR",
        _ => unreachable!("lines_with_terminators only yields LF/CRLF/CR"),
    }
}

/// Scan `source` for whitespace-hygiene violations under `style`, one
/// [`Diagnostic`] per finding, ordered by position.
///
/// This mirrors [`crate::hygiene::hygiene`]'s own rules (BOM, line endings,
/// trailing whitespace, final newline) plus one lint-only check hygiene does
/// not perform: leading-indentation character vs. the resolved
/// `indent_style` (`Style::indent`) — orphan files are never reformatted for
/// indentation, so a mismatch here is only ever reported, never fixed.
pub fn hygiene_diagnostics(source: &str, style: &Style) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if let Some(body) = source.strip_prefix('\u{feff}') {
        diagnostics.push(Diagnostic {
            code: "hygiene::bom",
            line: 1,
            column: 1,
            message: "file starts with a UTF-8 byte-order mark (BOM)".to_string(),
        });
        scan_lines(body, style, &mut diagnostics);
    } else {
        scan_lines(source, style, &mut diagnostics);
    }

    diagnostics.sort_by(|a, b| (a.line, a.column, a.code).cmp(&(b.line, b.column, b.code)));
    diagnostics
}

/// Check every line of (BOM-stripped) `body` for the remaining four
/// violation types and push any findings onto `diagnostics`.
fn scan_lines(body: &str, style: &Style, diagnostics: &mut Vec<Diagnostic>) {
    let canonical_eol = style.end_of_line.as_str();
    let lines = lines_with_terminators(body);
    let last_index = lines.len().saturating_sub(1);

    for (index, line) in lines.iter().enumerate() {
        if style.trim_trailing_whitespace {
            let trimmed = line.content.trim_end();
            if trimmed.len() < line.content.len() {
                let offset = line.start + trimmed.len();
                let (row, column) = line_col(body, offset);
                diagnostics.push(Diagnostic {
                    code: "hygiene::trailing-whitespace",
                    line: row,
                    column,
                    message: "trailing whitespace".to_string(),
                });
            }
        }

        let indent_end = line
            .content
            .find(|ch: char| ch != ' ' && ch != '\t')
            .unwrap_or(line.content.len());
        // Only judge indentation when the line has real content after it —
        // a blank/whitespace-only line has no indentation to be wrong about.
        if indent_end < line.content.len() {
            let prefix = &line.content[..indent_end];
            let offending = match style.indent {
                Indent::Spaces(_) => prefix.find('\t'),
                Indent::Tab => prefix.find(' '),
            };
            if let Some(at) = offending {
                let offset = line.start + at;
                let (row, column) = line_col(body, offset);
                let message = match style.indent {
                    Indent::Spaces(_) => "indentation uses a tab; expected spaces",
                    Indent::Tab => "indentation uses a space; expected a tab",
                };
                diagnostics.push(Diagnostic {
                    code: "hygiene::indent",
                    line: row,
                    column,
                    message: message.to_string(),
                });
            }
        }

        match line.terminator {
            Some(terminator) if terminator != canonical_eol => {
                let (row, column) = line_col(body, line.terminator_offset);
                diagnostics.push(Diagnostic {
                    code: "hygiene::eol",
                    line: row,
                    column,
                    message: format!(
                        "line ending is {}; expected {}",
                        eol_name(terminator),
                        eol_name(canonical_eol)
                    ),
                });
            }
            None if index == last_index
                && style.insert_final_newline
                && !line.content.is_empty() =>
            {
                let (row, column) = line_col(body, line.terminator_offset);
                diagnostics.push(Diagnostic {
                    code: "hygiene::final-newline",
                    line: row,
                    column,
                    message: "missing a final newline".to_string(),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
        diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn clean_file_has_no_findings() {
        let style = Style::default();
        assert_eq!(hygiene_diagnostics("title\n", &style), vec![]);
    }

    #[test]
    fn flags_a_leading_bom_at_the_start_of_the_file() {
        let style = Style::default();
        let diagnostics = hygiene_diagnostics("\u{feff}title\n", &style);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "hygiene::bom");
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 1));
    }

    #[test]
    fn flags_trailing_whitespace_with_its_column() {
        let style = Style::default();
        let diagnostics = hygiene_diagnostics("title  \nbody\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::trailing-whitespace"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 6));
    }

    #[test]
    fn does_not_flag_trailing_whitespace_when_style_disables_trimming() {
        let style = Style {
            trim_trailing_whitespace: false,
            ..Style::default()
        };
        assert_eq!(hygiene_diagnostics("title  \n", &style), vec![]);
    }

    #[test]
    fn flags_a_line_ending_that_does_not_match_the_canonical_style() {
        let style = Style::default(); // canonical LF
        let diagnostics = hygiene_diagnostics("a\r\nb\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::eol"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 2));
    }

    #[test]
    fn flags_mixed_endings_once_per_offending_line() {
        let style = Style::default();
        let diagnostics = hygiene_diagnostics("a\r\nb\nc\r\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::eol", "hygiene::eol"]);
        assert_eq!(diagnostics[0].line, 1);
        assert_eq!(diagnostics[1].line, 3);
    }

    #[test]
    fn flags_a_missing_final_newline() {
        let style = Style::default();
        let diagnostics = hygiene_diagnostics("title", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::final-newline"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 6));
    }

    #[test]
    fn does_not_flag_a_missing_final_newline_when_style_forbids_one() {
        let style = Style {
            insert_final_newline: false,
            ..Style::default()
        };
        assert_eq!(hygiene_diagnostics("title", &style), vec![]);
    }

    #[test]
    fn flags_a_tab_used_for_indentation_when_style_wants_spaces() {
        let style = Style::default(); // Indent::Spaces(2)
        let diagnostics = hygiene_diagnostics("a\n\ttab-indented\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::indent"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 1));
    }

    #[test]
    fn flags_a_space_used_for_indentation_when_style_wants_tabs() {
        let style = Style {
            indent: Indent::Tab,
            ..Style::default()
        };
        let diagnostics = hygiene_diagnostics("a\n  space-indented\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::indent"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (2, 1));
    }

    #[test]
    fn does_not_flag_indentation_on_a_blank_line() {
        let style = Style::default();
        // Line 2 is whitespace-only: it legitimately trips
        // `trailing-whitespace` (the whole line is trailing whitespace), but
        // must not also trip `indent` — there's no real content to misjudge.
        let diagnostics = hygiene_diagnostics("a\n\t\nb\n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::trailing-whitespace"]);
    }

    #[test]
    fn orders_findings_by_position_not_check_type() {
        let style = Style::default();
        // Line 1: trailing whitespace *and* a non-canonical ending; line 2:
        // a tab-indent violation. Expect file order, not check-type order.
        let diagnostics = hygiene_diagnostics("a  \r\n\tb\n", &style);
        assert_eq!(
            codes(&diagnostics),
            vec![
                "hygiene::trailing-whitespace",
                "hygiene::eol",
                "hygiene::indent"
            ]
        );
    }

    #[test]
    fn multi_byte_chars_report_char_columns_not_byte_offsets() {
        let style = Style::default();
        // "é " — a 2-byte char then a trailing space.
        let diagnostics = hygiene_diagnostics("é \n", &style);
        assert_eq!(codes(&diagnostics), vec!["hygiene::trailing-whitespace"]);
        assert_eq!((diagnostics[0].line, diagnostics[0].column), (1, 2));
    }
}
