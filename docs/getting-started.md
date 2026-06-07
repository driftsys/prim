# Getting started

## Install

### From a release (recommended)

```bash
curl -sSfL https://raw.githubusercontent.com/driftsys/prim/main/install.sh | bash
```

This downloads the prebuilt `prim` binary for your platform, verifies its
SHA-256 checksum, installs it to `~/.local/bin`, and sets up shell completions.

### From crates.io

```bash
cargo install prim-cli
```

The crate is `prim-cli`; the installed binary is `prim`.

### From source

```bash
git clone https://github.com/driftsys/prim
cd prim
./bootstrap          # installs git-std and configures git hooks
cargo build --release
```

## First run

```bash
prim --version
prim --help
```

prim formats files in place, or a whole directory tree:

```bash
prim README.md config.yaml   # specific files
prim .                       # the current directory, recursively
```

> **Note:** at this early stage prim applies **whitespace hygiene**
> (trailing-whitespace removal, single final line-feed, LF endings) to the
> parsed formats and the orphan allowlist. Structured per-format formatting
> (JSON/YAML/TOML/Markdown) lands in later milestones.
