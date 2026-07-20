# prim

Opinionated, near-zero-config formatter for a repository's _connective tissue_ —
Markdown, JSON/JSONC, YAML, TOML — plus whitespace hygiene on a curated set of
un-owned text files.

prim is **not** a source-code formatter and has **no plugin system**. It is the
single static binary that tidies the files no other formatter owns.

- **One canonical style** — honors `.editorconfig`, nothing else. No
  `prim.toml`.
- **Semantics-preserving** — never reorders keys, entries, or array elements.
- **Safe by default** — unparseable or non-UTF-8 files are left untouched and
  reported; writes are atomic.

> **Status:** v1 complete — all v1 requirements (FR-1 through FR-6) are
> implemented: recursive discovery, whitespace hygiene, `.editorconfig` style
> resolution, structured JSON/JSONC, TOML, YAML, and Markdown formatting (with
> prose-wrap guardrails), `fmt`/`lint`/`fix` verbs (with `--check` / `--diff` /
> `--stdin-filepath` as deprecated top-level sugar), and atomic writes. See
> [docs/SPEC.md](docs/SPEC.md).

## Install

```bash
# Prebuilt binary (verifies SHA-256, installs to ~/.local/bin, sets up completions)
curl -sSfL https://raw.githubusercontent.com/driftsys/prim/main/install.sh | bash

# …or from crates.io (crate is prim-cli; binary is prim)
cargo install prim-cli
```

Prebuilt binaries are published for Linux (x86-64/ARM64), macOS (Intel/Apple
Silicon), and Windows (x86-64). For the full platform matrix, manual download
with checksum verification, and building from source, see
[docs/installation.md](docs/installation.md).

## Usage

```bash
prim README.md config.yaml     # format files in place (bare alias for `fmt`)
prim fmt --check .             # CI gate: non-zero if anything would change
prim fmt --diff config.toml    # preview pending changes
prim fmt --stdin-filepath x.md # editor format-on-save (stdin → stdout)
prim lint .                    # report-only: hygiene + content violations
```

See the [user guide](https://driftsys.github.io/prim/) and
[docs/USAGE.md](docs/USAGE.md) for the full command reference.

## Development

```bash
git clone https://github.com/driftsys/prim.git
cd prim
./bootstrap     # installs git-std, configures git hooks
just build      # compile + test + lint
just verify     # full pre-PR gate
```

See [CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE) © driftsys
