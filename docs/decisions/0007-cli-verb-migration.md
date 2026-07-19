# AD-0007 — CLI verb migration: `fmt` / `lint` / `fix`

## Status

Accepted (spike #40). Implemented by story G1 (#57); supersedes the flag-driven
mode surface described in FR-5.

## Context

prim v1 exposes one command with mode **flags**: `prim [PATH]...` formats in
place, `--check` gates, `--diff` previews, `--stdin-filepath` does
format-on-save (`crates/prim-cli/src/cli.rs`, `app.rs`). v2 adds a Markdown
**content linter** (rumdl, lint-only — spike #39) and an autofix path. A
report-only linter and an in-place formatter are different operations with
different exit-code contracts and different default behaviours; overloading one
flat flag set onto both does not scale. The v2 epic (#37) reshapes the surface
into three verbs:

- `prim fmt` — format the parsed formats + whitespace hygiene (today's default).
- `prim lint` — **report only**, never rewrites: hygiene violations + a curated
  set of Markdown content rules.
- `prim fix` — `fmt` plus the autofixable content rules (mostly MD034).

The design questions this ADR settles: how bare `prim <files>` and the existing
flags map onto verbs, the deprecation path, and the per-verb exit-code contract.

## Decision

### 1. Verbs, with bare `prim` as a permanent `fmt` alias

`fmt`, `lint`, and `fix` are subcommands. **Bare `prim [PATH]...` is a permanent
alias for `prim fmt [PATH]...`** — not deprecated. prim's identity is
"near-zero-config, just run it"; requiring a verb for the overwhelmingly common
format action would hurt adoption and the folio-delegation story (F1). `prim`
with no args continues to format the current directory.

Implementation: because clap cannot cleanly disambiguate an optional subcommand
from a leading positional `PATH`, dispatch is resolved by a thin **argv
preprocessor** before `Cli::parse`: if the first non-flag argument is a known
subcommand (`fmt`, `lint`, `fix`, plus future `explain`, `init`, and the
`completions` path) or a global help/version flag, parse as-is; otherwise inject
an implicit `fmt`. This keeps `prim README.md`, `prim fmt README.md`, and
`prim --check` all working, and keeps the clap model a normal
`#[command(subcommand)]` enum.

Non-owned edge case: a file literally named `fmt`/`lint`/`fix` in the current
directory is shadowed by the verb. This is acceptable (documented);
`prim fmt
fmt` disambiguates.

### 2. Flag → verb mapping

| v1 surface                   | v2 home                                              |
| ---------------------------- | ---------------------------------------------------- |
| `prim [PATH]...`             | `prim fmt [PATH]...` (bare alias, permanent).        |
| `prim --check`               | `prim fmt --check` (format-drift gate).              |
| `prim --diff`                | `prim fmt --diff` (and `prim fix --diff`).           |
| `prim --stdin-filepath <p>`  | `prim fmt --stdin-filepath <p>` (also `fix`/`lint`). |
| `prim --exclude`, `--color`  | global flags, valid on every verb.                   |
| `prim --completions <shell>` | unchanged global (also offer `prim completions`).    |

`--check` and `--diff` are **format-drift** concepts and stay on `fmt`/`fix`.
`prim lint` is inherently report-only, so it needs neither. This preserves the
B1 distinction: `prim fmt --check` (would the _formatter_ change bytes?) is
separate from `prim lint` (are there _content_ violations?).

### 3. Deprecation path for top-level flags

The top-level flags `--check`, `--diff`, and `--stdin-filepath` remain accepted
directly on `prim` (implying `fmt`) as **deprecated sugar**:

- v1.x (introduction): they work and emit a one-line deprecation warning to
  **stderr** the first time one is seen in a run — e.g.
  `warning: 'prim --check'
  is deprecated; use 'prim fmt --check' (removed in v2.0)`.
  Behaviour is unchanged; stdout (the machine channel) is untouched, so CI gates
  keep working.
- v2.0 (removal): the top-level flags are removed; the verb forms are the only
  spelling. Bare `prim <paths>` = `fmt` survives (it is not deprecated).

Rationale: gives downstream CI and editor integrations a full pre-1.0→2.0 window
to migrate without a flag-day break, while keeping the long-term surface clean.

### 4. Exit-code contract, per verb

The `0` / `1` / `2` contract is kept but given a per-verb meaning. **`1` means
"actionable findings"; `2` means "prim could not do its job." Warnings never
raise the exit code — only errors do** (the G1 "errors only" rule).

| Verb / mode          | `0`                     | `1`                                    | `2`                      |
| -------------------- | ----------------------- | -------------------------------------- | ------------------------ |
| `fmt` (write)        | done (or nothing to do) | — (writing is not a "finding")         | parse / IO / usage error |
| `fmt --check`        | already formatted       | at least one file would change (drift) | parse / IO / usage error |
| `fmt --diff`         | always (preview only)   | —                                      | parse / IO / usage error |
| `lint`               | no error-severity finds | ≥1 **error-severity** diagnostic       | IO / usage error         |
| `fix` (write)        | applied; nothing broken | —                                      | parse / IO / usage error |
| `fix --check/--diff` | nothing to fix          | at least one fixable finding pending   | parse / IO / usage error |

Notes:

- **`lint` warnings do not fail.** Warning-severity content findings are printed
  but exit `0`; only error-severity findings exit `1`. Severity is assigned by
  the G3 matrix (`prim_mdlint_strict` may escalate warn→error).
- **`fix` does not fail on residual errors.** `fix` applies what it can and
  exits `0`; a non-autofixable error-severity finding does not raise the code —
  run `prim lint` for the gate. This keeps `fix` safe to run in a pre-commit
  hook.
- A malformed `--exclude` glob remains a usage error (exit `2`, FR-4.5); an
  explicitly named missing path remains exit `2`, an unowned named path a
  warning + exit `0` (FR-4.6) — unchanged, now per verb.

## Consequences

- **`cli.rs` gains a subcommand enum** and per-verb arg groups; `main` grows the
  argv preprocessor; `app::run` dispatches on the verb. Exit-code constants in
  `app.rs` stay but their emission moves behind the verb handlers.
- **FR-5 in `docs/SPEC.md` is rewritten** by G1: FR-5.1–5.5 describe the flag
  surface; they become the verb surface with the exit-code table above. FR-5.5's
  "`1` = changes needed (`--check`)" generalises to the per-verb meaning.
- **`spec/` trycmd snapshots** covering `--help`, `--check`, and `--diff` output
  must be regenerated for the verb forms, plus new snapshots for the deprecation
  warnings.
- **Docs** (`USAGE.md`, `getting-started.md`, `recipes.md`, completions) update
  to lead with verbs while documenting the bare alias and the deprecation
  window.
- **G1 scope** now explicitly includes: the argv preprocessor + bare alias, the
  deprecated-flag shim with stderr warning, and the per-verb exit-code table.
  B1's CLI shape is subsumed by G1 (as the epic states).
- The engine (`prim-fmt`) is **unaffected** — verbs are a `prim-cli` concern;
  `format` and `lint_markdown` are the two engine entry points the verbs call.

## Alternatives considered

- **Require explicit verbs (clean break, cargo-style).** Rejected: breaks every
  existing `prim`/`prim --check` invocation and the folio integration on day
  one, for little gain given the argv preprocessor is cheap.
- **Keep top-level flags permanently as fmt sugar (no deprecation).** Rejected:
  leaves two spellings of every mode forever, growing the arg surface prim's
  "one canonical way" philosophy tries to avoid. A bounded deprecation window is
  the compromise.
- **`lint`/`fix` as `--lint`/`--fix` flags rather than verbs.** Rejected: they
  have distinct defaults and exit-code contracts; verbs make that explicit and
  give each its own `--help`.

---

Satisfies: Epic #37, story G1 (#57); reshapes FR-5. Related: AD-0001 (crate
boundary — verbs stay in `prim-cli`), spike #39 (rumdl lint-only), spike #42
(diagnostic line:col for `lint`), `crates/prim-cli/src/cli.rs`,
`crates/prim-cli/src/app.rs`.
