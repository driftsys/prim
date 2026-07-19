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
- **FR-2.6** prim shall strip a leading UTF-8 BOM (`U+FEFF`), unconditionally,
  from every file it processes.

## FR-3 — Style resolution

- **FR-3.1** prim shall apply its built-in canonical style with no config file
  present.
- **FR-3.2** prim shall read `.editorconfig` and honor `indent_style`,
  `indent_size`, `max_line_length`, `end_of_line`, `insert_final_newline`,
  `trim_trailing_whitespace`, plus the custom Markdown-lint tier key
  `prim_mdlint_strict = true|false` (default `false`) — including the
  `root=true` chain and per-glob sections. (`charset` is out of scope: prim
  processes UTF-8 only — FR-6.5, AD-0002.)
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
- **FR-4.6** prim shall handle an explicitly named path strictly: a path that
  does not exist shall be reported as an error (exit `2`); an existing path
  whose type prim does not own shall be reported as a warning and left unchanged
  (exit `0`). An unowned path reached only by directory walking shall be skipped
  silently (FR-2.4).

## FR-5 — Operating modes (CLI)

prim exposes three verbs (AD-0007): `fmt`, `lint`, `fix`. Bare `prim [PATH]...`
is a permanent alias for `prim fmt [PATH]...` — no verb is required for the
default, format-in-place action.

- **FR-5.1** _(default)_ `prim fmt` (and its bare alias) shall format matched
  files in place.
- **FR-5.2** `prim fmt --check` (also `fix --check`) shall write nothing, exit
  `0` when all files are already formatted, exit non-zero when any file would
  change, and list the files that would change.
- **FR-5.3** `prim fmt --diff` shall print a unified diff of pending changes and
  write nothing; it shall exit `0` whether or not changes are pending (`--check`
  is the CI gate). `prim fix --diff` shares `fix --check`'s gated contract
  instead (FR-5.2): it also prints the diff and writes nothing, but exits
  non-zero when a fixable finding is pending, since `fix`'s `--check` and
  `--diff` are both format-drift gates, unlike `fmt --diff`'s preview-only
  behaviour (AD-0007 §4).
- **FR-5.4** With `--stdin-filepath <path>` (valid on `fmt`, `lint`, and `fix`),
  prim shall read stdin and, for `fmt`/`fix`, write the formatted result to
  stdout. The flag is mutually exclusive with `--check` and `--diff`.
- **FR-5.5** `prim lint` shall report hygiene and content violations without
  ever rewriting a file; it has neither `--check` nor `--diff` (report-only is
  its only mode).
  - **FR-5.5a** _(hygiene diagnostics, story B1)_ For the un-owned-text
    allowlist (the orphan set, shell excluded — same scope as FR-2.4/2.5),
    `prim lint` shall report each whitespace-hygiene violation individually: a
    leading BOM, a line ending that does not match the resolved `end_of_line`,
    trailing whitespace, an indentation character that contradicts the resolved
    `indent_style`, and a missing final newline (when `insert_final_newline` is
    set). Each finding carries a stable, namespaced diagnostic code
    (`hygiene::bom`, `hygiene::eol`, `hygiene::trailing-whitespace`,
    `hygiene::indent`, `hygiene::final-newline`) and a 1-indexed `file:line:col`
    (`prim_fmt::line_col`, AD-0008), printed as `path:line:col: message [code]`.
    JSON/JSONC/TOML/YAML keep the coarser format-drift finding until their own
    content diagnostics land (D2).
  - **FR-5.5b** _(Markdown content diagnostics, stories G2/G3)_ For Markdown
    files, `prim lint` shall run `rumdl_lib::lint()` in Standard flavor through
    `prim_fmt::lint_markdown`, filtering `rumdl_lib::rules::all_rules(&cfg)` to
    prim's active rule subset by `Rule::name()`. The per-file `.editorconfig`
    key `prim_mdlint_strict = true|false` (default `false`) is resolved through
    the normal EditorConfig cascade; `false` runs the always-on floor tier,
    `true` adds the strict tier and escalates warn-tier floor findings to
    errors. Each finding carries rumdl's rule code verbatim and a 1-indexed
    `path:line:col`, printed as `path:line:col: message [MD0xx]`. This path is
    lint-only: prim shall never invoke rumdl's formatter or auto-fix Markdown
    findings, and `prim fix` does not yet auto-fix these rules.
    - **Severity matrix (floor / strict):**

      | Group               | Rule                                     | Floor | Strict |
      | ------------------- | ---------------------------------------- | ----- | ------ |
      | defects / integrity | MD045                                    | warn  | error  |
      | defects / integrity | MD042                                    | error | error  |
      | defects / integrity | MD011                                    | error | error  |
      | defects / integrity | MD052                                    | error | error  |
      | defects / integrity | MD056                                    | error | error  |
      | defects / integrity | MD062                                    | error | error  |
      | defects / integrity | MD034                                    | error | error  |
      | defects / integrity | MD057                                    | error | error  |
      | defects / integrity | MD024                                    | warn  | error  |
      | defects / integrity | MD051                                    | warn  | error  |
      | defects / integrity | MD080                                    | warn  | error  |
      | defects / integrity | MD075                                    | warn  | error  |
      | defects / integrity | MD066                                    | off   | error  |
      | defects / integrity | MD068                                    | off   | error  |
      | defects / integrity | MD070                                    | off   | error  |
      | structure / opinion | MD025 (SUMMARY-safe via `.editorconfig`) | off   | warn   |
      | structure / opinion | MD041                                    | off   | warn   |
      | structure / opinion | MD001                                    | off   | warn   |
      | structure / opinion | MD040                                    | off   | warn   |
      | structure / opinion | MD033                                    | off   | warn   |
      | structure / opinion | MD026                                    | off   | warn   |
      | structure / opinion | MD036                                    | off   | warn   |
      | structure / opinion | MD059                                    | off   | warn   |
      | structure / opinion | MD053                                    | off   | warn   |
      | structure / opinion | MD073                                    | off   | warn   |
      | structure / opinion | MD082                                    | off   | warn   |
      | structure / opinion | MD067                                    | off   | warn   |

    - **Never linted (formatter territory):** MD003-005, MD007, MD009, MD010,
      MD012, MD018-023, MD027-032, MD035, MD037-039, MD046-050, MD055, MD058,
      MD060, MD064, MD065, MD071, MD076, MD077.
    - **Off in both tiers:** MD013, MD014, MD043, MD044, MD054, MD061, MD063,
      MD069, MD072 (frontmatter key sorting would violate prim's
      semantics-preserving guardrail), MD074, MD078, MD079, MD081.
    - **Exit-code implication:** warn-tier Markdown findings are still printed
      (and appear in JSON/SARIF output), but only error-tier findings raise
      `prim lint`'s exit code to `1`.
