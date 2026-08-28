//! Merge prim's canonical Markdown placement map into an `.editorconfig` a
//! person already wrote, without moving a byte of it.
//!
//! Every change is planned first and applied only if the file it would
//! produce resolves the way prim meant it to (see [`super::outcome`]) — the
//! map is positional, so where a section lands decides what it means.

use std::collections::BTreeMap;

use super::outcome;
use super::sections::{
    Bound, SectionSpec, bool_word, governing, has_top_level_root, header_lines, key_line,
    lower_bound, matching_sections, push_insert, section_block, split_lines, upper_bound,
};
use super::{EVERYTHING_GLOB, MDLINT_STRICT_KEY, WORKING_MEMORY};

pub(super) struct MergeResult {
    pub(super) contents: String,
    pub(super) actions: Vec<String>,
    /// Whether this merge prepends `root = true`. Carried out rather than
    /// re-derived by the caller, so the cascade warning can never disagree
    /// with the write it is about (see [`super::cascade`]).
    pub(super) added_root: bool,
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

/// prim's canonical sections, in the order they must appear, each with
/// one representative path the outcome check resolves to decide whether a
/// write did what prim meant it to.
pub(super) fn canonical_specs(strict_glob: &str) -> Vec<SectionSpec<'_>> {
    every_section(strict_glob)
        .into_iter()
        .filter(|(_, writes)| *writes)
        .map(|(spec, _)| spec)
        .collect()
}

/// Every canonical section prim knows about, including one it does not write
/// for this strict glob.
///
/// Reporting on the file prim leaves behind is a different question from
/// deciding what to write into it: a `[*.md]` a person wrote is worth
/// reporting on when it no longer decides anything, whether or not prim would
/// have written that section itself.
pub(super) fn reported_specs(strict_glob: &str) -> Vec<SectionSpec<'_>> {
    every_section(strict_glob)
        .into_iter()
        .map(|(spec, _)| spec)
        .collect()
}

/// Every canonical section in canonical order, each with whether prim
/// writes it for this strict glob.
///
/// A section the strict glob makes dead on arrival is not written — and drops
/// out of the write check with its representative path, which is drawn from
/// the very population the strict glob covers. A `README.md` under `[**.md]`
/// is the same kind of file as the strict glob's own probe; there is nothing
/// left for the dropped section to decide that the strict glob does not.
/// Writing a section that is dead on arrival is how the docs/wip exemption
/// went wrong twice.
fn every_section(strict_glob: &str) -> Vec<(SectionSpec<'_>, bool)> {
    let mut sections = vec![
        (
            SectionSpec {
                glob: "*.md",
                value: false,
                probes: vec!["README.md".to_string()],
            },
            strict_glob != EVERYTHING_GLOB,
        ),
        (
            SectionSpec {
                glob: strict_glob,
                value: true,
                probes: vec![strict_probe(strict_glob, "guide.md")],
            },
            true,
        ),
    ];

    // Superpowers working memory is exempt from the strict tier: specs and
    // plans under `docs/wip/` are transient, and gardening moves their raw
    // originals to `docs/archive/`. That move must not change a document's
    // tier, so the strict tier reaches neither, even when the strict glob
    // covers `docs/**` — unless the strict glob is that directory itself or
    // one inside it, where the exemption is the broader glob and would turn
    // the whole book back off.
    sections.extend(WORKING_MEMORY.iter().map(|memory| {
        (
            SectionSpec {
                glob: memory.glob,
                value: false,
                probes: vec![format!("{}/plan.md", memory.dir)],
            },
            !exemption_covers(strict_glob, memory.dir),
        )
    }));

    // Summaries decided by different sections: one under the strict glob, and
    // one inside each exempt directory. A single representative would leave
    // the others unchecked — and it was the second that the retired
    // section-order check used to catch.
    let mut summary_probes = vec![strict_probe(strict_glob, "SUMMARY.md")];
    summary_probes.extend(
        WORKING_MEMORY
            .iter()
            .map(|memory| format!("{}/SUMMARY.md", memory.dir)),
    );
    sections.push((
        SectionSpec {
            glob: "**/SUMMARY.md",
            value: false,
            probes: summary_probes,
        },
        true,
    ));

    sections
}

/// A representative path named `file` inside the directory the strict glob
/// covers — the repository root when that glob covers every directory.
fn strict_probe(strict_glob: &str, file: &str) -> String {
    match strict_glob.strip_suffix("/**.md") {
        Some(dir) => format!("{dir}/{file}"),
        None => file.to_string(),
    }
}

