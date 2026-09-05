//! Markdown content linting via the `rumdl` crate — **lint-only**.
//!
//! prim owns Markdown *formatting* through `dprint-plugin-markdown` (see
//! [`crate::markdown`]). This module adds a *content* linter on top: it reports
//! issues a formatter cannot and **never rewrites** — prim never invokes rumdl's
//! formatter, LSP, or file walker, only [`rumdl_lib::lint`].
//!
//! Story G3 (#59) splits rumdl's rules into two bands. The floor tier is
//! always on and runs the defect rules — rules that report something
//! objectively broken. `.editorconfig` `prim_mdlint_strict = true` adds the
//! convention tier on top. Every rule prim runs is an error: there is no
//! warning severity, so a finding's presence is its severity.
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

use rumdl_lib::config::{Config, MarkdownFlavor, RuleConfig};
use rumdl_lib::rules::all_rules;
use rumdl_lib::types::LineLength;

mod section_sign;

use self::section_sign::without_section_sign_false_positives;

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

/// The one rule outside the tier model. `prim_mdlint_report_line_length`
/// selects MD013 into whichever tier the path already runs, so it is absent
/// from [`ACTIVE_RULES`] and gated on the resolved line length instead.
///
/// prim pins the five of its options that decide what it reports (see
/// [`prim_config`]) because MD013 is the only rule whose behaviour must track
/// the formatter's own line width. MD013 has many more options than that;
/// the rest keep rumdl's defaults.
const LINE_LENGTH_RULE: &str = "MD013";

/// The flavor both passes lint under. Shared rather than named twice, so the
/// suppression can never be computed under different anchor rules than the
/// findings it filters.
const FLAVOR: MarkdownFlavor = MarkdownFlavor::Standard;

