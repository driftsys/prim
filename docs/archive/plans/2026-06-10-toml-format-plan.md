# TOML Formatter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Canonical, comment- and inline-table-preserving TOML formatting
(FR-1.5), dispatched from `prim_fmt::format`, consuming #8's `Style`.

**Architecture:** A `toml` module in `prim-fmt` calls `taplo` (pure Rust): parse
for error detection, then `format_syntax` with a `Style`-mapped `Options`, then
reuse `hygiene` for EOL + final newline. Reuses the fallible `format` API from
#9 (no API change; only a new dispatch arm).

**Tech Stack:** Rust, `taplo = "0.14"`, `assert_cmd`/`tempfile` (tests).

**Invariant:** every commit compiles and `cargo test --workspace` is green.

**Verified `taplo` 0.14 API** (confirmed against source at tag
`release-taplo-0.14.0`):

- `taplo::parser::parse(src: &str) -> taplo::parser::Parse`, with public
  `errors: Vec<Error>` and `green_node`;
  `Parse::into_syntax(self) -> SyntaxNode`.
- `taplo::formatter::format_syntax(node: SyntaxNode, options: Options) -> String`.
- `taplo::formatter::Options` — public fields incl. `indent_string: String`,
  `column_width: usize`, `inline_table_expand: bool`,
  `reorder_keys/reorder_arrays/reorder_inline_tables: bool`, `crlf: bool`,
  `trailing_newline: bool`; implements `Default`. (`crlf`/`trailing_newline`
  left at default — `hygiene` owns EOL + final newline.)

---

## Task 1: `toml` module — taplo integration + dispatch

**Files:**

- Create: `crates/prim-fmt/src/toml.rs`
- Modify: `crates/prim-fmt/src/lib.rs`, `crates/prim-fmt/Cargo.toml`

- [ ] **Step 1: Failing unit test** — create `crates/prim-fmt/src/toml.rs` with
      only a test (leave `format` undefined → red):

```rust
//! TOML formatting (FR-1.5) via `taplo`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Style;

    #[test]
    fn canonicalizes_key_value_spacing() {
        let out = format("a=1\n", &Style::default()).unwrap();
        assert!(out.contains("a = 1"), "{out:?}");
    }
}
```

- [ ] **Step 2: Run — expect FAIL (compile error: `format` not found)**

Run:
`cargo test -p prim-fmt toml::tests::canonicalizes_key_value_spacing 2>&1 | tail`
Expected: compile error — `cannot find function format`.

- [ ] **Step 3: Add the dependency** to `crates/prim-fmt/Cargo.toml`
      `[dependencies]` (alphabetical, after `dprint-plugin-json`):

```toml
taplo = "0.14"
```

- [ ] **Step 4: Implement `format`** at the top of `toml.rs` (above the test
      module):

```rust
use taplo::formatter::{format_syntax, Options};
use taplo::parser::parse;

use crate::hygiene::hygiene;
use crate::{FormatError, Indent, Style};

/// Format `source` as TOML under `style`, then apply whitespace hygiene for the
/// configured line ending and final newline.
///
/// taplo canonicalizes spacing/indentation and preserves comments; with
/// `inline_table_expand = false` it preserves inline-table style (FR-1.5) and
/// with `reorder_* = false` never reorders (FR-3.4/6.2). Malformed input is
/// detected via `parse().errors` and returned as [`FormatError::Parse`]
/// (FR-6.3) — taplo's formatter is otherwise lenient and would skip invalid
/// parts.
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let parsed = parse(source);
    if !parsed.errors.is_empty() {
        let message = parsed
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FormatError::Parse(message));
    }

    // Built via mutable Default so additions to taplo's Options never break this.
    let mut options = Options::default();
    options.indent_string = match style.indent {
        Indent::Spaces(width) => " ".repeat(width),
        Indent::Tab => "\t".to_string(),
    };
    options.column_width = style.max_line_length.unwrap_or(80);
    options.inline_table_expand = false; // FR-1.5: preserve inline-table style
    options.reorder_keys = false; // FR-3.4
    options.reorder_arrays = false; // FR-3.4
    options.reorder_inline_tables = false; // FR-3.4

    let printed = format_syntax(parsed.into_syntax(), options);
    Ok(hygiene(&printed, style))
}
```

