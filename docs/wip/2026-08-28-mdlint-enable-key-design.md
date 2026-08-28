# Markdown lint enable key — design

- Date: 2026-08-28
- Status: draft, awaiting review
- Issue: #123 (add `prim_mdlint_enable`, and amend AD-0012's subtract-only
  guarantee)
- Amends: AD-0012 (Markdown lint bands and rule exclusion)
- Disposition: local working memory. Garden into AD-0012, `docs/SPEC.md`,
  `docs/USAGE.md`, `docs/recipes.md` and `AGENTS.md`, then archive verbatim to
  `docs/archive/`.

## Problem

`.editorconfig` `prim_mdlint_disable` removes rules from the tier prim selected
for a path. There is no way to add one. A repository that wants a rule prim
excluded — MD013 line length being the usual request — cannot have it at any
setting, and a repository on the floor tier cannot pick up a single convention
rule without opting into all thirteen.

AD-0012 Decision 4 states the guarantee this breaks: the disable key _"can never
add a rule prim decided not to run, so a repository cannot invent a stricter
dialect of prim, and prim's curated set stays the ceiling."_ Alternative 6
rejected an additive key for that reason. Adding one is therefore an amendment
to AD-0012, not an addition alongside it.

The original objection to an additive key was that enabling excluded rules could
produce findings prim's own formatter creates — `prim fmt` output failing
`prim lint`. Measured on 0.4.0, prim's 21 tracked Markdown documents pass
markdownlint's entire default rule set, formatter-territory rules included, with
zero findings. That objection did not survive. Two others replace it, and they
shape the design below.

## Evidence

### The "off in both tiers" list is mostly not enableable

`docs/SPEC.md` lists fourteen rules as off in both tiers. Removing MD072
(semantics-preserving violation) and MD082 (dropped from `ACTIVE_RULES` by
AD-0012, so there is nothing to opt into) leaves twelve. Reading each rule's
config struct and `check()` in `rumdl 0.2.35`:

| Id    | Under prim                | Evidence                                                                 |
| ----- | ------------------------- | ------------------------------------------------------------------------ |
| MD013 | works, options supplied   | `line_length` defaults to 80 — the leak below                            |
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
what makes the last three structurally unreachable rather than merely unlikely.

The five that cannot fire without a list — MD043, MD044, MD054, MD061, MD081 —
all need a repository-supplied value. Supplying one is exactly the per-rule
options surface FR-3.3 forbids and AD-0012 Decision 3 restates. They cannot be
rescued without reversing a larger decision than the one being amended.

MD063 is a separate case worth stating precisely, because reading its config
alone gives the wrong answer. `MD063Config` carries `enabled: bool`, documented
as "opt-in rule" and defaulting to `false`, but in `rumdl 0.2.35` that field is
read nowhere outside its own config test. The rule fires whenever it is
selected. It is withheld anyway: its only meaningful setting is sentence case
versus title case, a house-style choice prim has no surface to let a repository
express. Admitting it would mean prim imposing one house style on every
repository that enables the rule, chosen by an upstream default rather than by
the repository. That is the same failure as options leakage, in a form
`.editorconfig` cannot correct.

`rumdl_lib::rules::all_rules` returns every rule including those the registry
marks `opt_in`; that filtering happens in `filter_rules`, which prim never
calls. So prim's name filter does reach MD063 — and reaches MD070, MD073 and
MD080, which AD-0012 already places in prim's tiers despite being opt-in
upstream.

### Options leakage is one rule wide

Rules whose options mirror an EditorConfig property would read rumdl's defaults
rather than the repository's resolved value. Within any set this key can reach,
`max_line_length` is the only EditorConfig property that mirrors a rule option,
and MD013 is the only rule that mirrors it.

The failure is concrete. MD013's `line-length` defaults to 80. Measured: with
`max_line_length = 120`, prim wraps prose at 119 characters, so a repository
that sets 120 and enables MD013 gets prim's own output failing prim's own lint,
at a threshold nobody chose.

