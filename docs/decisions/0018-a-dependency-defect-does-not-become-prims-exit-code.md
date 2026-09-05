# AD-0018 — A dependency defect does not become prim's exit code

## Status

Accepted. Closes #180. Not breaking: it removes findings prim should never have
reported, and adds none.

## Context

prim selects rumdl's rules and owns their output. A rule prim selects raises
prim's exit code, so a false positive upstream is a failed build downstream, and
the consumer has no way to tell prim's judgement from rumdl's.

MD051 reports a link fragment that resolves to no heading. rumdl computes a
heading's slug the way GitHub does, with one difference it did not intend:

```rust
} else if c == '§' {
    // Preserve our marker character
    result.push(c);
}
```

rumdl builds its emoji placeholder as the literal string `§emoji§`, and
preserves U+00A7 verbatim so that sentinel survives the character pass
(`rumdl/src/utils/anchor_styles/github.rs`). GitHub keeps only letters, digits,
spaces, hyphens and underscores, so it strips the character. For

```markdown
[l](#a-1-b)

## A §1 B
```

rumdl computes `a-§1-b` and reports the link; a reader's browser resolves
`a-1-b` and follows it. The finding is unactionable in the strict sense: the
ways to satisfy it are to write an anchor that resolves in no renderer, to
remove the character from the heading, or to add an explicit
`<a id="a-1-b"></a>` beside it — rumdl collects HTML anchors and accepts a
fragment from that set. None of the three is a change the author had a reason to
make.

A heading of the form `## 9. Worked Examples — the ridl §13 Contracts` is
ordinary in specification prose, and `§` is exactly the character such prose
reaches for. It was found that way: a ridl CI job that installs prim from
`install.sh` went from passing on 0.3.0 to failing on 0.7.0, on a link that is
not broken.

The slug defect is rumdl's and predates every version of prim that has run
MD051. What turned it into a failing build is prim's own: 0.4.0 made a lint
finding raise the exit code, and AD-0012 placed MD051 in the always-on floor
tier, one of six rules it names as not having gated there before. The false
positive was produced all along; between 0.3.0 and 0.7.0 prim started failing on
it.

The defect is upstream, and no release fixes it. It reproduces identically under
the pinned `rumdl = "=0.2.35"`, under 0.2.63, and under 0.2.65, the latest
release at the time of writing, so no bump reaches it.

## Options

1. **Report upstream and change nothing.** A consumer who hits it reaches for
   `prim_mdlint_disable` (AD-0012), which is the subtract-only hatch that exists
   for this. Zero risk, zero code — and an unbounded wait, during which every
   affected build stays red and the workaround costs each consumer the whole
   rule for that path.
2. **Drop MD051 from the floor tier** until the slug is right. Two lines, no
   cleverness. It withdraws a rule that reports something objectively broken
   from every repository, to work around a defect that reaches only headings
   holding one character.
3. **Reimplement the slug.** prim would compute anchors itself and filter
   findings against its own answer. It buys precision prim has no use for and
   makes prim own a rule surface AD-0012 and AD-0013 deliberately keep out of
   reach — a second slug implementation to keep in step with GitHub forever.
4. **Neutralise the sentinel and re-ask rumdl.**

## Decision

Option 4. When a document holds U+00A7 and MD051 reported against it, prim lints
a second copy in which every occurrence of that character has been replaced by a
stand-in, and drops the MD051 findings the second pass does not report. Every
other rule's findings pass through untouched; the filter reaches MD051 alone.

The second pass is an oracle, not a result: positions and messages come from the
first. A source holding none of the character cannot trigger the collision, so
what the second pass reports is what GitHub's own slug would produce. A finding
in both is one a reader would also hit. A finding only the first pass reports is
the artifact.

This keeps the rule at full strength for every document, and asks rumdl rather
than second-guessing it: prim gains no slug implementation of its own. When
upstream lands, the workaround goes by deleting `mdlint/section_sign.rs` and
`mdlint/section_sign/` whole, together with the module declaration and the one
call at the end of `lint`. `FLAVOR` stays: the first pass reads it too. Stating
the removal as a directory rather than a list of items is deliberate — an
earlier draft of this record named a count of functions and constants, and it
was wrong within one revision.

The second pass **substitutes** rather than deletes. Deleting removes a
character, and findings are matched between the passes by position, so a
document reading `[a](#dead1) § [b](#dead2)` reported the second dead link one
column to the left in the second pass, matched nothing, and lost a finding that
was genuine. Substituting one character for one character leaves every position
where it was.

