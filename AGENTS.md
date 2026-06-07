# AGENTS.md

Instructions for AI coding agents working in this repository.

## Project

prim is a single Rust CLI binary — an opinionated, near-zero-config formatter
for a repository's connective tissue (Markdown, JSON/JSONC, YAML, TOML) plus
whitespace hygiene on a curated allowlist of un-owned text files. It is **not**
a source-code formatter and has **no plugin system**.

Invoked as `prim`. The full specification lives in [docs/SPEC.md](docs/SPEC.md).

> **Status:** early. Recursive discovery and the format-agnostic **whitespace
> hygiene** pass (trailing-whitespace removal, single final line-feed, LF
> endings) are implemented and wired through the `prim-fmt` engine. The
> per-format structured passes (JSON/YAML/TOML/Markdown), `.editorconfig`
> resolution, and atomic writes are follow-up milestones.

## Build commands

```bash
cargo test <test_name>  # Run a single test
just assemble           # Compile
just test               # Run all tests
just lint               # Lint + format check
just check              # Tests + install tests + lint
just build              # Assemble + check
just verify             # Commit lint + build — run before PR
just fmt                # Format Rust + Markdown
```

## Architecture

**Workspace structure — two crates plus an acceptance crate:**

| Crate       | Role                                                          |
| ----------- | ------------------------------------------------------------- |
| `prim-fmt`  | LIBRARY — the formatting engine. Pure, no CLI dependencies.   |
| `prim-cli`  | BINARY (`prim`) — thin CLI: arg parsing, I/O, mode dispatch.  |
| `prim-spec` | test-only (`spec/`) — `trycmd` CLI snapshots + install tests. |

`prim-fmt` is the reusable engine; keep it free of clap/terminal dependencies so
other crates can consume it. `prim-cli` orchestrates: it reads files or stdin,
routes them through `prim_fmt::format`, and maps the outcome to an exit code.

**Command surface — one command, no subcommands:**

| Invocation                   | Purpose                                                 |
| ---------------------------- | ------------------------------------------------------- |
| `prim [PATH]...`             | Format the given files in place (default).              |
| `prim --check [PATH]...`     | CI gate: exit `1` and list files that would change.     |
| `prim --diff [PATH]...`      | Print a unified diff of pending changes; write nothing. |
| `prim --stdin-filepath <p>`  | Read stdin, write formatted result to stdout.           |
| `prim --completions <shell>` | Generate shell completion scripts.                      |

**Exit codes:** `0` success · `1` changes needed (`--check`) · `2` error
(parse/IO).

**Key design decisions:**

- One canonical style; honor `.editorconfig` only. No `prim.toml`, no per-rule
  flags.
- Semantics-preserving: never reorder keys, table entries, or array elements.
- Fail-safe: unparseable or non-UTF-8 files are left byte-for-byte unchanged and
  reported (exit `2`). Writes are atomic (temp file + rename).
- `.primignore` (gitignore syntax) is the committed escape hatch for
  tracked-but-unformatted files.

**Key dependencies:** `clap` (CLI), `clap_complete`/`clap_mangen` (completions +
man pages), `yansi` (colour), `ignore` (recursive file discovery).

## Workflow

Follow [CONTRIBUTING.md](CONTRIBUTING.md) for the issue model, PR process, and
review flow.

**Agent-specific rules:**

- **Start from the issue.** Read the acceptance criteria and `docs/SPEC.md`,
  propose an approach, and wait for approval before implementing.
- **ATDD + TDD.** Write the failing acceptance/behaviour test first, then TDD
  the unit tests and implementation. Three test layers:
  - `spec/` — blackbox CLI-output snapshots (`trycmd`) + install-script tests.
  - `crates/prim-cli/tests/` — behavioural integration tests that drive the
    `prim` binary against real temp files and stdin.
  - `#[cfg(test)]` inline modules — unit tests for library logic.
- **Single PR = code + tests + docs.** Every pull request ships implementation,
  tests, and updated documentation together.
- **Commits.** Use Conventional Commits — `feat`, `fix`, `refactor`, `docs`,
  `test`, `chore`. Imperative mood.
- **Before PR.** Run `just verify` — all must pass.
- **PR-based workflow — never push directly to `main`.**

## Module structure

Group modules by domain concept. Keep each module focused and small.

- **One concept per module.** Name modules after what they do (`format`,
  `discover`, `editorconfig`, `cli`). Never `utils`, `helpers`, or `common`.
- **`lib.rs` is an index.** Re-exports and submodule declarations — no logic.
- **File size.** Soft limit 300 lines, hard limit 500.
- **Crate boundaries.** `prim-fmt` stays pure (no clap/I/O/terminal). Only
  `prim-cli` does I/O and dispatch. Per-format parsers may later split into
  their own `prim-*` crates under the engine.

## Conventions

- **Zero warnings.** No warnings anywhere — compiler, `cargo test`, `clippy`, or
  Markdown (`dprint` + markdownlint). Do not suppress with `#[allow(...)]`
  unless unavoidable, and document the reason.
- **Code style:** `rustfmt` + `clippy`. Always run `just fmt` before committing.
- **Naming.** Names must reveal intent. Avoid `temp`, `data`, `flag`. Booleans
  use `is_`/`has_`/`can_`. No `get_` prefix. **Rust API guidelines and `clippy`
  supersede these when they conflict.**
- **Error handling.** `prim-fmt` (library) uses `thiserror` — typed, matchable
  error enums are part of the public contract (as parsers land). `prim-cli`
  (binary) maps outcomes to exit codes and prints via `ui::` helpers.
- **UI consistency.** Follow [clig.dev](https://clig.dev):
  - Human output → stderr via `ui::` helpers. The machine-readable `--check`
    file list goes to stdout.
  - Exit codes are the contract: `0` / `1` / `2` as above.
  - Colour via `yansi`, gated by `--color` and TTY detection. Respect
    `NO_COLOR`.
- **Comments:** doc comments on all public API items; brief inline comments on
  tricky internals only.

<!-- git-std:bootstrap -->

## Post-clone setup

Run `./bootstrap` after `git clone` or `git worktree add`.
