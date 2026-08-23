# AD-0009 — `.primignore` covers explicitly named paths

## Status

Accepted. Supersedes the note in `docs/recipes.md` that said an explicitly named
path is always processed. Breaking: `prim fmt <ignored-path>` no longer rewrites
the file.

## Context

`.primignore` is prim's committed escape hatch (FR-4.4). The file prim ships in
its own repository describes it as "paths listed here are left byte-for-byte
unchanged by prim", and `docs/recipes.md` recommends it for two jobs: protecting
a generated `CHANGELOG.md` that `git std bump` owns, and protecting byte-exact
golden fixtures whose whole purpose is to carry non-canonical whitespace.

Until now the guarantee held only for directory walks. A path named on the
command line bypassed `.primignore` entirely, which `docs/recipes.md` documented
as deliberate.

That exemption does not survive contact with how prim is actually invoked. Every
hook wiring prim ships passes explicit paths:

- `.githooks/pre-commit.hooks` runs `prim fmt $@`, where `$@` is git-std's
  staged-file list.
- `.pre-commit-hooks.yaml` declares `entry: prim fmt`, which the pre-commit
  framework narrows to staged paths.

So prim's own pre-commit hook would rewrite prim's own golden fixtures the
moment someone staged an edit to one — and git-std's `~` sigil re-stages the
result. `prim fmt --check .` reported those fixtures as clean while
`prim fmt --check <fixture>` reported them as needing a rewrite. Two answers to
the same question, decided by the shape of the invocation.

## Options

**A. Keep the exemption (status quo).** The convention among search tools —
ripgrep, `git grep` — is that an explicitly named target overrides ignore rules,
because there the cost of over-matching is noise. Rejected: for a formatter the
cost is a corrupted fixture or a rewritten generated file, applied silently and
re-staged by a hook.

**B. Keep the exemption, add an opt-in that turns exclusions back on.** This is
ruff's design: `--force-exclude`, documented as "enforce exclusions, even for
paths passed to Ruff directly on the command-line". `ruff-pre-commit` hardcodes
that flag into the entry of all three of its hooks. Rejected: a tool that must
switch its own default off in every hook it ships has the default backwards, and
it leaves anyone invoking prim from a hand-written script holding the unsafe
behaviour. It also adds a flag every consumer must remember, against prim's
near-zero-config goal.

**C. Honour `.primignore` everywhere, with an opt-out for the rare deliberate
case.** This is Prettier's rule: `prettier --write <ignored-file>` leaves the
file untouched. Chosen.

## Decision

1. `.primignore` applies to every path prim is given, walked or named, in `fmt`,
   `lint`, and `fix`.
2. Skipping a path that was **named on the command line** is reported on stderr.
   Skipping during a walk stays silent: filtering is what a walk is for, whereas
   naming a path and getting nothing back is a surprise worth a line. Warnings
   never raise the exit code, so hooks still pass.
3. `--no-primignore` processes ignored paths anyway. It is separate from
   `--no-ignore`, which continues to cover VCS ignore files only.
4. Naming an ignored **directory** skips it too, rather than walking into it.

Point 2 is where prim departs from Prettier, which skips silently and — worse —
reports "All matched files use Prettier code style!" from `--check` on a file it
never looked at. The reporter's complaint in #98 was as much about silence as
about the rewrite, and the fix should not reintroduce it at the other end.

Discovery already tracks provenance (`Discovered.explicit`), so the walked/named
distinction costs nothing. prim matches `.primignore` itself, for named paths
and for walked ones alike, by collecting the `.primignore` files from a path's
own directory upward, nearest first. It does not register `.primignore` with the
`ignore` crate's walker: that walker's ancestor stack has no bound and reads
ignore files from every directory up to the filesystem root. Sharing one matcher
between the two routes is what lets them give the same answer about the bound.
It does not make every answer identical — a `!` rule in a nested `.primignore`
still re-includes a file that walking would have pruned with its parent
directory, which predates this decision and is tracked separately. Matchers are
cached per directory and bound: a hook hands prim its whole staged list at once,
and a walk yields a directory's entries together.

The search is bounded, so a `.primignore` belonging to another repository can
never apply. The bound is the root of the repository containing whatever prim
was pointed at — a named path, or the root of a walk — resolved once and then
applied to every path considered under it. It is inclusive, so a repo-root
`.primignore` still counts, and a repository root is any directory holding a
`.git` entry: a directory in an ordinary clone, a file in a git worktree. Only
where no repository exists at all does the bound become the current working
directory instead; prim must still work outside a repository (FR-4.2).

Resolving the bound from what prim was pointed at, rather than per path, is what
makes both directions correct. Pointed at the enclosing repository, prim keeps
the enclosing rules, so a nested checkout the enclosing `.primignore` names is
still pruned — the behaviour a `.gitignore` entry gives. Pointed at the
checkout, the bound is the checkout, so the enclosing rules no longer reach it.

The ordering of the two bounds decides whether the escape hatch holds. The
working directory is a fallback, never an alternative: were it consulted first,
standing inside a directory that its own repository ignores would stop the
search short of the `.primignore` that names it, and the byte-exact fixtures the
escape hatch exists to protect would be rewritten.

A bound must also be one the search can actually reach. Where prim is pointed
outside the working directory with no repository above it, the working directory
is not an ancestor of anything being considered, so the bound becomes the
pointed-at directory itself; otherwise the search would pass every ancestor
without ever matching one and climb to the filesystem root. Paths are normalized
lexically first, because `..` left in place would make the directory it points
out of an ancestor of the result.

This bound was added with AD-0011, after a stray `.primignore` in a parent
directory was found to silently change how prim treated every repository beneath
it. It was completed later, when it covered only some of the ways prim reaches a
path: the walk was never bounded at all, and the search for a named directory
began one level above it, so an enclosing repository's `.primignore` could skip
a whole nested checkout.

## Consequences

- `prim fmt <ignored-path>` is now a no-op with a warning. Anyone relying on the
  old behaviour adds `--no-primignore`.
- `prim fmt --check <ignored-path>` exits 0 and lists nothing, consistent with
  `prim fmt --check .` over the same file.
- The recipes.md note is inverted, and the recipes it supports now hold: the
  `.primignore` entry for `CHANGELOG.md` protects it from the git-std hook, and
  the fixtures entry protects the correctness harness's golden files.
- prim's `.primignore` promise is now unconditional, which is what its own
  wording already claimed.
- A nested checkout that the enclosing `.primignore` names is processed when
  named directly and pruned when reached by walking the enclosing repository.
  That is a deliberate exception to "one answer per question": the two
  invocations point prim at different repositories, and the answer follows the
  one it was pointed at. Within a single repository the two still agree, which
  is the case #98 was about.

## Alternatives considered

- **Warn but still rewrite.** Keeps the convention and kills the silence, but
  leaves the destructive path open: a warning scrolling past in a pre-commit run
  does not un-rewrite the fixture the hook just re-staged.
- **Reuse `--no-ignore` for both.** Rejected: it is documented as covering VCS
  ignore files only, and overloading it would silently widen what an existing
  flag does in users' scripts.
- **Detect hook invocations and apply the ignore only there.** Rejected: prim
  cannot tell a hook's argument list from a human's, and a rule that depends on
  guessing the caller is worse than either fixed rule.

---

Satisfies: #98 and #110; reshapes FR-4.4 and adds FR-4.4a and FR-4.4b. Related:
AD-0007 (verb surface — the rule is per-verb-uniform),
`crates/prim-cli/src/discover.rs`, `crates/prim-cli/src/cli.rs`,
`docs/recipes.md`.
