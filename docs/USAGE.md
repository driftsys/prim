# Usage

```text
prim [fmt|lint|fix] [OPTIONS] [PATH]...
prim init [PATH]
prim explain <PATH>
```

prim exposes three formatting verbs (AD-0007) plus two utilities: `init` (repo
setup) and `explain` (config introspection). Bare `prim [PATH]...` is a
permanent alias for `prim fmt [PATH]...` — no verb is required for the common
case.

| Command   | Writes?              | Purpose                                                                                    |
| --------- | -------------------- | ------------------------------------------------------------------------------------------ |
| `fmt`     | yes (in place)       | Format the parsed formats + whitespace hygiene. Default action.                            |
| `lint`    | never                | Report hygiene and content violations only.                                                |
| `fix`     | yes (in place)       | `fmt` plus autofixable content rules (none yet, so `fix` is currently identical to `fmt`). |
| `init`    | `.editorconfig` only | Scaffold or minimally merge prim's Markdown strict-glob map.                               |
| `explain` | never                | Print the `.editorconfig` settings that apply to one file, and where each came from.       |

## Arguments

| Argument    | Description                                                                                                                                                                                                            |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[PATH]...` | Files or directories to process. Directories are searched recursively (honoring `.gitignore`/`.git/info/exclude`/global gitignore/`.ignore`/`.primignore` by default); defaults to the current directory when omitted. |

## Options

| Flag                            | Verbs                 | Description                                                                                                                                                                                               |
| ------------------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check`                       | `fmt`, `fix`          | Write nothing; exit non-zero if any file would change, and list it.                                                                                                                                       |
| `--diff`                        | `fmt`, `fix`          | Print a unified diff of pending changes; write nothing. Exit `0` on `fmt` regardless of pending changes; exit non-zero on `fix` if a fixable finding is pending (shares `fix --check`'s gated contract).  |
| `--check-idempotence`           | `fmt`                 | Write nothing; for each matched prim-owned file, format it in memory twice with the resolved `.editorconfig` style and exit non-zero if the second pass still changes bytes.                              |
| `--format <json\|sarif>`        | `fmt --check`, `lint` | Emit machine-readable findings to stdout instead of the default plain-text report. Valid only on `fmt --check` and `lint`.                                                                                |
| `--stdin-filepath <PATH>`       | `fmt`, `lint`, `fix`  | Read stdin and process it (format-on-save for `fmt`/`fix`; report for `lint`). Mutually exclusive with `--check`/`--diff`.                                                                                |
| `--exclude <GLOB>`              | all                   | Exclude paths matching the glob (repeatable). A malformed glob is a usage error.                                                                                                                          |
| `--no-ignore`                   | `fmt`, `lint`, `fix`  | Disable only VCS ignore files (`.gitignore`, global gitignore, `.git/info/exclude`). `.primignore`, `--exclude`, and the `.git/` directory prune still apply.                                             |
| `--since <REF>`                 | `fmt`, `lint`, `fix`  | Limit the file set to `git diff --name-only <REF>`: paths that differ between `<REF>` and the current working tree, including staged and unstaged changes (plain two-way diff, no merge-base comparison). |
| `--staged`                      | `fmt`, `lint`, `fix`  | Limit the file set to `git diff --name-only --cached`: paths staged in the git index relative to `HEAD`. Mutually exclusive with `--since`.                                                               |
| `--color <auto\|always\|never>` | all                   | When to use coloured output (default `auto`; `auto` honors `NO_COLOR`).                                                                                                                                   |
| `--completions <SHELL>`         | global                | Generate a shell completion script and print it to stdout.                                                                                                                                                |
| `-h, --help`                    | global                | Print help.                                                                                                                                                                                               |
| `-V, --version`                 | global                | Print version.                                                                                                                                                                                            |

The top-level `--check`, `--diff`, and `--stdin-filepath` flags remain accepted
directly on bare `prim` as **deprecated sugar** for the `fmt` forms: the first
use in a run prints a one-line deprecation warning to stderr. They are scheduled
for removal in v2.0 — the bare `fmt` alias itself is not deprecated.

## Exit codes

| Code | Meaning                                                             |
| ---- | ------------------------------------------------------------------- |
| `0`  | Nothing to do, or already clean.                                    |
| `1`  | Actionable: format drift (`fmt`/`fix --check`) or a `lint` finding. |
| `2`  | prim could not do its job (parse, I/O, or usage error).             |

Warnings never raise the exit code; only errors do.

## Operating modes

- **`fmt` (default)** — format the given files in place.
- **`fmt --check`** (also `fix --check`) — a CI gate: exit `1` and list the
  files that would change. Add `--format json` or `--format sarif` to emit the
  same findings as a machine-readable report instead of the default path list.
- **`--no-ignore`** — keep prim's own filters (`.primignore`, `--exclude`, and
  `.git/` pruning) but ignore VCS ignore files so paths hidden by `.gitignore`,
  global gitignore, or `.git/info/exclude` are walked again.
