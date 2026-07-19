//! Operating-mode dispatch over the prim formatting engine (AD-0007): `fmt`
//! and `fix` write, `lint` only ever reports.

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::cli::{Cli, FixArgs, FmtArgs, LintArgs, OutputFormat, Verb, WriteArgs};
use crate::diff;
use crate::discover;
use crate::editorconfig;
use crate::report::{self, Finding, ReportMode};
use crate::ui;
use crate::write;
use prim_fmt::{FileKind, Style};

/// Exit codes (AD-0007 §4): `0` nothing to do / already clean, `1`
/// actionable — format drift (`fmt`/`fix` `--check`) or a lint finding, `2`
/// prim could not do its job (parse/IO/usage error). Warnings never raise the
/// exit code; only errors do.
const EXIT_OK: i32 = 0;
const EXIT_ACTIONABLE: i32 = 1;
const EXIT_ERROR: i32 = 2;

/// A generic lint finding for the structured formats that still only have
/// format-drift reporting (JSON/JSONC/TOML/YAML). Markdown has itemized rumdl
/// content diagnostics instead; orphan files have itemized whitespace-hygiene
/// diagnostics (story B1). The `_CODE`/`_FINDING` split feeds both the
/// plain-text (`ui::lint_finding`) and machine-readable (`Finding::new`,
/// story D2) report paths.
const FORMAT_DRIFT_CODE: &str = "format::drift";
const FORMAT_CHECK_FINDING: &str = "would be reformatted";
const FORMAT_DRIFT_FINDING: &str = "does not match prim's canonical format (run `prim fmt` to fix)";

/// A file that formatted successfully: its path, kind, resolved style,
/// original text, and formatted text.
type FormattedFile = (PathBuf, FileKind, Style, String, String);

/// Process the parsed CLI and return the process exit code.
pub fn run(cli: &Cli) -> i32 {
    match &cli.verb {
        // `fix` is `fmt` plus autofixable content rules; those rules don't
        // exist yet, so `fix` is byte-for-byte `fmt` for now.
        // Exit codes still differ (AD-0007 §4): unlike `fmt --diff` (always
        // `0`, preview-only), `fix --check`/`--diff` share one gated
        // contract, so `run_fix` still dispatches through the shared
        // `run_fmt_paths(..., is_fix = true)` helper.
        Verb::Fmt(args) => run_fmt(args, &cli.exclude),
        Verb::Fix(args) => run_fix(args, &cli.exclude),
        Verb::Lint(args) => run_lint(args, &cli.exclude),
    }
}

fn run_fmt(args: &FmtArgs, excludes: &[String]) -> i32 {
    if let Some(path) = args.write.stdin_filepath.as_deref() {
        return run_fmt_stdin(path);
    }
    run_fmt_paths(&args.write, args.format, excludes, false)
}

fn run_fix(args: &FixArgs, excludes: &[String]) -> i32 {
    if let Some(path) = args.write.stdin_filepath.as_deref() {
        return run_fmt_stdin(path);
    }
    run_fmt_paths(&args.write, None, excludes, true)
}

fn run_lint(args: &LintArgs, excludes: &[String]) -> i32 {
    if let Some(path) = args.stdin_filepath.as_deref() {
        return run_lint_stdin(path, args.format);
    }
    run_lint_paths(args, excludes)
}

/// Read stdin, format it, and write the result to stdout (format-on-save).
///
/// The path selects the formatter; if prim does not own that file type, the
/// input is passed through unchanged.
fn run_fmt_stdin(path: &Path) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    match prim_fmt::classify(path) {
        Some(kind) => {
            let style = editorconfig::resolve(path);
            match prim_fmt::format(kind, &input, &style) {
                Ok(text) => print!("{text}"),
                Err(err) => {
                    // Preserve the editor buffer on a parse failure: echo the
                    // original to stdout and report on stderr (FR-6.3).
                    ui::error(&format!("{}: {err}", path.display()));
                    print!("{input}");
                    return EXIT_ERROR;
                }
            }
        }
        None => print!("{input}"),
    }
    EXIT_OK
}