- [ ] **Step 5: Wire dispatch in `lib.rs`** — add `mod toml;` (after
      `mod style;`, keeping alphabetical-ish order with the other modules) and
      add the `Toml` arm:

```rust
pub fn format(kind: FileKind, source: &str, style: &Style) -> Result<String, FormatError> {
    match kind {
        FileKind::Json | FileKind::Jsonc => json::format(source, style),
        FileKind::Toml => toml::format(source, style),
        FileKind::Markdown | FileKind::Yaml | FileKind::Orphan => {
            Ok(hygiene::hygiene(source, style))
        }
    }
}
```

- [ ] **Step 6: Expand `toml.rs` tests** (replace the test module):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Indent, LineEnding, Style};

    #[test]
    fn canonicalizes_key_value_spacing() {
        let out = format("a=1\n", &Style::default()).unwrap();
        assert!(out.contains("a = 1"), "{out:?}");
    }

    #[test]
    fn preserves_inline_table_style() {
        let out = format("x = {a=1}\n", &Style::default()).unwrap();
        assert!(out.contains("{ a = 1 }"), "inline table kept: {out:?}");
        assert!(!out.contains("[x]"), "not expanded to a table: {out:?}");
    }

    #[test]
    fn preserves_comments() {
        let out = format("# keep me\na = 1\n", &Style::default()).unwrap();
        assert!(out.contains("# keep me"), "{out:?}");
    }

    #[test]
    fn preserves_key_order() {
        let out = format("b = 2\na = 1\n", &Style::default()).unwrap();
        let b = out.find("b =").unwrap();
        let a = out.find("a =").unwrap();
        assert!(b < a, "order preserved (b before a): {out:?}");
    }

    #[test]
    fn indents_multiline_array_to_style_width() {
        let style = Style { indent: Indent::Spaces(4), ..Style::default() };
        let out = format("arr = [\n1,\n2,\n]\n", &style).unwrap();
        assert!(out.contains("\n    1,"), "4-space array element: {out:?}");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style { end_of_line: LineEnding::CrLf, ..Style::default() };
        let out = format("a = 1\n", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn invalid_toml_is_a_parse_error() {
        assert!(matches!(format("a = = 1\n", &Style::default()), Err(FormatError::Parse(_))));
    }

    #[test]
    fn is_idempotent() {
        let src = "b=2\n# c\na   =   1\narr=[1,2]\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
```

- [ ] **Step 7: Run — expect PASS**

Run: `cargo test -p prim-fmt toml` Expected: all 8 tests pass. The assertions
are tolerant of taplo's exact byte output; if `invalid_toml_is_a_parse_error`
does not trigger (taplo parsed it), swap the input for another clearly-invalid
TOML such as `"= 1\n"` (value with no key) and re-run.

- [ ] **Step 8: Commit**

```bash
git add crates/prim-fmt/src/toml.rs crates/prim-fmt/src/lib.rs crates/prim-fmt/Cargo.toml Cargo.lock
git commit -m "feat(fmt): TOML formatting via taplo (FR-1.5)"
```

---

## Task 2: Behavioural CLI coverage

**Files:**

- Create: `crates/prim-cli/tests/toml.rs`

- [ ] **Step 1: Write the behavioural tests**:

```rust
//! Behavioural tests: prim formats TOML and fails safe on invalid input.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn reformats_messy_toml_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a=1\nb   =   2\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("a = 1"), "{out:?}");
    assert!(out.contains("b = 2"), "{out:?}");
}

#[test]
fn check_flags_noncanonical_toml() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a=1\n").unwrap();

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
    let file = dir.path().join("a.toml");
    fs::write(&file, "arr = [\n1,\n2,\n]\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\n    1,"), "4-space array element: {out:?}");
}

#[test]
fn inline_table_and_comments_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "# note\nx = {a=1}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("# note"), "{out:?}");
    assert!(out.contains("{ a = 1 }"), "{out:?}");
}

#[test]
fn invalid_toml_explicit_path_errors_and_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.toml");
    fs::write(&file, "a = = 1").unwrap();

    prim().arg(&file).assert().failure().code(2);

    assert_eq!(fs::read_to_string(&file).unwrap(), "a = = 1");
}

#[test]
fn invalid_toml_discovered_warns_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.toml"), "a = = 1").unwrap();

    prim().arg(dir.path()).assert().success();
    assert_eq!(fs::read_to_string(dir.path().join("bad.toml")).unwrap(), "a = = 1");
}

