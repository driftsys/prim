//! Every rule rumdl registers has a place in prim's census: selected into a
//! tier by `ACTIVE_RULES`, selected by `prim_mdlint_report_line_length`
//! (MD013), or in one of the three groups below that never run. The test
//! checks the code's lists against the rules rumdl registers, not the prose:
//! `docs/SPEC.md` and `docs/USAGE.md` print the same census, and whoever
//! edits a list here edits them too (the failure message says so). A rumdl
//! that adds a rule fails here until a record places it or a group names it;
//! a rumdl that drops one fails the other way.

use std::collections::BTreeSet;

use rumdl_lib::config::Config;
use rumdl_lib::rules::all_rules;

use super::super::{ACTIVE_RULES, LINE_LENGTH_RULE};
use super::rule_set;

/// Formatter territory: prim's own Markdown formatter decides these.
const FORMATTER_TERRITORY: &[&str] = &[
    "MD003", "MD004", "MD005", "MD007", "MD009", "MD010", "MD012", "MD018", "MD019", "MD020",
    "MD021", "MD022", "MD023", "MD027", "MD028", "MD029", "MD030", "MD031", "MD032", "MD035",
    "MD037", "MD038", "MD039", "MD046", "MD047", "MD048", "MD049", "MD050", "MD055", "MD058",
    "MD060", "MD064", "MD065", "MD071", "MD076", "MD077",
];

/// Off in both tiers by a record (AD-0012, AD-0013).
const OFF_IN_BOTH_TIERS: &[&str] = &[
    "MD014", "MD043", "MD044", "MD054", "MD057", "MD061", "MD063", "MD069", "MD072", "MD074",
    "MD078", "MD079", "MD081", "MD082",
];

/// Added by rumdl after the census was drawn (0.2.66); off until a record
/// places them (#194).
const UNPLACED_SINCE_CENSUS: &[&str] = &[
    "MD083", "MD084", "MD085", "MD086", "MD087", "MD088", "MD089", "MD091",
];

const PROSE: &str =
    "docs/SPEC.md § FR-5.5 and docs/USAGE.md print the same census: edit them with this file";

#[test]
fn every_rumdl_rule_is_selected_or_in_one_never_run_group() {
    let registered: BTreeSet<String> = all_rules(&Config::default())
        .iter()
        .map(|rule| rule.name().to_string())
        .collect();
    let selected: BTreeSet<String> = ACTIVE_RULES
        .iter()
        .map(|policy| policy.rule.to_string())
        .chain([LINE_LENGTH_RULE.to_string()])
        .collect();
    let groups = [
        rule_set(FORMATTER_TERRITORY),
        rule_set(OFF_IN_BOTH_TIERS),
        rule_set(UNPLACED_SINCE_CENSUS),
    ];

    let mut placed = selected;
    for group in &groups {
        let overlap: Vec<_> = placed.intersection(group).collect();
        assert!(overlap.is_empty(), "a rule sits in two groups: {overlap:?}");
        placed.extend(group.iter().cloned());
    }

    let unplaced: Vec<_> = registered.difference(&placed).collect();
    let vanished: Vec<_> = placed.difference(&registered).collect();
    assert!(
        unplaced.is_empty(),
        "rumdl registers rules prim's census does not place: {unplaced:?} ({PROSE})"
    );
    assert!(
        vanished.is_empty(),
        "prim's census names rules rumdl no longer registers: {vanished:?} ({PROSE})"
    );
}
