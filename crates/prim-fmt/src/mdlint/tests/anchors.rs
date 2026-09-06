//! The anchor slug rumdl computes, pinned through `lint` so a pin move that
//! changes it fails here (rvben/rumdl#854, AD-0018).
//!
//! GitHub's anchor filter keeps word characters, hyphens and spaces, drops
//! every other character, and turns each space into a hyphen. rumdl 0.2.35
//! retained `§` and counted one hyphen too many around an emoji not surrounded
//! by spaces; prim carried a workaround for the first until rumdl 0.2.66 fixed
//! both. MD051 and MD080 resolve a heading through the anchor style `FLAVOR`
//! selects, and MD073 through GitHub's slug whatever the flavor; all three
//! moved with the upstream fix, and each is pinned here in the direction a
//! regression would reverse.

use super::super::{MdDiagnostic, lint};

fn named(findings: &[MdDiagnostic], rule: &str) -> usize {
    findings.iter().filter(|d| d.rule == rule).count()
}

fn only<'a>(findings: &'a [MdDiagnostic], rule: &str) -> &'a MdDiagnostic {
    let mut matching = findings.iter().filter(|d| d.rule == rule);
    let first = matching
        .next()
        .unwrap_or_else(|| panic!("no {rule}: {findings:?}"));
    assert!(
        matching.next().is_none(),
        "more than one {rule}: {findings:?}"
    );
    first
}

/// `#a-1-b` is the anchor a browser resolves for `A §1 B`, in the ATX and the
/// setext form alike. rumdl 0.2.35 computed `a-§1-b` and reported the link.
#[test]
fn a_section_sign_heading_resolves_the_anchor_github_produces() {
    let atx = lint("# T\n\n[l](#a-1-b)\n\n## A §1 B\n\nx\n", false, &[], None);
    assert_eq!(named(&atx, "MD051"), 0, "ATX: {atx:?}");

    let setext = lint(
        "# T\n\n[l](#a-1-b)\n\nA §1 B\n------\n\nx\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&setext, "MD051"), 0, "setext: {setext:?}");
}

/// The mirror: `#a-§1-b` resolves no anchor a renderer produces. rumdl 0.2.35
/// accepted it because its own slug retained the character.
#[test]
fn a_fragment_holding_a_section_sign_is_reported() {
    let findings = lint("# T\n\n[l](#a-§1-b)\n\n## A §1 B\n\nx\n", false, &[], None);
    let finding = only(&findings, "MD051");
    assert_eq!((finding.line, finding.column), (3, 1), "{finding:?}");
    assert!(finding.message.contains("#a-§1-b"), "{finding:?}");
}

/// Two headings whose slugs become equal get GitHub's `-1` suffix on the
/// second, so both `#a-1-b` and `#a-1-b-1` resolve and a `-2` that no heading
/// earned still reports. This is the one place a stripped character can
/// change which heading a fragment reaches.
#[test]
fn two_headings_with_the_same_slug_number_the_second() {
    let findings = lint(
        "# T\n\n[a](#a-1-b)\n[b](#a-1-b-1)\n[c](#a-1-b-2)\n\n## A §1 B\n\n## A °1 B\n",
        false,
        &[],
        None,
    );
    let finding = only(&findings, "MD051");
    assert_eq!(finding.line, 5, "{finding:?}");
    assert!(finding.message.contains("#a-1-b-2"), "{finding:?}");
}

/// An explicit `<a id="...">` is an anchor MD051 matches as a literal, with
/// no slug involved: the exact id resolves and any other fragment reports.
#[test]
fn an_explicit_html_anchor_resolves_its_exact_id() {
    let exact = lint(
        "# T\n\n[l](#custom-id)\n\n<a id=\"custom-id\"></a>\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&exact, "MD051"), 0, "{exact:?}");

    let other = lint(
        "# T\n\n[l](#custom)\n\n<a id=\"custom-id\"></a>\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&other, "MD051"), 1, "{other:?}");
}

/// `## A🚀 B` slugs to `a-b`: the emoji is deleted and the one space becomes
/// the one hyphen. rumdl 0.2.35 produced `a--b`, so a correct link was
/// reported and a link written to the wrong slug was accepted.
#[test]
fn an_emoji_glued_to_a_word_adds_no_hyphen() {
    let correct = lint("# T\n\n[l](#a-b)\n\n## A🚀 B\n\nx\n", false, &[], None);
    assert_eq!(named(&correct, "MD051"), 0, "{correct:?}");

    let old_slug = lint("# T\n\n[l](#a--b)\n\n## A🚀 B\n\nx\n", false, &[], None);
    assert_eq!(named(&old_slug, "MD051"), 1, "{old_slug:?}");
}

/// A heading holding both a section sign and an emoji exercises both parts
/// of the upstream fix together.
#[test]
fn a_heading_holding_both_a_section_sign_and_an_emoji_resolves_its_anchor() {
    let findings = lint(
        "# T\n\n[l](#a-1-b-)\n\n## A §1 B 🚀\n\nx\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&findings, "MD051"), 0, "{findings:?}");
}

/// MD080 (strict tier) compares headings by the same slug: two headings that
/// differ only by a section sign now collide on `a-1-b`, where rumdl 0.2.35
/// kept them apart as `a-§1-b` and `a-1-b`.
#[test]
fn md080_reports_two_headings_that_differ_only_by_a_section_sign() {
    let findings = lint("# T\n\n## A §1 B\n\nx\n\n## A 1 B\n\ny\n", true, &[], None);
    let finding = only(&findings, "MD080");
    assert_eq!(finding.line, 7, "{finding:?}");
    assert!(finding.message.contains("a-1-b"), "{finding:?}");
}

/// MD073 (strict tier) validates a TOC entry against the same slug: an entry
/// written to `#a-1-b` for `## A §1 B` was reported as both stale and missing
/// under rumdl 0.2.35 and is accepted now.
#[test]
fn md073_accepts_a_toc_entry_to_a_section_sign_heading() {
    let findings = lint(
        "# T\n\n<!-- toc -->\n- [A §1 B](#a-1-b)\n<!-- tocstop -->\n\n## A §1 B\n\nx\n",
        true,
        &[],
        None,
    );
    assert_eq!(named(&findings, "MD073"), 0, "{findings:?}");
}

/// `FLAVOR` is `Standard`. Each other flavor changes what some rule accepts,
/// and a swap to most of them passes the rest of the crate, so the constant
/// is pinned by the cases that differ: `MkDocs` exempts a `#fn:` footnote
/// fragment from MD051, and `Quarto` and `MyST` exempt a directive fence
/// from MD040. Under `Standard` each is reported.
#[test]
fn rules_run_under_the_standard_flavor() {
    let footnote = lint("# T\n\n[l](#fn:note)\n\nx\n", false, &[], None);
    assert_eq!(named(&footnote, "MD051"), 1, "{footnote:?}");

    let directive = lint("# T\n\n```{note}\nx\n```\n\ny\n", true, &[], None);
    assert_eq!(named(&directive, "MD040"), 1, "{directive:?}");
}
