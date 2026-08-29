# AD-0012 — Markdown lint bands and rule exclusion

## Status

Accepted. New behavior: `prim lint`'s Markdown rules are placed into two bands —
a floor tier of 13 defect rules, always on, and a strict tier of 13 convention
rules added by `.editorconfig` `prim_mdlint_strict = true`. Every rule prim
runs, at either tier, is an error; there is no warning severity for Markdown any
more. MD082 is dropped from prim's rule set entirely. MD025's
`front-matter-title` option is emptied by default. A new, subtract-only
`.editorconfig` key, `prim_mdlint_disable`, removes named rules from the tier
selected for a path.

Amended for #134: MD057 is dropped from the floor tier as well, leaving 12
defect rules. AD-0013 records why — a cross-file link's target depends on the
renderer, so a file-existence check is the wrong question at this layer.

## Context

Before this change, prim placed each Markdown rule at one of `off`/`warn`/
`error` per tier, and gated `prim lint`'s exit code on `error` only. Twelve
rules sat at `warn` in the strict tier and `off` at floor, so they could never
fail a build at any setting: a CI job running `prim lint <paths>` passed
unconditionally on a document whose only violations were among those twelve.
This surfaced as issue #102, filed as a request for a `--deny-warnings` or
`--max-warnings` flag to make the twelve reachable.

The flag would have addressed the symptom, not the cause. prim's single
`MdDiagnostic::is_error` boolean was doing two independent jobs: deciding
whether a finding fails the build, and deciding how loudly it is drawn (LSP
severity, JSON/SARIF `level`). Both upstream tools keep those jobs apart: rumdl
assigns a per-rule severity that feeds its own display and gates separately
through `--fail-on` (default `any`); markdownlint treats severity as policy
itself and defaults every enabled rule to `error`. Because prim merged the two
axes, a rule that should be visible-but-quiet in an editor was automatically
excluded from the gate — the whole of issue #102.

Untangling display from policy still left the harder question: for gating, which
rules belong at the floor tier, which belong at strict, and which belong in
neither? That question was decided by two tests, both answerable without asking
what a specific author meant:

1. Is the violation decidable without knowing what the author intended?
2. Does it fire on documents that are otherwise fine?

A rule that answers yes to (1) and no to (2) is a **defect**: it reports
something objectively broken — a dead link, a dangling reference, a malformed
table — and essentially never fires on a document that has no such defect.
Because it never legitimately fires, it can gate every repository with no
opt-in, so it belongs at the floor tier.

A rule that is still decidable but fires on documents that are otherwise fine is
a **convention**: inline HTML, a fence without a language tag, a chapter that
intentionally opens at `##` because a book generator renders its `<h1>`
elsewhere. A convention rule cannot gate at the floor tier: gating it there
would fail builds for reasons unrelated to a defect, on every repository, with
no way to opt out short of abandoning lint entirely. It can only gate once a
repository has opted in (`prim_mdlint_strict = true`), because only the
repository knows whether its own conventions collide with the rule.

### Validating the split against documents prim did not grow up on

Two open-source corpora tested the placement against maintained documentation
prim had no hand in shaping.

**Documentation trees — 225 files.** `rust-lang/book` `src/` (112),
`markdownlint` `doc/` (57), `mdBook` `guide/` (35), `cli/cli` `docs/` (21).

**READMEs — 400 files.** One README per crate, sampled across the open-source
Rust ecosystem via the local Cargo registry.

| Population    | Fails floor today | Newly fails under the floor promotions |
| ------------- | ----------------: | -------------------------------------: |
| Documentation |          5 of 225 |                                      0 |
| READMEs       |         89 of 400 |                              13 of 400 |

Promoting MD045, MD051, MD075, MD066, MD068, and MD070 to the floor tier costs
nothing on the documentation corpus, and 13 of 400 READMEs — every one of them
MD045, a status badge written `![](url)` with no alt text. The 89 READMEs
already failing today are dominated by MD034 (bare URLs, 80 files), unchanged by
this decision.

