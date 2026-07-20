# Installation

prim ships as a single self-contained binary named `prim`. Pick whichever method
suits your platform; all of them install the same binary.

## Supported platforms

Prebuilt binaries are published for each release and cover:

| Platform | Architecture             | Target triple                |
| -------- | ------------------------ | ---------------------------- |
| Linux    | x86-64                   | `x86_64-unknown-linux-musl`  |
| Linux    | ARM64 / aarch64          | `aarch64-unknown-linux-musl` |
| macOS    | Intel (x86-64)           | `x86_64-apple-darwin`        |
| macOS    | Apple Silicon (M-series) | `aarch64-apple-darwin`       |
| Windows  | x86-64                   | `x86_64-pc-windows-msvc`     |

The Linux builds are statically linked against musl, so they run on any
distribution without a libc dependency. Any platform with a Rust toolchain can
also build prim [from source](#from-source) or install it
[from crates.io](#from-cratesio).

## Install script (recommended)

```bash
curl -sSfL https://raw.githubusercontent.com/driftsys/prim/main/install.sh | bash
```

The script detects your platform, downloads the matching prebuilt from the
latest GitHub release, **verifies its SHA-256 checksum**, installs the binary,
installs the man page, and sets up shell completions.

It respects two environment variables:

| Variable           | Default                   | Purpose                        |
| ------------------ | ------------------------- | ------------------------------ |
| `PRIM_INSTALL_DIR` | `~/.local/bin`            | Where the `prim` binary lands. |
| `PRIM_MAN_DIR`     | `~/.local/share/man/man1` | Where the man page lands.      |

```bash
# Install to /usr/local/bin instead:
curl -sSfL https://raw.githubusercontent.com/driftsys/prim/main/install.sh \
  | PRIM_INSTALL_DIR=/usr/local/bin bash
```

If the install directory is not on your `PATH`, the script prints a note; add it
(for example `export PATH="$HOME/.local/bin:$PATH"`) to your shell profile.

> **Windows:** the install script targets Unix shells. On Windows, either use it
> under [WSL](https://learn.microsoft.com/windows/wsl/) or
> [download the binary manually](#manual-download). A native
> `x86_64-pc-windows-msvc` build is published with every release.

## From crates.io

```bash
cargo install prim-cli
```

The crate is named `prim-cli`; the installed binary is `prim`. This compiles
prim locally, so it works on any target with a Rust toolchain — including ones
without a prebuilt.

## Manual download

Every release attaches a `prim-<target>.tar.gz` tarball and a matching `.sha256`
checksum file. To install by hand, verifying the checksum:

```bash
VERSION=v1.0.0                  # the release tag you want
TARGET=aarch64-apple-darwin     # your target triple from the table above
BASE="prim-$TARGET"
URL="https://github.com/driftsys/prim/releases/download/$VERSION"

# Download the tarball and its checksum.
curl -sSfLO "$URL/$BASE.tar.gz"
curl -sSfLO "$URL/$BASE.tar.gz.sha256"

# Verify (use `shasum -a 256 -c` on macOS if `sha256sum` is absent).
sha256sum -c "$BASE.tar.gz.sha256"

# Unpack and install onto your PATH.
tar -xzf "$BASE.tar.gz"
install -m 0755 prim ~/.local/bin/prim
```

Each tarball also contains prim's man page (`prim.1`); copy it into a `man1`
directory on your `MANPATH` if you want `man prim`.

## From source

```bash
git clone https://github.com/driftsys/prim
cd prim
./bootstrap          # installs git-std and configures git hooks
cargo build --release
```

The binary is written to `target/release/prim`.

## After installing

- Verify: `prim --version`.
- Shell completions and the man page are set up automatically by the install
  script; for other methods, generate completions yourself with
  `prim --completions <shell>` (see [Usage](USAGE.md#options)).
