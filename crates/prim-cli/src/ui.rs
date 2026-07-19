//! Minimal CLI output helpers.
//!
//! Human-readable messages go to stderr; machine-readable output (the list
//! of files that would change under `--check`) goes to stdout.

use std::path::Path;

use yansi::Paint;

use crate::cli::ColorWhen;

/// Print a prefixed error message to stderr.
pub fn error(msg: &str) {
    eprintln!("{} {msg}", "error:".red().bold());
}

/// Print a prefixed warning message to stderr.
pub fn warning(msg: &str) {
    eprintln!("{} {msg}", "warning:".yellow().bold());
}

/// Report, on stdout, that `path` would be reformatted (`--check`).
pub fn would_reformat(path: &Path) {
    println!("{}", path.display());
}

/// Report, on stdout, a lint finding for `path` (`prim lint` — report-only,
/// never rewrites). Coarse shape kept for structured formats
/// (JSON/JSONC/TOML/YAML/Markdown) until finer-grained content diagnostics
/// land (G2/D2); orphan files get itemized codes via [`lint_diagnostic`]
/// (story B1).
pub fn lint_finding(path: &Path, message: &str) {
    println!("{}: {message}", path.display());
}

/// Report, on stdout, one positioned, coded lint finding for `path` (story
/// B1). `--format json`/`--format sarif` machine-readable output is D2's
/// scope, not this one.
pub fn lint_diagnostic(path: &Path, diagnostic: &prim_fmt::Diagnostic) {
    println!(
        "{}:{}:{}: {} [{}]",
        path.display(),
        diagnostic.line,
        diagnostic.column,
        diagnostic.message,
        diagnostic.code
    );
}

/// Decide whether coloured output is enabled: an explicit `--color always` /
/// `--color never` wins; `auto` colours only when stderr (the human-output
/// stream) is a terminal and `NO_COLOR` is unset (clig.dev).
pub fn resolve_color(when: ColorWhen, stderr_is_tty: bool, no_color: bool) -> bool {
    match when {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => stderr_is_tty && !no_color,
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::ColorWhen;

    use super::resolve_color;

    #[test]
    fn always_and_never_ignore_the_environment() {
        assert!(resolve_color(ColorWhen::Always, false, true));
        assert!(!resolve_color(ColorWhen::Never, true, false));
    }

    #[test]
    fn auto_needs_a_tty_and_no_color_unset() {
        assert!(resolve_color(ColorWhen::Auto, true, false));
        assert!(!resolve_color(ColorWhen::Auto, false, false));
        assert!(!resolve_color(ColorWhen::Auto, true, true));
    }
}