One character, not two bytes: rumdl reports a column as a character offset
within the line, so a stand-in of equal byte width but two characters would
shift every column after it just as deletion did. That property is guaranteed by
the stand-in's `char` type rather than asserted. The property that does depend
on rumdl — that the stand-in is stripped from a computed slug exactly as GitHub
strips U+00A7 — is pinned by a test over every candidate, so a future rumdl that
started retaining one fails the build.

The stand-in is **chosen per document**, as the first candidate the source does
not already contain. MD051 does not resolve every fragment through a slug: an
explicit `<a id="...">` is stored verbatim and compared as a literal string, so
substituting a character the document already uses can make two literals equal
that were not. A fragment `#x°y` beside `<a id="x§y">` is dead as written, and
was silently swallowed while the stand-in was fixed at U+00B0. A stand-in the
document does not contain cannot collide with anything in it. When the document
contains every candidate, prim suppresses nothing rather than risk it.

The substitution runs over the whole source rather than over heading lines
alone. A setext heading's text line carries no marker, so recognising headings
means parsing Markdown, and prim does not hand-roll a Markdown parser (the same
reasoning as #149). Substituting everywhere needs no parse, covers ATX and
setext headings alike, and costs nothing elsewhere: the stand-in is stripped
from a slug wherever it lands, and a link fragment holding it matches no slug in
either pass.

It does not reach a heading written as raw HTML. rumdl computes no slug for an
`<h2>` element, so `#a-1-b` was already reported against one before this change
and still is. That is rumdl's pre-existing behaviour rather than a limit this
correction introduces, and #149 is where cross-renderer anchor questions of that
kind belong.

The cost is one extra lint pass, restricted to MD051, for a document that holds
the character _and_ already produced a finding. Every other document takes an
unchanged path.

## Consequences

- **The rule keeps its coverage.** A dead anchor still reports, including in the
  documents the workaround touches: beside a section-sign heading, on a line
  that itself holds the character, and in a fragment that holds it.
- **`prim_mdlint_disable` still wins.** The pass runs inside the tier prim
  already selected, and never puts back a rule a consumer removed.
- **A second pass that cannot run suppresses nothing.** The findings the first
  pass produced reach the caller unchanged. That covers a failed lint, a rule
  set that came back empty, and a document holding every stand-in candidate — in
  each case prim reports what rumdl said rather than guessing, so the upstream
  false positive returns rather than a real finding disappearing.
- **One case stays as it was**, because the pass can only subtract. A link
  written `#a-§1-b` beside a heading `## A §1 B` resolves no anchor a renderer
  produces, and rumdl matches it against its own slug and reports nothing — with
  no first-pass finding there is nothing to filter. Closing that would need
  option 3.
- **This is a workaround with an owner.** It is prim's to delete. The exit
  condition is the upstream report, [rvben/rumdl#854][rumdl-854], filed on
  2026-09-05. Its fix, [rvben/rumdl#855][rumdl-855], was merged the same day as
  [rvben/rumdl@d530737][rumdl-d530737]; no release carries it yet — 0.2.65 is
  the latest at the time of writing. The fix deletes the emoji marker pass the
  sentinel served, rather than renaming the sentinel: the same pass also counted
  one hyphen too many around an emoji not surrounded by spaces (`## A🚀 B`
  slugged to `a--b` where GitHub resolves `a-b`), a second false positive prim
  does not correct. When a rumdl release carries the fix, bump the pin and
  delete the workaround as described above.

[rumdl-854]: https://github.com/rvben/rumdl/issues/854
[rumdl-855]: https://github.com/rvben/rumdl/pull/855
[rumdl-d530737]: https://github.com/rvben/rumdl/commit/d530737

## A limit this record does not close

prim now carries a local correction for one upstream defect. `docs/SPEC.md` and
`docs/USAGE.md` say so in prose for this rule, but prim has no general way to
carry that state — no machine-readable record a consumer could read, and nothing
that ties the correction to the upstream report that ends it.

Whether prim wants a general "provisionally corrected pending an upstream fix"
surface, distinct from a consumer's own `prim_mdlint_disable`, is worth deciding
once rather than per rule. One instance is not yet a pattern, and a mechanism
built for one case would be built against a sample of one. This record leaves it
open and marks the second occurrence as the trigger.
