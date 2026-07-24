# Changelog

## [0.2.3] (2026-07-24)

### Refactoring

- **prim-cli:** formalize prim_* editorconfig key resolution ([#74]) ([2a20ec8])

### Documentation

- **release:** fix stale project-status claims before cutting a release ([#88])
  ([54af354])
- **release:** document install methods and prebuilt platform matrix (F3)
  ([#84]) ([c50014c]), closes [#56]
- **prim-fmt:** require feat/feat! typing for output-changing commits ([#79])
  ([c5b3a97]), closes [#53]
- **prim-cli:** add CLI verb-migration ADR (AD-0007) ([#40]) ([#64]) ([b14258b])
- **prim-fmt:** add prim v1 architecture recon for v2 spikes ([#62]) ([1219064])

### Bug Fixes

- **ci:** update convco download to its new release asset layout ([#69])
  ([29fd001])

### Features

- **prim-cli:** surface prim lint findings as LSP diagnostics ([#83]) ([#87])
  ([dc80aeb]), closes [#83]
- **prim-fmt:** add per-file Markdown lint strict override (G5) ([#85])
  ([04469b3]), closes [#61]
- **prim-cli:** add prim lsp format-on-save language server (D1) ([#82])
  ([8bb0a7b]), refs 48

* fix(prim-cli): simplify uri_to_path with question-mark operator

CI clippy (1.97) flags clippy::question_mark on the match returning None;
replace with the ? operator. No behaviour change.

- docs(prim-cli): note deferred LSP diagnostics/hover debt

Point the lsp module doc at issue #83, which tracks surfacing prim's lint
findings (B1/G2) as LSP diagnostics. D1 ships format-only by design.

- **prim-cli:** add prim explain for .editorconfig provenance (C2) ([#81])
  ([ad0943b]), closes [#47]
- **prim-cli:** ship git-std and pre-commit hook shims (D3) ([#80]) ([875e0d5]),
  closes [#50]
- **prim-cli:** add changed-file git scopes ([#78]) ([b79c369])
- **prim-cli:** parallelize file processing and add no-ignore ([#77])
  ([d2e1201])
- **prim-cli:** add prim init strict-glob scaffolder ([#76]) ([fde8228])
- **prim-cli:** add --check-idempotence self-check mode ([#75]) ([089e916])
- **prim-fmt:** add markdown severity matrix ([#73]) ([d23bd67])
- **prim-cli:** add json and sarif report formats ([#72]) ([6b92cb9])
- **prim-cli:** wire markdown lint into prim lint ([#71]) ([2e57a6a])
- **prim-fmt:** add coded, positioned hygiene diagnostics for prim lint ([#70])
  ([688ea62]), closes 44.
- **prim-fmt:** spike rumdl lint-only integration ([#39]) ([#63]) ([2046ba0])
- **prim-fmt:** strip leading UTF-8 BOM in whitespace hygiene ([#43]) ([#67])
  ([6ef7258])
- **prim-cli:** fmt/lint/fix verb model ([#57]) ([#68]) ([9dab1b8]), closes
  [#57]
- **prim-fmt:** add line:col mapper for parse diagnostics (spike [#42]) ([#66])
  ([bf54272])

[0.2.3]: https://github.com/driftsys/prim/compare/v0.2.2...v0.2.3
[2a20ec8]: https://github.com/driftsys/prim/commit/2a20ec8
[#74]: https://github.com/driftsys/prim/issues/74
[54af354]: https://github.com/driftsys/prim/commit/54af354
[#88]: https://github.com/driftsys/prim/issues/88
[c50014c]: https://github.com/driftsys/prim/commit/c50014c
[#84]: https://github.com/driftsys/prim/issues/84
[#56]: https://github.com/driftsys/prim/issues/56
[c5b3a97]: https://github.com/driftsys/prim/commit/c5b3a97
[#79]: https://github.com/driftsys/prim/issues/79
[#53]: https://github.com/driftsys/prim/issues/53
[b14258b]: https://github.com/driftsys/prim/commit/b14258b
[#40]: https://github.com/driftsys/prim/issues/40
[#64]: https://github.com/driftsys/prim/issues/64
[1219064]: https://github.com/driftsys/prim/commit/1219064
[#62]: https://github.com/driftsys/prim/issues/62
[29fd001]: https://github.com/driftsys/prim/commit/29fd001
[#69]: https://github.com/driftsys/prim/issues/69
[dc80aeb]: https://github.com/driftsys/prim/commit/dc80aeb
[#83]: https://github.com/driftsys/prim/issues/83
[#87]: https://github.com/driftsys/prim/issues/87
[04469b3]: https://github.com/driftsys/prim/commit/04469b3
[#85]: https://github.com/driftsys/prim/issues/85
[#61]: https://github.com/driftsys/prim/issues/61
[8bb0a7b]: https://github.com/driftsys/prim/commit/8bb0a7b
[#82]: https://github.com/driftsys/prim/issues/82
[ad0943b]: https://github.com/driftsys/prim/commit/ad0943b
[#81]: https://github.com/driftsys/prim/issues/81
[#47]: https://github.com/driftsys/prim/issues/47
[875e0d5]: https://github.com/driftsys/prim/commit/875e0d5
[#80]: https://github.com/driftsys/prim/issues/80
[#50]: https://github.com/driftsys/prim/issues/50
[b79c369]: https://github.com/driftsys/prim/commit/b79c369
[#78]: https://github.com/driftsys/prim/issues/78
[d2e1201]: https://github.com/driftsys/prim/commit/d2e1201
[#77]: https://github.com/driftsys/prim/issues/77
[fde8228]: https://github.com/driftsys/prim/commit/fde8228
[#76]: https://github.com/driftsys/prim/issues/76
[089e916]: https://github.com/driftsys/prim/commit/089e916
[#75]: https://github.com/driftsys/prim/issues/75
[d23bd67]: https://github.com/driftsys/prim/commit/d23bd67
[#73]: https://github.com/driftsys/prim/issues/73
[6b92cb9]: https://github.com/driftsys/prim/commit/6b92cb9
[#72]: https://github.com/driftsys/prim/issues/72
[2e57a6a]: https://github.com/driftsys/prim/commit/2e57a6a
[#71]: https://github.com/driftsys/prim/issues/71
[688ea62]: https://github.com/driftsys/prim/commit/688ea62
[#70]: https://github.com/driftsys/prim/issues/70
[2046ba0]: https://github.com/driftsys/prim/commit/2046ba0
[#39]: https://github.com/driftsys/prim/issues/39
[#63]: https://github.com/driftsys/prim/issues/63
[6ef7258]: https://github.com/driftsys/prim/commit/6ef7258
[#43]: https://github.com/driftsys/prim/issues/43
[#67]: https://github.com/driftsys/prim/issues/67
[9dab1b8]: https://github.com/driftsys/prim/commit/9dab1b8
[#57]: https://github.com/driftsys/prim/issues/57
[#68]: https://github.com/driftsys/prim/issues/68
[bf54272]: https://github.com/driftsys/prim/commit/bf54272
[#42]: https://github.com/driftsys/prim/issues/42
[#66]: https://github.com/driftsys/prim/issues/66

## [0.2.2] (2026-07-04)

### Performance

- **prim-cli:** cache the .editorconfig cascade per directory ([4532403])

[0.2.2]: https://github.com/driftsys/prim/compare/v0.2.1...v0.2.2
[4532403]: https://github.com/driftsys/prim/commit/4532403

## [0.2.1] (2026-07-04)

### Documentation

- **prim-cli:** drop .env from the dotfile-discovery comment ([86f1891])

[0.2.1]: https://github.com/driftsys/prim/compare/v0.2.0...v0.2.1
[86f1891]: https://github.com/driftsys/prim/commit/86f1891

## [0.2.0] (2026-07-04)

### Features

- **prim-fmt:** curate orphan allowlist — drop .env, add CODEOWNERS and .mailmap
  ([41d9f06])

### Bug Fixes

- **prim-fmt:** make the fence guard collision-safe ([188992c])
- **prim-fmt:** keep markdown-tagged fenced blocks verbatim (FR-1.6) ([988cfbf])
- **prim-cli:** correct colour help text, exclude error message, and
  explicit-path docs ([67ffb4f])
- **prim-cli:** honor NO_COLOR and key auto colour off stderr ([1945c6e])
- **prim-cli:** reject --stdin-filepath combined with --check/--diff ([e6be4da])
- **prim-cli:** make a malformed --exclude glob a usage error ([45d1dcd])
- **prim-cli:** report explicitly named paths prim cannot process ([273e78f])

### Documentation

- **prim-fmt:** add the style-stability policy ([9afb26c])
- **prim-cli:** document the orphan allowlist in usage ([64943e9])
- **prim-cli:** record CLI hardening in spec and usage ([f4d4e8e])
- **prim-cli:** add golden-file recipe, JSON leniency note, and archive ignores
  ([d54882c])
- **prim-cli:** record charset scope, trim precedence, --diff exit code, JSON
  leniency ([ae1cf1b])
- **prim-fmt:** sync status docs with the implemented v1 reality ([87576bd])
- **prim-cli:** drop system design and ADs from published book ([c0d5896])
- **prim-fmt:** document benchmark usage ([d09ffa0])

[0.2.0]: https://github.com/driftsys/prim/compare/v0.1.0...v0.2.0
[41d9f06]: https://github.com/driftsys/prim/commit/41d9f06
[188992c]: https://github.com/driftsys/prim/commit/188992c
[988cfbf]: https://github.com/driftsys/prim/commit/988cfbf
[67ffb4f]: https://github.com/driftsys/prim/commit/67ffb4f
[1945c6e]: https://github.com/driftsys/prim/commit/1945c6e
[e6be4da]: https://github.com/driftsys/prim/commit/e6be4da
[45d1dcd]: https://github.com/driftsys/prim/commit/45d1dcd
[273e78f]: https://github.com/driftsys/prim/commit/273e78f
[9afb26c]: https://github.com/driftsys/prim/commit/9afb26c
[64943e9]: https://github.com/driftsys/prim/commit/64943e9
[f4d4e8e]: https://github.com/driftsys/prim/commit/f4d4e8e
[d54882c]: https://github.com/driftsys/prim/commit/d54882c
[ae1cf1b]: https://github.com/driftsys/prim/commit/ae1cf1b
[87576bd]: https://github.com/driftsys/prim/commit/87576bd
[c0d5896]: https://github.com/driftsys/prim/commit/c0d5896
[d09ffa0]: https://github.com/driftsys/prim/commit/d09ffa0

## 0.1.0 (2026-07-01)

### Bug Fixes

- **fmt:** disable dprint-core debug assertions so inline-code-with-newline
  never panics ([3d1227f])

### Documentation

- correctness harness done; v1 requirements complete ([#13]) ([bea8f79])
- --diff implemented (FR-5.3); update status ([8c047b7])
- document Markdown formatting + dprint retirement (AD-0006); all formats land
  ([979dce9])
- document YAML formatting (AD-0005) + status ([090393f])
- document TOML formatting (AD-0004) + status ([d78d417])
- document JSON/JSONC formatting (AD-0003) + status ([d0c6491])
- garden durable design + decision records (AD-0001/0002) ([4d06170])
- document .editorconfig resolution and its scope (FR-3) ([9b58d72])

### Features

- **cli:** --diff unified-diff rendering via similar (FR-5.3) ([b6e85c6])
- **fmt:** Markdown formatting + prose wrap via dprint-plugin-markdown
  (FR-1.1/1.1a/1.6) ([8c0252e])
- **fmt:** YAML formatting via pretty_yaml (FR-1.4) ([6c9b1fe])
- **fmt:** TOML formatting via taplo (FR-1.5) ([0475267])
- **fmt:** JSON/JSONC formatting via dprint-plugin-json (FR-1.2/1.3) ([05d47df])
- **cli:** resolve Style from .editorconfig via ec4rs (FR-3) ([04556f5])
- **fmt:** make whitespace hygiene Style-driven (FR-2.3/FR-3.2) ([8770979])
- **fmt:** add resolved Style with canonical default (FR-3.1) ([7f1eef2])
- **write:** atomic writes & non-UTF-8 reporting (FR-6.3/6.4/6.5) ([b1c14b6]),
  closes [#7]
- **fmt:** whitespace hygiene + orphan allowlist (FR-2) ([8b29ebf]), closes [#6]
- **discover:** recursive file discovery (FR-4) ([e4cc239]), closes [#5]
- scaffold Rust workspace and walking-skeleton prim CLI ([bae51c3]), refs [#1],
  [#2]

### Refactoring

- **fmt:** make format fallible with FormatError (FR-6.3) ([155217f])

[3d1227f]: https://github.com/driftsys/prim/commit/3d1227f
[bea8f79]: https://github.com/driftsys/prim/commit/bea8f79
[#13]: https://github.com/driftsys/prim/issues/13
[8c047b7]: https://github.com/driftsys/prim/commit/8c047b7
[979dce9]: https://github.com/driftsys/prim/commit/979dce9
[090393f]: https://github.com/driftsys/prim/commit/090393f
[d78d417]: https://github.com/driftsys/prim/commit/d78d417
[d0c6491]: https://github.com/driftsys/prim/commit/d0c6491
[4d06170]: https://github.com/driftsys/prim/commit/4d06170
[9b58d72]: https://github.com/driftsys/prim/commit/9b58d72
[b6e85c6]: https://github.com/driftsys/prim/commit/b6e85c6
[8c0252e]: https://github.com/driftsys/prim/commit/8c0252e
[6c9b1fe]: https://github.com/driftsys/prim/commit/6c9b1fe
[0475267]: https://github.com/driftsys/prim/commit/0475267
[05d47df]: https://github.com/driftsys/prim/commit/05d47df
[04556f5]: https://github.com/driftsys/prim/commit/04556f5
[8770979]: https://github.com/driftsys/prim/commit/8770979
[7f1eef2]: https://github.com/driftsys/prim/commit/7f1eef2
[b1c14b6]: https://github.com/driftsys/prim/commit/b1c14b6
[#7]: https://github.com/driftsys/prim/issues/7
[8b29ebf]: https://github.com/driftsys/prim/commit/8b29ebf
[#6]: https://github.com/driftsys/prim/issues/6
[e4cc239]: https://github.com/driftsys/prim/commit/e4cc239
[#5]: https://github.com/driftsys/prim/issues/5
[bae51c3]: https://github.com/driftsys/prim/commit/bae51c3
[#1]: https://github.com/driftsys/prim/issues/1
[#2]: https://github.com/driftsys/prim/issues/2
[155217f]: https://github.com/driftsys/prim/commit/155217f