/// Whether an exemption for `dir` would cover everything the strict glob does
/// — that directory itself, or any directory inside it.
fn exemption_covers(strict_glob: &str, dir: &str) -> bool {
    let Some(strict_dir) = strict_glob.strip_suffix("/**.md") else {
        return false;
    };
    strict_dir == dir || strict_dir.starts_with(&format!("{dir}/"))
}

/// The map prim writes into a repository with no `.editorconfig`, or the
/// reasons it would not hold.
///
/// The only way to obtain that text: the check is the constructor, not a
/// guard beside it, so no caller can end up writing an unchecked map. What it
/// compares is the rendered file against the declared map — every
/// representative path must resolve to the value the last section prim writes
/// for it. A user's `book.toml` cannot make that fail; a future edit to
/// either the section list or the rendering can, which is what this is for.
pub(super) fn checked_scaffold(strict_glob: &str) -> Result<String, Vec<String>> {
    let specs = canonical_specs(strict_glob);
    let text = render_scaffold(&specs);
    let flaws = map_flaws(&specs, &text);
    if flaws.is_empty() {
        Ok(text)
    } else {
        Err(flaws)
    }
}

/// The canonical map as a file: `root = true`, then every section, in
/// canonical order. Deliberately private — [`checked_scaffold`] is the only
/// way to obtain this text, so no caller can write a map prim has not checked.
fn render_scaffold(specs: &[SectionSpec<'_>]) -> String {
    let sections = specs
        .iter()
        .map(|spec| {
            format!(
                "[{}]\n{MDLINT_STRICT_KEY} = {}\n",
                spec.glob,
                bool_word(spec.value)
            )
        })
        .collect::<String>();
    format!("root = true\n{sections}")
}

/// Every way `text` fails to resolve the way `specs` say it should.
pub(super) fn map_flaws(specs: &[SectionSpec<'_>], text: &str) -> Vec<String> {
    let written = vec![true; specs.len()];
    outcome::violations(specs, &written, text, text)
        .iter()
        .map(outcome::describe)
        .collect()
}

pub(super) fn merge(existing: &str, strict_glob: &str) -> MergeResult {
    let specs = canonical_specs(strict_glob);

    // prim's own line scanner is more forgiving than an EditorConfig parser,
    // so a file prim can walk may still be one it cannot resolve — and
    // without a resolution there is nothing to check a write against. prim
    // does not edit a file it cannot read.
    if outcome::resolves_strict(existing, &specs[0].probes[0]).is_none() {
        return MergeResult {
            contents: existing.to_string(),
            actions: Vec::new(),
            added_root: false,
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
        if occurrences.iter().any(|occurrence| occurrence.has_key()) {
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

    let kept = safe_writes(&plan, &specs, existing, &lines, added_root);
    // Line numbers are shown to somebody reading the file prim is about to
    // leave behind, not the one it read: prim's own inserts move them.
    let line_of = |index: usize| final_line(&plan, kept, added_root, index);

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
    let contents = if actions.is_empty() {
        existing.to_string()
    } else {
        render(&plan, kept, existing, &lines, added_root)
    };

    // Asked of the file prim is leaving behind, not the one it read: a
    // section that no longer decides its own path is worth reporting whether
    // it lost to something the author wrote or to something prim just added.
    // Line numbers need no mapping here — they are lines of that same file.
    let mut warnings = outcome::defeated_sections(&reported_specs(strict_glob), &contents);
    warnings.extend(refusals.into_values());

    MergeResult {
        contents,
        actions,
        added_root,
        warnings,
    }
}

/// The largest set of planned writes prim can make while every canonical glob
/// still resolves the way prim intended and no other one moves, as a bitmask
/// over `plan`; among equally large sets, the one that keeps the canonically
/// earliest writes wins. Writing nothing is always a candidate, so this
/// always has an answer — prim does as much good as it safely can, and no
/// more. The search is exhaustive over every subset of `plan`, which is
/// affordable because `plan` holds at most one write per canonical section and
/// `every_section` is a fixed short list.
fn safe_writes(
    plan: &[PlannedWrite],
    specs: &[SectionSpec<'_>],
    existing: &str,
    lines: &[&str],
    added_root: bool,
) -> u32 {
    let mut masks: Vec<u32> = (0..1u32 << plan.len()).collect();
    masks.sort_by_key(|mask| (std::cmp::Reverse(mask.count_ones()), *mask));
    for mask in masks {
        let candidate = render(plan, mask, existing, lines, added_root);
        let written = written_specs(plan, mask, specs.len());
        if outcome::violations(specs, &written, existing, &candidate).is_empty() {
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
