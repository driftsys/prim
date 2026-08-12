# AD-0010 — Arrays keep the layout the source wrote

## Status

Accepted. Changes prim's canonical TOML output: an array written across several
lines is no longer collapsed onto one, even when it would fit.

## Context

prim collapsed any array that fitted inside the resolved width. That rule loses
to the tools that own the files prim formats.

`uv add` writes `pyproject.toml`'s dependency list one entry per line, and does
so unconditionally — a single short dependency still gets its own line:

```toml
dependencies = [
    "idna>=3.18",
]
```

prim collapsed it, `uv add` re-expanded it on the next edit, and
`prim fmt --check` failed after every dependency change. A pre-commit hook
running prim produced a diff each time. Neither tool converges, because both are
asserting a layout rule on every write and only one of them is also the file's
owner.

The same shape appears wherever a generator or package manager re-asserts a
one-per-line list. prim is the only participant that has no claim on the file.

## Options

**A. Keep collapsing, exempt `pyproject.toml` through an ecosystem profile.**
Rejected for now: it needs a per-file style mechanism that does not exist, and
it fixes one file rather than the class. Worth revisiting only if some ecosystem
demands the opposite rule, which none observed so far does.

**B. Stop collapsing everywhere.** Chosen. `array_auto_collapse = false`, with
`array_auto_expand` left on so an over-width array still breaks one element per
line (FR-1.5, #96).

**C. Collapse only when the array fits and no owning tool is known.** Rejected:
requires the same per-file knowledge as A plus a heuristic about ownership, and
produces output that cannot be predicted from the file alone.

## Decision

prim expands an array that overflows the resolved line width, and otherwise
leaves the array's line structure as the author wrote it. An array on one line
stays on one line; an array across several lines stays across several lines.

Verified that this converges rather than moving the fight:

- prim normalises the file once — the expansion is kept, the indent becomes
  prim's resolved style.
- `uv` preserves an existing indent, so it accepts prim's normalisation.
- `prim fmt --check` then exits `0` after each subsequent `uv add`, checked
  across three consecutive adds.
- prim's own repository is unchanged (`prim fmt --check .` exits `0`).
- `cargo add` was already compatible and stays so: a fitting feature list
  written inline stays inline.

## Consequences

- prim no longer enforces a single canonical layout for arrays. Two files
  holding the same data can differ in line structure, and prim will preserve
  both. This is a real loss of canonicality, accepted because the alternative is
  a rule prim cannot actually enforce against the file's owner.
- Formatting is still deterministic and idempotent: the output depends on the
  input's line structure, which is itself stable under formatting.
- An over-width array still expands, so #96 is unaffected.
- The Rust Style Guide's "put the entire list on one line if it fits" is no
  longer applied by prim to an already-expanded list. prim does not contradict
  the guide — it declines to reformat toward it — and `cargo` does not
  re-collapse either.

## Alternatives considered

- **Collapse only arrays below some element count.** Rejected: an arbitrary
  threshold that still fights `uv` for short lists, which is the failing case.
- **Leave it and document the conflict.** Rejected: the failure is silent and
  recurring, and it lands in exactly the pre-commit hooks `docs/recipes.md`
  recommends setting up.

---

Satisfies: #105; refines FR-1.5. Related: AD-0004 (TOML via taplo), #96
(over-width arrays expand), `crates/prim-fmt/src/toml.rs`.
