# `docs/USAGE.md` draft — Markdown line-length linting

> **Superseded.** This is archived working memory, kept verbatim as the record
> of how the design was reached. Two things in it were later found wrong and are
> corrected in AD-0014 and `docs/USAGE.md`: prose _is_ reported (that is the
> feature's purpose — `prim fmt` wraps it, so a formatted repository has none
> left), and AD-0012's ceiling claim did need amending. Read the durable records
> for what prim actually does.

Drop-in replacement text for the four places `docs/USAGE.md` must change when
`prim_mdlint_report_line_length` lands. Companion to
[the design note](2026-08-29-md-line-length-lint-design.md).

## 1. New subsection, under `prim lint`'s Markdown coverage

### Line length

`max_line_length` (default 80) already controls how prim wraps Markdown prose.
Setting `prim_mdlint_report_line_length = true` additionally makes `prim lint`
report lines that exceed it.

prim only reports lines you can do something about. Prose is never reported,
because prim already wrapped it. Table rows, fenced code, inline code spans and
link URLs are never reported either: a line break cannot be inserted into any of
them without changing what the document means, so there is nothing to fix.

Headings are the one case in between. prim cannot wrap a heading — a line break
would end the heading and turn the rest into a paragraph — but you can shorten
the wording. So a long heading is reported when `prim_mdlint_strict = true`, and
silent otherwise, like every other convention-tier check.

The limit is whatever `max_line_length` resolves to for that file, so the
formatter and the linter always agree.

If your headings are long by convention — numbered decision records, for example
— keep those files at the floor tier.

What is reported, in full:

| Line content                          | Floor  | Strict   |
| ------------------------------------- | ------ | -------- |
| Prose (already wrapped by prim)       | silent | silent   |
| Table row                             | silent | silent   |
| Fenced or indented code               | silent | silent   |
| Line long only because of inline code | silent | silent   |
| Line long only because of a link URL  | silent | silent   |
| Heading                               | silent | reported |

Findings are printed like every other Markdown finding, carrying rumdl's rule
code verbatim:

```console
$ prim lint docs/decisions/0014-example.md
docs/decisions/0014-example.md:1:81: Line length 94 exceeds 80 characters [MD013]
```

This is MD013. It is the one rule prim excludes from both tiers by default and
allows a repository to switch on, because it is the only rule whose options must
track the formatter's own line width. The options prim sets for it are not
configurable:

| MD013 option  | Floor                      | Strict  | Why                                        |
| ------------- | -------------------------- | ------- | ------------------------------------------ |
| `line-length` | resolved `max_line_length` |         | So the formatter and linter agree.         |
| `code-blocks` | `false`                    | `false` | prim preserves fenced content verbatim.    |
| `code-spans`  | `false`                    | `false` | prim keeps inline code on one line.        |
| `tables`      | `false`                    | `false` | A table row cannot carry a line break.     |
| `headings`    | `false`                    | `true`  | prim cannot wrap a heading; an author can. |

`prim_mdlint_disable = MD013` turns the rule off again for a narrower glob, the
same way it removes any other rule from the tier a path runs.

## 2. Honored-keys table — new row

Add after `prim_mdlint_disable`:

| Key                              | Effect                                                                                                   |
| -------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `prim_mdlint_report_line_length` | `false` (default) or `true`; report Markdown lines longer than `max_line_length` (Markdown lint, MD013). |

## 3. Scope notes — two edits

Replace:

> - `prim_mdlint_strict` and `prim_mdlint_disable` are currently the **only**
>   documented `prim_*` keys.

with:

> - `prim_mdlint_strict`, `prim_mdlint_disable` and
>   `prim_mdlint_report_line_length` are currently the **only** documented
>   `prim_*` keys.

Add a note after the `prim_mdlint_disable` paragraph:

> - `prim_mdlint_report_line_length` resolves through the same per-glob cascade
>   as `prim_mdlint_strict`. It selects MD013 into the tier the path already
>   runs; it does not change the tier, and it does not let a repository
>   configure MD013's options — prim sets those itself, and one of them varies
>   by tier. The limit is the same `max_line_length` the formatter wraps to, so
>   enabling the key cannot make prim report a line prim itself produced, except
>   a heading the formatter was never able to wrap.

## 4. Rule-tier lists — MD013 moves

In **Off in both tiers**, remove `MD013` from the leading list and add a
sentence:

> - **Off in both tiers:** MD014, MD043, MD044, MD054, MD061, MD063, MD069,
>   MD072 (frontmatter key sorting stays off because prim must remain
>   semantics-preserving), MD074, MD078, MD079, MD081, MD057 (dropped — see
>   AD-0013), and MD082 (dropped entirely — see AD-0012). MD013 is off in both
>   tiers unless `prim_mdlint_report_line_length = true` selects it — see "Line
>   length" above.

Add MD013 to the sentence describing rule options prim sets for itself, which
currently names only MD025:

> - **Rule options prim sets for itself:** prim configures MD025's
>   `front-matter-title` option to an empty string, so a page's front-matter
>   `title:` is treated as metadata rather than a heading. When
>   `prim_mdlint_report_line_length = true`, prim also configures MD013's five
>   options (see "Line length" above), one of which varies by tier. Both are
>   prim's own canonical defaults for rules it runs, not a configuration surface
>   a repository can reach — see AD-0012.

## Wording notes

- The explanation leads with what prim does, not with MD013. Readers arriving
  from markdownlint search for the rule code and find it in the reference table;
  readers meeting the feature for the first time should not have to hold a rule
  id, five options and a tier split at once.
- "prim only reports lines you can do something about" is the sentence to keep
  if the section is ever shortened. Every option in the table follows from it.
- The `docs/decisions/` caveat is stated as advice, not as a warning about a
  defect. Long conventional titles are a legitimate style; the floor tier is the
  supported answer.
