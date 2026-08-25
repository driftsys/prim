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

/// The user-visible result of `prim init`.
#[derive(Debug)]
pub struct Outcome {
    pub message: String,
}

/// `prim init` failures map to exit code `2`.
#[derive(Debug)]
pub enum Error {
    NotDirectory(PathBuf),
    ReadBookToml { path: PathBuf, source: io::Error },
    ReadEditorConfig { path: PathBuf, source: io::Error },
    WriteEditorConfig { path: PathBuf, source: io::Error },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotDirectory(path) => write!(f, "{}: not a directory", path.display()),
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
    let editorconfig = target_dir.join(EDITORCONFIG_NAME);

    if !editorconfig.exists() {
        write::atomic(&editorconfig, &scaffold(&strict_glob)).map_err(|source| {
            Error::WriteEditorConfig {
                path: editorconfig.clone(),
                source,
            }
        })?;
        // The docs/wip exemption is only its own arrow in the summary when it
        // is a distinct section from the strict glob (see `scaffold`).
        let placement = if strict_glob == DOCS_WIP_GLOB {
            format!("[*.md] → [{strict_glob}] → [**/SUMMARY.md]")
        } else {
            format!("[*.md] → [{strict_glob}] → [{DOCS_WIP_GLOB}] → [**/SUMMARY.md]")
        };
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

fn scaffold(strict_glob: &str) -> String {
    // When the detected strict glob is itself `docs/wip/**.md` (e.g. a
    // mdBook with `src = "docs/wip"`), the exemption would be a second
    // section for the exact same glob, written after the strict one — under
    // EditorConfig's last-match-wins resolution that silently flips the
    // whole directory back to the floor tier. The author asked for that
    // directory to be strict, so skip the exemption rather than defeat it.
    let docs_wip_exemption = if strict_glob == DOCS_WIP_GLOB {
        String::new()
    } else {
        format!("[{DOCS_WIP_GLOB}]\n{MDLINT_STRICT_KEY} = false\n")
    };
    format!(
        "root = true\n[*.md]\n{MDLINT_STRICT_KEY} = false\n[{strict_glob}]\n{MDLINT_STRICT_KEY} = true\n{docs_wip_exemption}[**/SUMMARY.md]\n{MDLINT_STRICT_KEY} = false\n"
    )
}

#[cfg(test)]
mod tests;
