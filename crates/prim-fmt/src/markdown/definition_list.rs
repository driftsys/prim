//! Definition-list guard for the Markdown pass (FR-1.1a, issue #229).
//!
//! CommonMark has no definition lists, so `dprint-plugin-markdown` sees
//!
//! ```text
//! Term
//! : first line
//! second line.
//! ```
//!
//! as one paragraph and rewraps it to `Term : first line second line.`, which
//! drops the `:` marker for every tool that does read definition lists. Guard:
//! swap each such paragraph for a one-word sentinel paragraph before dprint
//! runs, then put the original lines back afterwards. The text is preserved
//! as written, not rewrapped, which keeps the result stable under repeated
//! formatting.

use super::container_prefix_len;

/// A guarded source plus the paragraphs it replaced.
pub(super) struct Guarded {
    /// The source with each definition-list paragraph replaced by a sentinel.
    pub(super) text: String,
    stem: String,
    /// The original lines of each guarded paragraph, without line endings.
    paragraphs: Vec<Vec<String>>,
}

/// Where a line sits: the open fenced block, and the content column of each
/// open list item. A line indented four columns past the innermost item is an
/// indented code block.
#[derive(Default)]
struct Context {
    open_fence: Option<(u8, usize)>,
    columns: Vec<usize>,
}

impl Context {
    fn base(&self) -> usize {
        self.columns.last().copied().unwrap_or(0)
    }

    fn is_code(&self, line: &str) -> bool {
        indent(line) >= self.base() + 4
    }

    /// Update the list-item columns for a non-blank line outside a fence.
    fn enter(&mut self, line: &str) {
        let column = marker_column(line).unwrap_or_else(|| indent(line));
        while self.columns.last().is_some_and(|&top| top > column) {
            self.columns.pop();
        }
        if !self.is_code(line) && marker_column(line).is_some() {
            self.columns.push(container_prefix_len(line));
        }
    }
}

/// Replace every paragraph of `source` that holds a definition-list line with a
/// sentinel paragraph. Fenced code and indented code are never inspected.
pub(super) fn guard(source: &str) -> Guarded {
    let stem = sentinel_stem(source);
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut text = String::with_capacity(source.len());
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    let mut context = Context::default();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let body = trim_line_ending(line);

        if let Some((fence_char, open_len)) = context.open_fence {
            if closes_fence(body, fence_char, open_len) {
                context.open_fence = None;
            }
            text.push_str(line);
            index += 1;
            continue;
        }
        if is_blank(body) {
            text.push_str(line);
            index += 1;
            continue;
        }

        context.enter(body);
        if context.is_code(body) {
            // An indented code block is not prose; copy its lines through.
            text.push_str(line);
            index += 1;
            continue;
        }
        if let Some(fence) = opens_fence(body) {
            context.open_fence = Some(fence);
            text.push_str(line);
            index += 1;
            continue;
        }

        let end = paragraph_end(&lines, index, context.base());
        let paragraph: Vec<&str> = lines[index..end]
            .iter()
            .map(|line| trim_line_ending(line))
            .collect();
        if has_definition_line(&paragraph) {
            let prefix = &paragraph[0][..container_prefix_len(paragraph[0])];
            let ending = &lines[end - 1][paragraph[paragraph.len() - 1].len()..];
            text.push_str(prefix);
            text.push_str(&format!("{stem}{}", paragraphs.len()));
            text.push_str(if ending.is_empty() { "\n" } else { ending });
            paragraphs.push(paragraph.iter().map(|line| line.to_string()).collect());
        } else {
            for line in &lines[index..end] {
                text.push_str(line);
            }
        }
        index = end;
    }

    Guarded {
        text,
        stem,
        paragraphs,
    }
}

