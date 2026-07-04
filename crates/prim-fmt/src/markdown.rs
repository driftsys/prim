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

    let guarded = guard_markdown_fences(source);
    let result = format_text(&guarded, &config, |_, _, _| Ok(None));
    match result {
        Ok(Some(formatted)) => Ok(hygiene(&unguard_markdown_fences(&formatted), style)),
        Ok(None) => Ok(hygiene(source, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}

/// dprint-plugin-markdown unconditionally recurses into fenced blocks tagged
/// `markdown`/`md` (the tag is matched before the code-block callback runs),
/// which would violate FR-1.6. Guard: swap the fence language for a sentinel
/// tag dprint treats as foreign (and therefore preserves verbatim), then
/// restore it after formatting.
const GUARD_MARKDOWN: &str = "prim-fence-guard-markdown";
const GUARD_MD: &str = "prim-fence-guard-md";

fn guard_markdown_fences(source: &str) -> String {
    swap_fence_languages(source, &[("markdown", GUARD_MARKDOWN), ("md", GUARD_MD)])
}

fn unguard_markdown_fences(source: &str) -> String {
    swap_fence_languages(source, &[(GUARD_MARKDOWN, "markdown"), (GUARD_MD, "md")])
}

/// Rewrite the language word of every fenced-code opening line whose language
/// exactly matches a swap source. Lines are inspected structurally: optional
/// indentation and blockquote markers, a run of ≥ 3 backticks or tildes, then
/// the info string. Every rewrite is reversed by the opposite swap after
/// formatting, so a false positive inside verbatim content round-trips
/// unchanged.
fn swap_fence_languages(source: &str, swaps: &[(&str, &str)]) -> String {
    source
        .split_inclusive('\n')
        .map(|line| swap_fence_language_line(line, swaps))
        .collect()
}

fn swap_fence_language_line(line: &str, swaps: &[(&str, &str)]) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Optional indentation and blockquote markers ("  > > ").
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'>') {
        i += 1;
    }
    let fence_char = match bytes.get(i) {
        Some(b'`') => b'`',
        Some(b'~') => b'~',
        _ => return line.to_string(),
    };
    let fence_start = i;
    while i < bytes.len() && bytes[i] == fence_char {
        i += 1;
    }
    if i - fence_start < 3 {
        return line.to_string();
    }
    let lang_start = i;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let lang = &line[lang_start..i];
    for (from, to) in swaps {
        if lang == *from {
            return format!("{}{}{}", &line[..lang_start], to, &line[i..]);
        }
    }
    line.to_string()
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
    fn inline_code_spanning_a_newline_does_not_panic() {
        // dprint-core has a debug-only assertion that panics on an inline code
        // span containing a newline; disabled for the dprint-core package in the
        // dev profile (see root Cargo.toml / AD-0006). This valid Markdown must
        // format without panicking and keep the code text.
        let src = "A paragraph with `format_text(input, &Opts) -> Result<String, Error>` inline.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("format_text(input"), "code text kept: {out:?}");
        // Idempotent on its own output (which may keep the span across lines).
        assert_eq!(format(&out, &Style::default()).unwrap(), out);
    }

    #[test]
    fn preserves_markdown_tagged_fence_verbatim() {
        let src = "```markdown\nThis single line is deliberately much longer than eighty columns so that the formatter would want to wrap it.\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert_eq!(out, src, "markdown fence content and tag must survive");
    }

    #[test]
    fn preserves_md_tagged_fence_and_restores_the_tag() {
        let src = "```md\n#    spaced heading stays exactly as written\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("```md\n"), "{out:?}");
        assert!(out.contains("#    spaced heading"), "{out:?}");
    }

    #[test]
    fn no_sentinel_leaks_into_output() {
        let src = "prose\n\n```markdown\ntext\n```\n\n```md\ntext\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(!out.contains("prim-fence-guard"), "{out:?}");
    }

    #[test]
    fn other_fence_tags_are_untouched_by_the_guard() {
        let src = "```js\nconst x=1\n```\n";
        assert_eq!(guard_markdown_fences(src), src);
    }

    #[test]
    fn guard_handles_tilde_and_blockquote_fences() {
        assert_eq!(
            guard_markdown_fences("~~~markdown\n"),
            "~~~prim-fence-guard-markdown\n"
        );
        assert_eq!(
            guard_markdown_fences("> ```md\n"),
            "> ```prim-fence-guard-md\n"
        );
        // Round-trip is the invariant the fix relies on.
        assert_eq!(
            unguard_markdown_fences(&guard_markdown_fences("> ```md\n")),
            "> ```md\n"
        );
    }

    #[test]
    fn is_idempotent() {
        let src = "#  Heading\n\nA  paragraph   with   odd spacing that goes on and on and on past the wrap width here.\n\n- item\n- item\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn is_idempotent_with_a_markdown_tagged_fence() {
        let src = "#  Heading\n\n```markdown\nThis single line is deliberately much longer than eighty columns so that the formatter would want to wrap it.\n```\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
