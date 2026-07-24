# prim

**prim** is a single-binary, opinionated, near-zero-config formatter for a
repository's _connective tissue_ — Markdown, JSON/JSONC, YAML, TOML — plus
whitespace hygiene on a curated set of un-owned text files.

It is **not** a source-code formatter and has **no plugin system**. Think of it
as the tool that tidies the files no other formatter owns.

- **One canonical style.** No `prim.toml`, no per-rule knobs — prim honors
  `.editorconfig` and nothing else.
- **Semantics-preserving.** prim never reorders keys, table entries, or array
  elements, and never changes the parsed data model of a document.
- **Safe by default.** Unparseable or non-UTF-8 files are left byte-for-byte
  unchanged and reported.

## Project status

All v1 requirements are implemented: recursive file discovery, the
format-agnostic **whitespace hygiene** pass (trailing-whitespace removal, single
final line-feed, LF endings, leading-BOM strip), `.editorconfig` style
resolution, and structured formatting for JSON/JSONC, YAML, TOML, and Markdown
(with prose-wrap guardrails) — all wired through the
[`prim-fmt`](https://docs.rs/prim-fmt) engine. prim formats its own Markdown.

Beyond v1, prim also ships `prim lint` (hygiene plus Markdown content
diagnostics, with JSON/SARIF machine-readable output), `prim fix`, `prim init`
(`.editorconfig` scaffolding), `prim explain` (per-file config provenance), and
`prim lsp` (an LSP formatting-and-diagnostics server for editor integration).
See the [Specification](SPEC.md) for the full v1 scope and [Usage](USAGE.md) for
the complete command reference.
