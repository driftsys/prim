# Debt remediation — design

Date: 2026-07-04. Status: approved; plan written (PR 5 added by user decision
after the FR-1.6 bug was found during planning).

## Context

A post-v1 design review found no architectural rework needed — scope,
architecture, safety model, and CLI contract are sound — but identified contract
and documentation debt in seven areas:

1. No style-stability contract: the canonical style is delegated to third-party
   formatters (`dprint-plugin-json`/`-markdown`, `taplo`, `pretty_yaml`); a
   dependency bump can silently change output and break every downstream
   `prim --check` CI gate.
2. Silent exit 0 for explicitly named paths that do not exist or that prim does
   not own (`crates/prim-cli/src/app.rs`).
3. `--exclude` glob errors are silently swallowed
   (`crates/prim-cli/src/discover.rs`).
4. Documentation contradicts the implementation: stale README status, stale
   module docs in `lib.rs`/`classify.rs`, `NO_COLOR` promised but not
   implemented, `--color auto` keyed to stdout while human output goes to
   stderr, SPEC FR-3.2 lists `charset` while AD-0002 excludes it.
5. Decisions missing from the user-facing spec: lenient JSON/JSONC handling
   (recorded in AD-0003 but absent from SPEC/USAGE), `--diff` exit code, FR-2.1
   vs `trim_trailing_whitespace = false` precedence.
6. Allowlist judgment calls: `.env` is the only orphan entry holding data
   values; `CODEOWNERS` and `.mailmap` are missing. (Eleven landed specs/plans
   also linger in `docs/wip/`; the directory is gitignored — private
   working-memory mode — so this is local clutter, not repo debt.)
7. FR-1.6 violation, found while writing the plan: prim reformats the contents
   of fenced code blocks tagged `markdown`/`md`. `dprint-plugin-markdown`
   recurses into those tags (`Context::format_text`, `gen_types.rs`) before
   consulting the code-block callback that protects foreign-language fences.
   Reproduced: a long line inside a `markdown`-tagged fence is rewrapped while
   the same line in a `js`-tagged fence stays verbatim.

## Goals

- Make the CI-gate stability promise an enforced contract, not an accident of
  dependency versions.
- Remove the silent-failure paths from the CLI contract.
- Bring every document back in line with the implemented reality.
- Resolve and record the open design decisions.

## Non-goals

- NFR-4 parallelization and benchmarks (covered by the in-flight
  `2026-07-02-format-benchmarks-plan.md`).
- Per-directory `Style` cache (deferred, AD-0002).
- Colorized `--diff` output (deferred).

## Decisions

| Fork                       | Decision                                         | Rationale                                                                                                                          |
| -------------------------- | ------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| Delivery                   | Themed PR series (5 PRs) + local cleanup         | Matches single-focus PR culture; each self-contained and revertable.                                                               |
| FR-1.6 fence bug           | Fix in `prim-fmt` via a fence-language guard     | dprint recurses into `markdown`-tagged fences before the callback can veto; the callback path is proven verbatim for foreign tags. |
| `.env` / `.env.*`          | Drop from orphan allowlist                       | Only entry holding data values; hygiene can corrupt multi-line values.                                                             |
| Explicit path, nonexistent | Error, exit 2                                    | Naming a missing file is a user error; must not pass silently.                                                                     |
| Explicit path, unowned     | Warning on stderr, exit 0                        | FR-2.4 intact; `prim *` over mixed directories keeps working.                                                                      |
| `--diff` exit code         | Always 0; record in FR-5.3                       | `--check` stays the single CI gate; `--diff` is a preview.                                                                         |
| `Json`/`Jsonc` `FileKind`  | Keep both variants; document leniency in AD-0003 | Smallest diff; preserves a future strict-mode option.                                                                              |
| Style-stability policy     | Output change = minor version + CHANGELOG entry  | Pre-1.0-compatible; makes churn visible and deliberate.                                                                            |

## Work packages

### PR 1 — docs truth-sync (docs only, no behavior change)

- README status block: v1 complete (mirror AGENTS.md).
- `crates/prim-fmt/src/lib.rs` and `classify.rs` module docs: remove "later
  milestones" phrasing; describe the implemented dispatch.
