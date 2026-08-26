# Design — `--diff` unified-diff rendering (FR-5.3) · issue #14

**Status:** approved (autonomous loop) · **Branch:** `feat/diff-output` ·
**Epic:** #4

Replace the no-op `--diff` branch in `prim-cli::app::run_paths` with a real
unified diff of pending changes, printed to stdout, writing nothing.

## Requirements

- **FR-5.3** `--diff` prints a unified diff of pending changes and writes
  nothing.
- **FR-5.5** Exit `1` is the `--check` signal; `--diff` is a preview and exits
  `0` (or `2` on error).

## Decision: `similar` crate, in a `prim-cli` `diff` module

- `similar = "3"` — the standard Rust text-diff crate.
  `TextDiff::from_lines(old, new).unified_diff().header("a/<p>", "b/<p>")`
  renders a conventional unified diff (`--- a/p` / `+++ b/p`, `@@` hunks,
  `-`/`+` lines) via its `Display` impl.
- Diff rendering is a CLI concern → a new `prim-cli/src/diff.rs`:
  `pub fn unified(path, original, formatted) -> String` (empty when identical).
- The diff is **content output → stdout** (like the `--check` file list), per
  clig.dev.

## Architecture

```text
prim-cli
  diff.rs  NEW  unified(path, original, formatted) -> String  (similar)
  app.rs        run_paths: the `--diff` arm prints diff::unified(...) to stdout
  main.rs       mod diff;
```

`run_paths` already computes `original` and `formatted` and only enters the mode
arms when they differ. The `--diff` arm becomes:

```rust
} else if cli.diff {
    print!("{}", diff::unified(&file.path, &original, &formatted));
}
```

Exit logic is unchanged: `cli.check` is false in `--diff` mode, so a pending
change does not trigger exit 1 — `--diff` exits 0 (FR-5.5). Plain (uncolored)
diff for v1; colorization via `yansi` is a possible follow-up.

## Testing (strict TDD)

- **`prim-cli` `diff` units:** `unified()` of differing inputs contains the
  `--- a/…` / `+++ b/…` headers and `-old`/`+new` lines; identical inputs →
  empty string.
- **`prim-cli` behavioural (`tests/diff.rs`):** `prim --diff <messy.json>`
  prints a unified diff to stdout, leaves the file unchanged, exits 0;
  `prim --diff` on an already-canonical file prints nothing and exits 0;
  `--diff` never writes.

## File plan

| File                                                | Change                             |
| --------------------------------------------------- | ---------------------------------- |
| `crates/prim-cli/src/diff.rs`                       | NEW — `unified(...)` via `similar` |
| `crates/prim-cli/src/main.rs`                       | `mod diff;`                        |
| `crates/prim-cli/src/app.rs`                        | `--diff` arm prints the diff       |
| `crates/prim-cli/Cargo.toml`                        | add `similar = "3"`                |
| `crates/prim-cli/tests/diff.rs`                     | NEW — behavioural coverage         |
| `AGENTS.md`/`docs/USAGE.md`/`docs/design/system.md` | `--diff` now implemented           |

## Out of scope

- Colorized diff output (follow-up).
- `--diff` for `--stdin-filepath` (stdin always emits the formatted result).
