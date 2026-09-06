# AD-0020 — prim-cli carries an internal library target

## Status

Accepted. Closes #158. Not breaking: the `prim` command line is unchanged and no
crate gains a runtime dependency. `prim-cli` gains `criterion` as a
dev-dependency, which the benchmark needs and a consumer never builds.

## Context

AD-0002 records that the per-directory `.editorconfig` cascade cache made a
5,000-file tree about 9 % faster to `--check`. That figure came from a commit
message and nothing reproduces it, so it can be neither re-checked nor defended
against regression (#158). The only benchmark in the workspace,
`crates/prim-fmt/benches/format.rs`, calls `prim_fmt::format` with
`Style::default()` and never touches `.editorconfig` resolution at all.

`Resolver` lives in `crates/prim-cli/src/editorconfig.rs`, and `prim-cli` was a
binary-only crate: no `lib.rs`, no `[lib]` target, every module declared with a
plain `mod` in `main.rs`. Cargo cannot link a bench or an integration test
against a `[[bin]]` target, so nothing outside the binary could name `Resolver`.
The existing tests in `crates/prim-cli/tests/` all drive the built binary for
the same reason: through `assert_cmd`, except `lsp.rs`, which spawns
`CARGO_BIN_EXE_prim` directly to speak JSON-RPC over its stdio.

## Options

1. **Move resolution into `prim-fmt`.** It would be reachable from the existing
   bench harness immediately. Rejected: `Resolver` reads files, and AD-0001
   keeps `prim-fmt` free of I/O. This trades a measurement problem for a
   boundary violation.
2. **Extract resolution into a new `prim-editorconfig` crate.** Clean, and
   AGENTS.md already anticipates per-format parsers splitting out this way.
   Rejected as disproportionate: a third crate, its own manifest and release
   surface, to make one benchmark possible.
3. **Benchmark the binary end to end**, timing `prim fmt --check` over a
   generated tree. No structural change, and it matches the shape of the
   original measurement. Rejected as insufficient on its own: no flag disables
   the cache, so this cannot compare cached against uncached and therefore
   cannot attribute any part of the result to the cache, which is the specific
   claim #158 says is undefendable.
4. **Give `prim-cli` a library target** alongside its binary.

## Decision

Option 4. `crates/prim-cli/src/lib.rs` declares the crate's modules and
`main.rs` consumes them as an ordinary dependent, which is the conventional Rust
layout for a binary whose internals need to be linked by tests or benchmarks.

**The public surface is the smallest one that works.** Only the modules crossed
by an external caller are `pub`: `app`, `argv`, `cli` and `ui` because `main.rs`
now reaches them from outside the crate, and `editorconfig` because the
benchmark does. Every other module is `pub(crate)`. Making them all `pub`
produced `private_interfaces` warnings where a public function returned a
`pub(crate)` type — `discover::collect` and its `discover::Error` among them —
and the repository does not permit warnings. Promoting those types to `pub` to
silence it would have been the wrong direction: it would enlarge a published API
for the benefit of an internal caller.

**No stability promise attaches to any of it.** `prim-cli` is published, so this
library target is published with it. The supported surface is the `prim` command
line, and the reusable engine is `prim-fmt` (AD-0001). `lib.rs` says so in its
own module documentation, because a crates.io consumer reads the docs rather
than this record.

## Consequences

- **The cache's effect is now measurable.**
  `crates/prim-cli/benches/resolution.rs` resolves a 1,000-file tree through one
  reused `Resolver` and again through `editorconfig::resolve`, which parses a
  fresh cascade per file. That is the A/B option 3 could not express.
- **The benchmark measures the mechanism, not AD-0002's figure.** The 9 % came
  from a whole `--check` run, where resolution is one cost among many; a
  component benchmark isolating resolution necessarily shows a larger relative
  difference. The first run of this benchmark resolved its 1,000 files in about
  9.4 ms cached against about 34.9 ms uncached — roughly 3.7 times — which is
  that larger difference, and is why the two numbers must not be read as
  versions of each other. Absolute timings are machine-specific; `just bench`
  re-measures locally. AD-0002 and `docs/design/system.md` keep the 9 % as the
  end-to-end measurement it was, and now name what does and does not reproduce
  it.
- **`prim-cli` compiles as a library and a binary**, so its modules are built
  once and linked twice rather than compiled twice. Inline `#[cfg(test)]`
  modules now run under the library target; the integration tests in
  `crates/prim-cli/tests/` still drive the binary and are unchanged.
- **`private_interfaces` does not find every leak.** The warning fires where the
  leaked item carries a narrower visibility keyword — `pub(crate) enum Error`
  inside a `pub` module — and stays silent where a plain `pub` item sits inside
  a `pub(crate)` module, though an external caller can reach neither type by
  name. `Resolver::resolve_mdlint_policy` was the second shape and is now
  `pub(crate)`. Treat the compiler as a first pass over this split, not the
  audit.
- **The commit lands as `feat`, which below 1.0.0 is a patch release.**
  `prim-cli` carries no `publish = false`, so the library target is published
  with the binary and a dependent can reach these five modules whatever this
  record says about supporting them. Semver grades what is reachable rather than
  what is promised, so the addition is recorded instead of landing as a no-bump
  `refactor`. git-std maps `feat` on a `0.x` version to a patch — `0.7.1` to
  `0.7.2` — and reserves the minor bump for a breaking change, which is how
  `0.7.0` was cut. That patch does not by itself insulate a dependent, since
  Cargo reads `0.7.2` as compatible with `^0.7.1`; that protection arrives only
  when a later change to this surface is marked breaking. The disclaimer belongs
  in the documentation, which is where it is, rather than in a commit type that
  understates what shipped.
- **A future test may now link the crate's internals directly** instead of
  driving the binary. That is a capability, not an instruction: a behaviour the
  CLI promises should still be tested through the CLI, because that is the
  surface the promise is made about.

---

Satisfies: #158 (item 3). Related: AD-0001 (the `prim-fmt` purity boundary this
deliberately does not cross), AD-0002 (the cache and its measurement),
`crates/prim-cli/src/lib.rs`, `crates/prim-cli/benches/resolution.rs`.
