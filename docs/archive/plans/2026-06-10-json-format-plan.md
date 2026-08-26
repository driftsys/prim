# JSON + JSONC Formatter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Canonical, comment-preserving JSON/JSONC formatting (FR-1.2/1.3),
dispatched from `prim_fmt::format`, consuming #8's `Style`.

**Architecture:** A `json` module in `prim-fmt` calls `dprint-plugin-json`
(pure, no I/O) to print canonical JSONC, then reuses `hygiene` for EOL +
final-newline. The engine's `format` becomes fallible
(`Result<String, FormatError>`) so the CLI leaves unparseable files unchanged
and reports them.

**Tech Stack:** Rust, `dprint-plugin-json = "0.21"`, `thiserror`,
`assert_cmd`/`tempfile` (tests).

**Invariant:** every commit compiles and `cargo test --workspace` is green.
Hygiene-only kinds keep identical output.

**Verified `dprint-plugin-json` 0.21 API** (confirmed against source at tag
`0.21.3`):

- `dprint_plugin_json::format_text(path: &Path, text: &str, config: &Configuration) -> anyhow::Result<Option<String>>`
  — `Ok(Some)` reformatted, `Ok(None)` already canonical, `Err` parse failure.
- `dprint_plugin_json::configuration::ConfigurationBuilder::new()`; methods
  `.line_width(u32)`, `.use_tabs(bool)`, `.indent_width(u8)`,
  `.trailing_commas(TrailingCommaKind)`, `.build() -> Configuration`.
- `TrailingCommaKind::{Jsonc, Never, Always}` (re-exported from
  `dprint_plugin_json::configuration`).
- EOL is owned by `hygiene` (not dprint), so `new_line_kind` is intentionally
  not set.

---

## Task 1: Make `format` fallible (`FormatError`), no behaviour change

**Files:**

- Create: `crates/prim-fmt/src/error.rs`
- Modify: `crates/prim-fmt/src/lib.rs`, `crates/prim-fmt/Cargo.toml`
- Modify: `crates/prim-cli/src/app.rs` (handle `Result` at both call sites)

- [ ] **Step 1: Add `thiserror`** to `crates/prim-fmt/Cargo.toml`. Add a
      `[dependencies]` section if absent:

```toml
[dependencies]
thiserror = "2"
```

- [ ] **Step 2: Create `crates/prim-fmt/src/error.rs`**:

```rust
//! Engine error types — part of the public contract.

/// An error returned by [`crate::format`] when a source cannot be formatted.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// The source could not be parsed as its format. The string carries the
    /// underlying parser's message (including location, when available).
    #[error("{0}")]
    Parse(String),
}
```

- [ ] **Step 3: Update `lib.rs`** — declare the module, re-export, and make
      `format` fallible. Hygiene kinds wrap in `Ok`; `Json`/`Jsonc` temporarily
      stay on hygiene (real printer lands in Task 2):

```rust
mod classify;
mod error;
mod hygiene;
mod style;

pub use classify::{FileKind, classify};
pub use error::FormatError;
pub use style::{Indent, LineEnding, Style};
```

and replace the `format` fn:

```rust
/// Format `source` as the given [`FileKind`] under `style`.
///
/// Returns [`FormatError`] when a structured format cannot be parsed; the CLI
/// then leaves the file unchanged and reports it (FR-6.3).
pub fn format(kind: FileKind, source: &str, style: &Style) -> Result<String, FormatError> {
    match kind {
        FileKind::Markdown
        | FileKind::Json
        | FileKind::Jsonc
        | FileKind::Yaml
        | FileKind::Toml
        | FileKind::Orphan => Ok(hygiene::hygiene(source, style)),
    }
}
```

- [ ] **Step 4: Update `app.rs` `run_paths`** — replace the
      `let formatted = prim_fmt::format(kind, &original, &style);` line with
      `Result` handling that mirrors the existing non-UTF-8 branch:

```rust
let formatted = match prim_fmt::format(kind, &original, &style) {
    Ok(text) => text,
    Err(err) => {
        // An owned file prim cannot parse is left unchanged and reported
        // (FR-6.3): an error for an explicitly named file (exit 2), a
        // warning for a discovered one.
        let message = format!("{}: {err}", file.path.display());
        if file.explicit {
            ui::error(&message);
            had_error = true;
        } else {
            ui::warning(&message);
        }
        continue;
    }
};
```