- **FR-5.6** _(exit codes)_ `0` = nothing to do / already clean · `1` =
  actionable — format drift (`fmt`/`fix --check`) or a lint finding · `2` = prim
  could not do its job (parse/IO/usage error). Warnings never raise the exit
  code; only errors do.
- **FR-5.7** _(deprecated top-level flags)_ The top-level `--check`, `--diff`,
  and `--stdin-filepath` flags remain accepted directly on bare `prim` as
  deprecated sugar for the `fmt` forms; the first use in a run emits a one-line
  deprecation warning to stderr. They are scheduled for removal in v2.0; the
  bare alias itself is not deprecated.
- **FR-5.8** _(machine-readable reports, story D2)_ `--format <json|sarif>`
  shall be accepted only on `prim fmt --check` and `prim lint`. It changes only
  stdout for those report-only modes: write behaviour and exit codes are
  unchanged, and warnings/errors remain on stderr. Without `--format`, the
  existing plain-text stdout for `fmt --check` and `lint` remains unchanged.
  - **FR-5.8a** `--format json` shall emit a stable JSON document of the form
    `{ "version": 1, "mode": "fmt-check"|"lint", "findings": [...] }`. Each
    finding includes `path`, `code`, and `message`; positioned findings also
    include 1-indexed `line` and `column`. `fmt --check` reports one
    `format::drift` finding per file that would change, with the message
    `"would be reformatted"`. `prim lint` reports the existing coarse structured
    format drift as `format::drift`, plus the B1 hygiene diagnostics for orphan
    files with their stable `hygiene::*` codes and positions.
  - **FR-5.8b** `--format sarif` shall emit a valid SARIF 2.1.0 log with one
    result per finding. Each result's `ruleId` shall match the stable `code`,
    `artifactLocation.uri` shall be the reported file path, and
    `region.startLine` / `region.startColumn` shall be present whenever the
    finding has a known position.

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

## Style stability

The canonical style is a compatibility contract. Any change to prim's output for
already-canonical input — including a change inherited from a formatter
dependency upgrade (`dprint-plugin-json`, `dprint-plugin-markdown`, `taplo`,
`pretty_yaml`) — is a versioned, release-noted event: a **minor** version bump
while prim is pre-1.0, a **major** bump once prim reaches 1.0. The release notes
must call out the changed output explicitly so downstream `prim --check` gates
upgrade deliberately. The fixture harness
(`crates/prim-fmt/tests/correctness/fixtures/`) enforces this: its
`spec_cases_format_as_expected` test byte-compares formatter output against each
fixture's committed `-- expected --` section, so canonical-output drift fails
the build until it is reverted, or deliberately regenerated with
`PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness
spec_cases_format_as_expected`,
reviewed in the diff, and released as above.

## Non-goals

- No source-code formatting (Rust/JS/TS/Python/Go/…).
- No plugins or user-facing extensibility API.
- No schema validation or generalized lint framework beyond the documented
  whitespace-hygiene and Markdown-content checks.
- No style knobs beyond `.editorconfig`.

## Phase 2 — roadmap (not v1)

- prim _may_ format shell scripts (`*.sh`/`*.bash`) by embedding `shfmt`
  compiled to WebAssembly. This brushes the "no plugins" non-goal and is to be
  decided deliberately at Phase 2 start: prim has no plugin _system_ (no
  user-supplied formatters), but _may embed_ specific curated wasm formatters
  internally.
