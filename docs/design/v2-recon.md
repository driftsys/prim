# prim v1 architecture recon (Spike #38)

A map of prim v1 as the v2 stories will find it: crate layout, CLI surface,
per-format pipeline, `.editorconfig` resolution, and the concrete extension
points the v2 work hangs off. Companion to `docs/design/system.md`; this note is
oriented at the v1.x→v2 epic (#37).

## Crate layout

| Crate      | Kind          | Owns                                                       |
| ---------- | ------------- | ---------------------------------------------------------- |
| `prim-fmt` | library-pure  | classification + formatting engine; no clap/I/O/terminal.  |
| `prim-cli` | binary `prim` | arg parsing, discovery, `.editorconfig` I/O, atomic write. |
| `spec`     | test-only     | `trycmd` CLI snapshots + install-script tests.             |

The crate boundary (AD-0001) is the load-bearing invariant: `prim-fmt` takes a
resolved `Style` and returns a `String`/`FormatError`. All I/O, config reading,
and terminal concerns live in `prim-cli`.

## Engine pipeline (`prim-fmt`)

```text
classify(path) -> Option<FileKind>          classify.rs   (name/ext only, never content)
format(kind, source, &Style) -> Result<String, FormatError>   lib.rs  (dispatch)
  ├─ Json | Jsonc -> json::format     dprint-plugin-json  -> hygiene
  ├─ Toml         -> toml::format     taplo               -> hygiene
  ├─ Yaml         -> yaml::format     pretty_yaml         -> hygiene
  ├─ Markdown     -> markdown::format dprint-plugin-markdown -> hygiene
  └─ Orphan       -> hygiene only
```

- **`classify`** decides ownership from the final path component only. Parsed
  formats by extension; an `Orphan` (un-owned text) allowlist by exact name /
  prefix (`Dockerfile.*`, `LICENSE*`) / `.txt`/`.text`. `.env`, `.sh`, and
  source files return `None` and are left byte-for-byte unchanged.
- **Each per-format module** parses with a third-party crate, maps `Style`
  (indent, `max_line_length`) onto that crate's options, then funnels the result
  through the shared `hygiene` pass. Parse failure →
  `FormatError::Parse(String)` (the only error variant; it carries the
  underlying parser's message, sometimes with a location).
- **`hygiene`** is the format-agnostic tail: normalise EOL to
  `Style.end_of_line`, optionally trim trailing whitespace, apply the
  final-newline rule. Idempotent.
- **Semantics-preserving** is enforced per module: taplo `reorder_* = false`,
  JSON/YAML/Markdown never reorder. This is the sacred invariant to preserve in
  v2 (e.g. rumdl's MD072 frontmatter-sort stays **off**).

## CLI surface (`prim-cli`)

One command, flag-driven modes (`cli.rs` / `app.rs`):

| Invocation                   | Mode dispatch (`app::run`)                           |
| ---------------------------- | ---------------------------------------------------- |
| `prim [PATH]...`             | discover → format in place via `write::atomic`.      |
| `prim --check [PATH]...`     | list would-change files to stdout; exit 1.           |
| `prim --diff [PATH]...`      | print unified diff to stdout; write nothing.         |
| `prim --stdin-filepath <p>`  | `run_stdin`: format stdin → stdout (format-on-save). |
| `prim --completions <shell>` | handled in `main` before any file work.              |
| `prim --exclude <glob>`      | repeatable discovery filter.                         |
| `prim --color <auto          | always                                               |

- **Dispatch shape:** `main` → `app::run(&cli)` → `run_stdin` or `run_paths`.
  Mode is decided by boolean flags on one flat `Cli` struct; there are **no
  subcommands** today. `--check` and `--diff` are `conflicts_with`;
  `--stdin-filepath` conflicts with both.
- **Exit codes** (`app.rs` constants): `0` ok · `1` changes needed (`--check`
  only) · `2` error (parse/IO/missing named file). Warnings on discovered files
  do not fail; errors on explicitly-named files do.
- **Discovery** (`discover.rs`) uses the `ignore` crate: recursive walk honoring
  `.gitignore`/`.ignore`/`.primignore`, `--exclude` globs, `explicit` flag to
  distinguish named vs walked paths.
- **Config resolution** (`editorconfig.rs`) via `ec4rs`: a `Resolver` caches
  each directory's parsed cascade; `resolve(path)` replays sections root-first
  (nearer/later wins) and maps `Properties` → `Style`. Only standard keys are
  read today (`end_of_line`, `trim_trailing_whitespace`, `insert_final_newline`,
  `indent_*`, `max_line_length`).

## Extension points for v2

| v2 need                                          | Where it attaches                                                                                                                                                                                                   |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Verbs** `fmt`/`lint`/`fix` (#40,G1)            | `cli.rs` `Cli` struct + `main`/`app::run` dispatch. Today flat flags; needs a subcommand or verb layer. Exit-code constants live in `app.rs`.                                                                       |
| **Markdown content lint** (#39,G2)               | New `prim-fmt` module (e.g. `mdlint`) invoked alongside `markdown::format`; or a lint entrypoint parallel to `format`. rumdl is lint-only, never its formatter.                                                     |
| **Diagnostics / line:col** (#42,B1,D2)           | `FormatError` is a single stringly-typed `Parse` variant — needs a richer diagnostic type (code + span) to carry `file:line:col`. Per-format modules already receive parser errors that sometimes embed a location. |
| **`prim_*` editorconfig keys** (#41,C1)          | `editorconfig.rs` `style_from`/`apply`. `ec4rs` `Properties` may expose unknown keys; `Style` gains namespaced fields (e.g. `prim_mdlint_strict`).                                                                  |
| **Hygiene contract over un-owned text** (A1)     | `classify.rs` `is_orphan` allowlist + `hygiene.rs`. Shell stays excluded.                                                                                                                                           |
| **gitignore-aware / changed-files walk** (E1,E2) | `discover.rs` (already `ignore`-based); add `--since`/`--staged` git queries and `--no-ignore`.                                                                                                                     |
| **Machine output SARIF/JSON** (D2)               | `ui.rs` (human→stderr) + a new serializer for the stdout machine channel.                                                                                                                                           |
| **`prim explain` / effective config** (C2)       | `editorconfig.rs` resolver already tracks section provenance in the cascade.                                                                                                                                        |

## Constraints v2 must not break

- Keep `prim-fmt` pure — no clap/terminal/I/O leaks (AD-0001).
- Never reorder semantically meaningful lines or keys (`.gitignore`,
  `.gitattributes`, frontmatter, TOML/YAML/JSON keys).
- `.editorconfig` is the only config surface (FR-3.3); no `prim.toml`.
- Fail-safe: unparseable / non-UTF-8 files left unchanged, reported, exit `2`.
- Writes are atomic (temp + rename).
- Module size soft-limit 300 / hard-limit 500 lines; one concept per module.
