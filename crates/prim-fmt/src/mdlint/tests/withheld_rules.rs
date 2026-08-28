//! Proof that the rules prim withholds because they *cannot fire* really
//! cannot fire under the pinned `rumdl = "=0.2.35"`, the `Standard` flavor
//! prim pins, and the `source_file: None` prim passes.
//!
//! Each claim below is an assumption about a dependency, not a property prim
//! controls. Without this module a rumdl bump could make one of these rules
//! start reporting, and `prim_mdlint_enable` would silently accept an id whose
//! documented reason for refusal had stopped being true.
//!
//! These rules cannot be reached through `lint`, which is the point, so the
//! fixtures call `rumdl_lib::lint` directly with prim's own config.

use rumdl_lib::config::MarkdownFlavor;
use rumdl_lib::rules::all_rules;

use super::super::{RuleReach, prim_config, rule_reach};
use crate::Style;

/// One withheld rule, why it cannot fire, and input that would trigger it if
/// the reason stopped being true.
struct WithheldRule {
    rule: &'static str,
    reason: &'static str,
    src: &'static str,
}

const CANNOT_FIRE: &[WithheldRule] = &[
    WithheldRule {
        rule: "MD043",
        reason: "needs a repository-supplied `headings` list; it defaults empty",
        src: "# One\n\n## Two\n",
    },
    WithheldRule {
        rule: "MD044",
        reason: "needs a repository-supplied `names` list; it defaults empty",
        src: "Writing javascript and github in lower case.\n",
    },
    WithheldRule {
        rule: "MD054",
        reason: "all six link-style booleans default to allowed, so no style is forbidden",
        src: "[inline](https://example.com) and <https://example.com>\n",
    },
    WithheldRule {
        rule: "MD061",
        reason: "needs a repository-supplied `terms` list; it defaults empty",
        src: "We should blacklist that host.\n",
    },
    WithheldRule {
        rule: "MD081",
        reason: "`max-per-paragraph` and `max-consecutive` both default to unset",
        src: "**a** **b** **c** **d** **e** **f** **g** **h**\n",
    },
    WithheldRule {
        rule: "MD074",
        reason: "MkDocs flavor only, and it then needs a source_file to find mkdocs.yml",
        src: "# Page\n\nText.\n",
    },
    WithheldRule {
        rule: "MD078",
        reason: "Quarto flavor only",
        src: "```{r}\n1 + 1\n```\n",
    },
    WithheldRule {
        rule: "MD079",
        reason: "Quarto flavor only",
        src: "```{r my chunk}\n1 + 1\n```\n",
    },
];

/// Select one rule out of rumdl's registry by name, under prim's config.
fn rule_named(rule: &str, cfg: &rumdl_lib::config::Config) -> Vec<Box<dyn rumdl_lib::rule::Rule>> {
    let selected: Vec<_> = all_rules(cfg)
        .into_iter()
        .filter(|known| known.name() == rule)
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "{rule} is not in rumdl's registry under the pinned version"
    );
    selected
}

#[test]
fn withheld_rules_that_cannot_fire_report_nothing() {
    let cfg = prim_config(&Style::default());
    for case in CANNOT_FIRE {
        let warnings = rumdl_lib::lint(
            case.src,
            &rule_named(case.rule, &cfg),
            false,
            MarkdownFlavor::Standard,
            None,
            Some(&cfg),
        )
        .expect("rumdl lint");
        assert!(
            warnings.is_empty(),
            "{} fired, so its documented reason for being withheld — {} — is no \
             longer true: {warnings:?}",
            case.rule,
            case.reason
        );
        assert_eq!(
            rule_reach(case.rule),
            RuleReach::Withheld,
            "{} must stay unreachable through prim_mdlint_enable",
            case.rule
        );
    }
}

#[test]
fn md063_is_withheld_by_choice_rather_than_by_construction() {
    // MD063Config carries `enabled: bool`, documented as opt-in and defaulting
    // to false — but the field is read nowhere in rumdl 0.2.35 outside its own
    // config test, so the rule fires whenever it is selected. It is withheld
    // because its only meaningful setting is sentence case versus title case,
    // a house-style choice prim has no surface to let a repository express and
    // will not impose. If a future rumdl honours `enabled`, this test fails and
    // the reason recorded in AD-0012 has to be rewritten.
    let cfg = prim_config(&Style::default());
    let warnings = rumdl_lib::lint(
        "# this heading is not capitalised\n",
        &rule_named("MD063", &cfg),
        false,
        MarkdownFlavor::Standard,
        None,
        Some(&cfg),
    )
    .expect("rumdl lint");
    assert!(
        !warnings.is_empty(),
        "MD063 stopped firing at its defaults; it is now withheld by \
         construction rather than by choice"
    );
    assert_eq!(rule_reach("MD063"), RuleReach::Withheld);
}
