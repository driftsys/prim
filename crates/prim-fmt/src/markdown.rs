//! Markdown formatting + prose wrap (FR-1.1/1.1a/1.6) via `dprint-plugin-markdown`.

use dprint_plugin_markdown::configuration::{ConfigurationBuilder, TextWrap};
use dprint_plugin_markdown::format_text;

use crate::hygiene::hygiene;
use crate::{FormatError, Style};

/// Format `source` as Markdown under `style`, then apply whitespace hygiene for
/// the configured line ending and final newline.
///
/// Prose is hard-wrapped to `style.max_line_length` (else 80) with the FR-1.1a
/// guardrails honored by dprint (inline code atomic, links not split, tables and
/// fenced code not wrapped, hard breaks preserved). Fenced code-block contents
/// are preserved verbatim (FR-1.6): the `format_code_block_text` callback returns
/// `Ok(None)`, so dprint never reformats embedded code.
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let config = ConfigurationBuilder::new()
        .line_width(style.max_line_length.unwrap_or(80) as u32)
        .text_wrap(TextWrap::Always)
        .build();

    let result = format_text(source, &config, |_, _, _| Ok(None));
    match result {
        Ok(Some(formatted)) => Ok(hygiene(&formatted, style)),
        Ok(None) => Ok(hygiene(source, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineEnding;

    fn max_line_width(s: &str) -> usize {
        s.lines().map(|l| l.chars().count()).max().unwrap_or(0)
    }

    #[test]
    fn normalizes_atx_heading_spacing() {
        let out = format("#   Title\n", &Style::default()).unwrap();
        assert!(out.contains("# Title"), "{out:?}");
    }

    #[test]
    fn hard_wraps_long_prose_to_width() {
        let para = "word ".repeat(40); // ~200 chars, no newlines
        let out = format(&format!("{para}\n"), &Style::default()).unwrap();
        assert!(out.contains('\n'), "wrapped onto multiple lines: {out:?}");
        assert!(max_line_width(&out) <= 80, "no line exceeds 80: {out:?}");
    }

    #[test]
    fn never_breaks_inline_code() {
        let long = "a ".repeat(50);
        let src = format!("{long}`do not break this code span` {long}\n");
        let out = format(&src, &Style::default()).unwrap();
        assert!(
            out.contains("`do not break this code span`"),
            "inline code intact: {out:?}"
        );
    }

    #[test]
    fn preserves_fenced_code_verbatim() {
        let src = "```js\nconst x=1\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("const x=1"), "code not reformatted: {out:?}");
    }

    #[test]
    fn never_splits_a_link_url() {
        let long = "word ".repeat(30);
        let src = format!("{long}[link](https://example.com/a/very/long/path/here)\n");
        let out = format(&src, &Style::default()).unwrap();
        assert!(
            out.contains("https://example.com/a/very/long/path/here"),
            "URL intact: {out:?}"
        );
    }

    #[test]
    fn preserves_hard_break() {
        // Two-space hard break: the two lines must stay separate.
        let out = format("line one  \nline two\n", &Style::default()).unwrap();
        let one = out.find("line one").unwrap();
        let two = out.find("line two").unwrap();
        assert!(out[one..two].contains('\n'), "hard break kept: {out:?}");
    }

    #[test]
    fn wraps_to_editorconfig_width() {
        let style = Style {
            max_line_length: Some(40),
            ..Style::default()
        };
        let para = "word ".repeat(40);
        let out = format(&format!("{para}\n"), &style).unwrap();
        assert!(max_line_width(&out) <= 40, "no line exceeds 40: {out:?}");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        let out = format("# Title\n", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn is_idempotent() {
        let src = "#  Heading\n\nA  paragraph   with   odd spacing that goes on and on and on past the wrap width here.\n\n- item\n- item\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
