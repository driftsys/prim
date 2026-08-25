//! Merge prim's canonical Markdown placement map into an `.editorconfig` a
//! person already wrote, without moving a byte of it.
//!
//! Every change is planned first and applied only if the file it would
//! produce resolves the way prim meant it to (see [`super::outcome`]) — the
//! map is positional, so where a section lands decides what it means.

use std::collections::BTreeMap;

use super::outcome;
use super::sections::{
    Bound, SectionSpec, bool_word, existing_order_warnings, governing, has_top_level_root,
    header_lines, key_line, lower_bound, matching_sections, push_insert, section_block,
    split_lines, upper_bound,
};
use super::{DOCS_WIP_GLOB, MDLINT_STRICT_KEY, scaffold};

pub(super) struct MergeResult {
    pub(super) contents: String,
    pub(super) actions: Vec<String>,
    /// Everything prim could not do to this file, each entry naming work the
    /// reader now has to do by hand — prim never reorders sections a person
    /// wrote. Three things land here: a change prim planned and dropped (a
    /// section with no position left for it, or a write that would not have
    /// resolved the way prim intended), two already-present sections whose
    /// relative order contradicts the canonical one (nothing was planned for
    /// those — both already carry the key), and the one whole-file refusal,
    /// for an `.editorconfig` that does not parse. Except for that last one,
    /// a warning here means "not this change"; every other planned change is
    /// still made in the same run.
    pub(super) warnings: Vec<String>,
}

/// One change `merge` is prepared to make, held as a plan rather than applied
/// on sight so the resolution it would produce can be checked first.
struct PlannedWrite {
    /// Index into `specs` of the canonical section this write is for.
    spec: usize,
    /// Index of the line the text is inserted before.
    at: usize,
    text: String,
    /// `true` when the write creates the section, `false` when it adds the
    /// key to a section that is already there.
    creates_section: bool,
}

/// prim's four canonical sections, in the order they must appear, each with
/// one representative path the outcome check resolves to decide whether a
/// write did what prim meant it to.
fn canonical_specs(strict_glob: &str) -> Vec<SectionSpec<'_>> {
    let mut specs = vec![
        SectionSpec {
            glob: "*.md",
            value: false,
            probe: "README.md".to_string(),
        },
        SectionSpec {
            glob: strict_glob,
            value: true,
            probe: strict_probe(strict_glob, "guide.md"),
        },
    ];
    // Superpowers specs and plans under `docs/wip/` are transient working
    // memory, so the strict tier must not apply to them even when the strict
    // glob covers `docs/**` — unless the strict glob already IS
    // `docs/wip/**.md`, in which case the author asked for that directory to
    // be strict and a separate exemption section would just defeat it (see
    // `scaffold`).
    if strict_glob != DOCS_WIP_GLOB {
        specs.push(SectionSpec {
            glob: DOCS_WIP_GLOB,
            value: false,
            probe: "docs/wip/plan.md".to_string(),
        });
    }
    specs.push(SectionSpec {
        glob: "**/SUMMARY.md",
        value: false,
        probe: strict_probe(strict_glob, "SUMMARY.md"),
    });
    specs
}

/// A representative path named `file` inside the directory the strict glob
/// covers — the repository root when that glob covers every directory.
fn strict_probe(strict_glob: &str, file: &str) -> String {
    match strict_glob.strip_suffix("/**.md") {
        Some(dir) => format!("{dir}/{file}"),
        None => file.to_string(),
    }
}