- [ ] **Step 5: Update `app.rs` `run_stdin`** — replace the
      `Some(kind) => { … }` arm:

```rust
Some(kind) => {
    let style = editorconfig::resolve(path);
    match prim_fmt::format(kind, &input, &style) {
        Ok(text) => print!("{text}"),
        Err(err) => {
            // Preserve the editor buffer on a parse failure: echo the
            // original to stdout and report on stderr.
            ui::error(&format!("{}: {err}", path.display()));
            print!("{input}");
            return EXIT_ERROR;
        }
    }
}
```

- [ ] **Step 6: Add a dispatch test** to `lib.rs` (new
      `#[cfg(test)] mod tests`):

```rust
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
```

- [ ] **Step 7: Build + test — expect PASS**

Run: `cargo test --workspace` Expected: green.
(`grep -rn "prim_fmt::format" crates/prim-cli` first to confirm both call sites
were updated.)

- [ ] **Step 8: Commit**

```bash
git add crates/prim-fmt/src/error.rs crates/prim-fmt/src/lib.rs crates/prim-fmt/Cargo.toml \
        Cargo.lock crates/prim-cli/src/app.rs
git commit -m "refactor(fmt): make format fallible with FormatError (FR-6.3)"
```

---

## Task 2: `json` module — dprint integration + dispatch

**Files:**

- Create: `crates/prim-fmt/src/json.rs`
- Modify: `crates/prim-fmt/src/lib.rs` (`mod json;` + dispatch),
  `crates/prim-fmt/Cargo.toml`

- [ ] **Step 1: Failing unit test** — create `crates/prim-fmt/src/json.rs` with
      only the test module and a stub:

```rust
//! JSON / JSONC formatting (FR-1.2/1.3) via `dprint-plugin-json`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Indent, LineEnding, Style};

    #[test]
    fn one_space_after_colon() {
        let out = format("{\"a\":1}", &Style::default()).unwrap();
        assert!(out.contains("\"a\": 1"), "{out:?}");
    }
}
```

(Leave `format` undefined for now so the test fails to compile = red.)

- [ ] **Step 2: Run — expect FAIL (compile error: `format` not found)**

Run: `cargo test -p prim-fmt json::tests::one_space_after_colon 2>&1 | tail`
Expected: compile error — `cannot find function format`.

- [ ] **Step 3: Add the dependency** to `crates/prim-fmt/Cargo.toml`
      `[dependencies]`:

```toml
dprint-plugin-json = "0.21"
```

- [ ] **Step 4: Implement `format`** at the top of `json.rs` (above the test
      module):

```rust
use std::path::Path;

use dprint_plugin_json::configuration::{ConfigurationBuilder, TrailingCommaKind};
use dprint_plugin_json::format_text;

use crate::hygiene::hygiene;
use crate::{FormatError, Indent, Style};

/// Format `source` as JSONC under `style`, then apply whitespace hygiene for the
/// configured line ending and final newline.
///
/// JSON is a subset of JSONC, so both `FileKind::Json` and `FileKind::Jsonc`
/// route here; comments are preserved in either (FR-1.3). dprint's defaults give
/// one space after `:` and no trailing commas (FR-1.2) and never reorder
/// (FR-3.4/6.2). Invalid input yields [`FormatError::Parse`] (FR-6.3).
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let mut builder = ConfigurationBuilder::new();
    builder
        .line_width(style.max_line_length.unwrap_or(80) as u32)
        .trailing_commas(TrailingCommaKind::Never);
    match style.indent {
        Indent::Spaces(width) => {
            builder.use_tabs(false).indent_width(width as u8);
        }
        Indent::Tab => {
            builder.use_tabs(true);
        }
    }
    let config = builder.build();

    // A synthetic `.jsonc` path selects dprint's comment-aware mode; no file is
    // read (dprint uses only the extension). EOL is handled by `hygiene`.
    let printed = match format_text(Path::new("source.jsonc"), source, &config) {
        Ok(Some(text)) => text,
        Ok(None) => source.to_string(),
        Err(err) => return Err(FormatError::Parse(err.to_string())),
    };
    Ok(hygiene(&printed, style))
}
```

- [ ] **Step 5: Wire dispatch in `lib.rs`** — add `mod json;` (after
      `mod hygiene;`) and split the `match`:

