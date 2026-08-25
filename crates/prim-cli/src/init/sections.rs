//! Low-level `.editorconfig` section parsing and canonical-order bound
//! checks that back `init`'s `merge` (story G4). Split out of `init.rs` to
//! keep it under its size limit; nothing here is meant to be used outside
//! `init`.

use std::collections::BTreeMap;

use super::MDLINT_STRICT_KEY;

pub(super) struct SectionSpec<'a> {
    pub(super) glob: &'a str,
    pub(super) value: bool,
    /// A path this section is meant to decide the tier of — one
    /// representative per canonical glob, used to check what prim's writes
    /// would actually resolve to (see [`super::outcome`]).
    pub(super) probe: String,
}

#[derive(Clone, Copy)]
pub(super) struct SectionOccurrence {
    pub(super) header_line: usize,
    pub(super) insert_at: usize,
    pub(super) has_key: bool,
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
pub(super) fn governing(occurrences: &[SectionOccurrence]) -> Option<SectionOccurrence> {
    occurrences
        .iter()
        .rev()
        .find(|occurrence| occurrence.has_key)
        .or_else(|| occurrences.last())
        .copied()
}

/// A warning for each pair of specs whose sections both already carry
/// `prim_mdlint_strict` but appear in the wrong relative order — the
/// canonically earlier spec's section ends after the canonically later one's
/// starts. prim writes nothing into either in that case (both already have
/// their key), so without this the run would report plain success over a file
/// that resolves the wrong way.
///
/// Every pair is compared, not only canonically adjacent ones: a canonical
/// section that sits between two of them without carrying the key takes no
/// part in the order, and must not hide the pair it separates.
///
/// Only occurrences that carry the key are compared: an occurrence prim would
/// write into is judged by [`super::outcome`] instead, which reports the
/// resolution it would produce rather than its position. `line_of` maps a
/// 0-indexed line of the file prim read to the 1-indexed line it will have
/// once prim's own writes land.
pub(super) fn existing_order_warnings(
    specs: &[SectionSpec<'_>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
    line_of: &dyn Fn(usize) -> usize,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for index in 0..specs.len() {
        let Some(earlier) = keyed(&occurrences_by_spec[index]).next_back() else {
            continue;
        };
        for (offset, later_occurrences) in occurrences_by_spec[index + 1..].iter().enumerate() {
            let Some(later) = keyed(later_occurrences).next() else {
                continue;
            };
            if earlier.insert_at > later.header_line {
                warnings.push(format!(
                    "[{}] (line {}) comes after [{}] (line {}) in this .editorconfig, which \
                     contradicts prim's canonical section order; prim will not reorder sections a \
                     person wrote, so reorder them yourself",
                    specs[index].glob,
                    line_of(earlier.header_line),
                    specs[index + 1 + offset].glob,
                    line_of(later.header_line),
                ));
            }
        }
    }
    warnings
}

fn keyed(
    occurrences: &[SectionOccurrence],
) -> impl DoubleEndedIterator<Item = SectionOccurrence> + '_ {
    occurrences
        .iter()
        .filter(|occurrence| occurrence.has_key)
        .copied()
}

/// The latest point any already-present, canonically earlier spec's governing
/// section ends at, if one exists in `existing` — every section that must
/// precede the spec at `index`.
pub(super) fn lower_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
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
    occurrences_by_spec: &[Vec<SectionOccurrence>],
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
        .filter_map(|(index, line)| parse_header(line).map(|glob| (index, glob.to_string())))
        .collect()
}

pub(super) fn has_top_level_root(lines: &[&str], headers: &[(usize, String)]) -> bool {
    let first_section = headers.first().map_or(lines.len(), |(index, _)| *index);
    lines
        .iter()
        .take(first_section)
        .filter_map(|line| parse_key(line))
        .any(|key| key.eq_ignore_ascii_case("root"))
}

pub(super) fn matching_sections(
    lines: &[&str],
    headers: &[(usize, String)],
    glob: &str,
) -> Vec<SectionOccurrence> {
    headers
        .iter()
        .enumerate()
        .filter(|(_, (_, header_glob))| header_glob == glob)
        .map(|(header_pos, (line_index, _))| {
            let next_header = headers
                .get(header_pos + 1)
                .map_or(lines.len(), |(next_index, _)| *next_index);
            let has_key = lines[*line_index + 1..next_header]
                .iter()
                .filter_map(|line| parse_key(line))
                .any(|key| key.eq_ignore_ascii_case(MDLINT_STRICT_KEY));
            SectionOccurrence {
                header_line: *line_index,
                insert_at: next_header,
                has_key,
            }
        })
        .collect()
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

fn parse_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

fn parse_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
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
