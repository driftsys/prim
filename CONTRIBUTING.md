# Contributing to prim

For org-wide guidelines — AI policy, commit messages, pull request workflow,
code review, issue model, and documentation style — see the
[driftsys contributing guide][org-contributing] and [process][org-process].

This file covers what is specific to the prim repository.

[org-contributing]: https://github.com/driftsys/.github/blob/main/CONTRIBUTING.md
[org-process]: https://github.com/driftsys/.github/blob/main/PROCESS.md

## Reporting issues

Open bugs and feature requests at <https://github.com/driftsys/prim/issues>.

## Dev setup

You need the Rust toolchain and a few extra tools:

- **Rust**: stable toolchain (install via [rustup](https://rustup.rs))
- **[just]**: command runner
- **[dprint]**: Markdown formatter
- **[cargo-audit]**: dependency auditor

```bash
git clone https://github.com/driftsys/prim.git
cd prim
./bootstrap          # post-clone setup: installs git-std, configures hooks
just build
```

[just]: https://github.com/casey/just
[dprint]: https://dprint.dev
[cargo-audit]: https://github.com/rustsec/rustsec

## Architecture

The project is a Cargo workspace with one engine library crate, one binary crate
that also carries an internal library target (AD-0020), and a test-only
acceptance crate:

```text
prim/
├── crates/
│   ├── prim-fmt/             # LIBRARY — the formatting engine (no CLI deps)
│   └── prim-cli/             # BINARY  — `prim`; thin CLI (+ internal lib)
├── spec/                     # test-only acceptance crate (trycmd + install tests)
├── docs/
│   └── SPEC.md               # full specification
└── .githooks/                # hook definitions (managed by git-std)
```

**Design principle:** `prim-fmt` is pure domain logic — strings in, strings out,
no CLI dependencies — so other crates can depend on it without pulling in clap.
`prim-cli` is the orchestrator: argument parsing, file/stdin I/O, operating-mode
dispatch, and terminal output, all wired over `prim-fmt`.

Read [docs/SPEC.md](docs/SPEC.md) for the full specification.

## Testing

```bash
just test               # Run all tests
cargo test <test_name>  # Run a specific test
just check              # Tests + install tests + lint
just verify             # Full pre-PR gate (commit lint + build)
```

`just verify` lints the commits in `origin/main..HEAD`, the range your pull
request will be reviewed against. When every commit on your branch is already on
`origin/main`, that range is empty, so commit lint is skipped and only the build
runs. The recipe reads `origin/main` and never fetches it, so the range is as
fresh as your last `git fetch`, and an `origin` remote is required.

### Test conventions

- **Acceptance / CLI-snapshot tests** go in `spec/` — blackbox `trycmd` cases
  (binary input/output only) plus the `install.sh` `bash_unit` tests.
- **Behavioural integration tests** go in `crates/prim-cli/tests/` — they drive
  the `prim` binary against real temp files and stdin. They live in the bin
  crate (not `spec/`) so cargo provides `CARGO_BIN_EXE_prim` for reliable binary
  resolution.
- **Unit tests** go inline in `#[cfg(test)]` modules alongside the code.
- Follow ATDD + TDD: write the failing acceptance/behaviour test first, then TDD
  the implementation.

## Code style

```bash
just fmt    # Format Rust + Markdown
just lint   # Lint + format check
```

- Rust code must pass `cargo fmt`, `cargo clippy`, and rustdoc with no warnings.
  `just lint` runs all three; its rustdoc step passes `-D warnings` through
  `RUSTDOCFLAGS`, because `cargo doc` has no `--` passthrough. It also passes
  `--document-private-items`, without which `cargo doc` skips a library's
  internals and a broken link in `prim-fmt` goes unseen.
- Markdown files must pass `dprint check`.
- Always run `just fmt` before committing.
