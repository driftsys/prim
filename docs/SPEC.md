# Specification (v1)

> This is the human-readable v1 requirements specification for prim. It mirrors
> [issue #1](https://github.com/driftsys/prim/issues/1). Code and tests remain
> the source of truth; this document describes the intended system.

## Identity

prim is a single-binary, opinionated, near-zero-config formatter for a
repository's connective tissue — Markdown, JSON/JSONC, YAML, TOML — plus
whitespace hygiene on a curated set of un-owned text files. It is **not** a
source-code formatter and has **no plugin system**.

## Settled decisions

| Fork             | Decision                                                               |
| ---------------- | ---------------------------------------------------------------------- |
| Scope            | Config/docs/data only (md, json/jsonc, yaml, toml). No source code.    |
| Config           | One canonical style; honor `.editorconfig`. No `prim.toml`.            |
| Ordering         | Never reorder keys/entries/arrays (semantics-preserving).              |
| Other text files | Hygiene on a curated orphan allowlist, never on source.                |
| Markdown wrap    | Hard-wrap prose to width (`.editorconfig` `max_line_length`, else 80). |
| JSON5            | Excluded (JSONC covers comment needs).                                 |
| `.primignore`    | Yes — committed escape hatch (gitignore syntax).                       |
| Make / Shell     | Out of v1 allowlist; shell deferred to Phase 2 (shfmt/wasm).           |

## FR-1 — Structured formatting

- **FR-1.1** prim shall format Markdown to one canonical style (ATX headings,
  normalized list markers, normalized table padding, normalized blank-line
  spacing) and hard-wrap paragraph prose to the line width — `max_line_length`
  from `.editorconfig`, else 80.
- **FR-1.1a** _(wrap guardrails)_ prim shall wrap prose paragraphs only; it
  shall not break inside inline code, shall not split a URL or link, shall not
  wrap tables or fenced code blocks, and shall preserve explicit hard line
  breaks (trailing `\` or two-space).
- **FR-1.2** prim shall format JSON to a canonical style (consistent
  indentation, one space after `:`, no trailing commas).
- **FR-1.3** prim shall format JSONC, preserving all comments in position.
  `.json` files are parsed with the same lenient JSONC parser: comments and
  trailing commas are accepted on input and never emitted (AD-0003). (JSON5
  excluded.)
- **FR-1.4** prim shall format YAML, preserving comments, anchors/aliases, and
  multi-line scalar styles.
- **FR-1.5** prim shall format TOML, preserving comments and inline-table style.
- **FR-1.6** prim shall preserve fenced code-block contents verbatim (no
  reformatting of embedded source).

## FR-2 — Text hygiene (parsed formats + orphan allowlist)

- **FR-2.1** For every file it processes, prim shall remove trailing whitespace
  from each line, unless `.editorconfig` sets `trim_trailing_whitespace = false`
  (FR-3.2 takes precedence).
- **FR-2.2** prim shall ensure each processed file ends with exactly one
  line-feed.
- **FR-2.3** prim shall normalize line endings to LF, unless `.editorconfig`
  sets `end_of_line = crlf`.
- **FR-2.4** _(scope)_ prim shall process only (a) the parsed formats
  (md/json/jsonc/yaml/toml) and (b) a built-in orphan allowlist of un-owned text
  files. Every other file — recognized source code, unknown types, binaries — is
  left byte-for-byte unchanged.
- **FR-2.5** prim shall identify allowlisted files by filename/extension, not
  content sniffing.

## FR-3 — Style resolution

- **FR-3.1** prim shall apply its built-in canonical style with no config file
  present.
- **FR-3.2** prim shall read `.editorconfig` and honor `indent_style`,
  `indent_size`, `max_line_length`, `end_of_line`, `insert_final_newline`,
  `trim_trailing_whitespace` — including the `root=true` chain and per-glob
  sections. (`charset` is out of scope: prim processes UTF-8 only — FR-6.5,
  AD-0002.)
- **FR-3.3** prim shall expose no other style configuration (no `prim.toml`, no
  per-rule flags).
- **FR-3.4** prim shall never reorder keys, table entries, or array elements.

## FR-4 — File discovery

- **FR-4.1** prim shall default to the current working directory, recursively,
  when given no paths.
- **FR-4.2** prim shall respect `.gitignore` and `.ignore` (via the `ignore`
  crate) without invoking git, and shall function in non-git directories.
- **FR-4.3** prim shall process explicit file/directory path arguments.
- **FR-4.4** prim shall respect a committed `.primignore` (gitignore syntax).
- **FR-4.5** prim shall accept CLI exclude globs; a malformed glob is a usage
  error (exit `2`).

## FR-5 — Operating modes (CLI)

- **FR-5.1** _(default)_ prim shall format matched files in place.
- **FR-5.2** `--check` shall write nothing, exit `0` when all files are already
  formatted, exit non-zero when any file would change, and list the files that
  would change.
- **FR-5.3** `--diff` shall print a unified diff of pending changes and write
  nothing; it shall exit `0` whether or not changes are pending (`--check` is
  the CI gate).
- **FR-5.4** With `--stdin-filepath <path>`, prim shall read stdin and write the
  formatted result to stdout. The flag is mutually exclusive with `--check` and
  `--diff`.
- **FR-5.5** _(exit codes)_ `0` = success · `1` = changes needed (`--check`) ·
  `2` = error (parse/IO).

## FR-6 — Correctness & safety

- **FR-6.1** _(idempotency)_ Running prim on its own output shall produce zero
  further changes.
- **FR-6.2** _(semantic preservation)_ Formatting shall not change the parsed
  data model of a JSON/JSONC/YAML/TOML document.
- **FR-6.3** _(fail-safe)_ An unparseable file shall be left byte-for-byte
  unchanged and reported as an error (exit `2`).
- **FR-6.4** _(atomic write)_ prim shall write via a temporary file and atomic
  rename, preserving permission bits.
- **FR-6.5** prim shall process only UTF-8 text; it shall leave non-UTF-8 files
  unchanged and report them.

## NFR — non-functional (targets, tunable)

- **NFR-1** One statically linked binary, zero runtime dependencies.
- **NFR-2** Linux/macOS/Windows on `amd64` + `arm64`.
- **NFR-3** _(determinism)_ identical input → byte-identical output on every
  supported platform.
- **NFR-4** _(throughput)_ format a 5,000-file repository in under 2 s on an
  8-core machine with warm cache, parallelized across files.
- **NFR-5** _(footprint)_ peak memory scales with the largest single file, not
  repository size.

## Non-goals

- No source-code formatting (Rust/JS/TS/Python/Go/…).
- No plugins or user-facing extensibility API.
- No linting/diagnostics beyond format-checking.
- No schema validation.
- No style knobs beyond `.editorconfig`.

## Phase 2 — roadmap (not v1)

- prim _may_ format shell scripts (`*.sh`/`*.bash`) by embedding `shfmt`
  compiled to WebAssembly. This brushes the "no plugins" non-goal and is to be
  decided deliberately at Phase 2 start: prim has no plugin _system_ (no
  user-supplied formatters), but _may embed_ specific curated wasm formatters
  internally.
