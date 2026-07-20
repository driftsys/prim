//! Markdown content linting via the `rumdl` crate — **lint-only**.
//!
//! prim owns Markdown *formatting* through `dprint-plugin-markdown` (see
//! [`crate::markdown`]). This module adds a *content* linter on top: it reports
//! issues a formatter cannot and **never rewrites** — prim never invokes rumdl's
//! formatter, LSP, or file walker, only [`rumdl_lib::lint`].
//!
//! Story G3 (#59) defines prim's own off/warn/error matrix over rumdl's rules.
//! The floor tier is always on; `.editorconfig` `prim_mdlint_strict = true`
//! enables the strict tier, which adds strict-only rules and escalates selected
//! warnings to errors. prim's severity is derived from that matrix, not from
//! rumdl's built-in defaults.
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

use rumdl_lib::config::{Config, MarkdownFlavor};
use rumdl_lib::rules::all_rules;

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
    /// Whether prim treats the finding as an error (fails) or a warning.
    pub is_error: bool,
    /// Human-readable message from rumdl.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrimSeverity {
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy)]
struct RulePolicy {
    rule: &'static str,
    floor: Option<PrimSeverity>,
    strict: Option<PrimSeverity>,
}

const fn rule(
    rule: &'static str,
    floor: Option<PrimSeverity>,
    strict: Option<PrimSeverity>,
) -> RulePolicy {
    RulePolicy {
        rule,
        floor,
        strict,
    }
}

const ACTIVE_RULES: &[RulePolicy] = &[
    rule("MD045", Some(PrimSeverity::Warn), Some(PrimSeverity::Error)),
    rule(
        "MD042",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD011",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD052",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD056",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD062",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD034",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule(
        "MD057",
        Some(PrimSeverity::Error),
        Some(PrimSeverity::Error),
    ),
    rule("MD024", Some(PrimSeverity::Warn), Some(PrimSeverity::Error)),
    rule("MD051", Some(PrimSeverity::Warn), Some(PrimSeverity::Error)),
    rule("MD080", Some(PrimSeverity::Warn), Some(PrimSeverity::Error)),
    rule("MD075", Some(PrimSeverity::Warn), Some(PrimSeverity::Error)),
    rule("MD066", None, Some(PrimSeverity::Error)),
    rule("MD068", None, Some(PrimSeverity::Error)),
    rule("MD070", None, Some(PrimSeverity::Error)),
    rule("MD025", None, Some(PrimSeverity::Warn)),
    rule("MD041", None, Some(PrimSeverity::Warn)),
    rule("MD001", None, Some(PrimSeverity::Warn)),
    rule("MD040", None, Some(PrimSeverity::Warn)),
    rule("MD033", None, Some(PrimSeverity::Warn)),
    rule("MD026", None, Some(PrimSeverity::Warn)),
    rule("MD036", None, Some(PrimSeverity::Warn)),
    rule("MD059", None, Some(PrimSeverity::Warn)),
    rule("MD053", None, Some(PrimSeverity::Warn)),
    rule("MD073", None, Some(PrimSeverity::Warn)),
    rule("MD082", None, Some(PrimSeverity::Warn)),
    rule("MD067", None, Some(PrimSeverity::Warn)),
];

fn effective_severity(rule: &str, strict: bool) -> Option<PrimSeverity> {
    ACTIVE_RULES
        .iter()
        .find(|policy| policy.rule == rule)
        .and_then(|policy| if strict { policy.strict } else { policy.floor })
}

