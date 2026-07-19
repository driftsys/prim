# AD-0008 — Diagnostic span → `line:col` mapping

> **Spike #42 output** (de-risking; unblocks stories B1 #44 and D2 #49).
> Approved by the SPIKES driver: enrich `FormatError` into a structured
> diagnostic under story **B1**; this spike only documents the strategy and
> ships the shared mapper primitive with a proof.

## Context

B1 (`prim lint` diagnostics) and D2 (SARIF/JSON machine output) require a stable
diagnostic code plus a precise **`file:line:col`** for every finding. The spike
brief assumed `serde` would be in the parse path and therefore drop source
positions — steering toward either a span-preserving parser swap (marked-yaml,
"JSON-with-spans", …) or a **JSON-pointer → line** fallback that re-walks the
document to recover a location.

That premise does not hold for prim. prim's structured formatters are **not**
serde round-trips; each is a comment/CST-preserving parser that already reports
a byte offset (or line:col) on a parse error:

| Format     | Parser (crate)                    | Error carries                                                                                 |
| ---------- | --------------------------------- | --------------------------------------------------------------------------------------------- |
| JSON/JSONC | `jsonc-parser`                    | `ParseError::line_display`/`column_display` (1-indexed) **and** `range().start` (byte offset) |
| TOML       | `taplo`                           | `parser::Error::range: TextRange` (byte offsets)                                              |
| YAML       | `yaml_parser` (via `pretty_yaml`) | `SyntaxError::offset()` (byte offset)                                                         |
| Markdown   | `rumdl` (lint, Spike #39)         | `LintWarning.line`/`.column` (1-indexed)                                                      |
| Hygiene    | prim's own pass                   | line known by construction (per-line scan)                                                    |

So every diagnostic source already knows _where_ — the only missing primitive is
a uniform byte-offset → `line:col` conversion for the two parsers (TOML, YAML)
that report a raw offset.

## Options

**A. Span-preserving parser swap (marked-yaml / JSON-with-spans).** Rejected:
the parsers already in the stack (`taplo`, `jsonc-parser`, `yaml_parser`)
preserve spans. Swapping to add span support would _regress_ formatting fidelity
(these were chosen precisely for comment/CST preservation in AD-0003/0004/0005)
and add dependencies for a capability we already have.

**B. JSON-pointer → line fallback.** Rejected: this only exists to recover a
location that serde discarded. prim never discards it, so the fallback is dead
weight — and it is approximate (re-walking to a key, not the actual error
token).

**C. Span-preserving parsers already in the stack + one shared mapper
(chosen).** Add a single pure function,
`prim_fmt::line_col(source, byte_offset) ->
(line, col)`, and feed it each
parser's offset. JSON/JSONC and rumdl already emit 1-indexed line:col, so the
mapper is used to _cross-check_ them and to _derive_ line:col for TOML and YAML.

## Decision: shared `line_col` mapper; enrich `FormatError` in B1

1. **`prim_fmt::line_col(source, byte_offset) -> (usize, usize)`** — 1-indexed,
   columns count Unicode scalar values (chars) from the line start, EOF- and
   char-boundary-safe. Lives in `crates/prim-fmt/src/position.rs`. This is the
   whole primitive.

2. **No parser swap, no JSON-pointer fallback.** Each format maps its native
   error position through `line_col` (TOML `range.start()`, YAML `offset()`);
   JSON/JSONC and rumdl already provide line:col directly.

3. **`FormatError` enrichment is B1's work, not this spike's.** Today
   `FormatError::Parse(String)` is stringly-typed. B1 will replace it with a
   structured diagnostic carrying `{ format, code, line, col, message }` (and
   the byte offset for SARIF regions), built via `line_col`. The spike ships the
   mapper + proof only; it does **not** refactor the error type, to avoid
   pre-committing the public error contract before B1 designs the code taxonomy.

### Proof

`crates/prim-fmt/src/position.rs` includes a `#[cfg(test)]` proof that drives
each real parser on a genuinely invalid document and asserts a concrete
`line:col`:

- **JSON** — unexpected `@` token → `line 2`, and `line_col` agrees with
  `jsonc-parser`'s own `line_display`/`column_display`.
- **TOML** — bare `=` with no value → `line 1`, via `taplo` `range.start()`.
- **YAML** — unclosed flow sequence → `line ≥ 1`, via `pretty_yaml`
  `SyntaxError::offset()`.

## Consequences

- **B1/D2 are de-risked with no AC change.** B1 already promises "stable code +
  file:line:col"; this spike confirms it is achievable on the existing parser
  stack, so B1's scope is unchanged — it now has a proven mapper and a
  documented `FormatError` enrichment plan instead of an open parser-selection
  question.
- **No new dependencies.** `taplo`, `jsonc-parser`, and
  `pretty_yaml`/`yaml_parser` are already linked. (`jsonc-parser` is presently a
  `prim-fmt` **dev**-dependency; if B1 needs structured JSON positions outside
  tests it graduates to a normal dependency — a one-line change, not a parser
  swap.)
- **Two candidate directions in the spike brief are formally closed:**
  marked-yaml/JSON-with-spans (unnecessary) and the JSON-pointer fallback
  (unnecessary and approximate).

---

Satisfies: Spike #42 (mapping strategy + a spike that reports a real line:col).\
Unblocks: B1 #44 (diagnostics `file:line:col`), D2 #49 (SARIF/JSON locations).\
Related: AD-0003 (JSON via dprint/jsonc-parser), AD-0004 (TOML via taplo),
AD-0005 (YAML via pretty_yaml), Spike #39 (rumdl lint line:col),
`crates/prim-fmt/src/position.rs`.
