# Design — YAML formatter (FR-1.4) · issue #11

**Status:** approved (autonomous loop; user directive "continue in a loop") ·
**Branch:** `feat/yaml-format` · **Epic:** #4

Canonical YAML formatting preserving comments, anchors/aliases, and multi-line
(block) scalar styles. Third per-format structured pass; reuses the fallible
`format` API and `hygiene` composition from JSON/TOML (AD-0003/0004). Only new
code is a `yaml` module.

## Requirements (from #1)

- **FR-1.4** Format YAML, preserving comments, anchors/aliases, and multi-line
  scalar styles.
- **FR-3.4** Never reorder keys or sequence entries.
- **FR-6.2** Do not change the parsed data model.
- **FR-6.3** Unparseable files left unchanged and reported.

## Decision: use `pretty_yaml` as a library

`pretty_yaml` (g-plane; the YAML member of the same CST-formatter family used by
several dprint plugins) is the YAML analog of `dprint-plugin-json`/`taplo`.

- `pretty_yaml::format_text(input: &str, &FormatOptions) -> Result<String, SyntaxError>`
  — **returns a parse error directly** on invalid YAML, so no separate parse
  step is needed (cleaner than taplo). Built on `yaml_parser` (a rowan CST).
- It preserves comments, anchors/aliases, and block (literal `|` / folded `>`)
  scalar styles, and never reorders — satisfying FR-1.4/3.4/6.2 by design.
- Pure Rust (`yaml_parser`, `rowan`, `tiny_pretty`).

`FormatOptions { layout: LayoutOptions, language: LanguageOptions }`;
`LayoutOptions { print_width: usize=80, indent_width: usize=2, line_break: LineBreak=Lf }`
(neither struct `#[non_exhaustive]`). `LanguageOptions` defaults are used.

The Rust YAML landscape lacks another mature, comment/anchor-preserving
_formatter_: `yaml-rust2`/`saphyr`/`yaml-peg` are parsers without canonical
printers, and `serde_yaml` is deprecated and strips comments. `pretty_yaml` is
the only library-grade fit; hand-rolling a YAML printer (anchors, aliases, flow
vs block, multi-line scalars) was rejected as far too much surface.

## Architecture

```text
prim-fmt (pure)
  yaml.rs   NEW  format(source, &Style) -> Result<String, FormatError>
                 - map Style -> pretty_yaml FormatOptions
                 - format_text(source, &opts) -> Ok(hygiene(..)) | Err(Parse)
  lib.rs         FileKind::Yaml -> yaml::format  (new dispatch arm)
```

### `yaml::format`

```rust
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let options = FormatOptions {
        layout: LayoutOptions {
            print_width: style.max_line_length.unwrap_or(80),
            // YAML forbids tab indentation; Tab falls back to 2 spaces.
            indent_width: match style.indent {
                Indent::Spaces(n) => n,
                Indent::Tab => 2,
            },
            line_break: LineBreak::Lf, // hygiene owns EOL
        },
        ..FormatOptions::default()
    };
    match pretty_yaml::format_text(source, &options) {
        Ok(printed) => Ok(hygiene(&printed, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}
```

## Decisions

1. **`pretty_yaml::format_text` returns `Result`** — its `SyntaxError` maps
   straight to `FormatError::Parse`; no pre-parse needed. CLI handling unchanged
   from JSON/TOML (explicit→exit 2, discovered→warn, stdin→echo+exit 2).
2. **`Indent::Tab` → 2-space indent.** YAML forbids tabs for indentation, and
   `LayoutOptions` has no tab option, so tab indentation falls back to 2 spaces.
3. **EOL owned by `hygiene`** (`line_break: Lf`, then hygiene converts) — one
   source of truth across formats.
4. **`LanguageOptions` defaults** are used (quote/flow preferences); block
   scalar styles, anchors/aliases, and comments are preserved, which is what
   FR-1.4 requires. Verified by tests.

## Testing (strict TDD)

- **`prim-fmt` `yaml` units:** canonical key spacing/indent from Style;
  **comment preserved**; **anchor/alias preserved** (`&a`/`*a`); **block scalar
  `|` preserved**; key order preserved; invalid YAML → `Err(Parse)`;
  idempotency; crlf via Style.
- **`prim-cli` behavioural (`tests/yaml.rs`):** in-place reformat; `--check`
  flags non-canonical; `.editorconfig` `indent_size = 4` honored; comment +
  anchor + block-scalar preservation end-to-end; invalid explicit→exit 2 /
  discovered→warn; stdin invalid→echo+exit 2; round-trip; `.yml` and `.yaml`
  both formatted.
- **Dogfood:** `prim --check .` exit 0 after prim formats the repo's YAML
  (`.github/workflows/*.yml`, `.editorconfig` is not YAML). Diff shown before
  commit; pretty_yaml preserves data + order so changes are style-only.

## File plan

| File                                                                              | Change                                      |
| --------------------------------------------------------------------------------- | ------------------------------------------- |
| `crates/prim-fmt/src/yaml.rs`                                                     | NEW — `format(source, &Style) -> Result<…>` |
| `crates/prim-fmt/src/lib.rs`                                                      | `mod yaml;` + `FileKind::Yaml` dispatch arm |
| `crates/prim-fmt/Cargo.toml`                                                      | add `pretty_yaml = "0.6"`                   |
| `crates/prim-cli/tests/yaml.rs`                                                   | NEW — behavioural coverage                  |
| `AGENTS.md`, `docs/USAGE.md`, `docs/design/system.md`, `docs/decisions/0005-*.md` | docs (gardened inline)                      |

## Out of scope

- Markdown (#12).
- `--diff` (#14).
- Honoring any YAML-specific config (prim honors only `.editorconfig`, FR-3.3).
