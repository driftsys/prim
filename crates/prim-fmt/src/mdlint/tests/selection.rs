//! Which rule objects `lint` builds for a tier, and the order its findings
//! come back in.

use super::super::{LINE_LENGTH_RULE, MdDiagnostic, build_rule, lint, prim_config, selected_rules};
use super::{CONVENTION_RULES, DEFECT_RULES};

/// The built rules' names, sorted but not deduplicated: a name built twice
/// would show as a duplicate.
fn names(strict: bool, disabled: &[String], line_length: Option<usize>) -> Vec<String> {
    let cfg = prim_config(strict, line_length);
    let mut names: Vec<String> = selected_rules(&cfg, strict, disabled, line_length)
        .iter()
        .map(|rule| rule.name().to_string())
        .collect();
    names.sort();
    names
}

fn sorted<'a>(rules: impl IntoIterator<Item = &'a &'a str>) -> Vec<String> {
    let mut names: Vec<String> = rules.into_iter().map(|rule| rule.to_string()).collect();
    names.sort();
    names
}

/// The tier table is the only source of the selection: the floor at the
/// floor tier, both bands at strict, MD013 only with a width, and never a
/// name `prim_mdlint_disable` removed — each name built exactly once.
#[test]
fn lint_builds_exactly_the_rules_the_tier_selects() {
    assert_eq!(names(false, &[], None), sorted(&DEFECT_RULES));

    let both: Vec<&str> = DEFECT_RULES
        .iter()
        .chain(&CONVENTION_RULES)
        .copied()
        .collect();
    assert_eq!(names(true, &[], None), sorted(&both));

    let with_width: Vec<&str> = both.iter().copied().chain([LINE_LENGTH_RULE]).collect();
    assert_eq!(names(true, &[], Some(80)), sorted(&with_width));

    let disabled = ["MD051".to_string()];
    let without: Vec<&str> = DEFECT_RULES
        .iter()
        .copied()
        .filter(|r| *r != "MD051")
        .collect();
    assert_eq!(names(false, &disabled, None), sorted(&without));
}

/// A name the pinned rumdl cannot build is a defect in the tier table or the
/// pin, and it fails loudly rather than leaving a gate one rule short.
#[test]
#[should_panic(expected = "MD999 is in prim's tier table")]
fn a_rule_the_pinned_rumdl_cannot_build_panics() {
    let _ = build_rule("MD999", &prim_config(false, None));
}

/// Findings come back in file order, whatever order the rules ran in. rumdl
/// returns each rule's findings as a block in rule order, so the document
/// that tells the two apart trips three rules whose file order matches
/// neither rule-number order (MD011 < MD034 < MD042) nor the tier table's
/// (MD042 before MD011 before MD034): a bare URL on line 3, an empty link on
/// line 5, a reversed link on line 7.
#[test]
fn findings_are_ordered_by_position_not_by_rule() {
    let findings = lint(
        "# T\n\nhttp://x.example\n\n[empty]()\n\n(text)[http://y]\n\nz\n",
        false,
        &[],
        None,
    );
    let order: Vec<(usize, &str)> = findings
        .iter()
        .map(|d: &MdDiagnostic| (d.line, d.rule.as_str()))
        .collect();
    assert_eq!(
        order,
        [(3, "MD034"), (5, "MD042"), (7, "MD011")],
        "{findings:?}"
    );
}
