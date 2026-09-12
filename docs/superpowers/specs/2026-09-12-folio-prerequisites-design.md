# Folio Prerequisites Design

## Purpose

Implement the prim-side contracts required for Folio binary orchestration in
issues #54, #198, and #55. Folio invokes a pinned `prim` executable and owns
installation, repository walking, file selection, aggregation, policy, and
`FOLIO-*` code mapping. prim retains its standalone recursive behavior and owns
only formatting, diagnostics, effect planning, and its release artifacts.

This design does not add crate linking, a library split, an embedded Wasm host,
an MCP server, shell formatting, or Folio-specific repository policy.

## Existing Evidence and Missing Work

The released v0.8.0 CLI already provides the `fmt`/`lint`/`fix` verbs, strict
explicit-path handling, standalone walking, Markdown/JSON/JSONC/YAML/TOML
formatting, un-owned-text hygiene, stable `0`/`1`/`2` exits, JSON and SARIF
findings for `fmt --check` and `lint`, output-stability tests, and five prebuilt
target archives with a matching SHA-256 file for each archive.

The remaining work is:

- contract tests and documentation for Folio-style explicit batches;
- an exact, no-write JSON effect plan for `fmt` and `fix`;
- a versioned diagnostic registry derived from the definitions used at run time;
- SPDX SBOMs, Sigstore/cosign signatures, and build-provenance attestations;
- validation of the release workflow and asset layout without publishing a
  release.

The first release containing the new interfaces cannot be identified until the
branch is merged and released. Documentation will identify them as unreleased
and record v0.8.0 only for contracts verified in that release.

## CLI Contract

### Explicit batches

The supported delegation invocations are:

```console
prim fmt PATH...
prim fmt --check --format json PATH...
prim fmt --dry-run --format json PATH...
prim lint --format json PATH...
prim fix PATH...
prim fix --dry-run --format json PATH...
```

Each `PATH` supplied by Folio names a file. prim processes exactly those
arguments after applying prim's own safety exclusions: `.primignore`, the
built-in generated-file list, unsupported file kinds, and symlink protection. It
does not search a parent or sibling directory merely because files were
provided. Passing a directory remains the existing standalone recursive-walk
interface.

Global `--exclude`, `--no-ignore`, `--no-primignore`, `--since`, and `--staged`
remain available to standalone callers. Folio does not need them because it owns
selection.

### Effect-plan mode

`--dry-run` is accepted by `fmt` and `fix`. It requires `--format json` and
conflicts with `--check`, `--diff`, `--check-idempotence`, and
`--stdin-filepath`. `--format sarif` is not an effect-plan format.

Effect planning executes the normal discovery, classification, `.editorconfig`
resolution, and formatting pipeline, but never calls the atomic writer. It
reports one replacement effect for each file whose bytes would change. Clean
files are examined but omitted from `effects`.

The exit contract is:

- `0`: every examined input is already canonical;
- `1`: at least one replacement effect is planned;
- `2`: prim could not complete the plan, including an explicitly named invalid
  input or a gate that examined nothing.

When some files can be planned and another file fails, the successful effects
remain in the document and the process exits `2`. No file is written.

### Effect-plan schema

The v1 JSON shape is:

```json
{
  "schema_version": 1,
  "tool": {
    "name": "prim",
    "version": "0.9.0"
  },
  "operation": "fmt",
  "effects": [
    {
      "path": "docs/guide.md",
      "kind": "markdown",
      "operation": "replace_contents",
      "before": {
        "sha256": "hex-encoded digest",
        "bytes": 14
      },
      "after": {
        "sha256": "hex-encoded digest",
        "bytes": 12
      },
      "configuration": {
        "end_of_line": "lf",
        "trim_trailing_whitespace": true,
        "insert_final_newline": true,
        "indent_style": "space",
        "indent_size": 2,
        "max_line_length": 80
      }
    }
  ],
  "errors": []
}
```

`path_encoded` is added beside `path` only for a filename that is not valid
UTF-8, using the same percent-encoding contract as existing JSON reports.
`max_line_length` is `null` when unset. `indent_size` is `null` when tab
indentation applies.

The plan describes only file-content replacement. It never promises file
creation, deletion, rename, permission changes, index changes, or Markdown
content-rule autofixes. `fix` currently produces the same content effects as
`fmt` because prim has no autofixable content rules; the distinct `operation`
value preserves the caller's intent and allows later additive effect kinds.

