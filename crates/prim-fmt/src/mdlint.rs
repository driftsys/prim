//! Markdown content linting via the `rumdl` crate — **lint-only**.
//!
//! prim owns Markdown *formatting* through `dprint-plugin-markdown` (see
//! [`crate::markdown`]). This module adds a *content* linter on top: it reports
//! issues a formatter cannot and **never rewrites** — prim never invokes rumdl's
//! formatter, LSP, or file walker, only [`rumdl_lib::lint`].
//!
//! Story G3 (#59) splits rumdl's rules into three tiers. The floor tier is
//! always on and runs the defect rules — rules that report something
//! objectively broken. `.editorconfig` `prim_mdlint_strict = true` adds the
//! convention tier on top. A third tier, opt-in, holds rules that run in
//! neither the floor nor the convention tier by default — `.editorconfig`
//! `prim_mdlint_enable` adds specific rule ids from it (any tier, including
//! opt-in) for a path regardless of the strict setting; `prim_mdlint_disable`
//! removes ids from whatever the tier and enable list selected. Every rule
//! prim runs is an error: there is no warning severity, so a finding's
//! presence is its severity.
//!
//! Story G5 (#61) adds the surgical override surface on top: a standalone
//! `<!-- prim-mdlint-strict: true|false -->` line anywhere in the file
//! overrides the `.editorconfig`-resolved strict tier for that file only.
//! rumdl's own inline directives (`rumdl-disable`/`markdownlint-disable` +
//! line/next-line/file scoping) need no wiring here — `rumdl_lib::lint`
//! already applies them internally regardless of prim's `source_file: None`.
//!
//! Key guarantees:
//!
//! - `rumdl = "=0.2.35"` links with `default-features = false` (no
//!   tokio/tower-lsp/notify/rayon), so the engine stays pure and small.
//! - rules are selected by [`rumdl_lib::rule::Rule::name`] from the full
//!   `all_rules(&cfg)` set, so off / formatter-territory rules never run.
//! - `rumdl_lib::lint` returns 1-indexed `line`/`column` diagnostics — the
//!   line:col that stories B1/D2 want (and which serde-based formats lack, per
//!   spike #42).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use rumdl_lib::config::{Config, MarkdownFlavor, RuleConfig};
use rumdl_lib::rules::all_rules;

use crate::Style;

/// A single Markdown content-lint finding, mapped out of rumdl's `LintWarning`
/// so callers never touch a rumdl type. Positions are 1-indexed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MdDiagnostic {
    /// The rumdl rule code, e.g. `"MD034"`.
    pub rule: String,
    /// 1-indexed line of the finding.
    pub line: usize,
    /// 1-indexed column of the finding.
    pub column: usize,
    /// Always `true`: every rule prim runs is an error, so a reported
    /// finding's presence is its severity. There is no warning severity.
    pub is_error: bool,
    /// Human-readable message from rumdl.
    pub message: String,
}

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

/// A rule outside both tiers that a repository may still add for a path with
/// `prim_mdlint_enable`. Admitted only when it is meaningful without a
/// repository-supplied option — see AD-0012 for the ones that are not.
const fn opt_in(rule: &'static str) -> RulePolicy {
    RulePolicy {
        rule,
        tier: Tier::OptIn,
    }
}

const SELECTABLE_RULES: &[RulePolicy] = &[
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
    opt_in("MD013"),
    opt_in("MD014"),
    opt_in("MD069"),
];

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

/// How prim treats a rule id written in a `prim_mdlint_*` key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleReach {
    /// prim can run this rule: it sits in one of prim's tiers, or
    /// `prim_mdlint_enable` can add it.
    Selectable,
    /// rumdl has this rule and prim will not run it — formatter territory, a
    /// rule that cannot fire under the flavor and context prim pins, one that
    /// needs an option prim has no surface to supply, one that would break
    /// prim's semantics-preserving guarantee, or one a decision record
    /// excludes.
    Withheld,
    /// No rumdl rule has this id. A typo.
    Unknown,
}

