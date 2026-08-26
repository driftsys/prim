# Spec-Test Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the 6-entry hardcoded `CORPUS` array in
`crates/prim-fmt/tests/correctness.rs` with a file-driven spec-test harness
(dprint-inspired plain-text golden files) that (a) checks formatted output
against an expected golden, and (b) automatically runs the existing FR-6.1
idempotency and FR-6.2 semantic-preservation checks over every fixture — so
growing coverage is "add a `.txt` file," not "edit a Rust array."

**Architecture:** A small parser module reads `-- config --` / `-- input --` /
`-- expected --` sections from plain-text fixture files under
`crates/prim-fmt/tests/correctness/fixtures/<format>/*.txt`. The directory name
maps to a `FileKind`. A discovery function walks the tree; `correctness.rs` runs
four generalized tests (format-equality, idempotency, JSON/TOML/YAML data-model
preservation) over every discovered case. An opt-in `PRIM_SPEC_UPDATE=1` env var
regenerates `expected` sections in place, so new fixtures are bootstrapped by
running the formatter, not hand-computed.

**Tech Stack:** Rust integration tests (`crates/prim-fmt/tests/`), existing
dev-dependencies only (`jsonc-parser`, `toml`, `yaml-rust2`) — no new crates.

## Global Constraints

- Zero warnings: `cargo test`, `clippy` must stay clean (AGENTS.md
  "Conventions").
- `rustfmt` formatting on all new Rust files; run `just fmt` before committing.
- Conventional Commits, imperative mood (`feat`, `test`, etc.).
- Single PR ships implementation + tests + docs together (AGENTS.md "Workflow").
- `prim-fmt` stays free of clap/CLI/terminal dependencies — this plan only
  touches `tests/`, never `src/`.
- Run `just verify` before considering the branch done.

---

## Spec File Format (reference for all tasks)

```text
-- config --
max_line_length: 40
-- input --
#  Heading
-- expected --
# Heading
```

- `-- config --` is optional. Omit it (and the whole block) to use
  `Style::default()`.
- `-- input --` and `-- expected --` are required, in that order, with
  `expected` always last (the update-mode rewriter depends on this).
- Recognized config keys: `max_line_length` (number), `indent` (`tab` or a
  number of spaces), `end_of_line` (`lf`/`crlf`), `trim_trailing_whitespace`
  (`true`/`false`), `insert_final_newline` (`true`/`false`).
- A fixture's _directory_ (`json/`, `jsonc/`, `toml/`, `yaml/`, `markdown/`,
  `hygiene/`) selects its `FileKind` (`hygiene/` → `FileKind::Orphan`).

---

### Task 1: Spec-file parser

**Files:**

- Create: `crates/prim-fmt/tests/correctness/spec_parser.rs`
- Modify: `crates/prim-fmt/tests/correctness.rs:1` (add `mod` declaration —
  content replaced fully in Task 3)

**Interfaces:**

- Produces:
  `pub struct SpecCase { pub name: String, pub style: prim_fmt::Style, pub input: String, pub expected: String }`,
  `pub fn parse_spec_file(name: &str, text: &str) -> SpecCase`,
  `pub fn discover(fixtures_root: &std::path::Path) -> Vec<(prim_fmt::FileKind, std::path::PathBuf)>`,
  `pub fn rewrite_expected(path: &std::path::Path, actual: &str)`.
- Consumes: `prim_fmt::{FileKind, Style, Indent, LineEnding}` (all public, see
  `crates/prim-fmt/src/style.rs` and `crates/prim-fmt/src/classify.rs`).

- [ ] **Step 1: Write the failing tests for section splitting and style
      parsing**

Create `crates/prim-fmt/tests/correctness/spec_parser.rs`:

