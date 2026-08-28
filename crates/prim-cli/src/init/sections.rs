//! Low-level `.editorconfig` section parsing and canonical-order bound
//! checks that back `init`'s `merge` (story G4). Split out of `init.rs` to
//! keep it under its size limit; nothing here is meant to be used outside
//! `init`.

use std::collections::BTreeMap;
use std::path::Path;

use crate::editorconfig::line;

use super::MDLINT_STRICT_KEY;

pub(super) struct SectionSpec<'a> {
    pub(super) glob: &'a str,
    pub(super) value: bool,
    /// Paths this section is meant to decide the tier of — the
    /// representatives prim checks its work against (see [`super::outcome`]).
    /// Usually one; `[**/SUMMARY.md]` needs two, because a summary under
    /// `docs/wip/` is decided by a different set of sections from one under
    /// the strict glob.
    pub(super) probes: Vec<String>,
    /// A second representative, in a place a narrower override of `probes`
    /// would not reach, used only to tell such an override (which still lets
    /// this section decide everything else) apart from a later section that
    /// genuinely decides nothing this section is for (see
    /// [`super::outcome::defeated_sections`]). Never named in a message.
    /// Every canonical section carries one.
    pub(super) witness: Option<String>,
}

#[derive(Clone, Copy)]
pub(super) struct SectionOccurrence<'a> {
    pub(super) header_line: usize,
    pub(super) insert_at: usize,
    /// The text written after `prim_mdlint_strict =` in this section, if the
    /// key is there at all. Kept verbatim rather than parsed: a value that is
    /// neither `true` nor `false` resolves to the floor tier silently, and
    /// reporting that needs the text the author actually wrote.
    pub(super) key_value: Option<&'a str>,
}

impl SectionOccurrence<'_> {
    pub(super) fn has_key(&self) -> bool {
        self.key_value.is_some()
    }
}

/// How prim reads a written `prim_mdlint_strict`. Mirrors
/// [`crate::editorconfig::prim_bool_from`]: anything but `true` is `false`, so
/// a typo never errors, it just resolves to the floor tier.
pub(super) fn reads_as_strict(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

/// Whether a written value is one prim recognises at all. A value outside
/// `true`/`false` still resolves (to `false`), so this separates "the author
/// chose the floor tier" from "the author wrote something prim cannot read".
pub(super) fn is_boolean(value: &str) -> bool {
    value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false")
}

/// One end of the range a missing section's insertion point must fall in,
/// established by an existing occurrence of some other canonical spec.
/// `header_line` is a 0-indexed line in the file prim read — a warning has to
/// map it through the writes prim is making before showing it.
#[derive(Clone, Copy)]
pub(super) struct Bound<'a> {
    pub(super) position: usize,
    pub(super) glob: &'a str,
    pub(super) header_line: usize,
}

/// The occurrence of a canonical glob that takes part in prim's placement
/// map, out of every occurrence of that glob in the file.
///
/// An occurrence participates only if it carries `prim_mdlint_strict` — the
/// last such occurrence is the one EditorConfig's last-match-wins resolution
/// reads, and the one prim leaves untouched — or, when no occurrence carries
/// the key, the last occurrence, because that is the one prim would write the
/// key into. Any other occurrence (an ordinary `[*.md] max_line_length = 80`
/// somebody appended) sets nothing prim resolves and must not constrain
/// prim's ordering.
pub(super) fn governing<'a>(
    occurrences: &[SectionOccurrence<'a>],
) -> Option<SectionOccurrence<'a>> {
    occurrences
        .iter()
        .rev()
        .find(|occurrence| occurrence.has_key())
        .or_else(|| occurrences.last())
        .copied()
}

/// The latest point any already-present, canonically earlier spec's governing
/// section ends at, if one exists in `existing` — every section that must
/// precede the spec at `index`.
pub(super) fn lower_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence<'_>>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[..index]
        .iter()
        .zip(&occurrences_by_spec[..index])
        .filter_map(|(spec, occurrences)| {
            governing(occurrences).map(|occurrence| Bound {
                position: occurrence.insert_at,
                glob: spec.glob,
                header_line: occurrence.header_line,
            })
        })
        .max_by_key(|bound| bound.position)
}

/// The earliest point any already-present, canonically later spec's governing
/// section starts at, if one exists in `existing` — every section that must
/// follow the spec at `index`.
pub(super) fn upper_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence<'_>>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[index + 1..]
        .iter()
        .zip(&occurrences_by_spec[index + 1..])
        .filter_map(|(spec, occurrences)| {
            governing(occurrences).map(|occurrence| Bound {
                position: occurrence.header_line,
                glob: spec.glob,
                header_line: occurrence.header_line,
            })
        })
        .min_by_key(|bound| bound.position)
}

pub(super) fn split_lines(content: &str) -> Vec<&str> {
    if content.is_empty() {
        Vec::new()
    } else {
        content.split_inclusive('\n').collect()
    }
}

