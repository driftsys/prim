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
mod hygiene;
mod style;

pub use classify::{FileKind, classify};
pub use style::{Indent, LineEnding, Style};

/// Format `source` as the given [`FileKind`] and return the result.
///
/// Every kind currently receives only the whitespace-hygiene pass; the `match`
/// is the dispatch point where structured per-format passes (FR-1) attach.
pub fn format(kind: FileKind, source: &str) -> String {
    match kind {
        FileKind::Markdown
        | FileKind::Json
        | FileKind::Jsonc
        | FileKind::Yaml
        | FileKind::Toml
        | FileKind::Orphan => hygiene::hygiene(source),
    }
}