/// Classify a rule id written in `.editorconfig`, so a deliberate refusal can
/// be reported differently from a typo.
///
/// `Withheld` is derived from rumdl's own registry rather than a
/// hand-maintained list, so it stays correct when rumdl adds rules. This
/// function runs once per Markdown file whose resolved `.editorconfig` list
/// carries an id outside `SELECTABLE_RULES`: the `Selectable` check returns
/// before the registry is ever consulted, so the common case — every
/// configured id is one prim already runs — never touches it. Consulting the
/// registry does not rebuild it: see [`withheld_rule_names`].
pub fn rule_reach(rule: &str) -> RuleReach {
    if SELECTABLE_RULES
        .iter()
        .any(|policy| policy.rule.eq_ignore_ascii_case(rule))
    {
        return RuleReach::Selectable;
    }
    if withheld_rule_names()
        .iter()
        .any(|known| known.eq_ignore_ascii_case(rule))
    {
        return RuleReach::Withheld;
    }
    RuleReach::Unknown
}

/// The full set of rule ids rumdl exposes, built once per process and cached.
///
/// The id set does not depend on the [`Style`] passed to [`prim_config`] —
/// only the registered rules' own names — and rumdl's rule set is a fixed
/// property of the pinned `rumdl = "=0.2.35"` dependency, so it cannot change
/// between calls within a run. A single build is correct for the whole
/// process, which matters because [`rule_reach`] is called once per Markdown
/// file for every non-selectable `.editorconfig` id, and a later
/// `prim_mdlint_enable` key doubles that call site.
fn withheld_rule_names() -> &'static [&'static str] {
    static NAMES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            all_rules(&prim_config(&Style::default()))
                .iter()
                .map(|rule| rule.name())
                .collect()
        })
        .as_slice()
}

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

/// Lint `source` as Markdown content, returning prim's own diagnostics.
///
/// `selection` chooses which rules run: the floor tier is always on,
/// `selection.strict` adds the convention tier on top, and
/// `selection.enabled` adds specific rule ids regardless of tier.
/// `selection.disabled` subtracts rule ids (case-insensitive) from whatever
/// the tier and `enabled` selected — it can only narrow the active set, never
/// add to it. A file-level `<!-- prim-mdlint-strict: true|false -->`
/// directive (story G5, #61) overrides `selection.strict` for this file
/// only — a surgical, per-file escape hatch on top of the
/// `.editorconfig`-resolved default, matching the same precedence rumdl's own
/// `rumdl-disable`/`markdownlint-disable` inline directives already get
/// (rumdl applies those inside `rumdl_lib::lint` itself, independent of
/// prim's tier table). Lint-only: `source` is never modified. Rules are
/// filtered from the full rumdl set by name so off/formatter-territory rules
/// never run.
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
            // The same predicate that chose `rules` above, applied again to
            // what came back. It guards the subtract-only guarantee: a rule
            // outside the selection must never reach a caller as a finding.
            //
            // Under the pinned `rumdl = "=0.2.35"` this second pass is
            // unexercised, because `rumdl_lib::lint` only ever names a rule
            // from the slice it was handed. That is an assumption about a
            // dependency, not a property prim controls, and no test can reach
            // the branch: `lint` is the only entry point, and it builds that
            // slice itself from this very predicate, so no input can make the
            // two disagree. The check stays as the guarantee's last line of
            // defence if a future rumdl reports a finding under a related
            // rule's name.
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

/// Scan `source` for a standalone `<!-- prim-mdlint-strict: true|false -->`
/// line (the whole line, once trimmed, must be exactly that comment) and
/// return its boolean, or `None` if no such line is present. When several
/// occurrences exist, the last one wins — consistent with a flat, top-to-
/// bottom read of the file rather than a cascade. An unparseable value (e.g.
/// `yes`) is ignored so a typo silently falls back to `selection.strict`
/// rather than erroring the whole lint run.
fn file_level_strict_override(source: &str) -> Option<bool> {
    source
        .lines()
        .filter_map(|line| directive_value(line.trim()))
        .next_back()
}

/// Parse one standalone `<!-- prim-mdlint-strict: <value> -->` line into its
/// boolean, or `None` if `line` isn't exactly that directive (wrong key,
/// missing comment delimiters, or an unrecognized value).
fn directive_value(line: &str) -> Option<bool> {
    let inner = line.strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let (key, value) = inner.split_once(':')?;
    if key.trim() != "prim-mdlint-strict" {
        return None;
    }
    match value.trim().to_ascii_lowercase().as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
