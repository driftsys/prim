# prim-fmt

The formatting engine behind [prim](https://github.com/driftsys/prim) — an
opinionated, near-zero-config formatter for a repository's connective tissue
(Markdown, JSON/JSONC, YAML, TOML) plus whitespace hygiene.

This crate is the pure library: strings in, strings out, no CLI or terminal
dependencies, so other tools can embed it. The `prim` binary is published
separately as [`prim-cli`](https://crates.io/crates/prim-cli).

> **Status:** early. `classify` + `format` apply the whitespace-hygiene pass to
> the parsed formats and the orphan allowlist; structured per-format passes land
> in later milestones. See the
> [specification](https://github.com/driftsys/prim/blob/main/docs/SPEC.md).

## Usage

```rust
use std::path::Path;

// Classify a file by name, then format its contents for that kind.
if let Some(kind) = prim_fmt::classify(Path::new("README.md")) {
    let formatted = prim_fmt::format(kind, "# Title  \n");
    assert_eq!(formatted, "# Title\n");
}
```

## Correctness fixtures

`tests/correctness/fixtures/<format>/*.txt` drive the correctness harness
(FR-6.1 idempotency, FR-6.2 semantic preservation, plus format-equality). Each
file has `-- input --` and `-- expected --` sections, plus an optional
`-- config --` section overriding the default `Style`. The directory name
selects the `FileKind` (`json`, `jsonc`, `toml`, `yaml`, `markdown`, `hygiene`).

To add a fixture: create the file with your `-- input --` and an empty
`-- expected --`, then run:

```bash
PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness spec_cases_format_as_expected
```

Review the generated diff before committing — this is the step where a formatter
bug would show up as unexpected output.

## Benchmarks

`benches/format.rs` times `format()` per file kind (JSON, TOML, YAML, Markdown)
across small/medium/large synthetic inputs generated at bench time (no vendored
corpus — deterministic and reproducible). Run:

```bash
just bench
```

This is not part of `just check` or CI — it's slow and its numbers are
machine-dependent, so it's for local regression-hunting, not a gate. HTML
reports land in `target/criterion/report/index.html`. There is currently no
tracked performance baseline; treat a run before and after your change as the
comparison, not an absolute number.

## License

MIT © driftsys
