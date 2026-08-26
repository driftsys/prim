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

## Decision: disable dependency debug assertions in the dev profile

`dprint-core`'s printer and `dprint-plugin-markdown`'s tokenizer each carry a
`debug_assert` that panics on Markdown a formatter must nonetheless accept:

- `dprint-core`: an **inline code span with an embedded newline** (e.g. a long
  backticked span that a previous wrap split across two source lines).
- `dprint-plugin-markdown`'s `is_list_word` tokenizer (`utils.rs`): a **Unicode
  space separator** (U+1680, U+2000-U+200A, U+202F, U+205F, U+3000; U+00A0 is
  exempt) immediately following an ASCII space.

Release builds — and the dprint wasm plugins — compile both assertions out,
which is why dprint itself never crashed. prim's dev/test builds hit them, and
in a directory walk the panic (exit 101) aborted the whole run, so healthy files
next to the one triggering file produced no output either.

Cargo has no mechanism to disable a single assertion inside a dependency; the
only alternatives are forking or patching the dependency, or waiting for a fixed
upstream release. A targeted profile override in the workspace `Cargo.toml`
disables debug assertions for each affected package instead:

```toml
[profile.dev.package.dprint-core]
debug-assertions = false

[profile.dev.package.dprint-plugin-markdown]
debug-assertions = false
```

prim's own assertions are unaffected; only these dependencies' over-aggressive
debug checks are silenced, so prim is robust on such input in every build.
Regression tests pin both:

- `crates/prim-fmt/src/markdown.rs::inline_code_spanning_a_newline_does_not_panic`
  (`dprint-core`).
- `crates/prim-cli/tests/markdown.rs::unicode_space_after_ascii_space_does_not_panic`,
  `crates/prim-cli/tests/discovery.rs::a_directory_walk_with_a_unicode_space_fixture_does_not_panic`,
  and
  `crates/prim-cli/tests/modes.rs::stdin_filepath_handles_a_unicode_space_after_an_ascii_space`
  (`dprint-plugin-markdown`).

Each override should be removed once its upstream release fixes the assertion it
silences.

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
