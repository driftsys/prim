# AD-0021 — A definition list is kept as written

## Status

Accepted. Closes #229. Breaking by the AGENTS.md rule: prim's canonical output
changes for a file that holds a definition list: the definition-list paragraph
used to be rewrapped into one paragraph and is now kept as written. The rest of
the file is formatted as before.

## Context

`markspec fmt` keeps a definition list in an entry body
(driftsys/markspec#1010):

```markdown
Term
: first line
second line.
```

CommonMark has no definition lists, so `dprint-plugin-markdown` parses that text
as one paragraph and rewraps it to `Term : first line second line.`. The `:`
marker is lost, and a file that `markspec fmt` formatted fails
`prim fmt --check`.

## Options

1. **Rewrap after dprint, then split at each `:` marker.** Keeps the wrap width.
   Rejected: the split has to guess where a marker was, and a term that contains
   `:`, or a rewrap that puts `:` at a line start by chance, gives a wrong
   result.
2. **Guard the paragraph before dprint runs, and restore it afterwards.** The
   lines are never rewrapped.
3. **Add definition-list support to dprint.** Out of reach for prim: it is an
   upstream change, and the plugin follows CommonMark.

## Decision

Option 2, in `crates/prim-fmt/src/markdown/definition_list.rs`. A paragraph
whose first line does not itself start with a colon and a space, and that holds
a later line starting with a colon and a space, is replaced by a one-word
sentinel paragraph before dprint runs. The original lines come back afterwards,
re-indented to the container prefix dprint gave the sentinel line. A two-space
hard break inside the paragraph is written back as a trailing backslash, as
dprint does for prose, because whitespace hygiene would strip the spaces.

The paragraph ends where CommonMark ends one: at a blank line, a fence, an ATX
heading, a thematic break, a list item (a bullet, or an ordered item numbered
1), or a change of blockquote depth. Fenced code and indented code are never
inspected; a line indented four columns past the innermost list item is code.
The sentinel carries a nonce, as the fence-language guard does, so it cannot
match text the author wrote.

## Consequences

- Definition text is not wrapped to the line width. The author's line breaks
  stay, which matches how prim already treats an HTML comment that ends a line.
- A definition line longer than the width is therefore not shortened by
  `prim fmt`. With `prim_mdlint_report_line_length` on, `prim lint` reports
  MD013 on it, so a repository that formats with prim can still see that finding
  there. This is the second exception beside headings.
- A paragraph that starts with a list marker or a quote marker and holds a
  definition line is kept as written as a whole.
- A definition line after a blank line is a separate paragraph and is wrapped as
  prose.
- The result is stable under repeated formatting.
