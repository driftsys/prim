# AD-0009 — `.primignore` covers explicitly named paths

## Status

Accepted. Supersedes the note in `docs/recipes.md` that said an explicitly named
path is always processed. Breaking: `prim fmt <ignored-path>` no longer rewrites
the file.

Amended for #112: point 5 adds the one case where a skip does raise the exit
code. Breaking: `prim fmt --check <ignored-path>` now exits `2` rather than `0`.

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
   never raise the exit code, so hooks still pass — except in the one case point
   5 names.
3. `--no-primignore` processes ignored paths anyway. It is separate from
   `--no-ignore`, which continues to cover VCS ignore files only.
4. Naming an ignored **directory** skips it too, rather than walking into it.
5. A **gate** — `fmt --check`, `fix --check`, `fix --diff`,
   `fmt --check-idempotence`, or `lint` — exits `2` when **every** path prim was
   pointed at was skipped: each path named on the command line, or the working
   directory when none was named. The modes that write (`fmt`, `fix`) and the
   preview mode (`fmt --diff`) still exit `0` in that case. Skipping only some
   of the paths given never raises the exit code in any mode.

Point 2 is where prim departs from Prettier, which skips silently and — worse —
reports "All matched files use Prettier code style!" from `--check` on a file it
never looked at. The reporter's complaint in #98 was as much about silence as
about the rewrite, and the fix should not reintroduce it at the other end.

Point 5 finishes that thought at the only place a machine reads (#112). A `0`
from a gate is the claim "I looked, and there is nothing to do"; where every
path was skipped it means "I looked at nothing", and a stderr warning does not
reach a pipeline that only tests the exit code. #110 removed the accidental
route into that state — an ancestor's `.primignore` covering a whole checkout —
but not the fail-open default that remained where the matching is deliberate.

Two boundaries make point 5 safe to adopt. The first is which modes it covers.
Only the gates assert something about the paths they were given; `fmt` and `fix`
assert that they wrote what they could, and doing nothing is the correct outcome
there. That distinction is what keeps the hooks this decision was written to
protect working: prim's own `.githooks/pre-commit.hooks` and
`.pre-commit-hooks.yaml` both run `prim fmt` over a staged-file list, and a
release commit that stages only a `.primignore`d `CHANGELOG.md` must not be
blocked. The second is that the rule fires only when _every_ path was skipped. A
staged list with one ignored path among several is the ordinary case, and it
keeps the exit code the rest of the run earns.

The code is `2`, not `1`. Exit `1` means an actionable finding, and under
`--check` it comes with the list of files that would change on stdout; an exit
`1` with an empty list would contradict that contract. Prim was asked a question
it could not answer, which is what exit `2` already means.

Where no path is named, prim is pointed at the working directory. It is judged
as a named `.` would be — skipped with the same warning, and gated the same way
— so the two spellings of one invocation cannot give different answers. This is
the same "one answer per question" rule the rest of this decision applies to
named and walked paths.

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
- `prim fmt --check <ignored-path>` lists nothing, consistent with
  `prim fmt --check .` over the same file, and exits `2` because that run
  examined nothing (point 5). Where the same command names an unignored path as
  well, the exit code is the ordinary `0` or `1`.
- A CI gate over a changed-file list that can legitimately be all-ignored — a
  release commit touching only a `.primignore`d `CHANGELOG.md` — now fails
  rather than passing silently. Such a pipeline treats `2` as "nothing to
  check", or runs the gate over the repository instead of over the diff.
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

For point 5 (#112):

- **Leave the exit code at `0` and rely on the warning.** Costs nothing and
  keeps every invocation working, but leaves the gate silent in exactly the
  place a gate is read. Rejected once #110 showed how easily a whole checkout
  reaches that state.
- **Exit non-zero in every mode.** One rule instead of two, and rejected for the
  reason AD-0009 exists: it blocks a commit that stages only ignored files,
  which is the hook case this decision was written to protect.
- **An opt-in flag, for example `--error-on-all-ignored`.** Rejected: against
  prim's near-zero-config goal, and a flag every consumer has to remember is a
  default that is backwards, the same argument that rejected option B above.

---

Satisfies: #98, #110, and #112; reshapes FR-4.4 and adds FR-4.4a, FR-4.4b, and
FR-4.4c. Related: AD-0007 (verb surface — the rule is per-verb-uniform),
`crates/prim-cli/src/discover.rs`, `crates/prim-cli/src/cli.rs`,
`crates/prim-cli/src/app/paths.rs`, `docs/recipes.md`.
