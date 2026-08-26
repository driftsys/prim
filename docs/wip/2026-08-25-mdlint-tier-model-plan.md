# Markdown lint tier model — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-place prim's Markdown lint rules into two gating bands, drop MD082,
and add a subtract-only `.editorconfig` key that removes named rules from the
tier prim selected for a path.

**Architecture:** `ACTIVE_RULES` in `prim-fmt` loses its three-state severity
matrix and becomes a two-state activation table: a rule runs at the floor tier
or only under `prim_mdlint_strict = true`, and every rule that runs is an error.
Rule exclusion resolves in `prim-cli` from `.editorconfig` and is passed into
the pure engine as data, so `prim-fmt` keeps doing no I/O.

**Tech Stack:** Rust workspace, `rumdl = "=0.2.35"`
(`default-features =
false`), `ec4rs = "1.2"` for `.editorconfig`, `just` for
the task runner, `trycmd` for CLI snapshots.

**Spec:** `docs/wip/2026-08-23-mdlint-tier-model-design.md`

## Global Constraints

- `prim-fmt` stays pure: no clap, no I/O, no terminal. Exclusions arrive as data
  from `prim-cli`.
- Zero warnings anywhere — compiler, `cargo test`, `clippy`, Markdown. No
  `#[allow(...)]` without a documented reason.
- Exit codes are the contract: `0` clean, `1` actionable, `2` prim could not do
  its job. Warnings never raise the exit code.
- `prim_mdlint_disable` is subtract-only. It removes rules from the tier prim
  selected; it can never add a rule prim decided not to run, and never changes a
  severity.
- Rule ids are matched case-insensitively everywhere.
- File size: soft limit 300 lines, hard limit 500. `editorconfig.rs` is already
  at 538 and `app.rs` at 524 — do not grow either; Task 2 extracts a module.
- Conventional Commits. Task 1 changes reported findings and exit codes, so it
  is `feat!`. Run `just fmt` before every commit.
- Never push to `main`. This work belongs on a branch and lands as one PR.
- **Public corpora only.** Nothing committed — source, doc comments, tests,
  documentation, decision records, commit messages — may reference the private
  workspaces the placement was also measured against, or any figure derived from
  them. Public open-source corpora may be cited by name and by number. Where a
  claim was measured privately, restate it from the public measurement or argue
  it on its merits.

---

### Task 1: Re-place the severity matrix

Replaces the off/warn/error matrix with an activation table, drops MD082, and
updates every test that depended on a Markdown finding being a warning.

**Files:**

- Modify: `crates/prim-fmt/src/mdlint.rs:50-131` (types and `ACTIVE_RULES`),
  `:133-138` (`effective_severity`), `:152-189` (`lint`), and the whole
  `#[cfg(test)]` module
- Modify: `crates/prim-fmt/src/lib.rs:37` (re-export)
- Modify: `crates/prim-cli/src/app.rs:261`, `:467`
- Modify: `crates/prim-cli/src/lsp/diagnostics.rs:40-46`
- Modify: `crates/prim-cli/tests/lint_diagnostics.rs:131-160`
- Test: `crates/prim-fmt/src/mdlint.rs` inline module

**Interfaces:**

- Consumes: nothing from earlier tasks.
- Produces:
  - `pub fn lint(source: &str, strict: bool, disabled: &[String]) -> Vec<MdDiagnostic>`
  - `pub fn is_known_rule(rule: &str) -> bool` — true when `rule` names a rule
    prim can run in either tier, case-insensitively. Task 3 uses it to validate
    `prim_mdlint_disable` entries.
  - `MdDiagnostic.is_error` stays a public field and is now always `true`.

- [ ] **Step 1: Replace the matrix test with the new bands**

In `crates/prim-fmt/src/mdlint.rs`, delete `severity_matrix_matches_issue_59`
entirely and put this in its place:

```rust
    const DEFECT_RULES: [&str; 13] = [
        "MD042", "MD011", "MD052", "MD056", "MD062", "MD057", "MD034", "MD051", "MD045",
        "MD075", "MD066", "MD068", "MD070",
    ];

    const CONVENTION_RULES: [&str; 13] = [
        "MD040", "MD041", "MD080", "MD024", "MD036", "MD025", "MD001", "MD026", "MD053",
        "MD033", "MD059", "MD073", "MD067",
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
        // MD082 was dropped (78 % of its findings flag a parent heading
        // followed by a deeper one); the rest are formatter territory or off.
        for rule in ["MD082", "MD013", "MD060", "MD072", "MD003", "MD047"] {
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p prim-fmt mdlint` Expected: FAIL —
`cannot find function 'is_active'` and `cannot find function
'is_known_rule'`.