#[test]
fn stdin_invalid_toml_echoes_original_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.toml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a = = 1")
        .assert()
        .failure()
        .code(2)
        .stdout("a = = 1");
}

#[test]
fn stdin_roundtrips_valid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.toml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a=1\n")
        .assert()
        .success();
}
```

- [ ] **Step 2: Run — expect PASS**

Run: `cargo test -p prim-cli --test toml` Expected: all green. (If the chosen
invalid TOML parses cleanly under taplo, replace `"a = = 1"` consistently across
these tests and Task 1 with `"= 1"`.)

- [ ] **Step 3: Commit**

```bash
git add crates/prim-cli/tests/toml.rs
git commit -m "test(cli): behavioural coverage for TOML formatting"
```

---

## Task 3: Dogfood + docs

**Files:**

- Modify (if prim reformats them): the repo's `Cargo.toml` files
- Modify: `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`,
  `docs/SUMMARY.md`
- Create: `docs/decisions/0004-toml-via-taplo.md`

- [ ] **Step 1: Dogfood — let prim format the repo's TOML**

Run: `cargo build && ./target/debug/prim .` then `git diff --stat`. Review any
`Cargo.toml` changes (taplo never reorders, so they are whitespace-only); keep
them. Run `./target/debug/prim --check .` → must exit 0. Confirm `Cargo.lock` is
untouched (it is not owned).

- [ ] **Step 2: Update `AGENTS.md` status** — add TOML to the implemented list:

> … `.editorconfig` style resolution, JSON/JSONC formatting, and **TOML
> formatting** are implemented and wired through the `prim-fmt` engine. The
> remaining per-format structured passes (YAML, Markdown) are follow-up
> milestones.

- [ ] **Step 3: Update `docs/USAGE.md` status note** — add that TOML now
      receives structured canonical formatting (comments and inline-table style
      preserved); YAML and Markdown remain hygiene-only.

- [ ] **Step 4: Create `docs/decisions/0004-toml-via-taplo.md`** (AD-0004):
      record using `taplo` as a library, the alternatives (`toml_edit`
      preserve-only, hand-roll), the `Options` mapping (`indent_string`,
      `column_width`, `inline_table_expand = false` for FR-1.5,
      `reorder_* = false` for FR-3.4, EOL owned by hygiene), and parse-error
      detection via `taplo::parser::parse`. Trace:
      `Satisfies: FR-1.5, FR-3.4, FR-6.2, FR-6.3`;
      `Related: AD-0003, docs/design/system.md`.

- [ ] **Step 5: Update `docs/design/system.md`** — add `toml.rs` to the
      component map, add the `FileKind::Toml` dispatch to the Format step, and
      update the Implementation status section to list TOML (FR-1.5) as
      implemented. Add the AD-0004 link to `docs/SUMMARY.md`.

- [ ] **Step 6: Format docs + commit**

Run:
`dprint fmt && dprint check && npx --yes markdownlint-cli 'docs/**/*.md' 'AGENTS.md' --ignore node_modules`

```bash
git add -A
git commit -m "docs: document TOML formatting (AD-0004) + dogfood repo TOML"
```

---

## Final verification (pre-PR gate)

- [ ] `cargo test --workspace` — green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — clean
- [ ] `dprint check` + `npx markdownlint-cli 'docs/**/*.md'` — clean
- [ ] `./target/debug/prim --check .` — exit 0
- [ ] Open PR `feat/toml-format` → `main`, watch CI to green.

## Self-review (done while writing)

- **Spec coverage:** FR-1.5 (Task 1 spacing/indent/inline-table/comment tests),
  FR-3.4 (key-order test + `reorder_*=false`), FR-6.2 (taplo preserves data;
  idempotency), FR-6.3 (parse-error detection + invalid-input tests Task 1/2). ✓
- **Placeholder scan:** none — all steps carry complete code. ✓
- **Type consistency:** `format(source, &Style) -> Result<String, FormatError>`,
  `Options` field names, `parse`/`into_syntax`/`format_syntax` match the
  verified API; dispatch arm matches lib.rs `match`. ✓
- **EOL:** owned by `hygiene`; taplo `crlf`/`trailing_newline` left at default —
  consistent with #9. ✓