```rust
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
                    Indent::Spaces(
                        value
                            .parse()
                            .unwrap_or_else(|_| panic!("indent must be 'tab' or a number, got {value:?}")),
                    )
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
                style.trim_trailing_whitespace = value
                    .parse()
                    .unwrap_or_else(|_| panic!("trim_trailing_whitespace must be true/false, got {value:?}"));
            }
            "insert_final_newline" => {
                style.insert_final_newline = value
                    .parse()
                    .unwrap_or_else(|_| panic!("insert_final_newline must be true/false, got {value:?}"));
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
    let style = sections.get("config").map(|c| parse_style(c)).unwrap_or_default();
    let input = sections
        .get("input")
        .unwrap_or_else(|| panic!("{name}: missing '-- input --' section"))
        .clone();
    let expected = sections
        .get("expected")
        .unwrap_or_else(|| panic!("{name}: missing '-- expected --' section"))
        .clone();
    SpecCase { name: name.to_string(), style, input, expected }
}

/// Walk `fixtures_root`, mapping each immediate subdirectory name to a
/// [`FileKind`] and collecting every `*.txt` file inside it. Sorted by path
/// for deterministic test ordering.
pub fn discover(fixtures_root: &Path) -> Vec<(FileKind, PathBuf)> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(fixtures_root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", fixtures_root.display()))
    {
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
        let dir = std::env::temp_dir().join(format!(
            "prim-spec-parser-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("case.txt");
        std::fs::write(&path, "-- config --\nmax_line_length: 40\n-- input --\nfoo\n-- expected --\nold\n").unwrap();

        rewrite_expected(&path, "new\n");

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            "-- config --\nmax_line_length: 40\n-- input --\nfoo\n-- expected --\nnew\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
```

- [ ] **Step 2: Run the new unit tests to verify they pass**

Run: `cargo test -p prim-fmt --test correctness spec_parser -- --nocapture`

This will fail to compile until Task 2 wires `mod spec_parser;` into
`correctness.rs` — for this step, temporarily add just the `mod` line so the
module compiles standalone:

In `crates/prim-fmt/tests/correctness.rs`, prepend:

```rust
mod spec_parser;
```

Expected: all 4 tests in `spec_parser::tests` PASS. (The pre-existing
`correctness.rs` tests below the new `mod` line still reference the old `CORPUS`
— leave them as-is for now; Task 3 replaces them.)

- [ ] **Step 3: Commit**

```bash
git add crates/prim-fmt/tests/correctness/spec_parser.rs crates/prim-fmt/tests/correctness.rs
git commit -m "test(fmt): add spec-file parser for fixture-driven correctness tests"
```

---

### Task 2: `PRIM_SPEC_UPDATE` bootstrap — proven via a throwaway fixture

**Files:**

- Create: `crates/prim-fmt/tests/correctness/fixtures/json/scratch.txt`
  (temporary, deleted at the end of this task)

**Interfaces:**

- Consumes: `spec_parser::{discover, parse_spec_file, rewrite_expected}` from
  Task 1.

This task proves the discover → format → rewrite loop end-to-end before Task 3
wires it into real assertions.

- [ ] **Step 1: Create a throwaway fixture with an empty expected section**

Create `crates/prim-fmt/tests/correctness/fixtures/json/scratch.txt`:

```text
-- input --
{"a":1,"b":2}
-- expected --
```

- [ ] **Step 2: Write a scratch test that exercises discover + parse + rewrite**

Temporarily append to `crates/prim-fmt/tests/correctness/spec_parser.rs`'s
`#[cfg(test)] mod tests` block:

```rust
    #[test]
    fn scratch_discover_and_rewrite_roundtrip() {
        let root = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/correctness/fixtures"
        ));
        let found = discover(root);
        let (kind, path) = found
            .iter()
            .find(|(_, p)| p.ends_with("scratch.txt"))
            .expect("scratch fixture discovered");
        assert_eq!(*kind, FileKind::Json);

        let text = std::fs::read_to_string(path).unwrap();
        let case = parse_spec_file("scratch", &text);
        let actual = prim_fmt::format(*kind, &case.input, &case.style).unwrap();
        rewrite_expected(path, &actual);

        let reparsed = parse_spec_file("scratch", &std::fs::read_to_string(path).unwrap());
        assert_eq!(reparsed.expected, actual);
    }
```

- [ ] **Step 3: Run it**