- [ ] **Step 3: Replace the matrix types and table**

In `crates/prim-fmt/src/mdlint.rs`, delete `enum PrimSeverity`,
`struct
RulePolicy`, the `rule` constructor, `ACTIVE_RULES` and
`effective_severity` (lines 50-138), and write:

```rust
/// One rule prim runs, and the tier at which it starts running.
///
/// There is no severity column: every rule prim runs is an error. prim reports
/// nothing it will not fail on, so a finding's presence is its severity. The
/// tier chooses *which* rules run, never how loudly they speak.
#[derive(Debug, Clone, Copy)]
struct RulePolicy {
    rule: &'static str,
    /// `true` when the rule runs in the always-on floor tier, and therefore in
    /// the strict tier as well.
    floor: bool,
}

/// A rule that reports something objectively broken: a dead link, a dangling
/// reference, a malformed table. Runs in both tiers.
const fn defect(rule: &'static str) -> RulePolicy {
    RulePolicy { rule, floor: true }
}

/// A rule that reports a documentation convention — decidable, but it fires on
/// documents that are otherwise fine. Runs only under `prim_mdlint_strict`.
const fn convention(rule: &'static str) -> RulePolicy {
    RulePolicy { rule, floor: false }
}

const ACTIVE_RULES: &[RulePolicy] = &[
    defect("MD042"),
    defect("MD011"),
    defect("MD052"),
    defect("MD056"),
    defect("MD062"),
    defect("MD057"),
    defect("MD034"),
    defect("MD051"),
    defect("MD045"),
    defect("MD075"),
    defect("MD066"),
    defect("MD068"),
    defect("MD070"),
    convention("MD040"),
    convention("MD041"),
    convention("MD080"),
    convention("MD024"),
    convention("MD036"),
    convention("MD025"),
    convention("MD001"),
    convention("MD026"),
    convention("MD053"),
    convention("MD033"),
    convention("MD059"),
    convention("MD073"),
    convention("MD067"),
];

/// Whether `rule` runs for a file at this tier.
fn is_active(rule: &str, strict: bool) -> bool {
    ACTIVE_RULES
        .iter()
        .any(|policy| policy.rule == rule && (policy.floor || strict))
}

/// Whether `rule` names a rule prim can run in either tier. Callers validating
/// user-supplied rule ids use this so a typo can be reported rather than
/// silently matching nothing.
pub fn is_known_rule(rule: &str) -> bool {
    ACTIVE_RULES
        .iter()
        .any(|policy| policy.rule.eq_ignore_ascii_case(rule))
}

/// Whether `rule` was excluded for this file by `prim_mdlint_disable`.
fn is_disabled(rule: &str, disabled: &[String]) -> bool {
    disabled
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(rule))
}
```

- [ ] **Step 4: Rewrite `lint` to filter on activation and exclusion**

Replace the body of `lint` (lines 152-189) with:

```rust
pub fn lint(source: &str, strict: bool, disabled: &[String]) -> Vec<MdDiagnostic> {
    let strict = file_level_strict_override(source).unwrap_or(strict);
    let cfg = Config::default();
    let rules: Vec<_> = all_rules(&cfg)
        .into_iter()
        .filter(|rule| is_active(rule.name(), strict) && !is_disabled(rule.name(), disabled))
        .collect();

    // `source_file = None` keeps this pure (no path/I/O); `verbose = false`.
    let warnings = match rumdl_lib::lint(
        source,
        &rules,
        false,
        MarkdownFlavor::Standard,
        None,
        Some(&cfg),
    ) {
        Ok(warnings) => warnings,
        // A linter failure must never corrupt a format run: report nothing and
        // let formatting proceed. Real error surfacing is G2's contract.
        Err(_) => return Vec::new(),
    };

    warnings
        .into_iter()
        .filter_map(|warning| {
            let rule = warning.rule_name?;
            if !is_active(&rule, strict) || is_disabled(&rule, disabled) {
                return None;
            }
            Some(MdDiagnostic {
                rule,
                line: warning.line,
                column: warning.column,
                is_error: true,
                message: warning.message,
            })
        })
        .collect()
}
```

