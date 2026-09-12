# Folio Prerequisites Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the prim-side CLI, registry, and release-integrity contracts that
Folio consumes through a pinned executable.

**Architecture:** Extend the existing `fmt`/`lint`/`fix` CLI without moving
walking or policy into prim. A shared diagnostic catalog feeds runtime
diagnostics, JSON/SARIF reports, configuration explanation, and the new
registry. Release archives are built and attested in a reusable workflow, then
verified before an unchanged artifact is published.

**Tech Stack:** Rust 2024 workspace, clap, serde/serde_json, sha2, jsonschema,
GitHub Actions, cosign/Sigstore, GitHub artifact attestations, Syft SPDX JSON,
bash acceptance tests.

**Spec:** `docs/superpowers/specs/2026-09-12-folio-prerequisites-design.md`

## Global Constraints

- Folio invokes a pinned `prim` executable through CLI plus JSON; no crate
  linking, embedded Wasm host, library split, or prim MCP server.
- Folio owns repository walking, selection, installation, aggregation, policy,
  and `FOLIO-*` mapping. Explicit file arguments must not widen scope.
- prim retains its existing standalone directory walking and formatting.
- Shell formatting and linting remain outside prim.
- Registry entries must be projected from the definitions used by runtime
  diagnostics and configuration explanation.
- Every behavior change starts with a failing acceptance or unit test.
- Existing JSON report changes follow the style-stability policy and use a
  breaking Conventional Commit marker.
- Release validation must not publish a release, close an issue, or claim a SLSA
  level merely because an attestation exists.

## Acceptance-Criterion Traceability

| Issue / acceptance criterion                                                       | Existing evidence on `origin/main`                                                                            | Missing work                                                                                       | Verification                                                                                                                     |
| ---------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| #54: batched explicit `fmt`, verification, `lint`, `fix`; retain walking           | `WriteArgs.paths`/`LintArgs.paths`; `discover::collect`; `verbs.rs`, `discovery.rs`; released v0.8.0          | One Folio conformance suite and normative documentation; dry-run contract                          | Real binary tests with file lists, directories, spaces, ignored and unsupported files                                            |
| #54: md/json/jsonc/yaml/toml and un-owned hygiene; shell excluded                  | `prim_fmt::classify`; per-format integration suites; hygiene allowlist; v0.8.0                                | Cross-format batch fixture proving one invocation                                                  | Compare expected bytes for all supported kinds; assert `.sh` unchanged                                                           |
| #54: machine results, stable codes, clean stdout on success/failure                | JSON/SARIF `report.rs`; 0/1/2 exits; panic containment; v0.8.0                                                | Structured operational errors for JSON and SARIF                                                   | Schema validation on clean, actionable, partial-failure, and full-failure runs                                                   |
| #54: exact no-write effect plan for `fmt`/`fix`                                    | `--check`/`--diff` are no-write but not exact plans                                                           | `--dry-run --format json`; digests, lengths, style snapshot, structured errors                     | No-write assertions and plan/apply digest fidelity under unchanged inputs/config                                                 |
| #54: assets/checksums resolvable by version                                        | v0.8.0 has five `prim-<target>.tar.gz` plus `.sha256` assets; `release.yml`, `installation.md`, install tests | Conformance test and release mapping                                                               | `gh release view v0.8.0`; local asset-contract tests                                                                             |
| #54: pinned standalone and Folio-style invocation produce identical bytes          | Per-format formatter tests and golden corpus                                                                  | Dedicated equivalence fixture                                                                      | Run directory invocation in one copy and explicit batch in another; compare bytes recursively                                    |
| #54: record satisfying release                                                     | v0.8.0 satisfies reused contracts only                                                                        | Mark new contracts unreleased until a tag exists                                                   | Documentation table separating v0.8.0 evidence from unreleased work                                                              |
| #198: `registry --format json` with codes/descriptions/metadata                    | Runtime hygiene codes, Markdown tier table, `format::drift`, explain keys                                     | Registry command and serializer                                                                    | Snapshot/schema test; clean stdout and exit 0                                                                                    |
| #198: schema/tool versions and compatibility                                       | CLI package version; report version 1                                                                         | Published registry schema and policy                                                               | Validate emitted registry with checked-in schema                                                                                 |
| #198: no duplicate inventory                                                       | `ACTIVE_RULES`, `LINE_LENGTH_RULE`, hygiene literals currently drive behavior                                 | Shared public catalog definitions                                                                  | Unit tests prove selection, `is_known_rule`, emission, explain keys, and registry share definitions                              |
| #198: every emitted code resolves; disabled/configurable metadata; aliases/retired | Markdown census and rule fixtures                                                                             | Coverage test and inline-control metadata                                                          | Gather emitted codes from constructors/fixtures and assert registry lookup; assert empty aliases/retired                         |
| #198: release containing interface                                                 | None                                                                                                          | Mark registry unreleased                                                                           | Replace with actual version only after release                                                                                   |
| #55: cosign/Sigstore-signed artifacts                                              | No signing in `release.yml`                                                                                   | Keyless cosign bundles and blocking verification                                                   | Local ephemeral-key sign/verify; actionlint; hosted OIDC verification remains release-candidate gate                             |
| #55: SPDX 2.3+ SBOM per release                                                    | No SBOM                                                                                                       | Syft v1.51.1 `spdx-json` asset and signature                                                       | Assert `spdxVersion == SPDX-2.3`; validate JSON; asset-contract test                                                             |
| #55: SLSA Build Level 3 provenance                                                 | No provenance                                                                                                 | Reusable build workflow, GitHub/Sigstore provenance, signer/digest verification before publication | Static permission/dependency tests; `gh attestation verify` in blocking hosted job; repository protection audit remains external |
| #55: `cargo audit` in CI                                                           | `.github/workflows/ci.yml` `audit` job already gates umbrella `ci` job                                        | No implementation change                                                                           | Preserve workflow test and run repository checks                                                                                 |

