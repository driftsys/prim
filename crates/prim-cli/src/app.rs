//! Operating-mode dispatch over prim's formatting verbs (`fmt`/`lint`/`fix`,
//! AD-0007) plus the one-shot `init` scaffolder.

use std::path::Path;

mod load;
mod paths;
mod stdin;

use self::paths::{run_check_idempotence_paths, run_fmt_paths, run_lint_paths};
use self::stdin::{run_fmt_stdin, run_lint_stdin};
use crate::changed_files::ChangedFilesScope;
use crate::cli::{Cli, ExplainArgs, FixArgs, FmtArgs, InitArgs, LintArgs, OutputFormat, Verb};
use crate::discover;
use crate::explain;
use crate::init;
use crate::lsp;
use crate::provenance;
use crate::report::{self, Finding, ReportMode};
use crate::ui;

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
        Verb::Fmt(args) => run_fmt(
            args,
            &cli.exclude,
            ignore_settings(cli),
            &changed_files_scope,
        ),
        Verb::Fix(args) => run_fix(
            args,
            &cli.exclude,
            ignore_settings(cli),
            &changed_files_scope,
        ),
        Verb::Lint(args) => run_lint(
            args,
            &cli.exclude,
            ignore_settings(cli),
            &changed_files_scope,
        ),
        Verb::Init(args) => run_init(args),
        Verb::Explain(args) => run_explain(args),
        Verb::Lsp => lsp::run(),
    }
}

fn ignore_settings(cli: &Cli) -> discover::IgnoreSettings {
    discover::IgnoreSettings {
        vcs: !cli.no_ignore,
        primignore: !cli.no_primignore,
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
    ignores: discover::IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    if let Some(path) = args.write.stdin_filepath.as_deref() {
        return run_fmt_stdin(path);
    }
    if args.check_idempotence {
        return run_check_idempotence_paths(&args.write, excludes, ignores, changed_files_scope);
    }
    run_fmt_paths(
        &args.write,
        args.format,
        excludes,
        false,
        ignores,
        changed_files_scope,
    )
}

fn run_fix(
    args: &FixArgs,
    excludes: &[String],
    ignores: discover::IgnoreSettings,
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
        ignores,
        changed_files_scope,
    )
}

fn run_lint(
    args: &LintArgs,
    excludes: &[String],
    ignores: discover::IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> i32 {
    if let Some(path) = args.stdin_filepath.as_deref() {
        return run_lint_stdin(path, args.format);
    }
    run_lint_paths(args, excludes, ignores, changed_files_scope)
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
            let explanation = provenance::explain(path, kind);
            print!("{}", explain::render(path, &explanation.settings));
            // Reporting a refused `prim_mdlint_enable` or `prim_mdlint_disable`
            // id is the command's job, not the query's: the reporter has to
            // outlive a single resolution for "once per run" to mean anything.
            if let Some(policy) = &explanation.mdlint_policy {
                crate::mdlint_policy::RejectedRuleReporter::new().report(policy);
            }
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

fn emit_report(format: OutputFormat, mode: ReportMode, findings: &[Finding]) {
    print!("{}", report::render(format, mode, findings));
}