Update the doc comment above `lint` so it describes the tiers as rule selection,
documents the `disabled` parameter as subtract-only, and drops the sentence
about escalating warnings to errors.

- [ ] **Step 5: Update the remaining engine tests for the new signature**

Every other `lint(...)` call in the inline test module takes a third argument.
Replace `floor_and_strict_tiers_use_prim_owned_severities` with:

```rust
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
```

In `verifies_selected_rumdl_extension_rules_on_real_markdown`, delete the
`MD082` case, change every remaining tuple's third element to `true`, and call
`lint(src, true, &[])`. In `never_linted_and_off_rules_stay_excluded`,
`reports_a_bare_url_with_real_line_col`, `clean_markdown_yields_no_findings` and
`lint_never_mutates_source`, add `, &[]` to each `lint` call.

- [ ] **Step 6: Stop MD025 counting front matter as a heading**

Write the failing test first, in the inline test module:

```rust
    #[test]
    fn a_front_matter_title_is_metadata_not_a_heading() {
        let page = "---\ntitle: FAQ\n---\n\n# FAQ\n\nText.\n";
        assert!(
            lint(page, true, &[]).iter().all(|d| d.rule != "MD025"),
            "front-matter title plus one body H1 is a normal page: {:?}",
            lint(page, true, &[])
        );

        let two_titles = "# One\n\nText.\n\n# Two\n\nText.\n";
        assert!(
            lint(two_titles, true, &[]).iter().any(|d| d.rule == "MD025"),
            "two real top-level headings are still reported"
        );
    }
```

Run: `cargo test -p prim-fmt a_front_matter_title`

Expected: FAIL — MD025 fires on the first document, because rumdl counts the
front-matter `title:` as a top-level heading.

Then replace `let cfg = Config::default();` in `lint` with
`let cfg =
prim_config();` and add:

```rust
/// prim's canonical rumdl configuration.
///
/// MD025 counts a front-matter `title:` as a top-level heading by default, so a
/// page written the way Docusaurus and VitePress expect — front-matter title for
/// the sidebar, one body H1 for the rendered heading — reports a duplicate
/// title. Measured across six documentation sites, 123 of 139 MD025 findings
/// were that shape and only 16 were two real H1s. An empty `front-matter-title`
/// stops the rule counting page metadata as a heading.
///
/// This is prim choosing its canonical defaults, not a user-facing surface:
/// there is still no way for a repository to configure a rule's options.
fn prim_config() -> Config {
    let mut config = Config::default();
    config.rules.insert(
        "MD025".to_string(),
        RuleConfig {
            severity: None,
            values: BTreeMap::from([(
                "front-matter-title".to_string(),
                toml::Value::String(String::new()),
            )]),
        },
    );
    config
}
```

Imports: `use std::collections::BTreeMap;` and extend the rumdl import to
`use rumdl_lib::config::{Config, MarkdownFlavor, RuleConfig};`. `toml` is
already a `prim-fmt` dependency and resolves to the same 1.1.2 rumdl uses, so
`toml::Value` is the same type and nothing new is added to `Cargo.toml`.

Run: `cargo test -p prim-fmt a_front_matter_title`

Expected: PASS.

- [ ] **Step 7: Update the three call sites**

`crates/prim-fmt/src/lib.rs:37`:

```rust
pub use mdlint::{MdDiagnostic, is_known_rule, lint as lint_markdown};
```

`crates/prim-cli/src/app.rs:261`:

```rust
let diagnostics = prim_fmt::lint_markdown(
    &input,
    editorconfig::resolve_mdlint_strict(path),
    &[],
);
```

`crates/prim-cli/src/app.rs:467`:

```rust
let diagnostics = prim_fmt::lint_markdown(&original, markdown_strict, &[]);
```

`crates/prim-cli/src/lsp/diagnostics.rs:40`:

```rust
prim_fmt::lint_markdown(text, strict, &[])
```

The empty slices are temporary — Task 3 replaces each with the resolved
exclusion list. Leave the `if diagnostic.is_error` branch in
`lsp/diagnostics.rs` alone: it still compiles, and Task 3's LSP work is
unaffected by it.

- [ ] **Step 8: Update the integration tests that assumed a floor warning**

In `crates/prim-cli/tests/lint_diagnostics.rs`, replace
`markdown_floor_warning_prints_but_does_not_raise_the_exit_code` and
`markdown_strict_mode_escalates_warnings_via_editorconfig` with:

