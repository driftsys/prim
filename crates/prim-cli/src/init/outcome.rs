//! Check what a candidate `.editorconfig` would actually resolve to, before
//! `prim init` writes it.
//!
//! prim's placement map is positional: EditorConfig has no specificity
//! ranking, so a later section wins and position alone decides meaning.
//! Reasoning about that positionally — is this section before that one — has
//! repeatedly approximated the property that actually matters, which is about
//! outcomes:
//!
//! > After any write prim makes, every canonical glob must resolve to the
//! > value prim intended, and no canonical glob prim did not intend to change
//! > may resolve differently than it did before.
//!
//! The check is over prim's own canonical globs, one representative path
//! each. A glob a person wrote that no representative path stands for — say
//! `[docs/generated/**.md]` — is not covered: prim's `[docs/**.md]` can still
//! be appended after it and win. Widening the check would mean deriving a
//! path from an arbitrary glob, which is not generally possible.
//!
//! This module checks that property directly. It resolves one representative
//! path per canonical section prim places — a top-level file, a file under
//! the strict glob, a file under `docs/wip/` when that section is one prim
//! places, and a `SUMMARY.md` under the strict glob — against the candidate
//! text, applying EditorConfig's last-match-wins section order, and compares
//! the result with both the value prim intends and the value the file
//! resolved to before the run.

use std::path::Path;

use ec4rs::{ConfigParser, Properties, PropertiesSource};

use super::MDLINT_STRICT_KEY;
use super::sections::{Bound, SectionSpec, bool_word};
use crate::mdlint_policy;

/// One way a candidate `.editorconfig` breaks the invariant.
pub(super) enum Violation {
    /// prim wrote this spec's section, but a later section overrides it, so
    /// the write does not have the effect prim reported.
    Ineffective {
        path: String,
        resolved: bool,
        intended: bool,
    },
    /// prim did not write this spec's section, yet a write elsewhere changes
    /// how its representative path resolves. `owner` is the spec whose
    /// section is meant to decide that path.
    Collateral {
        owner: usize,
        path: String,
        before: bool,
        after: bool,
    },
    /// prim cannot resolve the candidate at all, so it cannot tell whether
    /// the write is safe. `merge` refuses to plan anything for a file that
    /// does not parse, so in practice this reports a candidate prim itself
    /// produced; it exists so [`violations`] can never answer "nothing wrong"
    /// about a file it could not read.
    Unverifiable,
}

/// The effective `prim_mdlint_strict` for `path` under `content`, applying
/// each section in file order so the last matching one wins — EditorConfig's
/// own resolution, run over text prim has not written yet. `None` when
/// `content` does not parse as an `.editorconfig`.
pub(super) fn resolves_strict(content: &str, path: &str) -> Option<bool> {
    let parser = ConfigParser::new_buffered(content.as_bytes()).ok()?;
    let mut props = Properties::new();
    for section in parser {
        let section = section.ok()?;
        let _ = (&section).apply_to(&mut props, Path::new(path));
    }
    Some(mdlint_policy::strict_from(&props))
}