impl Guarded {
    /// Put the original lines back in place of each sentinel in `formatted`.
    pub(super) fn restore(&self, formatted: &str) -> String {
        if self.paragraphs.is_empty() {
            return formatted.to_string();
        }
        let mut out = String::with_capacity(formatted.len());
        for line in formatted.split_inclusive('\n') {
            let body = trim_line_ending(line);
            let ending = &line[body.len()..];
            match self.sentinel_in(body) {
                Some((start, paragraph, joined)) => {
                    let prefix = &body[..start];
                    let last = paragraph.len() - 1;
                    for (position, original) in paragraph.iter().enumerate() {
                        let restored = if position == 0 {
                            format!("{prefix}{}", &original[container_prefix_len(original)..])
                        } else {
                            rebase(original, &paragraph[0], prefix)
                        };
                        out.push_str(&hard_break_to_backslash(restored, position < last));
                        out.push_str(if ending.is_empty() { "\n" } else { ending });
                    }
                    // Text a formatter joined onto the sentinel line goes on a
                    // line of its own, so no text is ever lost.
                    if !joined.trim().is_empty() {
                        out.push_str(&rebase_indent(prefix));
                        out.push_str(joined.trim());
                        out.push_str(if ending.is_empty() { "\n" } else { ending });
                    }
                }
                None => out.push_str(line),
            }
        }
        out
    }

    /// The offset of the sentinel in `line` and the paragraph it stands for.
    fn sentinel_in<'a>(&'a self, line: &'a str) -> Option<(usize, &'a [String], &'a str)> {
        let start = line.find(&self.stem)?;
        let after = &line[start + self.stem.len()..];
        let digits_len = after.bytes().take_while(u8::is_ascii_digit).count();
        let number: usize = after[..digits_len].parse().ok()?;
        Some((start, self.paragraphs.get(number)?, &after[digits_len..]))
    }
}

/// Spell a two-space hard break as a trailing backslash, as dprint does for
/// prose. Whitespace hygiene strips trailing spaces after this pass, so a break
/// written with spaces would otherwise be lost.
fn hard_break_to_backslash(line: String, is_inside_paragraph: bool) -> String {
    if is_inside_paragraph && line.ends_with("  ") && !line.trim().is_empty() {
        format!("{}\\", line.trim_end())
    } else {
        line
    }
}

/// Re-indent a continuation line of a guarded paragraph. The first line's
/// container prefix in the source is replaced by the prefix dprint gave the
/// sentinel line: quote markers stay, every other marker becomes a space.
fn rebase(original: &str, first_original: &str, sentinel_prefix: &str) -> String {
    let source_prefix_len = container_prefix_len(first_original);
    let own_prefix_len = original
        .char_indices()
        .take(source_prefix_len)
        .take_while(|(_, c)| matches!(c, ' ' | '\t' | '>'))
        .last()
        .map_or(0, |(offset, c)| offset + c.len_utf8());
    format!(
        "{}{}",
        rebase_indent(sentinel_prefix),
        &original[own_prefix_len..]
    )
}

/// A continuation prefix for `sentinel_prefix`: quote markers stay, every other
/// marker becomes a space.
fn rebase_indent(sentinel_prefix: &str) -> String {
    sentinel_prefix
        .chars()
        .map(|c| if c == '>' || c == '\t' { c } else { ' ' })
        .collect()
}

/// The index just past the last line of the paragraph that starts at `start`.
/// A paragraph ends at a blank line, a deeper quote (a line with fewer quote
/// markers is a lazy continuation and stays), a setext
/// underline (which is its last line), or a line that starts another block: a
/// fence, an ATX heading, a thematic break or a list item. A first line that is
/// itself a heading or a thematic break is a paragraph of one line.
fn paragraph_end(lines: &[&str], start: usize, base: usize) -> usize {
    let first = trim_line_ending(lines[start]);
    if is_heading_or_break(first) {
        return start + 1;
    }
    let depth = quote_depth(first);
    let content_column = container_prefix_len(first);
    let mut end = start + 1;
    while end < lines.len() {
        let body = trim_line_ending(lines[end]);
        if indent(body) < base + 4 && quote_depth(body) <= depth && is_setext_underline(body) {
            return end + 1;
        }
        // An indented line continues the paragraph, whatever it looks like.
        let is_code = indent(body) >= base + 4;
        if is_blank(body)
            || quote_depth(body) > depth
            || (!is_code
                && (opens_fence(body).is_some()
                    || is_heading_or_break(body)
                    || interrupts_with_list_marker(body, content_column)))
        {
            break;
        }
        end += 1;
    }
    end
}

