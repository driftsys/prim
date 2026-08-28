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
use super::sections::{
    Bound, SectionSpec, bool_word, deciding_section, governing, header_lines, is_boolean,
    matching_sections, owns, reads_as_strict, split_lines,
};
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

/// Every way `candidate` breaks the invariant, in canonical spec order.
/// `written[i]` says whether `candidate` contains prim's write for `specs[i]`;
/// `before` is the text prim started from.
///
/// The value each spec is meant to reach is the one the spec declares. It is
/// deliberately not read back out of the map prim would write from scratch:
/// deriving intent from the scaffold makes the scaffold correct by
/// definition, so a defective one could never be a violation. Passing the
/// scaffold as both `before` and `candidate`, with everything `written`, is
/// therefore a self-check of the scaffold itself.
pub(super) fn violations(
    specs: &[SectionSpec<'_>],
    written: &[bool],
    before: &str,
    candidate: &str,
) -> Vec<Violation> {
    let mut found = Vec::new();
    for (index, spec) in specs.iter().enumerate() {
        for probe in &spec.probes {
            let (Some(after_value), Some(before_value)) = (
                resolves_strict(candidate, probe),
                resolves_strict(before, probe),
            ) else {
                return vec![Violation::Unverifiable];
            };
            if written[index] {
                let intended = intended_for(specs, probe);
                if after_value != intended {
                    found.push(Violation::Ineffective {
                        path: probe.clone(),
                        resolved: after_value,
                        intended,
                    });
                }
            } else if after_value != before_value {
                found.push(Violation::Collateral {
                    owner: index,
                    path: probe.clone(),
                    before: before_value,
                    after: after_value,
                });
            }
        }
    }
    found
}

/// What prim's canonical map means for `probe`: the value of the last section
/// prim writes whose glob matches it, since a later section wins.
///
/// This evaluates the declared map — globs and values — and never the text
/// prim renders from it. That is what keeps the check honest: intent read back
/// out of the rendered file would make any defect in that file correct by
/// definition, while a divergence between the declared map and the rendered
/// one is exactly what the scaffold self-check is for.
///
fn intended_for(specs: &[SectionSpec<'_>], probe: &str) -> bool {
    specs
        .iter()
        .filter(|spec| ec4rs::Section::new(spec.glob).applies_to(Path::new(probe)))
        .map(|spec| spec.value)
        .next_back()
        .unwrap_or(false)
}

/// One violation as a phrase, for a message that is not about a write prim
/// chose to skip — the scaffold self-check, where every section is prim's own
/// and there is nothing to advise the reader to do.
///
/// Only [`Violation::Ineffective`] can arise from that self-check: it passes
/// the same text as `before` and `candidate`, so nothing can move, and marks
/// every section written, so nothing is left to move it. The other two arms
/// are here because this is total over the type, and would be what a future
/// caller with a real `before` needs.
pub(super) fn describe(violation: &Violation) -> String {
    match violation {
        Violation::Ineffective {
            path,
            resolved,
            intended,
        } => format!(
            "{path} resolves to {MDLINT_STRICT_KEY} = {}, not {}",
            bool_word(*resolved),
            bool_word(*intended)
        ),
        Violation::Collateral {
            path,
            before,
            after,
            ..
        } => format!(
            "{path} resolves to {MDLINT_STRICT_KEY} = {}, not {}",
            bool_word(*after),
            bool_word(*before)
        ),
        Violation::Unverifiable => "prim could not read the map it built".to_string(),
    }
}

/// A warning for each canonical section the file already has, with its key,
/// that does not actually decide its own representative path.
///
/// prim plans no write for such a section — the key is already there — so
/// nothing it writes can be checked, and the run would otherwise report plain
/// success over a map that does not hold. Two ways a written section fails:
/// a later section overrides it for the very path it is there to place, or
/// its value is one prim does not read as a tier.
///
/// This is a warning, never a refusal. A person's narrower override is
/// legitimate, and prim must stay able to report on a file it disagrees with.
///
/// It also subsumes the pairwise section-order check prim used to run beside
/// it: two keyed canonical sections in the wrong relative order defeat one of
/// them, which shows up here as the defeated section — and only when the
/// order actually changes an outcome, rather than whenever positions
/// disagree.
pub(super) fn defeated_sections(specs: &[SectionSpec<'_>], file: &str) -> Vec<String> {
    let lines = split_lines(file);
    let headers = header_lines(&lines);
    let mut warnings = Vec::new();
    for spec in specs {
        let occurrences = matching_sections(&lines, &headers, spec.glob);
        let Some(occurrence) = governing(&occurrences) else {
            continue;
        };
        let Some(written) = occurrence.key_value else {
            continue;
        };
        let value = reads_as_strict(written);
        let at = occurrence.header_line + 1;

        // One entry per distinct thing that went wrong, each collecting the
        // paths it went wrong for: a section's representatives usually fail
        // the same way, and saying it twice is noise, not detail.
        let mut failures: Vec<(Cause, Vec<String>)> = Vec::new();
        for probe in &spec.probes {
            // What the file actually does, asked before anything is claimed
            // about it — including in the non-boolean case, where the tier is
            // still whatever the cascade says and not necessarily this
            // section's.
            let Some(resolved) = resolves_strict(file, probe) else {
                continue;
            };

            let cause = if !is_boolean(written) {
                Cause::NotBoolean {
                    tier: tier_word(resolved),
                }
            } else if resolved == value {
                continue;
            } else {
                // `occurrence` and the section `deciding_section` names both
                // now come from the same header parse and the same `ec4rs`
                // matcher, so if `occurrence` were the one deciding `probe`,
                // `resolved == value` above would already have taken this
                // path — the two sides cannot disagree about which section
                // that is, and this filter is expected to be unreachable.
                // Verified: reintroducing the pre-#117 `.trim()` into
                // `editorconfig::line::parse`'s header case — the exact
                // divergence between prim's own scan and real `ec4rs` this
                // commit removes — makes this filter fire again for
                // `route_8`'s fixture. It stays as defence-in-depth against
                // `line.rs` and `ec4rs` drifting apart again in the future,
                // at the cost of one `.filter`; the fallback message below
                // exists for the same reason.
                let winner = deciding_section(&lines, &headers, probe)
                    .filter(|(line, _)| *line != occurrence.header_line)
                    .map(|(line, glob)| {
                        format!("[{glob}] (line {}) comes after it and wins", line + 1)
                    })
                    .unwrap_or_else(|| "a later section in this .editorconfig wins".to_string());
                Cause::Defeated { winner, resolved }
            };

            match failures.iter_mut().find(|(seen, _)| seen == &cause) {
                Some((_, paths)) => paths.push(probe.clone()),
                None => failures.push((cause, vec![probe.clone()])),
            }
        }

        // A later section can defeat this one while still agreeing with its
        // value, which nothing above catches. Ask directly whether this
        // occurrence decides any representative of its own — probes or
        // witness. A narrower, agreeing override still leaves the witness (a
        // path it cannot also reach) to this section; a section that reaches
        // every representative leaves it deciding nothing, worth a warning
        // even though no value moved. Skipped once `Defeated` already fired
        // for this spec, to avoid saying the same thing twice.
        let already_defeated = failures
            .iter()
            .any(|(cause, _)| matches!(cause, Cause::Defeated { .. }));
        if !already_defeated {
            let representatives = spec.probes.iter().chain(spec.witness.iter());
            let considered: Vec<bool> = representatives
                .filter_map(|path| owns(&lines, &headers, occurrence.header_line, spec.glob, path))
                .collect();
            if !considered.is_empty() && considered.iter().all(|owned| !owned) {
                let primary = &spec.probes[0];
                let winner = deciding_section(&lines, &headers, primary)
                    .filter(|(line, _)| *line != occurrence.header_line)
                    .map(|(line, glob)| format!("[{glob}] (line {})", line + 1))
                    .unwrap_or_else(|| "nothing else in this .editorconfig".to_string());
                failures.push((Cause::Inert { winner }, vec![primary.clone()]));
            }
        }

        warnings.extend(failures.into_iter().map(|(cause, paths)| {
            let paths = paths.join(" and ");
            match cause {
                Cause::NotBoolean { tier } => format!(
                    "[{}] (line {at}) sets {MDLINT_STRICT_KEY} = {written}, which is neither true \
                     nor false; prim reads every value but true as false, so the {tier} tier \
                     applies to {paths} — write true or false there instead",
                    spec.glob,
                ),
                Cause::Defeated { winner, resolved } => format!(
                    "[{}] (line {at}) sets {MDLINT_STRICT_KEY} = {}, but {winner}, so \
                     {MDLINT_STRICT_KEY} = {} applies to {paths} instead; prim will not reorder \
                     sections a person wrote, so reorder them yourself",
                    spec.glob,
                    bool_word(value),
                    bool_word(resolved),
                ),
                Cause::Inert { winner } => format!(
                    "[{}] (line {at}) sets {MDLINT_STRICT_KEY} = {}, but {winner} comes after it \
                     and is what decides {paths}; the two agree today, so nothing resolves \
                     wrongly, but [{}] decides nothing and will not hold the floor if it changes \
                     — prim will not reorder sections a person wrote, so move [{}] after it \
                     yourself if you want it to",
                    spec.glob,
                    bool_word(value),
                    spec.glob,
                    spec.glob,
                ),
            }
        }));
    }
    warnings
}

/// Why a section that is written does not decide a path of its own. Grouped
/// on, so that a section failing the same way for two paths is one report.
/// `Inert`'s `winner` is the section that decides the primary probe instead,
/// shaped like `Defeated`'s: `[glob] (line N)`, or a fallback when nothing
/// else in the file decides the probe either. It fires when a section
/// decides none of its own representatives, even though its value still
/// agrees with whatever does decide them today.
#[derive(PartialEq, Eq)]
enum Cause {
    NotBoolean { tier: &'static str },
    Defeated { winner: String, resolved: bool },
    Inert { winner: String },
}

/// The tier a resolved `prim_mdlint_strict` names, for a message about which
/// tier a path ended up in rather than which value a key holds.
fn tier_word(strict: bool) -> &'static str {
    if strict { "strict" } else { "floor" }
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
