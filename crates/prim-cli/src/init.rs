//! Scaffold or minimally merge prim's Markdown strict-glob placement map into
//! `.editorconfig` (story G4).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use prim_fmt::LineEnding;
use toml::Value;

use crate::editorconfig;
use crate::ui;
use crate::write;

mod cascade;
mod map;
mod outcome;
mod sections;

use map::merge;

const EDITORCONFIG_NAME: &str = ".editorconfig";
const MDBOOK_NAME: &str = "book.toml";
const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";
const DEFAULT_STRICT_DIR: &str = "docs";
const MDBOOK_DEFAULT_SRC: &str = "src";
/// One directory of Superpowers working memory that the strict tier must not
/// reach.
struct WorkingMemory {
    /// The directory itself, for comparing a detected strict glob against it.
    dir: &'static str,
    /// The exemption section prim writes for it. Spelled out rather than
    /// derived from `dir` so it can stay a `&'static str`; a test pins the
    /// two against each other.
    glob: &'static str,
}

/// The directories holding Superpowers working memory, in canonical order.
/// Specs and plans live under `docs/wip/` while a branch is open; gardening
/// moves the raw originals to `docs/archive/`. Both are exempt from the strict
/// tier, because gardening is a move rather than an edit: a document whose
/// lint tier changed the moment somebody relocated it, without touching a
/// byte of it, would make a repository's own CI fail on work it had just
/// filed away.
const WORKING_MEMORY: [WorkingMemory; 2] = [
    WorkingMemory {
        dir: "docs/wip",
        glob: "docs/wip/**.md",
    },
    WorkingMemory {
        dir: "docs/archive",
        glob: "docs/archive/**.md",
    },
];
/// The strict glob a book rooted at the repository itself derives — it covers
/// every Markdown file, so no floor section can sit under it.
const EVERYTHING_GLOB: &str = "**.md";

/// The user-visible result of `prim init`.
#[derive(Debug)]
pub struct Outcome {
    pub message: String,
}

/// `prim init` failures map to exit code `2`.
#[derive(Debug)]
pub enum Error {
    NotDirectory(PathBuf),
    /// prim's own placement map for this strict glob does not resolve the way
    /// its sections say it should. A bug in prim, not in the repository.
    DefectiveMap {
        glob: String,
        flaws: Vec<String>,
    },
    ReadBookToml {
        path: PathBuf,
        source: io::Error,
    },
    ReadEditorConfig {
        path: PathBuf,
        source: io::Error,
    },
    WriteEditorConfig {
        path: PathBuf,
        source: io::Error,
    },
    /// The `.editorconfig` prim would write is a symbolic link. Writing is a
    /// temporary file plus a rename (FR-6.4), which would replace the link
    /// with a regular file and leave the shared config it points at unchanged
    /// (AD-0016).
    SymlinkedEditorConfig(PathBuf),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory(path) => write!(f, "{}: not a directory", path.display()),
            Self::SymlinkedEditorConfig(path) => write!(
                f,
                "{}: is a symbolic link; prim wrote nothing (run prim init on the \
                 directory holding the file it points at)",
                path.display()
            ),
            Self::DefectiveMap { glob, flaws } => write!(
                f,
                "prim's own Markdown map for [{glob}] does not resolve the way it is meant to \
                 ({}); this is a bug in prim, so it wrote nothing — please report it",
                flaws.join("; ")
            ),
            Self::ReadBookToml { path, source }
            | Self::ReadEditorConfig { path, source }
            | Self::WriteEditorConfig { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
        }
    }
}

