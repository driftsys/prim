use super::*;
use crate::Style;

mod rule_fixtures;

const DEFECT_RULES: [&str; 13] = [
    "MD042", "MD011", "MD052", "MD056", "MD062", "MD057", "MD034", "MD051", "MD045", "MD075",
    "MD066", "MD068", "MD070",
];

const CONVENTION_RULES: [&str; 13] = [
    "MD040", "MD041", "MD080", "MD024", "MD036", "MD025", "MD001", "MD026", "MD053", "MD033",
    "MD059", "MD073", "MD067",
];

const OPT_IN_RULES: [&str; 3] = ["MD013", "MD014", "MD069"];

/// A selection with nothing enabled and nothing disabled, at the given tier.
fn tier(strict: bool) -> MdLintSelection {
    MdLintSelection {
        strict,
        ..MdLintSelection::default()
    }
}

/// A selection at the floor tier with `rule` enabled.
fn enabling(rule: &str) -> MdLintSelection {
    MdLintSelection {
        strict: false,
        enabled: vec![rule.to_string()],
        disabled: Vec::new(),
    }
}

#[test]
fn defect_rules_run_in_both_tiers_and_conventions_only_in_strict() {
    for rule in DEFECT_RULES {
        assert!(is_active(rule, &tier(false)), "{rule} floor");
        assert!(is_active(rule, &tier(true)), "{rule} strict");
    }
    for rule in CONVENTION_RULES {
        assert!(!is_active(rule, &tier(false)), "{rule} floor");
        assert!(is_active(rule, &tier(true)), "{rule} strict");
    }
}

#[test]
fn opt_in_rules_run_only_when_enabled() {
    for rule in OPT_IN_RULES {
        assert!(!is_active(rule, &tier(false)), "{rule} floor");
        assert!(
            !is_active(rule, &tier(true)),
            "{rule} must stay off under prim_mdlint_strict — the strict tier is \
             prim's convention band, not everything prim can run"
        );
        assert!(is_active(rule, &enabling(rule)), "{rule} enabled");
    }
}

#[test]
fn enabling_reaches_a_convention_rule_from_the_floor_tier() {
    // The a-la-carte case: adopt one convention rule without the other twelve.
    assert!(is_active("MD033", &enabling("MD033")));
    assert!(
        !is_active("MD041", &enabling("MD033")),
        "enabling one convention rule must not pull in its band"
    );
}

#[test]
fn disabling_beats_enabling_for_the_same_rule() {
    let selection = MdLintSelection {
        strict: false,
        enabled: vec!["MD013".to_string()],
        disabled: vec!["md013".to_string()],
    };
    assert!(
        !is_active("MD013", &selection),
        "prim_mdlint_disable is applied after prim_mdlint_enable, so it wins"
    );
}

#[test]
fn withheld_rules_never_run_at_any_tier_or_enable() {
    // MD072 would reorder front-matter keys; MD082 was dropped by AD-0012;
    // MD063's only meaningful setting is a house-style choice prim will not
    // impose; MD003 and MD047 are formatter territory.
    for rule in ["MD072", "MD082", "MD063", "MD003", "MD047"] {
        assert!(!is_active(rule, &tier(false)), "{rule} floor");
        assert!(!is_active(rule, &tier(true)), "{rule} strict");
        assert!(!is_active(rule, &enabling(rule)), "{rule} enabled");
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
    let structure_floor = lint("Intro\n\n# Title\n", &Style::default(), &tier(false));
    assert!(
        structure_floor.iter().all(|d| d.rule != "MD041"),
        "convention rule stays off by default: {structure_floor:?}"
    );

    let structure_strict = lint("Intro\n\n# Title\n", &Style::default(), &tier(true));
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
    let floor = lint("![](hero.png)\n", &Style::default(), &tier(false));
    assert!(
        floor.iter().any(|d| d.rule == "MD045"),
        "MD045 runs at the floor tier: {floor:?}"
    );
    assert!(floor.iter().all(|d| d.is_error), "{floor:?}");
}

#[test]
fn a_disabled_rule_is_not_reported() {
    let src = "![](hero.png)\n";
    assert!(
        lint(src, &Style::default(), &tier(false))
            .iter()
            .any(|d| d.rule == "MD045")
    );
    let upper_disabled = MdLintSelection {
        disabled: vec!["MD045".to_string()],
        ..tier(false)
    };
    assert!(
        lint(src, &Style::default(), &upper_disabled).is_empty(),
        "exclusion silences the rule"
    );
    let lower_disabled = MdLintSelection {
        disabled: vec!["md045".to_string()],
        ..tier(false)
    };
    assert!(
        lint(src, &Style::default(), &lower_disabled).is_empty(),
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
        lint(src, &Style::default(), &tier(false))
            .iter()
            .all(|d| d.rule != "MD060" && d.rule != "MD013"),
        "formatter-territory and off rules stay disabled: {:?}",
        lint(src, &Style::default(), &tier(false))
    );
}

#[test]
fn reports_a_bare_url_with_real_line_col() {
    let src = "See https://example.com for details.\n";
    let diags = lint(src, &Style::default(), &tier(false));
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
        lint(src, &Style::default(), &tier(false)).is_empty(),
        "{:?}",
        lint(src, &Style::default(), &tier(false))
    );
}

#[test]
fn lint_never_mutates_source() {
    let src = "See https://example.com\n";
    let before = src.to_string();
    let _ = lint(src, &Style::default(), &tier(false));
    assert_eq!(src, before, "lint is read-only");
}

#[test]
fn file_level_directive_false_overrides_editorconfig_strict_true() {
    // MD041 is strict-only; the directive drops the file back to floor
    // even though the caller (an .editorconfig `prim_mdlint_strict =
    // true` glob) asked for strict.
    let src = "<!-- prim-mdlint-strict: false -->\nIntro\n\n# Title\n";
    let diags = lint(src, &Style::default(), &tier(true));
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
    let diags = lint(src, &Style::default(), &tier(false));
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
    let diags = lint(src, &Style::default(), &tier(false));
    assert!(
        diags.iter().all(|d| d.rule != "MD041"),
        "the later directive wins: {diags:?}"
    );
}

#[test]
fn directive_boolean_is_case_insensitive() {
    let src = "<!-- prim-mdlint-strict: TRUE -->\nIntro\n\n# Title\n";
    let diags = lint(src, &Style::default(), &tier(false));
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "TRUE is accepted: {diags:?}"
    );
}

#[test]
fn malformed_directive_value_is_ignored() {
    let src = "<!-- prim-mdlint-strict: yes -->\nIntro\n\n# Title\n";
    let diags = lint(src, &Style::default(), &tier(true));
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "a bad value falls back to the caller's strict setting: {diags:?}"
    );
}

#[test]
fn a_look_alike_comment_that_is_not_the_sole_line_content_is_ignored() {
    let src = "Some text <!-- prim-mdlint-strict: false --> more text\nIntro\n\n# Title\n";
    let diags = lint(src, &Style::default(), &tier(true));
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "an inline (non-standalone) comment is not a directive: {diags:?}"
    );
}

// MD025's front-matter-title fixture lives in `rule_fixtures` alongside the
// rest of the per-rule fixture table (issue #120).
