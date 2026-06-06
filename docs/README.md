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

prim is at an **early** stage. Recursive file discovery and the format-agnostic
**whitespace hygiene** pass (trailing-whitespace removal, single final
line-feed, LF endings) are implemented across the parsed formats and the orphan
allowlist, wired through the [`prim-fmt`](https://docs.rs/prim-fmt) engine. The
structured per-format passes (JSON/YAML/TOML/Markdown) and `.editorconfig`
resolution land in later milestones. See the [Specification](SPEC.md) for the
full v1 scope.