/// Scaffold or minimally merge `.editorconfig` in `target_dir`.
pub fn run(target_dir: &Path) -> Result<Outcome, Error> {
    if !target_dir.is_dir() {
        return Err(Error::NotDirectory(target_dir.to_path_buf()));
    }

    let strict_glob = detect_strict_glob(target_dir)?;
    // The map is only obtainable already checked, so no path here can write
    // an unchecked one. Intent comes from the declared sections, never from
    // this text, so the check cannot pass by circularity.
    let scaffold = map::checked_scaffold(&strict_glob).map_err(|flaws| Error::DefectiveMap {
        glob: strict_glob.clone(),
        flaws,
    })?;
    let editorconfig = target_dir.join(EDITORCONFIG_NAME);
    // Before anything is written, and before `exists()` is consulted: that
    // follows the link, so a live one reaches the merge path and a dangling
    // one reaches the scaffold path, and both renames destroy it (AD-0016).
    if crate::symlink::is_symlink(&editorconfig) {
        return Err(Error::SymlinkedEditorConfig(editorconfig));
    }
    // Asked before any write: prim's own `root = true` stops the very walk
    // this reads, so afterwards there is nothing left to find.
    let ancestry = cascade::from_ancestors(target_dir);

    if !editorconfig.exists() {
        write_resolved(&editorconfig, &scaffold).map_err(|source| Error::WriteEditorConfig {
            path: editorconfig.clone(),
            source,
        })?;
        // The scaffold opens with `root = true`, so creating one here cuts
        // this directory off from anything above it just as a merge would.
        warn_if_severed(target_dir, &ancestry);
        // Read off the same sections the scaffold was built from: a summary
        // that advertises a section prim decided not to write is the same
        // defect in miniature as writing one that does not hold.
        let placement = map::canonical_specs(&strict_glob)
            .iter()
            .map(|spec| format!("[{}]", spec.glob))
            .collect::<Vec<_>>()
            .join(" → ");
        return Ok(Outcome {
            message: format!(
                "created {} with Markdown strict-glob map ({placement})",
                editorconfig.display()
            ),
        });
    }

    let existing = fs::read_to_string(&editorconfig).map_err(|source| Error::ReadEditorConfig {
        path: editorconfig.clone(),
        source,
    })?;
    let merged = merge(&existing, &strict_glob);

    for warning in &merged.warnings {
        ui::warning(warning);
    }

    if merged.actions.is_empty() {
        // A warning above means at least one canonical section was left out
        // or is contradicted by the file's own order — that is not the same
        // outcome as "the map is already present", and a scripted caller or
        // a skimming reader takes this message, not the warnings, as the
        // result.
        let message = if merged.warnings.is_empty() {
            format!(
                "{} already contains the Markdown strict-glob map",
                editorconfig.display()
            )
        } else {
            format!(
                "{} left unchanged — see the warning(s) above",
                editorconfig.display()
            )
        };
        return Ok(Outcome { message });
    }

    write_resolved(&editorconfig, &merged.contents).map_err(|source| Error::WriteEditorConfig {
        path: editorconfig.clone(),
        source,
    })?;
    if merged.added_root {
        warn_if_severed(target_dir, &ancestry);
    }
    Ok(Outcome {
        message: format!(
            "updated {}: {}",
            editorconfig.display(),
            merged.actions.join("; ")
        ),
    })
}

/// Write `text` to `path` under the `end_of_line` that will apply to `path`
/// once it is there.
///
/// `prim init` composes its output from the existing file's lines plus its own
/// additions, which carry LF, so in a CRLF file the two mix. FR-2.3 says which
/// one wins: the `end_of_line` resolved for the file.
///
/// That has to be resolved *after* the write, not before. The file prim writes
/// opens with `root = true`, which stops EditorConfig's upward walk, so a
/// `end_of_line` an ancestor declared before the write no longer applies to the
/// file once it exists. Resolving first would write CRLF that the very next
/// `prim fmt --check` reports. Writing LF first and correcting settles on what
/// `prim fmt` will see, and costs a second write only where CRLF is asked for.
fn write_resolved(path: &Path, text: &str) -> io::Result<()> {
    write::atomic(path, &with_line_endings(text, LineEnding::Lf))?;
    let ending = editorconfig::resolve(path).end_of_line;
    if ending != LineEnding::Lf {
        write::atomic(path, &with_line_endings(text, ending))?;
    }
    Ok(())
}

/// `text` with every line ending — CRLF, LF, or a bare CR — rewritten to
/// `ending`.
fn with_line_endings(text: &str, ending: LineEnding) -> String {
    let lf = text.replace("\r\n", "\n").replace('\r', "\n");
    match ending {
        LineEnding::Lf => lf,
        LineEnding::CrLf => lf.replace('\n', "\r\n"),
    }
}

/// Report the cascade `root = true` just cut off, if there is anything to
/// report. Emitted after the write so it follows the write it describes, but
/// read from `ancestry`, which was gathered before it. Never changes the exit
/// code.
fn warn_if_severed(target_dir: &Path, ancestry: &cascade::Ancestry) {
    if let Some(warning) = cascade::severing_warning(target_dir, ancestry) {
        ui::warning(&warning);
    }
}

fn detect_strict_glob(target_dir: &Path) -> Result<String, Error> {
    let book_toml = target_dir.join(MDBOOK_NAME);
    if !book_toml.exists() {
        return Ok(strict_glob_for_dir(DEFAULT_STRICT_DIR));
    }

    let content = fs::read_to_string(&book_toml).map_err(|source| Error::ReadBookToml {
        path: book_toml,
        source,
    })?;
    Ok(strict_glob_from_book_toml(&content))
}

fn strict_glob_from_book_toml(content: &str) -> String {
    let src = toml::from_str::<Value>(content)
        .ok()
        .and_then(|value| {
            value
                .get("book")
                .and_then(|book| book.get("src"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|src| !src.trim().is_empty())
        .unwrap_or_else(|| MDBOOK_DEFAULT_SRC.to_string());
    strict_glob_for_dir(&src)
}

fn strict_glob_for_dir(dir: &str) -> String {
    let mut dir = dir.trim().trim_matches('/');
    while let Some(stripped) = dir.strip_prefix("./") {
        dir = stripped.trim_start_matches('/');
    }
    if dir.is_empty() || dir == "." {
        "**.md".to_string()
    } else {
        format!("{dir}/**.md")
    }
}

#[cfg(test)]
mod tests;