````rust
#[test]
fn markdown_floor_defect_raises_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "# Title\n\n![](hero.png)\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains("README.md:3:")
            .and(predicates::str::contains("[MD045]"))
            .and(predicates::str::contains("Image missing alt text")),
    );
}

#[test]
fn markdown_convention_rules_are_silent_until_strict() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "# Title\n\n```\ncode\n```\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD040]").not());

    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD040]"));
}
````

- [ ] **Step 9: Run the full suite**

Run: `just fmt && just test` Expected: PASS. If `crates/prim-cli/tests/init.rs`,
`verbs.rs` or `machine_readable.rs` fail, it is because MD045 now exits `1`
where they expected `0`; change only the expected exit code, never the assertion
about which rule fired.

- [ ] **Step 10: Commit**

```bash
git add crates/prim-fmt/src/mdlint.rs crates/prim-fmt/src/lib.rs \
  crates/prim-cli/src/app.rs crates/prim-cli/src/lsp/diagnostics.rs \
  crates/prim-cli/tests/
git commit -m "feat!: re-place Markdown lint rules into defect and convention bands

Every rule prim runs is now an error, so a finding's presence is its
severity. Defect rules run at both tiers; convention rules run only under
prim_mdlint_strict. MD082 is dropped: 78 % of its findings flagged a parent
heading followed by a deeper one, which is the ordinary outline shape.

BREAKING CHANGE: prim lint now exits 1 on findings that were previously
warning-severity, and MD082 is no longer reported."
```

---

### Task 2: Extract the Markdown lint policy module

A pure move with no behaviour change, so Task 3 has a home that is not the
538-line `editorconfig.rs`.

**Files:**

- Create: `crates/prim-cli/src/mdlint_policy.rs`
- Modify: `crates/prim-cli/src/editorconfig.rs` (remove the mdlint key, its
  resolver methods and their tests)
- Modify: `crates/prim-cli/src/main.rs` (declare the module)
- Modify: `crates/prim-cli/src/app.rs`, `crates/prim-cli/src/app/load.rs`,
  `crates/prim-cli/src/lsp/diagnostics.rs`, `crates/prim-cli/src/provenance.rs`
  (import path)

**Interfaces:**

- Consumes: `editorconfig::Resolver::properties_for`,
  `editorconfig::prim_bool_from`.
- Produces:
  - `pub(crate) const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";`
  - `pub fn resolve_strict(path: &Path) -> bool` — one-shot, no caching.
  - `impl Resolver { pub fn resolve_mdlint_strict(&mut self, path: &Path) -> bool }`
    stays on `editorconfig::Resolver` and delegates to this module, so no caller
    has to hold two resolvers.

- [ ] **Step 1: Create the module with the moved code**

Create `crates/prim-cli/src/mdlint_policy.rs`:

```rust
//! Resolve prim's Markdown lint policy for one file: which tier applies, and
//! which rules that path excludes.
//!
//! Lives in the CLI crate because it reads `.editorconfig`; `prim-fmt` takes
//! the resolved policy as data and stays pure.

use std::path::Path;

use ec4rs::Properties;

use crate::editorconfig::{self, Resolver};

pub(crate) const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";

/// Read `prim_mdlint_strict` out of already-resolved properties. Unset or
/// non-`true` values mean the floor tier.
pub(crate) fn strict_from(props: &Properties) -> bool {
    editorconfig::prim_bool_from(props, MDLINT_STRICT_KEY).unwrap_or(false)
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve_strict(path: &Path) -> bool {
    let mut resolver = Resolver::new();
    strict_from(&resolver.properties_for(path))
}
```

- [ ] **Step 2: Point `editorconfig.rs` at it**

In `crates/prim-cli/src/editorconfig.rs`, delete `MDLINT_STRICT_KEY`,
`resolve_prim_bool_key`, the `Resolver::resolve_mdlint_strict` body's key lookup
and the free `resolve_mdlint_strict`, then re-implement the method as a
delegation:

```rust
/// Resolve `prim_mdlint_strict` for `path`, reusing the cached cascade for
/// its directory when one is present.
pub fn resolve_mdlint_strict(&mut self, path: &Path) -> bool {
    crate::mdlint_policy::strict_from(&self.properties_for(path))
}
```

Keep `prim_bool_from` in `editorconfig.rs` and make it `pub(crate)` — it is the
generic `.editorconfig` value reader, not mdlint-specific.

- [ ] **Step 3: Fix the imports**