Run:
`cargo test -p prim-fmt --test correctness scratch_discover_and_rewrite_roundtrip -- --nocapture`
Expected: PASS. Inspect
`crates/prim-fmt/tests/correctness/fixtures/json/scratch.txt` — its
`-- expected --` section now contains prim's formatted output for
`{"a":1,"b":2}`.

- [ ] **Step 4: Remove the scratch test and scratch fixture (this task was a
      proof, not a permanent fixture)**

```bash
git checkout -- crates/prim-fmt/tests/correctness/spec_parser.rs 2>/dev/null || true
rm crates/prim-fmt/tests/correctness/fixtures/json/scratch.txt
```

(If `spec_parser.rs` was already committed in Task 1 without the scratch test,
`git checkout --` cleanly removes it; otherwise manually delete the
`scratch_discover_and_rewrite_roundtrip` function.)

- [ ] **Step 5: Confirm clean state**

Run: `git status --porcelain crates/prim-fmt/tests/correctness/` Expected: no
output (scratch fixture removed, `spec_parser.rs` back to Task 1's committed
state).

No commit for this task — it leaves no permanent changes.

---

### Task 3: Rewrite `correctness.rs` to run generalized spec-driven tests

**Files:**

- Modify: `crates/prim-fmt/tests/correctness.rs` (full replacement of body,
  `mod spec_parser;` stays)

**Interfaces:**

- Consumes:
  `spec_parser::{SpecCase, discover, parse_spec_file, rewrite_expected}` (Task
  1), `prim_fmt::{FileKind, Style, format}`.
- Produces: four `#[test]` functions that any later fixture (Tasks 5–6) is
  automatically covered by.

- [ ] **Step 1: Write the failing test — replace `correctness.rs` wholesale**

At this point there are zero fixtures under
`crates/prim-fmt/tests/correctness/fixtures/` (Task 2's scratch fixture was
removed), so `load_cases()` returns an empty `Vec` and every test below
trivially passes on an empty corpus — that's expected; Task 5 repopulates it.

Replace `crates/prim-fmt/tests/correctness.rs` entirely with:

```rust
//! Cross-cutting correctness harness (FR-6.1 idempotency, FR-6.2 semantic
//! preservation) plus format-equality assertions, all driven from the
//! plain-text fixtures under `tests/correctness/fixtures/`. Adding coverage
//! means adding a `.txt` fixture — see
//! `docs/wip/plans/2026-07-02-spec-test-harness-plan.md` for the format.

mod spec_parser;

use prim_fmt::{FileKind, format};
use spec_parser::{SpecCase, discover, parse_spec_file, rewrite_expected};
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/correctness/fixtures")).to_path_buf()
}

fn load_cases() -> Vec<(FileKind, SpecCase)> {
    discover(&fixtures_root())
        .into_iter()
        .map(|(kind, path)| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let name = path.display().to_string();
            (kind, parse_spec_file(&name, &text))
        })
        .collect()
}

#[test]
fn spec_cases_format_as_expected() {
    let update = std::env::var_os("PRIM_SPEC_UPDATE").is_some();
    let mut failures = Vec::new();
    for (kind, case) in load_cases() {
        let actual = format(kind, &case.input, &case.style).expect("formats");
        if actual != case.expected {
            if update {
                rewrite_expected(Path::new(&case.name), &actual);
            } else {
                failures.push(case.name.clone());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "spec cases produced unexpected output: {failures:?}\n\
         run `PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness \
         spec_cases_format_as_expected` to regenerate, then review the diff \
         before committing"
    );
}

#[test]
fn spec_cases_are_idempotent() {
    for (kind, case) in load_cases() {
        let once = format(kind, &case.input, &case.style).expect("formats");
        let twice = format(kind, &once, &case.style).expect("formats");
        assert_eq!(once, twice, "not idempotent: {}", case.name);
    }
}

fn json_value(text: &str) -> serde_json::Value {
    jsonc_parser::parse_to_serde_value::<serde_json::Value>(
        text,
        &jsonc_parser::ParseOptions::default(),
    )
    .expect("parses")
}

#[test]
fn spec_cases_preserve_json_data_model() {
    for (kind, case) in load_cases() {
        if !matches!(kind, FileKind::Json | FileKind::Jsonc) {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        assert_eq!(
            json_value(&case.input),
            json_value(&actual),
            "JSON data model changed: {}",
            case.name
        );
    }
}

#[test]
fn spec_cases_preserve_toml_data_model() {
    for (kind, case) in load_cases() {
        if kind != FileKind::Toml {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        let before: toml::Table = case.input.parse().expect("parses");
        let after: toml::Table = actual.parse().expect("parses");
        assert_eq!(before, after, "TOML data model changed: {}", case.name);
    }
}

#[test]
fn spec_cases_preserve_yaml_data_model() {
    use yaml_rust2::YamlLoader;
    for (kind, case) in load_cases() {
        if kind != FileKind::Yaml {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        let before = YamlLoader::load_from_str(&case.input).expect("parses");
        let after = YamlLoader::load_from_str(&actual).expect("parses");
        assert_eq!(before, after, "YAML data model changed: {}", case.name);
    }
}
```

