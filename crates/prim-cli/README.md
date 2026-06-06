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

> **Status:** early. Recursive discovery and whitespace hygiene are wired
> end-to-end; structured per-format passes and `.editorconfig` land in later
> milestones. See the
> [specification](https://github.com/driftsys/prim/blob/main/docs/SPEC.md).

## License

MIT © driftsys