- **`--since <REF>`** — limit discovery to the paths
  `git diff --name-only <REF>` reports: files that differ between `<REF>` and
  the current working tree, including both staged and unstaged changes. prim
  uses the plain two-way `git diff <REF>` semantics here — no merge-base (`...`)
  comparison.
- **`--staged`** — limit discovery to the paths `git diff --name-only --cached`
  reports: files staged in the git index relative to `HEAD`.
- **Changed-file filters** — `--since` and `--staged` are mutually exclusive.
  They compose by intersection with `--check`, `--diff`, `lint`, `fix`, explicit
  path arguments, `.primignore`, `--exclude`, and `--no-ignore`. Deleted paths
  reported by git are skipped silently, and both flags require the current
  working directory to be inside a git working tree.
- **`fmt --diff`** — preview pending changes without writing; always exits `0`
  (`--check` is the CI gate).
- **`fmt --check-idempotence`** — a formatter self-check: prim formats each
  matched file in memory, reformats that output with the same resolved style,
  and exits `1` only if the second pass still changes bytes. It never writes to
  disk, even when the original file is not already in canonical form. Bare
  `prim --check-idempotence [PATH]...` works too through the permanent `fmt`
  alias.
- **`fix --diff`** — preview pending changes without writing, like `fmt --diff`,
  but exits `1` if a fixable finding is pending — `fix`'s `--check` and `--diff`
  share one gated contract (AD-0007 §4), unlike `fmt --diff`'s preview-only
  behaviour.
- **`lint`** — report-only: prints one finding per violation and never rewrites.
  - For the un-owned-text allowlist (BOM, line endings, trailing whitespace,
    indentation, missing final newline — same set `.editorconfig`/hygiene
    covers), each finding is a coded, positioned diagnostic:
    `path:line:col: message [code]` (e.g.
    `notes.txt:1:6: trailing whitespace
    [hygiene::trailing-whitespace]`).
  - For Markdown, `prim lint` runs rumdl in Standard flavor with prim's own
    severity matrix, selected per file through `.editorconfig`
    `prim_mdlint_strict = true|false` (default `false`). `false` runs the
    always-on floor tier; `true` adds the strict tier and escalates the
    warn-tier floor rules to errors. prim prints each finding as
    `path:line:col: message [MD0xx]`, passes rumdl's rule codes through
    verbatim, never invokes rumdl's formatter/fixer, and does not auto-fix these
    findings in `fix` yet.
    - **Defects / integrity (floor → strict):**

      | Rule  | Floor | Strict |
      | ----- | ----- | ------ |
      | MD045 | warn  | error  |
      | MD042 | error | error  |
      | MD011 | error | error  |
      | MD052 | error | error  |
      | MD056 | error | error  |
      | MD062 | error | error  |
      | MD034 | error | error  |
      | MD057 | error | error  |
      | MD024 | warn  | error  |
      | MD051 | warn  | error  |
      | MD080 | warn  | error  |
      | MD075 | warn  | error  |
      | MD066 | off   | error  |
      | MD068 | off   | error  |
      | MD070 | off   | error  |

    - **Structure / opinion (floor → strict):**

      | Rule                                     | Floor | Strict |
      | ---------------------------------------- | ----- | ------ |
      | MD025 (SUMMARY-safe via `.editorconfig`) | off   | warn   |
      | MD041                                    | off   | warn   |
      | MD001                                    | off   | warn   |
      | MD040                                    | off   | warn   |
      | MD033                                    | off   | warn   |
      | MD026                                    | off   | warn   |
      | MD036                                    | off   | warn   |
      | MD059                                    | off   | warn   |
      | MD053                                    | off   | warn   |
      | MD073                                    | off   | warn   |
      | MD082                                    | off   | warn   |
      | MD067                                    | off   | warn   |

    - **Never linted (formatter territory):** MD003-005, MD007, MD009, MD010,
      MD012, MD018-023, MD027-032, MD035, MD037-039, MD046-050, MD055, MD058,
      MD060, MD064, MD065, MD071, MD076, MD077.
    - **Off in both tiers:** MD013, MD014, MD043, MD044, MD054, MD061, MD063,
      MD069, MD072 (frontmatter key sorting stays off because prim must remain
      semantics-preserving), MD074, MD078, MD079, MD081.
    - Warn-tier Markdown findings still print, but they do **not** raise the
      `prim lint` exit code; only error-tier findings do.
  - JSON/JSONC/YAML/TOML still report the coarser format drift `fmt --check`
    would report; their own content diagnostics are future work.
  - Add `--format json` or `--format sarif` to switch stdout from the plain-text
    report above to a machine-readable document carrying the same findings
    (hygiene, Markdown, and format-drift alike).