- [ ] **Step 2: Run it to verify it passes on an empty corpus**

Run: `cargo test -p prim-fmt --test correctness` Expected: PASS — 5 tests run
(`spec_parser::tests::*` plus the 5 new top-level tests), all green, since
`load_cases()` is empty until Task 5.

- [ ] **Step 3: Commit**

```bash
git add crates/prim-fmt/tests/correctness.rs
git commit -m "test(fmt): drive correctness harness from spec-file fixtures"
```

---

### Task 4: Migrate the 6 existing `CORPUS` entries to fixtures

**Files:**

- Create: `crates/prim-fmt/tests/correctness/fixtures/json/basic_object.txt`
- Create: `crates/prim-fmt/tests/correctness/fixtures/json/empty_array.txt`
- Create:
  `crates/prim-fmt/tests/correctness/fixtures/jsonc/trailing_comma_comment.txt`
- Create: `crates/prim-fmt/tests/correctness/fixtures/toml/mixed_syntax.txt`
- Create:
  `crates/prim-fmt/tests/correctness/fixtures/yaml/anchors_and_block_scalar.txt`
- Create:
  `crates/prim-fmt/tests/correctness/fixtures/markdown/heading_and_long_prose.txt`
- Create:
  `crates/prim-fmt/tests/correctness/fixtures/hygiene/trailing_whitespace_and_blank_lines.txt`

These are the exact 6 `CORPUS` inputs from the pre-Task-3 `correctness.rs`, one
per file, each with an empty `-- expected --` section to be populated by update
mode.

- [ ] **Step 1: Create each fixture with its known input and an empty expected
      section**

`crates/prim-fmt/tests/correctness/fixtures/json/basic_object.txt`:

```text
-- input --
{"a":1,"b":[1,2,3],"c":{"d":true}}
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/json/empty_array.txt`:

```text
-- input --
[]
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/jsonc/trailing_comma_comment.txt`:

```text
-- input --
{
// a comment
"a": 1,
"b": 2,
}
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/toml/mixed_syntax.txt`:

```text
-- input --
a=1
b = "x"
[t]
c=[1,2]
d = {e=1}
# comment
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/yaml/anchors_and_block_scalar.txt`:

```text
-- input --
a: 1
b:
  - 1
  - 2
base: &id 1
ref: *id
block: |
  l1
  l2
# comment
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/markdown/heading_and_long_prose.txt`:

```text
-- input --
#  Heading

Some   prose with `inline code` that runs on and on and on well past the wrap.

- one
- two
-- expected --
```

`crates/prim-fmt/tests/correctness/fixtures/hygiene/trailing_whitespace_and_blank_lines.txt`:

```text
-- input --
trailing
lines


-- expected --
```