/// Read stdin and report whether it would violate the canonical format;
/// writes nothing, ever (`lint` is report-only).
fn run_lint_stdin(path: &Path, format: Option<OutputFormat>) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    match prim_fmt::classify(path) {
        Some(FileKind::Orphan) => {
            // Story B1: itemized, coded, positioned findings.
            let style = editorconfig::resolve(path);
            let diagnostics = prim_fmt::hygiene_diagnostics(&input, &style);
            if let Some(format) = format {
                let findings = diagnostics
                    .iter()
                    .map(|diagnostic| Finding::diagnostic(path, diagnostic))
                    .collect::<Vec<_>>();
                emit_report(format, ReportMode::Lint, &findings);
                if diagnostics.is_empty() {
                    EXIT_OK
                } else {
                    EXIT_ACTIONABLE
                }
            } else if diagnostics.is_empty() {
                EXIT_OK
            } else {
                for diagnostic in &diagnostics {
                    ui::lint_diagnostic(path, diagnostic);
                }
                EXIT_ACTIONABLE
            }
        }
        Some(FileKind::Markdown) => {
            let diagnostics = prim_fmt::lint_markdown(&input);
            if let Some(format) = format {
                let findings = diagnostics
                    .iter()
                    .map(|diagnostic| Finding::markdown(path, diagnostic))
                    .collect::<Vec<_>>();
                emit_report(format, ReportMode::Lint, &findings);
                if diagnostics.is_empty() {
                    EXIT_OK
                } else {
                    EXIT_ACTIONABLE
                }
            } else if diagnostics.is_empty() {
                EXIT_OK
            } else {
                for diagnostic in &diagnostics {
                    ui::lint_markdown_diagnostic(path, diagnostic);
                }
                EXIT_ACTIONABLE
            }
        }
        Some(kind) => {
            let style = editorconfig::resolve(path);
            match prim_fmt::format(kind, &input, &style) {
                Ok(text) if text == input => {
                    if let Some(format) = format {
                        emit_report(format, ReportMode::Lint, &[]);
                    }
                    EXIT_OK
                }
                Ok(_) => {
                    if let Some(format) = format {
                        let findings =
                            vec![Finding::new(path, FORMAT_DRIFT_CODE, FORMAT_DRIFT_FINDING)];
                        emit_report(format, ReportMode::Lint, &findings);
                    } else {
                        ui::lint_finding(path, FORMAT_DRIFT_FINDING);
                    }
                    EXIT_ACTIONABLE
                }
                Err(err) => {
                    ui::error(&format!("{}: {err}", path.display()));
                    if let Some(format) = format {
                        emit_report(format, ReportMode::Lint, &[]);
                    }
                    EXIT_ERROR
                }
            }
        }
        None => {
            if let Some(format) = format {
                emit_report(format, ReportMode::Lint, &[]);
            }
            EXIT_OK
        }
    }
}

/// Discover the target files and format each with its resolved style.
///
/// Files prim does not own are left byte-for-byte unchanged (FR-2.4): walked
/// files are skipped silently, an explicitly named path is answered — a
/// missing one is an error, an unowned one a warning. An owned file that
/// fails to read (non-UTF-8) or parse is likewise skipped and reported.
/// Returns the (path, original, formatted) triples for every file that
/// formatted successfully, plus whether a hard error occurred.
fn load_and_format(
    paths: &[PathBuf],
    excludes: &[String],
) -> Result<(Vec<FormattedFile>, bool), ignore::Error> {
    let mut had_error = false;
    let mut results = Vec::new();
    // Caches each directory's `.editorconfig` cascade so a repository parses
    // every config once, not once per file.
    let mut resolver = editorconfig::Resolver::new();

    let files = discover::collect(paths, excludes)?;

    for file in files {
        let Some(kind) = prim_fmt::classify(&file.path) else {
            if file.explicit {
                if file.path.exists() {
                    ui::warning(&format!(
                        "{}: not a file type prim formats; skipped",
                        file.path.display()
                    ));
                } else {
                    ui::error(&format!("{}: no such file", file.path.display()));
                    had_error = true;
                }
            }
            continue;
        };

        let original = match std::fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(err) => {
                let message = format!("{}: {err}", file.path.display());
                if file.explicit {
                    ui::error(&message);
                    had_error = true;
                } else {
                    ui::warning(&message);
                }
                continue;
            }
        };

        let style = resolver.resolve(&file.path);
        let formatted = match prim_fmt::format(kind, &original, &style) {
            Ok(text) => text,
            Err(err) => {
                let message = format!("{}: {err}", file.path.display());
                if file.explicit {
                    ui::error(&message);
                    had_error = true;
                } else {
                    ui::warning(&message);
                }
                continue;
            }
        };

        results.push((file.path, kind, style, original, formatted));
    }

    Ok((results, had_error))
}

