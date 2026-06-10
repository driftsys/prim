# AD-0005 — YAML via `pretty_yaml`

## Context

FR-1.4 requires canonical YAML formatting that preserves comments,
anchors/aliases, and multi-line (block) scalar styles, without reordering keys
or sequence entries (FR-3.4) or changing the data model (FR-6.2). Unparseable
input must be left unchanged and reported (FR-6.3). YAML is the hardest format
to round-trip, and the Rust ecosystem has few formatter-grade options.

## Options

**`pretty_yaml` (chosen).** A configurable YAML formatter by g-plane, the YAML
member of the same CST-formatter family (`tiny_pretty` + `yaml_parser`, a rowan
CST) that backs several dprint plugins.
`format_text(input, &FormatOptions) ->
Result<String, SyntaxError>` returns a
parse error directly on invalid YAML — so no separate parse step is needed. It
preserves comments, anchors/aliases, and block (literal `|` / folded `>`) scalar
styles, and never reorders. Pure Rust.

**`yaml-rust2` / `saphyr` / `yaml-peg`.** YAML 1.2 parsers without a canonical,
comment-preserving printer. Using one would mean writing prim's own YAML printer
— anchors, aliases, flow vs block, multi-line scalars — far too much surface.
Rejected.

**`serde_yaml`.** Deprecated, and its value model strips comments and styles.
Rejected.

**Hand-rolled formatter.** Rejected on minimum-code grounds; YAML's round-trip
fidelity is exactly what `pretty_yaml` already solves.

## Decision: `pretty_yaml` as a library, in a `prim-fmt` `yaml` module

`pretty_yaml = "0.6"` is added to `prim-fmt`. The integration lives in a `yaml`
module (mirroring `json`/`toml`). `prim_fmt::format` dispatches `FileKind::Yaml`
to it.

**`Options` mapping from `Style`.** `LayoutOptions.print_width` from
`max_line_length` (default 80); `LayoutOptions.indent_width` from
`Style::indent`; `LayoutOptions.line_break = Lf` (the existing `hygiene` pass
owns end-of-line and final-newline, keeping one source of truth across formats).
`LanguageOptions` defaults are used.

**Tab indentation.** YAML forbids tabs for indentation and `pretty_yaml` has no
tab option, so `Indent::Tab` falls back to a two-space indent.

**Parse errors.** `format_text` returns `Err(SyntaxError)` on invalid YAML,
mapped to `FormatError::Parse`. CLI handling is identical to JSON/TOML (explicit
→ exit 2, discovered → warning, stdin → echo original + exit 2).

## Consequences

`pretty_yaml` (and its transitive `yaml_parser`, `rowan`, `tiny_pretty`) become
`prim-fmt` dependencies — all pure Rust, no FFI. With YAML done, only Markdown
(#12, FR-1.1) remains among the per-format passes. A pre-existing
`.editorconfig` behavioural test that used a `.yaml` file as a hygiene-only
vehicle was retargeted to a `.txt` orphan, which stays hygiene-only regardless
of future per-format passes.

---

Satisfies: FR-1.4 (YAML canonical; comments, anchors/aliases, block scalar
styles preserved), FR-3.4 (no reorder), FR-6.2 (data model unchanged), FR-6.3
(unparseable files unchanged and reported).\
Related: AD-0003 (the fallible `format` API), AD-0004 (TOML via taplo),
`docs/design/system.md`, `crates/prim-fmt/src/yaml.rs`.