/// True when the paragraph has a term line followed by a `: ` definition line.
fn has_definition_line(paragraph: &[&str]) -> bool {
    let first = paragraph[0];
    // A setext heading is a heading, not a term and its definitions.
    if first.trim_start_matches([' ', '>']).starts_with(": ")
        || is_setext_underline(paragraph[paragraph.len() - 1])
    {
        return false;
    }
    paragraph[1..]
        .iter()
        .any(|line| line.trim_start_matches([' ', '\t', '>']).starts_with(": "))
}

/// Columns of leading whitespace, a tab counting as four.
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn quote_depth(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t' | '>'))
        .filter(|&c| c == '>')
        .count()
}

/// The byte column of a list marker, when the line holds one after any
/// indentation and quote markers.
fn marker_column(line: &str) -> Option<usize> {
    let before_marker = line.len() - line.trim_start_matches([' ', '\t', '>']).len();
    (container_prefix_len(line) > before_marker).then_some(before_marker)
}

/// A list marker that ends a paragraph whose content starts at `content_column`:
/// a marker left of that column belongs to a sibling or an enclosing item, so it
/// ends the paragraph whatever its number. Further right, the marker starts a
/// list inside the paragraph only if it is a bullet or an ordered marker
/// numbered 1 (CommonMark).
fn interrupts_with_list_marker(line: &str, content_column: usize) -> bool {
    let Some(column) = marker_column(line) else {
        return false;
    };
    let marker = line[column..container_prefix_len(line)].trim_end();
    column < content_column
        || match marker.strip_suffix(['.', ')']) {
            Some(number) => number.parse::<u64>() == Ok(1),
            None => true,
        }
}

/// A line of only `=` or only `-`: the underline that turns the paragraph above
/// it into a setext heading.
fn is_setext_underline(line: &str) -> bool {
    let content = line.trim_start_matches([' ', '\t', '>']).trim_end();
    !content.is_empty()
        && (content.bytes().all(|c| c == b'=') || content.bytes().all(|c| c == b'-'))
}