Plan/apply fidelity requires all planned file bytes and their effective
`.editorconfig` settings to remain unchanged between invocations. Under those
preconditions, applying the same explicit invocation must produce each
`after.sha256` and `after.bytes` value. Folio owns the final comparison of the
worktree after apply against the approved plan.

The schema is published in the repository as
`schemas/prim-effect-plan-v1.schema.json`. Within schema version 1, new optional
fields may be added; removing a field, changing its meaning or type, or changing
the exit meanings requires a new schema version and release-note callout.

Errors in plan mode are structured and leave stdout as one parseable document.
Each error has `code`, `message`, and an optional encoded path. Initial codes
cover input reads, parse failures, contained formatter panics, and the
examined-nothing gate. Human details may also be written to stderr, but logs
never enter stdout.

The same error objects are added as an `errors` array to the existing
`fmt --check --format json` and `lint --format json` envelopes. They therefore
identify every failed path and stable error code on success, partial failure,
and full failure instead of requiring a JSON consumer to scrape stderr. The
field is additive within report schema version 1 and is always present, empty
when the run had no operational error. SARIF represents the same errors as
error-level `results` using the same `ruleId`; an error with a path carries an
artifact location and a run-wide error does not. Exit `2` remains the
authoritative indication that the operation could not be completed.

## Diagnostic Registry

`prim registry --format json` is read-only. It does not inspect files, resolve
repository configuration, walk directories, or emit diagnostics. Other output
formats are rejected with exit `2`.

The v1 document is:

```json
{
  "schema_version": 1,
  "tool": {
    "name": "prim",
    "version": "0.9.0"
  },
  "diagnostics": [
    {
      "code": "MD041",
      "description": "First line in a file should be a top-level heading",
      "category": "markdown",
      "default_severity": "error",
      "formats": ["markdown"],
      "enabled_by": "prim_mdlint_strict",
      "configuration_keys": [
        "prim_mdlint_strict",
        "prim_mdlint_disable"
      ],
      "inline_controls": [
        "prim-mdlint-strict",
        "rumdl-disable",
        "markdownlint-disable"
      ],
      "can_disable": true
    }
  ],
  "aliases": [],
  "retired": []
}
```

The registry contains:

- `format::drift`;
- all whitespace-hygiene codes;
- every Markdown rule prim can emit in the floor, strict, or configurable
  line-length tier;
- stable operational error codes emitted by effect-plan JSON.

The Markdown catalog is projected from the same rule-policy table that selects
rules for lint. Descriptions come from the rule objects supplied by the pinned
rumdl version. `is_known_rule`, `prim_mdlint_disable` validation, lint
selection, registry metadata, and configuration explanation all consume that
shared source. Hygiene emission similarly refers to shared definitions rather
than spelling codes separately.

`enabled_by` is one of `always`, `editorconfig`, `prim_mdlint_strict`,
`prim_mdlint_report_line_length`, or `runtime`. `configuration_keys` identifies
every EditorConfig key that can affect the diagnostic. `inline_controls`
identifies the existing file-level strict override and rumdl/markdownlint file-,
block-, line-, and next-line disable directives wherever they can alter
selection or suppression, plus `markdownlint-configure-file` for per-file rule
options. Registry documentation defines the syntax and scope of each named
control; the per-diagnostic array makes its applicability machine-readable.
`aliases` maps a former code to its canonical replacement. `retired` records
codes that no longer emit; both arrays are empty initially but are present to
make retirement explicit.

The schema is published as `schemas/prim-registry-v1.schema.json`. Additive
metadata is allowed within v1. Removing or redefining a code or field requires a
new schema version and release-note callout. Registry stdout is always one JSON
document and contains no logs.

## Release Integrity

### Assets

The existing five targets remain:

- `x86_64-unknown-linux-musl`;
- `aarch64-unknown-linux-musl`;
- `x86_64-apple-darwin`;
- `aarch64-apple-darwin`;
- `x86_64-pc-windows-msvc`.

For each target, a release publishes:

```text
prim-<target>.tar.gz
prim-<target>.tar.gz.sha256
prim-<target>.tar.gz.sigstore.json
prim-<target>.tar.gz.provenance.sigstore.json
```

The release also publishes a release-wide SPDX JSON SBOM and its cosign bundle:

