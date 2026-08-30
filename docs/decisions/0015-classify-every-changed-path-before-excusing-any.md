# AD-0015 — Classify every changed path before excusing any

## Status

Accepted. New behavior: `--since` and `--staged` ask git for the status of each
changed path (`--name-status --no-renames`) rather than asking it to filter
deletions out. Each reported path is then sorted: on disk it is selected,
whatever its status; absent and reported as a deletion it is passed over; absent
but tracked as one the index will not materialise it is passed over; absent and
tracked as one git will materialise it raises a usage error (exit `2`) within
the paths prim was pointed at; absent and not tracked at all it raises a usage
error wherever prim was pointed, because only prim can have produced such a
name. FR-4.2g records the rule.

## Context

`ChangedFiles::resolve` ended with one line:

```rust
.filter_map(|relative| std::fs::canonicalize(repo_root.join(relative)).ok())
```

Every path git reported that prim could not resolve was discarded — no counter,
no diagnostic, no effect on the exit code. That discard is why three defects
produced no output rather than a wrong result:

- [#164](https://github.com/driftsys/prim/issues/164) — `core.quotePath`
  C-quoted a non-ASCII filename into a literal that never resolved.
- [#165](https://github.com/driftsys/prim/issues/165) — `diff.relative=true`
  made git report paths relative to the process working directory while prim
  joined them onto the repository root.
- [#167](https://github.com/driftsys/prim/issues/167) — a `<REF>` reached git as
  `--output=<file>`, so git wrote the path list into that file and handed prim
  an empty selection.

In each case `prim fmt --check` exited `0` over files it never examined. The
three causes are fixed; this decision is about the discard that hid them.

## Decision

Ask git what happened to each path, and classify before excusing.

The earlier attempt asked git to hide deletions (`--diff-filter=d`) and treated
whatever remained and did not resolve as an error. That was wrong in both
directions, and each direction was reproduced against git 2.49:

- It hid a file prim must format. `git rm --cached u.txt` leaves `u.txt` on
  disk; git calls it a deletion, but the file is there and drifting, and
  `prim fmt --check --staged .` exited `0` over it.
- It could not tell a sparse checkout, a `skip-worktree` entry or a tracked
  dangling symlink from a path prim had mangled, so ordinary repositories
  failed.

Reading `--name-status` instead keeps the information that filter threw away.
The buckets, in order:

1. **Resolves on disk** — selected. A deletion whose file is still present is
   still formatted, which is the `git rm --cached` case.
2. **Absent, status `D`** — the case FR-4.2b has always passed over in silence.
3. **Absent, but something is at the path** (`symlink_metadata` succeeds) — a
   dangling symlink or a directory. Discovery admits only regular files, so it
   has already declined this one.
4. **Absent, and the index will not materialise it** — `git ls-files -v` tags it
   `S` for skip-worktree, which is how sparse checkout is implemented, or a
   lowercase letter for assume-unchanged. Absent by design.
5. **Absent, tracked, and prim owns the file type** — prim was pointed at
   content it cannot read. Exit `2`, within the paths prim was pointed at.
6. **Absent and not tracked at all** — git named something it does not have. No
   repository state produces that, so prim misread git's output. Exit `2`
   wherever prim was pointed, because the fault is prim's rather than the
   caller's.

Bucket 6 is what makes this worth doing. It is a signal no ordinary
configuration can raise, so it can be loud without ever failing a working
repository — which is precisely what the earlier attempts lacked.

The index query is `git ls-files -v --full-name -z -- :/`, run from the
repository root. Without `--full-name` and the pathspec, git lists only the
subtree of the current directory and names paths relative to it, while the diff
paths are repository-root-relative — the two key spaces never meet, and buckets
4 and 6 silently swap. It runs only when something failed to resolve.

## Consequences

A path staged as a **modification** and then removed from the working tree lands
in bucket 5, so it now exits `2` where it was previously skipped in silence.
This is deliberate: prim was pointed at content it cannot read, and the commit
that follows would record bytes prim never examined.

Excluding renames with `--no-renames` decomposes each into a delete plus an add,
so the origin lands in bucket 2 and the destination is gated normally.

Two failures remain, and neither is reachable by any design that inspects only
the failure path: a mangled path that happens to resolve to a _different_
existing file is selected silently, and a report that is well formed but short
has nothing missing to notice, which is the shape issue 167 took.

## Alternatives considered

**Ask git to filter deletions out (`--diff-filter=d`) and refuse the rest.**
Tried, and reverted: it dropped a staged deletion of a file still on disk, and
it failed sparse checkouts, `skip-worktree` entries and dangling symlinks.

**Drop `canonicalize` and match paths lexically.** Canonicalization is the
matching mechanism that makes two spellings of one file compare equal. Removing
it breaks matching under symlinked directories, which collides with the
undecided [#152](https://github.com/driftsys/prim/issues/152) and
[#166](https://github.com/driftsys/prim/issues/166).

**Count the drops and report them on stderr.** A warning never raises the exit
code, so a CI gate would still pass. Every defect in this class was a gate that
reported success to an automated caller; making the drop visible to a human
alone does not correct that.

**Infer deletion from the filesystem** — treat `NotFound` as a deletion and any
other error as a failure. This is the inference that created the conflation, and
it misclassifies the interesting case: a path prim decoded wrongly does not
exist either, so it reads as `NotFound` and stays silent.
