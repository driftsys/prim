use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// When to use coloured output.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorWhen {
    /// Colour if stderr is a TTY and `NO_COLOR` is unset.
    Auto,
    /// Always use colour.
    Always,
    /// Never use colour.
    Never,
}

/// prim — opinionated, near-zero-config formatter for a repository's
/// connective tissue (Markdown, JSON/JSONC, YAML, TOML) plus whitespace
/// hygiene on a curated set of un-owned text files.
///
/// Bare `prim [PATH]...` is a permanent alias for `prim fmt [PATH]...`
/// (AD-0007) — the verb is optional in practice because `main`'s argv
/// preprocessor injects `fmt` before parsing when no verb is given.
#[derive(Parser, Debug)]
#[command(name = "prim", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub verb: Verb,

    /// Exclude paths matching the given glob (repeatable).
    #[arg(long, value_name = "GLOB", global = true)]
    pub exclude: Vec<String>,

    /// When to use coloured output.
    #[arg(long, default_value = "auto", global = true)]
    pub color: ColorWhen,

    /// Generate a shell completion script for the given shell and print it
    /// to stdout.
    #[arg(long, value_name = "SHELL", global = true)]
    pub completions: Option<Shell>,
}

/// The three operating verbs (AD-0007). Each has its own exit-code contract:
/// `fmt`/`fix` write and gate on format drift; `lint` never writes and gates
/// on error-severity findings only.
#[derive(Subcommand, Debug)]
pub enum Verb {
    /// Format the parsed formats and apply whitespace hygiene (writes in
    /// place by default; today's default behaviour).
    Fmt(FmtArgs),
    /// Report hygiene and content violations; never rewrites a file.
    Lint(LintArgs),
    /// Format, plus autofixable content rules on top of `fmt`.
    Fix(FmtArgs),
}

/// Shared arguments for `fmt` and `fix`: both write in place by default and
/// share the `--check`/`--diff`/`--stdin-filepath` format-drift surface
/// (AD-0007 §2). `fix` has no autofix-specific flags yet — the autofixable
/// content rules are still future work.
#[derive(Args, Debug)]
pub struct FmtArgs {
    /// Files or directories to process. Directories are searched recursively,
    /// honoring .gitignore, .ignore, and .primignore. Defaults to the current
    /// directory when no paths are given.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Check mode: write nothing, exit non-zero if any file would change,
    /// and list the files that would change. Intended as a CI gate.
    #[arg(long, conflicts_with = "diff")]
    pub check: bool,

    /// Diff mode: print a unified diff of pending changes and write nothing.
    #[arg(long)]
    pub diff: bool,

    /// Read from stdin and write the formatted result to stdout. The path
    /// names the file so the right formatter is selected (format-on-save).
    /// Mutually exclusive with --check and --diff.
    #[arg(long, value_name = "PATH", conflicts_with_all = ["check", "diff"])]
    pub stdin_filepath: Option<PathBuf>,
}

/// Arguments for `lint`: report-only, so it has neither `--check` nor
/// `--diff` (AD-0007 §2) — every run already reports every finding.
#[derive(Args, Debug)]
pub struct LintArgs {
    /// Files or directories to lint. Directories are searched recursively,
    /// honoring .gitignore, .ignore, and .primignore. Defaults to the current
    /// directory when no paths are given.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Read from stdin and report findings for it. The path names the file so
    /// the right rules are selected.
    #[arg(long, value_name = "PATH")]
    pub stdin_filepath: Option<PathBuf>,
}
