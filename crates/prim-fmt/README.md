# prim-fmt

The formatting engine behind [prim](https://github.com/driftsys/prim) — an
opinionated, near-zero-config formatter for a repository's connective tissue
(Markdown, JSON/JSONC, YAML, TOML) plus whitespace hygiene.

This crate is the pure library: strings in, strings out, no CLI or terminal
dependencies, so other tools can embed it. The `prim` binary is published
separately as [`prim-cli`](https://crates.io/crates/prim-cli).

> **Status:** walking skeleton. `format` currently returns its input unchanged;
> the structured parsers and the whitespace-hygiene pass land in later
> milestones. See the
> [specification](https://github.com/driftsys/prim/blob/main/docs/SPEC.md).

## Usage

```rust
let formatted = prim_fmt::format("# Title\n");
```

## License

MIT © driftsys