```rust
pub fn format(kind: FileKind, source: &str, style: &Style) -> Result<String, FormatError> {
    match kind {
        FileKind::Json | FileKind::Jsonc => json::format(source, style),
        FileKind::Markdown | FileKind::Yaml | FileKind::Toml | FileKind::Orphan => {
            Ok(hygiene::hygiene(source, style))
        }
    }
}
```

- [ ] **Step 6: Expand `json.rs` tests** with the full property set (replace the
      test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Indent, LineEnding, Style};

    #[test]
    fn one_space_after_colon() {
        let out = format("{\"a\":1}", &Style::default()).unwrap();
        assert!(out.contains("\"a\": 1"), "{out:?}");
    }

    #[test]
    fn reindents_nested_object_to_style_width() {
        let src = "{\n\"a\": {\n\"b\": 1\n}\n}";
        let out = format(src, &Style::default()).unwrap(); // 2-space
        assert!(out.contains("\n  \"a\":"), "top key 2 spaces: {out:?}");
        assert!(out.contains("\n    \"b\":"), "nested key 4 spaces: {out:?}");
    }

    #[test]
    fn drops_trailing_comma() {
        let out = format("[\n1,\n2,\n]", &Style::default()).unwrap();
        assert!(!out.contains("2,"), "trailing comma dropped: {out:?}");
        assert!(out.contains('2'), "value kept: {out:?}");
    }

    #[test]
    fn preserves_comments() {
        let src = "{\n// keep me\n\"a\": 1\n}";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("// keep me"), "{out:?}");
    }

    #[test]
    fn tab_indent_from_style() {
        let style = Style { indent: Indent::Tab, ..Style::default() };
        let out = format("{\n\"a\": 1\n}", &style).unwrap();
        assert!(out.contains("\n\t\"a\""), "{out:?}");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style { end_of_line: LineEnding::CrLf, ..Style::default() };
        let out = format("{\n\"a\": 1\n}", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn invalid_json_is_a_parse_error() {
        assert!(matches!(format("{", &Style::default()), Err(FormatError::Parse(_))));
    }

    #[test]
    fn is_idempotent() {
        let src = "{\n\"a\":   1,\n  \"b\":2\n}";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
```

- [ ] **Step 7: Run — expect PASS**

Run: `cargo test -p prim-fmt json` Expected: all 8 tests pass. If any _exact_
expectation mismatches dprint's canonical output, the property assertions above
are deliberately tolerant; only adjust a test if dprint's output legitimately
violates an FR (it should not).

- [ ] **Step 8: Commit**

```bash
git add crates/prim-fmt/src/json.rs crates/prim-fmt/src/lib.rs crates/prim-fmt/Cargo.toml Cargo.lock
git commit -m "feat(fmt): JSON/JSONC formatting via dprint-plugin-json (FR-1.2/1.3)"
```

---

## Task 3: Behavioural CLI coverage

**Files:**

- Create: `crates/prim-cli/tests/json.rs`

- [ ] **Step 1: Write the behavioural tests**:

```rust
//! Behavioural tests: prim formats JSON/JSONC and fails safe on invalid input.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn reformats_messy_json_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\n\"a\":1,\n\"b\":   2\n}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\"a\": 1"), "{out:?}");
    assert!(out.contains("\"b\": 2"), "{out:?}");
    assert!(out.ends_with('\n'));
}

#[test]
fn check_flags_noncanonical_json() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\"a\":1}\n").unwrap(); // missing space after colon

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn editorconfig_indent_size_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nindent_style=space\nindent_size=4\n",
    )
    .unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\n\"a\": {\n\"b\": 1\n}\n}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\n    \"a\":"), "4-space top key: {out:?}");
    assert!(out.contains("\n        \"b\":"), "8-space nested key: {out:?}");
}

#[test]
fn jsonc_comments_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.jsonc");
    fs::write(&file, "{\n// note\n\"a\": 1\n}\n").unwrap();

    prim().arg(&file).assert().success();

    assert!(fs::read_to_string(&file).unwrap().contains("// note"));
}

#[test]
fn invalid_json_explicit_path_errors_and_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.json");
    fs::write(&file, "{ not valid").unwrap();

    prim().arg(&file).assert().failure().code(2);

    assert_eq!(fs::read_to_string(&file).unwrap(), "{ not valid");
}

#[test]
fn invalid_json_discovered_warns_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();

    // Discovered (directory walk), not explicitly named: warn, exit 0, untouched.
    prim().arg(dir.path()).assert().success();
    assert_eq!(fs::read_to_string(dir.path().join("bad.json")).unwrap(), "{ not valid");
}

