# Markdown line-length linting — design

> **Superseded.** This is archived working memory, kept verbatim as the record
> of how the design was reached. Two things in it were later found wrong and are
> corrected in AD-0014 and `docs/USAGE.md`: prose _is_ reported (that is the
> feature's purpose — `prim fmt` wraps it, so a formatted repository has none
> left), and AD-0012's ceiling claim did need amending. Read the durable records
> for what prim actually does.

Status: design settled, not implemented. Supersedes the `prim_mdlint_enable`
framing in [#123](https://github.com/driftsys/prim/issues/123).

## Problem

prim wraps Markdown prose to `max_line_length` and reports nothing about line
length. MD013 is off in both lint tiers, and `prim_mdlint_disable` is
subtract-only, so a repository that wants line length enforced cannot have it at
any setting. Re-enabling MD013 is the most frequently requested exception to
AD-0012's curated rule set.

The obstacle is that MD013 is the first rule whose _options_ must track the
formatter. Enabled at rumdl's defaults, it reports lines prim itself just wrote.

## Decision

Add one `.editorconfig` key, `prim_mdlint_report_line_length` (boolean, default
`false`). When true, `prim lint` runs MD013 with options prim owns:

| Option        | Floor                      | Strict  | Reason                                            |
| ------------- | -------------------------- | ------- | ------------------------------------------------- |
| `code-blocks` | `false`                    | `false` | FR-1.6 preserves fenced content verbatim.         |
| `code-spans`  | `false`                    | `false` | FR-1.1a keeps inline code atomic.                 |
| `tables`      | `false`                    | `false` | Pinned, not inherited — rumdl's default may move. |
| `headings`    | `false`                    | `true`  | Unwrappable by a formatter, fixable by an author. |
| `line-length` | resolved `max_line_length` |         | Written to both `global` and `[MD013]`.           |

The governing idea, and the one users need: **prim reports only the long lines
you can act on.** Prose is never reported because prim already wrapped it. Table
rows, fenced code, inline code spans and link URLs are never reported because a
line break cannot be inserted into any of them without changing what the
document means. Headings sit in between — prim cannot wrap one, but an author
can shorten the wording — so they are reported at the strict tier only.

`heading-line-length` is deliberately not set. One limit, not two.

The key is a verb phrase, not a noun phrase, because it holds a boolean.
EditorConfig's own convention separates the two — `max_line_length` and
`indent_size` hold numbers, `insert_final_newline` and
`trim_trailing_whitespace` hold booleans — and `prim explain` prints the whole
honored set together, where a key ending in `_length` holding `true` sits four
lines below the numeric `max_line_length`. The failure that name invites is
concrete: a repository writes `prim_mdlint_line_length = 100` expecting a
separate lint limit and gets silence, because an unrecognized value falls back
to the default the way `prim_mdlint_strict` does.
`prim_mdlint_check_line_length` was rejected in turn: `--check` already names
the CI gate mode in prim's vocabulary.

## Evidence

Measured on prim's own 34 tracked Markdown files, formatted by prim at
`max_line_length = 80`, counting characters rather than bytes.

Lines longer than 80, by construct:

| Construct           | Lines |
| ------------------- | ----- |
| Table rows          | 152   |
| Fenced code content | 36    |
| Inline code spans   | 8     |
| Links               | 4     |
| Prose               | 0     |
| Headings            | 0     |
| Raw HTML            | 0     |

Every overflowing line is a construct the formatter deliberately refuses to
break. No prose line overflows. This is what makes the three-bucket model true
rather than aspirational.

At rumdl's default MD013 options the same corpus yields roughly 44 findings — 36
from `code-blocks = true` and 8 from `code-spans = true` — on files prim itself
formatted. That is the `prim fmt` output failing `prim lint` failure mode, and
it is what the four prim-owned options exist to prevent. The 44 is arithmetic
over the measured corpus and rumdl's documented defaults; MD013 was not executed
against the corpus.

Headings, split by the tier each file resolves to today:

| Tier   | Headings | Over 80 |
| ------ | -------- | ------- |
| Floor  | 287      | 4       |
| Strict | 176      | 0       |

All four overflowing headings are in `docs/archive/plans/`, which resolves to
the floor tier. Enabling this feature on prim itself is a no-op today.

The margin at the strict tier is thin: `docs/decisions/0002-*.md`'s H1 is 78
characters and several other decision-record titles sit between 72 and 77.
Renaming a decision record by three words would fail CI. This is the rule
working as specified, but it is a real cost for `docs/decisions/`, where titles
are long by convention.

## Mechanics that forced the choices

**Headings cannot be wrapped.** A line break ends an ATX heading; the remainder
parses as a paragraph. Verified with pandoc: `# Heading\nrest` renders as `<h1>`
plus `<p>`, not one heading. `dprint-plugin-markdown` exposes no heading-wrap
option because there is no correct one. The same argument covers table rows,
fenced code, inline code spans and link destinations. Setext headings are the
one multi-line-capable heading form; prim does not configure `heading_kind`.

**rumdl's global/rule precedence is a sentinel check, not a presence check.**
`MD013::from_config` overwrites the rule's `line-length` with the global value
whenever the rule value still equals the default 80. An explicit
`[MD013]
line-length = 80` alongside a global 120 therefore silently resolves
to 120. prim avoids this by writing the same resolved value to both
`global.line_length` and `[MD013] line-length`: when the value is not 80 the
check is false and the rule value stands; when it is 80 the overwrite
substitutes an identical value. Both branches converge, so rumdl's precedence
rule becomes irrelevant to prim — including if rumdl later changes it.

**Severity is per-rule, not per-context.** `RuleConfig.severity` covers all of
MD013, so reporting headings at a lower severity than prose is not expressible.
Combined with AD-0012's removal of warning severity for Markdown, a heading
finding is an error or it is nothing. The tier is the only available dial, which
is why `headings` varies by tier rather than by severity.

**Length is measured in display columns.** rumdl's `length-mode` defaults to
`visual` (`unicode_width`), matching how dprint wraps. The two axes agree on the
unit. Byte-based counting does not: an initial measurement of this corpus with
`awk` reported around 50 overflowing prose lines, all of which were 80-column
lines containing em dashes. Any future measurement must count columns.

## Alternatives considered

1. **Generic `prim_mdlint_enable`, as
   [#123](https://github.com/driftsys/prim/issues/123) was filed.** Rejected. It
   advertises that any excluded rule can be re-enabled, when MD013 is the only
   rule for which the option analysis above has been done. The next request
   would arrive with no such analysis. It also breaks AD-0012's subtract-only
   ceiling, which the narrow key leaves standing. The cost of the narrow key is
   that a second rule earning re-entry needs a second boolean rather than
   reusing a mechanism; that trade is accepted deliberately, because it forces
   each rule to be argued on evidence.

2. **`headings = false` at both tiers.** Consistent with tables and simpler to
   document, but it discards a finding an author can act on. Rejected in favour
   of the tier split, at the cost of the tier model now varying a rule's options
   and not only which rules run.

3. **`heading-line-length` set higher than `line-length`.** Would relieve the
   thin margin in `docs/decisions/`. Rejected for now: it adds a fifth
   prim-owned option and weakens the single-canonical-limit story. Reconsider
   only if a real rename is actually blocked.

4. **Reporting headings as warnings.** Not expressible. See severity above.

## What must change

- **FR-3.3** — the sentence "no way for a repository to configure a rule's
  options" becomes false in letter once `max_line_length` feeds MD013's
  `line-length`. The intent is preserved: there is still no per-rule dial. The
  wording needs an amendment naming this one path.
- **AD-0012** — no amendment needed. The subtract-only guarantee for
  `prim_mdlint_disable` is untouched; this key adds a rule prim selected for
  itself rather than widening what a repository may select.
- **FR-5.5b** — record the new key, the tier-varying `headings` option, and the
  fact that MD013 moves out of "off in both tiers".
- **`prim_config()`** ([mdlint.rs](../../crates/prim-fmt/src/mdlint.rs)) — takes
  the resolved line length and the tier; currently a constant with no inputs.
- **`lint_markdown`** — gains a fourth parameter, threaded through three call
  sites: `app/paths.rs`, `app/stdin.rs`, `lsp/diagnostics.rs`. This is a
  breaking change for `prim-fmt` consumers, the second after AD-0012's third
  parameter.
- **`prim explain`** — prints the complete honored-key set with provenance,
  including keys nobody set (`prim_mdlint_disable = unset (prim's default)`), so
  a key absent from its output would be a defect rather than a scope choice. The
  new key ships with the same change, not as follow-up work. It is 30 characters
  against the current 24-character label column, so the alignment widens by six.
- **`docs/USAGE.md`** — the honored-keys table, the lint-tier section, and the
  plain-language explanation drafted below.

## Draft user-facing prose

> ### Line length
>
> `max_line_length` (default 80) already controls how prim wraps Markdown prose.
> Setting `prim_mdlint_report_line_length = true` additionally makes `prim lint`
> report lines that exceed it.
>
> prim only reports lines you can do something about. Prose is never reported,
> because prim already wrapped it. Table rows, fenced code, inline code spans
> and link URLs are never reported either: a line break cannot be inserted into
> any of them without changing what the document means, so there is nothing to
> fix.
>
> Headings are the one case in between. prim cannot wrap a heading — a line
> break would end the heading and turn the rest into a paragraph — but you can
> shorten the wording. So a long heading is reported when
> `prim_mdlint_strict = true`, and silent otherwise, like every other
> convention-tier check.
>
> The limit is whatever `max_line_length` resolves to for that file, so the
> formatter and the linter always agree.
>
> If your headings are long by convention — numbered decision records, for
> example — keep those files at the floor tier.

MD013 is named in the reference table but not in this explanation: readers
arriving from markdownlint search for the code, while readers meeting the
feature for the first time should not have to hold a rule id, five options and a
tier split at once.

## Open items

- Verify the 44-finding arithmetic by executing MD013 against the corpus once
  the wiring exists. It is currently arithmetic over the measured corpus and
  rumdl's documented defaults, not an observed run.
- Decide whether `prim init` should scaffold the key, or leave it unset so a
  repository opts in deliberately.
