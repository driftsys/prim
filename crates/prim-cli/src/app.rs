//! Operating-mode dispatch over prim's formatting verbs (`fmt`/`lint`/`fix`,
//! AD-0007) plus the one-shot `init` scaffolder.

use std::io::Read;
use std::path::Path;

mod load;

use self::load::load_and_format;
use crate::changed_files::ChangedFilesScope;
use crate::cli::{
    Cli, ExplainArgs, FixArgs, FmtArgs, InitArgs, LintArgs, OutputFormat, Verb, WriteArgs,
};
use crate::diff;
use crate::editorconfig;
use crate::explain;
use crate::init;
use crate::lsp;
use crate::provenance;
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

/// Process the parsed CLI and return the process exit code.
pub fn run(cli: &Cli) -> i32 {
    let changed_files_scope = changed_files_scope(cli);
    match &cli.verb {
        // `fix` is `fmt` plus autofixable content rules; those rules don't
        // exist yet, so `fix` is byte-for-byte `fmt` for now.
        // Exit codes still differ (AD-0007 §4): unlike `fmt --diff` (always
        // `0`, preview-only), `fix --check`/`--diff` share one gated
        // contract, so `run_fix` still dispatches through the shared
        // `run_fmt_paths(..., is_fix = true)` helper.
        Verb::Fmt(args) => run_fmt(args, &cli.exclude, !cli.no_ignore, &changed_files_scope),
        Verb::Fix(args) => run_fix(args, &cli.exclude, !cli.no_ignore, &changed_files_scope),
        Verb::Lint(args) => run_lint(args, &cli.exclude, !cli.no_ignore, &changed_files_scope),
        Verb::Init(args) => run_init(args),
        Verb::Explain(args) => run_explain(args),
        Verb::Lsp => lsp::run(),
    }
}

fn changed_files_scope(cli: &Cli) -> ChangedFilesScope {
    if cli.staged {
        ChangedFilesScope::Staged
    } else if let Some(reference) = &cli.since {
        ChangedFilesScope::Since(reference.clone())
    } else {
        ChangedFilesScope::All
    }
}

fn run_fmt(
    args: &FmtArgs,
    excludes: &[String],
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    if let Some(path) = args.write.stdin_filepath.as_deref() {
        return run_fmt_stdin(path);
    }
    if args.check_idempotence {
        return run_check_idempotence_paths(
            &args.write,
            excludes,
            respect_vcs_ignore,
            changed_files_scope,
        );
    }
    run_fmt_paths(
        &args.write,
        args.format,
        excludes,
        false,
        respect_vcs_ignore,
        changed_files_scope,
    )
}

fn run_fix(
    args: &FixArgs,
    excludes: &[String],
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    if let Some(path) = args.write.stdin_filepath.as_deref() {
        return run_fmt_stdin(path);
    }
    run_fmt_paths(
        &args.write,
        None,
        excludes,
        true,
        respect_vcs_ignore,
        changed_files_scope,
    )
}

fn run_lint(
    args: &LintArgs,
    excludes: &[String],
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    if let Some(path) = args.stdin_filepath.as_deref() {
        return run_lint_stdin(path, args.format);
    }
    run_lint_paths(args, excludes, respect_vcs_ignore, changed_files_scope)
}

fn run_init(args: &InitArgs) -> i32 {
    let target = args.path.as_deref().unwrap_or_else(|| Path::new("."));
    match init::run(target) {
        Ok(outcome) => {
            ui::status(&outcome.message);
            EXIT_OK
        }
        Err(err) => {
            ui::error(&err.to_string());
            EXIT_ERROR
        }
    }
}

