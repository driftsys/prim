//! The prim formatting engine.
//!
//! prim is an opinionated, near-zero-config formatter for a repository's
//! connective tissue — Markdown, JSON/JSONC, YAML, TOML — plus whitespace
//! hygiene on a curated set of un-owned text files.
//!
//! The engine has two steps:
//!
//! 1. [`classify`] decides whether prim owns a file (by name/extension) and, if
//!    so, what [`FileKind`] it is. Files prim does not own are left untouched.
//! 2. [`format`] applies the canonical formatting for that kind.
//!
//! At this stage [`format`] applies only the format-agnostic **whitespace
//! hygiene** pass (trailing-whitespace removal, single final line-feed, LF line
//! endings). Structured per-format canonicalisation (Markdown wrapping, JSON
//! re-indentation, …) is added per [`FileKind`] in later milestones.

mod classify;
mod error;
mod hygiene;
mod json;
mod markdown;
mod style;
mod toml;
mod yaml;

pub use classify::{FileKind, classify};
pub use error::FormatError;
pub use style::{Indent, LineEnding, Style};

/// Format `source` as the given [`FileKind`] under `style`.
///
/// Returns [`FormatError`] when a structured format cannot be parsed; the CLI
/// then leaves the file unchanged and reports it (FR-6.3). The `match` is the
/// dispatch point where structured per-format passes (FR-1) attach.
pub fn format(kind: FileKind, source: &str, style: &Style) -> Result<String, FormatError> {
    match kind {
        FileKind::Json | FileKind::Jsonc => json::format(source, style),
        FileKind::Toml => toml::format(source, style),
        FileKind::Yaml => yaml::format(source, style),
        FileKind::Markdown => markdown::format(source, style),
        FileKind::Orphan => Ok(hygiene::hygiene(source, style)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hygiene_kinds_return_ok() {
        let style = Style::default();
        assert_eq!(format(FileKind::Orphan, "x  \n", &style).unwrap(), "x\n");
        assert_eq!(format(FileKind::Markdown, "a\r\n", &style).unwrap(), "a\n");
    }
}
