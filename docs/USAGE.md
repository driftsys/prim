# Usage

```text
prim [OPTIONS] [PATH]...
```

## Arguments

| Argument    | Description                                                                                                                                                           |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[PATH]...` | Files or directories to format. Directories are searched recursively (honoring `.gitignore`/`.ignore`/`.primignore`); defaults to the current directory when omitted. |

## Options

| Flag                            | Description                                                                                                    |
| ------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| `--check`                       | Write nothing; exit non-zero if any file would change, and list it.                                            |
| `--diff`                        | Print a unified diff of pending changes; write nothing.                                                        |
| `--stdin-filepath <PATH>`       | Read stdin, write the formatted result to stdout (format-on-save). Mutually exclusive with `--check`/`--diff`. |
| `--exclude <GLOB>`              | Exclude paths matching the glob (repeatable). A malformed glob is a usage error.                               |
| `--color <auto\|always\|never>` | When to use coloured output (default `auto`; `auto` honors `NO_COLOR`).                                        |
| `--completions <SHELL>`         | Generate a shell completion script and print it to stdout.                                                     |
| `-h, --help`                    | Print help.                                                                                                    |
| `-V, --version`                 | Print version.                                                                                                 |

## Exit codes

| Code | Meaning                                        |
| ---- | ---------------------------------------------- |
| `0`  | Success.                                       |
| `1`  | Changes needed (`--check` found a difference). |
| `2`  | Error (parse or I/O failure).                  |

## Operating modes

- **Default** — format the given files in place.
- **`--check`** — a CI gate: exit `1` and list the files that would change.
- **`--diff`** — preview pending changes without writing.
- **`--stdin-filepath`** — editor format-on-save: stdin in, formatted stdout
  out.
- Naming a path explicitly is strict: a missing file is an error (exit `2`); an
  existing file prim does not own is skipped with a warning.

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

## Configuration

prim honors [`.editorconfig`](https://editorconfig.org) as its **only** style
configuration — there is no `prim.toml` and there are no per-rule flags. With no
`.editorconfig` present, prim applies its built-in canonical style (LF endings,
trailing whitespace stripped, exactly one final newline, two-space indent).

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
