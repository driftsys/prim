# Markdown lint enable key — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an additive `.editorconfig` key, `prim_mdlint_enable`, that adds
named rules to the set prim runs for a path, and amend AD-0012's subtract-only
guarantee to match.

**Architecture:** `prim-fmt`'s rule table gains a third tier — `OptIn` — beside
`Floor` and `Convention`, and rule selection becomes a function of one pure
`MdLintSelection { strict, enabled, disabled }` value that `prim-cli` resolves
from `.editorconfig`. `lint` also takes the same `&Style` the formatter
received, so MD013's threshold is the width prim actually wrapped to rather than
rumdl's unrelated default of 80.

**Tech Stack:** Rust workspace, `rumdl = "=0.2.35"`
(`default-features =
false`), `ec4rs = "1.2"` for `.editorconfig`,
`dprint-plugin-markdown` for Markdown formatting, `just` for the task runner,
`trycmd` for CLI snapshots.

**Spec:** `docs/wip/2026-08-28-mdlint-enable-key-design.md`

## Global Constraints

- `prim-fmt` stays pure: no clap, no I/O, no terminal. The selection arrives as
  data from `prim-cli`.
- Zero warnings anywhere — compiler, `cargo test`, `clippy`, Markdown. No
  `#[allow(...)]` without a documented reason.
- Exit codes are the contract: `0` clean, `1` actionable, `2` prim could not do
  its job. Warnings never raise the exit code.
- Rule ids are matched case-insensitively everywhere.
- `prim_mdlint_disable` is applied after `prim_mdlint_enable`, so a disable wins
  a conflict.
- prim exposes no way for a repository to configure a rule's options (FR-3.3
  first clause). The key selects rules; it never sets their options.
- The enableable set is exactly the 26 ids already in prim's tiers plus **MD013,
  MD014, MD069**. Everything else rumdl has is withheld.
- File size: soft limit 300 lines, hard limit 500. `app.rs` is at 537 and
  `mdlint_policy.rs` at 319 — do not grow either. Task 5 moves
  `mdlint_policy.rs`'s test module into `mdlint_policy/tests.rs` before adding
  anything.
- Conventional Commits, imperative mood. This work changes what `prim lint`
  reports for a repository that opts in, so the feature commits are `feat`. Run
  `just fmt` before every commit.
- Never push to `main`. This work is on `feat/123-mdlint-enable-key` and lands
  as one PR.
- **Public corpora only.** Nothing committed — source, doc comments, tests,
  documentation, decision records, commit messages — may reference private
  workspaces or any figure derived from them.

---

### Task 1: One width, read in one place

MD013's threshold must equal the width the Markdown formatter wrapped to. Today
that width is an inline `unwrap_or(80)` in the formatter. Extract it so the
linter cannot read a different number.

**Files:**

- Modify: `crates/prim-fmt/src/style.rs` (add the method and its test)
- Modify: `crates/prim-fmt/src/markdown.rs:21` (call it)

**Interfaces:**

- Produces: `Style::effective_line_width(&self) -> usize`

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the end of
`crates/prim-fmt/src/style.rs`:

```rust
#[test]
fn effective_line_width_is_the_width_the_formatter_wraps_to() {
    assert_eq!(
        Style::default().effective_line_width(),
        80,
        "an unset max_line_length means prim's canonical 80"
    );
    assert_eq!(
        Style {
            max_line_length: Some(120),
            ..Style::default()
        }
        .effective_line_width(),
        120
    );
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p prim-fmt effective_line_width_is_the_width` Expected: FAIL —
`no method named 'effective_line_width' found`

- [ ] **Step 3: Add the method**

Insert directly above `impl Default for Style` in
`crates/prim-fmt/src/style.rs`:

```rust
impl Style {
    /// The hard-wrap width prim actually uses: `max_line_length` when set,
    /// else prim's canonical 80.
    ///
    /// The Markdown formatter and the Markdown linter must agree on this
    /// number. A linter measuring a different width than the formatter
    /// wrapped to would report prim's own output as a violation, at a
    /// threshold nobody chose. Both read it here, so the agreement is
    /// structural rather than a convention two call sites have to remember.
    pub fn effective_line_width(&self) -> usize {
        self.max_line_length.unwrap_or(80)
    }
}
```

- [ ] **Step 4: Call it from the formatter**

In `crates/prim-fmt/src/markdown.rs`, replace:

```rust
.line_width(style.max_line_length.unwrap_or(80) as u32)
```

with:

```rust
.line_width(style.effective_line_width() as u32)
```

- [ ] **Step 5: Run the engine tests**

Run: `cargo test -p prim-fmt` Expected: PASS — the formatter's wrap-width tests
are unchanged, which is the point: this is a pure extraction.

- [ ] **Step 6: Commit**

```bash
just fmt
git add crates/prim-fmt/src/style.rs crates/prim-fmt/src/markdown.rs
git commit -m "refactor(prim-fmt): read the wrap width from one place (#123)"
```

---

### Task 2: A three-tier selection model in the engine

Replace the two-state activation table with three tiers, add the three opt-in
rules, and make `lint` take the resolved `Style` and one selection value. No
user-visible behaviour changes yet: nothing populates `enabled`, and MD013's
config entry is inert while MD013 is unselected.

**Files:**

- Modify: `crates/prim-fmt/src/mdlint.rs:56-131` (tier types and the rule
  table), `:133-155` (`is_active`, `is_disabled`, `prim_config`), `:160-224`
  (`lint`)
- Modify: `crates/prim-fmt/src/lib.rs:37` (exports)
- Modify: `crates/prim-fmt/src/mdlint/tests.rs` (matrix tests)
- Modify: `crates/prim-cli/src/mdlint_policy.rs` (`MdLintPolicy` holds a
  selection)
- Modify: `crates/prim-cli/src/app.rs:265-268`, `:474-480`,
  `crates/prim-cli/src/lsp/diagnostics.rs:44-49`,
  `crates/prim-cli/src/provenance.rs:80-95` (call sites)

**Interfaces:**

- Consumes: `Style::effective_line_width()` from Task 1
- Produces:
  - `prim_fmt::MdLintSelection { strict: bool, enabled: Vec<String>, disabled:
    Vec<String> }`,
    `Default` + `Clone` + `Debug` + `PartialEq` + `Eq`
  - `prim_fmt::lint_markdown(source: &str, style: &Style, selection:
    &MdLintSelection) -> Vec<MdDiagnostic>`
  - `crate::mdlint_policy::MdLintPolicy { selection: MdLintSelection, unknown:
    Vec<String>, unknown_origin: SettingOrigin }`

