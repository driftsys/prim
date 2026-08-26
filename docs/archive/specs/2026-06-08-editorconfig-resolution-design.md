# Design — `.editorconfig` resolution (FR-3) · issue #8

**Status:** approved (brainstorm) · **Branch:** `feat/editorconfig` · **Epic:**
#4

Honor `.editorconfig` as prim's _only_ style configuration. Resolve a canonical
`Style` from the `.editorconfig` cascade and thread it through the formatting
engine, making the existing whitespace-hygiene pass configuration-driven (and
picking up FR-2.3's deferred `end_of_line = crlf` branch).

## Requirements (acceptance criteria, from #1)

- **FR-3.1** Apply the built-in canonical style when no `.editorconfig` is
  present.
- **FR-3.2** Read `.editorconfig` and honor `indent_style`, `indent_size`,
  `max_line_length`, `end_of_line`, `charset`, `insert_final_newline`,
  `trim_trailing_whitespace` — including the `root = true` chain (walk up to
  root) and per-glob sections.
- **FR-3.3** Expose no other style configuration (no `prim.toml`, no per-rule
  flags).
- **FR-3.4** Never reorder keys, table entries, or array elements.
- **FR-2.3** Normalize line endings to LF, _unless_ `.editorconfig` sets
  `end_of_line = crlf` (deferred from #6, picked up here).

## Architecture

`.editorconfig` resolution is I/O (walk the directory tree reading files), so it
lives in **`prim-cli`**; the engine stays pure. The resolved settings travel as
a plain data struct (`Style`) that lives in **`prim-fmt`** so the engine can
consume it without any I/O dependency.

```
prim-fmt (pure)                         prim-cli (I/O + dispatch)
  style.rs   NEW  Style / LineEnding /    editorconfig.rs NEW  resolve(path)->Style
                  Indent (+ Default)                           (ec4rs Properties -> Style)
  hygiene.rs      hygiene(src, &Style)     app.rs   resolve(), then format(.., &style)
  lib.rs          format(kind, src, &Style)  main.rs  mod editorconfig
```

**Crate boundary:** `Style` must be visible to `prim_fmt::format`, and
`prim-fmt` is the lower crate, so `Style` lives in `prim-fmt`. `prim-cli`
_constructs_ it by reading `.editorconfig` via `ec4rs`. Engine stays free of
clap / I/O / terminal.

### Engine API change

`prim_fmt::format(kind, source)` → `prim_fmt::format(kind, source, &Style)`.
Both call sites in `app.rs` (`run_paths`, `run_stdin`) resolve a `Style` first.
`Style` is re-exported from `prim_fmt` (`lib.rs` index).

## The `Style` struct (built-in canonical style = `Default`, FR-3.1)

```rust
pub struct Style {
    pub end_of_line: LineEnding,         // Lf (default) | CrLf
    pub trim_trailing_whitespace: bool,  // default true
    pub insert_final_newline: bool,      // default true
    pub indent: Indent,                  // default Spaces(2)
    pub max_line_length: Option<usize>,  // default None
}

pub enum LineEnding { Lf, CrLf }
pub enum Indent { Spaces(usize), Tab }
```

- `Style::default()` is the FR-3.1 built-in canonical style and **must reproduce
  today's exact hygiene output** (LF, trim, exactly one final newline). A
  regression-guard unit test pins this.
- `max_line_length` default is `None` — the "else 80" wrap default (FR-1.1)
  stays in the Markdown formatter (`style.max_line_length.unwrap_or(80)`), not
  baked into `Style`.
- `indent` and `max_line_length` are **resolved and carried but not yet
  consumed** — no parser exists until #9–12. They remain testable now at the
  resolution layer (assert the resolved `Style` for a path), so they are not
  untested dead fields.

## Hygiene becomes `Style`-driven

Today `hygiene` hard-codes LF + trim + one-final-newline. New behavior, all
gated on `Style`:

- **`end_of_line`** — normalize _all_ existing endings (`\r\n`, lone `\r`, `\n`)
  to the configured EOL sequence (`\n` or `\r\n`). This is FR-2.3's `crlf`
  branch.
- **`trim_trailing_whitespace`** — gate the per-line `trim_end`. When `false`,
  EOL normalization still happens (FR-2.3 is independent of trimming).
- **`insert_final_newline`** — `true` → exactly one trailing EOL (today's
  behavior); `false` → strip all trailing newlines (file ends with content, no
  EOL). Empty / whitespace-only content stays empty either way.

Idempotency (FR-6.1) holds under every `Style`.

## Resolution: `ec4rs` → `Style` (`prim-cli/src/editorconfig.rs`)

`resolve(path: &Path) -> Style` uses `ec4rs` to walk up from the path's
directory, honor `root = true`, glob-match sections, and apply property
precedence, then maps the resolved `Properties` onto `Style`:

| editorconfig key            | `ec4rs` property     | `Style` field / mapping                         |
| --------------------------- | -------------------- | ----------------------------------------------- |
| `end_of_line`               | `EndOfLine`          | `lf`→Lf · `crlf`→CrLf · `cr`→Lf (see decisions) |
| `trim_trailing_whitespace`  | `TrimTrailingWs`     | bool                                            |
| `insert_final_newline`      | `InsertFinalNewline` | bool                                            |
| `indent_style` + `..._size` | `IndentStyle`/`Size` | `tab`→Tab · `space`→Spaces(size)                |
| `max_line_length`           | `MaxLineLength`      | number→Some · `off`/unset→None                  |
| (unset)                     | —                    | `Style::default()` value                        |

`--stdin-filepath`: resolve relative to that path's directory (pass it to
`resolve`).

## Decisions

1. **`insert_final_newline = false` → strip all trailing newlines**
   (EditorConfig spec wording: false = "ensure it does not end with a newline").
2. **`charset` — out of scope for #8.** prim is a UTF-8-only formatter
   (non-UTF-8 files are already left unchanged + reported). `utf-8-bom` /
   `latin1` / `utf-16*` would require transcoding, which prim does not do.
   `charset` is _not_ carried in `Style` (no consumer, no testable application).
   Documented as a deliberate scope cut; prim treats files as UTF-8.
3. **`end_of_line = cr`** (bare CR, deprecated) → map to `Lf`. FR-2.3 only
   carves out `crlf`; an exotic `cr` falls back to canonical LF.
4. **Malformed `.editorconfig`** → fail-safe: fall back to `Style::default()`
   and emit a `ui::warning`; never crash or exit 2.
5. **Per-file resolution.** Resolve `Style` per file (the cascade depends on the
   file's directory). No per-directory `Style` cache now (YAGNI /
   measure-first); noted as a follow-up if profiling ever shows NFR-4 (5000
   files < 2 s) pressure.

## Alternatives considered

- **Hand-roll the parser** (INI + `root` chain + EditorConfig glob grammar
  `{a,b}`/`**`/`[]` + precedence). Rejected: ~300+ lines of fiddly,
  easy-to-get-subtly-wrong code to own and test against the core edge cases.
  `ec4rs` (pure Rust, descends from the editorconfig-core test suite) does it in
  one small dependency. Violates "minimum code that solves the problem".
- **FFI crates** (`editorconfig-rs`/`-sys` wrapping C `libeditorconfig`).
  Rejected: a C dependency undermines prim's single static-binary distribution.
- **`charset` resolved-but-unconsumed field.** Rejected per YAGNI — no consumer,
  no testable application, and prim's pipeline is UTF-8-only by design.
- **`max_line_length` default baked as `Some(80)` in `Style`.** Rejected — keeps
  the "else 80" Markdown-wrap default (FR-1.1) where it belongs, in the future
  Markdown formatter, not in shared style state.

## Testing strategy (strict TDD, real fixtures, red→green)

- **`prim-fmt` units** (`style.rs`, `hygiene.rs`): `Style::default()` values;
  hygiene under each `Style` (crlf emits `\r\n`; trim=false preserves trailing
  ws but still normalizes EOL; insert_final_newline=false strips final newline);
  `Style::default()` regression guard == today's output; idempotency per style.
- **`prim-cli` units** (`editorconfig.rs`): `resolve()` over temp-dir fixtures —
  no config→default; `root=true` chain stops at root; per-glob sections (`*.md`
  width 80, `*.rs` indent 4, `[*]` 2-space); `end_of_line=crlf`→CrLf; malformed→
  default + warning. Real `.editorconfig` files on disk; no mocks.
- **`prim-cli` behavioural** (`tests/editorconfig.rs`, `assert_cmd`): drive
  `prim` on temp repos — crlf written and `--check`-detected; final-newline
  stripped; trailing-ws preserved while EOL normalized; `--stdin-filepath`
  honors a sibling `.editorconfig`; no-config behavior unchanged.
- **Dogfood:** the repo's own `.editorconfig` → `prim --check .` stays exit 0.

## File / module plan

| File                                         | Change                                           |
| -------------------------------------------- | ------------------------------------------------ |
| `crates/prim-fmt/src/style.rs`               | NEW — `Style`, `LineEnding`, `Indent`, `Default` |
| `crates/prim-fmt/src/lib.rs`                 | `mod style`; re-export; `format(.., &Style)`     |
| `crates/prim-fmt/src/hygiene.rs`             | `hygiene(src, &Style)` — style-driven            |
| `crates/prim-cli/src/editorconfig.rs`        | NEW — `resolve(path) -> Style` via `ec4rs`       |
| `crates/prim-cli/src/main.rs`                | `mod editorconfig`                               |
| `crates/prim-cli/src/app.rs`                 | resolve + pass `&style` at both call sites       |
| `crates/prim-cli/Cargo.toml`                 | add `ec4rs = "1.2"`                              |
| `AGENTS.md`, `docs/USAGE.md`, `docs/SPEC.md` | status note; honored keys + charset/cr scope     |

## Out of scope (this issue)

- Structured per-format parsers (#9–12) — they consume the carried `indent` /
  `max_line_length`.
- `charset` transcoding / non-UTF-8 emission.
- Per-directory `Style` caching (deferred optimization).
