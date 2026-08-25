//! Low-level `.editorconfig` section parsing and canonical-order bound
//! checks that back `init`'s `merge` (story G4). Split out of `init.rs` to
//! keep it under its size limit; nothing here is meant to be used outside
//! `init`.

use std::collections::BTreeMap;

use super::MDLINT_STRICT_KEY;

pub(super) struct SectionSpec<'a> {
    pub(super) glob: &'a str,
    pub(super) value: bool,
}

#[derive(Clone, Copy)]
pub(super) struct SectionOccurrence {
    pub(super) header_line: usize,
    pub(super) insert_at: usize,
    pub(super) has_key: bool,
}

/// One end of the range a missing section's insertion point must fall in,
/// established by an existing occurrence of some other canonical spec.
/// `line` is 1-indexed, for warning text.
pub(super) struct Bound<'a> {
    pub(super) position: usize,
    pub(super) glob: &'a str,
    pub(super) line: usize,
}

/// Warn about every canonically-adjacent pair of specs that are BOTH already
/// present in `existing` but appear in the wrong relative order — the
/// canonically earlier spec's section starts at or after the canonically
/// later one's. This is checked only between adjacent specs, not every pair:
/// a spec that is itself missing defers to the insertion-time bound check in
/// `merge`'s main loop, which already spans the gap around it, so this
/// function only has to notice sections that already exist and therefore
/// never go through that loop's insertion path at all.
pub(super) fn existing_order_conflicts(
    specs: &[SectionSpec<'_>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
) -> Vec<String> {
    let mut warnings = Vec::new();
    for index in 0..specs.len().saturating_sub(1) {
        let Some(earlier) = occurrences_by_spec[index].last() else {
            continue;
        };
        let Some(later) = occurrences_by_spec[index + 1].first() else {
            continue;
        };
        if earlier.insert_at > later.header_line {
            warnings.push(format!(
                "[{}] (line {}) comes after [{}] (line {}) in this .editorconfig, which \
                 contradicts prim's canonical section order; prim will not reorder sections a \
                 person wrote, so reorder them yourself",
                specs[index].glob,
                earlier.header_line + 1,
                specs[index + 1].glob,
                later.header_line + 1,
            ));
        }
    }
    warnings
}

/// The latest point any already-present, canonically earlier spec's section
/// ends at, if one exists in `existing` — every section that must precede the
/// spec at `index`.
pub(super) fn lower_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[..index]
        .iter()
        .zip(&occurrences_by_spec[..index])
        .filter_map(|(spec, occurrences)| {
            occurrences.last().map(|occurrence| Bound {
                position: occurrence.insert_at,
                glob: spec.glob,
                line: occurrence.header_line + 1,
            })
        })
        .max_by_key(|bound| bound.position)
}

/// The earliest point any already-present, canonically later spec's section
/// starts at, if one exists in `existing` — every section that must follow
/// the spec at `index`.
pub(super) fn upper_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[index + 1..]
        .iter()
        .zip(&occurrences_by_spec[index + 1..])
        .filter_map(|(spec, occurrences)| {
            occurrences.first().map(|occurrence| Bound {
                position: occurrence.header_line,
                glob: spec.glob,
                line: occurrence.header_line + 1,
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