- SPEC.md: drop `charset` from FR-3.2; add FR-2.1 precedence note ("unless
  `.editorconfig` sets `trim_trailing_whitespace = false`"); record in FR-5.3
  that `--diff` always exits 0.
- JSON leniency at the user surface: AD-0003 already records the decision, so no
  addendum — add a note to SPEC FR-1.3 and a USAGE format note (`.json` parsed
  as JSONC; comments and trailing commas accepted, never emitted).
- recipes.md: `.primignore` recipe for golden/fixture files (cite the repo's own
  correctness-fixtures entry as the example); note that `--exclude` never
  applies to explicitly named paths.
- .gitignore and .markdownlintignore: add `docs/archive/` alongside `docs/wip/`
  so locally archived working memory stays out of `git status` and lint.
- Acceptance: `just verify` green; prim formats its own docs cleanly.

### PR 2 — style-stability contract

- Committed corpus at `crates/prim-fmt/tests/corpus/`: input/expected pairs per
  format covering JSONC comments, YAML anchors/aliases and multi-line scalars,
  TOML inline tables and comments, Markdown hard breaks, long links, and front
  matter.
- Test: format each input under `Style::default()`, byte-compare with the
  committed expected output; assert idempotency on expected outputs.
- SPEC.md "Style stability" section: any canonical-output change is a minor
  version bump, listed in the CHANGELOG under "Formatting changes".
- .primignore and .markdownlintignore: exclude the whole corpus directory —
  inputs are deliberately non-canonical, and expected outputs must change only
  via the bless workflow (`just lint` runs both tools repo-wide).
- Acceptance: mutating one expected file locally makes the suite fail with a
  message pointing at the policy.

### PR 3 — CLI contract hardening + color

- `app.rs`: explicit nonexistent path → error, exit 2; explicit unowned path →
  stderr warning, exit 0. Walked unowned files stay silent.
- `discover.rs`: `collect` returns `Result`; a malformed `--exclude` glob is a
  usage error (exit 2) naming the offending glob.
- `cli.rs`: `--stdin-filepath` gains `conflicts_with_all(["check", "diff"])`.
- `main.rs`: honor `NO_COLOR`; key `--color auto` off stderr being a TTY.
- SPEC.md: FR-4.5 gains "invalid glob = usage error"; FR-5 notes stdin-mode
  exclusivity.
- Acceptance: behaviour tests in `crates/prim-cli/tests/` plus trycmd snapshots
  for each new error message; `just verify` green.

### PR 4 — allowlist curation (after PR 1)

- `classify.rs`: remove `.env` and `.env.*`; add `CODEOWNERS` and `.mailmap`;
  tests pin both directions.
- USAGE.md: the full orphan allowlist becomes the canonical user-facing
  reference; the code comment points there.
- Acceptance: `prim .env` leaves the file untouched (unowned-path warning from
  PR 3); `CODEOWNERS` receives hygiene.

### PR 5 — FR-1.6: markdown-tagged fences stay verbatim (after PR 2)

- `crates/prim-fmt/src/markdown.rs`: before dprint runs, swap the fence language
  of blocks tagged `markdown`/`md` for a sentinel tag; restore it after. dprint
  treats the sentinel as a foreign language and preserves the block verbatim.
  Every rewrite is reversed, so a false positive inside verbatim content
  round-trips unchanged.
- Unit tests: fence content and tag byte-identical; `md`, tilde-fence, and
  blockquote variants; no sentinel ever leaks into output.
- Corpus: `sample.md` gains a `markdown`-tagged fence; re-bless and inspect —
  canonical output changes deliberately, which is exactly the policy's
  minor-bump path.
- Acceptance: the reproduction case no longer changes under prim; the stability
  suite is green.

### Local cleanup — gardening (no PR)

- `docs/wip/` is gitignored (private working-memory mode), so there is no
  gardening debt on `main`. Locally: move the eleven landed files (eight specs,
  three plans) to `docs/archive/` (kept out of `git status` by the PR 1
  `.gitignore` entry); keep the two in-flight plans (benchmarks,
  spec-test-harness).
- Durable records for the landed work already exist (`docs/design/system.md`,
  AD-0001…AD-0006), so no record-writing is needed.

## Alternatives considered

- **One cleanup mega-PR** — fastest wall-clock, rejected: mixes behavior and doc
  changes into one review, against the single-focus PR rule.
- **Issues-first, top item only** — most process-correct, rejected as ceremony
  for debt this small; issues may still be filed per PR.
- **Keep `.env`, document the edge** — rejected: the risk is silent value
  corruption; documentation does not prevent it.
- **Explicit unowned path → error (prettier-style)** — rejected: breaks
  `prim *`-style invocations over mixed directories.
- **`--diff` exit 1 on pending changes** — rejected: would create a second CI
  gate and muddy the `--check` contract.
- **Collapse `Json`/`Jsonc` variants** — rejected: larger public-API diff for no
  behavior change; leniency is documented instead.

## Verification

`just verify` per PR; the correctness harness
(`crates/prim-fmt/tests/correctness.rs`) must stay green throughout; trycmd
snapshot regenerations must be deliberate, never bulk-accepted.
