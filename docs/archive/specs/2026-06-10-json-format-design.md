# Design — JSON + JSONC formatter (FR-1.2/1.3) · issue #9

**Status:** approved (brainstorm) · **Branch:** `feat/json-format` · **Epic:**
#4

Canonical, comment-preserving formatting for JSON and JSONC — the first
per-format structured pass, dispatched from `prim_fmt::format`. Consumes #8's
resolved `Style` (indent width). Introduces the engine's first fallible format
path (parse errors).

## Requirements (acceptance criteria, from #1)

- **FR-1.2** Format JSON to a canonical style: consistent indentation, one space
  after `:`, no trailing commas.
- **FR-1.3** Format JSONC, preserving all comments in position. (JSON5
  excluded.)
- **FR-3.4** Never reorder keys or array elements (semantics-preserving).
- **FR-6.2** Do not change the parsed data model.
- **FR-6.3** Unparseable files are left byte-for-byte unchanged and reported.

## Decision: use `dprint-plugin-json` as a library

prim formats JSON/JSONC by calling `dprint-plugin-json` (the dprint JSON
formatter) as a library — the same engine this repository already uses via
dprint, so prim's JSON output matches the established style.

- Its defaults already satisfy FR-1.2 (no trailing commas, one space after `:`).
- It preserves comments (FR-1.3) and the author's line-break shape, and never
  reorders (FR-3.4 / FR-6.2).
- It is pure Rust with no I/O (the `path` argument is only for extension/mode
  detection), so `prim-fmt` can depend on it and stay pure.
- `format_text(path, text, &Configuration) -> Result<Option<String>, anyhow::Error>`:
  `Ok(Some)` reformatted, `Ok(None)` already canonical, `Err` parse failure.

