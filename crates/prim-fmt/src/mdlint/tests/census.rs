//! Every rule rumdl registers has a place in prim's census: selected into a
//! tier by `ACTIVE_RULES`, selected by `prim_mdlint_report_line_length`
//! (MD013), or listed here as never run. `docs/SPEC.md` and `docs/USAGE.md`
//! print the same census in prose; this test is what keeps that prose true
//! across a rumdl bump. A rumdl that adds a rule fails here until a record
//! places it or this list names it; a rumdl that drops one fails the other
//! way.

use std::collections::BTreeSet;

use rumdl_lib::config::Config;
use rumdl_lib::rules::all_rules;

use super::super::{ACTIVE_RULES, LINE_LENGTH_RULE};

/// The rules prim never runs, in the SPEC's own groups.
const NEVER_RUN: &[&str] = &[
    // Formatter territory: prim's own Markdown formatter decides these.
    "MD003", "MD004", "MD005", "MD007", "MD009", "MD010", "MD012", "MD018", "MD019", "MD020",
    "MD021", "MD022", "MD023", "MD027", "MD028", "MD029", "MD030", "MD031", "MD032", "MD035",
    "MD037", "MD038", "MD039", "MD046", "MD047", "MD048", "MD049", "MD050", "MD055", "MD058",
    "MD060", "MD064", "MD065", "MD071", "MD076", "MD077",
    // Off in both tiers (AD-0012, AD-0013).
    "MD014", "MD043", "MD044", "MD054", "MD057", "MD061", "MD063", "MD069", "MD072", "MD074",
    "MD078", "MD079", "MD081", "MD082",
    // Added by rumdl after the census was drawn (0.2.66); off until a record
    // places them.
    "MD083", "MD084", "MD085", "MD086", "MD087", "MD088", "MD089", "MD091",
];

#[test]
fn every_rumdl_rule_is_selected_or_listed_as_never_run() {
    let registered: BTreeSet<String> = all_rules(&Config::default())
        .iter()
        .map(|rule| rule.name().to_string())
        .collect();
    let placed: BTreeSet<String> = ACTIVE_RULES
        .iter()
        .map(|policy| policy.rule.to_string())
        .chain([LINE_LENGTH_RULE.to_string()])
        .chain(NEVER_RUN.iter().map(|rule| rule.to_string()))
        .collect();

    let unplaced: Vec<_> = registered.difference(&placed).collect();
    let vanished: Vec<_> = placed.difference(&registered).collect();
    assert!(
        unplaced.is_empty(),
        "rumdl registers rules prim's census does not place: {unplaced:?}"
    );
    assert!(
        vanished.is_empty(),
        "prim's census names rules rumdl no longer registers: {vanished:?}"
    );
}