#[test]
fn stdin_invalid_json_echoes_original_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.json");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("{ not valid")
        .assert()
        .failure()
        .code(2)
        .stdout("{ not valid");
}

#[test]
fn stdin_roundtrips_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.json");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("{\"a\":1}")
        .assert()
        .success();
}
```

- [ ] **Step 2: Run — expect PASS**

Run: `cargo test -p prim-cli --test json` Expected: all green. (If
`invalid_json_discovered_warns_and_succeeds` surfaces an exit-code surprise,
confirm `app.rs` routes discovered parse errors to `ui::warning` without setting
`had_error`.)

- [ ] **Step 3: Commit**

```bash
git add crates/prim-cli/tests/json.rs
git commit -m "test(cli): behavioural coverage for JSON/JSONC formatting"
```

---

## Task 4: Dogfood + docs

**Files:**

- Modify (if prim reformats them): `.markdownlint.json`, `dprint.json`
- Modify: `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`
- Create: `docs/decisions/0003-json-via-dprint-plugin-json.md`

- [ ] **Step 1: Dogfood — let prim format the repo's JSON**

Run: `cargo build && ./target/debug/prim .` Then `git diff --stat` — if
`.markdownlint.json` / `dprint.json` changed, that is prim formatting its own
connective tissue; keep the changes. Run `./target/debug/prim --check .` → must
exit 0.

- [ ] **Step 2: Update `AGENTS.md` status** — move JSON into implemented:

> Recursive discovery, whitespace hygiene, atomic writes, `.editorconfig` style
> resolution, and **JSON/JSONC formatting** are implemented and wired through
> the `prim-fmt` engine. The remaining per-format passes (YAML, TOML, Markdown)
> are follow-up milestones.

- [ ] **Step 3: Update `docs/USAGE.md`** — in the Status note, state that
      JSON/JSONC are now structured (canonical indentation, one space after `:`,
      no trailing commas, comments preserved), while YAML/TOML/Markdown remain
      hygiene-only.

- [ ] **Step 4: Create `docs/decisions/0003-json-via-dprint-plugin-json.md`**
      (AD-0003): record using `dprint-plugin-json` as a library, the
      alternatives (jsonc-parser + own printer, hand-roll), the
      fallible-`format` change with `FormatError`, the `.json`-as-JSONC
      leniency, and the stdin-echo-on-error behaviour. Trace:
      `Satisfies: FR-1.2, FR-1.3, FR-6.3`;
      `Related: AD-0001, docs/design/system.md`.

- [ ] **Step 5: Update `docs/design/system.md`** — note `format` now returns
      `Result<String, FormatError>`, the `json` module dispatch, and the
      parse-error data flow (in-place report vs stdin echo). Add to the
      `SUMMARY.md` if a new decision file warrants a link.

- [ ] **Step 6: Format docs + commit**

Run:
`dprint fmt && dprint check && npx --yes markdownlint-cli 'docs/**/*.md' --ignore node_modules`

```bash
git add -A
git commit -m "docs: document JSON/JSONC formatting (AD-0003) + dogfood repo JSON"
```

---

## Final verification (pre-PR gate)

- [ ] `cargo test --workspace` — green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — clean
- [ ] `dprint check` + `npx markdownlint-cli 'docs/**/*.md'` — clean
- [ ] `./target/debug/prim --check .` — exit 0
- [ ] Open PR `feat/json-format` → `main`, watch CI to green.

## Self-review (done while writing)

- **Spec coverage:** FR-1.2 (Task 2 colon/trailing-comma/indent tests), FR-1.3
  (comment tests Task 2/3), FR-3.4 + FR-6.2 (dprint preserves order/data — no
  reorder; idempotency test), FR-6.3 (fallible `format` Task 1 + invalid-input
  tests Task 2/3). ✓
- **Placeholder scan:** none — all steps carry complete code. ✓
- **Type consistency:** `FormatError::Parse`,
  `format(kind, source, &Style) -> Result<String, FormatError>`,
  `json::format(source, &Style)`, `ConfigurationBuilder`/`TrailingCommaKind`
  match the verified API across tasks. ✓
- **Decision 5 (.json vs .jsonc path):** resolved by always using a synthetic
  `.jsonc` path — both kinds format identically, comments preserved. ✓
