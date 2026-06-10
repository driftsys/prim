# AD-0003 — JSON/JSONC via `dprint-plugin-json`, and a fallible `format`

## Context

FR-1.2/1.3 require canonical, comment-preserving formatting for JSON and JSONC
(JSON5 excluded), without reordering keys or array elements (FR-3.4/6.2). This
is the first per-format structured pass, and the first that can fail: an
unparseable file must be left byte-for-byte unchanged and reported (FR-6.3). The
pre-existing `format` signature was infallible (`-> String`), which cannot
express a parse failure.

## Options for the formatter

**`dprint-plugin-json` (chosen).** The dprint JSON formatter, used as a library.
Its defaults already satisfy FR-1.2 (one space after `:`, and with
`TrailingCommaKind::Never`, no trailing commas), it preserves comments (FR-1.3)
and the author's line-break shape, and it never reorders. It is pure Rust with
no I/O (the `path` argument only selects a parse mode), so `prim-fmt` stays
pure. This is the same engine the repository already uses via dprint, so prim's
JSON output matches the established style.

**`jsonc-parser` (CST) + a hand-written printer.** dprint's lower-level parser
gives a comment-bearing CST, but prim would own the canonical printer —
~hundreds of lines, with comment re-attachment the tricky part. Rejected on
minimum-code grounds.

**Hand-rolled tokenizer + printer.** Rejected: unicode escapes, number fidelity,
and comment attachment are all easy to get subtly wrong, for no benefit over the
mature dprint printer.

JSON5 (single quotes, unquoted keys, trailing commas as syntax) is not parsed by
jsonc-parser, so JSON5 input becomes a parse error and the file is left
unchanged — consistent with "JSON5 excluded".

## Decision: `dprint-plugin-json` as a library, in a `prim-fmt` `json` module

`dprint-plugin-json = "0.21"` is added to `prim-fmt`. The integration lives in a
`json` module (not a separate `prim-json` crate — the glue is ~60 lines; YAGNI).
`prim_fmt::format` dispatches `FileKind::Json | FileKind::Jsonc` to it; both
kinds are formatted identically as JSONC, so comments are preserved even in
`.json` (lenient and semantics-preserving) rather than rejected. `Style` maps to
a dprint `Configuration` (`indent_width`/`use_tabs` from `Style::indent`,
`line_width` from `max_line_length` defaulting to 80,
`trailing_commas = never`). The line ending is **not** set on dprint; the
existing `hygiene` pass owns end-of-line and final-newline normalization,
keeping one source of truth for `Style`'s whitespace semantics across all
formats.

## Decision: `format` becomes fallible

`format(kind, source, &Style)` now returns `Result<String, FormatError>`.
`FormatError` is a public `thiserror` enum with a single `Parse(String)` variant
(carrying the parser's message and location), and will gain variants as YAML and
TOML land. The CLI maps a parse error to prim's existing fail-safe posture:

- **In-place mode** — an explicitly named file → error + exit `2`; a discovered
  file → warning; the file is left byte-for-byte unchanged either way (mirrors
  the non-UTF-8 handling).
- **`--stdin-filepath` mode** — the original source is echoed to stdout
  unchanged (so an editor's format-on-save never blanks the buffer) and the
  error is reported to stderr, exit `2`.

## Consequences

`dprint-plugin-json` (and its transitive `dprint-core`, `jsonc-parser`, `serde`,
`anyhow`, `text_lines`) become `prim-fmt` dependencies — all pure Rust, no FFI,
preserving the single-static-binary model. The `format` `match` remains the
attach point for the remaining per-format passes; each new parser returns the
same `Result` type. A future split of the `json` module into a `prim-json` crate
is mechanical if it grows.

---

Satisfies: FR-1.2 (JSON canonical), FR-1.3 (JSONC comment-preserving), FR-6.3
(unparseable files unchanged and reported).\
Related: AD-0001 (pure engine crate boundary), AD-0002 (`Style` resolution),
`docs/design/system.md`, `crates/prim-fmt/src/json.rs`.
