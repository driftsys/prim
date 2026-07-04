//! Parser for the plain-text spec-test fixture format used by the
//! correctness harness. See `docs/wip/plans/2026-07-02-spec-test-harness-plan.md`
//! for the format grammar.

use prim_fmt::{FileKind, Indent, LineEnding, Style};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One parsed fixture: the style to format under, the input, and the
/// expected formatted output.
#[derive(Debug, Clone)]
pub struct SpecCase {
    /// Used for diagnostics in later tasks (Task 2 onwards).
    #[allow(dead_code)]
    pub name: String,
    pub style: Style,
    pub input: String,
    pub expected: String,
}

fn split_sections(text: &str) -> BTreeMap<String, String> {
    let mut sections = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut buf = String::new();

    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if let Some(marker) = trimmed
            .strip_prefix("-- ")
            .and_then(|s| s.strip_suffix(" --"))
        {
            if let Some(name) = current.take() {
                sections.insert(name, std::mem::take(&mut buf));
            }
            current = Some(marker.to_string());
        } else {
            buf.push_str(line);
        }
    }
    if let Some(name) = current {
        sections.insert(name, buf);
    }
    sections
}

fn parse_style(cfg: &str) -> Style {
    let mut style = Style::default();
    for line in cfg.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once(':')
            .unwrap_or_else(|| panic!("invalid config line: {line:?}"));
        let value = value.trim();
        match key.trim() {
            "max_line_length" => {
                style.max_line_length =
                    Some(value.parse().unwrap_or_else(|_| {
                        panic!("max_line_length must be a number, got {value:?}")
                    }));
            }
            "indent" => {
                style.indent = if value == "tab" {
                    Indent::Tab
                } else {
                    Indent::Spaces(value.parse().unwrap_or_else(|_| {
                        panic!("indent must be 'tab' or a number, got {value:?}")
                    }))
                };
            }
            "end_of_line" => {
                style.end_of_line = match value {
                    "crlf" => LineEnding::CrLf,
                    "lf" => LineEnding::Lf,
                    other => panic!("unknown end_of_line: {other:?}"),
                };
            }
            "trim_trailing_whitespace" => {
                style.trim_trailing_whitespace = value.parse().unwrap_or_else(|_| {
                    panic!("trim_trailing_whitespace must be true/false, got {value:?}")
                });
            }
            "insert_final_newline" => {
                style.insert_final_newline = value.parse().unwrap_or_else(|_| {
                    panic!("insert_final_newline must be true/false, got {value:?}")
                });
            }
            other => panic!("unknown config key: {other:?}"),
        }
    }
    style
}

/// Parse one fixture file's contents into a [`SpecCase`]. `name` is used only
/// for diagnostics (typically the fixture's path).
pub fn parse_spec_file(name: &str, text: &str) -> SpecCase {
    let sections = split_sections(text);
    let style = sections
        .get("config")
        .map(|c| parse_style(c))
        .unwrap_or_default();
    let input = sections
        .get("input")
        .unwrap_or_else(|| panic!("{name}: missing '-- input --' section"))
        .clone();
    let expected = sections
        .get("expected")
        .unwrap_or_else(|| panic!("{name}: missing '-- expected --' section"))
        .clone();
    SpecCase {
        name: name.to_string(),
        style,
        input,
        expected,
    }
}

/// Walk `fixtures_root`, mapping each immediate subdirectory name to a
/// [`FileKind`] and collecting every `*.txt` file inside it. Sorted by path
/// for deterministic test ordering. Used by Task 2 (fixture discovery).
#[allow(dead_code)]
pub fn discover(fixtures_root: &Path) -> Vec<(FileKind, PathBuf)> {
    let mut found = Vec::new();
    let entries = match std::fs::read_dir(fixtures_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return found,
        Err(e) => panic!("cannot read {}: {e}", fixtures_root.display()),
    };
    for entry in entries {
        let dir = entry.expect("readable dir entry");
        if !dir.file_type().expect("file type").is_dir() {
            continue;
        }
        let dir_name = dir.file_name();
        let kind = match dir_name.to_str().expect("utf8 dir name") {
            "json" => FileKind::Json,
            "jsonc" => FileKind::Jsonc,
            "toml" => FileKind::Toml,
            "yaml" => FileKind::Yaml,
            "markdown" => FileKind::Markdown,
            "hygiene" => FileKind::Orphan,
            other => panic!("unknown fixture directory: {other:?}"),
        };
        for file in std::fs::read_dir(dir.path()).expect("readable fixture dir") {
            let file = file.expect("readable entry");
            if file.path().extension().and_then(|e| e.to_str()) == Some("txt") {
                found.push((kind, file.path()));
            }
        }
    }
    found.sort_by(|a, b| a.1.cmp(&b.1));
    found
}

/// Rewrite the `-- expected --` section of the fixture at `path` with
/// `actual`, leaving `config`/`input` untouched. Requires `expected` to be
/// the file's last section (true for every fixture per the format grammar).
pub fn rewrite_expected(path: &Path, actual: &str) {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let marker = "-- expected --\n";
    let idx = text
        .find(marker)
        .unwrap_or_else(|| panic!("{}: missing '-- expected --' marker", path.display()));
    let mut rewritten = text[..idx + marker.len()].to_string();
    rewritten.push_str(actual);
    std::fs::write(path, rewritten)
        .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_input_and_expected_without_config() {
        let case = parse_spec_file("t", "-- input --\nfoo\n-- expected --\nbar\n");
        assert_eq!(case.input, "foo\n");
        assert_eq!(case.expected, "bar\n");
        assert_eq!(case.style, Style::default());
    }

    #[test]
    fn parses_config_overrides() {
        let case = parse_spec_file(
            "t",
            "-- config --\nmax_line_length: 40\nindent: tab\n-- input --\na\n-- expected --\nb\n",
        );
        assert_eq!(case.style.max_line_length, Some(40));
        assert_eq!(case.style.indent, Indent::Tab);
    }

    #[test]
    #[should_panic(expected = "missing '-- input --' section")]
    fn missing_input_section_panics() {
        parse_spec_file("t", "-- expected --\nbar\n");
    }

    #[test]
    fn rewrite_expected_preserves_config_and_input() {
        let dir =
            std::env::temp_dir().join(format!("prim-spec-parser-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("case.txt");
        std::fs::write(
            &path,
            "-- config --\nmax_line_length: 40\n-- input --\nfoo\n-- expected --\nold\n",
        )
        .unwrap();

        rewrite_expected(&path, "new\n");

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "-- config --\nmax_line_length: 40\n-- input --\nfoo\n-- expected --\nnew\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
