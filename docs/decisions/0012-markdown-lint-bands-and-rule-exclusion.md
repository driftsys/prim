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

Amended 2026-08-28 (issue #134): MD057 is dropped from the floor tier as well,
leaving 12 defect rules. AD-0013 records why — a cross-file link's target
depends on the renderer, so a file-existence check is the wrong question at this
layer.

Amended 2026-08-28 (issue #123): a second `.editorconfig` key,
`prim_mdlint_enable`, adds named rules to the set prim runs for a path. The
subtract-only guarantee below is narrowed accordingly — see Decision 6. That key
does not reach MD057: AD-0013 withdrew prim's answer to the cross-file link
question rather than making it answerable on request.

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

### Why an additive key reaches three rules and not twelve

_Added by the 2026-08-28 amendment (issue #123)._

`prim_mdlint_disable` removes rules and nothing adds one. A repository that
wants a rule prim excluded — MD013 line length being the usual request — cannot
have it at any setting, and a repository on the floor tier cannot pick up a
single convention rule without opting into all thirteen. Issue #123 asked for
the additive key Decision 6 below adds.

The original objection to an additive key was that enabling an excluded rule
could produce findings prim's own formatter creates, so `prim fmt` output would
fail `prim lint`. Measured on 0.4.0, prim's 21 tracked Markdown documents pass
markdownlint's entire default rule set, formatter-territory rules included, with
zero findings. That objection did not survive. What replaced it is that most of
the off-list is not worth reaching.

Fifteen rules are off in both tiers. Three of them are withheld by a decision
already taken, so there is nothing to opt into: MD072 (frontmatter key sorting
would break prim's semantics-preserving guarantee), MD082 (dropped by
Decision 2) and MD057 (dropped by AD-0013, because a cross-file link's target
depends on the renderer). That leaves twelve. Reading each rule's config struct
and `check()` in `rumdl 0.2.35`:

| Id    | Under prim                | Evidence                                                                 |
| ----- | ------------------------- | ------------------------------------------------------------------------ |
| MD013 | works, options supplied   | `line_length` defaults to 80 — see the next subsection                   |
| MD014 | works at defaults         | `show_output: true`; no repository input needed                          |
| MD069 | works at defaults         | no config struct at all                                                  |
| MD043 | cannot fire               | `check()` returns empty while `headings` is empty; it defaults empty     |
| MD044 | cannot fire               | early-out on an empty `names` list; it defaults empty                    |
| MD061 | cannot fire               | early-out on an empty `terms` list; it defaults empty                    |
| MD081 | cannot fire               | early-out while `max-per-paragraph` and `max-consecutive` are both unset |
| MD054 | cannot fire               | all six style booleans default `true`, so no style is forbidden          |
| MD074 | cannot fire               | MkDocs flavor only, then needs `source_file` to locate `mkdocs.yml`      |
| MD078 | cannot fire               | Quarto flavor only                                                       |
| MD079 | cannot fire               | Quarto flavor only                                                       |
| MD063 | fires, but unconfigurable | see below                                                                |

prim pins `MarkdownFlavor::Standard` and passes `source_file: None`, which is
what makes MD074, MD078 and MD079 structurally unreachable rather than merely
unlikely.

The five that cannot fire without a list — MD043, MD044, MD054, MD061, MD081 —
each need a repository-supplied value. Supplying one is the per-rule options
surface FR-3.3 forbids and Decision 3 restates, so they cannot be rescued
without reversing a larger decision than this amendment.

MD063 is a separate case, because reading its config alone gives the wrong
answer. `MD063Config` carries `enabled: bool`, documented as "opt-in rule" and
defaulting to `false`, but in `rumdl 0.2.35` that field is read nowhere outside
its own config test: the rule fires whenever it is selected. It is withheld
anyway. Its only meaningful setting is sentence case versus title case, a
house-style choice prim has no surface to let a repository express, so admitting
it would mean prim imposing one house style — chosen by an upstream default
rather than by the repository — on everybody who enabled the rule.

`rumdl_lib::rules::all_rules` returns every rule, including those the registry
marks `opt_in`; that filtering happens in `filter_rules`, which prim never
calls. prim's name filter therefore does reach MD063 — and reaches MD070, MD073
and MD080, which Decision 1 already places in prim's tiers despite their being
opt-in upstream.

That leaves MD013, MD014 and MD069 as the whole of what an additive key adds
beyond prim's own two tiers.

### What MD013 measures, and what a corpus said about it

_Added by the 2026-08-28 amendment (issue #123)._

MD013's `line-length` defaults to 80 whatever the repository asked for.
Measured: with `max_line_length = 120`, prim wraps prose at 119 characters, so a
repository that set 120 and enabled MD013 would see prim's own output fail
prim's own lint at a threshold nobody chose. Decision 6 closes that by feeding
the rule the width the formatter used.

The threshold was the easy half. The design that produced this amendment also
claimed that on a prim-formatted document an enabled MD013 would report nothing
except over-width headings, because rumdl's non-strict mode already forgives
what `dprint-plugin-markdown` cannot break — a trailing token past the limit,
standalone link and image lines, HTML-only lines, link reference definitions and
inline link URLs — and because code blocks and tables are exempted separately. A
corpus was run to refute that claim, and it did.

774 Markdown files from six public documentation sites (FastAPI, React Native,
Redux, Building Secure Contracts, Vite, Vue) were formatted with `prim fmt` and
then linted with MD013 enabled, at three widths. Every file formatted; none
failed:

| Width | MD013 findings | Heading | Table row | Code-block line | Prose |
| ----- | -------------: | ------: | --------: | --------------: | ----: |
| 80    |            238 |     178 |         0 |               0 |    60 |
| 100   |             93 |      59 |         0 |               0 |    34 |
| 120   |             35 |      16 |         0 |               0 |    19 |

Table-row and code-block findings were zero at every width, so the two
protections the design did specify both held. The prose column is the
refutation: 113 findings across the three widths that the design said could not
exist.

Every one of the 113 was inspected in its source context. Each traces to an
atomic run the formatter must not break: an inline code span holding a long
TypeScript type union, an HTML tag's attributes (`<dfn title="...">`,
`<abbr title="...">`, `<ScrimbaLink href="...">`), a `$$...$$` display-math
line, or prose inside a raw HTML block such as Redux's `<DetailedExplanation>`,
whose content CommonMark treats as opaque. Not one was an ordinary sentence with
an available break point the formatter had failed to use. The same document
lines recur at all three widths, which agrees with that cause: the offending
run's length does not shrink when the wrap width changes.

The fix is Decision 6's `paragraphs = false`, which narrows MD013 to headings
only. Re-measured on the same corpus, the findings became 178 at width 80, 59 at
100 and 16 at 120 — all headings, with the prose class at zero for every width.
The reasoning is the placement test that already exempted code blocks: prim
guarantees prose width itself, by formatting, so an over-width paragraph line
found afterwards is by construction content prim must not touch, and reporting
it hands the author a finding with no correct fix. A heading is the opposite
case — prim never wraps headings, and a long heading is rewritable prose — so
headings stay checked.

rumdl's `code-spans = false` was considered as the fix instead, and rejected: it
exempts only the inline-code-span subset of the measured causes. HTML
attributes, display-math lines and raw HTML blocks would keep reporting, so it
would have left most of the refuting findings in place.

Two sampling caveats belong with the numbers. Five of the six projects are API
reference documentation, unusually dense with inline code spans and type
signatures, so the corpus probably over-represents that cause relative to
narrative prose. And 141 of the 178 width-80 heading findings come from one
convention — mkdocs-material's `{ #slug }` permalink anchor appended to every
heading, which roughly doubles the visible heading length — so the heading
totals are not a typical heading over-width rate either.

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

2. **MD082 is dropped from prim's rule table entirely.** It is not in the floor
   tier, not in the strict tier, and neither `prim_mdlint_disable` nor
   `prim_mdlint_enable` (Decision 6) can bring back a rule prim never runs —
   there is nothing to opt into. That table was named `ACTIVE_RULES` when this
   record was written; the 2026-08-28 amendment renamed it `SELECTABLE_RULES`,
   because an entry in it is now a rule prim _can_ run rather than one it always
   _does_ run.

3. **Rule options prim sets for itself.** prim passes rumdl a `Config` with
   `MD025`'s `front-matter-title` emptied, and — since the 2026-08-28 amendment
   — with MD013's three options set (Decision 6). These are prim choosing its
   own canonical defaults for rules it runs, not a configuration surface a
   repository can reach: there is still no way for a repository to configure a
   rule's options.

4. **A new, subtract-only `.editorconfig` key, `prim_mdlint_disable`.** Resolved
   through the same per-glob cascade as `prim_mdlint_strict`: EditorConfig's
   ordinary last-match-wins resolution applies per section, so a narrower
   section's value replaces a wider section's list rather than merging with it.
   Rule ids match case-insensitively. The key can only remove a rule from the
   set prim already selected for that path; it never adds one. When this record
   was written that made prim's curated set an absolute ceiling — a repository
   could not invent a stricter dialect of prim. The 2026-08-28 amendment narrows
   that: `prim_mdlint_enable` (Decision 6) can add a rule, and the ceiling now
   holds for every rule except MD013, MD014 and MD069. An id prim will not act
   on removes nothing; prim reports it on stderr — naming the `.editorconfig`
   file, line and section that set it, once per run for each section that
   carries it — and the exit code is unaffected. Decision 6 splits that report
   in two, because an id prim deliberately withholds is a different mistake from
   an id that names no rumdl rule at all. `prim explain` shows the key with its
   resolved value and its `.editorconfig` file, line, and section, the same
   provenance every other resolved setting gets.

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

6. **A second `.editorconfig` key, `prim_mdlint_enable`, which adds rules**
   (added by the 2026-08-28 amendment, issue #123). Its value is a
   comma-separated list of rule ids, resolved through the same per-glob cascade
   as its two siblings: EditorConfig's ordinary last-match-wins resolution
   applies per section, so a narrower section's value replaces a wider section's
   list rather than merging with it. Rule ids match case-insensitively. `unset`
   (EditorConfig's own reserved word) and `none` (prim's spelling of the same
   intent) clear the list rather than name a rule. The key means _add these
   rules to the set prim runs for this path_, independent of tier.

   **What it reaches.** Every id falls into one of three classes, decided per
   id:

   - **Selectable** — the 25 ids in prim's own two tiers, plus three ids prim
     runs at neither tier: MD013, MD014 and MD069. Enabling a floor-tier rule
     changes nothing, since it already runs. Enabling one convention rule from a
     floor-tier path, without adopting all thirteen, is the case the key exists
     to serve.
   - **Withheld** — any other rule rumdl has. prim knows the rule and will not
     run it: MD072, MD082, MD057, the nine off-list rules the Context above
     accounts for, and every formatter-territory rule. The set is derived from
     rumdl's own registry rather than a hand-maintained list, so it stays
     correct when rumdl adds a rule.
   - **Unknown** — an id naming no rumdl rule at all. A typo.

   **Reporting.** Withheld and Unknown ids are dropped from the list and warned
   about with distinct messages, so an author can tell a deliberate refusal from
   a mistyped id. The contract is the one `prim_mdlint_disable` already had:
   once per run for each `.editorconfig` section that carries the id, attributed
   to the file, line and section that set the key, never raising the exit code.
   `prim_mdlint_disable` gains the same three-way classification, so
   `prim_mdlint_disable = MD013` stops being reported as a typo.

   **Precedence.** `prim_mdlint_enable` adds to the tier's set;
   `prim_mdlint_disable` subtracts from the result. A disable therefore wins a
   conflict and stays a true veto, and because the two keys resolve
   independently through the cascade, a narrower section's disable can cancel a
   wider section's enable.

   **Tier independence.** An enabled rule runs whatever the tier, so a
   file-level `<!-- prim-mdlint-strict: false -->` moves the tier without
   cancelling an enable. There is no inline `prim-mdlint-enable` directive: the
   file-level surface stays the strict boolean plus rumdl's own inline
   directives listed in Decision 5.

   **MD013's options, owned by prim.** An enabled MD013 runs with `line-length`
   set to the width the formatter wrapped to (the resolved `max_line_length`, or
   80 when unset — one function, read by the formatter and the linter alike),
   `code-block-line-length = 0` (rumdl's "no limit"), and `paragraphs = false`.
   The rule therefore checks headings only. Both exemptions follow the same
   test: prim will not reflow a code block, and it already wraps every ordinary
   paragraph itself, so a finding in either place is one the author cannot
   correctly fix. Tables stay off at rumdl's own default, agreeing with prim
   never reflowing a table. The Context above records the corpus measurement
   that forced `paragraphs = false`. prim has no per-rule options surface, so a
   repository cannot widen MD013 back to whole lines; a repository that needs
   whole-line enforcement needs a different tool.

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
- **`prim_fmt::lint_markdown`'s signature then changed a second time**, in the
  2026-08-28 amendment. The three positional parameters are replaced by
  `(source, style, selection)`, where `selection` is a pure
  `prim_fmt::MdLintSelection { strict, enabled, disabled }`. Passing the same
  `Style` the formatter received is the point of the change: it makes MD013's
  threshold agree with the formatter's wrap width by construction rather than by
  convention. `prim_fmt::is_known_rule` is replaced by `prim_fmt::rule_reach`,
  which returns Decision 6's three admission classes. prim-cli is still the only
  consumer.
- **FR-3.3 splits in two.** Its first clause — no `prim.toml`, no per-rule
  flags, and no way for a repository to configure a rule's options — is
  untouched: `prim_mdlint_enable` selects rules, it never sets a rule's options.
  Its second clause, that `prim_mdlint_disable` only ever narrows the rule set
  prim already selected and never widens it, gains a stated exception for the
  new key. A new FR-3.2d specifies the key itself.
- **`prim lint` stops meaning the same thing across repositories** by one more
  degree than `prim_mdlint_disable` already conceded: a repository can now be
  stricter than prim's curated set, not only laxer. The Context above is why the
  concession is bounded to three rules, which is what made it acceptable.
- **An enabled MD013 will surprise somebody.** A repository writing
  `prim_mdlint_enable = MD013` reasonably expects whole-line enforcement and
  gets headings. Documentation is the only available mitigation — prim has no
  per-rule option surface to widen the rule with — so `docs/SPEC.md`,
  `docs/USAGE.md` and `docs/recipes.md` each state the narrowing where the key
  is introduced, not only here.

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
6. **Let `prim_mdlint_disable` also enable rules.** Superseded by Decision 6.
   The original reasoning was that a repository could then run a stricter
   dialect than prim's curated set, fragmenting prim's canonical behavior, and
   that subtract-only keeps the curated set as the ceiling. The objection to
   overloading one key with both meanings still stands, and Decision 6 uses a
   separate key.

   The six alternatives below were considered by the 2026-08-28 amendment (issue
   #123) rather than by the original decision.

7. **Accept the whole off-list except MD072, as issue #123 proposed.** Rejected:
   nine of the twelve ids would change nothing, because the rules cannot fire
   under the flavor and context prim pins. A configuration key that looks
   configurable and mostly is not is worse than a smaller one.
8. **Add a per-rule options surface, so MD043, MD044, MD054, MD061 and MD081
   become useful.** Rejected: that is the `prim.toml`-shaped surface FR-3.3 and
   Decision 3 both forbid, and it is a larger decision than the one this
   amendment makes.
9. **Reach only the three new opt-in rules, leaving the strict tier's rules
   unreachable individually.** Rejected: a floor-tier repository would still
   have to adopt all thirteen convention rules at once. Adopting them one at a
   time stays inside prim's curated set, so it costs the guarantee nothing that
   the three opt-in rules do not already cost.
10. **Let MD013 use rumdl's defaults once `line-length` is supplied.** Rejected:
    `code-blocks: true` reports wide code samples prim will never wrap and a
    repository cannot fix without changing what the code says — the shape
    Decision 1's own placement test rules out.
11. **Pin MD013 to prose only.** Rejected: `prim fmt` already guarantees prose
    width, so the rule would collapse into an unformatted-input detector that
    `prim fmt --check` already covers. The corpus measurement in the Context
    above then showed the stronger form of the same point: the paragraph lines
    MD013 does report after formatting are ones prim must not touch.
12. **Write a new decision record superseding this one.** Rejected: the
    working-memory lifecycle rule says to edit an existing record in place when
    new work changes it, and reserves a new record for a genuinely new topic.
    The subtract-only guarantee is this record's own clause. (AD-0013 is a new
    record for the genuinely new topic that rule reserves one for: where prim's
    link checking stops. It amends this record's floor tier list rather than
    superseding it.)

---

Satisfies: #102, #123; reshapes FR-5.5b and extends FR-3.2a with FR-3.2c and
FR-3.2d. Related: AD-0007 (exit-code contract, §4 warnings-never-gate),
`crates/prim-fmt/src/mdlint.rs`, `crates/prim-cli/src/mdlint_policy.rs`,
`crates/prim-cli/src/init.rs`, and issue #115 (a debug-build panic on one
quarantined documentation-site file).