- [ ] **Step 1: Rewrite the engine's matrix tests for three tiers**

In `crates/prim-fmt/src/mdlint/tests.rs`, replace the two tests
`defect_rules_run_in_both_tiers_and_conventions_only_in_strict` and
`dropped_and_formatter_territory_rules_never_run` with the following. Leave
`is_known_rule_covers_both_bands_case_insensitively` alone — Task 3 replaces it,
so `is_known_rule` keeps its coverage until the classifier arrives.

```rust
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
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p prim-fmt mdlint` Expected: FAIL to compile —
`MdLintSelection` does not exist and `is_active` takes `(&str, bool)`.

- [ ] **Step 3: Replace the tier types and the rule table**

In `crates/prim-fmt/src/mdlint.rs`, add `use crate::Style;` to the imports, then
replace the `RulePolicy` struct, the `defect`/`convention` constructors and
`ACTIVE_RULES` with:

```rust
/// The tier at which a rule starts running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    /// A defect rule: reports something objectively broken, so it can gate
    /// every repository with no opt-in. Always on.
    Floor,
    /// A documentation convention: decidable, but it fires on documents that
    /// are otherwise fine. On under `prim_mdlint_strict`, or when
    /// `prim_mdlint_enable` names it.
    Convention,
    /// Off in both tiers: runs only when `prim_mdlint_enable` names it. These
    /// are the three rules a repository may add beyond prim's curated tiers
    /// (AD-0012 Decision 6).
    OptIn,
}

/// One rule prim can run, and the tier at which it starts running.
///
/// There is no severity column: every rule prim runs is an error. prim reports
/// nothing it will not fail on, so a finding's presence is its severity. The
/// tier chooses *which* rules run, never how loudly they speak.
#[derive(Debug, Clone, Copy)]
struct RulePolicy {
    rule: &'static str,
    tier: Tier,
}

/// A rule that reports something objectively broken: a dead link, a dangling
/// reference, a malformed table. Runs in both tiers.
const fn defect(rule: &'static str) -> RulePolicy {
    RulePolicy {
        rule,
        tier: Tier::Floor,
    }
}

/// A rule that reports a documentation convention — decidable, but it fires on
/// documents that are otherwise fine. Runs under `prim_mdlint_strict`.
const fn convention(rule: &'static str) -> RulePolicy {
    RulePolicy {
        rule,
        tier: Tier::Convention,
    }
}
```

Then add the third constructor:

```rust
/// A rule outside both tiers that a repository may still add for a path with
/// `prim_mdlint_enable`. Admitted only when it is meaningful without a
/// repository-supplied option — see AD-0012 for the ones that are not.
const fn opt_in(rule: &'static str) -> RulePolicy {
    RulePolicy {
        rule,
        tier: Tier::OptIn,
    }
}
```

Rename `ACTIVE_RULES` to `SELECTABLE_RULES` (it now holds rules that are not
active by default) and append the three opt-in entries after the existing
convention entries:

```rust
    opt_in("MD013"),
    opt_in("MD014"),
    opt_in("MD069"),
];
```

- [ ] **Step 4: Add the selection type and rewrite activation**

Replace `is_active` and `is_disabled` in `crates/prim-fmt/src/mdlint.rs` with:

```rust
/// Which Markdown rules prim runs for one file.
///
/// Pure data: `prim-cli` resolves this from `.editorconfig` and hands it over,
/// so the engine never reads a configuration file. `enabled` is applied first
/// and `disabled` second, so a disable wins a conflict.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MdLintSelection {
    /// `prim_mdlint_strict` — adds the convention tier on top of the floor.
    pub strict: bool,
    /// `prim_mdlint_enable` — rule ids added for this path regardless of tier.
    pub enabled: Vec<String>,
    /// `prim_mdlint_disable` — rule ids removed from the result.
    pub disabled: Vec<String>,
}

/// Whether `ids` names `rule`, case-insensitively.
fn names(ids: &[String], rule: &str) -> bool {
    ids.iter().any(|id| id.eq_ignore_ascii_case(rule))
}

/// Whether `rule` runs for a file under this selection.
fn is_active(rule: &str, selection: &MdLintSelection) -> bool {
    let Some(policy) = SELECTABLE_RULES
        .iter()
        .find(|policy| policy.rule.eq_ignore_ascii_case(rule))
    else {
        // Not a rule prim will run at any tier or any enable.
        return false;
    };
    if names(&selection.disabled, rule) {
        return false;
    }
    match policy.tier {
        Tier::Floor => true,
        Tier::Convention => selection.strict || names(&selection.enabled, rule),
        Tier::OptIn => names(&selection.enabled, rule),
    }
}
```

- [ ] **Step 5: Give MD013 the width the formatter used**

Replace `prim_config` in `crates/prim-fmt/src/mdlint.rs` with:

```rust
/// prim's canonical rumdl configuration for one file's resolved [`Style`].
///
/// Two rules carry options prim sets for itself. Neither is a configuration
/// surface a repository can reach: there is still no way to configure a rule's
/// options (FR-3.3).
///
/// **MD025** counts a front-matter `title:` as a top-level heading by default,
/// so a page written the way Docusaurus and VitePress expect — front-matter
/// title for the sidebar, one body H1 for the rendered heading — reports a
/// duplicate title. Measured across six documentation sites, 123 of 139 MD025
/// findings were that shape and only 16 were two real H1s. An empty
/// `front-matter-title` stops the rule counting page metadata as a heading.
///
/// **MD013** defaults to a line length of 80 regardless of what the repository
/// asked for, so a repository setting `max_line_length = 120` and enabling the
/// rule would see prim's own output fail at a threshold nobody chose. prim
/// feeds it the width the formatter actually wrapped to.
/// `code-block-line-length = 0` is rumdl's "no limit": prim never reflows a
/// code block, and rewrapping a shell command changes what it says, so a wide
/// code sample is a finding with no correct fix. Headings stay checked — a
/// long heading is rewritable prose — and tables stay off at rumdl's own
/// default, agreeing with prim never reflowing a table.
fn prim_config(style: &Style) -> Config {
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
    config.rules.insert(
        "MD013".to_string(),
        RuleConfig {
            severity: None,
            values: BTreeMap::from([
                (
                    "line-length".to_string(),
                    toml::Value::Integer(style.effective_line_width() as i64),
                ),
                (
                    "code-block-line-length".to_string(),
                    toml::Value::Integer(0),
                ),
            ]),
        },
    );
    config
}
```