(Note: the `trailing` line above has two literal trailing spaces — preserve them
exactly, they're the point of the fixture.)

- [ ] **Step 2: Populate expected sections via update mode**

Run:
`PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness spec_cases_format_as_expected`
Expected: PASS (update mode never fails the assertion — it rewrites and moves
on). Inspect the diff:

Run: `git diff crates/prim-fmt/tests/correctness/fixtures/` Expected: every
fixture's `-- expected --` section is now populated. Read each one — this is the
human review step confirming prim's actual output looks right (it mirrors the
old `CORPUS`-based idempotency test's implicit trust, now made explicit and
inspectable).

- [ ] **Step 3: Run the full harness normally to confirm green**

Run: `cargo test -p prim-fmt --test correctness` Expected: PASS — all 5
top-level tests, now exercising 7 fixtures (6 migrated + the harness's own unit
tests still separate).

- [ ] **Step 4: Commit**

```bash
git add crates/prim-fmt/tests/correctness/fixtures/
git commit -m "test(fmt): migrate hardcoded CORPUS entries to spec-file fixtures"
```

---

### Task 5: Add edge-case fixtures per format (the "extensive" part)

**Files:**

- Create 5 new fixtures under `crates/prim-fmt/tests/correctness/fixtures/json/`
- Create 3 new fixtures under `crates/prim-fmt/tests/correctness/fixtures/toml/`
- Create 4 new fixtures under `crates/prim-fmt/tests/correctness/fixtures/yaml/`
- Create 4 new fixtures under
  `crates/prim-fmt/tests/correctness/fixtures/markdown/`
- Create 3 new fixtures under
  `crates/prim-fmt/tests/correctness/fixtures/hygiene/`

Same bootstrap technique as Task 4 (write input + empty expected, run update
mode, review diff). Content for each new fixture's `-- input --` section:

`fixtures/json/nested_unicode.txt`:

```text
-- input --
{"name":"café","emoji":"🎉","nested":{"deep":{"deeper":[1,2,[3,4,{"x":null}]]}}}
-- expected --
```

`fixtures/json/numbers.txt`:

```text
-- input --
{"pi":3.14159,"neg":-42,"exp":1.5e10,"negexp":-1.5e-10,"zero":-0}
-- expected --
```

`fixtures/json/empty_variants.txt`:

```text
-- input --
{"a":{},"b":[],"c":[[],[]],"d":{"e":{}}}
-- expected --
```

`fixtures/json/long_string_values.txt`:

```text
-- input --
{"description":"This is a fairly long string value that exists to check how the formatter treats long scalar values inside JSON objects without wrapping them, since JSON strings are not prose."}
-- expected --
```

`fixtures/jsonc/block_comment.txt`:

```text
-- input --
{
  /* block
     comment */
  "a": 1 /* inline */, "b": 2
}
-- expected --
```

`fixtures/toml/array_of_tables.txt`:

```text
-- input --
[[fruit]]
name = "apple"
[fruit.physical]
color = "red"
[[fruit]]
name = "banana"
-- expected --
```

`fixtures/toml/dotted_keys_and_datetime.txt`:

```text
-- input --
physical.color = "orange"
physical.shape = "round"
created = 1987-07-05T17:45:00Z
-- expected --
```

`fixtures/toml/multiline_strings.txt`:

```text
-- input --
description = """
line one
line two
"""
literal = '''
raw \n text
'''
-- expected --
```

`fixtures/yaml/flow_collections.txt`:

```text
-- input --
inline_map: {a: 1, b: 2}
inline_seq: [1, 2, 3]
mixed: {list: [1, 2], nested: {x: 1}}
-- expected --
```

`fixtures/yaml/merge_key.txt`:

```text
-- input --
defaults: &defaults
  adapter: postgres
  host: localhost
development:
  <<: *defaults
  database: dev_db
-- expected --
```

`fixtures/yaml/folded_scalar.txt`:

```text
-- input --
summary: >
  This is a folded
  block scalar that
  wraps onto one line.
literal: |-
  no trailing newline
  on this one
-- expected --
```

`fixtures/yaml/quoted_special_keys.txt`:

```text
-- input --
"key: with colon": 1
'key with spaces': 2
normal_key: 3
-- expected --
```

`fixtures/markdown/table.txt`:

