//! One minimal, self-contained Markdown fixture per rule in `ACTIVE_RULES`,
//! each written to make its own rule actually fire against the pinned
//! `rumdl = "=0.2.35"`.
//!
//! The tier matrix test in the parent module only asserts which band a rule
//! is placed in; it never asserts the rule still matches anything. A linter's
//! misses are invisible by construction: if a rule stopped firing — after a
//! rumdl bump, a config change, or a flavor default moving — prim would exit
//! `0` on a document that violates it and nothing would fail. This module is
//! the regression net for that.
//!
//! Fixtures were found empirically by linting a candidate and reading back
//! what rumdl actually reported, not derived from a rule's documentation:
//! several rules need a more specific shape than their name suggests. See the
//! comment on an individual fixture where that shape is not obvious.

use super::super::*;

/// One rule's fixture, and the tier at which that fixture is expected to
/// fire.
struct RuleFixture {
    rule: &'static str,
    /// Mirrors [`RulePolicy::floor`]: `true` for a defect rule (fires at the
    /// floor tier, and therefore at strict too); `false` for a convention
    /// rule (fires only under `prim_mdlint_strict = true`, and must NOT fire
    /// at the floor tier — the half of this test that pins the band model
    /// itself, not just the rule).
    floor: bool,
    src: &'static str,
}

const RULE_FIXTURES: &[RuleFixture] = &[
    // Defect rules — run in both tiers.
    RuleFixture {
        rule: "MD042",
        floor: true,
        src: "[text]()\n",
    },
    RuleFixture {
        rule: "MD011",
        floor: true,
        // Reversed order: `(url)[text]` instead of `[text](url)`.
        src: "(https://example.com)[Example]\n",
    },
    RuleFixture {
        rule: "MD052",
        floor: true,
        src: "[text][undefined]\n",
    },
    RuleFixture {
        rule: "MD056",
        floor: true,
        src: "| a | b |\n| --- | --- |\n| 1 | 2 | 3 |\n",
    },
    RuleFixture {
        rule: "MD062",
        floor: true,
        src: "[link]( https://example.com )\n",
    },
    // MD057 (existing relative links) is deliberately absent from this
    // table — it cannot fire through prim's entry point at all. See issue
    // #134 and `md057_cannot_fire_because_lint_never_passes_a_source_file`
    // below.
    RuleFixture {
        rule: "MD034",
        floor: true,
        src: "See https://example.com for details.\n",
    },
    RuleFixture {
        rule: "MD051",
        floor: true,
        src: "[link](#nonexistent)\n\n# Real Heading\n",
    },
    RuleFixture {
        rule: "MD045",
        floor: true,
        src: "![](hero.png)\n",
    },
    RuleFixture {
        rule: "MD075",
        floor: true,
        src: "Some text.\n\n| value1 | description1 |\n| value2 | description2 |\n",
    },
    RuleFixture {
        rule: "MD066",
        floor: true,
        src: "Text with orphan[^missing].\n",
    },
    RuleFixture {
        rule: "MD068",
        floor: true,
        src: "Text with [^1].\n\n[^1]:\n",
    },
    RuleFixture {
        rule: "MD070",
        floor: true,
        // A fenced block that itself contains a fence of the same length:
        // rumdl reports the inner fence as interfering with the outer one's
        // parsing.
        src: "```markdown\n```rust\nfn main() {}\n```\n```\n",
    },
    // Convention rules — fire only under `prim_mdlint_strict = true`.
    RuleFixture {
        rule: "MD040",
        floor: false,
        src: "```\ncode\n```\n",
    },
    RuleFixture {
        rule: "MD041",
        floor: false,
        src: "Intro\n\n# Title\n",
    },
    RuleFixture {
        rule: "MD080",
        floor: false,
        // Two headings whose text differs only by an ampersand and a space
        // run normalize to the same anchor slug; that collision is what
        // MD080 reports, not the literal heading text.
        src: "# Setup & Run\n\n# Setup  Run\n",
    },
    RuleFixture {
        rule: "MD024",
        floor: false,
        // rumdl's default `siblings_only = true` needs both duplicates at
        // the same heading level with the same parent; two nested H1s do
        // not count as siblings.
        src: "## Foo\n\nText.\n\n## Foo\n\nText.\n",
    },
    RuleFixture {
        rule: "MD036",
        floor: false,
        src: "**Bold Heading**\n\nText.\n",
    },
    RuleFixture {
        rule: "MD025",
        floor: false,
        src: "# One\n\nText.\n\n# Two\n\nText.\n",
    },
    RuleFixture {
        rule: "MD001",
        floor: false,
        src: "# Title\n\n### Subsection\n",
    },
    RuleFixture {
        rule: "MD026",
        floor: false,
        src: "# Title.\n",
    },
    RuleFixture {
        rule: "MD053",
        floor: false,
        src: "[unused]: https://example.com\n",
    },
    RuleFixture {
        rule: "MD033",
        floor: false,
        src: "<div>Text</div>\n",
    },
    RuleFixture {
        rule: "MD059",
        floor: false,
        src: "[click here](https://example.com)\n",
    },
    RuleFixture {
        rule: "MD073",
        floor: false,
        // A marker-delimited TOC that names a heading that does not exist
        // and omits one that does.
        src: "<!-- toc -->\n- [Wrong](#wrong)\n<!-- tocstop -->\n\n## Real\n\nContent.\n",
    },
    RuleFixture {
        rule: "MD067",
        floor: false,
        src: "Text with [^2] and then [^1].\n\n[^1]: First definition\n[^2]: Second definition\n",
    },
];

