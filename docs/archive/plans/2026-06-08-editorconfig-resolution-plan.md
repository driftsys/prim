# `.editorconfig` Resolution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make prim honor `.editorconfig` as its only style configuration
(FR-3), turning the hard-coded whitespace-hygiene pass into a `Style`-driven one
(incl. FR-2.3 `crlf`).

**Architecture:** A pure `Style` data struct lives in `prim-fmt`;
`prim_fmt::format` and `hygiene` consume it. `prim-cli` resolves a `Style` from
the `.editorconfig` cascade via the `ec4rs` crate (the only I/O) and passes it
into the engine. Missing config → `Style::default()` (canonical, FR-3.1);
malformed/unreadable → default + warning.

**Tech Stack:** Rust, `ec4rs = "1.2"` (EditorConfig cascade + glob),
`assert_cmd`/`predicates` (behavioural tests), `tempfile` (fixtures).

**Invariant:** every commit compiles and `cargo test --workspace` is green.
`Style::default()` reproduces today's exact hygiene output, so CLI behaviour is
unchanged until Task 3 wires resolution in.

**Verified `ec4rs` 1.2 API** (do not guess — these are confirmed against the
source at tag `v1.2.0`):

- `ec4rs::properties_of(path: impl AsRef<Path>) -> Result<ec4rs::Properties, ec4rs::Error>`;
  returns `Ok(empty)` when no `.editorconfig` is found.
- `Properties::get::<T: PropertyKey + PropertyValue>(&self) -> Result<T, &RawValue>`
  (`Err` = unset/unparseable).
- `Properties::use_fallbacks(&mut self)` — adds spec fallbacks
  (indent_size/tab_width).
- `ec4rs::property::{EndOfLine{Lf,CrLf,Cr}, IndentStyle{Tabs,Spaces}, IndentSize{Value(usize),UseTabWidth}, MaxLineLen{Value(usize),Off}, TrimTrailingWs{Value(bool)}, FinalNewline{Value(bool)}, TabWidth{Value(usize)}}`.

---

## Task 1: `Style` data struct in `prim-fmt`

**Files:**

- Create: `crates/prim-fmt/src/style.rs`
- Modify: `crates/prim-fmt/src/lib.rs` (add `mod style;` + re-export)

- [ ] **Step 1: Write `style.rs` with its unit tests**

```rust
//! Resolved formatting style (FR-3): the single source of configuration the
//! engine consumes. Built by `prim-cli` from `.editorconfig` and passed into
//! [`crate::format`]. [`Style::default`] is prim's built-in canonical style
//! (FR-3.1), applied when no `.editorconfig` is present.

/// The line ending prim emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n` — prim's canonical default.
    Lf,
    /// `\r\n` — only when `.editorconfig` sets `end_of_line = crlf` (FR-2.3).
    CrLf,
}

impl LineEnding {
    /// The byte sequence for this line ending.
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

/// Indentation unit. Carried for the per-format parsers (FR-1, #9–12); the
/// whitespace-hygiene pass does not consume it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// `indent_style = space` with the given `indent_size`.
    Spaces(usize),
    /// `indent_style = tab`.
    Tab,
}

/// The resolved canonical style for one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Line ending to emit (FR-2.3).
    pub end_of_line: LineEnding,
    /// Strip trailing whitespace from each line (FR-2.1).
    pub trim_trailing_whitespace: bool,
    /// When true, end content with exactly one final line ending; when false,
    /// strip any final line ending (FR-2.2 / `insert_final_newline`).
    pub insert_final_newline: bool,
    /// Indentation unit (carried for FR-1 parsers; unused by hygiene).
    pub indent: Indent,
    /// Hard-wrap width (carried for FR-1 Markdown; unused by hygiene). `None`
    /// means unset — the Markdown formatter falls back to 80.
    pub max_line_length: Option<usize>,
}

