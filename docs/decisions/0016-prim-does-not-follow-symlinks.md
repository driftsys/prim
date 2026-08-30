# AD-0016 — prim does not follow symlinks

## Status

Accepted. Answers the question AD-0009 deferred (#152) and fixes the data loss
it left open (#166). Breaking: `prim fmt <symlink>` no longer replaces the link
with a regular file, `prim fmt --check <symlink>` no longer reports it, and
`prim init` refuses a symlinked `.editorconfig` rather than replacing it.

## Context

Two open questions turned on the same word, and they had opposite answers.

The first is #152, split out of #113. A `.primignore` rule **above** the
directory a symlink points at is not reached through that spelling. With a rule
`build/` at a tree root, and `link` pointing at `<root>/inner`:

```text
<root>/inner/build/doc.md      covered
<link>/build/doc.md            not covered
```

Both name the same file. AD-0009 recorded the limit and rejected — for now, not
on the merits — the only route that closes it: refusing such a path, as `git`
does (`fatal: pathspec ... is beyond a symbolic link`).

The second is #166. A symlink named as the target itself was followed. prim
resolved it, formatted the file at the other end, and wrote the result back to
the **link's** path. Because the write is a temporary file plus a rename
(FR-6.4), the rename replaced the symlink with a regular file:

```console
$ printf 'x  \n' > target.md && ln -s target.md link.md
$ prim fmt link.md
$ ls -l
-rw-r--r--  link.md      # the symlink is gone
-rw-r--r--  target.md    # still 'x  \n', still drifting
```

prim destroyed a file it was not asked to touch and failed to format the one it
was. The symlink cannot be recovered from prim's output.

The defect was not confined to `--staged`, where it was reported.
`ChangedFiles::contains` canonicalizing before testing membership is what let a
named symlink match its target's identity under that flag, but the write itself
was unconditional: plain `prim fmt <symlink>` reproduced it with no flag at all.

It was also already a contradiction of AD-0009's central rule, that one question
gets one answer. `walk_into` admits an entry only where `ft.is_file()`, and the
`ignore` walker does not follow links, so a walk never offered a symlink at all:

```console
prim fmt --check .          # lists target.md, never link.md   -> exit 1
prim fmt --check link.md    # lists link.md                    -> exit 1
```

Two answers about the same file, decided by the shape of the invocation — the
state AD-0009 exists to prevent.

## Options

**A. Refuse any path that goes through a symlinked directory, as `git` does.**
Closes #152's limit completely and is the only route that does. Rejected, on
cost: on macOS `/tmp` is a symlink to `private/tmp` and `/var` to `private/var`,
and `$TMPDIR` resolves under `/var/folders/...`. Refusing traversal would refuse
every `/tmp/...` and every `$TMPDIR/...` path on that platform. prim is also
handed paths by hooks and editors that do not choose their spelling —
`.githooks/pre-commit.hooks` passes git's staged-file list, and format-on-save
passes whatever the buffer's path happens to be. It would additionally need the
four sub-decisions #152 listed: which verbs refuse, what exit code, whether a
flag opts back in, and whether the LSP and `--stdin-filepath` are in scope.

**B. Refuse traversal only in the verbs that write.** Narrows the blast radius
of option A. Rejected: `fmt --check` would then report drift that `fmt` refuses
to fix, which is the split answer this decision is closing, not another instance
of it.

**C. Resolve the path and write to the resolved target.** Fixes the data loss —
the link survives, because prim writes to the file at the other end. Rejected:
prim would then rewrite a file the caller did not name, which is the same
surprise in the other direction, and it disagrees with the walk, which formats
that file only when the walk reaches it under its own name. It also cannot be
squared with `.primignore` matching, which AD-0009 keeps lexical precisely so a
rule naming a symlinked directory keeps protecting it.

**D. Do not follow a symlink named as the target; leave traversal alone.**
Chosen.

## Decision

1. prim never writes **to** a symlink. Where a link is itself the file prim
   would read or write, it is a path type prim does not own: the link is left
   byte-for-byte unchanged, and the file it points at is neither read nor
   written. "The file prim would write" is the whole of the rule — a link prim
   only passes through or walks into is a different shape, and points 5 and 6
   say why it is left alone.
2. Naming one is reported on stderr and leaves the exit code alone. This is
   FR-4.6's existing rule for an existing path whose type prim does not own, not
   a new one — prim was simply not applying it to symlinks.
3. The skip does **not** count toward FR-4.4c's "every pointed-at path was
   skipped" gate. AD-0009 already draws that line: the skips that count are the
   ones `.primignore` and the built-in generated-file list cause, while a path
   prim does not own is reported under FR-4.6 and leaves the exit code alone. A
   pre-commit hook whose staged list happens to contain a symlink must not fail
   the commit. `prim init` is the exception, and exits `2`: the file it was
   asked to write is the whole of what it was asked to do, not one path in a
   list.
4. This holds wherever a link can be the file in question: every verb — `fmt`,
   `lint`, `fix`, and the `--check` and `--diff` gates — plus the paths a
   changed-file scope reports, the `.editorconfig` `prim init` writes, and the
   path `prim explain` answers about. `--stdin-filepath` and the LSP are outside
   it by construction: both work on a buffer and write no file. One module
   carries the predicate and the wording so those routes cannot drift apart.
5. A path that merely **goes through** a symlinked directory is processed
   normally. #152 is decided in the negative: prim does not refuse it, and the
   `.primignore` reach limit AD-0009 records stands as decided rather than as a
   defect awaiting a fix.
6. A **symlinked directory named directly** is walked, not declined. It is point
   5 seen one component earlier: prim writes to the regular files inside it and
   destroys no link. Declining it would refuse `prim fmt /tmp/...` on any
   platform where `/tmp` is itself a link, which is the cost that decided point
   5. A walk still never descends into a symlinked directory it merely passes,
   so naming one and walking past it answer about different trees. That is the
   exception AD-0009 already makes for a nested checkout: the answer follows
   what prim was pointed at.

Points 1 and 5 look inconsistent until the failure modes are compared, and the
difference is what makes them one decision rather than two.

A path through a symlinked directory ends at a regular file. The rename replaces
that regular file, which is exactly what prim was asked to do; the symlinked
directory is untouched, and nothing is destroyed. The only consequence is the
`.primignore` reach limit — a rule that does not apply where a user might expect
it to, which is a matching surprise, not data loss. Set against the cost of
refusing, recorded under option A, the limit is the cheaper of the two.

A path whose final component is a symlink ends at the link itself. There the
rename does not replace what prim was asked to format; it replaces the link, and
the bytes prim was asked to format stay as they were. That is unrecoverable, and
no cost on the other side comes close. Naming a symlink to a Markdown file is
rare, the walk has always declined to offer one, and FR-4.6 already said what
should happen to it.

So the rule is not "prim distrusts symlinks" but the narrower one the failure
modes support: prim never writes **to** a symlink, and never needs to care about
one it merely passes through.

Only a named path is tested for this, and the extra `lstat` therefore stays off
the per-file walk path. The walk's own filter already guarantees no symlink
reaches it, which is why the named route had to be the one brought into line.

## Consequences

- `prim fmt <symlink>` is a no-op with a warning, where it previously destroyed
  the symlink. Anyone who wants the target formatted names the target.
- `prim fmt --check <symlink>` exits `0` and lists nothing, agreeing with
  `prim fmt --check .` over the same tree. A gate pointed only at a symlink
  exits `0`, not `2`, because point 3 keeps it outside FR-4.4c.
- `prim fmt --staged <symlink>` no longer replaces the link, and the staged file
  it pointed at is left for the invocation that actually names it. #166's second
  half — the staged file still drifting — is not a defect once the first half is
  fixed: prim was pointed at the link, and only at the link.
- A **dangling** symlink is now reported as an unowned path (exit `0`) rather
  than as a missing file (exit `2`). Its type is what prim declines, and whether
  the far end exists does not change that.
- `ChangedFiles::contains` still canonicalizes before testing membership, and
  still admits a named symlink: what declines that path is `load_one`, further
  down. The resolution has to stay, because it is what lets a path spelled
  through `/tmp` match one git reported under `/private/tmp`.
- The same resolution ran the other way in `ChangedFiles::resolve`, where a
  symlink git reported was canonicalized into the changed set under its
  **target's** identity. Staging only a link therefore pulled its target into
  scope: `prim fmt --check --staged .` reported a file git never staged, while
  `prim fmt --check --staged <link>` reported nothing. A symlink git reports is
  now passed over there, for the same reason a named one is.
- `prim init` never reaches the formatting path, so the same rename destroyed a
  symlinked `.editorconfig`: the link became a regular file holding prim's map,
  and the shared config it pointed at never received the merge. It now refuses.
- `prim explain` answered for a symlink with a full settings table while already
  declining a type prim does not format. It now declines both alike; describing
  settings prim will never apply to that path was the less useful answer.
- #152 closes. AD-0009's "Alternatives considered for #113" is amended to record
  the rejection as final, with the platform cost that decided it.

## A limit this record does not close

Points 5 and 6 rest on "the rename replaces a regular file, so nothing is
destroyed". That premise holds for a file with one name. It does not hold for a
**hard link**: the rename gives the formatted content a new inode, so a second
name for the same file silently keeps the pre-format bytes. That is the same
family as #166 and is not addressed here, because it is a property of every
write prim makes rather than of the paths this decision is about. It is recorded
so the premise above is not read as wider than it is.

---

Satisfies: #152 and #166; scopes FR-4.6 to name symlinks explicitly and adds
FR-4.6b. Related: AD-0009 (`.primignore` covers explicit paths — the limit this
decision declines to close, and the one-answer-per-question rule it applies),
FR-4.4c, FR-6.4, `crates/prim-cli/src/symlink.rs`,
`crates/prim-cli/src/app/load.rs`, `crates/prim-cli/src/changed_files.rs`,
`crates/prim-cli/src/init.rs`, `crates/prim-cli/src/app.rs`.