fn run_fmt_paths(
    args: &WriteArgs,
    format: Option<OutputFormat>,
    excludes: &[String],
    is_fix: bool,
) -> i32 {
    let (results, mut had_error) = match load_and_format(&args.paths, excludes) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&format!("--exclude: {err}"));
            return EXIT_ERROR;
        }
    };

    let mut any_would_change = false;
    let mut findings = Vec::new();
    for (path, _kind, _style, original, formatted) in results {
        if formatted == original {
            continue;
        }
        any_would_change = true;

        if args.check {
            if format.is_some() {
                findings.push(Finding::new(&path, FORMAT_DRIFT_CODE, FORMAT_CHECK_FINDING));
            } else {
                ui::would_reformat(&path);
            }
        } else if args.diff {
            // Print a unified diff of the pending change; write nothing (FR-5.3).
            print!("{}", diff::unified(&path, &original, &formatted));
        } else if let Err(err) = write::atomic(&path, &formatted) {
            // Atomic write (FR-6.4): on failure the original is left intact.
            ui::error(&format!("{}: {err}", path.display()));
            had_error = true;
        }
    }

    if let Some(format) = format
        && args.check
    {
        emit_report(format, ReportMode::FmtCheck, &findings);
    }

    // AD-0007 §4: `fmt --diff` is always a `0`-exit preview, but `fix
    // --check`/`--diff` share one gated contract — both report whether a
    // fixable finding is pending.
    let gates_on_pending_findings = args.check || (is_fix && args.diff);

    if had_error {
        EXIT_ERROR
    } else if gates_on_pending_findings && any_would_change {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

fn run_lint_paths(args: &LintArgs, excludes: &[String]) -> i32 {
    let (results, had_error) = match load_and_format(&args.paths, excludes) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&format!("--exclude: {err}"));
            return EXIT_ERROR;
        }
    };

    let mut any_finding = false;
    let mut findings = Vec::new();
    for (path, kind, style, original, formatted) in results {
        if kind == FileKind::Orphan {
            // Story B1: itemized, coded, positioned findings for the
            // un-owned-text allowlist — the same set A1's BOM strip covers.
            let diagnostics = prim_fmt::hygiene_diagnostics(&original, &style);
            if !diagnostics.is_empty() {
                any_finding = true;
                for diagnostic in &diagnostics {
                    if args.format.is_some() {
                        findings.push(Finding::diagnostic(&path, diagnostic));
                    } else {
                        ui::lint_diagnostic(&path, diagnostic);
                    }
                }
            }
        } else if kind == FileKind::Markdown {
            let diagnostics = prim_fmt::lint_markdown(&original);
            if !diagnostics.is_empty() {
                any_finding = true;
                for diagnostic in &diagnostics {
                    if args.format.is_some() {
                        findings.push(Finding::markdown(&path, diagnostic));
                    } else {
                        ui::lint_markdown_diagnostic(&path, diagnostic);
                    }
                }
            }
        } else if formatted != original {
            // JSON/JSONC/TOML/YAML keep the coarser format-drift finding until
            // their own content diagnostics land (future story).
            any_finding = true;
            if args.format.is_some() {
                findings.push(Finding::new(&path, FORMAT_DRIFT_CODE, FORMAT_DRIFT_FINDING));
            } else {
                ui::lint_finding(&path, FORMAT_DRIFT_FINDING);
            }
        }
    }

    if let Some(format) = args.format {
        emit_report(format, ReportMode::Lint, &findings);
    }

    if had_error {
        EXIT_ERROR
    } else if any_finding {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

fn emit_report(format: OutputFormat, mode: ReportMode, findings: &[Finding]) {
    print!("{}", report::render(format, mode, findings));
}
