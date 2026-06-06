# Usage

```text
prim [OPTIONS] [PATH]...
```

## Arguments

| Argument    | Description                                                   |
| ----------- | ------------------------------------------------------------- |
| `[PATH]...` | Files to format. (Recursive directory discovery lands later.) |

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

> **Status:** the formatter is currently a no-op (walking skeleton), so every
> file is reported as already formatted. The behaviours above describe the
> intended contract; see the [Specification](SPEC.md).