## File Map

- Create `crates/prim-cli/src/machine_path.rs`: display and percent-encoded path
  projection shared by reports and plans.
- Create `crates/prim-cli/src/run_diagnostic.rs`: stable operational diagnostic
  definitions and per-run error values.
- Create `crates/prim-cli/src/app/effect_plan.rs`: v1 plan model, hashing, and
  JSON rendering.
- Create `crates/prim-cli/src/registry.rs`: compose and render the registry from
  engine and CLI definitions.
- Modify `crates/prim-cli/src/cli.rs`: add `--dry-run`, conditional `--format`
  validation surface, and `registry`.
- Modify `crates/prim-cli/src/app.rs`, `app/load.rs`, `app/paths.rs`, and
  `app/stdin.rs`: dispatch plan mode and carry operational diagnostics into
  structured reports.
- Modify `crates/prim-cli/src/report.rs`: reuse machine paths and serialize
  operational errors into JSON/SARIF.
- Modify `crates/prim-fmt/src/diagnostics.rs`: source hygiene codes from shared
  definitions.
- Modify `crates/prim-fmt/src/mdlint.rs`: expose metadata from the active rule
  table and use it for selection/known-rule checks.
- Create `schemas/prim-effect-plan-v1.schema.json` and
  `schemas/prim-registry-v1.schema.json`.
- Create `crates/prim-cli/tests/folio_conformance.rs`, `effect_plan.rs`, and
  `registry.rs`.
- Modify `crates/prim-cli/tests/machine_readable.rs`, the trycmd help fixtures,
  and crate manifests.
- Create `.github/workflows/release-build.yml`: reusable build/sign/attest job.
- Modify `.github/workflows/release.yml`: matrix caller, SBOM, verification,
  release, and crates.io job dependencies.
- Create `tools/release/package.sh` and `spec/install/release_contract_test.sh`:
  reusable packaging/checksum logic and non-publishing contract tests.
- Modify `justfile`: include release-contract tests in repository checks.
- Modify `docs/SPEC.md`, `docs/USAGE.md`, `docs/installation.md`, `README.md`,
  and `CHANGELOG.md`: normative CLI/schema/release documentation.

---

### Task 1: #54 Explicit-Batch Conformance Baseline

