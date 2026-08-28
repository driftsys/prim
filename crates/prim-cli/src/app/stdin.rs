//! The stdin format-on-save pair: `fmt`/`fix --stdin-filepath` and
//! `lint --stdin-filepath`. Both read the buffer from stdin, classify it by
//! path, and either print the formatted result (`fmt`) or report findings
//! without writing anything (`lint`).

use std::io::Read;
use std::path::Path;

use super::{
    EXIT_ACTIONABLE, EXIT_ERROR, EXIT_OK, FORMAT_DRIFT_CODE, FORMAT_DRIFT_FINDING, emit_report,
};
use crate::cli::OutputFormat;
use crate::editorconfig;
use crate::report::{Finding, ReportMode};
use crate::ui;
use prim_fmt::FileKind;

/// Read stdin, format it, and write the result to stdout (format-on-save).
///
/// The path selects the formatter; if prim does not own that file type, the
/// input is passed through unchanged.
pub(super) fn run_fmt_stdin(path: &Path) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    // A generated file is echoed untouched: its tool owns every byte (AD-0011).
    if prim_fmt::generated_by(path).is_some() {
        print!("{input}");
        return EXIT_OK;
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
pub(super) fn run_lint_stdin(path: &Path, format: Option<OutputFormat>) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    // A generated file has no lint findings: its tool owns every byte
    // (AD-0011), so there is nothing actionable to report.
    if prim_fmt::generated_by(path).is_some() {
        if let Some(format) = format {
            emit_report(format, ReportMode::Lint, &[]);
        }
        return EXIT_OK;
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
            let policy = crate::mdlint_policy::resolve(path);
            crate::mdlint_policy::UnknownRuleReporter::new().report(&policy);
            let style = editorconfig::resolve(path);
            let diagnostics = prim_fmt::lint_markdown(&input, &style, &policy.selection);
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
