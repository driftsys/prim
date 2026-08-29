# Markdown lint tier model — design

- Date: 2026-08-23
- Status: draft, awaiting review
- Issue: #102 (`prim lint` exits 0 on warn-only findings)
- Disposition: local working memory. Its "Corpus" section measures private
  workspaces; scrub that section before this note is archived into the
  repository, and never carry its figures into a committed file. The open-source
  sections are public and may be cited freely.

## Problem

`prim lint` reports Markdown findings at two severities and gates only on
`Error`. Twelve rules are `Warn` in the strict tier and off in the floor tier,
so they can never fail a gate at any setting. A CI job written as
`prim lint <paths>` passes unconditionally on a document whose only violations
are those twelve. Issue #102 proposes a `--deny-warnings` or `--max-warnings`
flag to make them reachable.

The flag addresses the symptom. The cause is that prim uses one boolean,
`MdDiagnostic::is_error`, for two independent jobs:

- policy — does this finding fail the build (`app.rs:262`, `app.rs:469`)
- display — how loud is the finding drawn (`report.rs:129` for the JSON and
  SARIF `level`, `lsp/diagnostics.rs:44` for the LSP severity)

Both upstream tools keep those jobs apart. rumdl assigns a per-rule `Severity`
that feeds the LSP and the machine-readable formatters, and gates separately
through `--fail-on`, which defaults to `any`. markdownlint treats severity as
policy (`"warning"` opts a rule out of the build failure) and defaults every
enabled rule to `error`. markdownlint-cli2 exits `0` when only warnings exist.

Because prim merged the axes, a rule that should be visible-but-quiet in an
editor is automatically excluded from the gate. That is the whole of issue #102.

## Evidence

### Corpus

800 Markdown files from two internal workspaces, human and AI authored, copied
into a flat directory with `prim_mdlint_strict = true` and linted with
`prim 0.3.1` (debug build). Counts are findings and distinct files affected.

Sources: two private repository roots, 400 files each, excluding `node_modules`,
`target` and `.git`. The sample is the first 400 files `find` returned per root,
not a random draw, and both roots reached that cap. The workspaces are not named
here because this record is archived into a public repository.

Composition: 327 under a `docs/` directory, 217 other authored documents, 191
READMEs, 29 changelogs, 29 `.superpowers` AI working files, 4 `.github`
templates, 3 `.claude` agent definitions.

| Rule  | What it catches                  | Findings | Files |
| ----- | -------------------------------- | -------: | ----: |
| MD082 | heading with no body before next |     1880 |   451 |
| MD080 | heading anchor collision         |      979 |    74 |
| MD040 | fence without a language         |      404 |   126 |
| MD024 | duplicate heading                |      212 |    29 |
| MD034 | bare URL                         |      134 |    13 |
| MD041 | first line is not a top heading  |       81 |    81 |
| MD025 | multiple top-level headings      |       71 |    17 |
| MD036 | emphasis used as a heading       |       58 |    18 |
| MD033 | inline HTML                      |       18 |     1 |
| MD026 | trailing punctuation in heading  |       18 |     8 |
| MD001 | heading level skipped            |       15 |    15 |
| MD045 | image without alt text           |        8 |     3 |
| MD053 | unused reference definition      |        4 |     4 |
| MD051 | link fragment does not resolve   |        2 |     2 |
| MD059 | non-descriptive link text        |        1 |     1 |

Twelve active rules produced zero findings: MD042, MD011, MD052, MD056, MD062,
MD057, MD066, MD068, MD070, MD073, MD067, MD075.

Caveat: prim calls `rumdl_lib::lint` with `source_file = None`
(`mdlint.rs:161`), so MD057 cannot resolve relative link targets. Its true rate
is unmeasured, not zero. Tracked as an open question below.

### MD082 is wrong by construction

Each MD082 finding was classified by the level of the next heading:

