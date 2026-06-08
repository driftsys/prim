# AD-0002 — EditorConfig resolution: `ec4rs`, semantic choices, and scope cuts

## Context

FR-3 requires prim to honor `.editorconfig` as its only style configuration.
Implementing that requires (a) choosing how to parse and cascade `.editorconfig`
files, and (b) settling the semantics for several keys and edge cases that the
EditorConfig specification leaves ambiguous or where prim's design constrains
the answer.

## Options for the parser/cascade implementation

**Hand-roll the INI parser, glob matcher, and cascade walker.** The EditorConfig
glob grammar includes `{a,b}`, `**`, `[!…]`, and numeric ranges — non-trivial to
get right. The `root = true` chain and property precedence rules add further
surface. Estimated ~300+ lines of fiddly code to own, maintain, and test against
edge cases.

**`ec4rs` (pure Rust).** A pure-Rust crate that descends from the
editorconfig-core test suite. API: `properties_of(path) -> Result<Properties>`;
`Properties::get::<T>()` for typed property access; `use_fallbacks()` for spec
defaults. Zero native dependencies. Passes the upstream compatibility test
suite.

**FFI crates (`editorconfig-rs` / `editorconfig-sys`) wrapping C
`libeditorconfig`.** The canonical reference implementation. Drawback:
introduces a C dependency, which makes cross-compilation for the
single-static-binary distribution (NFR-1) significantly harder or impossible
without pre-built artifacts.

## Decision: use `ec4rs`

`ec4rs` is adopted as the sole EditorConfig dependency (`ec4rs = "1.2"` in
`prim-cli/Cargo.toml`). It solves the implementation problem with minimal owned
code, stays pure Rust (preserving NFR-1), and passes the core test suite. FFI
crates are rejected because a C dependency undermines the single-static-binary
distribution model. Hand-rolling is rejected on minimum-code grounds.

## Semantic decisions

The following choices apply to specific EditorConfig keys or edge cases.

**`insert_final_newline = false`** — when set, prim strips all trailing newlines
so the file ends with content and no line ending. This is the literal reading of
the EditorConfig specification ("ensure the file does not end with a newline").
`true` (the default) preserves today's behaviour: exactly one final newline.

**`end_of_line = cr`** (bare carriage-return, deprecated by EditorConfig) — prim
maps this to `Lf`. FR-2.3 carves out only `crlf` as an explicit exception to LF
normalization. The deprecated `cr` value has no valid use case in prim's target
file types and falls through to the canonical LF default.

**`charset` — out of scope.** prim is a UTF-8-only formatter. Non-UTF-8 files
are already left unchanged and reported (FR-6.5). Supporting `utf-8-bom`,
`latin1`, or `utf-16*` would require transcoding, which prim does not do.
`charset` is not carried in `Style` (no consumer, no testable application). This
is a deliberate scope cut, not an oversight.

**`indent` and `max_line_length` — resolved and carried, not yet consumed.**
Both fields are populated from `.editorconfig` and stored in `Style`, but the
whitespace-hygiene pass does not consume them. They are available to the
per-format parsers (FR-1, issues #9–12) when those land. Carrying them now
avoids an API break later and makes resolution testable at the unit level today.

**Per-file resolution; no per-directory cache.** `editorconfig::resolve` is
called once per file. The `.editorconfig` cascade depends on the file's
directory path, so caching by directory is possible but not implemented. YAGNI
applies: profile first, cache only if NFR-4 (5,000 files < 2 s) shows pressure.

**Malformed or unreadable `.editorconfig`** — prim falls back to
`Style::default()` and emits a `ui::warning`. The file is not left unprocessed.
This is the fail-safe posture: a bad config file should not silently corrupt
output or block the tool.

## Consequences

`ec4rs` appears as a `prim-cli` dependency. It does not appear in `prim-fmt`.
Any future change to the EditorConfig handling library is isolated to
`prim-cli/src/editorconfig.rs` and does not affect the engine API.

`charset` support, if ever needed, requires an explicit follow-up decision and
likely a pipeline change (prim would need to detect encoding before the UTF-8
read step). It is not a drop-in field addition.

A per-directory `Style` cache, if ever implemented, belongs in `prim-cli` (I/O
side). The engine API (`format(kind, source, &Style)`) does not need to change.

---

Satisfies: FR-3.1 (canonical default), FR-3.2 (`.editorconfig` cascade and
keys), FR-3.3 (no other config surface), FR-2.3 (`end_of_line = crlf` branch).\
Related: AD-0001 (crate boundary), `docs/design/system.md` (resolution mapping
table), `crates/prim-cli/src/editorconfig.rs`.
