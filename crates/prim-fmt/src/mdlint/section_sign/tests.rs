//! The MD051 section-sign correction (#180, AD-0018).
//!
//! rumdl retains U+00A7 in a heading's computed slug as an artifact of its own
//! `§emoji§` emoji sentinel, where GitHub strips it. prim lints a second copy
//! with the character replaced by a stand-in the document does not contain,
//! and keeps only the
//! fragment findings both passes report.
//!
//! Kept in its own module because the correction is a workaround with an
//! owner: when upstream fixes the slug, this file is deleted whole along with
//! the two functions it covers.

use super::super::lint;
use super::*;

/// rumdl retains U+00A7 in a heading's computed slug because it uses
/// `§emoji§` as an internal emoji sentinel and preserves that delimiter as a
/// literal character (`utils/anchor_styles/github.rs`). GitHub strips it, so
/// rumdl computes `a-§1-b` where a reader's browser resolves `a-1-b`, and
/// every correct link to such a heading was reported (#180).
#[test]
fn md051_accepts_the_anchor_github_resolves_for_a_heading_holding_a_section_sign() {
    let findings = lint("# T\n\n[l](#a-1-b)\n\n## A §1 B\n\nx\n", false, &[], None);

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "the anchor GitHub resolves must not be reported: {findings:?}"
    );
}

/// The suppression is not "any document holding a section sign is exempt":
/// a genuinely dead anchor in such a document must still report, or the
/// workaround would cost the rule's whole purpose.
#[test]
fn md051_still_reports_a_dead_anchor_in_a_document_holding_a_section_sign() {
    let findings = lint(
        "# T\n\n[l](#does-not-exist)\n\n## A §1 B\n\nx\n",
        false,
        &[],
        None,
    );

    let md051: Vec<_> = findings.iter().filter(|d| d.rule == "MD051").collect();
    assert_eq!(
        md051.len(),
        1,
        "dead anchor must still report: {findings:?}"
    );
    assert_eq!((md051[0].line, md051[0].column), (3, 1));
}

/// Both kinds in one document: the spurious finding goes, the real one stays,
/// and the real one keeps the position it was reported at.
#[test]
fn md051_keeps_the_dead_anchor_and_drops_the_spurious_one() {
    let findings = lint(
        "# T\n\n[a](#a-1-b)\n[b](#missing)\n\n## A §1 B\n\nx\n",
        false,
        &[],
        None,
    );

    let md051: Vec<_> = findings.iter().filter(|d| d.rule == "MD051").collect();
    assert_eq!(
        md051.len(),
        1,
        "only the dead anchor survives: {findings:?}"
    );
    assert!(
        md051[0].message.contains("#missing"),
        "the surviving finding is the dead one: {:?}",
        md051[0]
    );
    assert_eq!((md051[0].line, md051[0].column), (4, 1));
}

/// A document with no section sign takes the unmodified path. The guard's
/// other half — that the second pass does not run — is a cost property, not
/// an observable one; what this pins is that the output is unchanged.
#[test]
fn md051_is_unchanged_for_a_document_with_no_section_sign() {
    assert!(
        lint(
            "# T\n\n[l](#plain-heading)\n\n## Plain Heading\n",
            false,
            &[],
            None
        )
        .iter()
        .all(|d| d.rule != "MD051"),
        "a correct anchor stays clean"
    );

    let dead = lint("# T\n\n[l](#nope)\n\n## Plain Heading\n", false, &[], None);
    assert_eq!(
        dead.iter().filter(|d| d.rule == "MD051").count(),
        1,
        "a dead anchor still reports: {dead:?}"
    );
}

/// A setext heading's text line carries no `#`, so a fix that recognised only
/// ATX headings would leave this reporting. Removing the character from the
/// whole source is what covers it.
#[test]
fn md051_accepts_the_github_anchor_for_a_setext_heading_holding_a_section_sign() {
    let findings = lint(
        "# T\n\n[l](#a-1-b)\n\nA §1 B\n------\n\nx\n",
        false,
        &[],
        None,
    );

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "a setext heading resolves the same anchor: {findings:?}"
    );
}

