# rumdl_lib integration spike (#39)

Proves prim can link the `rumdl` Markdown linter **lint-only** without dragging
in its LSP/async runtime, and records the cost. De-risks stories G2 (#58) and G3
(#59). This is a skeleton + measurements, not the finished feature.

## What was proven

- `rumdl = "=0.2.35"` links into `prim-fmt` with **`default-features = false`**,
  keeping the engine pure (no clap/terminal/I/O leaks beyond a pure `&str → Vec`
  call).
- The lint entry point matches the intended call exactly:

  ```rust
  let cfg = Config::default();
  let rules: Vec<_> = all_rules(&cfg)
      .into_iter()
      .filter(|rule| CURATED.contains(&rule.name()))
      .collect();
  rumdl_lib::lint(source, &rules, false, MarkdownFlavor::Standard, None, Some(&cfg))
  ```

- Diagnostics carry **1-indexed `line`/`column`** — the real `line:col` that
  stories B1/D2 need and that the serde-based formats lack (see spike #42).
- Lint-only: prim never calls rumdl's formatter, LSP, or file walker. A linter
  error is swallowed so it can never corrupt a format run (G2 owns real error
  surfacing).

Skeleton lives in `crates/prim-fmt/src/mdlint.rs`, re-exported as
`prim_fmt::lint_markdown` / `prim_fmt::MdDiagnostic`. Four unit tests cover
line:col reporting, clean-input, curated-only filtering, and read-only
behaviour.

## Confirmed API (rumdl 0.2.35)

| Item                          | Signature / shape                                                                                                                                                               |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rumdl_lib::lint`             | `(content: &str, rules: &[Box<dyn Rule>], verbose: bool, flavor: MarkdownFlavor, source_file: Option<PathBuf>, config: Option<&Config>) -> Result<Vec<LintWarning>, LintError>` |
| `rumdl_lib::rules::all_rules` | `(config: &Config) -> Vec<Box<dyn Rule>>`                                                                                                                                       |
| `Rule::name`                  | `-> &'static str` (e.g. `"MD034"`)                                                                                                                                              |
| `LintWarning`                 | `{ message: String, line: usize /*1-idx*/, column: usize, severity: Severity, fix: Option<Fix>, rule_name: Option<String> }`                                                    |
| `Severity`                    | `Error` / `Warning` / `Info`                                                                                                                                                    |

## Cost

Baseline binary (`prim`, current `main`, release): **3,612,592 B (~3.45 MiB)**.

Marginal linked size of rumdl, measured with two equivalent release examples
(one calling `lint_markdown`, one calling only `format`):

| Example        | Size                         |
| -------------- | ---------------------------- |
| formatter only | 2,089,504 B (~1.99 MiB)      |
| + rumdl lint   | 5,249,072 B (~5.01 MiB)      |
| **delta**      | **≈ 3,159,568 B (~3.0 MiB)** |

Dependency count (`prim-fmt`, `-e no-dev`, `default-features = false`):

|              | crates  |
| ------------ | ------- |
| before rumdl | 74      |
| after rumdl  | 139     |
| **delta**    | **+65** |

rumdl's own subtree is 96 crates; ~31 are already shared with prim's existing
deps (serde, regex, pulldown-cmark, etc.).

**Heavy `native`-feature crates confirmed ABSENT** with
`default-features =
false`: `tokio`, `tower-lsp`, `notify`, `env_logger`, and
`rayon` (the `parallel` feature). The upstream default (`native`) pulls ~243
crates; turning it off cuts that to the +65 above.

## Findings the stories must account for

- **Curation is runtime-only.** `all_rules(&cfg)` instantiates _every_ rule; the
  `.name()` filter selects which run, but the binary still carries all rule
  code. The ~3 MiB / +65 crates is essentially fixed regardless of how small the
  curated subset is. G3's severity matrix does not shrink the binary.
- **`tikv-jemallocator` is still pulled in** as a non-optional rumdl dependency
  even with `default-features = false`. It is _linked_ but not activated as the
  global allocator (rumdl only sets `#[global_allocator]` in its own binary, not
  the library), so prim keeps the system allocator. Worth a confirmation test in
  G2. If it proves undesirable, raise upstream or vendor-patch.
- **API is stable enough to pin.** The `lint`/`all_rules`/`Rule::name` surface
  is public and matches the call shape the epic specified; pin `=0.2.35` (exact)
  and bump deliberately (E3 output-stability).
- **No AC changes needed.** G2/G3 acceptance criteria hold as written; the size
  cost is the only new fact for the release/prebuilt stories (F2/F3) to note.
