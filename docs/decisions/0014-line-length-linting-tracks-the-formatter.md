# AD-0014 — Line-length linting: MD013 tracks the formatter's own width

## Status

Accepted. New behavior: a new `.editorconfig` key,
`prim_mdlint_report_line_length` (boolean, default `false`), selects MD013 into
whichever Markdown lint tier the path already runs. prim owns the five MD013
options that decide what it reports, and exposes none of them. This is the
second named exception to FR-3.3's "no way for a repository to configure a
rule's options," after AD-0012's `prim_mdlint_disable`.

## Context

MD013 (line length) has been off in both of AD-0012's tiers since that decision
landed: prim wraps Markdown prose to `max_line_length` and reported nothing
about line length at all. `prim_mdlint_disable` only removes a rule from the
tier prim already selected for a path, so a repository that wanted line length
enforced had no setting that could reach it. Re-enabling MD013 is the most
frequently requested exception to AD-0012's curated rule set, filed as
[#123](https://github.com/driftsys/prim/issues/123) under the framing
`prim_mdlint_enable`, a generic re-enable switch for any excluded rule.

MD013 is not like the rules AD-0012 already placed. Every one of those is either
always safe to run (a floor-tier defect) or safe once a repository opts in (a
strict-tier convention), and none of them depend on a value the repository sets
elsewhere. MD013 does: at rumdl's own defaults it reports lines prim's formatter
just wrote, because the formatter and the rule do not agree on what "too long"
means unless something makes them agree.

### Measuring what already overflows

Measured on prim's own 34 tracked Markdown files, formatted by prim at
`max_line_length = 80`, counting characters in display columns rather than
bytes:

| Construct           | Lines over 80 |
| ------------------- | ------------: |
| Table rows          |           152 |
| Fenced code content |            36 |
| Inline code spans   |             8 |
| Links               |             4 |
| Prose               |             0 |
| Headings            |             0 |
| Raw HTML            |             0 |

Every overflowing line belongs to a construct the formatter deliberately refuses
to break. No prose line overflows, because prim already wrapped every one of
them to the same limit MD013 would measure against. This is the evidence the
three-bucket model below rests on: it describes what prim's own corpus actually
contains, not an assumption about what Markdown documents in general look like.

### The cost of enabling MD013 at rumdl's defaults

At rumdl's default MD013 options, the same corpus yields roughly 44 findings —
36 from `code-blocks = true` and 8 from `code-spans = true` — on files prim
itself formatted. That number is arithmetic over the measured corpus and rumdl's
documented defaults; MD013 was not executed against the corpus to produce it. It
is the shape of the failure this decision exists to prevent: a repository
turning on line-length linting and immediately failing CI on output prim itself
produced, with no line the author could act on.

### Headings, split by tier

Over the same 34 non-archive files as the table above, measured at `4bebce5`,
the commit this branch starts from:

| Tier   | Headings | Over 80 |
| ------ | -------: | ------: |
| Floor  |      111 |       0 |
| Strict |      176 |       0 |

Not one heading in the corpus exceeds 80 columns, at either tier, so enabling
this feature on prim's own repository today is a no-op for headings. Widening
the sweep to all 51 tracked files at that commit finds exactly four overflowing
headings, all of them in `docs/archive/plans/`, which resolves to the floor tier
under AD-0012's `[docs/archive/**.md]` exemption and is excluded from the corpus
for the same reason: archived material is not maintained prose.

The margin at the strict tier is thin, not absent: `docs/decisions/0002-*.md`'s
H1 is 78 display columns, and the next longest,
`0003-json-via-dprint-plugin-json.md`, is 72. Measuring the same titles in bytes
would put three in that band, which is the unit this record forbids everywhere
else. Renaming a decision record by three words could fail CI once the key is
turned on for `docs/decisions/`. That is the rule working as specified, not a
defect, but it is a real cost for a directory where titles are long by
convention — recorded here so a future rename knows why the file grew close to
the limit, not because of it.

### Why a heading cannot be wrapped

A line break ends an ATX heading; whatever follows parses as a new paragraph,
not a continuation. Verified directly:

```console
$ printf '# Heading\nrest\n' | pandoc -f markdown -t html
<h1 id="heading">Heading</h1>
<p>rest</p>
```

`dprint-plugin-markdown` exposes no heading-wrap option because there is no
correct one to expose. The same argument covers table rows, fenced code, inline
code spans, and link destinations: a line break changes what each of them means,
so none of them can be reflowed by a formatter, and MD013 must leave all four
alone regardless of tier.

### rumdl's global/rule precedence is a sentinel check, not a presence check

rumdl 0.2.35's `MD013::from_config` (`src/rules/md013_line_length.rs:699-708`)
resolves the rule's effective `line-length` like this:

```rust
fn from_config(config: &crate::config::Config) -> Box<dyn Rule>
where
    Self: Sized,
{
    let mut rule_config = crate::rule_config_serde::load_rule_config::<MD013Config>(config);
    // Use global line_length if rule-specific config still has default value
    if rule_config.line_length.get() == 80 {
        rule_config.line_length = config.global.line_length;
    }
    let mut rule = Self::from_config_struct(rule_config);
    ...
}
```

The check is `== 80`, the rule's own compiled-in default, not "was this value
set." An explicit `[MD013] line-length = 80` sitting beside a global
`line_length = 120` is therefore silently overwritten to 120 — the section that
looks like it pins the rule's width does not. prim writes the same resolved
`max_line_length` to both `config.global.line_length` and `[MD013] line-length`
so both branches of that check converge on the identical value: when the
resolved width is not 80 the sentinel is false and the rule value already
stands; when it is exactly 80 the overwrite substitutes an identical number.
Either way the two settings cannot disagree, including if rumdl changes this
precedence rule later.

### Severity is per-rule, not per-context

rumdl's `RuleConfig.severity` covers every finding MD013 produces; there is no
way to report a heading at a different severity than a prose line within the
same rule. Combined with AD-0012's removal of a warning severity for Markdown, a
heading finding is an error or it does not exist. The tier is therefore the only
dial available for varying how MD013 treats headings, which is why `headings` is
the one option that changes between floor and strict rather than staying fixed
like the other four.

### Length is measured in display columns

rumdl's `length-mode` defaults to `visual` (Unicode display width — CJK
characters and emoji count as two columns), matching how
`dprint-plugin-markdown` wraps prose. prim does not override `length-mode`; the
two tools already agree on the unit without prim's intervention. An earlier,
byte-based measurement of this corpus with `awk` reported roughly 50 overflowing
prose lines, every one of them an 80-column line containing an em dash — three
UTF-8 bytes counted as one display column. Any future measurement of this corpus
must count columns, not bytes, or it reproduces that false positive.