/// Lint `source` as Markdown content, returning prim's own diagnostics.
///
/// `strict = false` runs the always-on floor tier; `strict = true` adds the
/// strict tier and escalates warning-tier floor rules to errors. A file-level
/// `<!-- prim-mdlint-strict: true|false -->` directive (story G5, #61)
/// overrides `strict` for this file only — a surgical, per-file escape hatch
/// on top of the `.editorconfig`-resolved default, matching the same
/// precedence rumdl's own `rumdl-disable`/`markdownlint-disable` inline
/// directives already get (rumdl applies those inside `rumdl_lib::lint`
/// itself, independent of prim's `strict` matrix). Lint-only: `source` is
/// never modified. Rules are filtered from the full rumdl set by name so
/// off/formatter-territory rules never run.
pub fn lint(source: &str, strict: bool) -> Vec<MdDiagnostic> {
    let strict = file_level_strict_override(source).unwrap_or(strict);
    let cfg = Config::default();
    let rules: Vec<_> = all_rules(&cfg)
        .into_iter()
        .filter(|rule| effective_severity(rule.name(), strict).is_some())
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
            let severity = effective_severity(&rule, strict)?;
            Some(MdDiagnostic {
                rule,
                line: warning.line,
                column: warning.column,
                is_error: severity == PrimSeverity::Error,
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
mod tests {
    use super::*;

    #[test]
    fn severity_matrix_matches_issue_59() {
        let warn = Some(PrimSeverity::Warn);
        let error = Some(PrimSeverity::Error);

        for rule in ["MD045", "MD024", "MD051", "MD075", "MD080"] {
            assert_eq!(effective_severity(rule, false), warn, "{rule} floor");
            assert_eq!(effective_severity(rule, true), error, "{rule} strict");
        }

        for rule in [
            "MD042", "MD011", "MD052", "MD056", "MD062", "MD034", "MD057",
        ] {
            assert_eq!(effective_severity(rule, false), error, "{rule} floor");
            assert_eq!(effective_severity(rule, true), error, "{rule} strict");
        }

        for rule in ["MD066", "MD068", "MD070"] {
            assert_eq!(effective_severity(rule, false), None, "{rule} floor");
            assert_eq!(effective_severity(rule, true), error, "{rule} strict");
        }

        for rule in [
            "MD025", "MD041", "MD001", "MD040", "MD033", "MD026", "MD036", "MD059", "MD053",
            "MD073", "MD082", "MD067",
        ] {
            assert_eq!(effective_severity(rule, false), None, "{rule} floor");
            assert_eq!(effective_severity(rule, true), warn, "{rule} strict");
        }

        for rule in [
            "MD003", "MD004", "MD005", "MD007", "MD009", "MD010", "MD012", "MD018", "MD019",
            "MD020", "MD021", "MD022", "MD023", "MD027", "MD028", "MD029", "MD030", "MD031",
            "MD032", "MD035", "MD037", "MD038", "MD039", "MD046", "MD047", "MD048", "MD049",
            "MD050", "MD055", "MD058", "MD060", "MD064", "MD065", "MD071", "MD076", "MD077",
            "MD013", "MD014", "MD043", "MD044", "MD054", "MD061", "MD063", "MD069", "MD072",
            "MD074", "MD078", "MD079", "MD081",
        ] {
            assert_eq!(effective_severity(rule, false), None, "{rule} floor");
            assert_eq!(effective_severity(rule, true), None, "{rule} strict");
        }
    }

    #[test]
    fn floor_and_strict_tiers_use_prim_owned_severities() {
        let floor = lint("![](image.png)\n", false);
        let strict = lint("![](image.png)\n", true);
        let floor_image = floor.iter().find(|d| d.rule == "MD045").unwrap();
        let strict_image = strict.iter().find(|d| d.rule == "MD045").unwrap();
        assert!(!floor_image.is_error, "floor warning: {floor:?}");
        assert!(strict_image.is_error, "strict error: {strict:?}");

        let structure_floor = lint("Intro\n\n# Title\n", false);
        let structure_strict = lint("Intro\n\n# Title\n", true);
        assert!(
            structure_floor.iter().all(|d| d.rule != "MD041"),
            "strict-only rule stays off by default: {structure_floor:?}"
        );
        let first_line_heading = structure_strict
            .iter()
            .find(|d| d.rule == "MD041")
            .expect("MD041 enabled in strict");
        assert!(
            !first_line_heading.is_error,
            "strict warning: {structure_strict:?}"
        );

        let strict_only_defect = lint("Text with orphan[^missing].\n", true);
        let footnote = strict_only_defect
            .iter()
            .find(|d| d.rule == "MD066")
            .expect("MD066 enabled in strict");
        assert!(footnote.is_error, "strict error: {strict_only_defect:?}");
    }

    #[test]
    fn never_linted_and_off_rules_stay_excluded() {
        let src = "\
| a | bb |
| c | d |

This is an intentionally long line that would violate line-length linting if prim enabled MD013 for Markdown content checks.\n";
        assert!(
            lint(src, false)
                .iter()
                .all(|d| d.rule != "MD060" && d.rule != "MD013"),
            "formatter-territory and off rules stay disabled: {:?}",
            lint(src, false)
        );
    }

    #[test]
    fn verifies_selected_rumdl_extension_rules_on_real_markdown() {
        let cases = [
            ("MD062", "[link]( https://example.com )\n", true),
            ("MD066", "Text with orphan[^missing].\n", true),
            ("MD068", "Text with [^1].\n\n[^1]:\n", true),
            (
                "MD070",
                "```markdown\n```rust\nfn main() {}\n```\n```\n",
                true,
            ),
            (
                "MD075",
                "Some text.\n\n| value1 | description1 |\n| value2 | description2 |\n",
                true,
            ),
            ("MD080", "# Setup & Run\n\n# Setup  Run\n", true),
            (
                "MD082",
                "# Level 1 heading\n\nLevel 1 content\n\n## Empty Section\n### Level 3 heading\n",
                false,
            ),
        ];

        for (rule, src, is_error) in cases {
            let diags = lint(src, true);
            let diag = diags
                .iter()
                .find(|d| d.rule == rule)
                .unwrap_or_else(|| panic!("{rule} did not fire: {diags:?}"));
            assert_eq!(diag.is_error, is_error, "{rule} severity: {diags:?}");
            assert!(diag.line >= 1, "{rule} keeps 1-indexed lines: {diags:?}");
            assert!(
                diag.column >= 1,
                "{rule} keeps 1-indexed columns: {diags:?}"
            );
        }
    }

    #[test]
    fn reports_a_bare_url_with_real_line_col() {
        let src = "See https://example.com for details.\n";
        let diags = lint(src, false);
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
        assert!(lint(src, false).is_empty(), "{:?}", lint(src, false));
    }

    #[test]
    fn lint_never_mutates_source() {
        let src = "See https://example.com\n";
        let before = src.to_string();
        let _ = lint(src, false);
        assert_eq!(src, before, "lint is read-only");
    }

    #[test]
    fn file_level_directive_false_overrides_editorconfig_strict_true() {
        // MD041 is strict-only; the directive drops the file back to floor
        // even though the caller (an .editorconfig `prim_mdlint_strict =
        // true` glob) asked for strict.
        let src = "<!-- prim-mdlint-strict: false -->\nIntro\n\n# Title\n";
        let diags = lint(src, true);
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
        let diags = lint(src, false);
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
        let diags = lint(src, false);
        assert!(
            diags.iter().all(|d| d.rule != "MD041"),
            "the later directive wins: {diags:?}"
        );
    }

    #[test]
    fn directive_boolean_is_case_insensitive() {
        let src = "<!-- prim-mdlint-strict: TRUE -->\nIntro\n\n# Title\n";
        let diags = lint(src, false);
        assert!(
            diags.iter().any(|d| d.rule == "MD041"),
            "TRUE is accepted: {diags:?}"
        );
    }

    #[test]
    fn malformed_directive_value_is_ignored() {
        let src = "<!-- prim-mdlint-strict: yes -->\nIntro\n\n# Title\n";
        let diags = lint(src, true);
        assert!(
            diags.iter().any(|d| d.rule == "MD041"),
            "a bad value falls back to the caller's strict setting: {diags:?}"
        );
    }

    #[test]
    fn a_look_alike_comment_that_is_not_the_sole_line_content_is_ignored() {
        let src = "Some text <!-- prim-mdlint-strict: false --> more text\nIntro\n\n# Title\n";
        let diags = lint(src, true);
        assert!(
            diags.iter().any(|d| d.rule == "MD041"),
            "an inline (non-standalone) comment is not a directive: {diags:?}"
        );
    }
}
