# Design — Markdown formatter + prose wrap (FR-1.1/1.1a/1.6) · issue #12

**Status:** approved (user chose "full migration"; "continue in a loop") ·
**Branch:** `feat/markdown-format` · **Epic:** #4

The last per-format structured pass: canonical Markdown + hard-wrapped prose
with guardrails. Also **retires dprint** — which exists in this repo only to
format Markdown — making prim the sole formatter of its own connective tissue.

## Requirements (from #1)

- **FR-1.1** Canonical style (ATX headings, normalized list markers, table
  padding, blank-line spacing); hard-wrap prose to `max_line_length`
  (`.editorconfig`, else 80).
- **FR-1.1a** _(guardrails)_ Wrap prose paragraphs only; never break inside
  inline code, never split a URL/link, never wrap tables or fenced code blocks;
  preserve explicit hard breaks (trailing `\` or two-space).
- **FR-1.6** Preserve fenced code-block contents verbatim.
- **FR-3.4 / FR-6.2** Never reorder; do not change the data model.

## Decision: `dprint-plugin-markdown` as a library

The same engine the repo already uses (its dprint config loads the `markdown`
wasm plugin). The Rust crate `dprint-plugin-markdown = "0.22"` gives:

- `format_text(text, &Configuration, code_block_cb) -> anyhow::Result<Option<String>>`
  (`Some` = reformatted, `None` = already canonical, `Err` = parse failure —
  effectively unreachable, CommonMark accepts any input).
- `ConfigurationBuilder::new().line_width(w).text_wrap(TextWrap::Always).build()`.
- **FR-1.6 via the callback:** `format_code_block_text` returns `Ok(None)`, so
  dprint never reformats embedded code — fenced blocks pass through verbatim.
- dprint markdown's defaults are FR-1.1 canonical (ATX headings, dash list
  markers, padded tables, normalized blank lines) and its wrapper respects the
  FR-1.1a guardrails (inline code atomic, links not split, tables/code not
  wrapped, hard breaks preserved).

`Style` maps to config: `line_width = max_line_length.unwrap_or(80)`;
`text_wrap = Always` (FR-1.1 hard wrap); EOL/final newline stay with `hygiene`.

## Architecture

```text
prim-fmt (pure)
  markdown.rs  NEW  format(source, &Style) -> Result<String, FormatError>
                    - ConfigurationBuilder from Style
                    - format_text(src, &cfg, |_,_,_| Ok(None))  // FR-1.6
                    - Ok(hygiene(..)) | Err(Parse)
  lib.rs            FileKind::Markdown -> markdown::format  (last dispatch arm)
```

## The dprint retirement (full migration)

dprint's only role here is Markdown. With prim owning it:

1. **Reformat** all 19 `.md` files with prim (`prim .`). The 0.17.8→0.22.1 jump
   produces a one-time diff; prim's 0.22 output becomes the new canonical.
   Verify the result is markdownlint-clean and idempotent, and spot-check the
   diff against the FR-1.1a guardrails.
2. **Delete `dprint.json`.**
3. **`justfile`:** replace `dprint fmt` (in `fmt`) and `dprint check` (in
   `lint`/`check`) with prim invocations (`cargo run -q -p prim-cli --` for
   format / `--check`). Keep `markdownlint`.
4. **CI (`.github/workflows/ci.yml`):** replace the `dprint` job (which runs
   `dprint/check`) with a `prim`-check job that builds prim and runs
   `prim --check .`; update the `needs:` list of the gate job accordingly.

markdownlint stays as an independent lint (it checks rules prim does not, e.g.
heading content). prim + markdownlint must agree on the reformatted files.

## Test fallout: retarget `.md`-vehicle hygiene tests

Several pre-existing behavioural tests (`tests/editorconfig.rs`, possibly
`tests/hygiene.rs`/`modes.rs`) use `.md` files as _hygiene-only_ vehicles. With
Markdown now structured, those break. Retarget each to a `.txt` orphan (or other
permanently-hygiene-only file), as was done for the `.yaml` vehicle in #11.

## Testing (strict TDD)

- **`prim-fmt` `markdown` units:** ATX heading normalization (`Title\n===` →
  `# Title`); list marker normalization; prose hard-wrap to width; **FR-1.1a
  guardrails** — inline code span not broken, link/URL not split, fenced code
  preserved **verbatim** (FR-1.6), hard break preserved; idempotency; crlf via
  Style; indent/width from Style.
- **`prim-cli` behavioural (`tests/markdown.rs`):** in-place reformat; `--check`
  flags non-canonical; `.editorconfig` `max_line_length` honored for the wrap;
  fenced code + table + link preserved end-to-end; `.markdown` extension.
- **Dogfood:** after reformatting, `prim --check .` exit 0, `markdownlint`
  clean, and prim is idempotent on its own `.md`.

## File plan

| File                                                                              | Change                                     |
| --------------------------------------------------------------------------------- | ------------------------------------------ |
| `crates/prim-fmt/src/markdown.rs`                                                 | NEW — `format` via dprint-plugin-markdown  |
| `crates/prim-fmt/src/lib.rs`                                                      | `mod markdown;` + `FileKind::Markdown` arm |
| `crates/prim-fmt/Cargo.toml`                                                      | add `dprint-plugin-markdown = "0.22"`      |
| `crates/prim-cli/tests/markdown.rs`                                               | NEW — behavioural coverage                 |
| `crates/prim-cli/tests/editorconfig.rs` (+others)                                 | retarget `.md` vehicles → `.txt`           |
| 19 × `*.md`                                                                       | reformatted by prim (one-time)             |
| `dprint.json`                                                                     | deleted                                    |
| `justfile`                                                                        | dprint → prim                              |
| `.github/workflows/ci.yml`                                                        | `dprint` job → `prim --check` job          |
| `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`, `docs/decisions/0006-*.md` | docs                                       |

## Out of scope

- `--diff` (#14), idempotency/semantic harness (#13) — Milestone 4.
- Reformatting embedded code inside fenced blocks (FR-1.6 forbids it).