A second, larger corpus tested the same promotions against documentation sites
in production: React Native (Docusaurus, 226 files), FastAPI (MkDocs Material,
155), Building Secure Contracts (GitBook, 136), Vue (VitePress, 119), Redux
(Docusaurus, 80), Vite (VitePress, 57) — 773 files, one quarantined for a
debug-build panic (issue #115). 47 files already failed the floor tier; 15 more
failed under the promotions, 2% of the corpus.

The same corpora also argue for two rules staying at convention rather than
moving to defect. MD033 (inline HTML) fires in every one of the four
documentation-tree projects — 97 of 225 files (368 findings in `book/src`, 63 in
`cli/docs`, 55 in `markdownlint/doc`, 29 in `mdBook/guide`) — on tags that are
legitimate documentation markup: `<a>` 247, `<span>` 95, `<img>` 38, `<kbd>` 19,
`<code>` 15, `<td>` 14. It fires on all six documentation sites too — 276 of 773
files, up to 77% of Vue's pages. A rule every maintained documentation tree
violates cannot gate every repository unconditionally; `prim_mdlint_disable`
(below) is what keeps it usable as a convention rule rather than forcing a
project to abandon the strict tier outright. MD041 (first-line heading), by
contrast, fires almost entirely in one project — 87 of its 89 documentation-tree
hits are `rust-lang/book`, because mdBook renders the chapter title from
`SUMMARY.md` and 85 of its 112 files open at `##`. That is a house-style
collision, not a universal defect, which is exactly the shape a convention rule
is for: scoped away by the one project that disagrees, active everywhere else.

### Why MD082 was dropped rather than placed

MD082 flags a heading with no body text before the next heading. Measured across
the same six public documentation sites, 569 of 573 findings are a parent
heading immediately followed by a deeper one (for example `## Setup` directly
followed by `### Requirements`) — an ordinary outline shape, not an empty
section. Only 4 of 573 are a heading followed by another at the same or
shallower level with nothing between them, which is the case the rule is meant
to catch. The rule's own `level` option filters on the flagged heading's level,
not on its relation to the next heading, so it cannot separate the two cases by
configuration. It is absent from markdownlint entirely and opt-in (not enabled
by default) in rumdl, and it has no fix by design — it flags a structural
relationship between two headings, not a rewritable defect. A rule whose
true-positive rate on real documentation is under 1% and which cannot be tuned
to improve it does not earn a place in either band; it is removed.

### Why MD025's front-matter default was wrong

MD025 (multiple top-level headings) flags a document with more than one heading
at the top level. rumdl's `front-matter-title` option, defaulting to `"title"`,
counts a YAML/TOML front-matter `title:` field as an implicit top-level heading,
so a page with a front-matter title and one body H1 reports two. Measured across
the six documentation sites, 123 of 139 MD025 findings were exactly that shape —
a front-matter `title:` plus at most one body H1 — the format Docusaurus and
VitePress both expect: the front-matter title feeds the sidebar and the HTML
`<title>`, and the body H1 is the rendered heading. Only 16 of 139 were two
genuine top-level headings in the body. Because the false-positive shape so
dominates, prim configures `front-matter-title` to an empty string so MD025
stops counting page metadata as a heading — a page with a front-matter title and
one body H1 is a correct document, not a violation. MD041 (first-line heading)
is deliberately left at rumdl's default: the same `front-matter-title` default
works in prim's favor there, because a page whose front matter carries a title
already satisfies that rule.

### Upstream posture, for corroboration

rumdl enables every markdownlint core rule (MD001-MD059) by default; its own
eight opt-in extensions are MD060, MD063, MD070, MD072, MD073, MD074, MD080, and
MD082. markdownlint enables all rules by default and reports each as `error`
unless a user opts a rule down to `"warning"`. Neither fact drove the placement
above — rumdl's severities drive its own editor display, not gating, and a
corpus measurement is a stronger basis than mirroring an upstream default — but
both corroborate it: MD025 and MD001 are `Error` severity upstream, agreeing
with placing them in a gating band, and rumdl's own choice to hold MD082 back as
opt-in agrees with dropping it here.

## Decision

1. **Two gating bands, no warning band.** The floor tier runs 13 defect rules,
   always on:

   MD011, MD034, MD042, MD045, MD051, MD052, MD056, MD057, MD062, MD066, MD068,
   MD070, MD075.

   MD057 was removed from this list by AD-0013, leaving 12.

   `.editorconfig` `prim_mdlint_strict = true` adds 13 convention rules on top:

   MD001, MD024, MD025, MD026, MD033, MD036, MD040, MD041, MD053, MD059, MD067,
   MD073, MD080.

   Every rule prim runs, at either tier, is an error. A finding's presence is
   its severity; there is nothing further to configure per rule.

2. **MD082 is dropped from `ACTIVE_RULES` entirely.** It is not in the floor
   tier, not in the strict tier, and `prim_mdlint_disable` cannot bring back a
   rule prim never runs — there is nothing to opt into.

3. **One rule option prim sets for itself.** prim passes rumdl a `Config` with
   `MD025`'s `front-matter-title` emptied. This is prim choosing its own
   canonical default for a rule it already runs, not a configuration surface a
   repository can reach: at the time of this decision there was no way at all
   for a repository to configure a rule's options. AD-0014 later added the one
   named path — `max_line_length` supplying MD013's `line-length` — and FR-3.3
   records it; nothing else about this item changed.

4. **A new, subtract-only `.editorconfig` key, `prim_mdlint_disable`.** Resolved
   through the same per-glob cascade as `prim_mdlint_strict`: EditorConfig's
   ordinary last-match-wins resolution applies per section, so a narrower
   section's value replaces a wider section's list rather than merging with it.
   Rule ids match case-insensitively. The key can only remove a rule from the
   tier prim already selected for that path — it can never add a rule prim
   decided not to run, so a repository cannot invent a stricter dialect of prim,
   and prim's curated set stays the ceiling. An id that names no rule prim runs
   in either tier disables nothing; prim reports it on stderr — naming the
   `.editorconfig` file, line and section that set it, once per run for each
   section that carries it — and the exit code is unaffected. `prim explain`
   shows the key with its resolved value and its `.editorconfig` file, line, and
   section, the same provenance every other resolved setting gets. This
   guarantee is unchanged by AD-0014's `prim_mdlint_report_line_length`: that
   key selects MD013 into the tier prim already resolved for a path, using a
   different mechanism from this one, not a second way to widen it.

5. **Two escapes already existed and needed no new code, only documentation.**
   rumdl's own inline directives pass through `rumdl_lib::lint` untouched:

   | Directive                                         | Scope    |
   | ------------------------------------------------- | -------- |
   | `<!-- markdownlint-disable-file MD033 -->`        | file     |
   | `<!-- markdownlint-disable MD033 -->` … `-enable` | block    |
   | `<!-- markdownlint-disable-next-line MD033 -->`   | one line |
   | `<!-- rumdl-disable-file MD033 -->`               | file     |

   `prim_mdlint_disable` covers what those cannot: a whole tree, in one line, in
   the file where the tier is already chosen.

## Consequences

- **Exit codes.** AD-0007 §4 is unchanged in wording — warnings never raise the
  exit code — but no Markdown finding is a warning any more. `prim lint` at the
  floor tier fails only on defects; at the strict tier it fails on everything it
  reports, matching rumdl's own default posture (`--fail-on
  any`).
- **Issue #102** is resolved for Markdown by construction: nothing warn-only
  survives to need a `--deny-warnings` flag. It stays open only if a future
  non-Markdown content rule wants a warning tier.
- **`MdDiagnostic::is_error`** stays in prim's diagnostic type. Every Markdown
  finding sets it to `true` under this placement, so the field is vestigial for
  Markdown today, but it remains specified for a future content rule outside
  Markdown that might legitimately want a warning.
- **The floor tier gains six gating rules** (MD045, MD051, MD066, MD068, MD070,
  MD075) that previously did not gate at the floor tier: MD045, MD051, and MD075
  already ran there and were printed, but only as warnings, so they never failed
  a build; MD066, MD068, and MD070 did not run at the floor tier at all. A
  repository not yet running the strict tier sees new floor-tier failures from
  those six. A repository already running the strict tier sees no change to
  `prim lint`'s exit-code behaviour from this: those six already gated as errors
  under the old strict tier. Every other rule's tier is unchanged or moved down
  (MD024, MD080 leave the floor tier for strict), not up.
- **Eleven convention rules move from warning to error at the strict tier**:
  MD001, MD025, MD026, MD033, MD036, MD040, MD041, MD053, MD059, MD067, MD073.
  Under the old matrix these gated nothing — a finding printed but never failed
  the build. A repository already running `prim_mdlint_strict = true` sees
  `prim lint`'s exit code newly fail on any of these eleven where it did not
  before; this is the migration impact of adopting this decision on an existing
  strict-tier repository.
- **`prim init`'s placement map** gained two working-memory exemptions,
  `[docs/wip/**.md]` and `[docs/archive/**.md]`, both
  `prim_mdlint_strict = false`, so Superpowers specs and plans are never swept
  into the strict tier by a `docs/**` glob (FR-3.5). `docs/wip/` came first;
  `docs/archive/` followed because gardening moves the raw originals there, and
  a move is not an edit. Exempting only the transient half would mean a
  document's tier changed the moment somebody filed it away without touching a
  byte of it, and a repository's CI would start failing on work it had just
  archived.

  This buys a real cost, and it is worth naming. `docs/archive/` is a generic
  directory name: plenty of repositories use it for published-but-superseded
  documentation — retired guides, old release notes — which they do edit and
  would want linted. Every repository that runs `prim init` now gets the strict
  tier switched off there, and prim reports that once, at `prim init` time, in a
  line the reader may not take as a loss. The escape hatch is to write
  `prim_mdlint_strict = true` under the section; `prim init` leaves an explicit
  choice alone on every later run. The failure mode is one-directional —
  relaxing a tier never makes an existing repository's CI newly fail — which is
  why this was judged acceptable rather than gated behind a key.

  This list should not be extended a third time without a new decision record.
- **`prim_fmt::lint_markdown` gained a third parameter**, the exclusion list
  (`disabled: &[String]`), to carry `prim_mdlint_disable`'s resolved ids into
  the engine. Any crate consuming `prim-fmt` directly must update its call site.

## Alternatives considered

1. **Add `--deny-warnings` or `--max-warnings` (issue #102 as filed).** Rejected
   as the primary fix: even made reachable, a convention rule would still gate
   on every repository once "denied," including documents that are otherwise
   fine, so the flag does not decide which axis a rule belongs on. Retained as a
   possible future addition for a later non-Markdown content rule that might
   actually want a warning tier.
2. **Expose rumdl's `--fail-on any|warning|error|never` verbatim.** Rejected for
   now: it inherits the two-axis model without deciding what each rule means, so
   the rules that fire on fine documents would still need re-placing first.
   Cheap to add later on top of this design.
3. **Mirror rumdl's own per-rule severity (upstream parity).** Rejected as the
   placement criterion: rumdl's severities drive its own editor display, not
   gating, so importing them would decide prim's policy by an unrelated measure.
   Used only as corroboration, not evidence.
4. **Place rules by fixability — enforce what `prim fix` can repair.** Rejected
   on inspection of the fixes: MD025 demotes a heading and cascades its whole
   section, MD001 rewrites heading levels, MD026 deletes heading punctuation,
   MD040 writes a literal placeholder language on every unlabelled fence, and
   MD082 has no fix by design despite being the clearest case for removal.
   Fixability measures how easy an edit is, not whether the finding is real, and
   several of these edits conflict with prim's semantics-preserving guardrail.
5. **A `prim.toml` or per-rule CLI flags for the exclusion.** Rejected: prim
   honors `.editorconfig` only, and the exclusion is inherently per-path —
   exactly what `.editorconfig` sections already express. A separate file would
   duplicate the glob map that already chooses the tier.
6. **Let `prim_mdlint_disable` also enable rules.** Rejected: a repository could
   then run a stricter dialect than prim's curated set, fragmenting prim's
   canonical behavior. Subtract-only keeps the curated set as the ceiling.

---

Satisfies: #102; reshapes FR-5.5b and extends FR-3.2a with FR-3.2c. Related:
AD-0007 (exit-code contract, §4 warnings-never-gate), issue #115 (debug-build
panic on one quarantined documentation-site file),
`crates/prim-fmt/src/mdlint.rs`, `crates/prim-cli/src/mdlint_policy.rs`,
`crates/prim-cli/src/init.rs`.
