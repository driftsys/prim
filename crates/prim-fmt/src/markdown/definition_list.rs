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

/// Replace every paragraph of `source` that holds a definition-list line with a
/// sentinel paragraph. Fenced code and indented code are never inspected.
pub(super) fn guard(source: &str) -> Guarded {
    let stem = sentinel_stem(source);
    let lines: Vec<&str> = source.split_inclusive('\n').collect();
    let mut text = String::with_capacity(source.len());
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    let mut open_fence: Option<(u8, usize)> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        let body = trim_line_ending(line);

        if let Some(fence) = fence_marker(body) {
            open_fence = match open_fence {
                None => Some(fence),
                Some((open_char, open_len)) if fence.0 == open_char && fence.1 >= open_len => None,
                keep => keep,
            };
        }
        if open_fence.is_some() || is_blank(body) || fence_marker(body).is_some() {
            text.push_str(line);
            index += 1;
            continue;
        }

        let end = paragraph_end(&lines, index);
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
                Some((start, paragraph)) => {
                    let prefix = &body[..start];
                    for (position, original) in paragraph.iter().enumerate() {
                        if position == 0 {
                            out.push_str(prefix);
                            out.push_str(&original[container_prefix_len(original)..]);
                        } else {
                            out.push_str(&rebase(original, &paragraph[0], prefix));
                        }
                        out.push_str(if ending.is_empty() { "\n" } else { ending });
                    }
                }
                None => out.push_str(line),
            }
        }
        out
    }

    /// The offset of the sentinel in `line` and the paragraph it stands for.
    fn sentinel_in(&self, line: &str) -> Option<(usize, &[String])> {
        let start = line.find(&self.stem)?;
        let digits = &line[start + self.stem.len()..];
        let number: usize = digits.parse().ok()?;
        Some((start, self.paragraphs.get(number)?))
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
    let indent: String = sentinel_prefix
        .chars()
        .map(|c| if c == '>' || c == '\t' { c } else { ' ' })
        .collect();
    format!("{indent}{}", &original[own_prefix_len..])
}

/// The index just past the last line of the paragraph that starts at `start`.
/// A paragraph ends at a blank line or at a line that opens a fenced block.
fn paragraph_end(lines: &[&str], start: usize) -> usize {
    let mut end = start + 1;
    while end < lines.len() {
        let body = trim_line_ending(lines[end]);
        if is_blank(body) || fence_marker(body).is_some() {
            break;
        }
        end += 1;
    }
    end
}

/// True when the paragraph has a term line followed by a `: ` definition line,
/// and does not start as indented code.
fn has_definition_line(paragraph: &[&str]) -> bool {
    let first = paragraph[0];
    if is_indented_code(first) || first.trim_start_matches([' ', '>']).starts_with(": ") {
        return false;
    }
    paragraph[1..]
        .iter()
        .any(|line| line.trim_start_matches([' ', '\t', '>']).starts_with(": "))
}

/// Four or more columns of indentation with no list or quote marker: an
/// indented code block, whose contents are not prose.
fn is_indented_code(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches([' ', '\t']).len();
    let starts_wide = line.starts_with('\t') || indent >= 4;
    starts_wide && container_prefix_len(line) == indent
}

/// The fence character and run length when `line` opens or closes a fenced
/// code block, after indentation and quote markers.
fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let content = line.trim_start_matches([' ', '\t', '>']);
    let fence_char = *content
        .as_bytes()
        .first()
        .filter(|c| matches!(c, b'`' | b'~'))?;
    let length = content.bytes().take_while(|&c| c == fence_char).count();
    (length >= 3).then_some((fence_char, length))
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
