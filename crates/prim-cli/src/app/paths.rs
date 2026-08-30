//! The path-discovery pipeline shared by `fmt`, `fmt --check-idempotence`,
//! and `lint`: load and format every discovered file, then differ only in
//! what each verb does with the (original, formatted) pair.

use std::path::Path;

use super::load::{Loaded, load_and_format};
use super::{
    EXAMINED_NOTHING, EXIT_ACTIONABLE, EXIT_ERROR, EXIT_OK, FORMAT_CHECK_FINDING,
    FORMAT_DRIFT_CODE, FORMAT_DRIFT_FINDING, emit_report,
};
use crate::changed_files::ChangedFilesScope;
use crate::cli::{LintArgs, OutputFormat, WriteArgs};
use crate::diff;
use crate::discover;
use crate::report::{Finding, ReportMode};
use crate::ui;
use crate::write;
use prim_fmt::{FileKind, Style};

pub(super) fn run_fmt_paths(
    args: &WriteArgs,
    format: Option<OutputFormat>,
    excludes: &[String],
    is_fix: bool,
    ignores: discover::IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let Loaded {
        files: results,
        mut had_error,
        examined_nothing,
    } = match load_and_format(&args.paths, excludes, ignores, changed_files_scope) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_would_change = false;
    let mut written_to_worktree = 0usize;
    let mut findings = Vec::new();
    for (path, _kind, _style, _markdown_policy, original, formatted) in results {
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
        } else {
            written_to_worktree += 1;
        }
    }

    if *changed_files_scope == ChangedFilesScope::Staged && written_to_worktree > 0 {
        ui::warning(&staged_write_warning(written_to_worktree));
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
    } else if gates_on_pending_findings && examined_nothing {
        // Reported after the (empty) findings report, so `--format` still
        // changes stdout alone (FR-5.8): the document a pipeline uploads is
        // emitted either way, and the exit code carries the failure.
        ui::error(EXAMINED_NOTHING);
        EXIT_ERROR
    } else if gates_on_pending_findings && any_would_change {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

/// What `fmt`/`fix` say after writing under `--staged` (issue #159).
///
/// `--staged` chooses paths from the index, but the write goes to the working
/// tree and never touches the index, so a commit made straight afterwards can
/// still record the pre-format blob.
///
/// The message reports only what prim knows without inspecting the index: how
/// many files it wrote, and that it left the index alone. It does not claim
/// the index is stale, and it does not tell the user to re-stage. Both would
/// be wrong for a partially staged file, whose staged blob prim never read and
/// may already be canonical, and where `git add` would also stage the unstaged
/// remainder the user deliberately kept out of the commit. That is the same
/// reason prim declines to re-stage on the user's behalf: re-staging belongs
/// to the hook runner, which knows what it staged.
///
/// The pointer is plain `git diff`, not `git diff --cached`. The gap prim just
/// opened is working tree versus index, which is what `git diff` shows;
/// `git diff --cached` is index versus `HEAD` and cannot show it at all, so it
/// can look clean at the exact moment the commit would record unformatted
/// bytes.
///
/// Warnings never raise the exit code (AD-0007 §4), so the contract is
/// unchanged.
fn staged_write_warning(written_to_worktree: usize) -> String {
    let subject = if written_to_worktree == 1 {
        "1 file was".to_string()
    } else {
        format!("{written_to_worktree} files were")
    };
    format!(
        "{subject} formatted in the working tree, but --staged does not update the index. Run git diff to see what is not staged."
    )
}

pub(super) fn run_check_idempotence_paths(
    args: &WriteArgs,
    excludes: &[String],
    ignores: discover::IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let Loaded {
        files: results,
        mut had_error,
        examined_nothing,
    } = match load_and_format(&args.paths, excludes, ignores, changed_files_scope) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_non_idempotent = false;
    for (path, kind, style, _markdown_policy, _original, formatted) in results {
        let stable = match is_idempotent_second_pass(&path, kind, &formatted, &style) {
            Ok(stable) => stable,
            Err(message) => {
                ui::error(&message);
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
    } else if examined_nothing {
        ui::error(EXAMINED_NOTHING);
        EXIT_ERROR
    } else if any_non_idempotent {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

/// Whether formatting `formatted` again leaves it alone (FR-6.1).
///
/// Returns the message to report rather than a typed error: the two failures
/// have nothing in common past "prim could not answer" — one is a file the
/// formatter rejected on its own second pass, the other is a panic, which is
/// prim's bug and carries its own wording (AD-0017).
fn is_idempotent_second_pass(
    path: &Path,
    kind: FileKind,
    formatted: &str,
    style: &Style,
) -> Result<bool, String> {
    match crate::formatting::contained(|| prim_fmt::format(kind, formatted, style)) {
        Ok(Ok(reformatted)) => Ok(second_pass_matches_first(formatted, &reformatted)),
        Ok(Err(err)) => Err(format!(
            "{}: second formatting pass failed: {err}",
            path.display()
        )),
        Err(_) => Err(crate::formatting::panic_message(path)),
    }
}

fn second_pass_matches_first(formatted: &str, reformatted: &str) -> bool {
    formatted == reformatted
}

pub(super) fn run_lint_paths(
    args: &LintArgs,
    excludes: &[String],
    ignores: discover::IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    let Loaded {
        files: results,
        had_error,
        examined_nothing,
    } = match load_and_format(&args.paths, excludes, ignores, changed_files_scope) {
        Ok(outcome) => outcome,
        Err(err) => {
            ui::error(&err.to_string());
            return EXIT_ERROR;
        }
    };

    let mut any_error_finding = false;
    let mut findings = Vec::new();
    let mut unknown_rule_reporter = crate::mdlint_policy::UnknownRuleReporter::new();
    for (path, kind, style, markdown_policy, original, formatted) in results {
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
            unknown_rule_reporter.report(&markdown_policy);
            let diagnostics = prim_fmt::lint_markdown(
                &original,
                markdown_policy.strict,
                &markdown_policy.disabled,
                markdown_policy.report_line_length,
            );
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
    } else if examined_nothing {
        // `lint` is report-only, so its exit code is its whole answer:
        // reporting nothing after examining nothing is the fail-open #112
        // closed. The report is emitted first, as under `fmt --check`.
        ui::error(EXAMINED_NOTHING);
        EXIT_ERROR
    } else if any_error_finding {
        EXIT_ACTIONABLE
    } else {
        EXIT_OK
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

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

        assert!(
            is_idempotent_second_pass(Path::new("a.json"), FileKind::Json, &formatted, &style)
                .unwrap()
        );
    }
}
