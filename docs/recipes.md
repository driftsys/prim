# Recipes

## CI formatting gate

Fail the build when any tracked file is not formatted:

```yaml
- name: Check formatting
  run: prim fmt --check .
```

`prim fmt --check` writes nothing, exits `0` when everything is already
formatted, and exits `1` (listing the offending files) otherwise. The top-level
`prim --check` spelling still works as deprecated sugar (warns on stderr;
removed in v2.0) — prefer `prim fmt --check` in new pipelines.

## Editor format-on-save

Point your editor's "format with external command" hook at:

```bash
prim fmt --stdin-filepath "$FILE"
```

prim reads the buffer on stdin and writes the formatted result to stdout. The
path is used only to select the right formatter.

## Excluding files

prim respects `.gitignore` and `.ignore` automatically. To exclude a **tracked**
file from formatting (for example a deliberately malformed test fixture, or a
generated `CHANGELOG.md`), add it to a committed `.primignore` using gitignore
syntax:

```gitignore
# .primignore
CHANGELOG.md
fixtures/malformed.json
```

## Protecting golden files

Test fixtures and golden files often contain deliberate formatting violations
(trailing whitespace, missing final newlines, non-canonical indentation) that
must stay byte-exact. Add those directories to `.primignore` — prim's own
repository does this for its test fixtures:

```gitignore
# .primignore
crates/prim-fmt/tests/correctness/fixtures/
```

Note: `--exclude` and `.primignore` apply to directory walks; a file named
explicitly on the command line is always processed.

## Using prim with git-std

`git-std` generates `CHANGELOG.md`, which prim would otherwise hard-wrap as
Markdown. In repositories using both tools, add `CHANGELOG.md` to `.primignore`
(prim ships this entry by default).

### Wiring prim into a git-std pre-commit hook

`git-std hook run` already resolves the staged-file list for you — a glob at the
end of a `.githooks/pre-commit.hooks` line restricts `$@` to matching staged
files, and the `~` (fix) sigil stashes unstaged changes, runs the command, then
re-stages the result. prim needs none of that plumbing duplicated: pass it
whatever files git-std gives it, and prim's own file-type detection skips
anything it doesn't own (a `.rs`/`.sh` file, for example) with a warning instead
of failing:

```text
# .githooks/pre-commit.hooks
~ prim fmt $@
```

No glob is required — an explicit glob like `*.{md,json,yaml,toml}` also works
and avoids invoking prim on staged files it will just skip, but it is an
optimization, not a correctness requirement. prim's own repository wires itself
this way; see `.githooks/pre-commit.hooks`.

### Using prim with the `pre-commit` framework

For repositories using the separate [pre-commit](https://pre-commit.com)
framework instead of (or alongside) git-std, prim ships a
`.pre-commit-hooks.yaml` manifest at the root of this repository. Reference it
from a consumer repository's `.pre-commit-config.yaml`:

```yaml
repos:
  - repo: https://github.com/driftsys/prim
    rev: v0.2.2 # pin to a released tag
    hooks:
      - id: prim
```

The hook uses `language: system`, so prim must already be on `PATH` (install it
with the [install script](getting-started.md) or `cargo install`). The
`pre-commit` framework itself narrows the argument list to staged files matching
the hook's `types`, the same way git-std's `$@` does — prim never needs to
re-derive that list itself.