- **`--stdin-filepath`** — editor format-on-save: stdin in, formatted stdout out
  (`fmt`/`fix`), or a report (`lint`).
- Naming a path explicitly is strict: a missing file is an error (exit `2`); an
  existing file prim does not own is skipped with a warning.

## `prim init`

`prim init [PATH]` scaffolds or minimally merges `.editorconfig` in `PATH`
(default `.`). It writes no other file.

With no existing `.editorconfig`, prim writes this exact placement map when no
mdBook is detected:

```ini
root = true
[*.md]
prim_mdlint_strict = false
[docs/**.md]
prim_mdlint_strict = true
[**/SUMMARY.md]
prim_mdlint_strict = false
```

Section order is part of the contract: EditorConfig has no specificity ranking,
so the broader `[*.md]` floor must appear before the stricter middle section,
and `[**/SUMMARY.md]` must come last to opt mdBook summaries back down.

If `PATH/book.toml` exists, prim reads `[book].src` and uses that directory for
the strict middle glob instead of `docs/`; for example, `src = "guide"` yields
`[guide/**.md]`. If `book.toml` is present but omits `src`, or is malformed,
prim falls back to mdBook's conventional `src/**.md`.

If `.editorconfig` already exists, prim merges in place without reordering
unrelated content:

- leaves an existing top-level `root = ...` untouched; otherwise prepends
  `root = true` and a blank line
- for `[*.md]`, the detected strict glob, and `[**/SUMMARY.md]`, leaves an
  existing explicit `prim_mdlint_strict = ...` untouched
- if one of those sections exists but lacks the key, appends the key inside that
  section immediately before the next section (or end-of-file)
- if one of those sections is missing entirely, inserts a new block without
  moving existing bytes so the final prim-managed order still reads `[*.md]` →
  strict glob → `[**/SUMMARY.md]` (falling back to end-of-file only when no
  later prim-managed section needs to stay after it)

Running `prim init` twice is idempotent: once the map is present, the second run
reports a no-op and leaves `.editorconfig` byte-identical.

## `prim explain`

`prim explain <PATH>` prints every `.editorconfig` setting that applies to
`PATH`, its effective value, and where that value came from: a specific
`.editorconfig` file and line (with the `[glob]` section it came from, when one
could be recovered), or `prim's default` when no `.editorconfig` entry set it.
`PATH` need not exist — resolution is name/extension-based, the same
classification `fmt`/`lint`/`fix` use, so `explain` also works for a
not-yet-created file to preview what settings it would get.