Add `mod mdlint_policy;` to `crates/prim-cli/src/main.rs`. In
`crates/prim-cli/src/provenance.rs:15`, change the import to:

```rust
use crate::editorconfig::{self, Resolver};
use crate::mdlint_policy::MDLINT_STRICT_KEY;
```

In `crates/prim-cli/src/app.rs:261`, replace
`editorconfig::resolve_mdlint_strict(path)` with
`crate::mdlint_policy::resolve_strict(path)`.

- [ ] **Step 4: Move the test**

`editorconfig.rs` has exactly one test that belongs here:
`prim_custom_key_resolves_per_glob_more_specific_later_wins`
(`crates/prim-cli/src/editorconfig.rs:416`) and its helper `resolve_prim_bool`
(`:411`). Move both into `mdlint_policy.rs`'s test module and rewrite the helper
against the new entry point, since `resolve_prim_bool_key` no longer exists:

```rust
fn strict_for(dir: &Path, relative: &str) -> bool {
    resolve_strict(&dir.join(relative))
}
```

Change the three assertions from `Some(false)` / `Some(true)` to `false` /
`true`, and add `use super::*;`, `use std::fs;` and `use std::path::Path;` to
the module.

- [ ] **Step 5: Verify nothing changed**

Run: `just fmt && just test` Expected: PASS, with no test edited beyond its
module location.

Run: `wc -l crates/prim-cli/src/editorconfig.rs` Expected: below 500.

- [ ] **Step 6: Commit**

```bash
git add crates/prim-cli/src/
git commit -m "refactor(prim-cli): extract Markdown lint policy resolution

editorconfig.rs was at 538 lines, over the hard limit, and mixes style
resolution with lint policy. Pure move, no behaviour change."
```

---

### Task 3: `prim_mdlint_disable`, end to end

**Files:**

- Modify: `crates/prim-cli/src/mdlint_policy.rs`
- Modify: `crates/prim-cli/src/app.rs:261`, `:467`
- Modify: `crates/prim-cli/src/app/load.rs:105-111`
- Modify: `crates/prim-cli/src/lsp/diagnostics.rs:39-41`
- Test: `crates/prim-cli/src/mdlint_policy.rs` inline module, and
  `crates/prim-cli/tests/lint_diagnostics.rs`

**Interfaces:**

- Consumes: `prim_fmt::is_known_rule` (Task 1), `mdlint_policy::strict_from`
  (Task 2).
- Produces:
  - `pub struct MdLintPolicy { pub strict: bool, pub disabled: Vec<String> }`
  - `pub fn resolve(path: &Path) -> MdLintPolicy` — one-shot.
  - `impl Resolver { pub fn resolve_mdlint_policy(&mut self, path: &Path) -> MdLintPolicy }`
  - `LoadOutcome::Formatted` carries `MdLintPolicy` where it carried
    `markdown_strict: bool`.

- [ ] **Step 1: Write the failing unit tests**

Add to the inline test module in `crates/prim-cli/src/mdlint_policy.rs`:

```rust
    #[test]
    fn disable_list_splits_trims_and_uppercases() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root = true\n[*.md]\nprim_mdlint_disable = MD033 , md041\n",
        )
        .unwrap();

        let policy = resolve(&dir.path().join("a.md"));
        assert_eq!(policy.disabled, vec!["MD033", "MD041"]);
    }

    #[test]
    fn a_narrower_section_replaces_the_wider_list() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("docs")).unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root = true\n[*.md]\nprim_mdlint_disable = MD033\n[docs/**.md]\nprim_mdlint_disable = MD041\n",
        )
        .unwrap();

        assert_eq!(resolve(&dir.path().join("a.md")).disabled, vec!["MD033"]);
        assert_eq!(
            resolve(&dir.path().join("docs/g.md")).disabled,
            vec!["MD041"],
            "EditorConfig replaces a value, it does not merge lists"
        );
    }

    #[test]
    fn an_unknown_rule_id_is_dropped_rather_than_matched() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root = true\n[*.md]\nprim_mdlint_disable = MD999, MD033\n",
        )
        .unwrap();

        let policy = resolve(&dir.path().join("a.md"));
        assert_eq!(
            policy.disabled,
            vec!["MD033"],
            "unknown ids are reported and dropped"
        );
    }

    #[test]
    fn an_empty_or_unset_key_disables_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), "root = true\n[*.md]\n").unwrap();
        assert!(resolve(&dir.path().join("a.md")).disabled.is_empty());
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p prim-cli mdlint_policy` Expected: FAIL —
`cannot find function 'resolve'`.

