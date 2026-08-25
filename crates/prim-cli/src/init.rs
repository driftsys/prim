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

const EDITORCONFIG_NAME: &str = ".editorconfig";
const MDBOOK_NAME: &str = "book.toml";
const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";
const DEFAULT_STRICT_DIR: &str = "docs";
const MDBOOK_DEFAULT_SRC: &str = "src";

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
    /// One entry per canonical section prim could not place because the
    /// file's own section order contradicts the canonical order — prim never
    /// reorders sections a person wrote, so the section is left out rather
    /// than inserted somewhere that would resolve incorrectly.
    warnings: Vec<String>,
}

struct SectionSpec<'a> {
    glob: &'a str,
    value: bool,
}

#[derive(Clone, Copy)]
struct SectionOccurrence {
    header_line: usize,
    insert_at: usize,
    has_key: bool,
}

/// One end of the range a missing section's insertion point must fall in,
/// established by an existing occurrence of some other canonical spec.
/// `line` is 1-indexed, for warning text.
struct Bound<'a> {
    position: usize,
    glob: &'a str,
    line: usize,
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
        return Ok(Outcome {
            message: format!(
                "created {} with Markdown strict-glob map ([*.md] → [{strict_glob}] → [docs/wip/**.md] → [**/SUMMARY.md])",
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
        return Ok(Outcome {
            message: format!(
                "{} already contains the Markdown strict-glob map",
                editorconfig.display()
            ),
        });
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
    format!(
        "root = true\n[*.md]\n{MDLINT_STRICT_KEY} = false\n[{strict_glob}]\n{MDLINT_STRICT_KEY} = true\n[docs/wip/**.md]\n{MDLINT_STRICT_KEY} = false\n[**/SUMMARY.md]\n{MDLINT_STRICT_KEY} = false\n"
    )
}

fn merge(existing: &str, strict_glob: &str) -> MergeResult {
    let specs = [
        SectionSpec {
            glob: "*.md",
            value: false,
        },
        SectionSpec {
            glob: strict_glob,
            value: true,
        },
        // Superpowers specs and plans under `docs/wip/` are transient working
        // memory, so the strict tier must not apply to them even when the
        // strict glob covers `docs/**`.
        SectionSpec {
            glob: "docs/wip/**.md",
            value: false,
        },
        SectionSpec {
            glob: "**/SUMMARY.md",
            value: false,
        },
    ];

    let lines = split_lines(existing);
    let headers = header_lines(&lines);
    let mut actions = Vec::new();
    let mut warnings = Vec::new();
    let mut inserts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let occurrences_by_spec = specs
        .iter()
        .map(|spec| matching_sections(&lines, &headers, spec.glob))
        .collect::<Vec<_>>();
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

/// The latest point any already-present, canonically earlier spec's section
/// ends at, if one exists in `existing` — every section that must precede the
/// spec at `index`.
fn lower_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[..index]
        .iter()
        .zip(&occurrences_by_spec[..index])
        .filter_map(|(spec, occurrences)| {
            occurrences.last().map(|occurrence| Bound {
                position: occurrence.insert_at,
                glob: spec.glob,
                line: occurrence.header_line + 1,
            })
        })
        .max_by_key(|bound| bound.position)
}

/// The earliest point any already-present, canonically later spec's section
/// starts at, if one exists in `existing` — every section that must follow
/// the spec at `index`.
fn upper_bound<'a>(
    specs: &[SectionSpec<'a>],
    occurrences_by_spec: &[Vec<SectionOccurrence>],
    index: usize,
) -> Option<Bound<'a>> {
    specs[index + 1..]
        .iter()
        .zip(&occurrences_by_spec[index + 1..])
        .filter_map(|(spec, occurrences)| {
            occurrences.first().map(|occurrence| Bound {
                position: occurrence.header_line,
                glob: spec.glob,
                line: occurrence.header_line + 1,
            })
        })
        .min_by_key(|bound| bound.position)
}

fn split_lines(content: &str) -> Vec<&str> {
    if content.is_empty() {
        Vec::new()
    } else {
        content.split_inclusive('\n').collect()
    }
}

fn header_lines(lines: &[&str]) -> Vec<(usize, String)> {
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| parse_header(line).map(|glob| (index, glob.to_string())))
        .collect()
}

fn has_top_level_root(lines: &[&str], headers: &[(usize, String)]) -> bool {
    let first_section = headers.first().map_or(lines.len(), |(index, _)| *index);
    lines
        .iter()
        .take(first_section)
        .filter_map(|line| parse_key(line))
        .any(|key| key.eq_ignore_ascii_case("root"))
}

fn matching_sections(
    lines: &[&str],
    headers: &[(usize, String)],
    glob: &str,
) -> Vec<SectionOccurrence> {
    headers
        .iter()
        .enumerate()
        .filter(|(_, (_, header_glob))| header_glob == glob)
        .map(|(header_pos, (line_index, _))| {
            let next_header = headers
                .get(header_pos + 1)
                .map_or(lines.len(), |(next_index, _)| *next_index);
            let has_key = lines[*line_index + 1..next_header]
                .iter()
                .filter_map(|line| parse_key(line))
                .any(|key| key.eq_ignore_ascii_case(MDLINT_STRICT_KEY));
            SectionOccurrence {
                header_line: *line_index,
                insert_at: next_header,
                has_key,
            }
        })
        .collect()
}

fn push_insert(
    inserts: &mut BTreeMap<usize, Vec<String>>,
    index: usize,
    mut addition: String,
    existing: &str,
    lines: &[&str],
) {
    let entry = inserts.entry(index).or_default();
    if index == lines.len() && !existing.is_empty() && !existing.ends_with('\n') && entry.is_empty()
    {
        addition.insert(0, '\n');
    }
    entry.push(addition);
}

fn parse_header(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    trimmed
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

fn parse_key(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    trimmed.split_once('=').map(|(key, _)| key.trim())
}

fn section_block(glob: &str, value: bool) -> String {
    format!("[{glob}]\n{}", key_line(value))
}

fn key_line(value: bool) -> String {
    format!("{MDLINT_STRICT_KEY} = {}\n", bool_word(value))
}

fn bool_word(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

#[cfg(test)]
mod tests;