| Case                                               | Findings | Files |
| -------------------------------------------------- | -------: | ----: |
| Parent heading, next heading deeper (`##` → `###`) |     1456 |   426 |
| Next heading same level or shallower               |      412 |    66 |

78 % of findings flag the ordinary outline shape rather than an empty section.
The rule's `level` knob filters on the flagged heading's own level, not on its
relation to the next heading, so it cannot separate the cases: `level = 2`
removes 200 of 1880 findings, `level = 3` leaves 553.

### Open-source validation

Two further corpora, to test the placement against documents prim did not grow
up on.

**Documentation trees — 225 files.** `rust-lang/book` `src/` (112),
`markdownlint` `doc/` (57), `mdBook` `guide/` (35), `cli/cli` `docs/` (21).
Test-fixture directories were excluded; these are maintained, edited
documentation, and `markdownlint`'s own docs are linted by markdownlint with
every rule enabled.

**READMEs — 400 files.** One README per crate from the local Cargo registry,
sampled across the open-source Rust ecosystem.

| Population    | Fails floor today | Newly fails under Band A |
| ------------- | ----------------: | -----------------------: |
| Documentation |          5 of 225 |                        0 |
| READMEs       |         89 of 400 |                13 of 400 |

Band A holds. Promoting MD045, MD051, MD075, MD066, MD068 and MD070 to error at
the floor tier costs nothing at all on documentation, and 13 of 400 READMEs —
every one of them MD045, a status badge written as `![](url)` with no alt text.
The 89 READMEs already failing today are dominated by MD034 (bare URLs, 80
files), which is unchanged by this design.

Band B is refuted for one rule and questioned for a second:

| Band B rule              | Doc files hit (of 225) | Projects affected |
| ------------------------ | ---------------------: | ----------------: |
| MD033 inline HTML        |                     97 |            4 of 4 |
| MD041 first-line heading |                     89 |            1 of 4 |
| all other Band B rules   |                     25 |             mixed |

- **MD033 fires in every project**: 368 findings in `book/src`, 63 in
  `cli/docs`, 55 in `markdownlint/doc`, 29 in `mdBook/guide`. The tags are
  legitimate documentation markup — `<a>` 247, `<span>` 95, `<img>` 38, `<kbd>`
  19, `<code>` 15, `<td>` 14. A rule that every maintained documentation tree
  violates cannot be carried by the tier alone. The decision below keeps it in
  Band B only because `prim_mdlint_disable` gives a project one line to switch
  it off for a whole tree.
- **MD041 fires in one project**: 87 of its 89 hits are `rust-lang/book`, whose
  chapter files open at `##` because mdBook renders the chapter title from
  `SUMMARY.md`. 85 of its 112 files start at H2. The other three projects start
  every file at H1 and hit the rule once each, incidentally.

The distinction matters, and it adds a third question to the placement test:
does the rule fire across projects, or does one project's house style collide
with it. A rule of the first kind is wrong; a rule of the second kind can be
scoped away by the project that disagrees.

Excluding MD033 and MD041, the whole of Band B costs 25 of 225 documentation
files — roughly one file in nine — spread over MD001 (10), MD040 (8), MD053 (5),
MD025 (2), MD036 (2), MD080 (2), MD026 (2) and MD024 (1).

One honest counter-note on MD082: on these open-source documentation trees it
fires on only 7 of 225 files, far below the 451 of 800 measured on the internal
corpus. Its structural defect is unchanged — 78 % of its findings flag a parent
heading followed by a deeper one — but its noise level is a property of writing
style, not a universal.

### Documentation-site validation