pub(super) fn header_lines(lines: &[&str]) -> Vec<(usize, String)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| parse_header(line, index).map(|glob| (index, glob.to_string())))
        .collect()
}

pub(super) fn has_top_level_root(lines: &[&str], headers: &[(usize, String)]) -> bool {
    let first_section = headers.first().map_or(lines.len(), |(index, _)| *index);
    lines
        .iter()
        .enumerate()
        .take(first_section)
        .filter_map(|(index, line)| parse_key(line, index))
        .any(|key| key.eq_ignore_ascii_case("root"))
}

pub(super) fn matching_sections<'a>(
    lines: &[&'a str],
    headers: &[(usize, String)],
    glob: &str,
) -> Vec<SectionOccurrence<'a>> {
    headers
        .iter()
        .enumerate()
        .filter(|(_, (_, header_glob))| header_glob == glob)
        .map(|(header_pos, (line_index, _))| {
            let next_header = next_header_line(lines, headers, header_pos);
            SectionOccurrence {
                header_line: *line_index,
                insert_at: next_header,
                key_value: strict_value_in(&lines[*line_index + 1..next_header]),
            }
        })
        .collect()
}

/// The section that decides `path`'s `prim_mdlint_strict` in this file: the
/// last one whose glob applies to `path` and that sets the key at all. Glob
/// matching goes through `ec4rs`, the same matcher that resolves the file, so
/// a section written with any EditorConfig glob syntax is accounted for —
/// not only the ones prim writes itself.
pub(super) fn deciding_section<'a>(
    lines: &[&str],
    headers: &'a [(usize, String)],
    path: &str,
) -> Option<&'a (usize, String)> {
    headers
        .iter()
        .enumerate()
        .filter(|(header_pos, (line_index, glob))| {
            let next_header = next_header_line(lines, headers, *header_pos);
            strict_value_in(&lines[*line_index + 1..next_header]).is_some()
                && ec4rs::Section::new(glob).applies_to(Path::new(path))
        })
        .map(|(_, header)| header)
        .next_back()
}

/// Whether the section whose occurrence sits at `occurrence_line` is the one
/// that decides `path`, or `None` when `glob` — that section's own — does not
/// even apply to `path`. The `None` case guards a probe prim itself failed to
/// construct, such as a `book.toml` `src` containing glob syntax (`docs[1]`):
/// a path a section's own glob does not match is not evidence about that
/// section, and must be left out rather than counted against it.
pub(super) fn owns(
    lines: &[&str],
    headers: &[(usize, String)],
    occurrence_line: usize,
    glob: &str,
    path: &str,
) -> Option<bool> {
    if !ec4rs::Section::new(glob).applies_to(Path::new(path)) {
        return None;
    }
    Some(deciding_section(lines, headers, path).is_some_and(|(line, _)| *line == occurrence_line))
}

/// The text written after `prim_mdlint_strict =` in a section's body, taking
/// the last one when a section repeats the key — that is the one `ec4rs`
/// keeps.
fn strict_value_in<'a>(body: &[&'a str]) -> Option<&'a str> {
    body.iter()
        .filter_map(|line| parse_pair(line))
        .filter(|(key, _)| key.eq_ignore_ascii_case(MDLINT_STRICT_KEY))
        .map(|(_, value)| value)
        .next_back()
}

fn next_header_line(lines: &[&str], headers: &[(usize, String)], header_pos: usize) -> usize {
    headers
        .get(header_pos + 1)
        .map_or(lines.len(), |(next_index, _)| *next_index)
}

pub(super) fn push_insert(
    inserts: &mut BTreeMap<usize, Vec<String>>,
    index: usize,
    mut addition: String,
    existing: &str,
    lines: &[&str],
) {
    let entry = inserts.entry(index).or_default();
    if index == lines.len() && !existing.is_empty() && !existing.ends_with('\n') && entry.is_empty()
    {
        addition.insert(0, '\n');
    }
    entry.push(addition);
}

fn parse_header(text: &str, index: usize) -> Option<&str> {
    match line::parse_at(text, index) {
        line::Line::Section(glob) => Some(glob),
        _ => None,
    }
}

fn parse_key(text: &str, index: usize) -> Option<&str> {
    match line::parse_at(text, index) {
        line::Line::Pair(key, _) => Some(key),
        _ => None,
    }
}

/// Reads a `key = value` line from a *section body*, which is never the
/// file's first physical line — a header always precedes it — so this needs
/// no BOM handling and takes no line index.
fn parse_pair(text: &str) -> Option<(&str, &str)> {
    match line::parse(text) {
        line::Line::Pair(key, value) => Some((key, value)),
        _ => None,
    }
}

pub(super) fn section_block(glob: &str, value: bool) -> String {
    format!("[{glob}]\n{}", key_line(value))
}

pub(super) fn key_line(value: bool) -> String {
    format!("{MDLINT_STRICT_KEY} = {}\n", bool_word(value))
}

pub(super) fn bool_word(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
