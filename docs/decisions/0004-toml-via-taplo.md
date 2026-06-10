# AD-0004 — TOML via `taplo`

## Context

FR-1.5 requires canonical TOML formatting that preserves comments and
inline-table style, without reordering keys or table entries (FR-3.4) or
changing the data model (FR-6.2). Unparseable input must be left unchanged and
reported (FR-6.3). This is the second per-format structured pass and reuses the
fallible `format` API and `hygiene` composition established for JSON (AD-0003).

## Options

**`taplo` (chosen).** The canonical TOML formatter (the engine behind the "Even
Better TOML" tooling), used as a library. It canonicalizes spacing and
indentation, preserves comments, and exposes per-option control over reordering
and inline-table expansion. It is pure Rust; the formatter lives in taplo's core
crate (only the `serde` default feature — no LSP or schema machinery).

**`toml_edit` (cargo's format-preserving CST).** Preserves comments, inline
tables, and order, but _preserves_ the author's existing formatting rather than
canonicalizing it. Producing a canonical style would require prim to write its
own normalization rules over the CST. Rejected: more code, weaker
canonicalization, when taplo already canonicalizes.

**Hand-rolled parser + printer.** Rejected on minimum-code grounds; the TOML
grammar plus comment and inline-table fidelity are easy to get subtly wrong.

## Decision: `taplo` as a library, in a `prim-fmt` `toml` module

`taplo = "0.14"` is added to `prim-fmt`. The integration lives in a `toml`
module (mirroring `json`; not a separate crate). `prim_fmt::format` dispatches
`FileKind::Toml` to it.

**Parse-error detection.** taplo's formatter is lenient — "invalid parts are
skipped" — which would silently mangle malformed input. `toml::format` therefore
calls `taplo::parser::parse(source)` first and returns `FormatError::Parse` when
`parsed.errors` is non-empty; only a clean parse is formatted (via
`format_syntax(parsed.into_syntax(), options)`). The CLI handling is identical
to JSON (explicit → exit 2, discovered → warning, stdin → echo original + exit
2).

**`Options` mapping from `Style`.** `indent_string` from `Style::indent`
(`Spaces(n)` → _n_ spaces, `Tab` → a tab); `column_width` from `max_line_length`
(default 80); `inline_table_expand = false` to preserve inline-table style
(FR-1.5 — taplo defaults this to `true`); `reorder_keys`/`reorder_arrays`/
`reorder_inline_tables = false` (FR-3.4). taplo's `crlf` and `trailing_newline`
are left at their defaults; the existing `hygiene` pass owns end-of-line and
final-newline normalization, keeping one source of truth for `Style`'s
whitespace semantics across all formats. `Options` is built with struct-update
syntax (`..Options::default()`) to keep clippy's `field_reassign_with_default`
satisfied.

**Array layout.** taplo's `array_auto_expand` / `array_auto_collapse` defaults
are kept: arrays are reflowed to fit `column_width`. This changes array _layout_
but never data or order, so it is within "format TOML to a canonical style".

## Consequences

`taplo` (and its transitive `rowan`, `logos`, `serde`, etc.) become `prim-fmt`
dependencies — all pure Rust, no FFI, preserving the single-static-binary model.
Because array collapsing depends on `column_width`, tests that need to observe
per-element indentation set a small `max_line_length` to force expansion. A
future split of the `toml` module into a `prim-toml` crate is mechanical if it
grows.

---

Satisfies: FR-1.5 (TOML canonical, comments + inline-table preserved), FR-3.4
(no reorder), FR-6.2 (data model unchanged), FR-6.3 (unparseable files unchanged
and reported).\
Related: AD-0003 (JSON via dprint-plugin-json; the fallible `format` API),
`docs/design/system.md`, `crates/prim-fmt/src/toml.rs`.
