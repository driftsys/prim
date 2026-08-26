# Design — TOML formatter (FR-1.5) · issue #10

**Status:** approved (brainstorm) · **Branch:** `feat/toml-format` · **Epic:**
#4

Canonical, comment- and inline-table-preserving TOML formatting, dispatched from
`prim_fmt::format`. Reuses the fallible `format` API and `hygiene` composition
introduced for JSON (#9, AD-0003); the only new code is a `toml` module wrapping
the `taplo` formatter.

## Requirements (acceptance criteria, from #1)

- **FR-1.5** Format TOML, preserving comments and inline-table style.
- **FR-3.4** Never reorder keys or table entries.
- **FR-6.2** Do not change the parsed data model.
- **FR-6.3** Unparseable files are left byte-for-byte unchanged and reported.

## Decision: use `taplo` as a library

prim formats TOML by calling `taplo` (the canonical TOML formatter behind the
"Even Better TOML" tooling) as a library.

- It canonicalizes spacing and indentation, preserves comments, and with
  `reorder_keys`/`reorder_arrays`/`reorder_inline_tables = false` never reorders
  (FR-3.4) and never changes the data model (FR-6.2).
- It is pure Rust; the formatter lives in taplo's core crate (only the `serde`
  default feature — no LSP/schema machinery).
- `taplo::parser::parse(src) -> Parse { green_node, errors: Vec<Error> }`;
  `taplo::formatter::format_syntax(SyntaxNode, Options) -> String`.

`toml_edit` (format-_preserving_, not canonicalizing) and a hand-rolled
parser/printer were rejected — see Alternatives.

## Architecture

A new `toml` module in `prim-fmt`, mirroring the `json` module. No API change:
`format` is already `Result<String, FormatError>`, and `app.rs` already handles
it — only the dispatch arm is added.

```text
prim-fmt (pure)
  toml.rs   NEW  format(source, &Style) -> Result<String, FormatError>
                 - taplo::parser::parse -> errors? -> FormatError::Parse
                 - map Style -> taplo Options
                 - taplo::formatter::format_syntax(parsed.into_syntax(), opts)
                 - hygiene(printed, style)   (EOL + final newline)
  lib.rs         FileKind::Toml -> toml::format  (new dispatch arm)
```

### `toml::format`

```rust
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let parsed = taplo::parser::parse(source);
    if !parsed.errors.is_empty() {
        let message = parsed
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FormatError::Parse(message));
    }
    // Built via mutable Default so it is robust if Options is #[non_exhaustive].
    let mut options = taplo::formatter::Options::default();
    options.indent_string = match style.indent {
        Indent::Spaces(n) => " ".repeat(n),
        Indent::Tab => "\t".to_string(),
    };
    options.column_width = style.max_line_length.unwrap_or(80);
    options.inline_table_expand = false; // FR-1.5: preserve inline-table style
    options.reorder_keys = false; // FR-3.4
    options.reorder_arrays = false; // FR-3.4
    options.reorder_inline_tables = false; // FR-3.4
    let printed = taplo::formatter::format_syntax(parsed.into_syntax(), options);
    Ok(hygiene(&printed, style))
}
```

`parsed.errors` is checked (a borrow) before `parsed.into_syntax()` (a move).

## Decisions

1. **Parse-error detection via `taplo::parser::parse`.** taplo's `format` is
   lenient ("invalid parts are skipped"), which would silently mangle malformed
   input. prim checks `parsed.errors` first and only formats a clean parse;
   otherwise `FormatError::Parse`. The CLI handling is unchanged from #9
   (explicit → exit 2, discovered → warning, stdin → echo original + exit 2).
2. **`inline_table_expand = false`.** taplo defaults this to `true` (it expands
   inline tables); FR-1.5 mandates preserving inline-table style, so it is
   forced off.
3. **`reorder_* = false`** (FR-3.4) — set explicitly even though they are taplo
   defaults.
4. **Keep taplo's other canonical defaults** — including `array_auto_expand` /
   `array_auto_collapse` (reflow arrays by `column_width`) and `compact_arrays`.
   These change array _layout_ but never data or order, so they fall within
   "format TOML to a canonical style".
5. **`Options` built via mutable `Default`** rather than struct literal, robust
   against `#[non_exhaustive]`.
6. **EOL owned by `hygiene`.** taplo's `crlf` option is left at default; the
   existing hygiene pass converts to `Style::end_of_line` and applies the final
   newline, keeping one source of truth across formats.

## Alternatives considered

- **`toml_edit` (cargo's format-preserving CST).** Preserves comments, inline
  tables, and order, but _preserves_ the author's existing formatting rather
  than canonicalizing it — "format TOML to a canonical style" (FR-1.5) would
  require writing prim's own normalization rules on top of the CST. Rejected:
  more code, weaker canonicalization, when taplo already canonicalizes.
- **Hand-rolled parser + printer.** Rejected on minimum-code grounds; the TOML
  grammar plus comment and inline-table fidelity are easy to get subtly wrong.
- **Force arrays to preserve layout** (`array_auto_expand`/`collapse = false`).
  Rejected — taplo's width-based reflow is the canonical behaviour and changes
  no data; preserving arbitrary author layout is not an FR-1.5 requirement.

## Testing strategy (strict TDD, real fixtures, red→green)

- **`prim-fmt` `toml` units:** `a=1` → `a = 1` (canonical spacing); indent from
  `Style` (2 / 4 / tab); **inline table preserved** (`x = {a=1}` stays inline);
  comments preserved in position; key order preserved (no reorder); invalid TOML
  → `Err(Parse)`; idempotency; `end_of_line = crlf` via Style.
- **`prim-cli` behavioural (`tests/toml.rs`, `assert_cmd`):** in-place reformat;
  `--check` flags a non-canonical file (exit 1); `.editorconfig`
  `indent_size = 4` honored; inline-table + comment preservation end-to-end;
  invalid TOML explicit→exit 2 / discovered→warning; stdin invalid→echo
  original + exit 2; stdin round-trip.
- **Dogfood:** `prim --check .` stays exit 0 after prim formats the repo's
  `Cargo.toml` files (`Cargo.lock` is not owned). The diff is shown before
  commit.

## File / module plan

| File                                                                              | Change                                                |
| --------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `crates/prim-fmt/src/toml.rs`                                                     | NEW — `format(source, &Style) -> Result<…>` via taplo |
| `crates/prim-fmt/src/lib.rs`                                                      | `mod toml;` + `FileKind::Toml` dispatch arm           |
| `crates/prim-fmt/Cargo.toml`                                                      | add `taplo = "0.14"`                                  |
| `crates/prim-cli/tests/toml.rs`                                                   | NEW — behavioural coverage                            |
| `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`, `docs/decisions/0004-*.md` | docs (gardened inline)                                |
| repo `Cargo.toml` files                                                           | reformatted by dogfood (whitespace only)              |

## Out of scope (this issue)

- YAML (#11) and Markdown (#12) structured passes.
- Honoring `taplo.toml` / any TOML-specific config — prim honors only
  `.editorconfig` (FR-3.3).
- `--diff` rendering (#14).
- Extraction into a `prim-toml` crate (revisit if the module grows).