**Files:**

- Create: `crates/prim-cli/tests/folio_conformance.rs`
- Modify: `docs/SPEC.md`
- Modify: `docs/USAGE.md`

**Interfaces:**

- Consumes: existing `prim fmt PATH...`,
  `prim fmt --check --format json
  PATH...`, `prim lint --format json PATH...`,
  and `prim fix PATH...`.
- Produces: a black-box fixture helper that later effect-plan tests reuse only
  through real CLI invocations; no production API.

- [x] **Step 1: Write the explicit-scope and cross-format acceptance tests**

Create fixtures containing `README.md`, `config.json`, `config.jsonc`,
`config.yaml`, `config.toml`, `notes.txt`, `script.sh`, an ignored Markdown
file, a generated file, an explicitly named symlink, an unselected sibling, and
a selected path containing a space. The tests must run one explicit batch and
assert that only named, owned, non-ignored regular files change; the link and
its target must remain untouched.

```rust
prim()
    .current_dir(root)
    .arg("fmt")
    .args(selected_paths)
    .assert()
    .success();
assert_eq!(fs::read(root.join("script.sh"))?, original_shell);
assert_eq!(fs::read(root.join("unselected.md"))?, original_unselected);
```

- [x] **Step 2: Write the equivalence acceptance test**

Copy the same repository fixture twice. Run `prim fmt .` in one copy and pass
the walker-selected file list explicitly in the other. Include a root
`.editorconfig` and a narrower Markdown section. Recursively compare every
regular file's bytes after both invocations.

- [x] **Step 3: Run the conformance suite and record existing evidence**

Run:

```console
cargo test -p prim-cli --test folio_conformance
```

Expected: the existing behavior tests pass. If an assertion reveals an actual
scope defect, stop and use `systematic-debugging` before changing production
code.

- [x] **Step 4: Document the explicit-file ownership boundary**

Add a dedicated Folio/delegation subsection naming the supported explicit
invocations, safety exclusions, shell exclusion, and the distinction between a
file list and a directory argument.

- [x] **Step 5: Commit the verified existing contract**

```console
git add crates/prim-cli/tests/folio_conformance.rs docs/SPEC.md docs/USAGE.md
git commit -m "test(prim-cli): pin explicit delegation scope"
```

### Task 2: #54 Exact `fmt`/`fix` Effect Plans

**Files:**

- Create: `crates/prim-cli/src/machine_path.rs`
- Create: `crates/prim-cli/src/run_diagnostic.rs`
- Create: `crates/prim-cli/src/app/effect_plan.rs`
- Create: `crates/prim-cli/tests/effect_plan.rs`
- Create: `schemas/prim-effect-plan-v1.schema.json`
- Modify: `crates/prim-cli/Cargo.toml`
- Modify: `crates/prim-cli/src/cli.rs`
- Modify: `crates/prim-cli/src/argv.rs`
- Modify: `crates/prim-cli/src/app.rs`
- Modify: `crates/prim-cli/src/app/load.rs`
- Modify: `crates/prim-cli/src/app/paths.rs`
- Modify: `spec/tests/cmd/general/fmt_help.toml`
- Modify: `spec/tests/cmd/general/fix_help.toml`

**Interfaces:**

- Produces: `prim fmt --dry-run --format json PATH...` and
  `prim fix --dry-run --format json PATH...`.
- Produces internally:

```rust
pub(crate) enum PlannedOperation { Fmt, Fix }
pub(crate) fn render(
    operation: PlannedOperation,
    files: &[FormattedFile],
    errors: &[RunDiagnostic],
) -> String;
pub(crate) fn display(path: &Path) -> String;
pub(crate) fn encoded(path: &Path) -> Option<String>;
```

- [x] **Step 1: Write failing CLI parse and conflict tests**

Pin that dry-run requires `--format json`, rejects SARIF, and conflicts with
check/diff/idempotence/stdin. Pin that `fix --dry-run --format json` is
accepted. Run the focused tests and verify failure because `--dry-run` is
unknown.

- [x] **Step 2: Add the minimal clap surface and validation**

