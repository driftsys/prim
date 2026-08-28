# AD-0009 — `.primignore` covers explicitly named paths

## Status

Accepted. Supersedes the note in `docs/recipes.md` that said an explicitly named
path is always processed. Breaking: `prim fmt <ignored-path>` no longer rewrites
the file.

Amended for #112: point 5 adds the one case where a skip does raise the exit
code. Breaking: `prim fmt --check <ignored-path>` now exits `2` rather than `0`.

Amended for #113: where the working directory is the bound, the search climbs a
symlinked spelling as far as the directories holding it resolve inside the
working directory. Breaking: a `.primignore` at or below the outermost such
directory now applies to that spelling, where it was silently missed before. A
rule above it is still not reached — see the limit recorded in the Decision
section.

Amended for #114: a named path now obeys gitignore's re-inclusion rule, as a
walked one always did. Breaking: a `!` rule under an excluded directory no
longer re-includes the file it names. The two routes agree on the file, which is
what #114 asked for; their exit codes still differ where the named form is a
gate pointed only at that path, because point 5 then applies.

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

Point 5 applies the same reasoning to the exit code, which is the only signal a
pipeline reads (#112). A `0` from a gate is the claim "I looked, and there is
nothing to do"; where every path was skipped it means "I looked at nothing", and
a stderr warning does not reach a pipeline that only tests the exit code. #110
removed the accidental route into that state — an ancestor's `.primignore`
covering a whole checkout — but not the fail-open default that remained where
the matching is deliberate.

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

Only a skip counts. A named path that prim does not own — a `.rs` file in a
staged list — is reported under FR-4.6 and leaves the exit code alone, and so
does a directory prim walked into and found nothing in. Both look like "the gate
examined nothing", and neither raises the exit code, because the common case of
each is a run that should pass: `prim fmt --check` over a changed-file list from
a Rust-only commit, or over a `src/` holding no file prim owns. The cost of that
boundary is that an unowned path in the list masks the rule —
`prim fmt --check CHANGELOG.md main.rs` exits `0` where
`prim fmt --check CHANGELOG.md` exits `2`. Accepted: the rule fires on the paths
prim declined to look at, not on the ones it was never going to report on.

The same reading scopes `--since` and `--staged` out of the rule. Those flags
narrow what a pointed-at path yields; they are not themselves paths. So
`prim fmt --check --since <ref> .` was pointed at `.`, which is not skipped, and
exits `0` however few files the diff leaves — while
`prim fmt --check --since <ref> <ignored-path>` was pointed at a skipped path
and exits `2`. What #112 closes is a gate handed a path list, the shape its
report was about; the changed-file spelling `docs/recipes.md` recommends, which
points prim at `.`, is unaffected.

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
Matchers are cached per directory and bound: a hook hands prim its whole staged
list at once, and a walk yields a directory's entries together.

Sharing the matcher is not by itself enough to make every answer identical. A
walk applies gitignore's rule that a `!` rule cannot re-include a path under an
excluded directory by pruning that directory and never descending; matching a
named path stopped at the nearest `.primignore`, so a negation written there —
or beside the exclusion itself — re-included a file the walk would never have
offered (#114). prim therefore decides the directories holding a path first,
each against the stack that governs it, and matches the rules naming the path
only where every one of them survived. A negation still re-includes a file whose
parents are not excluded, which is what the documented `!package-lock.json`
recipe relies on (AD-0011).

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
outside the working directory with no repository above it, and no directory
holding the path resolves inside the working directory either, the bound becomes
the pointed-at directory itself; otherwise the search would pass every ancestor
without ever matching one and climb to the filesystem root. Paths are normalized
lexically first, because `..` left in place would make the directory it points
out of an ancestor of the result.

That leaves the working-directory half of the bound, where the two sides were
spelled differently and the prefix test never matched. `std::path::absolute`
leaves a symlink in place while `std::env::current_dir` reports a resolved path,
so a path spelled through a symlink shares no prefix with the working directory
even when it lies beneath it. The bound was then never reached, the fallback
treated it as one the search could not reach, and the search stopped at the
pointed-at directory — short of the `.primignore` protecting the file. That is
the destructive direction: `prim fmt` rewrote a file its own `.primignore`
covers when that file was named through a symlinked path (#113).

So where the plain prefix test fails, the directories holding the path are
compared with the working directory in resolved form, climbing while each one
still resolves inside it. The last that does becomes the bound: the symlink may
point anywhere under the working directory rather than at it, and every
`.primignore` between the path and that point still has to be read. What comes
back is that directory's own spelling, because that spelling is what the search
climbs. The comparison is reached at most once per path prim was pointed at —
never once per file beneath one, and not at all where a repository bounds the
search or the prefix test has already answered.

Only the comparison resolves. Matching stays lexical, which is what gitignore
semantics require: a rule naming a symlinked directory covers the paths written
through it, and resolving the symlink away would silently stop that rule
matching. `git` does not resolve either — it declines to match through such a
path at all.

That leaves a limit worth stating, because it is not a defect to be fixed later.
A rule **above** the directory a symlink points at is not reached through that
spelling: the path as spelled never passes it, so no bound can put it on the
search. A `.primignore` naming `build/` at a tree root covers
`<root>/inner/build/doc.md`, and does not cover the same file named as
`<link>/build/doc.md` where `link` points at `<root>/inner`. Reaching it would
mean matching the resolved path against a rule the given path never passes,
which is the option rejected below. Prim answers with what the spelling
supports: the fix closes the distance up to the symlink's target, not past it.

## Alternatives considered for #113

- **Resolve paths for matching.** Rejected: a `.primignore` naming a symlinked
  directory then matches nothing, because the name the rule uses is resolved
  away before matching — a rule written to protect a tree stops protecting it,
  which is worse than the defect. It also disagrees with `git`, which never
  resolves a path for ignore matching.
- **Refuse a path that goes through a symlinked directory,** as `git` does
  (`fatal: pathspec ... is beyond a symbolic link`). Rejected for now: prim is
  handed paths by hooks and editors that do not choose their spelling, and
  declining to format them is a larger change to the CLI contract than this
  defect warrants. It is the only route that closes the limit above.

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
  rather than passing silently. Such a pipeline runs the gate over the
  repository instead of over the diff. Accepting `2` unconditionally is not the
  answer: `2` is also how prim reports a file it could not parse, a path it
  could not read, and a malformed `--exclude` glob.
- The recipes.md note is inverted, and the recipes it supports now hold: the
  `.primignore` entry for `CHANGELOG.md` protects it from the git-std hook, and
  the fixtures entry protects the correctness harness's golden files.
- prim's `.primignore` promise is now unconditional, which is what its own
  wording already claimed.
- A `!` rule under an excluded directory stops re-including the file it names,
  in the only invocation where it ever did. A repository that has one already
  gets the excluded result from `prim fmt .`; the named form now agrees, rather
  than the other way round. Where the negation is meant to hold, the directory
  exclusion above it has to go.
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

Satisfies: #98, #110, #112, #113, and #114; reshapes FR-4.4 and adds FR-4.4a,
FR-4.4b, and FR-4.4c. Related: AD-0007 (verb surface — the skip is
per-verb-uniform, though point 5's exit code separates the gates from the
writing modes), `crates/prim-cli/src/discover.rs`,
`crates/prim-cli/src/discover/primignore.rs`, `crates/prim-cli/src/cli.rs`,
`crates/prim-cli/src/app/paths.rs`, `docs/recipes.md`.
