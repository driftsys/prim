use super::*;

mod rule_fixtures;

const DEFECT_RULES: [&str; 12] = [
    "MD042", "MD011", "MD052", "MD056", "MD062", "MD034", "MD051", "MD045", "MD075", "MD066",
    "MD068", "MD070",
];

const CONVENTION_RULES: [&str; 13] = [
    "MD040", "MD041", "MD080", "MD024", "MD036", "MD025", "MD001", "MD026", "MD053", "MD033",
    "MD059", "MD073", "MD067",
];

#[test]
fn defect_rules_run_in_both_tiers_and_conventions_only_in_strict() {
    for rule in DEFECT_RULES {
        assert!(is_active(rule, false), "{rule} floor");
        assert!(is_active(rule, true), "{rule} strict");
    }
    for rule in CONVENTION_RULES {
        assert!(!is_active(rule, false), "{rule} floor");
        assert!(is_active(rule, true), "{rule} strict");
    }
}

#[test]
fn dropped_and_formatter_territory_rules_never_run() {
    // MD082 was dropped (measured across six public documentation sites —
    // React Native, FastAPI, Vue, Redux, Vite, Building Secure Contracts —
    // 569 of 573 MD082 findings flag a parent heading followed by a
    // deeper one; 4 flag a genuinely empty section). MD057 was dropped
    // because a file-existence check is the wrong question at this layer
    // (AD-0013). The rest are formatter territory or off.
    for rule in [
        "MD082", "MD057", "MD013", "MD060", "MD072", "MD003", "MD047",
    ] {
        assert!(!is_active(rule, false), "{rule} floor");
        assert!(!is_active(rule, true), "{rule} strict");
    }
}

#[test]
fn is_known_rule_covers_both_bands_case_insensitively() {
    assert!(is_known_rule("MD045"));
    assert!(is_known_rule("md033"));
    assert!(!is_known_rule("MD082"));
    assert!(!is_known_rule("MD999"));
}

#[test]
fn strict_only_rules_stay_off_at_the_floor_tier() {
    let structure_floor = lint("Intro\n\n# Title\n", false, &[]);
    assert!(
        structure_floor.iter().all(|d| d.rule != "MD041"),
        "convention rule stays off by default: {structure_floor:?}"
    );

    let structure_strict = lint("Intro\n\n# Title\n", true, &[]);
    let first_line_heading = structure_strict
        .iter()
        .find(|d| d.rule == "MD041")
        .expect("MD041 enabled in strict");
    assert!(
        first_line_heading.is_error,
        "every reported finding is an error: {structure_strict:?}"
    );
}

#[test]
fn every_reported_finding_is_an_error() {
    let floor = lint("![](hero.png)\n", false, &[]);
    assert!(
        floor.iter().any(|d| d.rule == "MD045"),
        "MD045 runs at the floor tier: {floor:?}"
    );
    assert!(floor.iter().all(|d| d.is_error), "{floor:?}");
}

#[test]
fn a_disabled_rule_is_not_reported() {
    let src = "![](hero.png)\n";
    assert!(lint(src, false, &[]).iter().any(|d| d.rule == "MD045"));
    assert!(
        lint(src, false, &["MD045".to_string()]).is_empty(),
        "exclusion silences the rule"
    );
    assert!(
        lint(src, false, &["md045".to_string()]).is_empty(),
        "rule ids match case-insensitively"
    );
}

#[test]
fn never_linted_and_off_rules_stay_excluded() {
    let src = "\
| a | bb |
| c | d |

This is an intentionally long line that would violate line-length linting if prim enabled MD013 for Markdown content checks.\n";
    assert!(
        lint(src, false, &[])
            .iter()
            .all(|d| d.rule != "MD060" && d.rule != "MD013"),
        "formatter-territory and off rules stay disabled: {:?}",
        lint(src, false, &[])
    );
}

#[test]
fn reports_a_bare_url_with_real_line_col() {
    let src = "See https://example.com for details.\n";
    let diags = lint(src, false, &[]);
    let bare = diags
        .iter()
        .find(|d| d.rule == "MD034")
        .expect("MD034 bare-url reported");
    assert_eq!(bare.line, 1, "1-indexed line: {diags:?}");
    assert!(bare.column >= 1, "1-indexed column: {diags:?}");
}

#[test]
fn clean_markdown_yields_no_findings() {
    let src = "# Title\n\nSome prose with a [link](https://example.com).\n";
    assert!(
        lint(src, false, &[]).is_empty(),
        "{:?}",
        lint(src, false, &[])
    );
}

#[test]
fn lint_never_mutates_source() {
    let src = "See https://example.com\n";
    let before = src.to_string();
    let _ = lint(src, false, &[]);
    assert_eq!(src, before, "lint is read-only");
}

#[test]
fn file_level_directive_false_overrides_editorconfig_strict_true() {
    // MD041 is strict-only; the directive drops the file back to floor
    // even though the caller (an .editorconfig `prim_mdlint_strict =
    // true` glob) asked for strict.
    let src = "<!-- prim-mdlint-strict: false -->\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[]);
    assert!(
        diags.iter().all(|d| d.rule != "MD041"),
        "directive drops the file to floor: {diags:?}"
    );
}

#[test]
fn file_level_directive_true_overrides_editorconfig_strict_false() {
    // MD041 is strict-only; the directive raises this file to strict even
    // though the caller (an .editorconfig floor-tier glob) asked for
    // floor.
    let src = "<!-- prim-mdlint-strict: true -->\nIntro\n\n# Title\n";
    let diags = lint(src, false, &[]);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "directive raises the file to strict: {diags:?}"
    );
}

#[test]
fn last_file_level_directive_wins_when_several_are_present() {
    let src = "<!-- prim-mdlint-strict: true -->\n\
                    Intro\n\n# Title\n\n\
                    <!-- prim-mdlint-strict: false -->\n";
    let diags = lint(src, false, &[]);
    assert!(
        diags.iter().all(|d| d.rule != "MD041"),
        "the later directive wins: {diags:?}"
    );
}

#[test]
fn directive_boolean_is_case_insensitive() {
    let src = "<!-- prim-mdlint-strict: TRUE -->\nIntro\n\n# Title\n";
    let diags = lint(src, false, &[]);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "TRUE is accepted: {diags:?}"
    );
}

#[test]
fn malformed_directive_value_is_ignored() {
    let src = "<!-- prim-mdlint-strict: yes -->\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[]);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "a bad value falls back to the caller's strict setting: {diags:?}"
    );
}

#[test]
fn a_look_alike_comment_that_is_not_the_sole_line_content_is_ignored() {
    let src = "Some text <!-- prim-mdlint-strict: false --> more text\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[]);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "an inline (non-standalone) comment is not a directive: {diags:?}"
    );
}

// MD025's front-matter-title fixture lives in `rule_fixtures` alongside the
// rest of the per-rule fixture table (issue #120).
