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

## License

MIT © driftsys