/// The value prim intends each spec's representative path to resolve to: the
/// value the canonical map prim writes from scratch gives it. Taking the
/// intent from `scaffold` rather than from each spec's own `value` is what
/// makes the check right for a strict glob that overlaps another canonical
/// glob (an mdBook whose `src` is `.`, say, where `[**.md] = true` legitimately
/// covers top-level files too).
pub(super) fn intended_values(specs: &[SectionSpec<'_>], scaffold: &str) -> Vec<bool> {
    specs
        .iter()
        .map(|spec| resolves_strict(scaffold, &spec.probe).unwrap_or(spec.value))
        .collect()
}

/// Every way `candidate` breaks the invariant, in canonical spec order.
/// `written[i]` says whether `candidate` contains prim's write for `specs[i]`;
/// `before` is the text prim started from.
pub(super) fn violations(
    specs: &[SectionSpec<'_>],
    intended: &[bool],
    written: &[bool],
    before: &str,
    candidate: &str,
) -> Vec<Violation> {
    let mut found = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        let (Some(after_value), Some(before_value)) = (
            resolves_strict(candidate, &spec.probe),
            resolves_strict(before, &spec.probe),
        ) else {
            return vec![Violation::Unverifiable];
        };
        if written[index] {
            if after_value != intended[index] {
                found.push(Violation::Ineffective {
                    path: spec.probe.clone(),
                    resolved: after_value,
                    intended: intended[index],
                });
            }
        } else if after_value != before_value {
            found.push(Violation::Collateral {
                owner: index,
                path: spec.probe.clone(),
                before: before_value,
                after: after_value,
            });
        }
    }
    found
}

/// The warning for a write prim planned and then dropped, explaining it by
/// what the file would have resolved to rather than by where a section sits.
/// `neighbours` are the canonical sections the skipped one belongs between,
/// for the reader who now has to place it by hand; `present` says which
/// canonical sections the file actually has, so no advice names a section
/// that is not there.
pub(super) fn refusal(
    specs: &[SectionSpec<'_>],
    index: usize,
    creates_section: bool,
    found: &[Violation],
    neighbours: (Option<Bound<'_>>, Option<Bound<'_>>),
    present: &[bool],
) -> String {
    let spec = &specs[index];
    let skipped = skipped_write(spec, creates_section);

    if let Some(Violation::Ineffective {
        path,
        resolved,
        intended,
    }) = found
        .iter()
        .find(|violation| matches!(violation, Violation::Ineffective { .. }))
    {
        return format!(
            "{skipped}: {path} would still resolve to {MDLINT_STRICT_KEY} = {}, not {}, because a \
             later section in this .editorconfig sets it; {}",
            bool_word(*resolved),
            bool_word(*intended),
            place_it_yourself(spec, creates_section, &neighbours),
        );
    }

    let collateral: Vec<(usize, String)> = found
        .iter()
        .filter_map(|violation| match violation {
            Violation::Collateral {
                owner,
                path,
                before,
                after,
            } => Some((
                *owner,
                format!(
                    "for {path} from {} to {}",
                    bool_word(*before),
                    bool_word(*after)
                ),
            )),
            _ => None,
        })
        .collect();
    if !collateral.is_empty() {
        let changes: Vec<String> = collateral.iter().map(|(_, text)| text.clone()).collect();
        // A section that is not in the file cannot be reordered against, so
        // fall back to naming the canonical order when none of them is there.
        let sections: Vec<String> = collateral
            .iter()
            .filter(|(owner, _)| present[*owner])
            .map(|(owner, _)| format!("[{}]", specs[*owner].glob))
            .collect();
        let fix = if sections.is_empty() {
            format!(
                "put [{}] in prim's canonical section order yourself: {}",
                spec.glob,
                canonical_order(specs)
            )
        } else {
            format!(
                "reorder them yourself so [{}] comes before {}",
                spec.glob,
                join(&sections)
            )
        };
        return format!(
            "{skipped}: it would change {MDLINT_STRICT_KEY} {}, which prim did not intend; prim \
             will not reorder sections a person wrote, so {fix}",
            join(&changes),
        );
    }

    format!(
        "{skipped}: prim could not read the .editorconfig that write would produce, so it could \
         not tell what the change would resolve to"
    )
}

/// `not adding [glob]` / `not setting the key in [glob]` — the write prim
/// planned, named the way the summary line would have named it.
fn skipped_write(spec: &SectionSpec<'_>, creates_section: bool) -> String {
    if creates_section {
        format!("not adding [{}]", spec.glob)
    } else {
        format!(
            "not setting {MDLINT_STRICT_KEY} = {} in [{}]",
            bool_word(spec.value),
            spec.glob
        )
    }
}

/// The instruction for doing by hand what prim refused to do, including where
/// the section has to sit — the part that is hard to get right, since
/// EditorConfig ranks by position only.
fn place_it_yourself(
    spec: &SectionSpec<'_>,
    creates_section: bool,
    (after, before): &(Option<Bound<'_>>, Option<Bound<'_>>),
) -> String {
    let action = if creates_section {
        format!(
            "add [{}] with {MDLINT_STRICT_KEY} = {} yourself",
            spec.glob,
            bool_word(spec.value)
        )
    } else {
        format!(
            "move [{}] yourself and add {MDLINT_STRICT_KEY} = {} under it",
            spec.glob,
            bool_word(spec.value)
        )
    };
    // Two bounds that are themselves out of order describe a position that
    // does not exist; sending the reader there would be advice they cannot
    // follow.
    let position = match (after, before) {
        (Some(after), Some(before)) if after.position <= before.position => {
            format!(", after [{}] and before [{}]", after.glob, before.glob)
        }
        (Some(after), Some(_)) => {
            format!(", after [{}] once the sections are in order", after.glob)
        }
        (Some(after), None) => format!(", after [{}]", after.glob),
        (None, Some(before)) => format!(", before [{}]", before.glob),
        (None, None) => String::new(),
    };
    format!("prim will not reorder sections a person wrote, so {action}{position}")
}

/// prim's canonical section order, written out for a message that cannot
/// point at two sections the file actually has.
fn canonical_order(specs: &[SectionSpec<'_>]) -> String {
    specs
        .iter()
        .map(|spec| format!("[{}]", spec.glob))
        .collect::<Vec<_>>()
        .join(" → ")
}

fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [only] => only.clone(),
        [head @ .., last] => format!("{} and {last}", head.join(", ")),
    }
}