/// Print the `.editorconfig` settings that apply to `args.path`, and where
/// each came from (story C2). `explain` never reads `args.path` itself —
/// classification is name/extension-based, so it works for files that don't
/// exist yet.
fn run_explain(args: &ExplainArgs) -> i32 {
    let path = &args.path;
    match prim_fmt::classify(path) {
        Some(kind) => {
            let settings = provenance::explain(path, kind);
            print!("{}", explain::render(path, &settings));
            EXIT_OK
        }
        None => {
            ui::warning(&format!(
                "{}: not a file type prim formats; skipped",
                path.display()
            ));
            EXIT_OK
        }
    }
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
            let diagnostics =
                prim_fmt::lint_markdown(&input, editorconfig::resolve_mdlint_strict(path));
            let has_error = diagnostics.iter().any(|diagnostic| diagnostic.is_error);
            if let Some(format) = format {
                let findings = diagnostics
                    .iter()
                    .map(|diagnostic| Finding::markdown(path, diagnostic))
                    .collect::<Vec<_>>();
                emit_report(format, ReportMode::Lint, &findings);
                if has_error { EXIT_ACTIONABLE } else { EXIT_OK }
            } else if diagnostics.is_empty() {
                EXIT_OK
            } else {
                for diagnostic in &diagnostics {
                    ui::lint_markdown_diagnostic(path, diagnostic);
                }
                if has_error { EXIT_ACTIONABLE } else { EXIT_OK }
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

fn run_fmt_paths(
    args: &WriteArgs,
    format: Option<OutputFormat>,
    excludes: &[String],
    is_fix: bool,
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let (results, mut had_error) = match load_and_format(
        &args.paths,
        excludes,
        respect_vcs_ignore,
        changed_files_scope,
    ) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_would_change = false;
    let mut findings = Vec::new();
    for (path, _kind, _style, _markdown_strict, original, formatted) in results {
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

fn run_check_idempotence_paths(
    args: &WriteArgs,
    excludes: &[String],
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let (results, mut had_error) = match load_and_format(
        &args.paths,
        excludes,
        respect_vcs_ignore,
        changed_files_scope,
    ) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_non_idempotent = false;
    for (path, kind, style, _markdown_strict, _original, formatted) in results {
        let stable = match is_idempotent_second_pass(kind, &formatted, &style) {
            Ok(stable) => stable,
            Err(err) => {
                ui::error(&format!(
                    "{}: second formatting pass failed: {err}",
                    path.display()
                ));
                had_error = true;
                continue;
            }
        };

        if !stable {
            any_non_idempotent = true;
            ui::would_reformat(&path);
        }
    }

    if had_error {
        EXIT_ERROR
    } else if any_non_idempotent {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

fn is_idempotent_second_pass(
    kind: FileKind,
    formatted: &str,
    style: &Style,
) -> Result<bool, prim_fmt::FormatError> {
    let reformatted = prim_fmt::format(kind, formatted, style)?;
    Ok(second_pass_matches_first(formatted, &reformatted))
}

fn second_pass_matches_first(formatted: &str, reformatted: &str) -> bool {
    formatted == reformatted
}

fn run_lint_paths(
    args: &LintArgs,
    excludes: &[String],
    respect_vcs_ignore: bool,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let (results, had_error) = match load_and_format(
        &args.paths,
        excludes,
        respect_vcs_ignore,
        changed_files_scope,
    ) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_error_finding = false;
    let mut findings = Vec::new();
    for (path, kind, style, markdown_strict, original, formatted) in results {
        if kind == FileKind::Orphan {
            // Story B1: itemized, coded, positioned findings for the
            // un-owned-text allowlist — the same set A1's BOM strip covers.
            let diagnostics = prim_fmt::hygiene_diagnostics(&original, &style);
            if !diagnostics.is_empty() {
                any_error_finding = true;
                for diagnostic in &diagnostics {
                    if args.format.is_some() {
                        findings.push(Finding::diagnostic(&path, diagnostic));
                    } else {
                        ui::lint_diagnostic(&path, diagnostic);
                    }
                }
            }
        } else if kind == FileKind::Markdown {
            let diagnostics = prim_fmt::lint_markdown(&original, markdown_strict);
            if !diagnostics.is_empty() {
                any_error_finding |= diagnostics.iter().any(|diagnostic| diagnostic.is_error);
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
            any_error_finding = true;
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
    } else if any_error_finding {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

fn emit_report(format: OutputFormat, mode: ReportMode, findings: &[Finding]) {
    print!("{}", report::render(format, mode, findings));
}

#[cfg(test)]
mod tests {
    use super::{is_idempotent_second_pass, second_pass_matches_first};
    use prim_fmt::{FileKind, Style};

    #[test]
    fn comparison_flags_a_changed_second_pass() {
        assert!(!second_pass_matches_first("once\n", "twice\n"));
    }

    #[test]
    fn json_output_is_stable_on_a_second_pass() {
        let style = Style::default();
        let formatted = prim_fmt::format(FileKind::Json, "{\"a\":1}\n", &style).unwrap();

        assert!(is_idempotent_second_pass(FileKind::Json, &formatted, &style).unwrap());
    }
}
