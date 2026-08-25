//! Scaffold or minimally merge prim's Markdown strict-glob placement map into
//! `.editorconfig` (story G4).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::Value;

use crate::ui;
use crate::write;

mod map;
mod outcome;
mod sections;

use map::merge;

const EDITORCONFIG_NAME: &str = ".editorconfig";
const MDBOOK_NAME: &str = "book.toml";
const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";
const DEFAULT_STRICT_DIR: &str = "docs";
const MDBOOK_DEFAULT_SRC: &str = "src";
/// The literal docs/wip exemption glob — a constant so it can be compared
/// against a detected strict glob (a mdBook `src = "docs/wip"` derives this
/// same glob) rather than duplicated as a string literal.
const DOCS_WIP_GLOB: &str = "docs/wip/**.md";
/// The directory [`DOCS_WIP_GLOB`] exempts, for comparing a detected strict
/// glob against it.
const DOCS_WIP_DIR: &str = "docs/wip";
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
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory(path) => write!(f, "{}: not a directory", path.display()),
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

    if !editorconfig.exists() {
        write::atomic(&editorconfig, &scaffold).map_err(|source| Error::WriteEditorConfig {
            path: editorconfig.clone(),
            source,
        })?;
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

    write::atomic(&editorconfig, &merged.contents).map_err(|source| Error::WriteEditorConfig {
        path: editorconfig.clone(),
        source,
    })?;
    Ok(Outcome {
        message: format!(
            "updated {}: {}",
            editorconfig.display(),
            merged.actions.join("; ")
        ),
    })
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
