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

The project is a Cargo workspace with one library crate, one binary crate, and a
test-only acceptance crate:

```text
prim/
├── crates/
│   ├── prim-fmt/             # LIBRARY — the formatting engine (no CLI deps)
│   └── prim-cli/             # BINARY  — `prim`; thin CLI over prim-fmt
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

- Rust code must pass `cargo fmt` and `cargo clippy` with no warnings.
- Markdown files must pass `dprint check`.
- Always run `just fmt` before committing.
