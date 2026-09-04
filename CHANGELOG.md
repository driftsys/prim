# Changelog

## [0.7.1] (2026-09-04)

### Bug Fixes

- **prim-cli:** write a path to the machine-readable stream as its real bytes
  ([#182]) ([6b3ce77]), closes [#172]. See AD-0019.
- **prim-fmt:** keep a dependency's slug defect out of prim's exit code ([#181])
  ([1088f92]), closes [#180]. See AD-0018.

[0.7.1]: https://github.com/driftsys/prim/compare/v0.7.0...v0.7.1
[6b3ce77]: https://github.com/driftsys/prim/commit/6b3ce77
[#182]: https://github.com/driftsys/prim/issues/182
[#172]: https://github.com/driftsys/prim/issues/172
[1088f92]: https://github.com/driftsys/prim/commit/1088f92
[#181]: https://github.com/driftsys/prim/issues/181
[#180]: https://github.com/driftsys/prim/issues/180

## [0.7.0] (2026-08-30)

### Bug Fixes

- **prim-cli:** hold the exit-code contract through a panic and an undecodable
  argv ([#178]) ([f4bcc74]), closes [#125], [#173]
- **prim-cli:** report an .editorconfig prim could not open ([#177])
  ([de10472]), closes [#153]
- **prim-cli:** leave a named symlink intact instead of destroying it ([#175])
  ([f185b06]), closes [#152], [#166]
- **prim-cli:** classify every changed path before passing over any ([#174])
  ([29abeab]), closes [#169]
- **prim:** own a file whose name is not valid UTF-8 ([#171]) ([c26c829]),
  closes [#168], [#164]

### BREAKING CHANGES

- `prim fmt <symlink>` no longer rewrites the file the link
points at, and `prim fmt --check <symlink>` no longer reports it. `prim init`
exits 2 on a symlinked .editorconfig rather than replacing it. A changed-file
scope no longer selects a file because a symlink to it was staged. A dangling
symlink named on the command line is reported as an unowned path (exit 0)
rather than as a missing file (exit 2).
- a path staged as a modification and then removed from the
working tree now exits 2 where it was previously skipped in silence. prim was
pointed at content it cannot read, and the commit that follows would record
bytes prim never examined.

`--since` and `--staged` also require git 2.24 or newer, which `--end-of-options`
already required of `--since`.
- prim now owns files it previously left byte-for-byte
unchanged. On a platform whose filenames are byte strings, a name that is not
valid UTF-8 with an owned extension, or matching the orphan allowlist, is
formatted like any other — so `prim fmt .` rewrites files it used to skip, and
`prim fmt --check .` reports them. This is not limited to `--since`/`--staged`:
the classification rule governs every invocation.

Two limits remain, both recorded in FR-2.5: prim reports such a path through

[0.7.0]: https://github.com/driftsys/prim/compare/v0.6.0...v0.7.0
[f4bcc74]: https://github.com/driftsys/prim/commit/f4bcc74
[#178]: https://github.com/driftsys/prim/issues/178
[#125]: https://github.com/driftsys/prim/issues/125
[#173]: https://github.com/driftsys/prim/issues/173
[de10472]: https://github.com/driftsys/prim/commit/de10472
[#177]: https://github.com/driftsys/prim/issues/177
[#153]: https://github.com/driftsys/prim/issues/153
[f185b06]: https://github.com/driftsys/prim/commit/f185b06
[#175]: https://github.com/driftsys/prim/issues/175
[#152]: https://github.com/driftsys/prim/issues/152
[#166]: https://github.com/driftsys/prim/issues/166
[29abeab]: https://github.com/driftsys/prim/commit/29abeab
[#174]: https://github.com/driftsys/prim/issues/174
[#169]: https://github.com/driftsys/prim/issues/169
[c26c829]: https://github.com/driftsys/prim/commit/c26c829
[#171]: https://github.com/driftsys/prim/issues/171
[#168]: https://github.com/driftsys/prim/issues/168
[#164]: https://github.com/driftsys/prim/issues/164

## [0.6.0] (2026-08-29)

### Bug Fixes

- **prim-cli:** pin git's argument and output handling for changed files
  ([#170]) ([64bc381]), closes [#165], [#167]

### BREAKING CHANGES

- `prim fmt --check` under `--since` or `--staged` now reports
files it previously skipped without saying so — any path git would C-quote,
which is every non-ASCII UTF-8 name and every name holding a control character,
and every path at all when `diff.relative=true` is set and prim runs from a
subdirectory. A gate that passed on such a repository will now fail until the
files are formatted. A `<REF>` git cannot resolve as a revision now exits 2
instead of taking effect as an option or a pathspec, and a `<REF>` that names
both a branch and a path now resolves to the branch rather than being
ambiguous. `--since` therefore requires git 2.24 or newer, for
`--end-of-options`; `--staged` carries no REF and has no such floor.

A path whose bytes are not valid UTF-8 is still dropped (#168): git's output is
read through `String::from_utf8_lossy`, which is unchanged here. That leaves
#164's fail-open reachable on a filesystem permitting such names, so #164 stays
open until #168 lands.

[0.6.0]: https://github.com/driftsys/prim/compare/v0.5.0...v0.6.0
[64bc381]: https://github.com/driftsys/prim/commit/64bc381
[#170]: https://github.com/driftsys/prim/issues/170
[#165]: https://github.com/driftsys/prim/issues/165
[#167]: https://github.com/driftsys/prim/issues/167

## [0.5.0] (2026-08-29)

### Documentation

- **prim:** garden the mdlint tier-model working memory ([#161]) ([6edddd8])
- **prim:** refresh three stale implementation statuses in AD-0002 ([#157])
  ([4bebce5]), closes 156.
- **prim:** correct what prim reports about a broken .editorconfig ([df3ca85])
- **prim:** garden the generated-file protection working memory ([#145])
  ([14a9a9d])
- **prim:** add the incremental adoption recipe for --since ([#139])
  ([2a88953]), closes [#137]
- **prim:** track docs/wip and docs/archive in git ([538c4c5])

### Features

- **prim:** report Markdown line length via prim_mdlint_report_line_length
  ([#160]) ([692a209]), closes [#123]
- **prim-cli:** fail a gate that was pointed only at skipped paths ([#142])
  ([63726da]), closes [#112]
- **prim-cli:** exempt docs/archive from the strict tier and lint prim's own
  docs ([#136]) ([4fa8dcc])

### Refactoring

- **prim-cli:** split app.rs along its dispatch and pipeline seams ([#141])
  ([3523ea7])

### Bug Fixes

- **prim-cli:** warn that a --staged write did not update the index ([#162])
  ([4893639]), closes 159.
- **prim-cli:** write .editorconfig with one line ending, not two ([#151])
  ([dc4bc5e])
- **prim-cli:** climb a symlinked spelling toward the working directory ([#148])
  ([6084e29]), closes [#113]
- **prim-fmt:** remove MD057 from the Markdown lint rule set ([#150])
  ([380f2b2]), closes [#134]
- **prim-cli:** apply gitignore's re-inclusion rule to a named path ([#143])
  ([6c5ce51]), closes [#114]
- **prim-cli:** report a canonical section a later broader one swallows ([#140])
  ([15adfa2])
- **prim-cli:** report parent cascade prim init cuts off ([#132]) ([276c058])
- **prim-cli:** parse editorconfig section headers like ec4rs ([#128])
  ([3a1c728])
- **prim:** stop the debug-build panic on a Unicode space after an ASCII space
  ([#124]) ([b3c38f2])

### BREAKING CHANGES

- `prim_fmt::lint_markdown` takes a fourth parameter, the
resolved line-length limit. Pass `None` for the previous behaviour.
- a .primignore at or below the outermost directory holding a
symlinked spelling that resolves inside the working directory now applies to
that spelling, where it was silently missed before.
- a `!` rule under an excluded directory no longer re-includes
the file it names when that file is named on the command line. A repository
that has one already gets the excluded result from a walk. This also ends the
AD-0011 generated-file override for a lockfile whose parent directory the same
.primignore excludes.
- prim fmt --check over paths that are all ignored exits 2
rather than 0, and so do fix --check, fix --diff, --check-idempotence, and
lint.

[0.5.0]: https://github.com/driftsys/prim/compare/v0.4.0...v0.5.0
[6edddd8]: https://github.com/driftsys/prim/commit/6edddd8
[#161]: https://github.com/driftsys/prim/issues/161
[4bebce5]: https://github.com/driftsys/prim/commit/4bebce5
[#157]: https://github.com/driftsys/prim/issues/157
[df3ca85]: https://github.com/driftsys/prim/commit/df3ca85
[14a9a9d]: https://github.com/driftsys/prim/commit/14a9a9d
[#145]: https://github.com/driftsys/prim/issues/145
[2a88953]: https://github.com/driftsys/prim/commit/2a88953
[#139]: https://github.com/driftsys/prim/issues/139
[#137]: https://github.com/driftsys/prim/issues/137
[538c4c5]: https://github.com/driftsys/prim/commit/538c4c5
[692a209]: https://github.com/driftsys/prim/commit/692a209
[#160]: https://github.com/driftsys/prim/issues/160
[#123]: https://github.com/driftsys/prim/issues/123
[63726da]: https://github.com/driftsys/prim/commit/63726da
[#142]: https://github.com/driftsys/prim/issues/142
[#112]: https://github.com/driftsys/prim/issues/112
[4fa8dcc]: https://github.com/driftsys/prim/commit/4fa8dcc
[#136]: https://github.com/driftsys/prim/issues/136
[3523ea7]: https://github.com/driftsys/prim/commit/3523ea7
[#141]: https://github.com/driftsys/prim/issues/141
[4893639]: https://github.com/driftsys/prim/commit/4893639
[#162]: https://github.com/driftsys/prim/issues/162
[dc4bc5e]: https://github.com/driftsys/prim/commit/dc4bc5e
[#151]: https://github.com/driftsys/prim/issues/151
[6084e29]: https://github.com/driftsys/prim/commit/6084e29
[#148]: https://github.com/driftsys/prim/issues/148
[#113]: https://github.com/driftsys/prim/issues/113
[380f2b2]: https://github.com/driftsys/prim/commit/380f2b2
[#150]: https://github.com/driftsys/prim/issues/150
[#134]: https://github.com/driftsys/prim/issues/134
[6c5ce51]: https://github.com/driftsys/prim/commit/6c5ce51
[#143]: https://github.com/driftsys/prim/issues/143
[#114]: https://github.com/driftsys/prim/issues/114
[15adfa2]: https://github.com/driftsys/prim/commit/15adfa2
[#140]: https://github.com/driftsys/prim/issues/140
[276c058]: https://github.com/driftsys/prim/commit/276c058
[#132]: https://github.com/driftsys/prim/issues/132
[3a1c728]: https://github.com/driftsys/prim/commit/3a1c728
[#128]: https://github.com/driftsys/prim/issues/128
[b3c38f2]: https://github.com/driftsys/prim/commit/b3c38f2
[#124]: https://github.com/driftsys/prim/issues/124

## [0.4.0] (2026-08-25)

### Refactoring

- **prim-cli:** extract Markdown lint policy resolution ([fbcaa27])

### Features

- **prim-cli:** exempt docs/wip from the strict glob in prim init ([0e4fd05])
- **prim-cli:** show prim_mdlint_disable in prim explain ([0f54fe5])
- **prim-cli:** add prim_mdlint_disable for per-glob rule exclusion ([8895773])
- **prim-fmt:** re-place Markdown lint rules into defect and convention bands
  ([8473cfe])

### Bug Fixes

- **prim-cli:** check prim init's success claim and its own map by outcome
  ([da7b20b])
- **prim-cli:** decide prim init writes by what the file resolves to ([310d6ba])
- **prim-cli:** stop prim init from writing into a section it just flagged as
  out of order ([c46d622])
- **prim-cli:** detect out-of-order sections already present in prim init
  ([9d77c58])
- **prim-cli:** refuse to insert docs/wip when .editorconfig sections are out of
  order ([1c7f062])
- **prim-cli:** report unknown prim_mdlint_disable ids once per run ([d9480ca])

### Documentation

- **prim:** fix rumdl-disable scope and amend AD-0007's severity note
  ([4c9d6b8])
- **prim:** record the Markdown lint bands and rule exclusion ([21e51a4])

### BREAKING CHANGES

- prim lint now exits 1 on findings that were previously warning-severity, and
  MD082 is no longer reported.
- `prim_fmt::lint_markdown` takes a third parameter, the list of rule ids to
  exclude. Pass an empty slice to keep the previous behaviour.

[0.4.0]: https://github.com/driftsys/prim/compare/v0.3.2...v0.4.0
[fbcaa27]: https://github.com/driftsys/prim/commit/fbcaa27
[0e4fd05]: https://github.com/driftsys/prim/commit/0e4fd05
[0f54fe5]: https://github.com/driftsys/prim/commit/0f54fe5
[8895773]: https://github.com/driftsys/prim/commit/8895773
[8473cfe]: https://github.com/driftsys/prim/commit/8473cfe
[da7b20b]: https://github.com/driftsys/prim/commit/da7b20b
[310d6ba]: https://github.com/driftsys/prim/commit/310d6ba
[c46d622]: https://github.com/driftsys/prim/commit/c46d622
[9d77c58]: https://github.com/driftsys/prim/commit/9d77c58
[1c7f062]: https://github.com/driftsys/prim/commit/1c7f062
[d9480ca]: https://github.com/driftsys/prim/commit/d9480ca
[4c9d6b8]: https://github.com/driftsys/prim/commit/4c9d6b8
[21e51a4]: https://github.com/driftsys/prim/commit/21e51a4

## [0.3.2] (2026-08-24)

### Bug Fixes

- **prim-cli:** bound primignore at the repository prim was pointed at
  ([ce90f4a]), closes [#110]

[0.3.2]: https://github.com/driftsys/prim/compare/v0.3.1...v0.3.2
[ce90f4a]: https://github.com/driftsys/prim/commit/ce90f4a
[#110]: https://github.com/driftsys/prim/issues/110

## [0.3.1] (2026-08-21)

### Documentation

- **prim:** correct AD-0009's primignore search bound ([2bee350])
- **prim:** record why prim declines generated files ([c83e754])

### Bug Fixes

- **prim-cli:** guard LSP diagnostics against generated files ([b5b6ae7])
- **prim-cli:** tighten the primignore whitelist and generated-path checks
  ([13b5e7b])
- **prim-cli:** decline a generated file on lint over stdin ([3859bcc])
- **prim-cli:** apply editorconfig indent_size without indent_style ([9aae9a2]),
  closes [#104]

### Features

- **prim-cli:** decline generated files on stdin and over LSP ([9537bfe])
- **prim-cli:** skip generated files during discovery ([88e36b0])
- **prim-fmt:** add the generated-file predicate ([05b014d])
- **prim-fmt:** add .gitmodules to the orphan allowlist ([2f7e973])
- **prim-fmt:** keep the array line structure the source wrote ([f28598b]),
  closes [#105]

[0.3.1]: https://github.com/driftsys/prim/compare/v0.3.0...v0.3.1
[2bee350]: https://github.com/driftsys/prim/commit/2bee350
[c83e754]: https://github.com/driftsys/prim/commit/c83e754
[b5b6ae7]: https://github.com/driftsys/prim/commit/b5b6ae7
[13b5e7b]: https://github.com/driftsys/prim/commit/13b5e7b
[3859bcc]: https://github.com/driftsys/prim/commit/3859bcc
[9aae9a2]: https://github.com/driftsys/prim/commit/9aae9a2
[#104]: https://github.com/driftsys/prim/issues/104
[9537bfe]: https://github.com/driftsys/prim/commit/9537bfe
[88e36b0]: https://github.com/driftsys/prim/commit/88e36b0
[05b014d]: https://github.com/driftsys/prim/commit/05b014d
[2f7e973]: https://github.com/driftsys/prim/commit/2f7e973
[f28598b]: https://github.com/driftsys/prim/commit/f28598b
[#105]: https://github.com/driftsys/prim/issues/105

## [0.3.0] (2026-08-12)

### Features

- **prim-cli:** honor .primignore for explicitly named paths ([90af908]), closes
  [#98]
- **prim-fmt:** keep prose off an HTML comment's closing line ([2e375f6]),
  closes [#97]
- **prim-fmt:** honor max_line_length for arrays inside inline tables
  ([ce490c1]), closes [#96]

### BREAKING CHANGES

- `prim fmt <path>` no longer rewrites a file covered by
`.primignore`. Pass `--no-primignore` to restore the previous behaviour.

[0.3.0]: https://github.com/driftsys/prim/compare/v0.2.4...v0.3.0
[90af908]: https://github.com/driftsys/prim/commit/90af908
[#98]: https://github.com/driftsys/prim/issues/98
[2e375f6]: https://github.com/driftsys/prim/commit/2e375f6
[#97]: https://github.com/driftsys/prim/issues/97
[ce490c1]: https://github.com/driftsys/prim/commit/ce490c1
[#96]: https://github.com/driftsys/prim/issues/96

## [0.2.4] (2026-07-26)

### Features

- **prim-cli:** add cargo audit gate for supply-chain advisories (F2) ([#89])
  ([9dd9b90])

[0.2.4]: https://github.com/driftsys/prim/compare/v0.2.3...v0.2.4
[9dd9b90]: https://github.com/driftsys/prim/commit/9dd9b90
[#89]: https://github.com/driftsys/prim/issues/89

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