const ACTIVE_RULES: &[RulePolicy] = &[
    defect("MD042"),
    defect("MD011"),
    defect("MD052"),
    defect("MD056"),
    defect("MD062"),
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
///
/// `line_length` carries the resolved `max_line_length` when
/// `prim_mdlint_report_line_length` selected [`LINE_LENGTH_RULE`], and `None`
/// when it did not. That rule is gated on it alone: the tier chooses one of its
/// options, not whether it runs.
fn is_active(rule: &str, strict: bool, line_length: Option<usize>) -> bool {
    if rule == LINE_LENGTH_RULE {
        return line_length.is_some();
    }
    ACTIVE_RULES
        .iter()
        .any(|policy| policy.rule == rule && (policy.floor || strict))
}

/// Whether `rule` names a rule prim can run in either tier. Callers validating
/// user-supplied rule ids use this so a typo can be reported rather than
/// silently matching nothing.
pub fn is_known_rule(rule: &str) -> bool {
    rule.eq_ignore_ascii_case(LINE_LENGTH_RULE)
        || ACTIVE_RULES
            .iter()
            .any(|policy| policy.rule.eq_ignore_ascii_case(rule))
}

/// Whether `rule` was excluded for this file by `prim_mdlint_disable`.
fn is_disabled(rule: &str, disabled: &[String]) -> bool {
    disabled
        .iter()
        .any(|excluded| excluded.eq_ignore_ascii_case(rule))
}

/// prim's canonical rumdl configuration.
///
/// MD025 counts a front-matter `title:` as a top-level heading by default, so a
/// page written the way Docusaurus and VitePress expect — front-matter title for
/// the sidebar, one body H1 for the rendered heading — reports a duplicate
/// title. Measured across six documentation sites, 123 of 139 MD025 findings
/// were that shape and only 16 were two real H1s. An empty `front-matter-title`
/// stops the rule counting page metadata as a heading.
///
/// When `line_length` is `Some`, MD013 runs, and prim sets all five of its
/// options. Four are constants that follow from what the formatter does:
/// `code-blocks` and `code-spans` are off because prim preserves fenced content
/// verbatim (FR-1.6) and keeps an unbreakable inline span on one line
/// (FR-1.1a); `tables` is off because a table row cannot carry a line break,
/// pinned rather than inherited so a future rumdl default cannot change prim's
/// output. `headings` is the fifth and the only one that varies: prim cannot
/// wrap a heading — a line break would end it and turn the remainder into a
/// paragraph — but an author can shorten the wording, which is the strict
/// tier's definition of a finding.
///
/// The limit is written to both `global.line_length` and MD013's own
/// `line-length`. rumdl decides between them with a sentinel check — it
/// overwrites the rule value with the global one whenever the rule value still
/// equals the default 80 — so an explicit 80 beside a different global would
/// silently resolve to the global. Writing the same value to both makes every
/// branch of that check produce the same answer, and keeps prim correct if
/// rumdl's precedence ever changes.
///
/// This is prim choosing its canonical defaults, not a user-facing surface:
/// there is still no way for a repository to configure a rule's options. The
/// one value a repository supplies is `max_line_length`, which prim already
/// wraps to, so the formatter and the linter cannot disagree.
fn prim_config(strict: bool, line_length: Option<usize>) -> Config {
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
    if let Some(limit) = line_length {
        // `max_line_length` is a repository-supplied `usize`, and two values
        // need bounding before they reach rumdl. Zero means "no limit" to
        // `LineLength`, so the key would be a silent no-op; clamping to 1 at
        // least keeps the rule running. Neither end reports anything at zero —
        // the formatter emits one word per line and rumdl forgives a line that
        // is a single unbreakable token — so this is about keeping the config
        // well-formed, not about the two agreeing. A value above `i64::MAX` wraps
        // negative in the TOML conversion below, and rumdl rejects the whole
        // rule config and falls back to its own defaults — unpinning every
        // option set here, with `code-blocks` and `headings` back on, behind
        // three lines of rumdl's own stderr output, not a prim diagnostic.
        let limit = limit.clamp(1, i64::MAX as usize);
        config.global.line_length = LineLength::new(limit);
        config.rules.insert(
            LINE_LENGTH_RULE.to_string(),
            RuleConfig {
                severity: None,
                values: BTreeMap::from([
                    (
                        "line-length".to_string(),
                        toml::Value::Integer(limit as i64),
                    ),
                    // rumdl evaluates every skip below inside `if !strict`, so
                    // the three pins that follow are only in force while MD013's
                    // own `strict` is false. Pinning it, and `stern` with it,
                    // keeps them from becoming no-ops if rumdl's default moves.
                    ("strict".to_string(), toml::Value::Boolean(false)),
                    ("stern".to_string(), toml::Value::Boolean(false)),
                    ("code-blocks".to_string(), toml::Value::Boolean(false)),
                    ("code-spans".to_string(), toml::Value::Boolean(false)),
                    ("tables".to_string(), toml::Value::Boolean(false)),
                    ("headings".to_string(), toml::Value::Boolean(strict)),
                ]),
            },
        );
    }
    config
}

/// Lint `source` as Markdown content, returning prim's own diagnostics.
///
/// `strict = false` runs the always-on floor tier (defect rules only);
/// `strict = true` adds the convention tier on top. The tier chooses which
/// rules run — except `LINE_LENGTH_RULE` (`MD013`), where `line_length`
/// decides that and the tier chooses only whether headings are examined.
/// Every rule that runs reports an error.
///
/// `line_length` carries the resolved `max_line_length` when
/// `prim_mdlint_report_line_length` selected MD013, and `None` when it did
/// not. `None` is the previous behaviour: MD013 does not run. `disabled` subtracts
/// rule ids (case-insensitive) from whichever tier is selected — it can only
/// narrow the active set, never add to it. A file-level
/// `<!-- prim-mdlint-strict: true|false -->` directive (story G5, #61)
/// overrides `strict` for this file only — a surgical, per-file escape hatch
/// on top of the `.editorconfig`-resolved default, matching the same
/// precedence rumdl's own `rumdl-disable`/`markdownlint-disable` inline
/// directives already get (rumdl applies those inside `rumdl_lib::lint`
/// itself, independent of prim's tier table). Lint-only: `source` is
/// never modified. Rules are filtered from the full rumdl set by name so
/// off/formatter-territory rules never run.
pub fn lint(
    source: &str,
    strict: bool,
    disabled: &[String],
    line_length: Option<usize>,
) -> Vec<MdDiagnostic> {
    let strict = file_level_strict_override(source).unwrap_or(strict);
    let cfg = prim_config(strict, line_length);
    let rules: Vec<_> = all_rules(&cfg)
        .into_iter()
        .filter(|rule| {
            is_active(rule.name(), strict, line_length) && !is_disabled(rule.name(), disabled)
        })
        .collect();

    // `source_file = None` keeps this pure (no path/I/O); `verbose = false`.
    let warnings = match rumdl_lib::lint(source, &rules, false, FLAVOR, None, Some(&cfg)) {
        Ok(warnings) => warnings,
        // A linter failure must never corrupt a format run: report nothing and
        // let formatting proceed. Real error surfacing is G2's contract.
        Err(_) => return Vec::new(),
    };

    let diagnostics = warnings
        .into_iter()
        .filter_map(|warning| {
            let rule = warning.rule_name?;
            // The same predicate that chose `rules` above, applied again to
            // what came back. It guards the subtract-only guarantee: a rule
            // outside the selected tier, or one `prim_mdlint_disable`
            // removed, must never reach a caller as a finding.
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
            if !is_active(&rule, strict, line_length) || is_disabled(&rule, disabled) {
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
        .collect();

    without_section_sign_false_positives(source, &cfg, diagnostics, |rule| {
        is_active(rule, strict, line_length) && !is_disabled(rule, disabled)
    })
}

/// Scan `source` for a standalone `<!-- prim-mdlint-strict: true|false -->`
/// line (the whole line, once trimmed, must be exactly that comment) and
/// return its boolean, or `None` if no such line is present. When several
/// occurrences exist, the last one wins — consistent with a flat, top-to-
/// bottom read of the file rather than a cascade. An unparseable value (e.g.
/// `yes`) is ignored so a typo silently falls back to the caller's `strict`
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
