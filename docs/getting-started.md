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

prim takes file paths and formats them in place:

```bash
prim README.md config.yaml
```

> **Note:** at the walking-skeleton stage the formatter is a no-op, so prim
> reports success and leaves files unchanged. Wiring, exit codes, and the
> command-line surface are real; the formatting logic lands in later milestones.