/// A section sign sitting *before* a link on the same line must not cost that
/// link its finding. Deleting the character moved every column after it, so
/// the second pass reported the link to the left of where the first pass did,
/// the position match failed, and a genuine finding was dropped. The stand-in
/// is the same width, so the positions agree.
#[test]
fn md051_reports_every_dead_anchor_on_a_line_that_also_holds_a_section_sign() {
    let one = lint(
        "# T\n\nx §y [l](#gone)\n\n## A §1 B\n\nx\n",
        false,
        &[],
        None,
    );
    let names: Vec<_> = one
        .iter()
        .filter(|d| d.rule == "MD051")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(names.len(), 1, "the dead anchor survives: {one:?}");
    assert!(names[0].contains("#gone"), "{names:?}");

    let two = lint(
        "# T\n\n[a](#dead1) § [b](#dead2)\n\n## H\n",
        false,
        &[],
        None,
    );
    let both: Vec<_> = two.iter().filter(|d| d.rule == "MD051").collect();
    assert_eq!(both.len(), 2, "both dead anchors survive: {two:?}");
    assert!(both[0].message.contains("#dead1"), "{both:?}");
    assert!(both[1].message.contains("#dead2"), "{both:?}");
}

/// A link fragment that itself holds a section sign resolves against no
/// heading, so it is dead and must report. Deleting the character scrubbed the
/// link as well as the heading, which made `#a-§1-b` match a plain `## A 1 B`
/// and swallowed the finding.
#[test]
fn md051_reports_a_dead_fragment_that_itself_holds_a_section_sign() {
    let findings = lint("# T\n\n[l](#a-§1-b)\n\n## A 1 B\n\nx\n", false, &[], None);

    let md051: Vec<_> = findings.iter().filter(|d| d.rule == "MD051").collect();
    assert_eq!(
        md051.len(),
        1,
        "a fragment no renderer resolves still reports: {findings:?}"
    );
    assert!(
        md051[0].message.contains("#a-§1-b"),
        "the message is the first pass's, naming the anchor the author wrote, \
         not the neutralised text: {:?}",
        md051[0]
    );
}

/// Findings are matched between the passes by line *and* column. With both
/// links on one line the line alone cannot tell them apart, so matching by
/// line would keep the spurious finding too.
#[test]
fn md051_matches_a_finding_by_its_column_and_not_by_its_line_alone() {
    let findings = lint(
        "# T\n\n[a](#a-1-b) [b](#missing)\n\n## A §1 B\n\nx\n",
        false,
        &[],
        None,
    );

    let md051: Vec<_> = findings.iter().filter(|d| d.rule == "MD051").collect();
    assert_eq!(
        md051.len(),
        1,
        "only the dead anchor survives: {findings:?}"
    );
    assert!(md051[0].message.contains("#missing"), "{md051:?}");
}

/// The filter reaches [`FRAGMENT_RULE`] and nothing else: every other rule's
/// findings pass through a document the second pass ran on.
#[test]
fn a_section_sign_document_keeps_the_findings_of_every_other_rule() {
    let findings = lint(
        "# T\n\n[l](#a-1-b)\n\n## A §1 B\n\n![](hero.png)\n",
        false,
        &[],
        None,
    );

    let rules: Vec<_> = findings.iter().map(|d| d.rule.as_str()).collect();
    assert_eq!(
        rules,
        ["MD045"],
        "the image finding survives alone: {findings:?}"
    );
}

/// The tier is resolved before the second pass and unchanged by it.
#[test]
fn the_section_sign_pass_behaves_the_same_under_the_strict_tier() {
    assert!(
        lint("# T\n\n[l](#a-1-b)\n\n## A §1 B\n\nx\n", true, &[], None)
            .iter()
            .all(|d| d.rule != "MD051"),
        "the correct anchor is clean at strict too"
    );
    assert_eq!(
        lint("# T\n\n[l](#gone)\n\n## A §1 B\n\nx\n", true, &[], None)
            .iter()
            .filter(|d| d.rule == "MD051")
            .count(),
        1,
        "the dead anchor still reports at strict"
    );
}

/// The one property of a stand-in that depends on rumdl: rumdl strips it from
/// a computed slug, exactly as GitHub strips [`SECTION_SIGN`]. A future rumdl
/// that started retaining any of them would make the second pass compute a
/// different slug from GitHub's and silently stop meaning what it claims, so
/// every candidate is pinned rather than assumed.
///
/// The other two properties need no test: one character is guaranteed by the
/// `char` type, and absence from the document is what `stand_in_for`
/// establishes per call.
#[test]
fn every_stand_in_is_stripped_from_a_slug() {
    for stand_in in SECTION_SIGN_STAND_INS {
        let heading = format!("# T\n\n[l](#a-1-b)\n\n## A {stand_in}1 B\n\nx\n");

        assert!(
            lint(&heading, false, &[], None)
                .iter()
                .all(|d| d.rule != "MD051"),
            "{stand_in:?} must vanish from the slug the way the section sign does"
        );
    }
}

