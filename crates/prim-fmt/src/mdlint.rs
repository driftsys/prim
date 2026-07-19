//! Markdown content linting (SPIKE #39) via the `rumdl` crate — **lint-only**.
//!
//! prim owns Markdown *formatting* through `dprint-plugin-markdown` (see
//! [`crate::markdown`]). This module adds a *content* linter on top: it reports
//! issues a formatter cannot (bare URLs, empty links, missing image alt text)
//! and **never rewrites** — prim never invokes rumdl's formatter, LSP, or file
//! walker, only [`rumdl_lib::lint`].
//!
//! This is a de-risking skeleton, not the finished feature. The curated rule set
//! here is a placeholder; the real severity matrix and strict-tier toggle are
//! story G3 (#59). What this proves for the epic:
//!
//! - `rumdl = "=0.2.35"` links with `default-features = false` (no
//!   tokio/tower-lsp/notify/rayon), so the engine stays pure and small.
//! - rules are selected by [`rumdl_lib::rule::Rule::name`] from the full
//!   `all_rules(&cfg)` set, exactly as G2 will do.
//! - `rumdl_lib::lint` returns 1-indexed `line`/`column` diagnostics — the
//!   line:col that stories B1/D2 want (and which serde-based formats lack, per
//!   spike #42).

use rumdl_lib::config::{Config, MarkdownFlavor};
use rumdl_lib::rule::Severity;
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

/// The curated rule subset for the spike. The full off/warn/error matrix is
/// story G3; this placeholder set is enough to prove `.name()` filtering and
/// that lint-only diagnostics carry a real `line:col`.
const CURATED: &[&str] = &[
    "MD034", // no bare URLs
    "MD042", // no empty links
    "MD045", // images need alt text
];

/// Lint `source` as Markdown content, returning prim's own diagnostics.
///
/// Lint-only: `source` is never modified. Rules are filtered from the full set
/// by name so unlisted rules never run.
pub fn lint(source: &str) -> Vec<MdDiagnostic> {
    let cfg = Config::default();
    let rules: Vec<_> = all_rules(&cfg)
        .into_iter()
        .filter(|rule| CURATED.contains(&rule.name()))
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
        .map(|w| MdDiagnostic {
            rule: w.rule_name.unwrap_or_default(),
            line: w.line,
            column: w.column,
            is_error: matches!(w.severity, Severity::Error),
            message: w.message,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_a_bare_url_with_real_line_col() {
        let src = "See https://example.com for details.\n";
        let diags = lint(src);
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
        assert!(lint(src).is_empty(), "{:?}", lint(src));
    }

    #[test]
    fn only_curated_rules_run() {
        // MD009 (trailing spaces) is a real rumdl rule but not curated, so a
        // trailing-space line must produce no diagnostic from this module.
        let src = "trailing spaces here   \n";
        assert!(
            lint(src).iter().all(|d| CURATED.contains(&d.rule.as_str())),
            "no rule outside the curated set: {:?}",
            lint(src)
        );
    }

    #[test]
    fn lint_never_mutates_source() {
        let src = "See https://example.com\n";
        let before = src.to_string();
        let _ = lint(src);
        assert_eq!(src, before, "lint is read-only");
    }
}
