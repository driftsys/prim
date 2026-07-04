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
  error.rs      FormatError (thiserror)  (re-exported from lib.rs)
  hygiene.rs    hygiene(source, &Style) -> String
  json.rs       format(source, &Style) -> Result<String, FormatError>  (dprint-plugin-json)
  toml.rs       format(source, &Style) -> Result<String, FormatError>  (taplo)
  yaml.rs       format(source, &Style) -> Result<String, FormatError>  (pretty_yaml)
  markdown.rs   format(source, &Style) -> Result<String, FormatError>  (dprint-plugin-markdown)
  lib.rs        format(kind, source, &Style) -> Result<String, FormatError>  (dispatch)

prim-cli (binary "prim")
  cli.rs           Cli (clap struct), ColorWhen
  main.rs          entry point — colour init, completions, process::exit
  app.rs           run(&Cli) -> i32 — mode dispatch
  discover.rs      collect(paths, excludes) -> Vec<Discovered>
  editorconfig.rs  resolve(path) -> Style  (ec4rs -> Style)
  diff.rs          unified(path, original, formatted) -> String  (similar)
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
   hygiene pass (FR-2), and for structured formats the per-format pass followed
   by hygiene: `Json`/`Jsonc` via `dprint-plugin-json` (FR-1.2/1.3, AD-0003),
   `Toml` via `taplo` (FR-1.5, AD-0004), `Yaml` via `pretty_yaml` (FR-1.4,
   AD-0005), `Markdown` via `dprint-plugin-markdown` (FR-1.1/1.1a/1.6, AD-0006).
   It returns `Result<String, FormatError>`; a parse error leaves the file
   unchanged and is reported as in step 2 (explicit → exit 2, discovered →
   warning). All per-format passes are now implemented.
5. **Write** — if the formatted text differs from the original, `write::atomic`
   replaces the file via a same-directory temp file and rename, preserving
   permission bits (FR-6.4). In `--check` mode, the path is printed to stdout
   instead (FR-5.2). In `--diff` mode, a unified diff is printed to stdout via
   `diff::unified` (FR-5.3).

For `--stdin-filepath`, steps 2 and 5 are replaced by stdin-read and
stdout-write respectively; resolve and format use the supplied path for
`.editorconfig` lookup and classification (FR-5.4). A parse error in this mode
echoes the original source to stdout unchanged (so format-on-save never blanks
the buffer), reports to stderr, and exits 2 (AD-0003).

## Command surface and exit codes

| Invocation                   | Behaviour                                                     |
| ---------------------------- | ------------------------------------------------------------- |
| `prim [PATH]...`             | Format files in place.                                        |
| `prim --check [PATH]...`     | Exit 1 and list files that would change. Writes nothing.      |
| `prim --diff [PATH]...`      | Print unified diff (via `similar`) to stdout. Writes nothing. |
| `prim --stdin-filepath <p>`  | Read stdin, write formatted result to stdout.                 |
| `prim --completions <shell>` | Print shell completion script to stdout.                      |

Exit codes: `0` success · `1` changes needed (–check) · `2` error (parse/IO).
See FR-5.5.

## Engine API

```rust
// prim_fmt public surface
pub fn classify(path: &Path) -> Option<FileKind>;
pub fn format(kind: FileKind, source: &str, style: &Style) -> Result<String, FormatError>;
pub use style::{Style, LineEnding, Indent};
pub use classify::FileKind;
pub use error::FormatError;
```

`Style::default()` is prim's built-in canonical style (FR-3.1): LF line endings,
trailing whitespace stripped, exactly one final newline, two-space indent.

## Style resolution detail

`editorconfig::Resolver` is the sole I/O consumer of `.editorconfig`. It opens
the `.editorconfig` cascade for a file's directory, applies matching sections
and `use_fallbacks()` for EditorConfig spec defaults, then maps properties onto
`Style` fields. `Resolver` caches the parsed cascade **per directory**, so a
repository parses each `.editorconfig` once rather than once per file. Because
per-glob sections mean two files in one directory can resolve differently
(`[*.md]` vs `[*.toml]`), only the file reading and parsing is cached — the glob
matching still runs per file, so the cached result is byte-identical to an
uncached resolve (guarded by an equivalence test against
`ec4rs::properties_of`). The mapping is:

| EditorConfig key           | `Style` field              | Notes                                |
| -------------------------- | -------------------------- | ------------------------------------ |
| `end_of_line`              | `end_of_line`              | `cr` maps to `Lf` (AD-0002)          |
| `trim_trailing_whitespace` | `trim_trailing_whitespace` |                                      |
| `insert_final_newline`     | `insert_final_newline`     | `false` strips all trailing newlines |
| `indent_style` + `_size`   | `indent`                   | `tab_width` fallback applied         |
| `max_line_length`          | `max_line_length`          | `off` → `None`; unset → `None`       |
| `charset`                  | —                          | out of scope (AD-0002)               |

`indent` drives indentation in the JSON/JSONC, TOML, and YAML passes;
`max_line_length` (default 80) drives line width in those passes and the
Markdown prose wrap. (YAML forbids tab indentation, so `Indent::Tab` falls back
to two spaces there — AD-0005.)

## Crate boundary invariant

`prim-fmt` must never depend on `clap`, `yansi`, `ignore`, `ec4rs`, or any other
I/O or terminal crate. The boundary is enforced by the separation into two Cargo
packages. All I/O, including `.editorconfig` file reading, lives exclusively in
`prim-cli`. See AD-0001.

## Implementation status (v1 complete)

Implemented: recursive file discovery (FR-4), whitespace hygiene (FR-2),
`.editorconfig` resolution (FR-3), all per-format structured passes — JSON/JSONC
(FR-1.2/1.3, AD-0003), TOML (FR-1.5, AD-0004), YAML (FR-1.4, AD-0005),
Markdown + prose wrap (FR-1.1/1.1a/1.6, AD-0006) — atomic writes (FR-6.4), UTF-8
fail-safe reporting (FR-6.5), `--diff` unified output (FR-5.3), and a
cross-cutting idempotency/semantic-preservation harness (FR-6.1/6.2,
`crates/prim-fmt/tests/correctness.rs`). prim formats its own Markdown; the repo
no longer depends on dprint (AD-0006). All v1 requirements (FR-1 through FR-6)
are implemented.

Implemented post-v1: the per-directory `.editorconfig` cascade cache (AD-0002) —
a repository with a root `.editorconfig` formats ~9% faster by parsing each
config once instead of per file.

Deferred (post-v1, not requirements): colorized `--diff` output.