- [ ] **Step 6: Rewrite `lint`'s signature and body**

Replace `pub fn lint(source: &str, strict: bool, disabled: &[String])` in
`crates/prim-fmt/src/mdlint.rs` with the following, keeping the existing
doc-comment paragraphs about the file-level directive and the second-pass filter
and updating them to name `selection` instead of `strict`/`disabled`:

```rust
pub fn lint(source: &str, style: &Style, selection: &MdLintSelection) -> Vec<MdDiagnostic> {
    let mut selection = selection.clone();
    if let Some(strict) = file_level_strict_override(source) {
        selection.strict = strict;
    }
    let cfg = prim_config(style);
    let rules: Vec<_> = all_rules(&cfg)
        .into_iter()
        .filter(|rule| is_active(rule.name(), &selection))
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
        // let formatting proceed.
        Err(_) => return Vec::new(),
    };

    warnings
        .into_iter()
        .filter_map(|warning| {
            let rule = warning.rule_name?;
            if !is_active(&rule, &selection) {
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

Update the doc comment on `file_level_strict_override` only where it says
`strict` is "the caller's" — it now overrides `selection.strict`.

- [ ] **Step 7: Update the engine's remaining tests**

Every other test in `crates/prim-fmt/src/mdlint/tests.rs` and
`crates/prim-fmt/src/mdlint/tests/rule_fixtures.rs` calls
`lint(src, strict, &[])`. Replace each with
`lint(src, &Style::default(), &tier(strict))`.

Both files need `use crate::Style;`. `rule_fixtures.rs` also needs
`use super::tier;` — the helper is defined in the parent test module, and
`use super::super::*` reaches `mdlint`, not `mdlint::tests`. The
`RuleFixture.floor` field keeps its meaning; only the call changes.

- [ ] **Step 8: Update the exports**

In `crates/prim-fmt/src/lib.rs:37`:

```rust
pub use mdlint::{MdDiagnostic, MdLintSelection, is_known_rule, lint as lint_markdown};
```

- [ ] **Step 9: Make `MdLintPolicy` carry a selection**

In `crates/prim-cli/src/mdlint_policy.rs`, replace the `strict` and `disabled`
fields of `MdLintPolicy` with one:

```rust
pub struct MdLintPolicy {
    /// The rules prim runs for this path, ready to hand to the engine.
    pub selection: prim_fmt::MdLintSelection,
    /// Ids `prim_mdlint_disable` listed that name no rule prim runs in either
    /// tier, uppercased, in the order written.
    pub unknown: Vec<String>,
    /// Where `prim_mdlint_disable` was set, so a typo in it can be reported
    /// against the `.editorconfig` line that has to be edited.
    pub unknown_origin: SettingOrigin,
}
```

Update `policy_from` to build
`selection: MdLintSelection { strict:
strict_from(props), enabled: Vec::new(), disabled }`,
and update every `policy.strict` / `policy.disabled` reader in this file and its
tests to `policy.selection.strict` / `policy.selection.disabled`.

- [ ] **Step 10: Update the four call sites**

`crates/prim-cli/src/app.rs`, the stdin lint path (around line 265):

```rust
Some(FileKind::Markdown) => {
    let policy = crate::mdlint_policy::resolve(path);
    crate::mdlint_policy::UnknownRuleReporter::new().report(&policy);
    let style = editorconfig::resolve(path);
    let diagnostics = prim_fmt::lint_markdown(&input, &style, &policy.selection);
```

`crates/prim-cli/src/app.rs`, the walk path (around line 474) — `style` is
already bound by the loop at line 459:

```rust
unknown_rule_reporter.report(&markdown_policy);
let diagnostics =
    prim_fmt::lint_markdown(&original, &style, &markdown_policy.selection);
```

`crates/prim-cli/src/lsp/diagnostics.rs` (around line 44):

```rust
prim_fmt::FileKind::Markdown => {
    let policy = resolver.resolve_mdlint_policy(path);
    let style = resolver.resolve(path);
    unknown_rules.report(&policy);
    prim_fmt::lint_markdown(text, &style, &policy.selection)
```

`crates/prim-cli/src/provenance.rs` (around line 85): `policy.strict` becomes
`policy.selection.strict`, and `disable_value`'s `policy.disabled` becomes
`policy.selection.disabled`.

- [ ] **Step 11: Run the whole suite**

Run: `just check` Expected: PASS. No behaviour changed — `enabled` is empty
everywhere, and MD013's config entry is inert while MD013 is unselected.

- [ ] **Step 12: Commit**

```bash
just fmt
git add crates/
git commit -m "refactor(prim-fmt): select Markdown rules from one selection value (#123)"
```

---

### Task 3: Classify a rule id three ways

`is_known_rule` answers "does prim run this?", which was enough when the only
key subtracted. With an additive key an author needs to tell a deliberate
refusal from a typo, so classification becomes three-valued.

**Files:**

- Modify: `crates/prim-fmt/src/mdlint.rs` (replace `is_known_rule`)
- Modify: `crates/prim-fmt/src/lib.rs:37` (exports)
- Modify: `crates/prim-fmt/src/mdlint/tests.rs` (the classifier test)
- Modify: `crates/prim-cli/src/mdlint_policy.rs` (`disabled_from`'s call)

**Interfaces:**

- Produces: `prim_fmt::RuleReach { Selectable, Withheld, Unknown }` (`Debug`,
  `Clone`, `Copy`, `PartialEq`, `Eq`) and
  `prim_fmt::rule_reach(rule: &str) ->
  RuleReach`

- [ ] **Step 1: Write the failing test**

Add to `crates/prim-fmt/src/mdlint/tests.rs`:

```rust
#[test]
fn rule_reach_separates_selectable_withheld_and_unknown() {
    assert_eq!(rule_reach("MD045"), RuleReach::Selectable, "floor tier");
    assert_eq!(
        rule_reach("md033"),
        RuleReach::Selectable,
        "convention tier, matched case-insensitively"
    );
    assert_eq!(rule_reach("MD013"), RuleReach::Selectable, "opt-in");

    // rumdl has these; prim will not run them.
    assert_eq!(
        rule_reach("MD072"),
        RuleReach::Withheld,
        "sorting front-matter keys would break prim's semantics guarantee"
    );
    assert_eq!(rule_reach("MD082"), RuleReach::Withheld, "dropped by AD-0012");
    assert_eq!(
        rule_reach("MD063"),
        RuleReach::Withheld,
        "its only meaningful setting is a house-style choice prim will not impose"
    );
    assert_eq!(rule_reach("MD047"), RuleReach::Withheld, "formatter territory");
    assert_eq!(
        rule_reach("MD043"),
        RuleReach::Withheld,
        "needs a repository-supplied headings list prim has no surface for"
    );

    assert_eq!(rule_reach("MD999"), RuleReach::Unknown);
    assert_eq!(rule_reach("nonsense"), RuleReach::Unknown);
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p prim-fmt rule_reach_separates` Expected: FAIL to compile —
`rule_reach` and `RuleReach` do not exist.

- [ ] **Step 3: Replace `is_known_rule` with the classifier**

In `crates/prim-fmt/src/mdlint.rs`, delete `is_known_rule` and add:

```rust
/// How prim treats a rule id written in a `prim_mdlint_*` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleReach {
    /// prim can run this rule: it sits in one of prim's tiers, or
    /// `prim_mdlint_enable` can add it.
    Selectable,
    /// rumdl has this rule and prim will not run it — formatter territory, a
    /// rule that cannot fire under the flavor and context prim pins, one that
    /// needs an option prim has no surface to supply, or one a decision record
    /// excludes.
    Withheld,
    /// No rumdl rule has this id. A typo.
    Unknown,
}

/// Classify a rule id written in `.editorconfig`, so a deliberate refusal can
/// be reported differently from a typo.
///
/// `Withheld` is derived from rumdl's own registry rather than a
/// hand-maintained list, so it stays correct when rumdl adds rules. Building
/// that registry is not free, but this runs only while parsing a
/// `prim_mdlint_*` value — once per `.editorconfig` section that sets one,
/// not once per file.
pub fn rule_reach(rule: &str) -> RuleReach {
    if SELECTABLE_RULES
        .iter()
        .any(|policy| policy.rule.eq_ignore_ascii_case(rule))
    {
        return RuleReach::Selectable;
    }
    if all_rules(&prim_config(&Style::default()))
        .iter()
        .any(|known| known.name().eq_ignore_ascii_case(rule))
    {
        return RuleReach::Withheld;
    }
    RuleReach::Unknown
}
```

- [ ] **Step 4: Update the exports and the one caller**

`crates/prim-fmt/src/lib.rs:37`:

```rust
pub use mdlint::{MdDiagnostic, MdLintSelection, RuleReach, lint as lint_markdown, rule_reach};
```

In `crates/prim-cli/src/mdlint_policy.rs`, `disabled_from` replaces
`prim_fmt::is_known_rule(entry)` with:

```rust
if prim_fmt::rule_reach(entry) == prim_fmt::RuleReach::Selectable {
```

- [ ] **Step 5: Run the suite**

Run: `just check` Expected: PASS.

- [ ] **Step 6: Commit**

```bash
just fmt
git add crates/
git commit -m "feat(prim-fmt): classify a rule id as selectable, withheld or unknown (#123)"
```

---

### Task 4: Pin what "withheld" means

The design withholds nine rules from the enableable set. Eight of them cannot
fire under the flavor, context and defaults prim pins; MD063 can, and is
withheld by choice. Both facts are assumptions about a pinned dependency, and
both must fail a test if a rumdl bump changes them.

**Files:**

- Create: `crates/prim-fmt/src/mdlint/tests/withheld_rules.rs`
- Modify: `crates/prim-fmt/src/mdlint/tests.rs` (declare the module)

**Interfaces:**

- Consumes: `prim_config`, `rule_reach`, `RuleReach` from Tasks 2 and 3

- [ ] **Step 1: Create the fixture module**

Create `crates/prim-fmt/src/mdlint/tests/withheld_rules.rs`:

````rust
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
````

- [ ] **Step 2: Declare the module**

In `crates/prim-fmt/src/mdlint/tests.rs`, beside `mod rule_fixtures;`:

```rust
mod withheld_rules;
```

- [ ] **Step 3: Run the fixtures**

Run: `cargo test -p prim-fmt withheld` Expected: PASS. If any rule fires, stop:
the design's reason for withholding it is wrong and the spec needs revisiting
before continuing.

- [ ] **Step 4: Commit**

```bash
just fmt
git add crates/prim-fmt/src/mdlint/
git commit -m "test(prim-fmt): pin why each withheld Markdown rule stays unreachable (#123)"
```

---

### Task 5: `prim_mdlint_enable`, end to end

Resolve the new key, report the two rejection classes distinctly, and make the
whole thing work through the CLI.

**Files:**

- Create: `crates/prim-cli/src/mdlint_policy/tests.rs` (moved test module)
- Modify: `crates/prim-cli/src/mdlint_policy.rs` (the key, parsing, reporting)
- Modify: `crates/prim-cli/tests/lint_diagnostics.rs` (behaviour)

**Interfaces:**

- Consumes: `prim_fmt::rule_reach`, `prim_fmt::RuleReach`,
  `prim_fmt::MdLintSelection`
- Produces:
  - `crate::mdlint_policy::MDLINT_ENABLE_KEY: &str = "prim_mdlint_enable"`
  - `crate::mdlint_policy::RejectedRuleId { id: String, key: &'static str,
    reach: prim_fmt::RuleReach, origin: SettingOrigin }`
  - `MdLintPolicy { selection: MdLintSelection, rejected: Vec<RejectedRuleId> }`
  - `RejectedRuleReporter` replacing `UnknownRuleReporter`

- [ ] **Step 1: Move the test module out of the way**

`mdlint_policy.rs` is at 319 lines, over the 300-line soft limit, and this task
adds to it. Move its `#[cfg(test)] mod tests { ... }` block (from line 160 to
the end of the file) into a new `crates/prim-cli/src/mdlint_policy/tests.rs`,
dropping the `mod tests {` wrapper and its indentation, and leave behind:

```rust
#[cfg(test)]
mod tests;
```

Run `cargo test -p prim-cli mdlint_policy` and commit this move on its own:

```bash
just fmt
git add crates/prim-cli/src/mdlint_policy.rs crates/prim-cli/src/mdlint_policy/tests.rs
git commit -m "refactor(prim-cli): move the mdlint policy tests into their own file (#123)"
```

- [ ] **Step 2: Write the failing unit tests**

Add to `crates/prim-cli/src/mdlint_policy/tests.rs`:

```rust
#[test]
fn the_enable_list_splits_trims_and_uppercases() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD013 , md014\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(policy.selection.enabled, vec!["MD013", "MD014"]);
}

#[test]
fn a_narrower_section_replaces_the_wider_enable_list() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD013\n[docs/**.md]\nprim_mdlint_enable = MD014\n",
    )
    .unwrap();

    assert_eq!(
        resolve(&dir.path().join("a.md")).selection.enabled,
        vec!["MD013"]
    );
    assert_eq!(
        resolve(&dir.path().join("docs/g.md")).selection.enabled,
        vec!["MD014"],
        "EditorConfig replaces a value, it does not merge lists"
    );
}

#[test]
fn an_enable_value_of_unset_or_none_enables_nothing_and_is_not_rejected() {
    for value in ["unset", "UNSET", "none", "None"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            format!("root = true\n[*.md]\nprim_mdlint_enable = {value}\n"),
        )
        .unwrap();

        let policy = resolve(&dir.path().join("a.md"));
        assert!(policy.selection.enabled.is_empty(), "value: {value:?}");
        assert!(policy.rejected.is_empty(), "value: {value:?}");
    }
}

#[test]
fn a_withheld_id_is_rejected_separately_from_a_typo() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD072, MD999, MD013\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(policy.selection.enabled, vec!["MD013"]);
    let reached: Vec<_> = policy
        .rejected
        .iter()
        .map(|reject| (reject.id.as_str(), reject.reach, reject.key))
        .collect();
    assert_eq!(
        reached,
        vec![
            ("MD072", prim_fmt::RuleReach::Withheld, MDLINT_ENABLE_KEY),
            ("MD999", prim_fmt::RuleReach::Unknown, MDLINT_ENABLE_KEY),
        ]
    );
}

#[test]
fn each_key_attributes_its_own_rejects_to_its_own_line() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\nprim_mdlint_disable = MD998\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    let line_of = |id: &str| match &policy
        .rejected
        .iter()
        .find(|reject| reject.id == id)
        .expect("rejected id present")
        .origin
    {
        SettingOrigin::EditorConfig { line, .. } => *line,
        SettingOrigin::Default => panic!("{id} must be attributed to .editorconfig"),
    };
    assert_eq!(line_of("MD999"), 3, "the enable key is on line 3");
    assert_eq!(line_of("MD998"), 4, "the disable key is on line 4");
}

#[test]
fn a_selectable_id_the_path_does_not_run_is_not_rejected() {
    // MD013 is opt-in, so disabling it without enabling it changes nothing —
    // but it is a real rule prim can run, not a typo, and must not be reported
    // as one.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD013\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert!(policy.rejected.is_empty());
    assert_eq!(policy.selection.disabled, vec!["MD013"]);
}
```

Update the existing tests in this file that read `policy.unknown` to read
`policy.rejected`, matching on `reach` where they asserted an unknown id.

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p prim-cli mdlint_policy` Expected: FAIL to compile —
`MDLINT_ENABLE_KEY`, `policy.rejected` and `RejectedRuleId` do not exist.

- [ ] **Step 4: Implement resolution**

In `crates/prim-cli/src/mdlint_policy.rs`, add the key constant beside its
siblings:

```rust
pub(crate) const MDLINT_ENABLE_KEY: &str = "prim_mdlint_enable";
```

Replace `MdLintPolicy`'s `unknown`/`unknown_origin` fields with `rejected`, and
add the type:

```rust
/// An id a `prim_mdlint_*` key listed that prim will not act on, with the key
/// that listed it, why it was refused, and where it was written.
///
/// The origin is carried per id because two keys can each carry rejects, and a
/// message that names the wrong line sends the reader to a line with nothing
/// to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectedRuleId {
    /// The id as written, uppercased.
    pub id: String,
    /// The `.editorconfig` key that listed it.
    pub key: &'static str,
    /// Why prim refused it. Never [`prim_fmt::RuleReach::Selectable`].
    pub reach: prim_fmt::RuleReach,
    /// The `.editorconfig` file, line and section that set `key`.
    pub origin: SettingOrigin,
}
```

Replace `disabled_from` with a function that serves both keys:

```rust
/// Parse one comma-separated rule-id list out of already-resolved properties.
///
/// Entries are trimmed and uppercased, then split into ids prim can select
/// (kept) and ids it refuses (returned separately, dropped from the list
/// either way). This stays pure — reporting a refusal is a caller's job, so a
/// warning fires once per run per section rather than once per file (see
/// [`RejectedRuleReporter`]).
///
/// Returns `(accepted, rejected)`. The rejects carry a placeholder origin;
/// [`attribute`] fills it in only when there is something to report, because
/// recovering the section header re-reads the `.editorconfig`.
fn rule_ids_from(props: &Properties, key: &'static str) -> (Vec<String>, Vec<RejectedRuleId>) {
    let Some(raw) = props.get_raw_for_key(key).into_option() else {
        return (Vec::new(), Vec::new());
    };

    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        // `unset` is EditorConfig's own reserved word for "clear the inherited
        // value"; `none` is prim's own spelling of the same intent. Neither
        // names a rule, so neither is a refusal to report.
        if entry.eq_ignore_ascii_case("unset") || entry.eq_ignore_ascii_case("none") {
            continue;
        }
        let reach = prim_fmt::rule_reach(entry);
        if reach == prim_fmt::RuleReach::Selectable {
            accepted.push(entry.to_ascii_uppercase());
        } else {
            rejected.push(RejectedRuleId {
                id: entry.to_ascii_uppercase(),
                key,
                reach,
                origin: SettingOrigin::Default,
            });
        }
    }
    (accepted, rejected)
}

