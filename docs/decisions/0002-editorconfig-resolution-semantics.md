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
testable at the unit level immediately. Those parsers have since landed:
`max_line_length` is consumed by all four structured passes — JSON, TOML, YAML
and Markdown — and `indent` by JSON, TOML and YAML, and by orphan-file
diagnostics.

**Per-file resolution, with a per-directory cache added after measurement.**
Resolution is requested per file rather than per directory (a Markdown file
requests it twice, once for `Style` and once for its lint policy). Because the
`.editorconfig` cascade depends only on the file's directory, caching by
directory was identified as possible but deliberately left out under YAGNI:
profile first, and cache only if NFR-4 (5,000 files < 2 s) showed pressure.
`Resolver` now holds that cache, so a directory's cascade is parsed once per
resolver rather than once per file. That is not one parse per config file per
run: `build_cascade` re-reads the chain for each distinct directory, and the
parallel loader gives each worker thread its own resolver. The change was
measured at about 9 % faster in `--check` mode on a 5,000-file tree with a root
`.editorconfig`. That figure is an end-to-end one and nothing reproduces it;
`crates/prim-cli/benches/resolution.rs` reproduces the mechanism instead,
comparing one reused `Resolver` against a cascade parsed per file (AD-0020).
Only reading and parsing is cached — per-glob sections still resolve per file —
so output stays byte-identical, which an equivalence test against
`ec4rs::properties_of` guards.

**Malformed or unreadable `.editorconfig`** — prim falls back to
`Style::default()` and emits a `ui::warning`. The file is not left unprocessed.
This is the fail-safe posture: a bad config file should not silently corrupt
output or block the tool.

That posture was only partly implemented until #153. The line it fell on was not
malformed-versus-unreadable. prim reported a file in the cascade only when
`ec4rs` yields an error while iterating a `ConfigFile`'s sections, which
requires the file to have parsed far enough to have a valid first section
header. `ConfigParser::new_with_path` eagerly parses everything up to and
including that header; an invalid line anywhere in that span fails there,
`ConfigFile::open` propagates it, and `ConfigFiles::open` discards the error and
carries on with the walk. So a file whose first invalid line is the section
header itself — an unclosed `[*.md`, the common `.editorconfig` typo — was
skipped in silence, as was a file that could not be opened at all.

prim now reports both, by walking the same ancestry `ec4rs` walks and naming
what `ConfigFile::open` refused: the same climb, the same call, and the same
stop at the first `root = true` that opens, so a file above a root boundary —
which never reached resolution — is not named. The two faults are still told
apart, because they read differently to whoever has to fix them: an I/O failure
is reported as `unreadable`, anything else as `malformed`.

What does **not** change is resolution. A file `ec4rs` could not open still
resolves as absent, and the rest of the cascade still applies — including any
`.editorconfig` above a `root = true` prim did not get to read. That is what
`ec4rs` does and what any other EditorConfig reader with the same permissions
would do, and #153 was a diagnosis-quality complaint rather than a resolution
one: an ancestor at mode `000` turned `max_line_length = 120` into `unset` with
nothing said. So the tail of the message differs from the section-iteration case
above, which drops the whole cascade to canonical style; here only the one file
drops out.

Every bad `.editorconfig` in the cascade is now reported. What the split above
still decides is **which report, and what it costs**: a file that failed during
section iteration drops the whole cascade to canonical style, while one
`ConfigFile::open` refused drops only itself. One line can land on either side
of it: a byte-order mark immediately before a first-line section header is
stripped when `LineReader` classifies that line but not when `read_section`
re-parses it, so the file is constructed and then errors on line 1 — reported
under the first rule, at the cost of the whole cascade, where an unclosed
`[*.md` in the same position is reported under the second and costs only that
file. That asymmetry is the one described under "Line-level parsing is
reimplemented" below.

(The other warning `build_cascade` can emit, `cannot search for .editorconfig`,
is not about a file in the cascade: it reports the walk failing to start, which
for a relative probe means the working directory could not be determined.)

A skipped file also lets the walk continue past it. If that file carried
`root = true`, prim never sees the boundary and keeps reading `.editorconfig`
files above it, so resolution can gain settings as well as lose them. Measured,
for a file under a `root = true` parent: with the parent readable,
`indent_style` resolves to prim's default; with the same parent unreadable, it
resolves to `tab` inherited from the grandparent. That is still what happens —
the report says which file was skipped, and does not put its settings back.

The report costs one extra `open` attempt per **ancestor directory**, not per
`.editorconfig` — most ancestors hold no config, and the attempt is what
establishes that. Measured on 1944 files across 367 directories, warm cache,
release build: 99.6 ms before, 106.3 ms after, about 6 %. A `root = true`
boundary does not reduce it, because the cost sits in the directories below the
boundary rather than above it. It is affordable because the cascade is cached
per directory, so the walk runs once per directory rather than once per file.

The report is deduplicated per run: one resolver is built per rayon thread, so a
single bad ancestor is otherwise met once per directory per thread. In
`prim lsp` "per run" means per process — the server holds one resolver for its
lifetime — so a bad `.editorconfig` is announced once for the session, on
stderr, which most LSP clients discard.

Absent is told from unopenable by the error rather than by a `stat` of the
candidate. That is cheaper, and wider: an ancestor **directory** prim cannot
search fails the open with `EACCES`, and a `stat` of the candidate inside it
fails for the same reason, so a stat-based guard hid the very case this closes,
one level up. A dangling symlink comes back `NotFound` and stays silent, which
is right — there is no config there to have applied.

`ec4rs` reports a file that is not valid UTF-8 as an I/O error. prim classifies
that as `malformed`: it read those bytes, and they were not `.editorconfig`.
Without the exception the same fault was called `unreadable` here and
`malformed` from the section loop, and the position of the bad bytes decided
which word the reader got.

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

A per-directory cache landed in `prim-cli` (I/O side), where this record
anticipated it belonged. It caches the parsed cascade rather than `Style`, which
is still built per file. The engine API (`format(kind, source, &Style)`) did not
change.

---

Satisfies: FR-3.1 (canonical default), FR-3.2 (`.editorconfig` cascade and
keys), FR-3.3 (no other config surface), FR-2.3 (`end_of_line = crlf` branch).\
Related: AD-0001 (crate boundary), `docs/design/system.md` (resolution mapping
table), `crates/prim-cli/src/editorconfig.rs`.
