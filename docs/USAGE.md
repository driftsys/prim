# Usage

```text
prim [fmt|lint|fix] [OPTIONS] [PATH]...
```

prim exposes three verbs (AD-0007). Bare `prim [PATH]...` is a permanent alias
for `prim fmt [PATH]...` — no verb is required for the common case.

| Verb   | Writes?        | Purpose                                                                                    |
| ------ | -------------- | ------------------------------------------------------------------------------------------ |
| `fmt`  | yes (in place) | Format the parsed formats + whitespace hygiene. Default action.                            |
| `lint` | never          | Report hygiene and content violations only.                                                |
| `fix`  | yes (in place) | `fmt` plus autofixable content rules (none yet, so `fix` is currently identical to `fmt`). |

## Arguments

| Argument    | Description                                                                                                                                                            |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[PATH]...` | Files or directories to process. Directories are searched recursively (honoring `.gitignore`/`.ignore`/`.primignore`); defaults to the current directory when omitted. |

## Options

| Flag                            | Verbs                 | Description                                                                                                                                                                                              |
| ------------------------------- | --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check`                       | `fmt`, `fix`          | Write nothing; exit non-zero if any file would change, and list it.                                                                                                                                      |
| `--diff`                        | `fmt`, `fix`          | Print a unified diff of pending changes; write nothing. Exit `0` on `fmt` regardless of pending changes; exit non-zero on `fix` if a fixable finding is pending (shares `fix --check`'s gated contract). |
| `--format <json\|sarif>`        | `fmt --check`, `lint` | Emit machine-readable findings to stdout instead of the default plain-text report. Valid only on `fmt --check` and `lint`.                                                                               |
| `--stdin-filepath <PATH>`       | `fmt`, `lint`, `fix`  | Read stdin and process it (format-on-save for `fmt`/`fix`; report for `lint`). Mutually exclusive with `--check`/`--diff`.                                                                               |
| `--exclude <GLOB>`              | all                   | Exclude paths matching the glob (repeatable). A malformed glob is a usage error.                                                                                                                         |
| `--color <auto\|always\|never>` | all                   | When to use coloured output (default `auto`; `auto` honors `NO_COLOR`).                                                                                                                                  |
| `--completions <SHELL>`         | global                | Generate a shell completion script and print it to stdout.                                                                                                                                               |
| `-h, --help`                    | global                | Print help.                                                                                                                                                                                              |
| `-V, --version`                 | global                | Print version.                                                                                                                                                                                           |

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
- **`fmt --diff`** — preview pending changes without writing; always exits `0`
  (`--check` is the CI gate).
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
  - For Markdown, `prim lint` runs rumdl in Standard flavor with a fixed active
    subset — `MD034` (no bare URLs), `MD042` (no empty links), and `MD045`
    (images need alt text) — and prints each finding as
    `path:line:col: message [MD0xx]`. prim passes rumdl's rule codes through
    verbatim, never invokes rumdl's formatter/fixer, and does not auto-fix these
    findings in `fix` yet.
  - JSON/JSONC/YAML/TOML still report the coarser format drift `fmt --check`
    would report; their own content diagnostics are future work.
  - Add `--format json` or `--format sarif` to switch stdout from the plain-text
    report above to a machine-readable document carrying the same findings
    (hygiene, Markdown, and format-drift alike).
- **`--stdin-filepath`** — editor format-on-save: stdin in, formatted stdout out
  (`fmt`/`fix`), or a report (`lint`).
- Naming a path explicitly is strict: a missing file is an error (exit `2`); an
  existing file prim does not own is skipped with a warning.

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

Markdown content lint does not add a second config source: its active rumdl
rules are the fixed curated subset above, and `.editorconfig` remains prim's
only user-facing configuration file.

prim resolves the standard `.editorconfig` cascade for each file: it walks up
the directory tree, stops at the nearest `root = true`, and applies matching
per-glob sections (e.g. `[*.md]`). With `--stdin-filepath`, the cascade is
resolved relative to that path's directory.

Honored keys:

| Key                        | Effect                                                             |
| -------------------------- | ------------------------------------------------------------------ |
| `end_of_line`              | `lf` (default) or `crlf`; the emitted line ending.                 |
| `trim_trailing_whitespace` | `true` (default) strips trailing whitespace; `false` preserves it. |
| `insert_final_newline`     | `true` (default) keeps one final newline; `false` strips it.       |
| `indent_style`             | `space`/`tab` — drives JSON/JSONC, TOML, and YAML indentation.     |
| `indent_size`              | indent width for the JSON/JSONC, TOML, and YAML formatters.        |
| `max_line_length`          | line width for the structured formatters (default 80).             |

Scope notes:

- prim treats files as UTF-8; `charset` values other than `utf-8` are not
  supported (a non-UTF-8 file is left unchanged and reported).
- `end_of_line = cr` (bare carriage return) is treated as `lf`.
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
