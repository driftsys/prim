//! Minimal CLI output helpers.
//!
//! Human-readable messages go to stderr; machine-readable output (the list
//! of files that would change under `--check`) goes to stdout.

use std::path::Path;

use yansi::Paint;

/// Print a prefixed error message to stderr.
pub fn error(msg: &str) {
    eprintln!("{} {msg}", "error:".red().bold());
}

/// Report, on stdout, that `path` would be reformatted (`--check`).
pub fn would_reformat(path: &Path) {
    println!("{}", path.display());
}
