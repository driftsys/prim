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
/// never rewrites). One line per finding; today's shape is intentionally
/// coarse (no stable code or line:col yet — story B1 adds those, D2 adds
/// `--format json`/`--format sarif`).
pub fn lint_finding(path: &Path, message: &str) {
    println!("{}: {message}", path.display());
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