pub(super) fn merge(existing: &str, strict_glob: &str) -> MergeResult {
    let specs = canonical_specs(strict_glob);

    // prim's own line scanner is more forgiving than an EditorConfig parser,
    // so a file prim can walk may still be one it cannot resolve — and
    // without a resolution there is nothing to check a write against. prim
    // does not edit a file it cannot read.
    if outcome::resolves_strict(existing, &specs[0].probe).is_none() {
        return MergeResult {
            contents: existing.to_string(),
            actions: Vec::new(),
            warnings: vec![
                "this .editorconfig does not parse as EditorConfig, so prim cannot tell what a \
                 change to it would resolve to and made none"
                    .to_string(),
            ],
        };
    }

    let lines = split_lines(existing);
    let headers = header_lines(&lines);
    let occurrences_by_spec = specs
        .iter()
        .map(|spec| matching_sections(&lines, &headers, spec.glob))
        .collect::<Vec<_>>();

    let added_root = !has_top_level_root(&lines, &headers);
    // Sections prim cannot place at all, keyed by spec index. Held as the two
    // bounds rather than a message because the line numbers a message shows
    // are only known once prim knows which writes it is making.
    let mut unplaceable: BTreeMap<usize, (Bound<'_>, Bound<'_>)> = BTreeMap::new();
    let mut plan: Vec<PlannedWrite> = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let occurrences = &occurrences_by_spec[index];
        if occurrences.iter().any(|occurrence| occurrence.has_key) {
            continue;
        }

        if let Some(occurrence) = governing(occurrences) {
            plan.push(PlannedWrite {
                spec: index,
                at: occurrence.insert_at,
                text: key_line(spec.value),
                creates_section: false,
            });
            continue;
        }

        let lower = lower_bound(&specs, &occurrences_by_spec, index);
        let upper = upper_bound(&specs, &occurrences_by_spec, index);
        if let (Some(lower), Some(upper)) = (lower, upper)
            && lower.position > upper.position
        {
            unplaceable.insert(index, (lower, upper));
            continue;
        }

        plan.push(PlannedWrite {
            spec: index,
            at: upper.map_or(lines.len(), |bound| bound.position),
            text: section_block(spec.glob, spec.value),
            creates_section: true,
        });
    }

    let intended = outcome::intended_values(&specs, &scaffold(strict_glob));
    let kept = safe_writes(&plan, &specs, &intended, existing, &lines, added_root);
    // Line numbers are shown to somebody reading the file prim is about to
    // leave behind, not the one it read: prim's own inserts move them.
    let line_of = |index: usize| final_line(&plan, kept, added_root, index);

    // Sections that are already present can contradict the canonical order
    // too. prim writes nothing into them — they already carry their key — so
    // no planned write is there to be checked, and without this the run would
    // report plain success over a file that resolves the wrong way.
    let mut warnings = existing_order_warnings(&specs, &occurrences_by_spec, &line_of);
    // Keyed by spec index so refusals read in canonical order however they
    // were found.
    let mut refusals: BTreeMap<usize, String> = BTreeMap::new();
    for (index, (lower, upper)) in &unplaceable {
        refusals.insert(
            *index,
            format!(
                "not adding [{}]: [{}] (line {}) comes after [{}] (line {}) in this \
                 .editorconfig, which contradicts prim's canonical section order; prim will not \
                 reorder sections a person wrote, so put [{}] before [{}] yourself and add \
                 {MDLINT_STRICT_KEY} = {} under [{}] between them",
                specs[*index].glob,
                lower.glob,
                line_of(lower.header_line),
                upper.glob,
                line_of(upper.header_line),
                lower.glob,
                upper.glob,
                bool_word(specs[*index].value),
                specs[*index].glob,
            ),
        );
    }

    // Which canonical sections the file actually has, so a refusal never
    // sends the reader to reorder a section that is not there.
    let present: Vec<bool> = occurrences_by_spec
        .iter()
        .map(|occurrences| !occurrences.is_empty())
        .collect();

    let mut actions = Vec::new();
    if added_root {
        actions.push("added top-level root = true".to_string());
    }
    for (position, write) in plan.iter().enumerate() {
        let spec = &specs[write.spec];
        if kept & (1 << position) != 0 {
            actions.push(if write.creates_section {
                format!(
                    "added [{}] with {MDLINT_STRICT_KEY} = {}",
                    spec.glob,
                    bool_word(spec.value)
                )
            } else {
                format!(
                    "set {MDLINT_STRICT_KEY} = {} in [{}]",
                    bool_word(spec.value),
                    spec.glob
                )
            });
            continue;
        }

        // Every dropped write breaks the invariant when added back to the
        // set prim kept — otherwise `safe_writes` would have kept it too —
        // so this always finds the violation to report.
        let with_write = kept | (1 << position);
        let candidate = render(&plan, with_write, existing, &lines, added_root);
        let found = outcome::violations(
            &specs,
            &intended,
            &written_specs(&plan, with_write, specs.len()),
            existing,
            &candidate,
        );
        refusals.insert(
            write.spec,
            outcome::refusal(
                &specs,
                write.spec,
                write.creates_section,
                &found,
                (
                    lower_bound(&specs, &occurrences_by_spec, write.spec),
                    upper_bound(&specs, &occurrences_by_spec, write.spec),
                ),
                &present,
            ),
        );
    }
    warnings.extend(refusals.into_values());

    let contents = if actions.is_empty() {
        existing.to_string()
    } else {
        render(&plan, kept, existing, &lines, added_root)
    };

    MergeResult {
        contents,
        actions,
        warnings,
    }
}

