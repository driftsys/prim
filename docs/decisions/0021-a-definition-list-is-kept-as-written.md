# AD-0021 — A definition list is kept as written

## Status

Accepted. Closes #229. Breaking by the AGENTS.md rule: prim's canonical output
changes for a file that holds a definition list. A file that used to be
rewrapped into one paragraph is now left unchanged.

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
whose first line is not itself a `:` line, and that holds a later line starting
with `:`, is replaced by a one-word sentinel paragraph before dprint runs. The
original lines come back afterwards, re-indented to the container prefix dprint
gave the sentinel line. Fenced code and indented code are never inspected. The
sentinel carries a nonce, as the fence-language guard does, so it cannot match
text the author wrote.

## Consequences

- Definition text is not wrapped to the line width. The author's line breaks
  stay, which matches how prim already treats an HTML comment that ends a line.
- A paragraph that starts with a list marker or a quote marker and holds a `:`
  line is kept as written as a whole.
- A `:` line after a blank line is a separate paragraph and is wrapped as prose.
- The result is stable under repeated formatting.