```text
prim-<version>.spdx.json
prim-<version>.spdx.json.sigstore.json
```

Archives and checksums therefore remain directly resolvable from
`releases/download/v<version>/` without running `install.sh`. The install script
continues to verify SHA-256 and is not part of Folio's installation path.

### Signing and provenance

Prebuilt construction moves into a reusable workflow invoked once per target by
the tag-triggered release workflow. The reusable workflow performs the build,
packages the archive, writes its checksum, creates a keyless cosign signature
bundle, and creates a GitHub/Sigstore SLSA build-provenance attestation for the
archive. The caller only gathers the produced workflow artifacts and publishes
them.

This structure follows GitHub's documented prerequisite for using reusable,
vetted build instructions when targeting SLSA v1.0 Build Level 3. The workflow
and documentation do not claim that merely generating an attestation proves a
level. Publication is blocked until a verification job has downloaded every
archive and attestation, matched every subject name and SHA-256 digest, and run
`gh attestation verify` with the `driftsys/prim` repository and the exact
reusable workflow as the required signer. It also verifies each cosign bundle
against GitHub Actions' OIDC issuer and that signer identity. The final release
job consumes the already-verified workflow artifacts and performs no content
transformation.

The Level 3 release gate additionally requires the release tag to point at the
reviewed commit on the protected default branch, the reusable build workflow to
be the attestation signer, job permissions to remain least-privilege, and the
release environment/ruleset to prevent an unreviewed caller from replacing the
vetted workflow. These repository controls and the first hosted verification run
are external state: #55 remains incomplete, and no documentation claims Level 3,
until they are inspected and the blocking verification job succeeds. Once
verified, the release record links the run and identifies the enforced controls
rather than inferring a level from the presence of an attestation.

The release-wide SBOM is generated from the checked-out release source and
`Cargo.lock` with Syft v1.51.1's `spdx-json` output. Validation requires the
document's `spdxVersion` to equal `SPDX-2.3`; a different otherwise-valid SPDX
document fails the job. The SBOM job signs it with the caller workflow's keyless
Sigstore identity (`release.yml`); archive signatures identify the reusable
`release-build.yml` workflow. The existing scheduled and pull-request
`cargo audit` gate remains unchanged.

Least-privilege permissions are set per job. Build jobs receive read-only
contents plus the OIDC and attestation permissions needed to sign and attest.
Only the final release job receives `contents: write`. crates.io publication
keeps only its existing package credential.

## Tests and Verification

Behavior tests drive the real `prim` binary and cover:

- explicit batches across Markdown, JSON/JSONC, YAML, TOML, and un-owned text;
- paths containing spaces and a directory argument retaining recursive
  standalone behavior;
- `.primignore`, generated files, unsupported files, and malformed inputs;
- identical bytes from equivalent standalone and Folio-style explicit
  invocations under the same `.editorconfig`;
- no writes in both effect-plan operations;
- exact planned-versus-applied digest and length fidelity;
- clean inputs producing no effects;
- parseable plan JSON on success, actionable findings, partial failure, and full
  failure;
- coded, parseable JSON and SARIF errors from `fmt --check` and `lint`;
- stable plan exit codes and schema validation;
- registry schema validation and deterministic ordering;
- every supported emitted diagnostic code resolving in the registry;
- floor, strict, disabled, line-length, and inline-control Markdown metadata;
- release asset/checksum naming and discovery contracts.

Release validation without publication consists of shell tests for packaging and
checksum helpers, a generated SBOM assertion that pins `spdxVersion = SPDX-2.3`,
static workflow validation with `actionlint`, local cosign sign/verify using an
ephemeral test key, and inspection of the workflow dependency and permission
graph. Keyless OIDC signing, GitHub attestation storage, multi-platform hosted
builds, signer-identity enforcement, and protected release controls require a
release-candidate workflow run. That run is a blocking #55 acceptance gate, not
evidence inferred from workflow text and not optional follow-up after
publication.

All repository-required checks finish with `just verify`.

## Documentation and Compatibility

`docs/SPEC.md`, `docs/USAGE.md`, installation documentation, command help,
generated man pages, and acceptance snapshots are updated together. The JSON
schemas and examples are normative for Folio. Changes to canonical formatted
bytes or existing report output follow the repository's style-stability policy
and require the appropriate breaking Conventional Commit marker.