```text
-- input --
| Col A | Col B |
|---|---|
| 1 | 2 |
| longer value | x |
-- expected --
```

`fixtures/markdown/nested_lists.txt`:

```text
-- input --
1. first
   - nested bullet
   - another
2. second
   1. nested ordered
-- expected --
```

`fixtures/markdown/code_fence_with_language.txt`:

````text
-- input --
Some text.

```rust
fn main() {
    println!("hi");
}
````

More text. -- expected --

````
`fixtures/markdown/links_and_blockquote.txt`:
```text
-- input --
> A blockquote with a [reference link][ref] inside it.
>
> - and a nested list

[ref]: https://example.com "Example"
-- expected --
````

`fixtures/hygiene/crlf_mixed_with_trailing_ws.txt`:

```text
-- input --
line one  \r
line two\r
line three   \r
-- expected --
```

`fixtures/hygiene/multiple_trailing_blank_lines.txt`:

```text
-- input --
content


-- expected --
```

`fixtures/hygiene/no_final_newline.txt`:

```text
-- input --
content with no trailing newline
-- expected --
```

(Note: `fixtures/hygiene/no_final_newline.txt` — the input line itself has no
newline before the `-- expected --` marker. When creating this file, do not let
your editor auto-append a trailing newline to the input section; if it does,
this fixture degenerates into a duplicate of
`multiple_trailing_blank_lines.txt`'s opposite case and should be adjusted by
hand to strip it.)

- [ ] **Step 1: Create all 19 fixture files above** (5 json/jsonc, 3 toml, 4
      yaml, 4 markdown, 3 hygiene)

- [ ] **Step 2: Populate expected sections via update mode**

Run:
`PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness spec_cases_format_as_expected`

- [ ] **Step 3: Review every generated diff**

Run: `git diff crates/prim-fmt/tests/correctness/fixtures/` Expected: read each
new fixture's populated `-- expected --` section. This is the point where you'd
catch a formatter bug — if any output looks wrong (e.g. a TOML array-of-tables
re-ordered, a YAML merge key mis-rendered), stop and file it as a bug rather
than accepting the diff blindly.

- [ ] **Step 4: Run the full harness**

Run: `cargo test -p prim-fmt --test correctness` Expected: PASS — all fixtures
(7 migrated + 19 new = 26) pass format-equality, idempotency, and the relevant
data-model-preservation check.

- [ ] **Step 5: Commit**

```bash
git add crates/prim-fmt/tests/correctness/fixtures/
git commit -m "test(fmt): add edge-case spec fixtures for json/toml/yaml/markdown/hygiene"
```

---

### Task 6: Document the fixture format for contributors

**Files:**

- Modify: `crates/prim-fmt/README.md`

- [ ] **Step 1: Add a "Correctness fixtures" section**

Append to `crates/prim-fmt/README.md`:

````markdown
## Correctness fixtures

`tests/correctness/fixtures/<format>/*.txt` drive the correctness harness
(FR-6.1 idempotency, FR-6.2 semantic preservation, plus format-equality).
Each file has `-- input --` and `-- expected --` sections, plus an optional
`-- config --` section overriding the default `Style`. The directory name
selects the `FileKind` (`json`, `jsonc`, `toml`, `yaml`, `markdown`,
`hygiene`).

To add a fixture: create the file with your `-- input --` and an empty
`-- expected --`, then run:

```bash
PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness spec_cases_format_as_expected
````

Review the generated diff before committing — this is the step where a formatter
bug would show up as unexpected output.

````
- [ ] **Step 2: Verify the doc renders correctly**

Run: `prim --check crates/prim-fmt/README.md` (or `cargo run -q -p prim-cli -- --check crates/prim-fmt/README.md` if `prim` isn't installed)
Expected: exit 0 (no formatting changes needed) — if it flags changes, run `prim crates/prim-fmt/README.md` to fix, then re-check.

- [ ] **Step 3: Run full verification and commit**

Run: `just verify`
Expected: PASS.

```bash
git add crates/prim-fmt/README.md
git commit -m "docs(fmt): document the spec-test fixture format"
````
