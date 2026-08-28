# AD-0013 — prim does not check links across a file boundary

## Status

Accepted. New behavior: MD057 (existing relative links) is removed from
`ACTIVE_RULES`, leaving a floor tier of 12 defect rules. Amends AD-0012's floor
tier list.

No repository's `prim lint` result changes. MD057 was listed but could not
report anything, so removing it cannot turn a failing run into a passing one, or
the reverse.

## Context

MD057 checks that a relative link points at a file that exists. prim listed it
as a defect rule, which is prim's strongest promise about a rule: always on, no
opt-in, an error whenever it fires. Two separate findings say that promise was
wrong.

### It could not fire

rumdl resolves a relative link against the directory of the file being linted,
and derives that directory from `LintContext::source_file`. prim calls
`rumdl_lib::lint` with `source_file: None`, because `prim-fmt` is the pure
engine (AD-0001): it takes source text, returns source text, and performs no
path resolution and no I/O. Without a base directory, `rumdl 0.2.35`'s
`src/rules/md057_existing_relative_links.rs` returns its (empty) warning list
before it inspects a single link:

```rust
// If we still don't have a base path, we can't validate relative links
let Some(base_path) = base_path else {
    return Ok(warnings);
};
```

So for as long as MD057 has been listed, `prim lint` has reported nothing for
it, on any input. A test in `crates/prim-fmt/src/mdlint/tests/rule_fixtures.rs`
recorded that as a known gap, and the fixture-coverage test carried an explicit
exception for the one rule that had no fixture because no fixture could be
written.

### A file-existence check is the wrong question

The gap could have been closed by passing the path through. It was not, because
answering the question MD057 asks would not tell a reader whether the link
works. Whether a relative link resolves depends on the renderer, and prim's own
repository has both directions of the failure.

**A link that exists on disk and is still broken for the reader.**
`docs/archive/specs/2026-08-12-generated-file-protection-design.md` links to
`../../../crates/prim-cli/src/editorconfig.rs`. Resolved against the containing
directory, that is `crates/prim-cli/src/editorconfig.rs` in the repository root,
which exists — so a file-existence check passes it, and on GitHub the link
works. prim's own documentation site is built by mdBook with `src = "docs"`, and
the link escapes that root: `crates/` is never copied into the book output, so
the published page serves a link the reader cannot follow. The check says
"fine"; the reader gets a 404.

**A link written as if the repository root were the base.** A file nested under
`docs/archive/plans/` that writes `[docs/SPEC.md](docs/SPEC.md)` is asking for
`docs/archive/plans/docs/SPEC.md`, which does not exist, while the target the
author meant — `docs/SPEC.md` — does. GitHub and GitLab resolve relative to the
containing file and show the link as broken; a static site with a configured
document root can resolve the same text correctly. This shape appears in
`docs/archive/plans/2026-07-04-debt-remediation.md` at lines 60 and 70, inside
fenced sample blocks rather than as live links, so no linter would inspect those
two occurrences — but the shape is a common habit, and which renderer the file
is destined for decides whether it is a defect.

Neither case is decided by "does this path exist". They are decided by which
renderer the file is published through, and prim's engine does not know that and
should not have to.

### Where the line already sits

prim runs two link rules that are well defined at this layer. MD051 reports a
link to a heading anchor that the same file does not define, and MD052 reports a
reference-style link whose definition the same file does not carry. Both resolve
entirely inside one buffer, need no path and no I/O, and give the same answer in
every renderer. They stay.

## Decision

1. **MD057 is removed from `ACTIVE_RULES`.** It is in neither tier, and
   `prim_mdlint_disable` cannot bring back a rule prim never runs.

2. **The boundary, stated once:** prim checks links that resolve inside the
   file; a link that crosses a file boundary belongs to a link checker. MD051
   and MD052 are on the prim side of that line. Cross-file link checking is a
   documented non-goal, not a gap prim intends to close by widening the engine.

3. **The fixture-coverage test becomes exact.** Every `ACTIVE_RULES` entry has
   exactly one fixture in `rule_fixtures.rs`, with no exception. MD057 joins
   MD082 in `dropped_and_formatter_territory_rules_never_run`, which asserts a
   rule runs in neither tier — the same shape already used for a rule prim
   decided not to run.

4. **`docs/recipes.md` names the tools that do this job**, so a reader who wants
   cross-file link checking is pointed at one rather than left assuming prim
   covers it.

## Consequences

- **No observable behaviour changes for any input.** A rule that never reported
  anything is removed, so no `prim lint` exit code and no printed finding moves.
  This is why the change is a `fix` and not a breaking change to the rule set:
  the promise being withdrawn was never kept.
- **The floor tier is 12 defect rules, not 13.** `docs/SPEC.md`, `docs/USAGE.md`
  and AD-0012 are updated, and MD057 is listed among the rules that are off in
  both tiers.
- **prim's Markdown lint no longer claims any cross-file guarantee.** A reader
  who needs one adds a link checker beside prim; nothing in prim's output
  suggests the check has already been done.
- **prim may later grow renderer-aware link validation** — resolving a link the
  way the target renderer would, rather than the way the filesystem does. That
  is a different and larger feature, with a configuration surface prim does not
  have today, and it is tracked separately.

## Alternatives considered

1. **Pass `source_file` through and make MD057 work.** Rejected on two counts.
   It puts path resolution and filesystem access into `prim-fmt`'s call graph
   through the rumdl dependency, which is exactly the boundary AD-0001 draws and
   `AGENTS.md` requires — the engine takes text and returns text. And it would
   still answer the wrong question: both worked examples above are decided by
   the renderer, not by whether a path exists, so the working rule would pass
   the broken link and could flag a working one.
2. **Keep MD057 listed and inert.** Rejected: prim's rule list is a promise
   about what `prim lint` checks. A rule that is advertised as always on and
   never fires makes the list untrustworthy, and it is worse than a documented
   absence, because a reader who sees MD057 in the floor tier reasonably stops
   looking for a link checker.
3. **Defer to a general link checker.** Accepted for file existence, and
   recommended in `docs/recipes.md` — this is what `lychee` and
   `mdbook-linkcheck` are for, and they do the job better than prim could,
   including the network side prim will never touch. It is not a complete
   answer, because no general checker knows which renderer a given file is
   published through: `lychee` resolves against the filesystem, and
   `mdbook-linkcheck` resolves the way mdBook does. A repository whose files
   reach readers through more than one renderer still has to choose. That
   remaining problem is the renderer-aware validation named in Consequences, not
   something MD057 was ever going to solve.

---

Satisfies: #134; amends AD-0012 (floor tier list) and narrows FR-5.5b's floor
tier to 12 defect rules. Related: AD-0001 (pure engine crate boundary),
`crates/prim-fmt/src/mdlint.rs`,
`crates/prim-fmt/src/mdlint/tests/rule_fixtures.rs`, `docs/recipes.md`.