Add `dry_run: bool` to `WriteArgs`, allow `FmtArgs.format` with either check or
dry-run through explicit validation, and add `format: Option<OutputFormat>` to
`FixArgs`. Invalid combinations print a usage error and return `2` without
processing files.

- [x] **Step 3: Write the failing plan-schema and no-write tests**

Validate output against `prim-effect-plan-v1.schema.json`. Assert operation,
path (including spaces), kind, before/after lengths, 64-character lowercase
SHA-256 values, the six effective style fields, empty errors, exit `1`, and
byte-identical inputs after both fmt and fix planning.

- [x] **Step 4: Implement path projection and effect rendering**

Move the report module's existing path display/percent-encoding behavior into
`machine_path.rs`. Add `sha2 = "0.10"`. Serialize the exact schema from the
design with `serde`, hashing `original.as_bytes()` and `formatted.as_bytes()`.
Use `FileKind` names `markdown`, `json`, `jsonc`, `yaml`, `toml`, and `orphan`.

- [x] **Step 5: Dispatch dry-run before every write branch**

In `run_fmt_paths`, after `load_and_format` and before the loop can call
`write::atomic`, render the plan and return `2` on load error, `1` when effects
exist, `2` when nothing was examined, otherwise `0`. Preserve successful effects
when another input fails.

- [x] **Step 6: Write and pass plan/apply fidelity tests**

For each file kind and each verb, capture the plan, assert no writes, run the
same explicit apply invocation without changing file bytes or `.editorconfig`,
and compare actual SHA-256/length to every planned `after` value. Include a
clean file producing `effects: []` and exit `0`. On Unix, include a filename
that is not valid UTF-8 and assert its lossy `path`, exact percent-encoded
`path_encoded`, and that a valid UTF-8 path omits `path_encoded`.

- [x] **Step 7: Run focused tests and update help snapshots**

```console
cargo test -p prim-cli --test effect_plan
cargo test -p prim-spec general
```

Expected: all plan behavior and help snapshots pass.

- [x] **Step 8: Commit effect planning**

```console
git add crates/prim-cli crates/prim-cli/tests/effect_plan.rs schemas/prim-effect-plan-v1.schema.json spec/tests/cmd/general
git commit -m "feat(prim-cli): add exact formatting effect plans"
```

### Task 3: #54 Structured Failure Results

**Files:**

- Modify: `crates/prim-cli/src/run_diagnostic.rs`
- Modify: `crates/prim-cli/src/app/load.rs`
- Modify: `crates/prim-cli/src/app/paths.rs`
- Modify: `crates/prim-cli/src/app/stdin.rs`
- Modify: `crates/prim-cli/src/report.rs`
- Modify: `crates/prim-cli/tests/machine_readable.rs`
- Modify: `crates/prim-cli/tests/effect_plan.rs`

**Interfaces:**

- Produces internally:

```rust
pub(crate) struct RunDiagnostic {
    pub code: &'static str,
    pub path: Option<PathBuf>,
    pub message: String,
}
pub(crate) const INPUT_READ: Definition;
pub(crate) const FORMAT_PARSE: Definition;
pub(crate) const INTERNAL_PANIC: Definition;
pub(crate) const SCOPE_EMPTY: Definition;
pub(crate) const SCOPE_RESOLVE: Definition;
```

- [x] **Step 1: Write failing JSON and SARIF failure-envelope tests**

For `fmt --check`, `lint`, `fmt --dry-run`, and `fix --dry-run`, cover an
explicit malformed JSON file, a missing file, a contained panic, an
examined-nothing gate, and a mixed valid/invalid batch. Assert one schema-valid
stdout document, stable codes and paths, preserved normal findings/effects, exit
`2`, and byte-identical inputs. The effect-plan cases must prove both verbs
retain successful effects from a mixed batch while reporting its failures.

- [x] **Step 2: Carry typed run diagnostics through loading**

Replace `Loaded.had_error: bool` with `Loaded.errors: Vec<RunDiagnostic>` while
preserving ordered human stderr. Construct the five definitions at the exact
failure branches rather than inferring codes from message strings.