- [ ] **Step 3: Implement resolution and validation**

Add to `crates/prim-cli/src/mdlint_policy.rs`:

```rust
pub(crate) const MDLINT_DISABLE_KEY: &str = "prim_mdlint_disable";

/// The Markdown lint policy for one file: the tier that applies, and the rules
/// that path excludes from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MdLintPolicy {
    /// `true` when `prim_mdlint_strict` selected the strict tier.
    pub strict: bool,
    /// Rule ids excluded by `prim_mdlint_disable`, uppercased. Subtract-only:
    /// these are removed from the tier's rule set and can never add to it.
    pub disabled: Vec<String>,
}

/// Parse `prim_mdlint_disable` out of already-resolved properties.
///
/// The value is a comma-separated list of rule ids. Entries are trimmed and
/// uppercased. An id prim does not run is reported once and dropped, so a typo
/// is visible rather than silently excluding nothing; per AD-0007 that warning
/// does not raise the exit code.
fn disabled_from(props: &Properties, path: &Path) -> Vec<String> {
    let Some(raw) = props.get_raw_for_key(MDLINT_DISABLE_KEY).into_option() else {
        return Vec::new();
    };

    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            if prim_fmt::is_known_rule(entry) {
                Some(entry.to_ascii_uppercase())
            } else {
                ui::warning(&format!(
                    "{}: {MDLINT_DISABLE_KEY} lists '{entry}', which is not a rule prim runs — ignoring it",
                    path.display()
                ));
                None
            }
        })
        .collect()
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve(path: &Path) -> MdLintPolicy {
    Resolver::new().resolve_mdlint_policy(path)
}
```

Add `use crate::ui;` to the module's imports, and extend `editorconfig.rs`'s
`Resolver`:

```rust
/// Resolve the whole Markdown lint policy for `path`, reusing the cached
/// cascade for its directory when one is present.
pub fn resolve_mdlint_policy(&mut self, path: &Path) -> crate::mdlint_policy::MdLintPolicy {
    let props = self.properties_for(path);
    crate::mdlint_policy::policy_from(&props, path)
}
```

and add the assembling helper to `mdlint_policy.rs`:

```rust
pub(crate) fn policy_from(props: &Properties, path: &Path) -> MdLintPolicy {
    MdLintPolicy {
        strict: strict_from(props),
        disabled: disabled_from(props, path),
    }
}
```

- [ ] **Step 4: Run the unit tests**

Run: `cargo test -p prim-cli mdlint_policy` Expected: PASS.

- [ ] **Step 5: Write the failing integration test**

Add to `crates/prim-cli/tests/lint_diagnostics.rs`:

```rust
#[test]
fn disable_key_subtracts_a_rule_from_the_strict_tier() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("docs/guide.md");
    std::fs::write(&file, "# Title\n\nPress <kbd>Ctrl</kbd>.\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD033]").not());
}

#[test]
fn disable_key_does_not_reach_other_globs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let outside = dir.path().join("README.md");
    std::fs::write(&outside, "# Title\n\nPress <kbd>Ctrl</kbd>.\n").unwrap();

    prim()
        .arg("lint")
        .arg(&outside)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD033]"));
}

#[test]
fn an_unknown_disabled_rule_warns_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "# Title\n\nText.\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stderr(predicates::str::contains("MD999"));
}
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test -p prim-cli --test lint_diagnostics disable_key` Expected: FAIL
— MD033 still reported, because nothing threads the policy into the engine yet.

- [ ] **Step 7: Thread the policy through both lint paths**

`crates/prim-cli/src/app.rs:261`:

```rust
let policy = crate::mdlint_policy::resolve(path);
let diagnostics = prim_fmt::lint_markdown(&input, policy.strict, &policy.disabled);
```

`crates/prim-cli/src/app/load.rs:105-111`:

```rust
    let markdown_policy = if kind == FileKind::Markdown {
        resolver.resolve_mdlint_policy(&file.path)
    } else {
        MdLintPolicy::default()
    };

    LoadOutcome::Formatted((file.path, kind, style, markdown_policy, original, formatted))
```

Add `use crate::mdlint_policy::MdLintPolicy;` there, and update the three
destructuring sites in `app.rs` (`:335`, `:393`, `:451`) so the fourth element
is named `markdown_policy` (or `_markdown_policy` where unused). Then
`app.rs:467`:

