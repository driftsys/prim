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

**`indent` and `max_line_length` — carried before there was a consumer.** Both
fields were populated from `.editorconfig` and stored in `Style` while the
whitespace-hygiene pass, then the only pass, ignored them. They were carried for
the per-format parsers (FR-1, issues #9–12) on two grounds: adding them to
`Style` later would have broken its API, and carrying them made resolution
testable at the unit level immediately. Those parsers have since landed and both
fields are consumed — `max_line_length` by the JSON, TOML and Markdown passes,
`indent` by the JSON, TOML and YAML passes and by orphan-file diagnostics.

**Per-file resolution, with a per-directory cache added after measurement.**
Resolution is requested once per file. Because the `.editorconfig` cascade
depends only on the file's directory, caching by directory was identified as
possible but deliberately left out under YAGNI: profile first, and cache only if
NFR-4 (5,000 files < 2 s) showed pressure. `Resolver` now holds that cache, so a
repository parses each `.editorconfig` once rather than once per file; the
change was measured at about 9 % faster on a 5,000-file tree with a root
`.editorconfig`. Only reading and parsing is cached — per-glob sections still
resolve per file — so output stays byte-identical, which an equivalence test
against `ec4rs::properties_of` guards.

**Malformed or unreadable `.editorconfig`** — prim falls back to
`Style::default()` and emits a `ui::warning`. The file is not left unprocessed.
This is the fail-safe posture: a bad config file should not silently corrupt
output or block the tool.

That posture is only partly implemented, and the line it actually falls on is
not malformed-versus-unreadable. prim reports a file in the cascade only when
`ec4rs` yields an error while iterating a `ConfigFile`'s sections, which
requires the file to have parsed far enough to have a valid first section
header. `ConfigParser::new_with_path` eagerly parses everything up to and
including that header; an invalid line anywhere in that span fails there,
`ConfigFile::open` propagates it, and `ConfigFiles::open` discards the error and
carries on with the walk. So a file whose first invalid line is the section
header itself — an unclosed `[*.md`, the common `.editorconfig` typo — is
skipped in silence, as is a file that cannot be opened at all. An unreadable
file is indistinguishable from one broken at or above its first header.

The split is therefore not "broken line after the first header" but "parsed far
enough to be constructed, then errored during section iteration". One line can
satisfy both: a byte-order mark immediately before a first-line section header
is stripped when `LineReader` classifies that line but not when `read_section`
re-parses it, so the file is constructed and then reports on line 1. That
asymmetry is the one described under "Line-level parsing is reimplemented"
below, and it is the only case where an invalid first header is reported rather
than passed over.

(The other warning `build_cascade` can emit, `cannot search for .editorconfig`,
is not about a file in the cascade: it reports the walk failing to start, which
for a relative probe means the working directory could not be determined.)

Both silent cases also let the walk continue past the skipped file. If that file
carried `root = true`, prim never sees the boundary and keeps reading
`.editorconfig` files above it, so resolution can gain settings as well as lose
them. Measured, for a file under a `root = true` parent: with the parent
readable, `indent_style` resolves to prim's default; with the same parent
unreadable, it resolves to `tab` inherited from the grandparent.

Closing this needs prim to notice an `.editorconfig` it can `stat` but cannot
turn into a `ConfigFile`, on a resolution path that runs per directory during a
walk. The decision above stands as the intended posture; the gap between it and
the implementation is tracked as issue #153.

**Line-level parsing is reimplemented, not called, and is pinned to `ec4rs`'s
`ConfigParser`, not its private `parse_line`.** `.editorconfig`-writing and
`.editorconfig`-explaining code (`prim init`'s section scan, `prim explain`'s
provenance lookup) each need to classify one raw line as a section header, a
key/value pair, or neither — the same job `ec4rs`'s own line parser does. That
parser lives in a private module `ec4rs` does not export, so prim carries its
own copy (`crates/prim-cli/src/editorconfig/line.rs`) rather than depending on
an unstable internal. Two independent hand-rolled copies of that job existed
before and diverged from `ec4rs` and from each other on a section header's
brackets and trailing comments, so `prim init` could write a key into a section
the resolver did not actually see (issue #117). `ConfigParser` — public, and the
thing that actually resolves prim's settings — is the authority prim's copy is
checked against, not `parse_line` directly, because agreeing with the private
function would not guarantee agreeing with what prim's own resolution depends
on. `ConfigParser` also contradicts `parse_line` in one place: a UTF-8 BOM on a
file's first line is stripped before classifying a key/value pair, but not
before re-parsing a section header, so a BOM'd first-line header is invalid
while a BOM'd first-line pair is not. prim's copy deliberately mirrors that
asymmetry rather than resolving it, to stay agreeing with `ConfigParser`.
Holding the copy to this is a differential test that drives real `ec4rs` through
`ConfigParser` and checks prim's verdict against it line by line; it, not a
compiler check, is what must be re-verified — reading `ec4rs`'s `linereader.rs`
again and updating both if it changed — on every `ec4rs` version bump, including
one that only changes the lockfile.

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
