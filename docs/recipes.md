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
for the two routes to it.

## Incremental adoption on an unformatted repository

The gate above reports every pre-existing file that is not already in prim's
canonical form, which in a repository that has never been formatted can be most
of the tree. Two strategies get that gate passing. Choose by whether a large
one-off reformatting commit is acceptable.

### Format as you touch

Gate only the files a change already modifies. The gate passes from the first
commit, and coverage grows as the repository is edited:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0 # a merge base needs the history on both sides
- name: Check formatting of changed files
  run: prim fmt --check --since "$(git merge-base origin/${{ github.base_ref }} HEAD)" .
```

That snippet is for a `pull_request`-triggered workflow: `github.base_ref` is
empty on a `push` event, which leaves prim with `origin/` and exits `2`.

Compare against the merge base, not against the branch name. `--since <REF>` is
a plain two-way `git diff --name-only <REF>`: it reports every path that differs
between `<REF>` and the working tree, in either direction. Naming the branch
directly also matches files **modified on `main`** after the branch point that
this branch never touched — the gate then fails on an unrelated unformatted
file. A branch cut from the tip of `main` therefore passes at first and starts
failing as `main` advances underneath it, and a branch that has already merged
`main` in does not hit the problem at all. (Only modifications trigger it: a
file _added_ on `main` is a deletion relative to this branch's working tree, and
prim drops those silently.) The REF is handed to `git diff` unchanged, so any
revision expression `git diff` accepts works, a merge-base SHA included — though
a name that is also a path in the tree, such as a `docs` branch beside a `docs/`
directory, is ambiguous to `git diff` and exits `2`.

Locally the same filter formats rather than checks:

```bash
prim fmt --since "$(git merge-base main HEAD)" .
```

`--staged` applies the same idea to the index: it selects the paths
`git diff --name-only --cached` reports, which is what a commit is about to
contain. Use it to report on a commit before making it:

```bash
prim fmt --check --staged .
```

The index chooses the paths, but prim still reads each one from the working
tree, so the report describes the commit only for files staged in full. A file
staged in part is reported on its working-tree content, including the part that
is not being committed.

Do **not** reach for `prim fmt --staged .` as a pre-commit hook on its own.
`--staged` only chooses which paths prim looks at; prim then writes the
formatted bytes to the **working-tree** file and never touches the index, so
`git commit` still records the unformatted blob that was staged before prim ran.
A hook has to re-stage the result, and for a partially staged file it has to
deal with the unstaged remainder first. Both existing hook recipes below —
[git-std](#wiring-prim-into-a-git-std-pre-commit-hook) and
[pre-commit](#using-prim-with-the-pre-commit-framework) — already do that and
pass prim the staged list themselves, so a hook needs neither of these flags.

Two limits are worth knowing before relying on either flag as a gate:

- They see only what git reports. An **untracked** file does not appear in
  `git diff` output at all, so it is not gated until it is added to the index —
  `prim fmt --check --since HEAD .` passes on a brand-new file that
  `prim fmt --check .` would fail. Deleted paths that git does report are
  skipped silently.
- Both require the working directory to be inside a git working tree, and
  `--since` requires git to resolve its REF. On a shallow `actions/checkout`
  clone there is no merge base, so the `git merge-base` substitution in the
  workflow above fails in the shell and prim is handed an empty REF. prim then
  exits `2` — a usage error, not a format finding — rather than passing
  silently. That is what `fetch-depth: 0` above prevents.

Both filters intersect with the rest of discovery rather than replacing it, so
`.primignore`, `--exclude`, and the ignore files still apply, and the two are
mutually exclusive. See [USAGE.md](USAGE.md#operating-modes) for the full
semantics.

### Format once and record the exceptions

The alternative is to reformat the whole tree in a single commit, add whatever
must stay byte-exact to `.primignore` (see
[Protecting golden files](#protecting-golden-files)), and use the full
`prim fmt --check .` gate from then on:

```bash
prim fmt .
git commit -am "style: format the repository with prim"
git rev-parse HEAD >> .git-blame-ignore-revs
```

Make that commit on an otherwise clean tree: `git commit -am` also sweeps in any
unrelated pending edit, and this is the commit you least want anything else
mixed into.

This keeps every later diff free of formatting churn, at the cost of one commit
that touches many files. Point git at the ignore file
(`git config blame.ignoreRevsFile .git-blame-ignore-revs`) so that commit does
not obscure `git blame` output.

Format-as-you-touch suits a repository where a large reformatting commit would
conflict with branches already open, or is not permitted by review policy. Its
running cost is the mirror image of that: a change touching one line of an
unformatted file reformats the whole file, so every diff carries reformatting
that has nothing to do with the change under review, for as long as adoption
takes. Formatting once suits everything else: it is a shorter adoption period
and a simpler gate. Either way, once the whole tree is formatted, drop `--since`
and go back to the [CI formatting gate](#ci-formatting-gate).

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

`prim_mdlint_disable` only removes rules; adding one is the separate
`prim_mdlint_enable` key below. prim applies the disable list after the enable
list, so an id named by both keys does not run. See
[USAGE.md](USAGE.md#configuration) for the full resolution semantics.

## Adding one Markdown lint rule without the strict tier

The strict tier is all thirteen convention rules or none. To adopt a single one
— say MD040, which asks every fenced code block for a language tag — name it
with `prim_mdlint_enable` and leave the tier alone:

```ini
[docs/**.md]
prim_mdlint_strict = false
prim_mdlint_enable = MD040
```

The rule runs regardless of tier, so this works from a floor-tier path and
survives a file-level `<!-- prim-mdlint-strict: false -->`. The key reaches the
26 rules in prim's own floor and strict tiers, plus MD013, MD014 and MD069; any
other id is refused with a warning on stderr that names the `.editorconfig`
line, and the exit code is unaffected.

## Enforcing a line length on Markdown

MD013 is off in both tiers. Enable it, and set the width with the standard
EditorConfig key rather than a rule option:

```ini
[docs/**.md]
max_line_length = 100
prim_mdlint_enable = MD013
```

prim feeds MD013 the same width the formatter wrapped to, so the linter and the
formatter can never disagree about the threshold.

**Read this before you enable it: prim's MD013 checks headings only.** It does
not report over-width paragraphs, code-block lines, or table rows. That is
deliberate. `prim fmt` already wraps every ordinary paragraph to
`max_line_length`, so a paragraph line still over the limit afterwards is one
prim could not break without changing what it means — a long inline code span,
an HTML tag's attributes, a display-math line, or prose inside a raw HTML block
— and reporting it would hand the author a finding with no correct fix. Measured
across 774 files from six public documentation sites, every single non-heading
finding was of that kind. A long heading is the opposite case: prim never wraps
headings, and a heading can be rewritten shorter, so headings stay checked.

prim has no per-rule option surface, so there is no way to widen MD013 back to
whole lines. A repository that needs whole-line enforcement needs a different
tool. See [AD-0012](decisions/0012-markdown-lint-bands-and-rule-exclusion.md)
for the measurement.

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