```console
$ prim explain docs/USAGE.md
docs/USAGE.md
  end_of_line              = lf         (/repo/.editorconfig:5 [*])
  trim_trailing_whitespace = true       (/repo/.editorconfig:7 [*])
  insert_final_newline     = true       (/repo/.editorconfig:6 [*])
  indent_style             = space      (/repo/.editorconfig:8 [*])
  indent_size              = 2          (/repo/.editorconfig:9 [*])
  max_line_length          = 80         (/repo/.editorconfig:12 [*.md])
  prim_mdlint_strict       = false      (prim's default)
```

The settings shown depend on the file's kind: un-owned text files (the
[Orphan allowlist](#what-prim-formats)) only get the three universal hygiene
settings (`end_of_line`, `trim_trailing_whitespace`, `insert_final_newline`);
Markdown additionally shows `prim_mdlint_strict`. A path prim does not format at
all reports a warning (`not a file type prim formats; skipped`) and prints no
settings, but still exits `0` — `explain` never gates a build.

## Machine-readable output

`--format json` and `--format sarif` are available only on `prim fmt --check`
and `prim lint`. They change only stdout; warnings, parse errors, missing-path
errors, and deprecation warnings still go to stderr exactly as they do in the
default plain-text modes.

### JSON schema

prim's JSON report is intentionally small and stable:

```json
{
  "version": 1,
  "mode": "lint",
  "findings": [
    {
      "path": "doc.json",
      "code": "format::drift",
      "message": "does not match prim's canonical format (run `prim fmt` to fix; content-rule diagnostics land with story G2)"
    },
    {
      "path": "notes.txt",
      "code": "hygiene::trailing-whitespace",
      "message": "trailing whitespace",
      "line": 1,
      "column": 6
    }
  ]
}
```

- `version` is the report-schema version, starting at `1`.
- `mode` is `fmt-check` or `lint`.
- `findings` contains one object per reported finding.
- `line` and `column` appear only when prim has a concrete source position.
- `fmt --check` emits `format::drift` findings with the message
  `would be reformatted`.

### SARIF 2.1.0

`--format sarif` emits a SARIF 2.1.0 log for the same findings. `ruleId` matches
prim's stable finding code, `artifactLocation.uri` is the reported path, and
`region.startLine` / `region.startColumn` are included when prim has a
positioned finding.

```json
{
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "prim"
        }
      },
      "results": [
        {
          "ruleId": "hygiene::trailing-whitespace",
          "message": { "text": "trailing whitespace" },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": { "uri": "notes.txt" },
                "region": { "startLine": 1, "startColumn": 6 }
              }
            }
          ]
        }
      ]
    }
  ]
}
```

### GitHub Actions integration

**SARIF upload** and **problem matchers** are separate GitHub features:

- Use `prim ... --format sarif` when you want to upload a SARIF artifact with
  `github/codeql-action/upload-sarif`.
- Use a problem matcher when you want GitHub Actions to parse prim's default
  plain-text `lint` output from the step log.

Example SARIF upload:

```yaml
- name: Run prim lint as SARIF
  run: prim lint --format sarif . > prim.sarif

- name: Upload prim SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: prim.sarif
```

Example problem matcher for plain-text `prim lint` output:

```json
{
  "problemMatcher": [
    {
      "owner": "prim-hygiene",
      "pattern": [
        {
          "regexp": "^([^:]+):(\\d+):(\\d+): (.+) \\[([^\\]]+)\\]$",
          "file": 1,
          "line": 2,
          "column": 3,
          "message": 4,
          "code": 5
        }
      ]
    },
    {
      "owner": "prim-format-drift",
      "pattern": [
        {
          "regexp": "^([^:]+): (does not match prim's canonical format.*)$",
          "file": 1,
          "message": 2
        }
      ]
    }
  ]
}
```

Register it in a workflow step before running prim:

```yaml
- run: echo "::add-matcher::.github/problem-matchers/prim.json"
- run: prim lint .
```

## What prim formats

Parsed formats (structured canonical formatting plus whitespace hygiene), by
extension: `.md`, `.markdown`, `.json`, `.jsonc`, `.yaml`, `.yml`, `.toml`.

Orphan allowlist (whitespace hygiene only) — un-owned text files matched by
exact name or pattern:

| Kind          | Entries                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ignore files  | `.gitignore`, `.gitattributes`, `.dockerignore`, `.npmignore`, `.eslintignore`, `.prettierignore`, `.primignore`, `.helmignore`, `.containerignore` |
| Repo metadata | `CODEOWNERS`, `.mailmap`, `.editorconfig`, `AUTHORS`, `CONTRIBUTORS`, `NOTICE`, `COPYING`, `LICENSE*`                                               |
| Containers    | `Dockerfile`, `Dockerfile.*`, `Containerfile`                                                                                                       |
| Plain text    | `*.txt`, `*.text`                                                                                                                                   |

Everything else — source code, unknown types, binaries — is left byte-for-byte
unchanged. `.env` files are deliberately excluded: their values are data and may
be whitespace-sensitive.

Whitespace hygiene also strips a leading UTF-8 BOM (`U+FEFF`), unconditionally,
from every file prim processes (parsed formats and orphans alike).

## Configuration

prim honors [`.editorconfig`](https://editorconfig.org) as its **only** style
configuration — there is no `prim.toml` and there are no per-rule flags. With no
`.editorconfig` present, prim applies its built-in canonical style (LF endings,
trailing whitespace stripped, exactly one final newline, two-space indent).

Markdown content lint does not add a second config source: `.editorconfig`
remains prim's only user-facing configuration file, including the documented
`prim_*` keys below.

prim resolves the standard `.editorconfig` cascade for each file: it walks up
the directory tree, stops at the nearest `root = true`, and applies matching
per-glob sections (e.g. `[*.md]`). With `--stdin-filepath`, the cascade is
resolved relative to that path's directory.

Honored keys (standard EditorConfig keys plus prim's closed custom-key set):

| Key                        | Effect                                                                      |
| -------------------------- | --------------------------------------------------------------------------- |
| `end_of_line`              | `lf` (default) or `crlf`; the emitted line ending.                          |
| `trim_trailing_whitespace` | `true` (default) strips trailing whitespace; `false` preserves it.          |
| `insert_final_newline`     | `true` (default) keeps one final newline; `false` strips it.                |
| `indent_style`             | `space`/`tab` — drives JSON/JSONC, TOML, and YAML indentation.              |
| `indent_size`              | indent width for the JSON/JSONC, TOML, and YAML formatters.                 |
| `max_line_length`          | line width for the structured formatters (default 80).                      |
| `prim_mdlint_strict`       | `false` (default) = floor tier; `true` = add strict tier for Markdown lint. |

Scope notes:

- prim treats files as UTF-8; `charset` values other than `utf-8` are not
  supported (a non-UTF-8 file is left unchanged and reported).
- `end_of_line = cr` (bare carriage return) is treated as `lf`.
- `prim_mdlint_strict` is currently the **only** documented `prim_*` key.
- Any other `prim_*` entry is silently ignored. That is intentional: `prim_*` is
  a closed allowlist, not a generic extension hook or a second config file.
- Standard EditorConfig keys and documented `prim_*` keys resolve together for
  the same file; custom keys do not interfere with `Style` resolution.
- An unreadable or malformed `.editorconfig` is ignored with a warning, and the
  built-in canonical style applies.

> **Status:** prim applies whitespace hygiene (trailing-whitespace removal,
> final newline, line endings) — driven by `.editorconfig` — to every file it
> owns, and structured canonical formatting to all of its parsed formats:
> JSON/JSONC (consistent indentation, one space after `:`, no trailing commas),
> TOML (canonical spacing, inline-table style preserved), YAML (canonical layout
> with anchors/aliases and block scalar styles preserved), and Markdown (ATX
> headings, normalized lists/tables, and prose hard-wrapped to `max_line_length`
> with guardrails — inline code, links, tables, and fenced code are never
> broken, and fenced code is preserved verbatim). All formats preserve comments
> and never reorder. See the [Specification](SPEC.md).

## Format notes

- `.json` files are parsed leniently as JSONC: comments and trailing commas are
  accepted on input (trailing commas are removed on output). prim never rejects
  a `.json` file for containing comments (AD-0003).