impl Default for Style {
    /// prim's built-in canonical style (FR-3.1): LF endings, trailing
    /// whitespace stripped, exactly one final newline, two-space indent.
    fn default() -> Self {
        Style {
            end_of_line: LineEnding::Lf,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            indent: Indent::Spaces(2),
            max_line_length: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_canonical_style() {
        let s = Style::default();
        assert_eq!(s.end_of_line, LineEnding::Lf);
        assert!(s.trim_trailing_whitespace);
        assert!(s.insert_final_newline);
        assert_eq!(s.indent, Indent::Spaces(2));
        assert_eq!(s.max_line_length, None);
    }

    #[test]
    fn line_ending_bytes() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    }
}
```

- [ ] **Step 2: Wire into `lib.rs`** — add under the existing `mod classify;`
      line:

```rust
mod style;
```

and under the existing `pub use classify::{FileKind, classify};`:

```rust
pub use style::{Indent, LineEnding, Style};
```

- [ ] **Step 3: Run tests — expect PASS**

Run: `cargo test -p prim-fmt style` Expected: `default_is_the_canonical_style`
and `line_ending_bytes` PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/prim-fmt/src/style.rs crates/prim-fmt/src/lib.rs
git commit -m "feat(fmt): add resolved Style with canonical default (FR-3.1)"
```

---

## Task 2: `Style`-driven hygiene + `format(&Style)`, defaults threaded at call sites

**Files:**

- Modify: `crates/prim-fmt/src/hygiene.rs` (signature + behaviour + tests)
- Modify: `crates/prim-fmt/src/lib.rs` (`format` signature)
- Modify: `crates/prim-cli/src/app.rs` (pass `&prim_fmt::Style::default()` at
  both call sites)

- [ ] **Step 1: Replace `hygiene.rs` body** with the `Style`-driven version and
      tests:

```rust
//! Whitespace hygiene (FR-2.1/2.2/2.3): the format-agnostic pass applied to
//! every file prim owns, driven by the resolved [`Style`].

use crate::Style;

/// Apply whitespace hygiene to `source` under `style`:
///
/// - normalise every line ending to `style.end_of_line` (FR-2.3),
/// - when `style.trim_trailing_whitespace`, strip trailing whitespace from each
///   line (FR-2.1),
/// - when `style.insert_final_newline`, end non-empty content with exactly one
///   line ending; otherwise strip any final ending (FR-2.2). Empty (or, when
///   trimming, whitespace-only) content stays empty.
///
/// The pass is idempotent (FR-6.1).
pub fn hygiene(source: &str, style: &Style) -> String {
    // Normalise existing endings to LF so we can reason in logical lines.
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");

    // Optionally strip trailing whitespace, re-joining by LF for now.
    let mut joined = String::with_capacity(normalized.len());
    for line in normalized.split('\n') {
        if style.trim_trailing_whitespace {
            joined.push_str(line.trim_end());
        } else {
            joined.push_str(line);
        }
        joined.push('\n');
    }

    // Content body with the trailing newline run removed.
    let body = joined.trim_end_matches('\n');
    if body.is_empty() {
        return String::new();
    }

    // Apply the configured line ending and the final-newline rule.
    let eol = style.end_of_line.as_str();
    let mut result = body.replace('\n', eol);
    if style.insert_final_newline {
        result.push_str(eol);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Indent, LineEnding};

    fn canonical() -> Style {
        Style::default()
    }

    // --- Regression guard: default Style == the pre-#8 hard-coded behaviour ---

    #[test]
    fn default_trims_trailing_whitespace_per_line() {
        assert_eq!(hygiene("a  \nb\t\n", &canonical()), "a\nb\n");
    }

    #[test]
    fn default_preserves_leading_and_inner_whitespace() {
        assert_eq!(hygiene("  a  b  \n", &canonical()), "  a  b\n");
    }

    #[test]
    fn default_ensures_single_final_newline() {
        assert_eq!(hygiene("a", &canonical()), "a\n");
        assert_eq!(hygiene("a\n\n\n", &canonical()), "a\n");
    }

    #[test]
    fn default_normalizes_crlf_and_cr_to_lf() {
        assert_eq!(hygiene("a\r\nb\rc\n", &canonical()), "a\nb\nc\n");
    }

    #[test]
    fn default_empty_or_whitespace_only_stays_empty() {
        assert_eq!(hygiene("", &canonical()), "");
        assert_eq!(hygiene("   \n  \n", &canonical()), "");
    }

    // --- Style-driven behaviour (FR-3.2) ---

    #[test]
    fn crlf_end_of_line_is_emitted() {
        let style = Style { end_of_line: LineEnding::CrLf, ..Style::default() };
        assert_eq!(hygiene("a\nb\n", &style), "a\r\nb\r\n");
        // Mixed input normalises to the configured ending.
        assert_eq!(hygiene("a\r\nb\nc\r\n", &style), "a\r\nb\r\nc\r\n");
    }

    #[test]
    fn trim_disabled_preserves_trailing_whitespace_but_normalizes_eol() {
        let style = Style { trim_trailing_whitespace: false, ..Style::default() };
        assert_eq!(hygiene("a  \r\nb \n", &style), "a  \nb \n");
    }

    #[test]
    fn insert_final_newline_false_strips_final_newline() {
        let style = Style { insert_final_newline: false, ..Style::default() };
        assert_eq!(hygiene("a\nb\n", &style), "a\nb");
        assert_eq!(hygiene("a\n\n", &style), "a");
    }

    #[test]
    fn carried_fields_do_not_affect_hygiene() {
        // indent / max_line_length are unconsumed by hygiene; output unchanged.
        let style = Style { indent: Indent::Tab, max_line_length: Some(100), ..Style::default() };
        assert_eq!(hygiene("a  \nb\n", &style), "a\nb\n");
    }

    #[test]
    fn is_idempotent_under_each_style() {
        let styles = [
            Style::default(),
            Style { end_of_line: LineEnding::CrLf, ..Style::default() },
            Style { trim_trailing_whitespace: false, ..Style::default() },
            Style { insert_final_newline: false, ..Style::default() },
        ];
        for style in styles {
            for input in ["a  \r\nb\n\n", "", "x", "  keep\nlead  \n", "   \n"] {
                let once = hygiene(input, &style);
                assert_eq!(hygiene(&once, &style), once, "not idempotent: {input:?} / {style:?}");
            }
        }
    }
}
```

- [ ] **Step 2: Update `format` in `lib.rs`** — change the signature and the
      dispatch call:

```rust
/// Format `source` as the given [`FileKind`] under `style`.
///
/// Every kind currently receives only the whitespace-hygiene pass; the `match`
/// is the dispatch point where structured per-format passes (FR-1) attach.
pub fn format(kind: FileKind, source: &str, style: &Style) -> String {
    match kind {
        FileKind::Markdown
        | FileKind::Json
        | FileKind::Jsonc
        | FileKind::Yaml
        | FileKind::Toml
        | FileKind::Orphan => hygiene::hygiene(source, style),
    }
}
```

- [ ] **Step 3: Thread `Style::default()` at the two `app.rs` call sites**
      (keeps behaviour identical; Task 3 swaps in real resolution).

In `run_stdin`, replace the `match prim_fmt::classify(path) { ... }` block with:

```rust
match prim_fmt::classify(path) {
    Some(kind) => {
        let style = prim_fmt::Style::default();
        print!("{}", prim_fmt::format(kind, &input, &style));
    }
    None => print!("{input}"),
}
```

In `run_paths`, replace the `let formatted = prim_fmt::format(kind, &original);`
line with:

```rust
let style = prim_fmt::Style::default();
let formatted = prim_fmt::format(kind, &original, &style);
```

- [ ] **Step 4: Run the suites — expect PASS**

Run: `cargo test --workspace` Expected: all green, including the new hygiene
tests and the unchanged CLI tests (default Style reproduces prior output).

- [ ] **Step 5: Commit**

```bash
git add crates/prim-fmt/src/hygiene.rs crates/prim-fmt/src/lib.rs crates/prim-cli/src/app.rs
git commit -m "feat(fmt): make whitespace hygiene Style-driven (FR-2.3/FR-3.2)"
```

---

## Task 3: Resolve `Style` from `.editorconfig` and wire it in

**Files:**

- Modify: `crates/prim-cli/Cargo.toml` (add `ec4rs = "1.2"`)
- Create: `crates/prim-cli/src/editorconfig.rs`
- Modify: `crates/prim-cli/src/main.rs` (`mod editorconfig;`)
- Modify: `crates/prim-cli/src/app.rs` (replace `Style::default()` with
  `editorconfig::resolve(...)`)

ATDD note: Step 1 writes a failing end-to-end behavioural test first; the rest
greens it.

- [ ] **Step 1: Failing acceptance test** — create
      `crates/prim-cli/tests/editorconfig.rs`:

```rust
//! Behavioural tests: prim honors `.editorconfig` (FR-3).

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn crlf_end_of_line_is_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), "root = true\n[*]\nend_of_line = crlf\n").unwrap();
    let file = dir.path().join("notes.md");
    fs::write(&file, "a\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\r\nb\r\n");
}
```

- [ ] **Step 2: Run it — expect FAIL** (prim still forces LF):

Run: `cargo test -p prim-cli --test editorconfig crlf_end_of_line_is_written`
Expected: FAIL — output is `a\nb\n`, not `a\r\nb\r\n`.

- [ ] **Step 3: Add the dependency** to `crates/prim-cli/Cargo.toml` under
      `[dependencies]` (alphabetical, before `ignore`):

```toml
ec4rs = "1.2"
```

- [ ] **Step 4: Create `crates/prim-cli/src/editorconfig.rs`**:

```rust
//! Resolve prim's [`Style`] from the `.editorconfig` cascade (FR-3).
//!
//! Walking the directory tree and reading files is I/O, so resolution lives in
//! the CLI crate; the engine consumes only the resolved [`Style`]. A missing
//! `.editorconfig` yields the built-in canonical style (FR-3.1); an unreadable
//! or malformed one falls back to it with a warning.

use std::path::Path;

use ec4rs::property::{
    EndOfLine, FinalNewline, IndentSize, IndentStyle, MaxLineLen, TabWidth, TrimTrailingWs,
};
use prim_fmt::{Indent, LineEnding, Style};

use crate::ui;

/// Resolve the [`Style`] that applies to `path` from the `.editorconfig`
/// cascade rooted at its directory.
pub fn resolve(path: &Path) -> Style {
    let style = Style::default();

    let mut cfg = match ec4rs::properties_of(path) {
        Ok(cfg) => cfg,
        Err(err) => {
            ui::warning(&format!(
                "{}: ignoring unreadable .editorconfig ({err}); using canonical style",
                path.display()
            ));
            return style;
        }
    };
    cfg.use_fallbacks();

    let mut style = style;
    if let Ok(eol) = cfg.get::<EndOfLine>() {
        // FR-2.3 carves out crlf only; deprecated bare `cr` falls back to LF.
        style.end_of_line = match eol {
            EndOfLine::CrLf => LineEnding::CrLf,
            EndOfLine::Lf | EndOfLine::Cr => LineEnding::Lf,
        };
    }
    if let Ok(TrimTrailingWs::Value(trim)) = cfg.get::<TrimTrailingWs>() {
        style.trim_trailing_whitespace = trim;
    }
    if let Ok(FinalNewline::Value(insert)) = cfg.get::<FinalNewline>() {
        style.insert_final_newline = insert;
    }
    style.indent = resolve_indent(&cfg, style.indent);
    if let Ok(max) = cfg.get::<MaxLineLen>() {
        style.max_line_length = match max {
            MaxLineLen::Value(n) => Some(n),
            MaxLineLen::Off => None,
        };
    }
    style
}

/// Map `indent_style` + `indent_size`/`tab_width` onto [`Indent`], keeping the
/// canonical default when `indent_style` is unset.
fn resolve_indent(cfg: &ec4rs::Properties, default: Indent) -> Indent {
    match cfg.get::<IndentStyle>() {
        Ok(IndentStyle::Tabs) => Indent::Tab,
        Ok(IndentStyle::Spaces) => Indent::Spaces(indent_width(cfg).unwrap_or(2)),
        Err(_) => default,
    }
}

fn indent_width(cfg: &ec4rs::Properties) -> Option<usize> {
    match cfg.get::<IndentSize>() {
        Ok(IndentSize::Value(n)) => Some(n),
        Ok(IndentSize::UseTabWidth) => match cfg.get::<TabWidth>() {
            Ok(TabWidth::Value(n)) => Some(n),
            _ => None,
        },
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Write `.editorconfig` `content` into a fresh temp dir and resolve the
    /// style for `relative` (a path under that dir).
    fn resolve_in(content: &str, relative: &str) -> (tempfile::TempDir, Style) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), content).unwrap();
        let style = resolve(&dir.path().join(relative));
        (dir, style)
    }

    #[test]
    fn no_editorconfig_yields_canonical_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve(&dir.path().join("a.md")), Style::default());
    }

    #[test]
    fn honors_end_of_line_crlf() {
        let (_d, style) = resolve_in("root=true\n[*]\nend_of_line=crlf\n", "a.md");
        assert_eq!(style.end_of_line, LineEnding::CrLf);
    }

    #[test]
    fn honors_trim_and_final_newline_disabled() {
        let cfg = "root=true\n[*]\ntrim_trailing_whitespace=false\ninsert_final_newline=false\n";
        let (_d, style) = resolve_in(cfg, "a.md");
        assert!(!style.trim_trailing_whitespace);
        assert!(!style.insert_final_newline);
    }

    #[test]
    fn per_glob_sections_select_indent_and_width() {
        let cfg = "root=true\n[*]\nindent_style=space\nindent_size=2\n[*.md]\nmax_line_length=80\n[*.rs]\nindent_size=4\n";
        let (_d, md) = resolve_in(cfg, "doc.md");
        assert_eq!(md.indent, Indent::Spaces(2));
        assert_eq!(md.max_line_length, Some(80));
        let (_d2, rs) = resolve_in(cfg, "main.rs");
        assert_eq!(rs.indent, Indent::Spaces(4));
        assert_eq!(rs.max_line_length, None);
    }

    #[test]
    fn honors_tab_indent_style() {
        let (_d, style) = resolve_in("root=true\n[Makefile]\nindent_style=tab\n", "Makefile");
        assert_eq!(style.indent, Indent::Tab);
    }

    #[test]
    fn root_chain_stops_at_root_true() {
        // Inner root=true must shadow an outer config that sets crlf.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), "root=true\n[*]\nend_of_line=crlf\n").unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".editorconfig"), "root=true\n[*]\nend_of_line=lf\n").unwrap();
        let style = resolve(&sub.join("a.md"));
        assert_eq!(style.end_of_line, LineEnding::Lf);
    }
}
```

- [ ] **Step 5: Register the module** in `crates/prim-cli/src/main.rs` — add to
      the `mod` block (alphabetical):

```rust
mod editorconfig;
```

- [ ] **Step 6: Wire resolution into `app.rs`** — replace the
      `let style = prim_fmt::Style::default();` lines introduced in Task 2.

In `run_stdin`:

```rust
Some(kind) => {
    let style = crate::editorconfig::resolve(path);
    print!("{}", prim_fmt::format(kind, &input, &style));
}
```

In `run_paths` (the file is owned at this point, so resolving is not wasted):

```rust
let style = crate::editorconfig::resolve(&file.path);
let formatted = prim_fmt::format(kind, &original, &style);
```

Add `use crate::editorconfig;` near the other `use crate::...;` lines and use
`editorconfig::resolve` if you prefer the unqualified form (match the file's
existing import style).

- [ ] **Step 7: Run everything — expect PASS**

Run: `cargo test -p prim-cli` Expected: the Step 1 acceptance test now PASSES,
plus all `editorconfig.rs` unit tests.

- [ ] **Step 8: Commit**

```bash
git add crates/prim-cli/Cargo.toml Cargo.lock crates/prim-cli/src/editorconfig.rs \
        crates/prim-cli/src/main.rs crates/prim-cli/src/app.rs crates/prim-cli/tests/editorconfig.rs
git commit -m "feat(cli): resolve Style from .editorconfig via ec4rs (FR-3)"
```

---

## Task 4: Behavioural coverage of the remaining honored keys + dogfood

**Files:**

- Modify: `crates/prim-cli/tests/editorconfig.rs` (add end-to-end cases)

- [ ] **Step 1: Add behavioural tests** to `tests/editorconfig.rs`:

```rust
#[test]
fn insert_final_newline_false_strips_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ninsert_final_newline=false\n",
    )
    .unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{}\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
}

#[test]
fn trim_disabled_keeps_trailing_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ntrim_trailing_whitespace=false\n",
    )
    .unwrap();
    let file = dir.path().join("a.yaml");
    fs::write(&file, "a:  \n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a:  \n");
}

#[test]
fn check_mode_flags_crlf_when_config_demands_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), "root=true\n[*]\nend_of_line=crlf\n").unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a = 1\n").unwrap(); // LF on disk, config wants CRLF

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn stdin_filepath_honors_sibling_editorconfig() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), "root=true\n[*]\nend_of_line=crlf\n").unwrap();
    let target = dir.path().join("x.md");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a\nb\n")
        .assert()
        .success()
        .stdout("a\r\nb\r\n");
}

#[test]
fn no_editorconfig_leaves_canonical_behaviour() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "a  \r\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\nb\n");
}
```

- [ ] **Step 2: Run them — expect PASS**

Run: `cargo test -p prim-cli --test editorconfig` Expected: all green.

- [ ] **Step 3: Dogfood — the repo's own `.editorconfig` must stay clean**

Run: `cargo build && ./target/debug/prim --check .` Expected: exit 0 (the repo
is LF / trim / final-newline / 2-space, matching canonical).

- [ ] **Step 4: Commit**

```bash
git add crates/prim-cli/tests/editorconfig.rs
git commit -m "test(cli): behavioural coverage for .editorconfig honored keys"
```

---

## Task 5: Documentation

**Files:**

- Modify: `AGENTS.md` (status note)
- Modify: `docs/USAGE.md` (honored keys + scope notes)
- Modify: `docs/SPEC.md` (FR-2.3 / FR-3 implemented touch-up, if a status line
  exists)

- [ ] **Step 1: Update `AGENTS.md` status** — move `.editorconfig` resolution
      out of "follow-up" into implemented. Change the Status blockquote so it
      reads (keep surrounding wording):

> Recursive discovery, the format-agnostic **whitespace hygiene** pass
> (trailing-whitespace removal, single final line-feed, LF endings), atomic
> writes, and **`.editorconfig` style resolution** are implemented and wired
> through the `prim-fmt` engine. The per-format structured passes
> (JSON/YAML/TOML/Markdown) are follow-up milestones.

- [ ] **Step 2: Add a `.editorconfig` section to `docs/USAGE.md`** documenting:
  - prim honors `.editorconfig` as its only configuration (FR-3.3 — no
    `prim.toml`, no flags).
  - Honored keys today: `end_of_line` (lf/crlf), `trim_trailing_whitespace`,
    `insert_final_newline` (false ⇒ no final newline), plus
    `indent_style`/`indent_size`/`max_line_length` carried for the upcoming
    parsers.
  - Scope notes: `charset` beyond `utf-8` is unsupported (prim is UTF-8 only);
    `end_of_line = cr` is treated as `lf`; a missing config applies the built-in
    canonical style; an unreadable/malformed config is ignored with a warning.

- [ ] **Step 3: Touch `docs/SPEC.md`** only if it carries an
      implementation-status marker for FR-2.3 / FR-3; otherwise leave the
      normative spec unchanged.

- [ ] **Step 4: Format docs and verify**

Run: `dprint fmt && dprint check` Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md docs/USAGE.md docs/SPEC.md
git commit -m "docs: document .editorconfig resolution and its scope (FR-3)"
```

---

## Final verification (pre-PR gate)

- [ ] `cargo test --workspace` — green
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — clean
- [ ] `cargo fmt --check` — clean
- [ ] `dprint check` — clean
- [ ] `./target/debug/prim --check .` — exit 0 (dogfood)
- [ ] Open PR `feat/editorconfig` → `main`, `gh pr checks --watch` to green
      (CodeQL runs on `main`-targeting PRs).

## Self-review (done while writing)

- **Spec coverage:** FR-3.1 (Task 1 Default + Task 3 no-config test), FR-3.2 all
  keys (Task 3 resolve + unit tests; Task 4 behavioural), FR-3.3 (no new config
  surface — documented Task 5), FR-3.4 (hygiene never reorders — unchanged,
  line-oriented), FR-2.3 crlf (Task 2 hygiene + Task 3/4 end-to-end). ✓
- **Placeholder scan:** none — all code blocks are complete. ✓
- **Type consistency:** `Style`/`LineEnding`/`Indent` field + variant names
  match across Tasks 1–4; `resolve`/`resolve_indent`/`indent_width` signatures
  consistent; ec4rs identifiers match the verified API. ✓
- **Charset:** deliberately omitted from `Style` (decision 2); documented as a
  scope cut in Task 5. ✓
