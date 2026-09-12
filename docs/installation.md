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

Every release attaches exactly one `prim-<target>.tar.gz` archive and matching
`prim-<target>.tar.gz.sha256` checksum for each supported target. Those names
are stable and version-addressable below `releases/download/<tag>/`. The
checksum file names the archive by its base name, so it can be verified after
downloading both files into any directory. To install by hand:

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

## Release signatures, SBOM, and provenance

Starting with **v0.9.0**, releases use this contract. v0.8.0 supplies the five
platform archives and their checksums; it does not supply the signature, SBOM,
or provenance assets described below. A supporting release is published only
after its tagged hosted build passes the verification gate.

Each supporting release contains exactly 22 assets: four per supported target
and two for the release as a whole.

| Asset                                           | Purpose                                       |
| ----------------------------------------------- | --------------------------------------------- |
| `prim-<target>.tar.gz`                          | Binary and man page archive.                  |
| `prim-<target>.tar.gz.sha256`                   | SHA-256 checksum naming that archive.         |
| `prim-<target>.tar.gz.sigstore.json`            | Keyless cosign signature bundle.              |
| `prim-<target>.tar.gz.provenance.sigstore.json` | GitHub SLSA build-provenance bundle.          |
| `prim-<version>.spdx.json`                      | Release-wide SPDX 2.3 source/dependency SBOM. |
| `prim-<version>.spdx.json.sigstore.json`        | Keyless signature bundle for the SBOM.        |

`<version>` omits the tag's leading `v`. All assets are directly addressable
under `https://github.com/driftsys/prim/releases/download/v<version>/`; Folio
and other pinned-binary consumers do not need to run `install.sh`.

The reusable `release-build.yml` workflow builds, packages, signs, and attests
each archive. The caller `release.yml` generates the SBOM from the tagged source
and `Cargo.lock` using Syft v1.51.1, requires `spdxVersion` to equal `SPDX-2.3`,
and signs it. Consequently archive signatures identify the reusable workflow,
while the SBOM signature identifies the caller.

To verify an archive from a supporting release, install cosign v3 and the GitHub
CLI, authenticate `gh`, and run:

```bash
VERSION=vX.Y.Z                 # replace with a supporting release tag
TARGET=aarch64-apple-darwin
ARCHIVE="prim-$TARGET.tar.gz"
URL="https://github.com/driftsys/prim/releases/download/$VERSION"
IDENTITY="https://github.com/driftsys/prim/.github/workflows/release-build.yml@refs/tags/$VERSION"
COMMIT=$(gh api "repos/driftsys/prim/commits/$VERSION" --jq .sha)

for ASSET in "$ARCHIVE" "$ARCHIVE.sha256" "$ARCHIVE.sigstore.json" "$ARCHIVE.provenance.sigstore.json"; do
  curl -sSfLO "$URL/$ASSET"
done
shasum -a 256 -c "$ARCHIVE.sha256"
cosign verify-blob --bundle "$ARCHIVE.sigstore.json" \
  --certificate-identity "$IDENTITY" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$ARCHIVE"
gh attestation verify "$ARCHIVE" \
  --bundle "$ARCHIVE.provenance.sigstore.json" \
  --repo driftsys/prim \
  --signer-workflow driftsys/prim/.github/workflows/release-build.yml \
  --source-ref "refs/tags/$VERSION" --source-digest "$COMMIT" \
  --signer-digest "$COMMIT" --deny-self-hosted-runners \
  --format json > verified-provenance.json
DIGEST=$(shasum -a 256 "$ARCHIVE" | cut -d ' ' -f 1)
jq -e --arg name "$ARCHIVE" --arg digest "$DIGEST" '
  length > 0 and all(.[];
    .verificationResult.statement.subject == [{name: $name, digest: {sha256: $digest}}]
  )' verified-provenance.json
```

`gh attestation verify` validates SLSA v1 provenance and the archive digest. The
additional assertion requires the verified statement's subject name and digest
to match exactly. These checks follow the
[GitHub CLI verification contract](https://cli.github.com/manual/gh_attestation_verify).
Verify the release-wide SBOM using its caller identity:

```bash
SBOM="prim-${VERSION#v}.spdx.json"
curl -sSfLO "$URL/$SBOM"
curl -sSfLO "$URL/$SBOM.sigstore.json"
jq -e '.spdxVersion == "SPDX-2.3"' "$SBOM"
cosign verify-blob --bundle "$SBOM.sigstore.json" \
  --certificate-identity "https://github.com/driftsys/prim/.github/workflows/release.yml@refs/tags/$VERSION" \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com \
  "$SBOM"
```

The release verifier checks the exact asset set, checksums, signature identities
and issuer, provenance signer/source commit and tag, verified subjects, and SPDX
version. It also requires the tag's commit to be an ancestor of `origin/main`.
Both GitHub Release and crates.io publication depend on this job. GitHub Release
downloads the verifier's immutable artifact ID and publishes those bytes without
repackaging them. `install.sh` continues to verify SHA-256 only.

These checks do not by themselves establish SLSA Build Level 3. Protected
branch, tag, and release ruleset controls must be inspected to establish that
the tagged commit and reusable workflow were reviewed and cannot be replaced by
an unreviewed caller. That inspection, together with hosted OIDC signing and
verification evidence, is required to establish #55's provenance level. No SLSA
level is claimed until those controls and run evidence have been reviewed and
recorded. The existing pull-request and scheduled `cargo audit` CI gate remains
in place.

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