JSON5 (single quotes, unquoted keys, etc.) is not parsed by dprint/jsonc-parser,
so JSON5 input becomes a parse error → file left unchanged (matches "JSON5
excluded").

## Architecture

A new `json` module in `prim-fmt` (not a separate crate). `dprint-plugin-json`
does the parsing and printing; prim's code is thin glue.

```text
prim-fmt (pure)
  json.rs   NEW  format(kind, source, &Style) -> Result<String, FormatError>
                 - map Style -> dprint Configuration
                 - format_text(path, source, &cfg)
                 - on Ok: hand result to hygiene; on Err: FormatError::Parse
  hygiene.rs     hygiene(source, &Style)   (reused for EOL + final newline)
  lib.rs         format(kind, source, &Style) -> Result<String, FormatError>
                 FileKind::Json | Jsonc -> json::format
                 others                  -> Ok(hygiene(..))
  error.rs  NEW  FormatError (thiserror)
```

The issue suggested "likely its own `prim-json` crate"; rejected for now on
YAGNI grounds — with dprint doing the heavy lifting the module is ~60 lines. The
`format` dispatch `match` remains the per-format attach point; a later split
into a `prim-json` crate is mechanical if the module grows.

## The format API becomes fallible

```rust
// prim-fmt/src/error.rs
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// The source could not be parsed as its format.
    #[error("{0}")]
    Parse(String),
}
```

`format(kind, source, &Style) -> Result<String, FormatError>`:

- Hygiene-only kinds (Markdown, YAML, TOML, Orphan today) →
  `Ok(hygiene(source, style))`.
- `Json` / `Jsonc` → `json::format(...)`, which returns
  `Ok(hygiene(printed, style))` or `Err(FormatError::Parse(message))` (carrying
  dprint's message + location).

`FormatError` is part of the public contract (AGENTS.md: `prim-fmt` uses
`thiserror`); it gains variants as YAML/TOML land.

## Composition & configuration mapping

`json::format`:

1. Build a dprint `Configuration` from `Style`:
   - `indent_width(n)` + `use_tabs` from `style.indent` (`Spaces(n)` / `Tab`),
   - `line_width = style.max_line_length.unwrap_or(80)`,
   - newline kind = LF (hygiene converts to CRLF when configured),
   - trailing commas = never (FR-1.2).
2. `format_text(path, source, &cfg)` → `Some(s)` (use `s`) / `None` (already
   canonical, use `source`) / `Err` → `FormatError::Parse`.
3. Return `Ok(hygiene(&candidate, style))` so `end_of_line` and
   `insert_final_newline` come from `Style` uniformly across all formats. dprint
   output has no trailing whitespace, so hygiene's trim is a no-op.

The exact `ConfigurationBuilder` method names are verified against the crate
source during planning (as was done for `ec4rs`).

## Decisions

1. **`json` module, not a separate crate** (YAGNI; thin glue over dprint).
2. **`.json` and `.jsonc` are formatted identically** by the comment-preserving
   printer — comments are preserved even in `.json` (lenient, semantics-
   preserving) rather than rejected. The `FileKind` distinction matters only at
   classification.
3. **stdin (`--stdin-filepath`) parse error** → emit the original source
   unchanged to stdout (so format-on-save never blanks an editor buffer), report
   to stderr, exit `2`.
4. **In-place parse error** mirrors today's non-UTF-8 handling: an explicitly
   named owned file → error + exit `2`; a discovered file → warning; the file is
   left byte-for-byte unchanged in both cases.
5. **Path passed to `format_text`** carries the extension matching the
   `FileKind` so dprint selects the right mode; verified during implementation
   that `.json` with comments is formatted (not errored) — if dprint rejects it,
   fall back to always using the JSONC mode.

## Alternatives considered

- **`jsonc-parser` (CST) + a hand-written printer.** Rejected: fewer deps but
  ~hundreds of lines to own, with comment re-attachment the tricky part.
  dprint's printer is mature and the repo already trusts it.
- **Hand-roll tokenizer + printer.** Rejected on minimum-code grounds; unicode
  escapes, number fidelity, and comment attachment are all easy to get subtly
  wrong.
- **Separate `prim-json` crate now.** Deferred — the glue does not justify a
  crate boundary yet.
- **Keep `format` infallible (return `Option`/passthrough on bad JSON).**
  Rejected: a typed `FormatError` carries the parse message for reporting and is
  the public contract AGENTS.md anticipates.

## Testing strategy (strict TDD, real fixtures, red→green)

- **`prim-fmt` `json` units:** re-indent to `Style` width (2 and 4 spaces,
  tabs); one space after `:`; trailing comma dropped; comments preserved in
  position (JSONC); idempotency; invalid JSON → `Err(Parse)`;
  `end_of_line = crlf` via Style.
- **`prim-fmt` `format`/dispatch units:** `Json`/`Jsonc` route to
  `json::format`; other kinds remain hygiene-only; the new `Result` return type.
- **`prim-cli` behavioural (`tests/json.rs`, `assert_cmd`):** in-place reformat
  of a messy JSON; `--check` flags a non-canonical file (exit 1);
  `--stdin-filepath` round-trips and honors a sibling `.editorconfig`
  `indent_size = 4`; invalid JSON left unchanged with explicit→exit 2 /
  discovered→warning; stdin invalid → original echoed + exit 2.
- **Dogfood:** `prim --check .` stays exit 0 after prim formats the repo's JSON
  files (`.markdownlint.json`, `dprint.json`) in this PR.

## File / module plan

| File                                                                              | Change                                             |
| --------------------------------------------------------------------------------- | -------------------------------------------------- |
| `crates/prim-fmt/src/error.rs`                                                    | NEW — `FormatError` (thiserror)                    |
| `crates/prim-fmt/src/json.rs`                                                     | NEW — `format(kind, source, &Style) -> Result<…>`  |
| `crates/prim-fmt/src/lib.rs`                                                      | `mod error/json`; re-export; fallible `format`     |
| `crates/prim-fmt/Cargo.toml`                                                      | add `dprint-plugin-json`, `thiserror`              |
| `crates/prim-cli/src/app.rs`                                                      | handle `Result` at both call sites (report + exit) |
| `crates/prim-cli/tests/json.rs`                                                   | NEW — behavioural coverage                         |
| `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`, `docs/decisions/0003-*.md` | docs                                               |

## Out of scope (this issue)

- YAML (#11) and TOML (#10) structured passes; Markdown (#12).
- JSON5.
- `--diff` rendering (#14) — the diff path stays a no-op here.
- Extraction into a `prim-json` crate (revisit if the module grows).
