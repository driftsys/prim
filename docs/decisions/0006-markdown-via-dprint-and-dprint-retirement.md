# AD-0006 — Markdown via `dprint-plugin-markdown`, and retiring dprint

## Context

FR-1.1/1.1a/1.6 require canonical Markdown with hard-wrapped prose, guardrails
(never break inline code, split links, or wrap tables/fenced code; preserve hard
breaks), and verbatim fenced code. This is the last per-format pass. The repo
already formatted its Markdown with **dprint** (the `markdown` wasm plugin,
`lineWidth 80`, `textWrap always`), gated by a required CI job — so prim taking
over Markdown overlaps and, per the issue, should replace it.

## Decision: `dprint-plugin-markdown` as a library

`dprint-plugin-markdown = "0.22"` — the same engine the repo already used, now a
Rust dependency of `prim-fmt`, in a `markdown` module. `prim_fmt::format`
dispatches `FileKind::Markdown` to it.

- `format_text(text, &Configuration, code_block_cb) -> anyhow::Result<Option<String>>`.
- Config from `Style`: `line_width = max_line_length.unwrap_or(80)`,
  `text_wrap = TextWrap::Always` (FR-1.1 hard wrap). EOL/final newline stay with
  `hygiene`.
- **FR-1.6 via the callback:** `format_code_block_text` returns `Ok(None)`, so
  dprint never reformats embedded code — fenced blocks pass through verbatim.
- dprint's defaults give FR-1.1 canonical output (ATX headings, dash list
  markers, padded tables, normalized blank lines) and its wrapper honors the
  FR-1.1a guardrails (inline code atomic, links not split, tables/code not
  wrapped, hard breaks preserved).
- Markdown is effectively infallible (CommonMark accepts any input), so the
  `FormatError::Parse` arm is defensive and unreachable in practice.

Because prim uses the same engine and config as the repo's dprint setup, prim's
output **matches the existing Markdown byte-for-byte** — the migration produced
zero reformatting churn.

## Decision: disable `dprint-core` debug assertions in the dev profile

`dprint-core`'s printer carries a `debug_assert` that panics on valid Markdown
containing an **inline code span with an embedded newline** (e.g. a long
backticked span that a previous wrap split across two source lines). Release
builds — and the dprint wasm plugins — compile the assertion out, which is why
dprint itself never crashed. prim's dev/test builds hit it.

A targeted profile override in the workspace `Cargo.toml` disables debug
assertions for the `dprint-core` package only:

```toml
[profile.dev.package.dprint-core]
debug-assertions = false
```

prim's own assertions are unaffected; only this dependency's over-aggressive
debug check is silenced, so prim is robust on such input in every build. A
regression test
(`markdown::tests::inline_code_spanning_a_newline_does_not_panic`) pins the
behaviour.

## Decision: retire dprint

dprint existed in this repo solely to format Markdown. With prim owning it:

- `dprint.json` is deleted.
- `justfile` `fmt`/`lint` call `prim` instead of `dprint fmt`/`dprint check`.
- The CI `Dprint` job is replaced by a `prim self-check` job
  (`cargo run -p prim-cli -- --check .`); the gate's `needs` is updated.
- `markdownlint` stays as an independent lint (it checks content rules prim does
  not). prim and markdownlint agree on the repo's Markdown.

prim now formats all of its own connective tissue — its stated purpose.

## Consequences

`dprint-plugin-markdown` (and the shared `dprint-core`/`jsonc-parser` stack from
AD-0003) are `prim-fmt` dependencies. With Markdown done, **all per-format
passes (FR-1.1–1.6) are implemented**; Milestone 3 is complete. The
`.md`-as-hygiene- vehicle behavioural tests were retargeted to `.txt` orphans,
completing the migration of those tests off owned-but-now-structured file types.

---

Satisfies: FR-1.1 (Markdown canonical + prose wrap), FR-1.1a (wrap guardrails),
FR-1.6 (fenced code verbatim), FR-3.4/6.2 (no reorder / data unchanged).\
Related: AD-0003 (JSON via dprint-plugin-json; the fallible `format` API and the
shared dprint-core stack), `docs/design/system.md`,
`crates/prim-fmt/src/markdown.rs`.