```rust
let diagnostics =
    prim_fmt::lint_markdown(&original, markdown_policy.strict, &markdown_policy.disabled);
```

`crates/prim-cli/src/lsp/diagnostics.rs:39-41`:

```rust
let policy = resolver.resolve_mdlint_policy(path);
prim_fmt::lint_markdown(text, policy.strict, &policy.disabled)
```

Wherever the tuple type is declared (`FormattedFile` in `app/load.rs`), change
the `bool` to `MdLintPolicy`.

- [ ] **Step 8: Delete what the threading orphaned**

Three entry points lose their last caller here, and prim allows no warnings:

- `mdlint_policy::resolve_strict` — `app.rs:261` was its last caller and now
  calls `mdlint_policy::resolve`. Delete it, and point the moved test's
  `strict_for` helper at `resolve(&dir.join(relative)).strict`.
- `editorconfig::Resolver::resolve_mdlint_strict` — `load.rs` and
  `lsp/diagnostics.rs` were its callers and now call `resolve_mdlint_policy`.
  Delete it.
- Keep `mdlint_policy::strict_from`: `policy_from` still calls it.

Run: `cargo clippy --workspace --all-targets`

Expected: no `dead_code` or `unused` diagnostics.

- [ ] **Step 9: Run the whole suite**

Run: `just fmt && just test` Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/prim-cli/src/ crates/prim-cli/tests/
git commit -m "feat(prim-cli): add prim_mdlint_disable for per-glob rule exclusion

