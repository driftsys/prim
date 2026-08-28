//! Markdown formatting + prose wrap (FR-1.1/1.1a/1.6) via `dprint-plugin-markdown`.

use std::collections::BTreeSet;

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
        .line_width(style.effective_line_width() as u32)
        .text_wrap(TextWrap::Always)
        .build();

    let (guard_markdown, guard_md) = fence_sentinels(source);
    let guarded = guard_markdown_fences(source, &guard_markdown, &guard_md);
    let result = format_text(&guarded, &config, |_, _, _| Ok(None));
    match result {
        Ok(Some(formatted)) => {
            // Restore the guarded tags. A plain replace is safe: the sentinels
            // are absent from `source` (see `fence_sentinels`), so every
            // occurrence in `formatted` was introduced by the guard step —
            // including one dprint may have moved while reflowing prose.
            let restored = formatted
                .replace(&guard_markdown, "markdown")
                .replace(&guard_md, "md");
            Ok(hygiene(&unjoin_html_comments(&restored, source), style))
        }
        Ok(None) => Ok(hygiene(source, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}

/// dprint-plugin-markdown unconditionally recurses into fenced blocks tagged
/// `markdown`/`md` (the tag is matched before the code-block callback runs),
/// which would violate FR-1.6. Guard: swap the fence language for a sentinel
/// tag dprint treats as foreign (and therefore preserves verbatim), then
/// restore it after formatting.
///
/// The sentinels are derived from `source` so they cannot already occur in it.
/// Restoration is then a plain replace that neither clobbers a document that
/// legitimately contains the sentinel text (e.g. docs describing this
/// mechanism) nor leaks a sentinel that dprint relocated while reflowing prose.
fn fence_sentinels(source: &str) -> (String, String) {
    let mut nonce = 0u32;
    loop {
        let markdown = format!("prim-guard-{nonce}-markdown");
        let md = format!("prim-guard-{nonce}-md");
        if !source.contains(&markdown) && !source.contains(&md) {
            return (markdown, md);
        }
        nonce += 1;
    }
}

/// Rewrite the language word of every fenced-code opening line tagged exactly
/// `markdown`/`md` to its sentinel. Lines are inspected structurally: optional
/// indentation and blockquote markers, a run of ≥ 3 backticks or tildes, then
/// the info string's first word.
fn guard_markdown_fences(source: &str, guard_markdown: &str, guard_md: &str) -> String {
    let swaps = [("markdown", guard_markdown), ("md", guard_md)];
    source
        .split_inclusive('\n')
        .map(|line| swap_fence_language_line(line, &swaps))
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

/// dprint-plugin-markdown forces a line break *before* an HTML node but has no
/// symmetric rule *after* one, so prose separated from a comment by a single
/// newline is joined onto the comment's closing line. Inside a list item that
/// changes rendering: CommonMark ends an HTML block at the line holding `-->`
/// and treats that whole line as raw HTML, so a code span or link landing after
/// the `-->` stops being parsed as Markdown (issue #97, FR-6.2).
///
/// Undo the join. Only comments that opened at the start of a line in `source`
/// *and* closed at the end of one there are re-broken: prim's job is to avoid
/// introducing the join, not to repair one the author wrote themselves. That
/// also makes the pass self-limiting — content prim preserves verbatim, such as
/// a fenced or indented code block, comes out of dprint unjoined and so is never
/// a candidate.
///
/// The continuation is not re-wrapped and can sit short of the configured width.
/// The result is still stable under repeated formatting (dprint re-joins it and
/// this pass re-splits it to the same text) and renders identically to the
/// input, which is the guarantee that matters.
fn unjoin_html_comments(formatted: &str, source: &str) -> String {
    let closings = line_terminated_comment_closings(source);
    if closings.is_empty() {
        return formatted.to_string();
    }

    let mut out = String::with_capacity(formatted.len());
    for line in formatted.split_inclusive('\n') {
        let body = line.trim_end_matches(['\n', '\r']);
        let line_ending = &line[body.len()..];
        match joined_comment_end(body, &closings) {
            Some(end) => {
                out.push_str(body[..end].trim_end());
                // The last line of a file may carry no ending of its own.
                out.push_str(if line_ending.is_empty() {
                    "\n"
                } else {
                    line_ending
                });
                out.push_str(&continuation_indent(body));
                out.push_str(body[end..].trim_start());
                out.push_str(line_ending);
            }
            None => out.push_str(line),
        }
    }
    out
}

/// The closing lines of every HTML comment in `source` that both opened at the
/// start of a line and ended one, trimmed of surrounding whitespace.
fn line_terminated_comment_closings(source: &str) -> BTreeSet<String> {
    let mut closings = BTreeSet::new();
    let mut is_open = false;

    for line in source.lines() {
        let search_from = if is_open {
            0
        } else {
            let content = container_prefix_len(line);
            if !line[content..].starts_with("<!--") {
                continue;
            }
            content + "<!--".len()
        };

        match line[search_from..].find("-->") {
            Some(offset) => {
                is_open = false;
                let end = search_from + offset + "-->".len();
                if line[end..].trim().is_empty() {
                    closings.insert(line[..end].trim().to_string());
                }
            }
            None => is_open = true,
        }
    }

    closings
}

/// Byte offset just past the `-->` of a comment that `closings` says ended a
/// line in the source but now has prose after it, or `None` if this line is
/// clean.
fn joined_comment_end(line: &str, closings: &BTreeSet<String>) -> Option<usize> {
    let mut search_from = 0;
    while let Some(offset) = line[search_from..].find("-->") {
        let end = search_from + offset + "-->".len();
        if !line[end..].trim().is_empty() && closings.contains(line[..end].trim()) {
            return Some(end);
        }
        search_from = end;
    }
    None
}

/// Indentation that keeps the re-broken text inside the same block container:
/// the line's own prefix with every marker character blanked out.
fn continuation_indent(line: &str) -> String {
    line[..container_prefix_len(line)]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect()
}

/// Byte length of a line's block-container prefix — indentation, blockquote
/// markers, and one list marker — after which the line's own content starts.
fn container_prefix_len(line: &str) -> usize {
    let bytes = line.as_bytes();
    let mut i = 0;
    let space_run = |bytes: &[u8], mut i: usize| {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        i
    };

    i = space_run(bytes, i);
    while i < bytes.len() && bytes[i] == b'>' {
        i = space_run(bytes, i + 1);
    }

    // One list marker: a bullet, or digits followed by `.`/`)`. A marker counts
    // only when whitespace separates it from the content.
    let marker_start = i;
    let mut after_marker = i;
    match bytes.get(after_marker) {
        Some(b'-' | b'*' | b'+') => after_marker += 1,
        Some(c) if c.is_ascii_digit() => {
            while after_marker < bytes.len() && bytes[after_marker].is_ascii_digit() {
                after_marker += 1;
            }
            match bytes.get(after_marker) {
                Some(b'.' | b')') => after_marker += 1,
                _ => after_marker = marker_start,
            }
        }
        _ => {}
    }
    if after_marker > marker_start {
        let content = space_run(bytes, after_marker);
        if content > after_marker {
            i = content;
        }
    }
    i
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
    fn whitespace_inside_a_word_does_not_panic() {
        // Issue #115: dprint-plugin-markdown's `is_list_word` asserts a word
        // holds no whitespace other than U+00A0, but words break on ASCII
        // space and line feed only, so any other whitespace character inside
        // a word after the first trips it. The assertion is disabled for the
        // package in the dev profile (see root Cargo.toml / AD-0006). U+00A0
        // is the assertion's own exemption, pinned here so that losing it
        // upstream is noticed.
        const WHITESPACE: [char; 22] = [
            '\u{0009}', '\u{000b}', '\u{000c}', '\u{0085}', '\u{00a0}', '\u{1680}', '\u{2000}',
            '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
            '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}',
            '\u{3000}',
        ];

        for character in WHITESPACE {
            let src = format!("alpha bra{character}vo charlie\n");
            let out = format(&src, &Style::default())
                .unwrap_or_else(|error| panic!("U+{:04X}: {error}", character as u32));
            assert_eq!(out, src, "U+{:04X} must round-trip", character as u32);
        }
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
        assert!(!out.contains("prim-guard-"), "{out:?}");
    }

    #[test]
    fn other_fence_tags_are_untouched_by_the_guard() {
        let src = "```js\nconst x=1\n```\n";
        assert_eq!(guard_markdown_fences(src, "GM", "GD"), src);
    }

    #[test]
    fn guard_handles_tilde_and_blockquote_fences() {
        assert_eq!(
            guard_markdown_fences("~~~markdown\n", "GM", "GD"),
            "~~~GM\n"
        );
        assert_eq!(guard_markdown_fences("> ```md\n", "GM", "GD"), "> ```GD\n");
        // Round-trip: guarding then restoring returns the original line.
        let guarded = guard_markdown_fences("> ```md\n", "GM", "GD");
        assert_eq!(guarded.replace("GD", "md"), "> ```md\n");
    }

    #[test]
    fn fence_sentinels_bump_the_nonce_past_any_collision() {
        assert_eq!(
            fence_sentinels("nothing here"),
            (
                "prim-guard-0-markdown".to_string(),
                "prim-guard-0-md".to_string()
            )
        );
        // A source already containing the nonce-0 sentinel forces a higher nonce.
        let (markdown, _) = fence_sentinels("```prim-guard-0-markdown\nx\n```\n");
        assert_ne!(markdown, "prim-guard-0-markdown");
    }

    #[test]
    fn preexisting_sentinel_like_tag_is_preserved() {
        // A document may legitimately contain a fence whose tag looks like a
        // guard sentinel (docs describing this mechanism, for instance). The
        // nonce-based sentinel must not clobber it. The heading forces dprint to
        // return `Ok(Some(..))`, so the restore step runs.
        let src = "#  Heading\n\n```prim-guard-0-markdown\nverbatim  content\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("```prim-guard-0-markdown\n"),
            "pre-existing sentinel-like tag must survive: {out:?}"
        );
        assert!(
            out.contains("verbatim  content"),
            "content verbatim: {out:?}"
        );
    }

    #[test]
    fn does_not_join_prose_onto_a_multi_line_comment_in_a_list_item() {
        let src = "- A list item whose prose runs on for a while before the note.\n  <!-- A correction note that spans\n  more than one line and ends here. -->\n  As-built `rest.rs` omits the two count fields.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("more than one line and ends here. -->\n"),
            "the closing line must stay free of prose: {out:?}"
        );
    }

    #[test]
    fn does_not_join_prose_onto_a_single_line_comment_in_a_list_item() {
        let src = "- A list item whose prose runs on for a while before the note.\n  <!-- A single line note. -->\n  As-built `rest.rs` omits the two count fields.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("  <!-- A single line note. -->\n  As-built `rest.rs`"),
            "the closing line must stay free of prose: {out:?}"
        );
    }

    #[test]
    fn does_not_join_prose_onto_a_comment_that_opens_a_list_item() {
        let src = "- <!-- A note that is the first thing in the item. -->\n  As-built `rest.rs` omits the two count fields and more prose here.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("- <!-- A note that is the first thing in the item. -->\n  As-built"),
            "the closing line must stay free of prose: {out:?}"
        );
    }

    #[test]
    fn does_not_join_prose_onto_a_comment_in_a_nested_list_item() {
        let src = "- outer\n  - A nested item whose prose runs on for a while before the note.\n    <!-- A single line note. -->\n    As-built `rest.rs` omits the two count fields.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("    <!-- A single line note. -->\n    As-built `rest.rs`"),
            "continuation keeps the nested indent: {out:?}"
        );
    }

    #[test]
    fn leaves_an_author_written_join_alone() {
        // The author put the prose after the `-->` themselves. That already
        // renders as raw HTML; re-breaking it would change the document just as
        // much as introducing the join does.
        let src = "- A list item.\n  <!-- A note. --> Prose the author put on the comment line.\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("<!-- A note. --> Prose the author put on the comment line."),
            "author's own line kept: {out:?}"
        );
    }

    #[test]
    fn comment_like_text_in_a_fenced_block_is_untouched() {
        let src = "- item\n\n  ```html\n  <!-- fenced --> trailing text\n  ```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("<!-- fenced --> trailing text"),
            "fenced content verbatim: {out:?}"
        );
    }

    #[test]
    fn is_idempotent_with_a_comment_in_a_list_item() {
        let src = "- A list item whose prose runs on for a while before the note.\n  <!-- A correction note that spans\n  more than one line and ends here. -->\n  As-built `rest.rs` omits the two count fields.\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
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
