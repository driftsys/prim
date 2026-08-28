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

`dprint-core`'s printer and `dprint-plugin-markdown`'s `is_list_word` predicate
each carry a `debug_assert` that panics on Markdown a formatter must nonetheless
accept:

- `dprint-core`: an **inline code span with an embedded newline** (e.g. a long
  backticked span that a previous wrap split across two source lines).
- `dprint-plugin-markdown`'s `is_list_word` (`utils.rs`): **whitespace inside a
  word**. The predicate asserts that the word handed to it contains no
  whitespace other than U+00A0. Words are split on ASCII space and line feed
  only, so any word **after the first** that contains any other
  `char::is_whitespace()` character trips it — U+0009 tab, U+000B, U+000C,
  U+0085, U+2028, U+2029, and the space separators U+1680, U+2000-U+200A,
  U+202F, U+205F and U+3000. The first word of a text run is exempt, because
  `flush_current_word` skips the check while the item list is still empty.
  U+200B is not `char::is_whitespace()`, so it never trips the assertion.

The tab case is the one most likely to appear in real documentation, and it is
not a space separator at all — describing the trigger as "a Unicode space
separator after an ASCII space" would send the next reader looking for a
different bug.

Release builds — and the dprint wasm plugins — compile both assertions out,
which is why dprint itself never crashed. prim's dev/test builds hit them, and
in a directory walk the panic (exit 101) aborted the whole run, so healthy files
next to the one triggering file produced no output either.

Cargo cannot disable a single assertion inside a dependency, so a targeted
profile override in the workspace `Cargo.toml` disables debug assertions for
each affected package instead:

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
- `crates/prim-fmt/src/markdown.rs::whitespace_inside_a_word_does_not_panic`
  covers the whole triggering character set plus the U+00A0 exemption;
  `crates/prim-cli/tests/markdown.rs::whitespace_inside_a_word_does_not_panic`,
  `crates/prim-cli/tests/discovery.rs::a_walk_reports_neighbours_of_a_file_that_used_to_panic`,
  `crates/prim-cli/tests/modes.rs::stdin_filepath_handles_whitespace_inside_a_word`
  and
  `crates/prim-cli/tests/lsp.rs::formats_markdown_holding_whitespace_inside_a_word`
  pin the four dispatch paths (`dprint-plugin-markdown`). Each of the four
  reaches `prim_fmt::format` on its own, so one representative character is
  enough per path; the character set itself is covered once, in `prim-fmt`.

### Alternatives considered

- **`[profile.dev.package."*"]`.** Cargo applies `"*"` to every package that is
  not a workspace member, which would silence dependency assertions in one line
  and end the per-package additions. Rejected for now as broader than the
  evidence justifies: it would also silence assertions in dependencies that have
  never misfired, including ones prim might want to hear from.
  `dprint-plugin-json` carries three live position assertions that no reported
  input has yet reached, so the choice is not yet forced.
- **A panic barrier.** prim formats through a rayon pool, so one panicking
  worker takes the whole process to exit `101`, which is outside prim's
  documented `0`/`1`/`2` contract. Catching the unwind around the per-file
  `prim_fmt::format` call and mapping a panic to exit `2` would honour the
  contract for every future dependency panic rather than one at a time. Rejected
  here as a larger change than a targeted fix warrants; tracked separately.

Each override should be removed once its upstream release fixes the assertion it
silences. Neither has an upstream issue filed, so the trigger for revisiting
them is a `dprint-*` dependency bump: drop the override, run the regression
tests above, and keep the override only if they still fail.

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