fn is_heading_or_break(line: &str) -> bool {
    let content = line.trim_start_matches([' ', '\t', '>']);
    let hashes = content.chars().take_while(|&c| c == '#').count();
    let is_heading = (1..=6).contains(&hashes)
        && content[hashes..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
    let marks: Vec<char> = content.chars().filter(|c| !c.is_whitespace()).collect();
    let is_break = marks.len() >= 3
        && matches!(marks[0], '-' | '*' | '_')
        && marks.iter().all(|&c| c == marks[0]);
    is_heading || is_break
}

/// The fence character and run length when `line` opens a fenced code block,
/// after indentation, quote markers and one list marker. A backtick fence
/// cannot carry a backtick in its info string.
fn opens_fence(line: &str) -> Option<(u8, usize)> {
    let (fence_char, length, info) = fence_parts(line)?;
    (fence_char != b'`' || !info.contains('`')).then_some((fence_char, length))
}

/// True when `line` closes a fence opened with `open_char` x `open_len`: the
/// same character, at least as long, and nothing after the run.
fn closes_fence(line: &str, open_char: u8, open_len: usize) -> bool {
    fence_parts(line).is_some_and(|(c, length, info)| {
        c == open_char && length >= open_len && info.trim().is_empty()
    })
}

/// The fence character, the run length and the text after the run.
fn fence_parts(line: &str) -> Option<(u8, usize, &str)> {
    let content = &line[container_prefix_len(line)..];
    let fence_char = *content
        .as_bytes()
        .first()
        .filter(|c| matches!(c, b'`' | b'~'))?;
    let length = content.bytes().take_while(|&c| c == fence_char).count();
    (length >= 3).then(|| (fence_char, length, &content[length..]))
}

fn is_blank(line: &str) -> bool {
    line.trim_matches([' ', '\t', '>']).is_empty()
}

fn trim_line_ending(line: &str) -> &str {
    line.trim_end_matches(['\n', '\r'])
}

/// A sentinel prefix that does not occur in `source`, so restoration cannot
/// replace text the author wrote.
fn sentinel_stem(source: &str) -> String {
    let mut nonce = 0u32;
    loop {
        let stem = format!("prim-guard-{nonce}-deflist-");
        if !source.contains(&stem) {
            return stem;
        }
        nonce += 1;
    }
}

#[cfg(test)]
mod tests {
    use crate::markdown::format;
    use crate::{LineEnding, Style};

    /// Format `src`, and check that a second run changes nothing.
    fn fmt(src: &str) -> String {
        let once = format(src, &Style::default()).unwrap();
        assert_eq!(
            format(&once, &Style::default()).unwrap(),
            once,
            "not idempotent"
        );
        once
    }

    /// The input is not canonical elsewhere, so dprint rewrites the document
    /// and the restore step has to run.
    fn forcing_restore(src: &str) -> String {
        format!("{src}\nx  y\n")
    }

    fn assert_kept(src: &str) {
        let out = fmt(&forcing_restore(src));
        assert_eq!(out, format!("{src}\nx y\n"), "definition list changed");
        assert_eq!(fmt(&out), out, "not idempotent");
    }

    #[test]
    fn keeps_a_definition_list_on_separate_lines() {
        assert_kept("Term\n: first line\nsecond line.\n");
    }

    #[test]
    fn keeps_several_definitions_and_a_long_definition_as_written() {
        assert_kept(
            "Term two\n: a definition that is quite long and goes on and on past the eighty column wrap width\n: second definition\n",
        );
    }

    #[test]
    fn restores_two_guarded_paragraphs_in_order() {
        assert_kept("A\n: one\n\nB\n: two\n");
    }

    #[test]
    fn keeps_a_definition_list_inside_a_list_item() {
        assert_kept("- [SRS_X_0001] Title\n\n  Term\n  : first line\n  second line.\n");
    }

    #[test]
    fn re_indents_a_definition_list_when_dprint_renormalises_the_marker() {
        let out = fmt("-   Term\n    : first\n    second\n");
        assert_eq!(out, "- Term\n  : first\n  second\n");
    }

    #[test]
    fn keeps_a_definition_list_inside_a_blockquote() {
        assert_kept("> Term\n> : first line\n> second line.\n");
    }

    #[test]
    fn keeps_a_definition_list_indented_four_columns_inside_a_list_item() {
        // dprint re-indents the item's content to two columns; the definition
        // keeps its lines and is not rewrapped.
        let out = fmt("- item\n\n    Term\n    : first\n    second line   is   here\n");
        assert_eq!(
            out,
            "- item\n\n  Term\n  : first\n  second line   is   here\n"
        );
    }

    #[test]
    fn still_formats_the_text_around_a_definition_list() {
        let out = fmt("#   Title\n\nTerm\n: first line\n\nA  paragraph   with   odd spacing.\n");
        assert_eq!(
            out,
            "# Title\n\nTerm\n: first line\n\nA paragraph with odd spacing.\n"
        );
    }

    #[test]
    fn a_sibling_list_item_stays_a_sibling() {
        let out = fmt("- a\n- Term\n  : def\n- b\n\nx  y\n");
        assert_eq!(out, "- a\n- Term\n  : def\n- b\n\nx y\n");
    }

    #[test]
    fn a_lazy_continuation_line_stays_in_the_blockquote_definition() {
        let out = fmt("> Term\n> : def\nlazy   line\n");
        assert!(!out.contains("prim-guard"), "{out:?}");
        assert_eq!(out, "> Term\n> : def\n> lazy   line\n");
        let out = fmt("> > Term\n> > : def\n> lazy\n");
        assert!(!out.contains("prim-guard"), "{out:?}");
        assert_eq!(out, ">> Term\n>> : def\n>> lazy\n");
    }

    #[test]
    fn a_thematic_break_after_a_definition_ends_it() {
        let out = fmt("Term\n: def\n***\n");
        assert_eq!(out, "Term\n: def\n\n---\n");
    }

    #[test]
    fn an_indented_line_continues_the_definition() {
        assert_kept("Term\n: def\n    # not   heading\n");
    }

    #[test]
    fn only_an_ordered_item_numbered_one_interrupts_a_definition() {
        assert_kept("Term\n: def\n2. x   y\n");
        let out = fmt("Term\n: def\n1. x   y\n");
        assert_eq!(out, "Term\n: def\n\n1. x y\n");
    }

    #[test]
    fn indented_code_after_a_list_is_not_a_fence() {
        assert_kept("- item\n\npara\n\n    ```\n    code\n\nTerm\n: d   e\n");
    }

    #[test]
    fn a_closing_fence_carries_no_info_string() {
        // dprint lengthens the outer fence; the definition is what matters.
        let out = fmt("```\n```text\n```\n\nTerm\n: d   e\n");
        assert!(out.ends_with("Term\n: d   e\n"), "{out:?}");
    }

    #[test]
    fn a_backtick_fence_has_no_backtick_in_its_info_string() {
        assert_kept("``` a`b\nTerm\n: d   e\n");
    }

    #[test]
    fn trailing_spaces_on_the_last_definition_line_are_dropped() {
        assert_eq!(fmt("Term\n: a\nb  \n"), "Term\n: a\nb\n");
    }

    #[test]
    fn a_heading_directly_before_a_term_is_formatted() {
        assert_eq!(fmt("#   H\nTerm\n: d\n"), "# H\n\nTerm\n: d\n");
    }

    #[test]
    fn a_fence_directly_after_a_definition_is_separate() {
        assert_eq!(
            fmt("Term\n: def\n```\ncode\n```\n"),
            "Term\n: def\n\n```\ncode\n```\n"
        );
    }

    #[test]
    fn text_joined_onto_a_sentinel_is_kept_on_its_own_line() {
        let guarded = super::guard("Term\n: d\n");
        let joined = format!(
            "{}0 trailing\n",
            guarded.text.trim_end().trim_end_matches('0')
        );
        assert_eq!(guarded.restore(&joined), "Term\n: d\ntrailing\n");
    }

    #[test]
    fn a_sibling_ordered_item_stays_a_sibling() {
        let src = "1. x\n2. Term\n   : d\n3. y\n\nx  y\n";
        assert_eq!(fmt(src), "1. x\n2. Term\n   : d\n3. y\n\nx y\n");
    }

    #[test]
    fn a_setext_underline_makes_a_heading_not_a_definition_list() {
        let out = fmt("Term\n: x\n---\n");
        assert!(out.starts_with("## Term"), "{out:?}");
        assert!(!out.contains("prim-guard"), "{out:?}");
        assert_eq!(fmt(&out), out);
    }

    #[test]
    fn a_heading_after_a_definition_is_still_formatted() {
        let out = fmt("Term\n: def\n#   Heading\n");
        assert!(out.contains("# Heading\n"), "{out:?}");
        assert!(out.starts_with("Term\n: def\n"), "{out:?}");
    }

    #[test]
    fn a_blockquote_after_a_definition_is_still_formatted() {
        let out = fmt("Term\n: def\n> a   b\n");
        assert!(out.starts_with("Term\n: def\n"), "{out:?}");
        assert!(out.contains("> a b\n"), "{out:?}");
    }

    #[test]
    fn a_colon_line_after_a_blank_line_is_wrapped_as_prose() {
        let out = fmt("Term\n\n: a   b\n: c   d\n");
        assert_eq!(out, "Term\n\n: a b : c d\n");
    }

    #[test]
    fn a_colon_line_in_a_fenced_block_is_left_to_the_fence_rule() {
        let src = "```text\nTerm\n: not a definition\n```\n";
        assert_eq!(fmt(src), src);
        let src = "- ```text\n  Term\n  : definition\n  ```\n";
        assert_eq!(fmt(src), src);
    }

    #[test]
    fn a_fence_inside_indented_code_does_not_switch_the_guard_off() {
        assert_kept("para\n\n    ```\n    code\n\nTerm\n: d   e\n");
    }

    #[test]
    fn keeps_a_two_space_hard_break_inside_a_definition() {
        let out = fmt("Term\n: a  \nb\n");
        assert_eq!(out, "Term\n: a\\\nb\n");
    }

    #[test]
    fn keeps_crlf_endings_around_a_definition_list() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        let src = "Term\r\n: first line\r\nsecond line.\r\n";
        assert_eq!(format(src, &style).unwrap(), src);
    }

    #[test]
    fn a_sentinel_like_text_in_the_source_is_not_replaced() {
        let src = "Term\n: d\n\nprim-guard-0-deflist-0\n\nx  y\n";
        assert_eq!(fmt(src), "Term\n: d\n\nprim-guard-0-deflist-0\n\nx y\n");
    }
}