- [x] **Step 3: Serialize JSON `errors` and SARIF error results**

Add `errors` to the existing JSON envelope. Render each error in SARIF with its
code as `ruleId`, `level: "error"`, and an optional artifact location. Ensure
the same definition supplies registry description, result code, and SARIF rule.

- [x] **Step 4: Pass all structured-output regression tests**

```console
cargo test -p prim-cli --test machine_readable
cargo test -p prim-cli --test effect_plan
cargo test -p prim-cli --test safety
```

- [x] **Step 5: Commit the schema-compatible report extension**

Because adding `errors` changes stable serialized output, use the repository's
breaking marker even though consumers can treat the field additively:

```console
git add crates/prim-cli/src crates/prim-cli/tests
git commit -m "feat(prim-cli)!: report structured operational errors"
```

### Task 4: #198 Shared Diagnostic Catalog and Registry

**Files:**

- Create: `crates/prim-cli/src/registry.rs`
- Create: `crates/prim-cli/tests/registry.rs`
- Create: `schemas/prim-registry-v1.schema.json`
- Modify: `crates/prim-fmt/src/diagnostics.rs`
- Modify: `crates/prim-fmt/src/mdlint.rs`
- Modify: `crates/prim-fmt/src/lib.rs`
- Modify: `crates/prim-cli/src/run_diagnostic.rs`
- Modify: `crates/prim-cli/src/report.rs`
- Modify: `crates/prim-cli/src/provenance.rs`
- Modify: `crates/prim-cli/src/cli.rs`
- Modify: `crates/prim-cli/src/argv.rs`
- Modify: `crates/prim-cli/src/app.rs`
- Modify: `crates/prim-cli/src/lib.rs`
- Create: `spec/tests/cmd/general/registry_help.toml`

**Interfaces:**

- Produces: `prim registry --format json`.
- Produces in `prim-fmt`:

```rust
pub struct DiagnosticDefinition {
    pub code: &'static str,
    pub description: &'static str,
}
pub enum MarkdownTier { Floor, Strict, LineLength }
pub struct MarkdownRuleDefinition {
    pub code: &'static str,
    pub description: &'static str,
    pub tier: MarkdownTier,
}
pub fn hygiene_diagnostic_definitions() -> &'static [DiagnosticDefinition];
pub fn markdown_rule_definitions() -> Vec<MarkdownRuleDefinition>;
```

- [x] **Step 1: Write failing engine catalog-sharing tests**

Assert every hygiene diagnostic's code is a catalog member; every active
Markdown fixture code is returned once; `is_known_rule` equals a
case-insensitive catalog lookup; and the floor/strict/MD013 metadata matches
selection.

- [x] **Step 2: Refactor hygiene and Markdown definitions without behavior
      change**

Introduce five hygiene constants and make diagnostic construction refer to them.
Add the tier to the existing Markdown `RulePolicy`; derive rule objects,
descriptions, selection, and `is_known_rule` from that table plus MD013. Do not
copy rule identifiers into the CLI crate.

- [x] **Step 3: Write the failing registry command and schema tests**

Assert deterministic code ordering, tool name/version, schema version 1,
categories, formats, severity, enablement, configuration keys, inline controls,
disable capability, and empty aliases/retired arrays. Validate against
`prim-registry-v1.schema.json`. Assert stderr is empty.

- [x] **Step 4: Implement registry composition and dispatch**

Compose `format::drift`, engine hygiene/Markdown definitions, and operational
definitions. Markdown metadata must name `prim_mdlint_disable`, strict or
line-length EditorConfig keys as applicable, `prim-mdlint-strict` where tier
selection can change, and rumdl/markdownlint disable directives for every
Markdown rule. MD013 must name both `prim_mdlint_report_line_length` and
`max_line_length`, and must include `prim-mdlint-strict` because strict mode
changes whether the enabled rule examines headings.

- [x] **Step 5: Make explain consume shared configuration-key constants**

Keep `prim explain PATH` behavior unchanged, but source every registry and
explain key name from the same `mdlint_policy` constants so neither can drift.

- [x] **Step 6: Prove every emitted code resolves**