/// Resolve `key`'s `.editorconfig` origin once and stamp it on every id that
/// key rejected.
fn attribute(rejected: &mut [RejectedRuleId], props: &Properties, key: &str) {
    if rejected.is_empty() {
        return;
    }
    let origin = provenance::origin_of(props, key);
    for reject in rejected {
        reject.origin = origin.clone();
    }
}
```

Rewrite `policy_from`:

```rust
pub(crate) fn policy_from(props: &Properties) -> MdLintPolicy {
    let (enabled, mut rejected) = rule_ids_from(props, MDLINT_ENABLE_KEY);
    attribute(&mut rejected, props, MDLINT_ENABLE_KEY);
    let (disabled, mut disable_rejects) = rule_ids_from(props, MDLINT_DISABLE_KEY);
    attribute(&mut disable_rejects, props, MDLINT_DISABLE_KEY);
    rejected.append(&mut disable_rejects);

    MdLintPolicy {
        selection: prim_fmt::MdLintSelection {
            strict: strict_from(props),
            enabled,
            disabled,
        },
        rejected,
    }
}
```

Rename `UnknownRuleReporter` to `RejectedRuleReporter` and rewrite `report`:

```rust
/// Warn about each refused id in `policy`, attributed to the
/// `.editorconfig` file, line and section that set the key — that is where
/// it has to be fixed. Ids this reporter already warned about for the same
/// key and location are skipped.
pub fn report(&mut self, policy: &MdLintPolicy) {
    for reject in &policy.rejected {
        let location = provenance::location_of(&reject.origin);
        if !self
            .reported
            .insert((reject.key, location.clone(), reject.id.clone()))
        {
            continue;
        }
        let attribution = if location.is_empty() {
            String::new()
        } else {
            format!("{location}: ")
        };
        let reason = match reject.reach {
            prim_fmt::RuleReach::Withheld => "which prim does not run at any tier",
            _ => "which is not a rule prim knows",
        };
        ui::warning(&format!(
            "{attribution}{} lists '{}', {reason} — ignoring it",
            reject.key, reject.id
        ));
    }
}
```

Change the dedup set's element type to `(&'static str, String, String)`.

- [ ] **Step 5: Run the unit tests**

Run: `cargo test -p prim-cli mdlint_policy` Expected: PASS.

- [ ] **Step 6: Rename the reporter at its three call sites**

`UnknownRuleReporter` is constructed in `crates/prim-cli/src/app.rs` (twice) and
`crates/prim-cli/src/lsp/diagnostics.rs`. Rename each to `RejectedRuleReporter`.
Update the comment in `lsp/diagnostics.rs` that says "a typo'd rule id is
silently ignored" to name both refusal classes.

- [ ] **Step 7: Write the failing integration tests**

Add to `crates/prim-cli/tests/lint_diagnostics.rs`:

```rust
#[test]
fn enabling_an_opt_in_rule_makes_it_gate() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nmax_line_length = 40\nprim_mdlint_enable = MD013\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    // A heading is the one over-width thing prim's formatter will not wrap,
    // so this finding survives `prim fmt`.
    std::fs::write(
        &file,
        "# A heading far longer than the forty columns this repository asked for\n\nText.\n",
    )
    .unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn an_enabled_md013_uses_the_width_the_formatter_wrapped_to() {
    // rumdl's own MD013 default is 80. A repository asking for 120 and
    // enabling the rule must not have its own formatted prose reported.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nmax_line_length = 120\nprim_mdlint_enable = MD013\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    let prose = "word ".repeat(40);
    std::fs::write(&file, format!("# Title\n\n{prose}\n")).unwrap();

    prim().arg("fmt").arg(&file).assert().code(0);
    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn an_enabled_convention_rule_gates_without_the_strict_tier() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    // Opening with prose rather than a heading is an MD041 violation, so the
    // negative assertion below has something to catch: if enabling MD033 pulled
    // in its whole band, MD041 would report here.
    std::fs::write(
        &file,
        "Intro\n\n# Title\n\nText with <span>inline HTML</span>.\n",
    )
    .unwrap();

    let assert = prim().arg("lint").arg(&file).assert().code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("[MD033]"), "{stdout}");
    assert!(
        !stdout.contains("[MD041]"),
        "enabling one convention rule must not pull in the rest of its band:\n{stdout}"
    );
}

#[test]
fn disabling_beats_enabling_for_the_same_rule() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD033\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    std::fs::write(&file, "# Title\n\nText with <span>inline HTML</span>.\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn an_enabled_rule_survives_a_file_level_strict_opt_out() {
    // The directive moves the tier; it does not cancel an enable.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\nprim_mdlint_enable = MD013\nmax_line_length = 40\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    std::fs::write(
        &file,
        "<!-- prim-mdlint-strict: false -->\n\n# A heading far longer than the forty columns asked for\n",
    )
    .unwrap();

    let assert = prim().arg("lint").arg(&file).assert().code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("[MD013]"), "{stdout}");
}

#[test]
fn a_withheld_enabled_rule_warns_that_prim_does_not_run_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD072\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("prim_mdlint_enable"), "{stderr}");
    assert!(stderr.contains("MD072"), "{stderr}");
    assert!(
        stderr.contains("does not run"),
        "a withheld rule is a deliberate refusal, not a typo:\n{stderr}"
    );
    assert!(stderr.contains(".editorconfig:3 [*.md]"), "{stderr}");
}

#[test]
fn an_unknown_enabled_rule_warns_as_a_typo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("not a rule prim knows"), "{stderr}");
}

#[test]
fn fmt_never_warns_about_an_enabled_rule() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("fmt").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        !stderr.contains("MD999"),
        "prim fmt never consumes prim_mdlint_enable:\n{stderr}"
    );
}
```

- [ ] **Step 8: Run them to verify they fail, then pass**

Run: `cargo test -p prim-cli --test lint_diagnostics` Expected: the new tests
FAIL before Steps 4-6 land and PASS after. Nothing in this step should need
further production code — if a test still fails, the resolution or the reporting
is wrong, not the test.

- [ ] **Step 9: Run the whole suite**

Run: `just check` Expected: PASS.

- [ ] **Step 10: Commit**

```bash
just fmt
git add crates/
git commit -m "feat(prim-cli): add prim_mdlint_enable to the editorconfig surface (#123)"
```

---

### Task 6: `prim explain` surfaces the key

Every resolved `prim_*` key shows in `prim explain` with its origin. The new one
must too, or a repository cannot see what it resolved to.

**Files:**

- Modify: `crates/prim-cli/src/provenance.rs:80-120` (the setting and its
  renderer)
- Modify: `crates/prim-cli/tests/explain.rs` (behaviour)

**Interfaces:**

- Consumes: `MDLINT_ENABLE_KEY`, `MdLintPolicy.selection.enabled`

- [ ] **Step 1: Write the failing test**

Add to `crates/prim-cli/tests/explain.rs`, following the shape of the existing
`prim_mdlint_disable` tests:

```rust
#[test]
fn explain_reports_the_enable_key_with_its_origin() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD013, MD014\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n").unwrap();

    let assert = prim()
        .arg("explain")
        .arg(dir.path().join("a.md"))
        .assert()
        .code(0);
    let enable = line_for(&assert.get_output().stdout, "prim_mdlint_enable");
    assert!(enable.contains("MD013, MD014"), "{enable}");
    assert!(enable.contains(".editorconfig:3"), "{enable}");
}

#[test]
fn explain_distinguishes_an_unset_enable_key_from_a_cleared_one() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".editorconfig"), "root = true\n[*.md]\n").unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n").unwrap();
    let assert = prim()
        .arg("explain")
        .arg(dir.path().join("a.md"))
        .assert()
        .code(0);
    assert!(
        line_for(&assert.get_output().stdout, "prim_mdlint_enable").contains("unset"),
        "a key nothing in the cascade sets is unset"
    );

    let cleared = tempfile::tempdir().unwrap();
    std::fs::write(
        cleared.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = none\n",
    )
    .unwrap();
    std::fs::write(cleared.path().join("a.md"), "# Title\n").unwrap();
    let assert = prim()
        .arg("explain")
        .arg(cleared.path().join("a.md"))
        .assert()
        .code(0);
    assert!(
        line_for(&assert.get_output().stdout, "prim_mdlint_enable").contains("none"),
        "a key set to none was cleared on purpose"
    );
}

#[test]
fn only_markdown_reports_the_enable_key() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*]\nprim_mdlint_enable = MD013\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.json"), "{}\n").unwrap();

    let assert = prim()
        .arg("explain")
        .arg(dir.path().join("a.json"))
        .assert()
        .code(0);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(!stdout.contains("prim_mdlint_enable"), "{stdout}");
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p prim-cli --test explain` Expected: FAIL — no
`prim_mdlint_enable` line in the output.

- [ ] **Step 3: Generalise the renderer and add the setting**

In `crates/prim-cli/src/provenance.rs`, rename `disable_value` to
`rule_list_value` and take the list rather than the whole policy, keeping its
doc comment and extending the first sentence to name both keys:

```rust
fn rule_list_value(ids: &[String], origin: &SettingOrigin) -> String {
    if !ids.is_empty() {
        return ids.join(", ");
    }
    match origin {
        SettingOrigin::Default => "unset".to_string(),
        SettingOrigin::EditorConfig { .. } => "none".to_string(),
    }
}
```

Then, in `Resolver::explain`, after the `prim_mdlint_strict` setting and before
the disable one:

```rust
let enable_origin = origin_of(&props, MDLINT_ENABLE_KEY);
settings.push(ResolvedSetting {
    key: MDLINT_ENABLE_KEY,
    value: rule_list_value(&policy.selection.enabled, &enable_origin),
    origin: enable_origin,
});
let disable_origin = origin_of(&props, MDLINT_DISABLE_KEY);
settings.push(ResolvedSetting {
    key: MDLINT_DISABLE_KEY,
    value: rule_list_value(&policy.selection.disabled, &disable_origin),
    origin: disable_origin,
});
```

Add `MDLINT_ENABLE_KEY` to the `use crate::mdlint_policy::{...}` import, and
update the `explain` doc comment where it says "both `prim_mdlint_*` keys" to
say all three.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p prim-cli --test explain` Expected: PASS.

- [ ] **Step 5: Refresh the CLI snapshots**

`prim explain`'s output gains a line, so any `trycmd` case under `spec/` that
shows it needs updating.

Run: `TRYCMD=overwrite cargo test -p prim-spec`, then `git diff spec/` and read
every changed line — the only change should be the added `prim_mdlint_enable`
row.

- [ ] **Step 6: Commit**

```bash
just fmt
git add crates/ spec/
git commit -m "feat(prim-cli): show prim_mdlint_enable in prim explain (#123)"
```

---

### Task 7: Measure MD013 against a public corpus

The design's one corpus-refutable claim: prim's own formatted output passes an
enabled MD013 at the effective width, with headings the only expected finding. A
finding that is not a heading refutes Decision 3 and sends `prim_config` back
for another option.

**Files:**

- No repository files. The harness lives in the scratchpad; only the resulting
  figures are committed, in Task 8.

- [ ] **Step 1: Build the corpus**

Work in the scratchpad, not the repository. Clone the documentation trees of six
public sites that _use_ a generator rather than being the generator's own
repository, matching AD-0012's non-Rust corpus:

```bash
# Use this session's scratchpad directory, not /tmp and not the repository.
CORPUS="$SCRATCHPAD/md013-corpus"
mkdir -p "$CORPUS/clones" "$CORPUS/sources" && cd "$CORPUS/clones"
git clone --depth 1 https://github.com/facebook/react-native-website
git clone --depth 1 https://github.com/fastapi/fastapi
git clone --depth 1 https://github.com/crytic/building-secure-contracts
git clone --depth 1 https://github.com/vuejs/docs vuejs-docs
git clone --depth 1 https://github.com/reduxjs/redux
git clone --depth 1 https://github.com/vitejs/vite

# One documentation tree per project, so the corpus holds documentation and
# not each project's own source, tests and tooling.
cp -R clones/react-native-website/docs      "$CORPUS/sources/react-native"
cp -R clones/fastapi/docs/en/docs           "$CORPUS/sources/fastapi"
cp -R clones/building-secure-contracts      "$CORPUS/sources/secure-contracts"
cp -R clones/vuejs-docs/src                 "$CORPUS/sources/vue"
cp -R clones/redux/docs                     "$CORPUS/sources/redux"
cp -R clones/vite/docs                      "$CORPUS/sources/vite"
rm -rf "$CORPUS/sources"/*/.git
```

Record the per-project Markdown file count:

```bash
for project in "$CORPUS/sources"/*; do
  printf '%s\t%s\n' "$(basename "$project")" "$(find "$project" -name '*.md' | wc -l)"
done
```

- [ ] **Step 2: Run one width per pass**

For each width in 80, 100 and 120: copy the corpus to a fresh directory, write
an `.editorconfig` at its root, format, then lint.

```bash
for WIDTH in 80 100 120; do
  RUN="$CORPUS/run-$WIDTH"
  rm -rf "$RUN" && cp -R "$CORPUS/sources" "$RUN"
  printf 'root = true\n[*.md]\nmax_line_length = %s\nprim_mdlint_enable = MD013\n' "$WIDTH" \
    > "$RUN/.editorconfig"
  cargo run --release -- fmt "$RUN" > "$CORPUS/fmt-$WIDTH.log" 2>&1
  cargo run --release -- lint --format json "$RUN" > "$CORPUS/lint-$WIDTH.json" 2>&1
done
```

Note in the log any file `prim fmt` could not parse; AD-0012 quarantined one for
a debug-build panic (issue #115), so a failure there is expected history rather
than a new defect.

- [ ] **Step 3: Classify every MD013 finding**

For each width, extract the MD013 findings and read the source line each one
points at. Classify each as: a heading, a table row, a code-block line, or
prose. Record per-project prevalence as well as per-file counts.

The expected result is that every finding is a heading. A table row is a
surprise worth investigating (`tables` defaults off, so one would mean the
option is not reaching the rule). A code-block line means
`code-block-line-length = 0` is not being applied. **Prose refutes the design**:
it means dprint left a line dprint could not break and rumdl's non-strict
forgiveness did not exempt.

- [ ] **Step 4: Decide**

If every finding is a heading, record the figures and continue to Task 8.

If prose findings appear, stop and take them back to the spec. Read the
offending lines before proposing anything: the fix is either an additional MD013
option prim pins, or admitting that MD013 cannot be offered at all. Do not
change `prim_config` on a guess — the whole point of this task is that the
option is chosen from what real documents contain.

- [ ] **Step 5: Write the figures down**

Save a short summary in the scratchpad — per-project file counts, findings per
width, the classification breakdown, and the sampling caveats — for Task 8 to
quote. Nothing is committed in this task.

---

### Task 8: Documentation and the AD-0012 amendment

**Files:**

- Modify: `docs/decisions/0012-markdown-lint-bands-and-rule-exclusion.md`
- Modify: `docs/SPEC.md:90-125` (FR-3.2a, new FR-3.2d, FR-3.3), `:299-345`
  (FR-5.5b's rule lists)
- Modify: `docs/USAGE.md:100-175` (the lint section and Configuration)
- Modify: `docs/recipes.md`
- Modify: `AGENTS.md` (the subtract-only sentence)

- [ ] **Step 1: Amend AD-0012**

Edit the record in place — do not write a new one. Four changes:

1. **Status** — append: _"Amended 2026-08-28 (issue #123): a second
   `.editorconfig` key, `prim_mdlint_enable`, adds named rules to the set prim
   runs for a path. The subtract-only guarantee below is narrowed accordingly."_
2. **Context** — add a subsection, "Why an additive key reaches three rules and
   not twelve", carrying the table from the design spec (which of the off-list
   rules can fire and why), the MD063 finding, and the corpus figures from Task
   7.
3. **Decision** — add item 6, `prim_mdlint_enable`: its cascade semantics, the
   three admission classes, the reporting contract, `disable` winning a
   conflict, tier independence, and MD013's two prim-owned options. Amend item
   4's sentence "it can never add a rule prim decided not to run … prim's
   curated set stays the ceiling" to name Decision 6 as the exception, and note
   that `ACTIVE_RULES` is now `SELECTABLE_RULES` (item 2 refers to it by name).
4. **Alternatives considered** — change alternative 6 from _Rejected_ to
   _Superseded by Decision 6_, keeping the original reasoning and adding one
   sentence: the objection to a shared key stands, and Decision 6 uses a
   separate one. Then add the alternatives from the design spec that this work
   rejected — the whole off-list, a per-rule options surface, opt-in rules only,
   rumdl's MD013 defaults, prose-only MD013, and a new AD-0013.
5. **Consequences** — record `lint_markdown`'s second signature change and the
   `FR-3.3` split (its first clause survives, its second gains an exception).

- [ ] **Step 2: Update `docs/SPEC.md`**

Add `prim_mdlint_enable` to FR-3.2a's list of recognised keys. Add **FR-3.2d**
after FR-3.2c, mirroring its wording: the comma-separated list, the per-glob
cascade with last-match-wins replacement, case-insensitive ids, `unset`/`none`
clearing the list, `disable` applied after `enable`, tier independence, the two
refusal classes each reported once per run per section without changing the exit
code, and the enableable set named explicitly.

Amend **FR-3.3**: keep the first sentence exactly as it is, and replace the
`prim_mdlint_disable` sentence with one that states both keys — the disable key
narrows, the enable key widens by at most the three opt-in rules, and neither
changes a rule's behaviour or its options.

In FR-5.5b, split the "Off in both tiers" list into **"Opt-in via
`prim_mdlint_enable`"** (MD013, MD014, MD069) and **"Withheld"**, with the
reason each of the rest is withheld: needing a repository-supplied list (MD043,
MD044, MD054, MD061, MD081), a flavor prim does not pin (MD074, MD078, MD079), a
house-style choice prim will not impose (MD063), prim's semantics-preserving
guarantee (MD072), or AD-0012's own removal (MD082). Add MD013's two prim-owned
options beside MD025's in "Rule configuration prim owns".

- [ ] **Step 3: Update `docs/USAGE.md` and `docs/recipes.md`**

In `USAGE.md`, mirror the SPEC's re-partitioned lists in the `lint` section, add
the key beside "Per-glob rule exclusion", and document it in Configuration with
the same reach, precedence and reporting rules.

In `recipes.md`, add a recipe for the case the issue was filed for: a repository
that wants MD013 at its own width, and one that wants a single convention rule
without the strict tier.

- [ ] **Step 4: Update `AGENTS.md`**

Its Key design decisions section says the one subtract-only exception is
`prim_mdlint_disable`. Replace with a sentence naming both keys: the disable key
removes a rule from the tier prim selected; the enable key adds one, reaching
prim's own tiers plus three opt-in rules, and never configures a rule's options
(AD-0012).

- [ ] **Step 5: Run the full gate**

Run: `just verify` Expected: PASS — tests, install tests, clippy, `rustfmt`,
prim's own Markdown, and commit lint.

- [ ] **Step 6: Commit and open the PR**

```bash
just fmt
git add docs/ AGENTS.md
git commit -m "docs(prim): amend AD-0012 for prim_mdlint_enable (#123)"
git push -u origin feat/123-mdlint-enable-key
gh pr create --fill
```

- [ ] **Step 7: Garden the working memory**

`docs/wip/` still holds this design and plan, plus the two AD-0012 notes from
August. A `main`-targeting PR with a non-empty `docs/wip/` is unfinished work.
Run the `sdd-gardening` skill before the PR merges.
