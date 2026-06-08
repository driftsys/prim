//! Whitespace hygiene (FR-2.1/2.2/2.3): the format-agnostic pass applied to
//! every file prim owns, driven by the resolved [`Style`].

use crate::Style;

/// Apply whitespace hygiene to `source` under `style`:
///
/// - normalise every line ending to `style.end_of_line` (FR-2.3),
/// - when `style.trim_trailing_whitespace`, strip trailing whitespace from each
///   line (FR-2.1),
/// - when `style.insert_final_newline`, end non-empty content with exactly one
///   line ending; otherwise strip any final ending (FR-2.2). Empty (or, when
///   trimming, whitespace-only) content stays empty.
///
/// The pass is idempotent (FR-6.1).
pub fn hygiene(source: &str, style: &Style) -> String {
    // Normalise existing endings to LF so we can reason in logical lines.
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");

    // Optionally strip trailing whitespace, re-joining by LF for now.
    let mut joined = String::with_capacity(normalized.len());
    for line in normalized.split('\n') {
        if style.trim_trailing_whitespace {
            joined.push_str(line.trim_end());
        } else {
            joined.push_str(line);
        }
        joined.push('\n');
    }

    // Content body with the trailing newline run removed.
    let body = joined.trim_end_matches('\n');
    if body.is_empty() {
        return String::new();
    }

    // Apply the configured line ending and the final-newline rule.
    let eol = style.end_of_line.as_str();
    let mut result = body.replace('\n', eol);
    if style.insert_final_newline {
        result.push_str(eol);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Indent, LineEnding};

    fn canonical() -> Style {
        Style::default()
    }

    // --- Regression guard: default Style == the pre-#8 hard-coded behaviour ---

    #[test]
    fn default_trims_trailing_whitespace_per_line() {
        assert_eq!(hygiene("a  \nb\t\n", &canonical()), "a\nb\n");
    }

    #[test]
    fn default_preserves_leading_and_inner_whitespace() {
        assert_eq!(hygiene("  a  b  \n", &canonical()), "  a  b\n");
    }

    #[test]
    fn default_ensures_single_final_newline() {
        assert_eq!(hygiene("a", &canonical()), "a\n");
        assert_eq!(hygiene("a\n\n\n", &canonical()), "a\n");
    }

    #[test]
    fn default_normalizes_crlf_and_cr_to_lf() {
        assert_eq!(hygiene("a\r\nb\rc\n", &canonical()), "a\nb\nc\n");
    }

    #[test]
    fn default_empty_or_whitespace_only_stays_empty() {
        assert_eq!(hygiene("", &canonical()), "");
        assert_eq!(hygiene("   \n  \n", &canonical()), "");
    }

    // --- Style-driven behaviour (FR-3.2) ---

    #[test]
    fn crlf_end_of_line_is_emitted() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        assert_eq!(hygiene("a\nb\n", &style), "a\r\nb\r\n");
        // Mixed input normalises to the configured ending.
        assert_eq!(hygiene("a\r\nb\nc\r\n", &style), "a\r\nb\r\nc\r\n");
    }

    #[test]
    fn trim_disabled_preserves_trailing_whitespace_but_normalizes_eol() {
        let style = Style {
            trim_trailing_whitespace: false,
            ..Style::default()
        };
        assert_eq!(hygiene("a  \r\nb \n", &style), "a  \nb \n");
    }

    #[test]
    fn insert_final_newline_false_strips_final_newline() {
        let style = Style {
            insert_final_newline: false,
            ..Style::default()
        };
        assert_eq!(hygiene("a\nb\n", &style), "a\nb");
        assert_eq!(hygiene("a\n\n", &style), "a");
    }

    #[test]
    fn carried_fields_do_not_affect_hygiene() {
        // indent / max_line_length are unconsumed by hygiene; output unchanged.
        let style = Style {
            indent: Indent::Tab,
            max_line_length: Some(100),
            ..Style::default()
        };
        assert_eq!(hygiene("a  \nb\n", &style), "a\nb\n");
    }

    #[test]
    fn is_idempotent_under_each_style() {
        let styles = [
            Style::default(),
            Style {
                end_of_line: LineEnding::CrLf,
                ..Style::default()
            },
            Style {
                trim_trailing_whitespace: false,
                ..Style::default()
            },
            Style {
                insert_final_newline: false,
                ..Style::default()
            },
        ];
        for style in styles {
            for input in ["a  \r\nb\n\n", "", "x", "  keep\nlead  \n", "   \n"] {
                let once = hygiene(input, &style);
                assert_eq!(
                    hygiene(&once, &style),
                    once,
                    "not idempotent: {input:?} / {style:?}"
                );
            }
        }
    }
}