/// The largest set of planned writes prim can make while every canonical glob
/// still resolves the way prim intended and no other one moves, as a bitmask
/// over `plan`; among equally large sets, the one that keeps the canonically
/// earliest writes wins. Writing nothing is always a candidate, so this
/// always has an answer — prim does as much good as it safely can, and no
/// more. The search is exhaustive because `plan` holds at most one write per
/// canonical section (four).
fn safe_writes(
    plan: &[PlannedWrite],
    specs: &[SectionSpec<'_>],
    intended: &[bool],
    existing: &str,
    lines: &[&str],
    added_root: bool,
) -> u32 {
    let mut masks: Vec<u32> = (0..1u32 << plan.len()).collect();
    masks.sort_by_key(|mask| (std::cmp::Reverse(mask.count_ones()), *mask));
    for mask in masks {
        let candidate = render(plan, mask, existing, lines, added_root);
        let written = written_specs(plan, mask, specs.len());
        if outcome::violations(specs, intended, &written, existing, &candidate).is_empty() {
            return mask;
        }
    }
    0
}

/// The 1-indexed line that line `index` of the file prim read ends up on once
/// the writes `mask` selects are applied. A warning that names a line has to
/// name a line of the file prim leaves behind.
fn final_line(plan: &[PlannedWrite], mask: u32, added_root: bool, index: usize) -> usize {
    let inserted: usize = plan
        .iter()
        .enumerate()
        .filter(|(position, write)| mask & (1 << position) != 0 && write.at <= index)
        .map(|(_, write)| write.text.lines().count())
        .sum();
    let root_lines = if added_root { 2 } else { 0 };
    index + 1 + inserted + root_lines
}

/// Which canonical sections `mask` writes, as one flag per spec.
fn written_specs(plan: &[PlannedWrite], mask: u32, spec_count: usize) -> Vec<bool> {
    let mut written = vec![false; spec_count];
    for (position, write) in plan.iter().enumerate() {
        if mask & (1 << position) != 0 {
            written[write.spec] = true;
        }
    }
    written
}

/// The file prim would write if it made exactly the writes `mask` selects.
fn render(
    plan: &[PlannedWrite],
    mask: u32,
    existing: &str,
    lines: &[&str],
    added_root: bool,
) -> String {
    let mut inserts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (position, write) in plan.iter().enumerate() {
        if mask & (1 << position) != 0 {
            push_insert(&mut inserts, write.at, write.text.clone(), existing, lines);
        }
    }

    let mut contents = String::new();
    if added_root {
        contents.push_str("root = true\n\n");
    }
    for index in 0..=lines.len() {
        if let Some(pending) = inserts.get(&index) {
            for addition in pending {
                contents.push_str(addition);
            }
        }
        if let Some(line) = lines.get(index) {
            contents.push_str(line);
        }
    }
    contents
}
