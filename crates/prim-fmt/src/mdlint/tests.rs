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
        assert!(is_active(rule, false, None), "{rule} floor");
        assert!(is_active(rule, true, None), "{rule} strict");
    }
    for rule in CONVENTION_RULES {
        assert!(!is_active(rule, false, None), "{rule} floor");
        assert!(is_active(rule, true, None), "{rule} strict");
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
        assert!(!is_active(rule, false, None), "{rule} floor");
        assert!(!is_active(rule, true, None), "{rule} strict");
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
    let structure_floor = lint("Intro\n\n# Title\n", false, &[], None);
    assert!(
        structure_floor.iter().all(|d| d.rule != "MD041"),
        "convention rule stays off by default: {structure_floor:?}"
    );

    let structure_strict = lint("Intro\n\n# Title\n", true, &[], None);
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
    let floor = lint("![](hero.png)\n", false, &[], None);
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
        lint(src, false, &[], None)
            .iter()
            .any(|d| d.rule == "MD045")
    );
    assert!(
        lint(src, false, &["MD045".to_string()], None).is_empty(),
        "exclusion silences the rule"
    );
    assert!(
        lint(src, false, &["md045".to_string()], None).is_empty(),
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
        lint(src, false, &[], None)
            .iter()
            .all(|d| d.rule != "MD060" && d.rule != "MD013"),
        "formatter-territory and off rules stay disabled: {:?}",
        lint(src, false, &[], None)
    );
}

#[test]
fn reports_a_bare_url_with_real_line_col() {
    let src = "See https://example.com for details.\n";
    let diags = lint(src, false, &[], None);
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
        lint(src, false, &[], None).is_empty(),
        "{:?}",
        lint(src, false, &[], None)
    );
}

#[test]
fn lint_never_mutates_source() {
    let src = "See https://example.com\n";
    let before = src.to_string();
    let _ = lint(src, false, &[], None);
    assert_eq!(src, before, "lint is read-only");
}

#[test]
fn file_level_directive_false_overrides_editorconfig_strict_true() {
    // MD041 is strict-only; the directive drops the file back to floor
    // even though the caller (an .editorconfig `prim_mdlint_strict =
    // true` glob) asked for strict.
    let src = "<!-- prim-mdlint-strict: false -->\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[], None);
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
    let diags = lint(src, false, &[], None);
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
    let diags = lint(src, false, &[], None);
    assert!(
        diags.iter().all(|d| d.rule != "MD041"),
        "the later directive wins: {diags:?}"
    );
}

#[test]
fn directive_boolean_is_case_insensitive() {
    let src = "<!-- prim-mdlint-strict: TRUE -->\nIntro\n\n# Title\n";
    let diags = lint(src, false, &[], None);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "TRUE is accepted: {diags:?}"
    );
}

#[test]
fn malformed_directive_value_is_ignored() {
    let src = "<!-- prim-mdlint-strict: yes -->\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[], None);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "a bad value falls back to the caller's strict setting: {diags:?}"
    );
}

#[test]
fn a_look_alike_comment_that_is_not_the_sole_line_content_is_ignored() {
    let src = "Some text <!-- prim-mdlint-strict: false --> more text\nIntro\n\n# Title\n";
    let diags = lint(src, true, &[], None);
    assert!(
        diags.iter().any(|d| d.rule == "MD041"),
        "an inline (non-standalone) comment is not a directive: {diags:?}"
    );
}

// MD025's front-matter-title fixture lives in `rule_fixtures` alongside the
// rest of the per-rule fixture table (issue #120).

// MD013 (`prim_mdlint_report_line_length`). It sits outside the tier model:
// the resolved line length decides whether it runs at all, and the tier
// decides only whether it looks at headings.

const LONG_PROSE: &str = "This prose line is written past the eighty column budget on purpose so that MD013 reports it.\n";

#[test]
fn md013_runs_only_when_a_line_length_is_resolved() {
    assert!(!is_active("MD013", false, None), "floor, key unset");
    assert!(!is_active("MD013", true, None), "strict, key unset");
    assert!(is_active("MD013", false, Some(80)), "floor, key set");
    assert!(is_active("MD013", true, Some(80)), "strict, key set");
}

#[test]
fn md013_is_a_known_rule_so_disabling_it_is_not_a_typo() {
    assert!(is_known_rule("MD013"));
    assert!(is_known_rule("md013"));
}

#[test]
fn md013_reports_long_prose_only_once_a_limit_is_given() {
    assert!(
        lint(LONG_PROSE, false, &[], None)
            .iter()
            .all(|d| d.rule != "MD013"),
        "silent with no limit"
    );
    assert!(
        lint(LONG_PROSE, false, &[], Some(80))
            .iter()
            .any(|d| d.rule == "MD013"),
        "reported at 80"
    );
    assert!(
        lint(LONG_PROSE, false, &[], Some(120))
            .iter()
            .all(|d| d.rule != "MD013"),
        "silent at 120: the line is shorter than the resolved limit"
    );
}

