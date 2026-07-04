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