The remaining exposure is not the threshold but what MD013 measures. rumdl's
non-strict mode already forgives most of what `dprint-plugin-markdown` cannot
break: a trailing token past the limit, standalone link and image lines,
HTML-only lines, link reference definitions, and inline link URLs
(`ignore-link-urls` defaults `true`). It does not forgive code blocks
(`code-blocks: true`) or headings (`headings: true`), neither of which
`prim fmt` wraps. Tables are already exempt (`tables: false`), which agrees with
prim never reflowing a table.

### What still needs a corpus

Two claims in this design have different shapes, and only one of them is a claim
a corpus can refute.

That the eight "cannot fire" rules cannot fire is a statement about rumdl's own
gating — empty default lists, unset thresholds, flavor checks against the
`Standard` prim pins. It is decidable with fixtures; a corpus adds nothing.

That prim's own output passes MD013 at the effective width is a statement about
how real documents are written: whether prose in the wild carries constructs
dprint cannot break and rumdl's non-strict forgiveness does not exempt. Only a
corpus can refute it. See [Verification](#verification).

## Decision

### 1. The key

A new `.editorconfig` key, `prim_mdlint_enable`, holding a comma-separated list
of rule ids. It resolves through the same per-glob cascade as its two siblings:
EditorConfig's ordinary last-match-wins resolution per section, so a narrower
section's value replaces a wider section's list rather than merging with it.
Rule ids match case-insensitively. `unset` (EditorConfig's own reserved word)
and `none` (prim's accepted spelling of the same intent) clear the list without
being reported as unrecognised ids.

The key means **add these rules to the set prim runs for this path**,
independent of tier.

### 2. What it reaches

Three admission classes, decided per id and independent of tier:

- **Selectable** — the 26 ids already in `ACTIVE_RULES`, plus three new opt-in
  ids: **MD013, MD014, MD069**. Enabling a floor-tier rule is a harmless no-op,
  since it already runs. Enabling a convention rule from a floor-tier path is
  the à-la-carte case the key exists to serve.
- **Withheld** — any other rule rumdl has. prim knows the rule and will not run
  it: MD072 (semantics-preserving), MD082 (dropped by AD-0012), the nine
  off-list rules above, and every formatter-territory rule.
- **Unknown** — an id naming no rumdl rule at all. A typo.

Withheld is derived from `all_rules(&cfg)` rather than a hand-maintained list,
so it stays correct when rumdl adds rules and needs no upkeep. Withheld and
Unknown warn with distinct messages, so an author can tell a deliberate refusal
from a mistyped id.

Warnings follow the contract `prim_mdlint_disable` already has: once per run for
each `.editorconfig` section that carries the id, attributed to the
`.editorconfig` file, line and section that set the key, never raising the exit
code.

`prim_mdlint_disable` gains the same three-way classification. Today it reports
any id outside `ACTIVE_RULES` as "not a rule prim runs"; once MD013, MD014 and
MD069 are reachable, `prim_mdlint_disable = MD013` must stop being reported as a
typo.

### 3. MD013's options, owned by prim

`prim_config()` becomes `prim_config(style)` and sets, alongside the existing
MD025 override:

```text
MD013: line-length              = the width the formatter used
       code-block-line-length   = 0        # rumdl's "no limit"
```

The width is `style.max_line_length.unwrap_or(80)`, extracted into
`Style::effective_line_width()` and called both by `crate::markdown`'s
`line_width` and by this config. The leak closes structurally: the linter and
the formatter read one function rather than two copies of a number, and a
repository that sets no `max_line_length` gets 80 in both places rather than 80
in one and rumdl's unrelated 80 in the other.

`code-block-line-length = 0` exempts code blocks. This follows AD-0012's own
placement test: a wide code sample fires on a document that is otherwise fine,
and has no correct fix — rewrapping a shell command changes what it says.
Headings stay checked, because a long heading is rewritable prose and therefore
a real convention violation. Tables stay off at rumdl's own default.

Every other MD013 option stays at rumdl's default, including the non-strict
forgiveness that covers what dprint cannot break.

This is prim continuing to own every rule option, as AD-0012 Decision 3 already
establishes. The amendment is only that one option is now derived from a
resolved `.editorconfig` property instead of being a constant.

### 4. Precedence and tier independence

`prim_mdlint_enable` adds to the tier's set; `prim_mdlint_disable` subtracts
from the result. `disable` therefore wins a conflict and remains a true veto.
Because the two keys resolve independently through the cascade, a narrower
section's `disable` can cancel a wider section's `enable`.

An enabled rule runs regardless of tier, so a file-level
`<!-- prim-mdlint-strict: false -->` moves the tier but does not cancel an
enable. There is no inline `prim-mdlint-enable` directive; the file-level
surface stays the strict boolean plus rumdl's own inline directives, as FR-5.5c
specifies.

### 5. Engine API

`prim_fmt::lint_markdown` currently takes `(source, strict, disabled)`. It now
needs the enable list and the width, which would make five positional
parameters. Instead:

```rust
pub fn lint_markdown(
    source: &str,
    style: &Style,
    selection: &MdLintSelection,
) -> Vec<MdDiagnostic>
```

with a new pure `prim_fmt::MdLintSelection { strict, enabled, disabled }`.
prim-cli's `MdLintPolicy` keeps its provenance fields and hands over the pure
part. Passing the same `Style` the formatter received is the point: it is what
makes the threshold agree by construction rather than by convention.

`RulePolicy`'s `floor: bool` becomes a three-value tier — `Floor`, `Convention`,
`OptIn` — and `is_active` becomes a function of the tier and the enable list:
`Floor` always runs, `Convention` runs under strict or when enabled, `OptIn`
runs only when enabled. `lint`'s second-pass filter over returned findings
applies the same predicate, as it does today.

`prim_fmt::is_known_rule` is replaced by a classifier returning the three
admission classes of Decision 2.

## Consequences

- **AD-0012's subtract-only guarantee is amended, not deleted.** prim's curated
  set stays the ceiling for 26 of the 29 reachable ids: enabling a convention
  rule from a floor path selects a rule prim already runs, at a tier prim
  already defined. Only MD013, MD014 and MD069 let a repository run something
  prim's own tiers do not. The sentence "a repository cannot invent a stricter
  dialect of prim" becomes false in that narrow, enumerated way; the reasoning
  that produced it — that prim's canonical behaviour should not fragment —
  survives as the reason the enableable set is three rules rather than the whole
  off-list.
- **AD-0012 Alternative 6** changes from _rejected_ to _superseded by Decision
  6_. It rejected letting `prim_mdlint_disable` also enable rules; the objection
  to a shared key stands, and this design uses a separate key.
- **FR-3.3 survives in the half that matters.** Its first clause — no
  `prim.toml`, no per-rule flags, no way for a repository to configure a rule's
  options — is untouched: `prim_mdlint_enable` selects rules, it never sets a
  rule's options. Its second clause, that `prim_mdlint_disable` "only ever
  narrows the rule set prim already selected, never widens it", gains a stated
  exception for the new key. A new **FR-3.2d** specifies the key itself.
- **`prim lint` stops meaning the same thing across repositories** by one more
  degree than `prim_mdlint_disable` already conceded. A repository can now be
  stricter than prim's curated ceiling, not only laxer.
- **`lint_markdown`'s signature breaks a second time**, after AD-0012's third
  parameter. prim-cli is still the only consumer.
- **Exit codes are unchanged.** An enabled rule is an error like every other
  rule prim runs; warnings about withheld or unknown ids never raise the exit
  code.
- **Commit type.** `feat` — this changes what `prim lint` reports for a
  repository that opts in.

## Affected surfaces

| Surface                                            | Change                                                                                  |
| -------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `crates/prim-fmt/src/mdlint.rs`                    | tier enum, opt-in rules, classifier, `prim_config(style)`                               |
| `crates/prim-fmt/src/style.rs`                     | `Style::effective_line_width()`                                                         |
| `crates/prim-fmt/src/markdown.rs`                  | call the new helper instead of inlining the fallback                                    |
| `crates/prim-fmt/src/lib.rs`                       | export `MdLintSelection`, replace `is_known_rule`                                       |
| `crates/prim-cli/src/mdlint_policy.rs`             | resolve the key, three-way reporting, hand over the selection                           |
| `crates/prim-cli/src/app.rs`, `lsp/diagnostics.rs` | new call signature                                                                      |
| `crates/prim-cli/src/provenance.rs` / `explain`    | show the key with file, line and section                                                |
| `docs/decisions/0012-*.md`                         | amend in place — Context, Decision 6, Alternative 6, Consequences                       |
| `docs/SPEC.md`                                     | new FR-3.2d; amend FR-3.3's no-widening sentence and FR-5.5b; re-partition the off-list |
| `docs/USAGE.md`, `docs/recipes.md`                 | the key, its reach, and the withheld reasons                                            |
| `AGENTS.md`                                        | the "one subtract-only exception" sentence                                              |

`prim init` is unchanged: the key is opt-in and there is no placement prim can
scaffold on a repository's behalf.

## Verification

### Fixtures

Eight ids assert silence: MD043, MD044, MD054, MD061, MD081, MD074, MD078 and
MD079, each run against input that should trigger it, asserting zero findings
under prim's `Standard` flavor and `source_file: None`. This pins the "cannot
fire" claim to the pinned rumdl version, so a future bump that changes any of
them fails a test rather than shipping a silent key.

MD063 asserts the opposite: that `prim_mdlint_enable = MD063` rejects the id,
since the rule does fire and is withheld by choice rather than by construction.

### Corpus

Rebuild the AD-0012 non-Rust documentation-site corpus — Docusaurus, VitePress,
MkDocs and GitBook sites that _use_ a generator rather than the generator's own
repository. Format it with prim at two or three `max_line_length` values, lint
with MD013 enabled, and classify every finding.

A finding on prim's own formatted output that is not a heading refutes the
design and sends Decision 3 back for another option. Report per-project
prevalence as well as per-file counts, and state the sampling caveats.

### Behavioural

- per-glob cascade resolution of the new key, including a narrower section
  replacing rather than merging a wider one
- `unset` and `none` clearing the list without a warning
- a convention rule enabled from a floor-tier path
- `disable` beating `enable` for the same id, across sections
- an enabled rule surviving `<!-- prim-mdlint-strict: false -->`
- the three warning classes, once per run per section, with provenance
- `prim explain` showing the key, its resolved value and its origin
- `prim_mdlint_disable = MD013` no longer reported as a typo

## Alternatives considered

1. **Accept the whole off-list minus MD072, as the issue proposes.** Rejected:
   nine of the twelve accepted ids would change nothing. A configuration key
   that looks configurable and mostly is not is worse than a smaller one.
2. **Add a per-rule options surface so MD043, MD044, MD054, MD061 and MD081
   become useful.** Rejected: this is the `prim.toml`-shaped surface FR-3.3 and
   AD-0012 Decision 3 both forbid, and it is a much larger decision than the one
   this amends.
3. **Reach only the three new opt-in rules, leaving strict-tier rules
   unreachable.** Rejected: a floor-tier repository would still have to adopt
   all thirteen convention rules at once. À-la-carte adoption stays inside
   prim's curated ceiling, so it costs the guarantee nothing that the three
   opt-in rules do not already cost.
4. **Let MD013 use rumdl's defaults once `line-length` is supplied.** Rejected:
   `code-blocks: true` reports wide code samples prim will never wrap and a
   repository cannot fix without changing what the code says — the shape
   AD-0012's own placement test rules out.
5. **Pin MD013 to prose only.** Rejected: `prim fmt` already guarantees prose
   width, so the rule would collapse into an unformatted-input detector that
   `prim fmt --check` already covers.
6. **A new AD-0013 superseding AD-0012.** Rejected: the working-memory lifecycle
   rule says to edit an existing record in place when new work changes it, and
   reserves a new record for a genuinely new topic. The subtract-only guarantee
   is AD-0012's own clause.

## Open questions

None blocking. The corpus run in [Verification](#verification) is the one
remaining way this design can be refuted before it is gardened into AD-0012.