A comma-separated list of rule ids in .editorconfig removes those rules from
the tier prim selected for matching files. Subtract-only: it can never add a
rule prim does not run, nor change a severity. Unknown ids warn and are
ignored."
```

---

### Task 4: `prim explain` surfaces the key

**Files:**

- Modify: `crates/prim-cli/src/provenance.rs:73-82`
- Test: `crates/prim-cli/tests/explain.rs`

**Interfaces:**

- Consumes: `mdlint_policy::{MDLINT_DISABLE_KEY, policy_from}`.
- Produces: one more `ResolvedSetting` for Markdown files.

- [ ] **Step 1: Write the failing test**

Add to `crates/prim-cli/tests/explain.rs`:

```rust
#[test]
fn explain_shows_the_disabled_rules_and_their_origin() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD033, MD041\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n").unwrap();

    prim()
        .current_dir(dir.path())
        .args(["explain", "a.md"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("prim_mdlint_disable")
                .and(predicates::str::contains("MD033, MD041"))
                .and(predicates::str::contains(".editorconfig:3")),
        );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p prim-cli --test explain explain_shows_the_disabled_rules`
Expected: FAIL — the key is absent from the output.

- [ ] **Step 3: Add the setting**

In `crates/prim-cli/src/provenance.rs`, replace the whole existing
`if kind == FileKind::Markdown` block (`:73-82`) with:

```rust
if kind == FileKind::Markdown {
    let policy = crate::mdlint_policy::policy_from(&props, path);
    settings.push(ResolvedSetting {
        key: MDLINT_STRICT_KEY,
        value: policy.strict.to_string(),
        origin: origin_of(&props, MDLINT_STRICT_KEY),
    });
    settings.push(ResolvedSetting {
        key: MDLINT_DISABLE_KEY,
        value: if policy.disabled.is_empty() {
            "unset".to_string()
        } else {
            policy.disabled.join(", ")
        },
        origin: origin_of(&props, MDLINT_DISABLE_KEY),
    });
}
```

Both values now come from one resolution rather than a second `prim_bool_from`
call. Import `MDLINT_DISABLE_KEY` alongside `MDLINT_STRICT_KEY`.

- [ ] **Step 4: Run the test**

Run: `cargo test -p prim-cli --test explain` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/prim-cli/src/provenance.rs crates/prim-cli/tests/explain.rs
git commit -m "feat(prim-cli): show prim_mdlint_disable in prim explain"
```

---

### Task 5: `prim init` exempts `docs/wip`

**Files:**

- Modify: `crates/prim-cli/src/init.rs:157-180`
- Test: `crates/prim-cli/src/init.rs` inline module,
  `crates/prim-cli/tests/init.rs`

**Interfaces:**

- Consumes: nothing from earlier tasks.
- Produces: a four-section scaffold instead of three.

- [ ] **Step 1: Update the scaffold test**

In the inline test module of `crates/prim-cli/src/init.rs`, change every
expected scaffold string to include the new section between the strict glob and
`SUMMARY.md`, for example:

```rust
"root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
```

and add `"added [docs/wip/**.md] with prim_mdlint_strict = false"` to the
expected action lists.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p prim-cli init` Expected: FAIL — scaffold missing the
`docs/wip` section.

- [ ] **Step 3: Add the section**

```rust
fn scaffold(strict_glob: &str) -> String {
    format!(
        "root = true\n[*.md]\n{MDLINT_STRICT_KEY} = false\n[{strict_glob}]\n{MDLINT_STRICT_KEY} = true\n[docs/wip/**.md]\n{MDLINT_STRICT_KEY} = false\n[**/SUMMARY.md]\n{MDLINT_STRICT_KEY} = false\n"
    )
}
```

and add the matching entry to `merge`'s `specs`, third in order:

```rust
SectionSpec {
    glob: "docs/wip/**.md",
    value: false,
},
```

Document why in a comment: Superpowers specs and plans under `docs/wip/` are
transient working memory, so the strict tier must not apply to them even when
the strict glob covers `docs/**`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prim-cli init` Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/prim-cli/src/init.rs crates/prim-cli/tests/init.rs
git commit -m "feat(prim-cli): exempt docs/wip from the strict glob in prim init"
```

---

### Task 6: Documentation and final gate

**Files:**

- Modify: `docs/SPEC.md:228-266` (FR-5.5b matrix and exit-code bullet)
- Modify: `docs/USAGE.md:140-160` (severity table)
- Modify: `docs/recipes.md` (CI and pre-commit examples)
- Modify: `AGENTS.md` (the "no per-rule flags" line, which now needs the
  subtract-only exception)
- Create: `docs/decisions/0012-markdown-lint-bands-and-rule-exclusion.md`

- [ ] **Step 1: Rewrite the SPEC matrix**

Replace the three-column severity matrix under FR-5.5b with two lists — the 13
defect rules and the 13 convention rules — state that every active rule is an
error, and replace the "Exit-code implication" bullet with: floor-tier findings
and strict-tier findings alike raise the exit code to `1`; no Markdown rule
emits a warning. Record MD082's removal and its reason in the "Off in both
tiers" list. Add a short subsection recording the one rule option prim sets for
itself — MD025's `front-matter-title` emptied — and state that it is prim's own
canonical default, not a configuration surface repositories can reach.

- [ ] **Step 2: Document the new key**

In `docs/USAGE.md`, mirror the same two lists, then document
`prim_mdlint_disable`: comma-separated rule ids, resolved per glob section,
subtract-only, unknown ids warn and are ignored, and EditorConfig replaces
rather than merges a value between sections. Add the four inline directives that
already work (`markdownlint-disable-file`, the `disable`/`enable` pair,
`disable-next-line`, `rumdl-disable`) as the per-file and per-line escapes.

- [ ] **Step 3: Write the decision record**

Create `docs/decisions/0012-markdown-lint-bands-and-rule-exclusion.md` following
the shape of `0011-generated-files-are-not-formatted.md`: Status, Context (the
two-axis problem — one boolean serving both the gate and the editor's display),
Decision (two bands, MD082 dropped, subtract-only key), Consequences,
Alternatives considered.

Evidence may cite only the public corpora, by name and by number: the
documentation trees of `rust-lang/book`, `markdownlint`, `mdBook` and `cli/cli`;
the documentation sites of React Native, FastAPI, Vue, Redux, Vite and Building
Secure Contracts; and READMEs sampled from the Cargo registry. The figures are
in `docs/wip/2026-08-23-mdlint-tier-model-design.md` under "Open-source
validation" and "Documentation-site validation". Never cite the private
workspaces measured in the "Corpus" section, or any figure taken from them.

- [ ] **Step 4: Run the full gate**

Run: `just verify` Expected: PASS — commit lint, build, tests, install tests,
lint.

- [ ] **Step 5: Re-measure against the corpora**

Rebuild the two corpora described in the spec's Evidence section and confirm:
the floor tier reports only defect-band rules, the strict tier reports no MD082,
and a `prim_mdlint_disable = MD033` line silences MD033 across a tree.

- [ ] **Step 6: Commit**

```bash
git add docs/ AGENTS.md
git commit -m "docs(prim): record the Markdown lint bands and rule exclusion"
```
