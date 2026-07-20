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

/// Machine-readable report formats for report-only command modes.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OutputFormat {
    /// Emit a stable JSON document describing findings.
    Json,
    /// Emit a SARIF 2.1.0 document describing findings.
    Sarif,
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

    /// Ignore VCS ignore files (.gitignore, global gitignore, .git/info/exclude).
    #[arg(long, global = true)]
    pub no_ignore: bool,

    /// Limit the file set to paths reported by `git diff --name-only <REF>`:
    /// files that differ between `<REF>` and the current working tree (staged
    /// + unstaged; plain two-way diff, no merge-base).
    #[arg(long, value_name = "REF", global = true, conflicts_with = "staged")]
    pub since: Option<String>,

    /// Limit the file set to paths reported by `git diff --name-only --cached`:
    /// files staged in the git index relative to `HEAD`.
    #[arg(long, global = true, conflicts_with = "since")]
    pub staged: bool,

    /// When to use coloured output.
    #[arg(long, default_value = "auto", global = true)]
    pub color: ColorWhen,

    /// Generate a shell completion script for the given shell and print it
    /// to stdout.
    #[arg(long, value_name = "SHELL", global = true)]
    pub completions: Option<Shell>,
}

/// prim's formatting verbs plus the one-shot `init` scaffolder. AD-0007 still
/// governs only the formatting verbs: `fmt`/`fix` write and gate on format
/// drift; `lint` never writes and gates on error-severity findings only.
#[derive(Subcommand, Debug)]
pub enum Verb {
    /// Format the parsed formats and apply whitespace hygiene (writes in
    /// place by default; today's default behaviour).
    Fmt(FmtArgs),
    /// Report hygiene and content violations; never rewrites a file.
    Lint(LintArgs),
    /// Format, plus autofixable content rules on top of `fmt`.
    Fix(FixArgs),
    /// Scaffold or minimally merge prim's Markdown strict-glob map into
    /// `.editorconfig`.
    Init(InitArgs),
    /// Print the `.editorconfig` settings that apply to a single file and
    /// where each came from (a `.editorconfig` section, or prim's default).
    Explain(ExplainArgs),
    /// Run a Language Server Protocol server over stdin/stdout, exposing
    /// prim's formatter as a format-on-save provider for editors.
    Lsp,
}

/// Shared arguments for `fmt` and `fix`: both write in place by default and
/// share the `--check`/`--diff`/`--stdin-filepath` format-drift surface
/// (AD-0007 §2). `fix` has no autofix-specific flags yet — the autofixable
/// content rules are still future work.
#[derive(Args, Debug)]
pub struct WriteArgs {
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

/// Arguments for `fmt`: write in place by default, gate on `--check`, preview
/// with `--diff`, and optionally emit machine-readable reports from
/// `--check`.
#[derive(Args, Debug)]
pub struct FmtArgs {
    #[command(flatten)]
    pub write: WriteArgs,

    /// Verify that prim reaches a fixed point after one formatting pass:
    /// format(format(x)) == format(x). Writes nothing and reports any file
    /// whose second pass still changes bytes.
    #[arg(long, conflicts_with_all = ["check", "diff", "stdin_filepath"])]
    pub check_idempotence: bool,

    /// Machine-readable report format for `--check`.
    #[arg(
        long,
        value_enum,
        requires = "check",
        conflicts_with_all = ["diff", "check_idempotence"]
    )]
    pub format: Option<OutputFormat>,
}

/// Arguments for `fix`: the same write/check/diff surface as `fmt`, but no
/// machine-readable report mode yet (story D2 is scoped to `fmt --check`
/// and `lint` only).
#[derive(Args, Debug)]
pub struct FixArgs {
    #[command(flatten)]
    pub write: WriteArgs,
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

    /// Machine-readable report format for lint findings.
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
}

/// Arguments for `init`: scaffold or minimally merge `.editorconfig` in the
/// target directory, defaulting to the current working directory.
#[derive(Args, Debug)]
pub struct InitArgs {
    /// The repository root to scaffold or update in place. prim reads
    /// `book.toml` and writes only `.editorconfig` in this directory.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

/// Arguments for `explain`: a single target file, unlike every other verb's
/// `paths: Vec<PathBuf>` — `explain` reports on exactly one file at a time.
#[derive(Args, Debug)]
pub struct ExplainArgs {
    /// The file to resolve `.editorconfig` settings for. Need not exist:
    /// resolution is name/extension-based, like the rest of prim's file-kind
    /// classification.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Verb};

    #[test]
    fn no_ignore_is_accepted_before_formatting_verbs() {
        for argv in [
            ["prim", "--no-ignore", "fmt", "."].as_slice(),
            ["prim", "--no-ignore", "lint", "."].as_slice(),
            ["prim", "--no-ignore", "fix", "."].as_slice(),
        ] {
            let cli =
                Cli::try_parse_from(argv).unwrap_or_else(|err| panic!("argv {argv:?}: {err}"));
            assert!(cli.no_ignore, "argv: {argv:?}");
            assert!(matches!(
                cli.verb,
                Verb::Fmt(_) | Verb::Lint(_) | Verb::Fix(_)
            ));
        }
    }

    #[test]
    fn changed_file_flags_are_accepted_before_formatting_verbs() {
        let staged =
            Cli::try_parse_from(["prim", "--staged", "fmt", "."]).expect("--staged parses");
        assert!(staged.staged);
        assert!(matches!(staged.verb, Verb::Fmt(_)));

        let since =
            Cli::try_parse_from(["prim", "--since", "main", "lint", "."]).expect("--since parses");
        assert_eq!(since.since.as_deref(), Some("main"));
        assert!(matches!(since.verb, Verb::Lint(_)));
    }
}
