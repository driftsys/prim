# prim — Rust project scaffold (PR #1) design

> Working memory for the scaffolding session (Superpowers spec). Captures the
> approved design before implementation. Per the `sdd-working-memory-lifecycle`
> rule (v0.2.0), this lives in `docs/wip/`; in prim it is gitignored (private
> mode), so it never rides along in the PR. Source-of-truth for scope is GitHub
> issues #1 (spec) and #2 (handoff).

## What prim is

A single-binary, opinionated, near-zero-config formatter for a repository's
_connective tissue_ — Markdown, JSON/JSONC, YAML, TOML — plus whitespace hygiene
on a curated orphan allowlist. Not a source-code formatter; no plugin system.
Mirrors the `driftsys/git-std` archetype for structure, install story, and docs.

## Settled decisions (this session)

| Fork                    | Decision                                                                                                                                               |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **PR #1 scope**         | Scaffold only — repo skeleton + compiling no-op `prim` CLI. No real formatting.                                                                        |
| **MarkSpec**            | None in this repo. Prose spec lives in `docs/SPEC.md` (mirrors git-std). `.markspec/` is an editor LSP artifact → gitignored.                          |
| **Crate split**         | Two crates: lib `prim-fmt` (engine, zero CLI deps) + bin `prim-cli` (`[[bin]] name = "prim"`). `cargo install prim-cli`. Both names free on crates.io. |
| **Dev/release tooling** | prim dogfoods `git-std` — ships `.git-std.toml` + `.githooks/`; `bootstrap` installs git-std. CI commit-lint uses standalone `convco`.                 |

Rationale for the lib/bin split now (not later): a single lib+bin package forces
external lib consumers to resolve the bin's CLI dependency tree (clap et al.)
unless feature-gated. A standalone `prim-fmt` lib stays lean for the "other
crates needing the lib" the maintainer anticipates. Cost is one extra
`Cargo.toml`; it is the git-std pattern (thin orchestrator bin over pure libs).

## Directory layout

```
prim/
├── Cargo.toml                 # [workspace] members = crates/prim-fmt, crates/prim-cli, spec
├── Cargo.lock
├── crates/
│   ├── prim-fmt/              # LIBRARY — engine. name="prim-fmt", lib "prim_fmt". Zero CLI deps.
│   │   ├── Cargo.toml
│   │   ├── README.md          # crates.io readme
│   │   └── src/lib.rs         # PR#1: no-op format(text)->text + core types/errors
│   └── prim-cli/              # BINARY — name="prim-cli", [[bin]] name="prim", deps prim-fmt
│       ├── Cargo.toml
│       ├── README.md
│       ├── build.rs           # clap_mangen man-page generation
│       └── src/{main.rs, app.rs, cli.rs, ui.rs}
├── spec/                      # e2e/acceptance crate (trycmd + snapbox), publish=false
│   ├── Cargo.toml
│   ├── tests/{general.rs, modes.rs}
│   ├── dump/                  # snapbox stdout/stderr fixtures
│   ├── support/mod.rs
│   └── install/install_test.sh
├── docs/                      # mdbook source (chapters below) + gitignored docs/wip/
│   ├── SUMMARY.md  README.md  getting-started.md  USAGE.md  recipes.md
│   ├── SPEC.md                # v1 requirements ported from issue #1 (prose)
│   └── wip/                   # Superpowers working memory (gitignored, private mode)
├── book.toml
├── tools/bash_unit
├── .github/workflows/{ci,docs,release}.yml   +  .github/dependabot.yml
├── .githooks/                 # driftsys + git-std managed hooks
├── .git-std.toml              # versioning config (tracks workspace + prim-cli's path dep)
├── bootstrap                  # installs git-std, runs `git std bootstrap`
├── install.sh                 # end-user installer (prebuilt `prim` binaries)
├── justfile
├── project.toml               # io.driftsys.prim descriptor
├── .editorconfig  dprint.json  .markdownlint.json  .markdownlintignore
├── .primignore                # committed; prim dogfoods itself
├── .gitignore                 # exists — add `.markspec/` and `docs/wip/`
├── AGENTS.md  CLAUDE.md  CONTRIBUTING.md  CODE_OF_CONDUCT.md
└── LICENSE  README.md         # exist (README expanded)
```

No `schemas/` (prim has no config file). No MarkSpec/`spec/*.md` requirements.

Working memory lives at `docs/wip/` per the v0.2.0 lifecycle rule. For the
scaffold PR it is gitignored (private mode) to keep PR #1 focused; flip to
tracked + garden-before-merge if collaborative working memory is wanted later.

## PR #1 scope

**In:**

- Cargo workspace + two crates that compile. `prim-cli` exposes the **full CLI
  surface** in clap — `prim [PATHS...]`, `--check`, `--diff`,
  `--stdin-filepath <p>`, `--exclude <glob>`, `--completions <shell>` — wired to
  `prim_fmt`'s **no-op** `format()` (returns input unchanged).
- `prim --help` / `--version` correct. `--stdin-filepath` is a true
  pass-through.
- Modes operate on **explicit path args + stdin only** with the no-op: in-place
  changes nothing, `--check` exits 0, `--diff` prints nothing. (Recursive
  discovery via the `ignore` crate is FR-4 → its own follow-up PR.)
- `spec/` e2e crate: snapshot tests for help/version + mode exit-code contract.
- Three CI workflows adapted (binary `prim`, two-crate publish ordered
  prim-fmt→prim-cli, dormant until tagged); `install.sh` + `bootstrap` +
  `bash_unit` install tests; mdbook skeleton with `SPEC.md` from issue #1;
  `AGENTS.md` adapted (prim identity, no-MarkSpec, prim conventions); tooling
  configs; git-std dogfood files (`.git-std.toml`, `.githooks/`).

**Out (deferred to issue decomposition from #2):** every parser (FR-1), hygiene
engine + atomic write (FR-2/6), `.editorconfig` resolution (FR-3), recursive
discovery + `.primignore` resolution (FR-4), idempotency/semantic harness
(FR-6.1/6.2), `git-prim` shim.

**Acceptance:** `just verify` + CI green · `prim --help`/`--version` correct ·
e2e snapshots pass · `install_test.sh` passes.

## Follow-up roadmap (post-scaffold, per issue #2)

1. File discovery — `ignore`/`.ignore`/`.primignore`/`--exclude`, recursion
   (FR-4).
2. Hygiene engine + atomic write + UTF-8/fail-safe (FR-2, FR-6.3/6.4/6.5).
3. `.editorconfig` resolution (FR-3).
4. Per-format parsers — json, jsonc, yaml, toml, markdown (FR-1); each may
   become its own `prim-*` crate under the lib.
5. Idempotency + semantic-preservation test harness (FR-6.1/6.2).
6. Distribution polish — `git-prim` shim, completions, man pages.