Six well-known sites that _use_ each generator, rather than the generators' own
repositories, whose authors write to their own linter's taste: React Native
(Docusaurus, 226 files), FastAPI (MkDocs Material, 155), Building Secure
Contracts (GitBook, 136), Vue (VitePress, 119), Redux (Docusaurus, 80), Vite
(VitePress, 57). 773 files, after quarantining one that panics prim's debug
build (issue #115).

Band A holds again: 47 files already fail the floor tier today, and 15 more fail
under the promotions — 2 % of the corpus.

Convention rules hitting at least 10 % of a site's files, which is what that
site would have to exclude:

| Site                      | Rules over 10 %                                                        |
| ------------------------- | ---------------------------------------------------------------------- |
| Vue (VitePress)           | MD033 77 %, MD041 14 %                                                 |
| FastAPI (MkDocs)          | MD033 48 %, MD040 12 %                                                 |
| React Native (Docusaurus) | MD033 33 %, MD025 28 %, MD001 14 %                                     |
| Redux (Docusaurus)        | MD025 61 %, MD080 20 %, MD036 16 %, MD033 12 %, MD001 23 %             |
| Vite (VitePress)          | MD025 24 %, MD033 24 %, MD036 19 %, MD026 17 %, MD059 12 %, MD040 12 % |
| Building Secure Contracts | MD040 13 %                                                             |

MD033 fires on 276 of 773 files across all six sites, which confirms the earlier
finding rather than softening it.

MD082's structure was re-derived on this public corpus so the claim never rests
on private measurement: of its 573 findings here, 569 flag a parent heading
followed by a deeper one and 4 flag a genuinely empty section. That is the
figure the shipped code comment and AD-0012 cite.

### MD025 is measuring the wrong thing

MD025 flags 139 files here, but the shape of those findings matters:

| Case                                            | Files |
| ----------------------------------------------- | ----: |
| Front-matter `title:` plus at most one body H1  |   123 |
| Two or more real top-level headings in the body |    16 |

88 % are pages written the way Docusaurus and VitePress expect: the front-matter
title feeds the sidebar and the HTML `<title>`, and the body H1 is the rendered
heading. Those are page metadata and one heading, not two headings.

rumdl's MD025 exposes `front-matter-title`, defaulting to `"title"`. Set to an
empty string it stops treating front matter as a heading, which removes all 123
and keeps all 16.

### Upstream posture

- rumdl enables every markdownlint core rule (MD001-MD059) by default. Its eight
  opt-in rules are all its own extensions: MD060, MD063, MD070, MD072, MD073,
  MD074, MD080, MD082.
- rumdl's `--fail-on` defaults to `any`, so any violation exits `1` regardless
  of severity.
- markdownlint enables all rules by default and reports each as `error` unless
  the user opts it down to `"warning"`.
- prim enables MD073 and MD082 although rumdl holds both back, and promotes
  MD080 to `error` under strict although rumdl does not enable it at all. MD080
  and MD082 together produce 74 % of prim's strict findings on the corpus.

## Decision

Place each rule by two questions, both answerable from the corpus:

1. Is the violation decidable without knowing what the author intended?
2. Does it fire on documents that are otherwise fine?

That yields two gating bands and no warn-only band.

### Band A — defect: error at floor and strict

| Rule  | Files | Today         | Change         |
| ----- | ----: | ------------- | -------------- |
| MD042 |     0 | error / error | none           |
| MD011 |     0 | error / error | none           |
| MD052 |     0 | error / error | none           |
| MD056 |     0 | error / error | none           |
| MD062 |     0 | error / error | none           |
| MD057 |    0* | error / error | none           |
| MD034 |    13 | error / error | none           |
| MD051 |     2 | warn / error  | error at floor |
| MD045 |     3 | warn / error  | error at floor |
| MD075 |     0 | warn / error  | error at floor |
| MD066 |     0 | off / error   | error at floor |
| MD068 |     0 | off / error   | error at floor |
| MD070 |     0 | off / error   | error at floor |

Six rules get stricter at the floor tier, together firing on 5 of 800 files.

### Band B — convention: off at floor, error at strict

"Files" counts the whole corpus; "docs" counts only the 327 files under a
`docs/` directory, which is the population the strict tier actually covers under
the placement map.

| Rule  | Files | docs | Today        | Change          |
| ----- | ----: | ---: | ------------ | --------------- |
| MD040 |   126 |   40 | off / warn   | error at strict |
| MD041 |    81 |    1 | off / warn   | error at strict |
| MD080 |    74 |   14 | warn / error | off at floor    |
| MD024 |    29 |    1 | warn / error | off at floor    |
| MD036 |    18 |    6 | off / warn   | error at strict |
| MD025 |    17 |    4 | off / warn   | error at strict |
| MD001 |    15 |    0 | off / warn   | error at strict |
| MD026 |     8 |    0 | off / warn   | error at strict |
| MD053 |     4 |    0 | off / warn   | error at strict |
| MD033 |     1 |    1 | off / warn   | error at strict |
| MD059 |     1 |    0 | off / warn   | error at strict |
| MD073 |     0 |    0 | off / warn   | error at strict |
| MD067 |     0 |    0 | off / warn   | error at strict |

MD025 is listed here as an error at strict on the understanding that prim
configures `front-matter-title` away (see "Rule configuration prim owns" below);
without that it fires on 61 % of Redux and 28 % of React Native pages.

MD024 and MD080 leave the floor tier. 24 of MD024's 29 affected files are
changelogs, which sit at the floor tier under the placement map.

Turning strict on for a `docs/` tree costs about 60 of 327 files, and MD040 is
40 of those. Every other convention rule is close to free there, because their
findings concentrate in files the floor tier already covers: tooling files,
READMEs, changelogs and AI working notes.

MD041 is the clearest case. Of its 82 affected files, 47 open with YAML front
matter carrying `name:` and `description:` rather than `title:` — agent, skill
and rule definitions, which are configuration rather than documents — and three
more are READMEs whose first line is an HTML license header. A floor-tier
warning would therefore be wrong more often than right, which is why the rule
stays off there rather than warning.

MD033 stays in Band B despite the open-source validation, which found it firing
in 4 of 4 projects on 97 of 225 documentation files. That retention is
conditional: it is only workable alongside the per-glob exclusion key below,
because without one a project like `rust-lang/book` would need a
`<!-- markdownlint-disable-file MD033 -->` line in 97 files, and would abandon
strict altogether instead — losing MD040, MD041 and MD025 enforcement with it.

### Band C — removed

MD082 is dropped from `ACTIVE_RULES`. It is absent from markdownlint, opt-in in
rumdl, has no fix by design, and 78 % of its findings describe a normal document
outline.

### Rule configuration prim owns

prim passes rumdl a `Config` rather than `Config::default()`, with one override:
MD025's `front-matter-title` is set to an empty string. This is prim choosing
its canonical defaults, not a user-facing surface — there is still no way for a
repository to configure a rule's options.

It is a deliberate deviation from markdownlint's default for MD025, recorded
here because the corpus says the default measures the wrong thing: it counts
page metadata as a heading, and 123 of 139 findings on real documentation sites
were that false positive.

The same reasoning does not extend to MD041, whose `front-matter-title` default
works in prim's favour — a page whose front matter carries a title already
satisfies the rule.

### Per-glob rule exclusion

One new `.editorconfig` key, resolved per glob section like the tier key:

```ini
[docs/**.md]
prim_mdlint_strict = true
prim_mdlint_disable = MD033, MD041
```

**Subtract-only.** The key removes rules from the tier prim selected for that
path. It cannot add a rule prim decided not to run, and it cannot change a
rule's severity. prim's curated set stays the ceiling, so behaviour cannot
fragment upward and a repository cannot invent a stricter dialect of prim.

The mechanism already exists and needs no new dependency:

- `ec4rs` returns unknown keys per section through `get_raw_for_key`, with
  EditorConfig's last-match-wins cascade. `prim_mdlint_strict` already ships on
  it, proven by the #41 spike recorded in
  `docs/design/v2-editorconfig-prim-keys.md`.
- `ec4rs` lowercases keys but preserves raw values, so `MD033, MD041` arrives as
  written. Rule ids are matched case-insensitively regardless.
- `prim explain` already renders `prim_*` keys with their file, line and
  section, so the new key gets provenance for free.
- An unrecognised rule id is reported as a warning naming the key's origin and
  is then ignored. A typo must not silently disable nothing, and per AD-0007 a
  warning never raises the exit code.

Already available, and to be documented rather than built — rumdl's inline
directives pass through `rumdl_lib::lint` untouched, verified against
`prim 0.3.1`:

| Directive                                         | Scope    |
| ------------------------------------------------- | -------- |
| `<!-- markdownlint-disable-file MD033 -->`        | file     |
| `<!-- markdownlint-disable MD033 -->` … `-enable` | block    |
| `<!-- markdownlint-disable-next-line MD033 -->`   | one line |
| `<!-- rumdl-disable MD033 -->`                    | file     |

`prim_mdlint_disable` covers what those cannot: a whole tree, in one line, in
the file where the tier is already chosen.

### Resulting contract

- Floor tier: 13 rules, all error severity.
- Strict tier: 26 rules, all error severity.
- No Markdown rule emits `Warn`. Every reported Markdown finding gates.
- `prim_mdlint_disable` subtracts rules per glob; nothing adds them back.

## Consequences

### Exit codes

AD-0007 §4 is unchanged in wording: warnings still never raise the exit code.
There is simply no Markdown finding at warning severity any more. `prim lint` at
the floor tier fails on defects; at the strict tier it fails on everything it
reports, which matches rumdl's default posture.

### Issue #102

Resolved for Markdown by construction — nothing warn-only survives to be denied.
The issue stays open only if a `--fail-on` flag is wanted for future
non-Markdown content rules. Recommendation: close with a pointer to this design
and reopen when a warn-tier content rule exists.

### `MdDiagnostic::is_error`

Kept. Every Markdown finding sets it to `true` under this placement, so the
field is vestigial for Markdown, but the warn path stays specified in AD-0007
for future content rules. The field itself stays; the engine's internal
`PrimSeverity` enum does not, because with no rule ever producing `Warn` the
variant would be dead code and prim allows no warnings. The matrix becomes a
two-state activation table instead.

### `prim init` placement map

The current scaffold (`init.rs:157`) marks `docs/**.md` strict, which would put
Superpowers specs and plans under `docs/wip/` into the strict tier. Add an
exemption:

```ini
[*.md]
prim_mdlint_strict = false
[docs/**.md]
prim_mdlint_strict = true
[docs/wip/**.md]
prim_mdlint_strict = false
[**/SUMMARY.md]
prim_mdlint_strict = false
```

READMEs, `CHANGELOG.md` and everything outside `docs/` already fall to the
relaxed tier through `[*.md]`, so generated changelogs need no `.primignore`
entry for lint purposes.

### Affected surfaces

- `crates/prim-fmt/src/mdlint.rs` — `ACTIVE_RULES` and the matrix test
  `severity_matrix_matches_issue_59`.
- `docs/SPEC.md` FR-5.5b severity matrix and its "Exit-code implication" bullet.
- `docs/USAGE.md` severity table.
- `crates/prim-cli/src/init.rs` scaffold and merge, plus
  `crates/prim-cli/tests/init.rs`.
- `crates/prim-cli/tests/lint_diagnostics.rs` — the tests
  `markdown_floor_warning_prints_but_does_not_raise_the_exit_code` and
  `markdown_strict_mode_escalates_warnings_via_editorconfig` both use MD045 as
  the floor-warning fixture. MD045 becomes an error at floor, so both need
  rewriting; there is no longer a Markdown warn fixture.
- `crates/prim-cli/tests/verbs.rs`, `machine_readable.rs` — MD045 and MD034
  exit-code expectations.
- `docs/recipes.md` — CI and pre-commit examples.
- `crates/prim-cli/src/editorconfig.rs` — a list-valued resolver beside
  `prim_bool_from`, plus validation of unknown rule ids.
- `crates/prim-cli/src/provenance.rs` and `explain.rs` — surface
  `prim_mdlint_disable` in `prim explain`.
- `crates/prim-fmt/src/mdlint.rs` — `lint` takes the excluded set and filters
  `ACTIVE_RULES` after tier selection.
- `docs/USAGE.md` and `docs/SPEC.md` — document the key, its subtract-only
  semantics, and the inline directives that already work.

### Commit type

This changes prim's reported findings and the exit code for existing inputs. Per
AGENTS.md it is `feat!`, not `fix` or `refactor`.

## Alternatives considered

1. **Add `--deny-warnings` or `--max-warnings` (issue #102 as filed).** Rejected
   as the primary fix: it makes the twelve reachable but leaves MD082 gating at
   a 56 % file hit rate, so the flag is unusable on real repositories without
   also re-tiering. Retained as a possible future addition for non-Markdown
   content rules.
2. **Expose rumdl's `--fail-on any|warning|error|never` verbatim.** Rejected for
   now: it inherits the two-axis model without deciding what each rule means, so
   the noisy rules still have to be re-placed first. Cheap to add later on top
   of this design.
3. **Strict escalates every current warn rule to error, MD082 included.**
   Rejected: 451 of 800 corpus files would fail a strict gate on MD082 alone,
   and the remediation is filler prose.
4. **Upstream parity — mirror rumdl's per-rule severity.** Rejected as the
   criterion: rumdl's severities drive editor display, not gating, so importing
   them decides prim's policy by an unrelated measure. Used as corroboration
   instead (MD025 and MD001 are `Error` upstream, which agrees with placing them
   in a gating band).
5. **Fixability — enforce what `prim fix` can repair.** Rejected on inspection
   of the fixes: MD040 writes the literal language `text` on every unlabelled
   fence, MD025 demotes a heading and cascades its whole section, MD001 rewrites
   heading levels, MD026 deletes heading punctuation, MD036's fix is opt-in and
   off by default, and MD082 has no fix by design. Fixability measures how easy
   the edit is, not whether the finding is real, and most of these edits
   conflict with prim's semantics-preserving guardrail.

6. **A `prim.toml` or per-rule CLI flags for the exclusion.** Rejected: prim
   honours `.editorconfig` only, and the exclusion is inherently per-path, which
   is what `.editorconfig` sections already express. A separate file would
   duplicate the glob map that chooses the tier.
7. **Letting `prim_mdlint_disable` also enable rules.** Rejected: a repository
   could then run a stricter dialect than prim's curated set, and prim's
   canonical behaviour would fragment. Subtract-only keeps the curated set as
   the ceiling.

## Open questions

1. **#102 disposition** — close as resolved by re-tiering, or keep open for a
   future `--fail-on`.
2. **MD057 blind spot** — prim passes `source_file = None`, so relative link
   existence is never checked. Own issue; not in this change.
3. **MD082 upstream** — a leaf-only mode (flag a heading only when the next
   heading is at the same level or shallower) would make the rule usable: 412
   findings across 66 files rather than 1880 across 451. Worth raising with
   rumdl; revisit prim's placement if it lands.

## Verification

1. Rewrite `severity_matrix_matches_issue_59` to assert the new bands → verify:
   `cargo test -p prim-fmt severity_matrix`.
2. Update the integration tests listed above so each asserts the exit code the
   new placement implies → verify: `just test`.
3. Re-run the corpus measurement after the change and confirm the floor tier
   reports only Band A rules and the strict tier reports no MD082 → verify: lint
   the 800-file corpus at both tiers.
4. Cover the exclusion key: a glob with `prim_mdlint_disable` silences only the
   named rules for matching files, leaves other globs untouched, matches rule
   ids case-insensitively, and warns on an unknown id without changing the exit
   code → verify: `cargo test -p prim-cli mdlint_disable`.
5. Confirm `prim explain` renders the key with its origin → verify:
   `cargo test -p prim-cli explain`.
6. Full gate → verify: `just verify`.
