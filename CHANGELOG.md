# Changelog

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

- **prim-fmt:** curate orphan allowlist — drop .env, add CODEOWNERS and
  .mailmap ([41d9f06])

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
