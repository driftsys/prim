# prim — System Design

prim is a single statically linked binary (`prim`) that formats a repository's
connective tissue: Markdown, JSON/JSONC, YAML, TOML, plus whitespace hygiene on
a curated orphan allowlist. It is not a source-code formatter and has no plugin
system.

## Workspace structure

The repository is a Cargo workspace with three crates.

`prim-fmt` is the formatting engine. It is a library with no CLI, terminal, or
I/O dependencies. It exposes the public surface that all other crates consume:
`classify`, `format`, `FileKind`, `Style`, `LineEnding`, and `Indent`.
Per-format structured passes (FR-1) will be added inside this crate as future
milestones; the `match kind { … }` dispatch in `format` is the intended
extension point.

`prim-cli` is the thin binary crate. Its `[[bin]]` target is named `prim`. It
owns all I/O: argument parsing (`clap`), file discovery (`ignore`),
`.editorconfig` resolution (`ec4rs`), atomic writes (`tempfile`), and coloured
terminal output (`yansi`). It calls into `prim-fmt` exclusively through the
`format` function. `cargo install prim-cli` is the user-facing install command.

`spec` (workspace path `spec/`) is a test-only crate (never published). It holds
`trycmd` CLI-output snapshot tests and shell-based install tests.

## Component map

```text
prim-fmt (library, pure)
  classify.rs   FileKind, classify(path) -> Option<FileKind>
  style.rs      Style, LineEnding, Indent  (re-exported from lib.rs)
  hygiene.rs    hygiene(source, &Style) -> String
  lib.rs        format(kind, source, &Style) -> String  (dispatch)

prim-cli (binary "prim")
  cli.rs           Cli (clap struct), ColorWhen
  main.rs          entry point — colour init, completions, process::exit
  app.rs           run(&Cli) -> i32 — mode dispatch
  discover.rs      collect(paths, excludes) -> Vec<Discovered>
  editorconfig.rs  resolve(path) -> Style  (ec4rs -> Style)
  write.rs         atomic(path, contents)
  ui.rs            error / warning / would_reformat
```

## Data flow

For every file that prim processes the steps are, in order:

1. **Classify** — `classify(&path)` returns the `FileKind`, or `None` if prim
   does not own the file. Files that are not owned are left byte-for-byte
   unchanged and not reported.
2. **Read** — `fs::read_to_string` loads the file as UTF-8. A failure is
   reported (exit 2 for an explicitly named file; warning and skip for a walked
   file) and the file is not written (FR-6.3, FR-6.5).
3. **Resolve** — `editorconfig::resolve(&path)` walks the `.editorconfig`
   cascade from the file's directory upward. A missing config yields
   `Style::default()` (FR-3.1). A malformed or unreadable config falls back to
   `Style::default()` with a warning (AD-0002).
4. **Format** — `prim_fmt::format(kind, &source, &style)` applies the whitespace
   hygiene pass (FR-2). The per-format structured passes (FR-1) will attach here
   per milestone.
5. **Write** — if the formatted text differs from the original, `write::atomic`
   replaces the file via a same-directory temp file and rename, preserving
   permission bits (FR-6.4). In `--check` mode, the path is printed to stdout
   instead (FR-5.2). In `--diff` mode, a unified diff is printed (FR-5.3, not
   yet implemented).

For `--stdin-filepath`, steps 2 and 5 are replaced by stdin-read and
stdout-write respectively; resolve and format use the supplied path for
`.editorconfig` lookup and classification (FR-5.4).

## Command surface and exit codes

| Invocation                   | Behaviour                                                |
| ---------------------------- | -------------------------------------------------------- |
| `prim [PATH]...`             | Format files in place.                                   |
| `prim --check [PATH]...`     | Exit 1 and list files that would change. Writes nothing. |
| `prim --diff [PATH]...`      | Print unified diff. Writes nothing. (Pending FR-5.3.)    |
| `prim --stdin-filepath <p>`  | Read stdin, write formatted result to stdout.            |
| `prim --completions <shell>` | Print shell completion script to stdout.                 |

Exit codes: `0` success · `1` changes needed (–check) · `2` error (parse/IO).
See FR-5.5.

## Engine API

```rust
// prim_fmt public surface
pub fn classify(path: &Path) -> Option<FileKind>;
pub fn format(kind: FileKind, source: &str, style: &Style) -> String;
pub use style::{Style, LineEnding, Indent};
pub use classify::FileKind;
```

`Style::default()` is prim's built-in canonical style (FR-3.1): LF line endings,
trailing whitespace stripped, exactly one final newline, two-space indent.

## Style resolution detail

`editorconfig::resolve(path)` is the sole I/O consumer of `.editorconfig`. It
calls `ec4rs::properties_of(path)`, applies `use_fallbacks()` for EditorConfig
spec defaults, then maps properties onto `Style` fields. The mapping is:

| EditorConfig key           | `Style` field              | Notes                                |
| -------------------------- | -------------------------- | ------------------------------------ |
| `end_of_line`              | `end_of_line`              | `cr` maps to `Lf` (AD-0002)          |
| `trim_trailing_whitespace` | `trim_trailing_whitespace` |                                      |
| `insert_final_newline`     | `insert_final_newline`     | `false` strips all trailing newlines |
| `indent_style` + `_size`   | `indent`                   | `tab_width` fallback applied         |
| `max_line_length`          | `max_line_length`          | `off` → `None`; unset → `None`       |
| `charset`                  | —                          | out of scope (AD-0002)               |

`indent` and `max_line_length` are resolved and carried in `Style` but are not
yet consumed by any formatter pass; they are available and testable now so the
per-format parsers (FR-1, issues #9–12) can read them without an API change.

## Crate boundary invariant

`prim-fmt` must never depend on `clap`, `yansi`, `ignore`, `ec4rs`, or any other
I/O or terminal crate. The boundary is enforced by the separation into two Cargo
packages. All I/O, including `.editorconfig` file reading, lives exclusively in
`prim-cli`. See AD-0001.

## Implementation status (as of feat/editorconfig, PR #19)

Implemented: recursive file discovery (FR-4), whitespace hygiene (FR-2),
`.editorconfig` resolution (FR-3), atomic writes (FR-6.4), UTF-8 fail-safe
reporting (FR-6.5).

Not yet implemented: per-format structured passes (FR-1), `--diff` unified
output (FR-5.3, scaffold comment present in `app.rs`), per-directory `Style`
cache (deferred per AD-0002).