Build a set from hygiene constructors, Markdown rule fixtures, format drift, and
structured-error tests. Assert each code has exactly one registry entry and
every registry code is either exercised by a fixture or explicitly marked
runtime-only.

- [x] **Step 7: Run engine, registry, report, and snapshot tests**

```console
cargo test -p prim-fmt diagnostics
cargo test -p prim-fmt mdlint
cargo test -p prim-cli --test registry
cargo test -p prim-cli --test machine_readable
cargo test -p prim-spec general
```

- [x] **Step 8: Commit the registry**

```console
git add crates/prim-fmt crates/prim-cli schemas/prim-registry-v1.schema.json spec/tests/cmd/general/registry_help.toml
git commit -m "feat(prim-cli): publish the diagnostic registry"
```

### Task 5: #55 Release Packaging and Asset Contracts

**Files:**

- Create: `tools/release/package.sh`
- Create: `spec/install/release_contract_test.sh`
- Modify: `spec/install/install_test.sh`
- Modify: `justfile`
- Modify: `docs/installation.md`

**Interfaces:**

- Produces:

```console
tools/release/package.sh TARGET BINARY MAN_PAGE OUTPUT_DIRECTORY
```

The script emits exactly `prim-<target>.tar.gz` and
`prim-<target>.tar.gz.sha256`, with the checksum file naming the archive's base
name rather than an absolute path.

- [x] **Step 1: Write failing packaging-contract shell tests**

Test all five target names, Windows `.exe` handling, archive contents, checksum
verification from another working directory, and rejection of a missing binary
or unknown target. Extend install URL tests to assert version-addressable asset
names.

- [x] **Step 2: Extract deterministic packaging from the workflow**

Implement the script with `set -euo pipefail`, an explicit five-target case, an
isolated staging directory, portable `sha256sum`/`shasum`, and no publishing or
network operation.

- [x] **Step 3: Wire release-contract tests into `just check`**

Add a `test-release` recipe and make `check` depend on it. The tests create all
artifacts under a temporary directory and clean them through the test harness.

- [x] **Step 4: Run install and release contract tests**

```console
just test-install
just test-release
```

- [x] **Step 5: Commit packaging and discovery evidence**

```console
git add tools/release spec/install justfile docs/installation.md
git commit -m "build(release): pin release asset contracts"
```

### Task 6: #55 Signed SBOM and Provenance Workflow

**Files:**

- Create: `.github/workflows/release-build.yml`
- Modify: `.github/workflows/release.yml`
- Modify: `spec/install/release_contract_test.sh`
- Modify: `docs/installation.md`
- Modify: `docs/SPEC.md`

**Interfaces:**

- `release-build.yml` is invoked with `target`, `runner`, `use_cross`, and
  `extension`. It uploads one workflow artifact containing the archive,
  checksum, cosign bundle, and provenance bundle.
- The release workflow publishes only after a verifier job confirms checksums,
  cosign identity/issuer, GitHub attestation signer workflow, subject names,
  subject digests, SBOM version, and the release-tag/default-branch relation.

- [x] **Step 1: Extend failing workflow-contract tests**

Parse both workflow files as YAML and pin the reusable `workflow_call`,
five-target matrix, `actions/attest@v4`, `sigstore/cosign-installer@v4.1.2`,
Syft v1.51.1, and SPDX-2.3 assertion. Assert each job's exact permission map,
the publish job's exact `needs`, verifier commands and identity/issuer
arguments, and that no publication step transforms an archive. Repository-wide
string searches are insufficient for these security properties.

- [x] **Step 2: Implement the reusable build/sign/attest workflow**

Move the existing cross/cargo build steps into the reusable workflow, call
`tools/release/package.sh`, run keyless `cosign sign-blob --yes --bundle`, run
`actions/attest@v4` over the archive, copy its bundle to the deterministic
`.provenance.sigstore.json` name, and upload all four files with
`if-no-files-found: error`.

- [x] **Step 3: Generate and sign the SPDX 2.3 SBOM**

Checkout the tagged source, invoke pinned Syft v1.51.1 with `spdx-json`, assert
`jq -e '.spdxVersion == "SPDX-2.3"'`, sign the document with keyless cosign, and
upload both assets for the verifier.

