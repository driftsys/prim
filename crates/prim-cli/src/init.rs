//! Scaffold or minimally merge prim's Markdown strict-glob placement map into
//! `.editorconfig` (story G4).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::Value;

use crate::ui;
use crate::write;

use sections::{
    SectionSpec, bool_word, existing_order_conflicts, has_top_level_root, header_lines, key_line,
    lower_bound, matching_sections, push_insert, section_block, split_lines, upper_bound,
};

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

struct MergeResult {
    contents: String,
    actions: Vec<String>,
    /// One entry per canonical-order violation prim found: either a section
    /// it could not place because the file's own section order contradicts
    /// the canonical order, or two already-present sections whose relative
    /// order contradicts it. Either way, prim never reorders sections a
    /// person wrote, so the file is left exactly as found and the warning is
    /// the only output.
    warnings: Vec<String>,
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

fn merge(existing: &str, strict_glob: &str) -> MergeResult {
    let mut specs = vec![
        SectionSpec {
            glob: "*.md",
            value: false,
        },
        SectionSpec {
            glob: strict_glob,
            value: true,
        },
    ];
    // Superpowers specs and plans under `docs/wip/` are transient working
    // memory, so the strict tier must not apply to them even when the strict
    // glob covers `docs/**` — unless the strict glob already IS
    // `docs/wip/**.md`, in which case the author asked for that directory to
    // be strict and a separate exemption section would just defeat it (see
    // `scaffold`).
    if strict_glob != DOCS_WIP_GLOB {
        specs.push(SectionSpec {
            glob: DOCS_WIP_GLOB,
            value: false,
        });
    }
    specs.push(SectionSpec {
        glob: "**/SUMMARY.md",
        value: false,
    });

    let lines = split_lines(existing);
    let headers = header_lines(&lines);
    let occurrences_by_spec = specs
        .iter()
        .map(|spec| matching_sections(&lines, &headers, spec.glob))
        .collect::<Vec<_>>();

    let mut actions = Vec::new();
    // Sections that are already present can be out of canonical order too —
    // the per-spec loop below only notices a missing section's placement is
    // ambiguous, so an already-present, wrongly-ordered pair would otherwise
    // sail through every iteration's has_key check with no warning at all.
    let mut warnings = existing_order_conflicts(&specs, &occurrences_by_spec);
    let mut inserts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let added_root = !has_top_level_root(&lines, &headers);

    if added_root {
        actions.push("added top-level root = true".to_string());
    }

    for (index, spec) in specs.iter().enumerate() {
        let occurrences = &occurrences_by_spec[index];
        if occurrences.iter().any(|occurrence| occurrence.has_key) {
            continue;
        }

        if let Some(occurrence) = occurrences.last().copied() {
            push_insert(
                &mut inserts,
                occurrence.insert_at,
                key_line(spec.value),
                existing,
                &lines,
            );
            actions.push(format!(
                "set {MDLINT_STRICT_KEY} = {} in [{}]",
                bool_word(spec.value),
                spec.glob
            ));
        } else {
            let lower = lower_bound(&specs, &occurrences_by_spec, index);
            let upper = upper_bound(&specs, &occurrences_by_spec, index);

            let out_of_order = matches!(
                (&lower, &upper),
                (Some(lower), Some(upper)) if lower.position > upper.position
            );

            if out_of_order {
                // Safe to unwrap: `out_of_order` only matches when both are
                // `Some`.
                let lower = lower.unwrap();
                let upper = upper.unwrap();
                warnings.push(format!(
                    "not adding [{}]: [{}] (line {}) comes after [{}] (line {}) in this \
                     .editorconfig, which contradicts prim's canonical section order; prim will \
                     not reorder sections a person wrote, so add {MDLINT_STRICT_KEY} = {} under \
                     [{}] yourself",
                    spec.glob,
                    lower.glob,
                    lower.line,
                    upper.glob,
                    upper.line,
                    bool_word(spec.value),
                    spec.glob
                ));
                continue;
            }

            let insert_at = upper.map_or(lines.len(), |bound| bound.position);
            push_insert(
                &mut inserts,
                insert_at,
                section_block(spec.glob, spec.value),
                existing,
                &lines,
            );
            actions.push(format!(
                "added [{}] with {MDLINT_STRICT_KEY} = {}",
                spec.glob,
                bool_word(spec.value)
            ));
        }
    }

    let mut contents = String::new();
    if added_root {
        contents.push_str("root = true\n\n");
    }

    for index in 0..=lines.len() {
        if let Some(pending) = inserts.get(&index) {
            for addition in pending {
                contents.push_str(addition);
            }
        }
        if let Some(line) = lines.get(index) {
            contents.push_str(line);
        }
    }

    if actions.is_empty() {
        contents = existing.to_string();
    }

    MergeResult {
        contents,
        actions,
        warnings,
    }
}

mod sections;

#[cfg(test)]
mod tests;
