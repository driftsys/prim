use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use clap_complete::Shell;

/// When to use coloured output.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorWhen {
    /// Colour if stdout is a TTY.
    Auto,
    /// Always use colour.
    Always,
    /// Never use colour.
    Never,
}

/// prim — opinionated, near-zero-config formatter for a repository's
/// connective tissue (Markdown, JSON/JSONC, YAML, TOML) plus whitespace
/// hygiene on a curated set of un-owned text files.
#[derive(Parser, Debug)]
#[command(name = "prim", version, about, long_about = None)]
pub struct Cli {
    /// Files to format. (Recursive directory discovery lands in a later
    /// milestone; for now pass explicit file paths.)
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
    #[arg(long, value_name = "PATH")]
    pub stdin_filepath: Option<PathBuf>,

    /// Exclude paths matching the given glob (repeatable).
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// When to use coloured output.
    #[arg(long, default_value = "auto")]
    pub color: ColorWhen,

    /// Generate a shell completion script for the given shell and print it
    /// to stdout.
    #[arg(long, value_name = "SHELL")]
    pub completions: Option<Shell>,
}
