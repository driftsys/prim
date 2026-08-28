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

A repository that is not formatted yet cannot adopt this gate on day one; see
[Incremental adoption on an unformatted repository](#incremental-adoption-on-an-unformatted-repository)
for the two ways in.

## Incremental adoption on an unformatted repository

The gate above fails on every pre-existing file the first time it runs in a
repository that has never been formatted. Two strategies reach a green gate.
Choose by whether a large one-off reformatting commit is acceptable.

### Format as you touch

Gate only the files a change already modifies. The gate is green from the first
commit, and coverage grows as the repository is edited:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0 # a merge base needs the history on both sides
- name: Check formatting of changed files
  run: prim fmt --check --since "$(git merge-base origin/${{ github.base_ref }} HEAD)" .
```

Compare against the merge base, not against the branch name. `--since <REF>` is
a plain two-way `git diff --name-only <REF>`: it reports every path that differs
between `<REF>` and the working tree, in either direction. Naming the branch
directly also matches files that changed **on `main`** after the branch point
and that this branch never touched — the gate then fails on an unrelated
unformatted file. (A branch that has already merged `main` in does not have this
problem, which is why it can pass for a while and then start failing.) The REF
is handed to `git diff` unchanged, so any revision expression works, a
merge-base SHA included.

Locally the same filter formats rather than checks:

```bash
prim fmt --since "$(git merge-base main HEAD)" .
```

In a pre-commit hook use `--staged` instead. It selects the paths in the index
relative to `HEAD`, which is exactly what the commit will contain:

```bash
prim fmt --staged .
```

Two limits are worth knowing before relying on either flag as a gate:

- They see only what git reports. An **untracked** file does not appear in
  `git diff` output at all, so it is not gated until it is added to the index —
  `prim fmt --check --since HEAD .` passes on a brand-new file that
  `prim fmt --check .` would fail. Deleted paths that git does report are
  skipped silently.
- Both require the working directory to be inside a git working tree, and
  `--since` requires its REF to resolve. A shallow `actions/checkout` clone has
  no merge base to resolve, so prim exits `2` — a usage error, not a format
  finding — rather than passing silently. That is what `fetch-depth: 0` above
  prevents.

The filters intersect with the rest of discovery rather than replacing it, so
`.primignore`, `--exclude`, and the ignore files still apply. `--since` and
`--staged` are mutually exclusive.

### Format once and record the exceptions

The alternative is to reformat the whole tree in a single commit, add whatever
must stay byte-exact to `.primignore` (see
[Protecting golden files](#protecting-golden-files)), and use the full
`prim fmt --check .` gate from then on:

```bash
prim fmt .
git commit -am "style: format the repository with prim"
```

This keeps every later diff free of formatting churn, at the cost of one commit
that touches many files. Record that commit in a `.git-blame-ignore-revs` file
and point git at it (`git config blame.ignoreRevsFile .git-blame-ignore-revs`)
so it does not obscure `git blame` output.

Format-as-you-touch suits a repository where a large reformatting commit would
conflict with in-flight branches or is not permitted by review policy.
Formatting once suits everything else: it is a shorter adoption period and a
simpler gate. Either way, once the whole tree is formatted, drop `--since` and
go back to the [CI formatting gate](#ci-formatting-gate).

## CI Markdown lint gate

Fail the build when a Markdown file has a content-lint finding:

```yaml
- name: Lint Markdown content
  run: prim lint .
```

`prim lint` never rewrites; it reports findings and exits `1` when any are
present. For Markdown specifically, every rule prim runs — the always-on floor
tier, plus the strict tier once a path opts in via `.editorconfig`
`prim_mdlint_strict = true` — is an error, so there is no silent warn-tier pass
to worry about: if `prim lint` prints a Markdown finding, the gate fails. See
[USAGE.md](USAGE.md#operating-modes) for the floor/strict rule lists.

## Excluding a Markdown lint rule for a tree

A convention rule can legitimately fire across an entire tree — for example
MD033 (inline HTML) on a docs tree that intentionally uses `<kbd>` or
`<img align="right">`. Rather than adding a
`<!-- markdownlint-disable-file MD033 -->` comment to every affected file,
exclude the rule once per glob:

```ini
[docs/**.md]
prim_mdlint_strict = true
prim_mdlint_disable = MD033, MD041
```

`prim_mdlint_disable` only removes rules from the tier prim already selected for
that path — it is subtract-only, so it can never turn on a rule prim decided not
to run. See [USAGE.md](USAGE.md#configuration) for the full resolution
semantics.

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

A lockfile needs no entry of its own: `package-lock.json`,
`npm-shrinkwrap.json`, `pnpm-lock.yaml`, and `packages.lock.json` are declined
outright by a built-in list (AD-0011), because prim can always tell that these
four are generated. `CHANGELOG.md` still needs the `.primignore` entry above —
some projects hand-author it, so prim cannot tell a generated changelog from a
hand-written one by name alone.

## Protecting golden files

Test fixtures and golden files often contain deliberate formatting violations
(trailing whitespace, missing final newlines, non-canonical indentation) that
must stay byte-exact. Add those directories to `.primignore` — prim's own
repository does this for its test fixtures:

```gitignore
# .primignore
crates/prim-fmt/tests/correctness/fixtures/
```

Note: `.primignore` applies however prim is invoked — a file it covers is left
alone whether prim walked to it or you named it on the command line (AD-0009).
That is what makes the entry worth having in a pre-commit hook, which passes
prim an explicit list of staged files. Naming an ignored path prints a warning
so the no-op is visible; pass `--no-primignore` to process it anyway.

A `.primignore` governs only the repository that holds it. prim reads the
`.primignore` files that apply from the path upward, stopping at the root of the
repository it was pointed at — the nearest directory holding a `.git` entry,
which is a file rather than a directory in a git worktree. So running prim
inside a nested checkout or a worktree uses that checkout's own `.primignore`,
even when it sits at a path the enclosing repository's `.primignore` names.
Running prim on the enclosing repository still prunes it, the way a `.gitignore`
entry would.

`--exclude` globs still apply to directory walks only.

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
