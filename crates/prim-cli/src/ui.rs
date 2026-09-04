//! Minimal CLI output helpers.
//!
//! Human-readable messages go to stderr; the default plain-text report modes
//! also write their findings to stdout.

use std::io::{self, Write};
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

/// Print a human-readable status message to stderr.
pub fn status(msg: &str) {
    eprintln!("{msg}");
}

/// Report, on stdout, that `path` would be reformatted (`--check`).
pub fn would_reformat(path: &Path) {
    to_stdout(|out| write_would_reformat(out, path));
}

fn write_would_reformat(out: &mut impl Write, path: &Path) -> io::Result<()> {
    write_path(out, path)?;
    out.write_all(b"\n")
}

/// Report, on stdout, a lint finding for `path` (`prim lint` — report-only,
/// never rewrites). Coarse shape kept for JSON/JSONC/TOML/YAML; orphan files
/// get itemized codes via [`lint_diagnostic`] (story B1), and Markdown has its
/// own positioned rumdl diagnostics via [`lint_markdown_diagnostic`] (story
/// G2).
pub fn lint_finding(path: &Path, message: &str) {
    to_stdout(|out| write_lint_finding(out, path, message));
}

fn write_lint_finding(out: &mut impl Write, path: &Path, message: &str) -> io::Result<()> {
    write_path(out, path)?;
    writeln!(out, ": {message}")
}

/// Report, on stdout, one positioned, coded lint finding for `path` (story
/// B1).
pub fn lint_diagnostic(path: &Path, diagnostic: &prim_fmt::Diagnostic) {
    lint_positioned(
        path,
        diagnostic.line,
        diagnostic.column,
        &diagnostic.message,
        diagnostic.code,
    );
}

/// Report, on stdout, one positioned Markdown content-lint finding for
/// `path` (story G2), using rumdl's own rule code verbatim (for example
/// `MD034`).
pub fn lint_markdown_diagnostic(path: &Path, diagnostic: &prim_fmt::MdDiagnostic) {
    lint_positioned(
        path,
        diagnostic.line,
        diagnostic.column,
        &diagnostic.message,
        &diagnostic.rule,
    );
}

fn lint_positioned(path: &Path, line: usize, column: usize, message: &str, code: &str) {
    to_stdout(|out| write_lint_positioned(out, path, line, column, message, code));
}

fn write_lint_positioned(
    out: &mut impl Write,
    path: &Path,
    line: usize,
    column: usize,
    message: &str,
    code: &str,
) -> io::Result<()> {
    write_path(out, path)?;
    writeln!(out, ":{line}:{column}: {message} [{code}]")
}

/// Write one report line to the locked stdout, panicking as `println!` does
/// when the stream cannot be written.
fn to_stdout(write: impl FnOnce(&mut io::StdoutLock<'_>) -> io::Result<()>) {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    write(&mut out).unwrap_or_else(|err| panic!("failed printing to stdout: {err}"));
}

/// Write `path` to the machine-readable stream as the bytes the filesystem
/// holds.
///
/// On unix a path is an arbitrary byte string. Rendering one through
/// `Path::display` replaces each byte that is not valid UTF-8 with U+FFFD, so
/// prim printed `caf\u{fffd}.md` — the bytes `EF BF BD` where the name holds
/// `E9` — and the documented `prim fmt --check --since ... | xargs` hook
/// received a path that does not open (#172). The bytes are the name, so they
/// are what this stream carries.
///
/// Human-facing output stays lossy: stderr is prose for a reader, and a
/// terminal cannot show an undecodable byte anyway.
#[cfg(unix)]
fn write_path(out: &mut impl Write, path: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    out.write_all(path.as_os_str().as_bytes())
}

/// Where a filename is Unicode there are no bytes to write. Such a name is not
/// always representable either — a Windows path may hold an unpaired surrogate,
/// which `Path::display` renders as U+FFFD — but there is no byte string to
/// offer in its place, so the lossy rendering stands. `report::path_bytes`
/// draws the same line for the same reason.
#[cfg(not(unix))]
fn write_path(out: &mut impl Write, path: &Path) -> io::Result<()> {
    write!(out, "{}", path.display())
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

    use super::*;

    /// A name holding a byte that is not valid UTF-8. It cannot be created on
    /// APFS or HFS+, so the end-to-end coverage is Linux-only; this reaches
    /// the rendering without touching a filesystem.
    #[cfg(unix)]
    fn undecodable() -> std::path::PathBuf {
        use std::os::unix::ffi::OsStrExt;

        std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"caf\xe9.md"))
    }

    #[test]
    #[cfg(unix)]
    fn the_check_list_writes_the_bytes_the_filesystem_holds() {
        let mut out = Vec::new();
        write_would_reformat(&mut out, &undecodable()).unwrap();

        assert_eq!(out, b"caf\xe9.md\n");
    }

    #[test]
    #[cfg(unix)]
    fn a_lint_finding_writes_the_bytes_the_filesystem_holds() {
        let mut out = Vec::new();
        write_lint_finding(&mut out, &undecodable(), "would be reformatted").unwrap();

        assert_eq!(out, b"caf\xe9.md: would be reformatted\n");
    }

    #[test]
    #[cfg(unix)]
    fn a_positioned_finding_writes_the_bytes_the_filesystem_holds() {
        let mut out = Vec::new();
        write_lint_positioned(&mut out, &undecodable(), 3, 1, "bad anchor", "MD051").unwrap();

        assert_eq!(out, b"caf\xe9.md:3:1: bad anchor [MD051]\n");
    }

    #[test]
    fn a_decodable_name_renders_exactly_as_before() {
        let mut out = Vec::new();
        write_would_reformat(&mut out, Path::new("docs/guide.md")).unwrap();
        write_lint_finding(&mut out, Path::new("a.json"), "would be reformatted").unwrap();
        write_lint_positioned(&mut out, Path::new("b.md"), 2, 5, "msg", "MD034").unwrap();

        assert_eq!(
            String::from_utf8(out).unwrap(),
            "docs/guide.md\na.json: would be reformatted\nb.md:2:5: msg [MD034]\n"
        );
    }

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
