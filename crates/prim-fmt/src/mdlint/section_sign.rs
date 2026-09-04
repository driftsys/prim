//! The MD051 section-sign correction (#180, AD-0018).
//!
//! rumdl builds its emoji placeholder as the literal string `§emoji§` and
//! preserves U+00A7 verbatim so that sentinel survives its character pass,
//! where GitHub strips it. This module filters the fragment findings that
//! exist only for that reason.
//!
//! It is a workaround with an owner. When rumdl fixes the slug, this file and
//! its tests are deleted whole and the one call at the end of [`super::lint`]
//! goes with them.

use std::collections::HashSet;

use rumdl_lib::config::Config;
use rumdl_lib::rules::all_rules;

use super::{FLAVOR, MdDiagnostic};

/// The link-fragment rule whose findings [`without_section_sign_false_positives`]
/// filters.
const FRAGMENT_RULE: &str = "MD051";

/// U+00A7. rumdl builds its emoji placeholder as the literal string
/// `§emoji§` and, to keep that sentinel intact, preserves this character
/// verbatim when it computes a heading's slug
/// (`rumdl/src/utils/anchor_styles/github.rs`). GitHub strips it, keeping
/// only letters, digits, spaces, hyphens and underscores. rumdl therefore
/// computes `a-§1-b` for `## A §1 B` where a reader's browser resolves
/// `a-1-b` (#180).
const SECTION_SIGN: char = '\u{a7}';

/// The characters the second pass may write in place of [`SECTION_SIGN`], in
/// preference order.
///
/// Each has to hold three properties. rumdl must strip it from a computed
/// slug, exactly as GitHub strips [`SECTION_SIGN`] — a fact about rumdl, so
/// `every_stand_in_is_stripped_from_a_slug` pins it for all of them. Each must
/// be **one character**, so that replacing one character with it leaves every
/// later column where it was; the `char` type guarantees that. And the one
/// actually used must be **absent from the document**, which
/// [`stand_in_for`] is what establishes.
///
/// Characters, not bytes: rumdl reports a column as a character offset within
/// the line, so a stand-in of equal byte width but two characters would still
/// shift every column after it.
///
/// Absence matters because MD051 does not resolve every fragment through a
/// slug. An explicit `<a id="...">` is stored verbatim and compared as a
/// literal string, so substituting a character the document already uses can
/// make two literals equal that were not: a fragment `#x°y` beside
/// `<a id="x§y">` is dead in the document the author wrote, and became a match
/// once both sides were rewritten to `x°y`. A stand-in the document does not
/// contain cannot collide with anything in it.
///
/// Substituting rather than deleting is what keeps the second property.
/// Deleting removes a character, so a document reading
/// `[a](#dead1) § [b](#dead2)` reported the second dead link one column to the
/// left in the second pass, matched nothing, and lost a finding that was
/// genuine.
const SECTION_SIGN_STAND_INS: [char; 4] = ['\u{b0}', '\u{a4}', '\u{a6}', '\u{ac}'];

/// The first [`SECTION_SIGN_STAND_INS`] entry `source` does not already
/// contain, or `None` when it contains every one of them — in which case the
/// caller suppresses nothing rather than risk a collision.
fn stand_in_for(source: &str) -> Option<char> {
    SECTION_SIGN_STAND_INS
        .into_iter()
        .find(|stand_in| !source.contains(*stand_in))
}

/// Drop the [`FRAGMENT_RULE`] findings that exist only because rumdl retained
/// [`SECTION_SIGN`] in a heading's computed slug.
///
/// A source holding no [`SECTION_SIGN`] cannot trigger the collision, so
/// linting a copy in which every occurrence has been replaced by
/// [`SECTION_SIGN_STAND_INS`] yields the fragment findings GitHub's own slug
/// would produce. A finding present in both passes is one a reader would also
/// hit; a finding that only the first pass reports is the artifact, and goes.
///
/// Substituting everywhere rather than in heading lines alone is deliberate: a
/// setext heading's text line carries no marker, so recognising headings means
/// parsing Markdown, which prim does not hand-roll (#149). Substituting costs
/// nothing elsewhere because the stand-in is stripped from a slug wherever it
/// lands, and because [`stand_in_for`] picks one the document does not already
/// contain — so no two strings MD051 compares literally, such as a fragment
/// and an explicit `<a id="...">`, become equal that were not.
///
/// A second pass that fails suppresses nothing.
///
/// One case stays as it was, because this pass can only subtract. A link
/// written `#a-§1-b` beside a heading `## A §1 B` resolves no anchor a
/// renderer produces, and rumdl matches it against its own slug and reports
/// nothing; with no first-pass finding there is nothing to filter. A fragment
/// holding the character beside any *other* heading is a genuine finding, and
/// reaches the caller.
pub(super) fn without_section_sign_false_positives(
    source: &str,
    cfg: &Config,
    diagnostics: Vec<MdDiagnostic>,
    active: impl Fn(&str) -> bool,
) -> Vec<MdDiagnostic> {
    // The finding check first: it walks a usually-empty vector, where the
    // source scan walks the whole document, and almost every document reaches
    // here with neither.
    if !diagnostics.iter().any(|d| d.rule == FRAGMENT_RULE)
        || !active(FRAGMENT_RULE)
        || !source.contains(SECTION_SIGN)
    {
        return diagnostics;
    }

    let Some(stand_in) = stand_in_for(source) else {
        return diagnostics;
    };

    let mut buffer = [0u8; 4];
    let neutralised = source.replace(SECTION_SIGN, stand_in.encode_utf8(&mut buffer));
    let Some(genuine) = fragment_positions(&neutralised, cfg) else {
        return diagnostics;
    };

    diagnostics
        .into_iter()
        .filter(|d| d.rule != FRAGMENT_RULE || genuine.contains(&(d.line, d.column)))
        .collect()
}

/// The positions [`FRAGMENT_RULE`] reports for `source`, or `None` when the
/// second pass could not be run — which the caller reads as "suppress
/// nothing".
///
/// An empty rule set is one of those cases rather than a clean run. rumdl
/// answers a lint over no rules with `Ok(vec![])`, which is indistinguishable
/// from "this document has no dead fragments", and reading it as the latter
/// would suppress every finding — the opposite of what the caller promises. A
/// rumdl that renamed or re-cased the rule lands exactly there. Under the
/// pinned `rumdl = "=0.2.35"` the branch is unreachable, so no test covers it;
/// it is the fail-open direction rather than a behaviour prim can demonstrate.
fn fragment_positions(source: &str, cfg: &Config) -> Option<HashSet<(usize, usize)>> {
    let rules: Vec<_> = all_rules(cfg)
        .into_iter()
        .filter(|rule| rule.name() == FRAGMENT_RULE)
        .collect();
    if rules.is_empty() {
        return None;
    }

    let warnings = rumdl_lib::lint(source, &rules, false, FLAVOR, None, Some(cfg)).ok()?;

    Some(
        warnings
            .into_iter()
            // The same name check [`super::lint`] applies to what rumdl hands
            // back: a warning under another rule's name must not keep a
            // fragment finding alive.
            .filter(|warning| warning.rule_name.as_deref() == Some(FRAGMENT_RULE))
            .map(|warning| (warning.line, warning.column))
            .collect(),
    )
}

#[cfg(test)]
mod tests;
