//! Scaffold or minimally merge prim's Markdown strict-glob placement map into
//! `.editorconfig` (story G4).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use toml::Value;

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
                "created {} with Markdown strict-glob map ([*.md] → [{strict_glob}] → [**/SUMMARY.md])",
                editorconfig.display()
            ),
        });
    }

    let existing = fs::read_to_string(&editorconfig).map_err(|source| Error::ReadEditorConfig {
        path: editorconfig.clone(),
        source,
    })?;
    let merged = merge(&existing, &strict_glob);

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
        "root = true\n[*.md]\n{MDLINT_STRICT_KEY} = false\n[{strict_glob}]\n{MDLINT_STRICT_KEY} = true\n[**/SUMMARY.md]\n{MDLINT_STRICT_KEY} = false\n"
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
        SectionSpec {
            glob: "**/SUMMARY.md",
            value: false,
        },
    ];

    let lines = split_lines(existing);
    let headers = header_lines(&lines);
    let mut actions = Vec::new();
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
            let insert_at = occurrences_by_spec[index + 1..]
                .iter()
                .filter_map(|occurrences| {
                    occurrences.first().map(|occurrence| occurrence.header_line)
                })
                .min()
                .unwrap_or(lines.len());
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

    MergeResult { contents, actions }
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
mod tests {
    use super::*;

    #[test]
    fn scaffold_matches_the_default_contract() {
        assert_eq!(
            scaffold("docs/**.md"),
            "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
        );
    }

    #[test]
    fn merge_prepends_root_and_appends_missing_sections_without_reordering_existing_content() {
        let existing = "[*]\nindent_style = space\nindent_size = 2\n";
        let merged = merge(existing, "docs/**.md");

        assert_eq!(
            merged.contents,
            "root = true\n\n[*]\nindent_style = space\nindent_size = 2\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
        );
        assert_eq!(
            merged.actions,
            vec![
                "added top-level root = true",
                "added [*.md] with prim_mdlint_strict = false",
                "added [docs/**.md] with prim_mdlint_strict = true",
                "added [**/SUMMARY.md] with prim_mdlint_strict = false",
            ]
        );
    }

    #[test]
    fn merge_adds_the_missing_key_in_place_for_an_existing_section() {
        let existing =
            "root = true\n[*.md]\nmax_line_length = 100\n[*.txt]\nindent_style = space\n";
        let merged = merge(existing, "docs/**.md");

        assert_eq!(
            merged.contents,
            "root = true\n[*.md]\nmax_line_length = 100\nprim_mdlint_strict = false\n[*.txt]\nindent_style = space\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
        );
    }

    #[test]
    fn merge_inserts_a_missing_floor_before_an_existing_strict_section() {
        let existing = "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n";
        let merged = merge(existing, "docs/**.md");

        assert_eq!(
            merged.contents,
            "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
        );
    }

    #[test]
    fn merge_leaves_an_existing_explicit_choice_untouched() {
        let existing =
            "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\n[**/SUMMARY.md]\n";
        let merged = merge(existing, "docs/**.md");

        assert_eq!(
            merged.contents,
            "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
        );
    }

    #[test]
    fn book_toml_custom_src_changes_the_strict_glob() {
        assert_eq!(
            strict_glob_from_book_toml("[book]\nsrc = \"guide\"\n"),
            "guide/**.md"
        );
    }

    #[test]
    fn book_toml_src_is_normalized_before_becoming_a_glob() {
        assert_eq!(
            strict_glob_from_book_toml("[book]\nsrc = \"./guide/\"\n"),
            "guide/**.md"
        );
    }

    #[test]
    fn book_toml_without_src_defaults_to_src_directory() {
        assert_eq!(
            strict_glob_from_book_toml("[book]\ntitle = \"prim\"\n"),
            "src/**.md"
        );
    }

    #[test]
    fn malformed_book_toml_also_defaults_to_src_directory() {
        assert_eq!(strict_glob_from_book_toml("[book]\nsrc =\n"), "src/**.md");
    }

    #[test]
    fn running_twice_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();

        let first = run(dir.path()).unwrap();
        let once = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();
        let second = run(dir.path()).unwrap();
        let twice = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();

        assert!(first.message.contains("created"));
        assert!(second.message.contains("already contains"));
        assert_eq!(once, twice);
    }

    #[test]
    fn non_utf8_editorconfig_is_reported_and_left_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".editorconfig");
        fs::write(&path, [0xFFu8, 0xFE, 0x00]).unwrap();

        let err = run(dir.path()).unwrap_err();

        assert!(matches!(err, Error::ReadEditorConfig { .. }));
        assert_eq!(fs::read(&path).unwrap(), [0xFFu8, 0xFE, 0x00]);
    }
}