- [x] **Step 4: Add blocking hosted verification before release publication**

Download all matrix/SBOM artifacts, verify every `.sha256`, run
`cosign verify-blob` with issuer `https://token.actions.githubusercontent.com`
and the exact reusable-workflow certificate identity, run
`gh attestation verify` with repository `driftsys/prim` and
`.github/workflows/release-build.yml` as signer, check the tag commit is
contained by `origin/main`, and verify the exact expected asset set. The release
job and every other publication job, including crates.io publication, must
`need` this verifier.

- [x] **Step 5: Preserve crates.io publication and cargo-audit gates**

Keep the current idempotent crate publishing logic behind the verifier and
confirm `ci.yml` still lists `audit` in the umbrella job's `needs`.

- [x] **Step 6: Validate without publishing**

Run:

```console
just test-release
actionlint .github/workflows/ci.yml .github/workflows/release.yml .github/workflows/release-build.yml
```

Download cosign v3 through the official installer into a temporary directory,
generate an ephemeral test key, sign a locally packaged archive, and verify the
bundle. Generate a local Syft v1.51.1 SBOM and assert SPDX-2.3. Do not invoke
the tag-triggered release workflow.

- [x] **Step 7: Document the external Level 3 gate precisely**

State that protected-branch/tag/ruleset inspection and the first hosted OIDC run
remain required before #55 can be considered complete. Document
`gh attestation verify` and `cosign verify-blob` commands without claiming the
level from attestation generation alone.

- [x] **Step 8: Commit workflow hardening**

```console
git add .github/workflows spec/install/release_contract_test.sh docs/installation.md docs/SPEC.md
git commit -m "feat(release): sign and attest release artifacts"
```

### Task 7: Documentation, Full Verification, and Review

**Files:**

- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/SPEC.md`
- Modify: `docs/USAGE.md`
- Modify: `docs/installation.md`
- Modify: `docs/SUMMARY.md` if schema pages are added to the book
- Modify: all files changed by earlier tasks when review finds a defect

**Interfaces:**

- Produces: the exact Folio invocation/schema/release contract and an evidence
  table that distinguishes v0.8.0 evidence from unreleased behavior.

- [x] **Step 1: Complete normative documentation**

Document every invocation, conflict, exit, field, error code, compatibility
rule, inline Markdown control, asset name, checksum/signature/provenance
verification command, supported target, unsupported effect, and plan
precondition from the approved design.

- [x] **Step 2: Run formatter and targeted tests**

```console
just fmt
cargo test -p prim-cli --test folio_conformance
cargo test -p prim-cli --test effect_plan
cargo test -p prim-cli --test machine_readable
cargo test -p prim-cli --test registry
just test-install
just test-release
```

- [x] **Step 3: Exercise real CLI examples manually**

Build once, then run the binary against temporary fixtures for explicit paths,
spaces, ignored/generated/unsupported inputs, malformed JSON, nested
`.editorconfig`, plan/apply fidelity, lint JSON, and registry JSON. Validate
stdout with `jq` and both schemas with the test validator.

- [x] **Step 4: Run all repository-required checks**

```console
just verify
```

Expected: tests, install/release tests, lint, rustdoc, commit lint, and build
all pass with zero warnings.

- [x] **Step 5: Use `requesting-code-review`**

Request a fresh review against the approved design, issue acceptance criteria,
AGENTS.md, and the branch diff. Fix every confirmed correctness, test-fidelity,
documentation, or release-security finding. Do not publish, merge, or close
issues.

- [x] **Step 6: Use `verification-before-completion`**

Re-run fresh evidence after all fixes, inspect `git status`, list commit IDs,
and record remaining hosted CI/release or repository-protection blockers.

- [x] **Step 7: Commit final documentation or review fixes if needed**

```console
git add README.md CHANGELOG.md docs schemas crates spec tools .github justfile Cargo.toml Cargo.lock
git commit -m "docs(prim): document Folio CLI contracts"
```

Skip this commit when no tracked changes remain after earlier task commits.