One related option needed no decision at all: rumdl's `ignore-link-urls`
defaults to `true`, so a line that is long only because of a link destination is
already excluded from MD013's count without prim setting anything. The
`Links: 4` row in the corpus table above is exempt for that reason and for
rumdl's link-reference-definition and standalone-link rules — not through
`code-blocks`/`code-spans`, and so not part of the 44 above. prim leaves all
three unpinned, which means that row is rumdl's behaviour rather than prim's
guarantee; if a future rumdl flips `ignore-link-urls`, prim would start
reporting link lines without any decision here changing.

## Decision

1. **One `.editorconfig` key, subtract-only in spirit.**
   `prim_mdlint_report_line_length = true|false` (default `false`) selects MD013
   into whichever tier `prim_mdlint_strict` already resolved for that path. It
   resolves through the same per-glob cascade. It does not change which tier
   applies, and `prim_mdlint_disable = MD013` removes the rule again exactly
   like any other rule prim runs.

2. **prim owns the five MD013 options it sets; a repository owns none of them.**

   | Option        | Floor                      | Strict  | Reason                                                |
   | ------------- | -------------------------- | ------- | ----------------------------------------------------- |
   | `line-length` | resolved `max_line_length` |         | So the formatter and the linter cannot disagree.      |
   | `code-blocks` | `false`                    | `false` | FR-1.6 preserves fenced content verbatim.             |
   | `code-spans`  | `false`                    | `false` | FR-1.1a keeps inline code atomic.                     |
   | `tables`      | `false`                    | `false` | Pinned, not inherited — rumdl's own default may move. |
   | `headings`    | `false`                    | `true`  | Unwrappable by prim, fixable by an author.            |

   `heading-line-length` is deliberately left unset: one limit, not two.

3. **The limit is written to both `global.line_length` and MD013's own
   `line-length`.** This neutralizes the sentinel-check precedence described
   above regardless of which branch it takes.

4. **`headings` is the only option that varies by tier**, because the tier is
   the only severity dial MD013 exposes and a heading is the one construct in
   this rule's scope that an author, rather than the formatter, can act on.

5. **`prim_config()` and `lint_markdown` take the resolved line length as an
   input.** `prim_config()` gains a `strict: bool` and a
   `line_length: Option<usize>` parameter; `lint_markdown` gains a fourth
   parameter threaded through its three call sites
   (`crates/prim-cli/src/app/paths.rs`, `crates/prim-cli/src/app/stdin.rs`,
   `crates/prim-cli/src/lsp/diagnostics.rs`).

## Consequences

- **FR-3.2a** now names three `prim_*` keys instead of two, and a new
  **FR-3.2d** records this key's contract.