/// MD051 stores an explicit `<a id="...">` verbatim and compares it as a
/// literal, with no slug involved. A stand-in the document already uses turns
/// two literals that differ into two that match: `#x°y` beside `<a id="x§y">`
/// is dead as written, and was silently swallowed once both sides became
/// `x°y`. Picking a stand-in absent from the source is what prevents it.
#[test]
fn a_dead_fragment_is_reported_when_the_document_already_holds_a_stand_in() {
    let findings = lint("# T\n\n[l](#x°y)\n\n<a id=\"x§y\"></a>\n", false, &[], None);

    assert_eq!(
        findings.iter().filter(|d| d.rule == "MD051").count(),
        1,
        "a fragment that resolves to no anchor must still report: {findings:?}"
    );
}

/// The correction still applies in a document that holds a stand-in: the pass
/// moves to the next candidate rather than giving up.
#[test]
fn the_correction_survives_a_document_that_holds_the_first_stand_in() {
    let findings = lint(
        "# T\n\n[l](#a-1-b)\n\n## A §1 B\n\nAmbient 20° C.\n",
        false,
        &[],
        None,
    );

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "a degree sign in the prose must not cost the correction: {findings:?}"
    );
}

/// A document already holding the stand-in is not a special case: the
/// character is stripped from a slug wherever it came from, so substituting
/// more of it changes no heading's anchor.
#[test]
fn a_document_holding_both_characters_resolves_the_same_anchors() {
    let findings = lint(
        "# T\n\n[a](#a-1-b)\n[b](#c-25-d)\n\n## A §1 B\n\n## C 25° D\n",
        false,
        &[],
        None,
    );

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "both correct anchors resolve: {findings:?}"
    );
}

/// Neutralising can merge two headings that were distinct, and a merge is the
/// one place a genuine finding could still be lost: rumdl appends `-1` to the
/// second of two equal slugs, so `#a-1-b-1` resolves only if that numbering
/// agrees with GitHub's. It does, and this pins it — a rumdl change to
/// duplicate-heading numbering would otherwise flip silently toward
/// suppressing a real finding.
#[test]
fn two_headings_that_merge_under_the_stand_in_still_resolve_both_anchors() {
    let findings = lint(
        "# T\n\n[a](#a-1-b)\n[b](#a-1-b-1)\n\n## A §1 B\n\n## A °1 B\n",
        false,
        &[],
        None,
    );

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "both anchors of a merged pair resolve: {findings:?}"
    );
}

/// The case AD-0018 records as unchanged, pinned so that "unchanged" is a
/// tested claim rather than a description. rumdl matches `#a-§1-b` against its
/// own slug for `## A §1 B` and reports nothing, so there is no first-pass
/// finding to filter — and the pass can only subtract.
#[test]
fn a_fragment_matching_rumdls_own_slug_stays_unreported() {
    assert!(
        lint("# T\n\n[l](#a-§1-b)\n\n## A §1 B\n\nx\n", false, &[], None)
            .iter()
            .all(|d| d.rule != "MD051"),
        "the mirror of the defect is out of reach of a subtract-only pass"
    );
}

/// The fail-safe [`stand_in_for`] provides: a document holding every candidate
/// leaves prim no character it can substitute without risking a collision, so
/// it suppresses nothing and the upstream false positive comes back. That is
/// the conservative direction — a finding the author can see and work around,
/// rather than a dead link silently swallowed.
#[test]
fn a_document_holding_every_stand_in_suppresses_nothing() {
    let candidates: String = SECTION_SIGN_STAND_INS.iter().collect();
    let source = format!("# T\n\n[l](#a-1-b)\n\n## A §1 B\n\nAll of {candidates} here.\n");

    assert_eq!(
        lint(&source, false, &[], None)
            .iter()
            .filter(|d| d.rule == "MD051")
            .count(),
        1,
        "with no safe stand-in left, prim reports rather than guesses"
    );
}

/// rumdl preserves the section sign only so its own `§emoji§` placeholder
/// survives the character pass, and rewrites that placeholder afterwards. A
/// heading holding both an author's section sign and a real emoji is the one
/// input where the two meet, and the one most likely to move under a rumdl
/// bump.
#[test]
fn a_heading_holding_both_a_section_sign_and_an_emoji_resolves_its_anchor() {
    let findings = lint(
        "# T\n\n[l](#a-1-b-)\n\n## A §1 B 🚀\n\nx\n",
        false,
        &[],
        None,
    );

    assert!(
        findings.iter().all(|d| d.rule != "MD051"),
        "the emoji path and the section sign do not interfere: {findings:?}"
    );
}
