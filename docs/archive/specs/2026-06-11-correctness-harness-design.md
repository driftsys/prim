# Design — Idempotency & semantic-preservation harness (FR-6.1/6.2) · issue #13

**Status:** approved (autonomous loop) · **Branch:** `feat/correctness-harness`
· **Epic:** #4

A cross-cutting correctness harness every formatter is run through. The last
Milestone-4 issue.

## Requirements

- **FR-6.1** _(idempotency)_ `format(format(x)) == format(x)` for every format.
- **FR-6.2** _(semantic preservation)_ Formatting does not change the parsed
  data model of a JSON/JSONC/YAML/TOML document.

## Decision: an integration test in `prim-fmt/tests/`

The harness is `crates/prim-fmt/tests/correctness.rs` — an integration test over
the engine's public `format`. It runs under `cargo test --workspace`, so it is
**already a CI gate** (the `test` job) with no workflow change.

A shared corpus of `(FileKind, &str)` inputs (representative + edge cases per
format) drives both checks under `Style::default()`.

### FR-6.1 — idempotency (all formats)

```rust
for (kind, input) in CORPUS {
    let once = format(*kind, input, &style).unwrap();
    let twice = format(*kind, &once, &style).unwrap();
    assert_eq!(once, twice, "not idempotent: {kind:?} / {input:?}");
}
```

Covers JSON, JSONC, TOML, YAML, Markdown, and Orphan (whitespace hygiene).

### FR-6.2 — semantic preservation (JSON/JSONC/YAML/TOML)

Parse `input` and `format(input)` with an **independent** parser (not the one
the formatter uses) and compare the data models:

| Format     | Independent parser → value                                 |
| ---------- | ---------------------------------------------------------- |
| JSON/JSONC | `jsonc_parser::parse_to_serde_value` → `serde_json::Value` |
| TOML       | `str::parse::<toml::Table>`                                |
| YAML       | `yaml_rust2::YamlLoader::load_from_str` → `Vec<Yaml>`      |

```rust
let a = parse(input);
let b = parse(&format(kind, input, &style).unwrap());
assert_eq!(a, b, "data model changed: {input:?}");
```

Independent parsers (different from `dprint-plugin-json`/`taplo`/`pretty_yaml`)
make the check meaningful — it validates the _data_, not the formatter's own
round-trip. Markdown has no data model and is excluded from FR-6.2 (covered by
idempotency only).

## Dev-dependencies (test-only, `prim-fmt`)

```toml
[dev-dependencies]
jsonc-parser = { version = "0.32", features = ["serde"] }
serde_json = "1"
toml = "1"
yaml-rust2 = "0.11"
```

All maintained and pure Rust; `yaml-rust2` is chosen over the deprecated
`serde_yaml` and is independent of `pretty_yaml`'s `yaml_parser`.

## Testing

The harness _is_ the test. It must be green. The corpus includes: nested
structures, comments (JSONC/YAML/TOML), inline tables (TOML), anchors/aliases
and block scalars (YAML), and odd-but-valid spacing — each exercised through
both checks. A failure names the format and input.

## File plan

| File                                   | Change                               |
| -------------------------------------- | ------------------------------------ |
| `crates/prim-fmt/tests/correctness.rs` | NEW — the harness                    |
| `crates/prim-fmt/Cargo.toml`           | `[dev-dependencies]` above           |
| `AGENTS.md`, `docs/design/system.md`   | mark #13 done / Milestone 4 complete |

## Out of scope

- Fuzzing / property generation (the corpus is curated, not randomized).
- Markdown semantic preservation (no data model; FR-6.2 lists only the data
  formats).