#[test]
fn every_active_rule_fires_its_own_fixture_at_the_tier_its_band_places_it_in() {
    for fixture in RULE_FIXTURES {
        let strict_diags = lint(fixture.src, true, &[]);
        let diag = strict_diags
            .iter()
            .find(|d| d.rule == fixture.rule)
            .unwrap_or_else(|| {
                panic!(
                    "{} did not fire under strict: {strict_diags:?}",
                    fixture.rule
                )
            });
        assert!(diag.is_error, "{}: {strict_diags:?}", fixture.rule);
        assert!(
            diag.line >= 1,
            "{}: 1-indexed line: {strict_diags:?}",
            fixture.rule
        );
        assert!(
            diag.column >= 1,
            "{}: 1-indexed column: {strict_diags:?}",
            fixture.rule
        );

        let floor_diags = lint(fixture.src, false, &[]);
        if fixture.floor {
            assert!(
                floor_diags.iter().any(|d| d.rule == fixture.rule),
                "{}: a defect rule must also fire at the floor tier: {floor_diags:?}",
                fixture.rule
            );
        } else {
            assert!(
                floor_diags.iter().all(|d| d.rule != fixture.rule),
                "{}: a convention rule must not fire at the floor tier: {floor_diags:?}",
                fixture.rule
            );
        }
    }
}

#[test]
fn rule_fixtures_cover_every_active_rule_exactly_once_except_the_documented_md057_gap() {
    let mut covered: Vec<&str> = RULE_FIXTURES.iter().map(|f| f.rule).collect();
    covered.push("MD057");
    covered.sort_unstable();

    let mut active: Vec<&str> = ACTIVE_RULES.iter().map(|p| p.rule).collect();
    active.sort_unstable();

    assert_eq!(
        covered, active,
        "every ACTIVE_RULES entry needs exactly one fixture, or the documented MD057 exception"
    );
}

#[test]
fn md057_cannot_fire_because_lint_never_passes_a_source_file() {
    // MD057 (existing relative links) resolves a link's target against the
    // directory of the file being linted. rumdl derives that directory from
    // `LintContext::source_file`, and `lint()` (see `mdlint.rs`) always calls
    // `rumdl_lib::lint` with `source_file: None` to keep the engine pure (no
    // path/I/O). Without a source file, rumdl's own check has no base
    // directory to resolve against and returns before inspecting any link —
    // so MD057 cannot fire through prim's lint entry point today, regardless
    // of how broken the link is. This is a real gap in prim's coverage, not
    // a fixture problem: filed as issue #134 rather than worked around here.
    let src = "[broken](./does-not-exist.md)\n";
    let diags = lint(src, true, &[]);
    assert!(
        diags.iter().all(|d| d.rule != "MD057"),
        "MD057 is currently unreachable through lint(): {diags:?}"
    );
}

/// MD025 counts a front-matter `title:` as a top-level heading by default;
/// `prim_config` overrides that (see `mdlint.rs`) so a page with front-matter
/// metadata plus one body H1 is not double-counted. `RULE_FIXTURES` above
/// pins the ordinary case (two real H1s still fire); this pins the override.
#[test]
fn md025_front_matter_title_is_metadata_not_a_heading() {
    let page = "---\ntitle: FAQ\n---\n\n# FAQ\n\nText.\n";
    let diags = lint(page, true, &[]);
    assert!(
        diags.iter().all(|d| d.rule != "MD025"),
        "front-matter title plus one body H1 is a normal page: {diags:?}"
    );
}
