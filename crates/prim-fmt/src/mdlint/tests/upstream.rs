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

/// MD052 (floor tier) reports a dangling reference link inside a table cell.
/// rumdl 0.2.35 skipped references in table cells altogether.
#[test]
fn md052_reports_a_dangling_reference_inside_a_table_cell() {
    let findings = lint(
        "# T\n\n| a | b |\n| - | - |\n| [x][undefined] | y |\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&findings, "MD052"), [5], "{findings:?}");
}

/// MD075 (floor tier) no longer reads a prose line holding `|` after a table
/// as an orphaned row; a row-shaped line after prose, with no table before
/// it, still reports.
#[test]
fn md075_ignores_prose_holding_a_pipe_after_a_table() {
    let prose = lint(
        "# T\n\n| a | b |\n| - | - |\n| 1 | 2 |\n\nNoise low|med|high.\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&prose, "MD075"), [] as [usize; 0], "{prose:?}");

    let orphan = lint(
        "# T\n\nSome text.\n\n| value1 | description1 |\n| value2 | description2 |\n",
        false,
        &[],
        None,
    );
    assert_eq!(named(&orphan, "MD075"), [5], "{orphan:?}");
}

/// MD045 (floor tier) no longer reports an Obsidian embed `![[image.png]]`,
/// which has no alt-text syntax; an image with empty alt text still reports.
#[test]
fn md045_ignores_a_wiki_link_embed() {
    let embed = lint("# T\n\n![[image.png]]\n", false, &[], None);
    assert_eq!(named(&embed, "MD045"), [] as [usize; 0], "{embed:?}");

    let image = lint("# T\n\n![](hero.png)\n", false, &[], None);
    assert_eq!(named(&image, "MD045"), [3], "{image:?}");
}
