# Usage

```text
prim [OPTIONS] [PATH]...
```

## Arguments

| Argument    | Description                                                                                                                                                           |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[PATH]...` | Files or directories to format. Directories are searched recursively (honoring `.gitignore`/`.ignore`/`.primignore`); defaults to the current directory when omitted. |

## Options

| Flag                            | Description                                                         |
| ------------------------------- | ------------------------------------------------------------------- |
| `--check`                       | Write nothing; exit non-zero if any file would change, and list it. |
| `--diff`                        | Print a unified diff of pending changes; write nothing.             |
| `--stdin-filepath <PATH>`       | Read stdin, write the formatted result to stdout (format-on-save).  |
| `--exclude <GLOB>`              | Exclude paths matching the glob (repeatable).                       |
| `--color <auto\|always\|never>` | When to use coloured output (default `auto`).                       |
| `--completions <SHELL>`         | Generate a shell completion script and print it to stdout.          |
| `-h, --help`                    | Print help.                                                         |
| `-V, --version`                 | Print version.                                                      |

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

> **Status:** prim currently applies whitespace hygiene (trailing-whitespace
> removal, single final line-feed, LF endings) to the parsed formats and the
> orphan allowlist. Structured per-format formatting is not yet implemented, so
> a file is reported as changed only when its whitespace differs. See the
> [Specification](SPEC.md).