#[test]
fn md013_headings_follow_the_tier() {
    let heading = "# A heading written past the eighty column budget so MD013 could report it\n";
    assert!(
        lint(heading, false, &[], Some(60))
            .iter()
            .all(|d| d.rule != "MD013"),
        "floor tier leaves a long heading alone"
    );
    assert!(
        lint(heading, true, &[], Some(60))
            .iter()
            .any(|d| d.rule == "MD013"),
        "strict tier reports it"
    );
}

#[test]
fn md013_headings_follow_a_file_level_tier_override() {
    // The `<!-- prim-mdlint-strict -->` directive picks the tier before the
    // rule options are built, so it moves the heading check with it.
    let promoted = "<!-- prim-mdlint-strict: true -->\n\n# A heading written past the eighty column budget so MD013 reports it\n";
    assert!(
        lint(promoted, false, &[], Some(60))
            .iter()
            .any(|d| d.rule == "MD013"),
        "a floor-tier file promoted to strict reports its long heading"
    );
}

#[test]
fn md013_is_subtract_only_like_every_other_rule() {
    assert!(
        lint(LONG_PROSE, false, &["MD013".to_string()], Some(80))
            .iter()
            .all(|d| d.rule != "MD013"),
        "prim_mdlint_disable removes it again"
    );
}

#[test]
fn md013_headings_follow_a_file_level_tier_demotion() {
    // The mirror of the promotion case: the directive moves the heading check
    // in both directions, so a strict-tier file can opt out of it.
    let demoted = "<!-- prim-mdlint-strict: false -->\n\n# A heading written past the eighty column budget so MD013 could report it\n";
    assert!(
        lint(demoted, true, &[], Some(60))
            .iter()
            .all(|d| d.rule != "MD013"),
        "a strict-tier file demoted to floor leaves its long heading alone"
    );
}

#[test]
fn md013_line_length_is_written_to_both_places_rumdl_reads() {
    // rumdl picks between its global `line_length` and MD013's own key with a
    // sentinel check — it overwrites the rule value whenever that value still
    // equals the default 80. Writing the same limit to both makes every branch
    // of that check agree, and keeps prim correct if the precedence changes.
    // Under one rumdl version either write alone resolves the same, so nothing
    // behavioural can pin this; only the config prim builds can.
    for limit in [80, 120] {
        let cfg = prim_config(false, Some(limit));
        assert_eq!(
            cfg.global.line_length.get(),
            limit,
            "global line_length at {limit}"
        );
        assert_eq!(
            cfg.rules["MD013"].values["line-length"],
            toml::Value::Integer(limit as i64),
            "MD013 line-length at {limit}"
        );
    }
}

#[test]
fn md013_carries_exactly_the_five_options_prim_owns() {
    // A sixth option leaking in, or one of these silently dropping out, would
    // change what prim reports without any behavioural test noticing.
    let cfg = prim_config(true, Some(80));
    let values = &cfg.rules["MD013"].values;
    let mut keys: Vec<_> = values.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "code-blocks",
            "code-spans",
            "headings",
            "line-length",
            "tables"
        ]
    );
    assert_eq!(values["code-blocks"], toml::Value::Boolean(false));
    assert_eq!(values["code-spans"], toml::Value::Boolean(false));
    assert_eq!(values["tables"], toml::Value::Boolean(false));
    assert_eq!(
        values["headings"],
        toml::Value::Boolean(true),
        "strict tier"
    );
    assert_eq!(
        prim_config(false, Some(80)).rules["MD013"].values["headings"],
        toml::Value::Boolean(false),
        "floor tier"
    );
}

#[test]
fn md013_is_absent_from_the_config_when_it_does_not_run() {
    assert!(!prim_config(true, None).rules.contains_key("MD013"));
}

#[test]
fn md013_clamps_a_line_length_rumdl_would_mishandle() {
    // 0 means "no limit" to rumdl, but the formatter wraps to one word per
    // line, so the two would disagree about a file every line of which is too
    // long. Above i64::MAX the TOML conversion wraps negative, rumdl rejects
    // the rule config wholesale, and every option prim pins here reverts.
    assert_eq!(prim_config(false, Some(0)).global.line_length.get(), 1);
    assert_eq!(
        prim_config(false, Some(0)).rules["MD013"].values["line-length"],
        toml::Value::Integer(1)
    );
    let huge = prim_config(false, Some(usize::MAX));
    assert_eq!(
        huge.rules["MD013"].values["line-length"],
        toml::Value::Integer(i64::MAX)
    );
    assert!(
        !lint("a\n", false, &[], Some(usize::MAX))
            .iter()
            .any(|d| d.rule == "MD013"),
        "a huge limit reports nothing rather than reverting to rumdl's defaults"
    );
}
