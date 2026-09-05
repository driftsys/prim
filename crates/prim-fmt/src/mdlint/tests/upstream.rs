//! Behaviour that moved with the rumdl 0.2.35 → 0.2.66 bump (#193) outside
//! the anchor slug, pinned in the direction a regression would reverse. Each
//! is named in the release note of that bump.

use super::super::{MdDiagnostic, lint};

fn named(findings: &[MdDiagnostic], rule: &str) -> Vec<usize> {
    findings
        .iter()
        .filter(|d| d.rule == rule)
        .map(|d| d.line)
        .collect()
}

/// MD034 (floor tier) reports a bare email address even when the file holds
/// no other link-like text. rumdl 0.2.35 skipped such a file before the rule
/// ran, by a category prefilter that looked only for `[`, a URL scheme or
/// `www.`; 0.2.66 lets MD034 decide for itself.
#[test]
fn md034_reports_a_bare_email_in_a_file_with_no_other_link() {
    let findings = lint("# Title\n\nContact: alice@example.com\n", false, &[], None);
    assert_eq!(named(&findings, "MD034"), [3], "{findings:?}");
}

/// MD026 (strict tier) no longer reads the `;` that closes an HTML entity as
/// trailing punctuation; a heading that really ends in punctuation still
/// reports, so the silence is the rule running and passing.
#[test]
fn md026_ignores_the_delimiter_of_an_html_entity() {
    let entity = lint("# T\n\n## Great news &amp;\n\nx\n", true, &[], None);
    assert_eq!(named(&entity, "MD026"), [] as [usize; 0], "{entity:?}");

    let period = lint("# T\n\n## Great news.\n\nx\n", true, &[], None);
    assert_eq!(named(&period, "MD026"), [3], "{period:?}");
}