- **FR-3.3** gains one named exception: `max_line_length` supplies MD013's
  `line-length`. The exception is narrow by construction — it reuses a value the
  repository already set for the formatter, and it is still not a per-rule dial
  or a way to reach any other option of any rule.
- **FR-5.5b** records MD013 as "selected by `prim_mdlint_report_line_length`,
  off otherwise" rather than folding it into the "off in both tiers" list it sat
  in before this decision.
- **`lint_markdown` gained a fourth parameter.** This is a breaking change for
  any crate calling `prim-fmt` directly, the second such change after AD-0012's
  third parameter (the exclusion list).
- **`prim explain` gained a row.** It prints every honored key including ones
  nobody set, so a key absent from its output would be a defect rather than a
  scope choice; the new key ships in the same change as the feature, not as
  follow-up work.
- **AD-0012 is amended, in one specific way.** Its subtract-only guarantee for
  `prim_mdlint_disable` — that the key can only remove a rule from the tier prim
  already selected, never add one — is untouched, and this decision does not
  change that key at all. What this decision does break is the wider conclusion
  AD-0012 drew alongside it: that "a repository cannot invent a stricter dialect
  of prim, and prim's curated set stays the ceiling." A repository setting
  `prim_mdlint_report_line_length` now runs MD013, which neither tier selects,
  so its active set exceeds prim's curated tiers by one rule. Issue #123 said
  this from the start and it was briefly argued away during design; the ceiling
  sentence is struck from AD-0012 item 4 and replaced with the narrower property
  that survives — a repository may select only from rules prim has designed for,
  one key at a time, configuring none of their options beyond what FR-3.3 names.
- **rumdl's `markdownlint-configure-file` directive reaches these options.**
  AD-0012 admitted rumdl's inline directives as a per-file escape hatch, and
  they are applied inside `rumdl_lib::lint` after prim builds its config. While
  MD013 never ran, that hatch could only turn rules off; now a file carrying a
  `markdownlint-configure-file` directive can change the width or re-enable the
  table and code checks for itself. It cannot select MD013 where the key has
  not, because prim filters the rule set before rumdl sees it. The guarantee
  this decision makes is therefore about the `.editorconfig` cascade, which no
  repository-wide setting can bend, not about a single file that deliberately
  opts out.
- **`prim init` does not scaffold the key.** It scaffolds the strict-glob map
  and nothing else, so a repository opts into line-length reporting deliberately
  rather than finding it switched on by a tool run.
- **Enabling this key on prim's own repository today is a no-op**, per the
  headings-by-tier measurement above, but the margin at the strict tier is thin
  enough that a future decision to enable it for `docs/decisions/` should expect
  occasional heading-length friction on renames, not treat it as free.
- **Issue #123 is resolved** by a narrow key rather than the generic
  `prim_mdlint_enable` it was filed as. Reopening it for a rule other than MD013
  requires the same option analysis this record performed, not reuse of this
  mechanism.

## Alternatives considered

1. **A generic `prim_mdlint_enable` key, as #123 was filed.** Rejected. It
   advertises that any rule AD-0012 excluded can be re-enabled, when MD013 is
   the only one this record has done the option analysis for. The next re-enable
   request would arrive with no such analysis behind it, and the key would also
   break AD-0012's subtract-only ceiling, which the narrow key leaves standing.
   The cost accepted in exchange is that a second rule earning re-entry needs
   its own boolean rather than reusing a mechanism — accepted deliberately, so
   each rule is argued on its own evidence.
2. **`headings = false` at both tiers.** Simpler to document and consistent with
   tables, but it discards the one finding in MD013's scope that an author can
   act on. Rejected in favor of the tier split, at the cost of a tier now
   varying a rule's options and not only which rules run.
3. **`heading-line-length` set higher than `line-length`.** Would relieve the
   thin margin measured in `docs/decisions/`. Rejected for now: it adds a sixth
   prim-owned option and weakens the single-canonical-limit story this decision
   otherwise tells. Worth reconsidering only if a real rename is actually
   blocked by the strict-tier margin.
4. **Report headings at a lower severity than prose instead of gating them by
   tier.** Not available: `RuleConfig.severity` applies to all of MD013, and
   AD-0012 already removed a warning severity for Markdown entirely, so there is
   no severity axis left to report through.

---

Satisfies: #123; extends FR-3.2a with FR-3.2d, names one exception in FR-3.3,
and updates FR-5.5b's rule-tier and rule-option lists. Related: AD-0012
(Markdown lint bands — the tier this key selects MD013 into; its subtract-only
guarantee for `prim_mdlint_disable` is unchanged),
`crates/prim-fmt/src/mdlint.rs`, `crates/prim-cli/src/mdlint_policy.rs`,
`crates/prim-cli/src/provenance.rs`,
`crates/prim-cli/tests/mdlint_line_length.rs`.
