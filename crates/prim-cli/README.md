# prim-cli

The command-line interface for [prim](https://github.com/driftsys/prim) — an
opinionated, near-zero-config formatter for a repository's connective tissue
(Markdown, JSON/JSONC, YAML, TOML) plus whitespace hygiene.

Installing this crate provides the **`prim`** binary:

```bash
cargo install prim-cli
```

The formatting engine lives in the separate, reusable
[`prim-fmt`](https://crates.io/crates/prim-fmt) library crate.

> **Status:** walking skeleton. The command-line surface is wired end-to-end,
> but the engine is currently a no-op. See the
> [specification](https://github.com/driftsys/prim/blob/main/docs/SPEC.md).

## License

MIT © driftsys
