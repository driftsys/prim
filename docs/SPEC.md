# Specification (v1)

> This is the human-readable v1 requirements specification for prim. It
> supersedes [issue #1](https://github.com/driftsys/prim/issues/1), which was
> closed once this document formalized the v1 scope it originally proposed. Code
> and tests remain the source of truth; this document describes the intended
> system.

## Identity

prim is a single-binary, opinionated, near-zero-config formatter for a
repository's connective tissue — Markdown, JSON/JSONC, YAML, TOML — plus
whitespace hygiene on a curated set of un-owned text files. It is **not** a
source-code formatter and has **no plugin system**.

## Settled decisions

| Fork             | Decision                                                               |
| ---------------- | ---------------------------------------------------------------------- |
| Scope            | Config/docs/data only (md, json/jsonc, yaml, toml). No source code.    |
| Config           | One canonical style; honor `.editorconfig`. No `prim.toml`.            |
| Ordering         | Never reorder keys/entries/arrays (semantics-preserving).              |
| Other text files | Hygiene on a curated orphan allowlist, never on source.                |
| Markdown wrap    | Hard-wrap prose to width (`.editorconfig` `max_line_length`, else 80). |
| JSON5            | Excluded (JSONC covers comment needs).                                 |
| `.primignore`    | Yes — committed escape hatch (gitignore syntax); covers named paths.   |
| Make / Shell     | Out of v1 allowlist; shell deferred to Phase 2 (shfmt/wasm).           |

## FR-1 — Structured formatting

- **FR-1.1** prim shall format Markdown to one canonical style (ATX headings,
  normalized list markers, normalized table padding, normalized blank-line
  spacing) and hard-wrap paragraph prose to the line width — `max_line_length`
  from `.editorconfig`, else 80.
- **FR-1.1a** _(wrap guardrails)_ prim shall wrap prose paragraphs only; it
  shall not break inside inline code, shall not split a URL or link, shall not
  wrap tables or fenced code blocks, shall not move text onto the closing line
  of an HTML comment that ended a line in the input (CommonMark parses the rest
  of that line as raw HTML), and shall preserve explicit hard line breaks
  (trailing `\` or two-space).
- **FR-1.2** prim shall format JSON to a canonical style (consistent
  indentation, one space after `:`, no trailing commas).
- **FR-1.3** prim shall format JSONC, preserving all comments in position.
  `.json` files are parsed with the same lenient JSONC parser: comments and
  trailing commas are accepted on input and never emitted (AD-0003). (JSON5
  excluded.)
- **FR-1.4** prim shall format YAML, preserving comments, anchors/aliases, and
  multi-line scalar styles.
- **FR-1.5** prim shall format TOML, preserving comments and inline-table style.
- **FR-1.5a** _(array layout)_ prim shall expand an array that exceeds the
  resolved line width to one element per line, and shall otherwise preserve the
  array's line structure as written — an array on one line stays on one line, an
  array across several lines is never collapsed (AD-0010).
- **FR-1.6** prim shall preserve fenced code-block contents verbatim (no
  reformatting of embedded source).

## FR-2 — Text hygiene (parsed formats + orphan allowlist)

- **FR-2.1** For every file it processes, prim shall remove trailing whitespace
  from each line, unless `.editorconfig` sets `trim_trailing_whitespace = false`
  (FR-3.2 takes precedence).
- **FR-2.2** prim shall ensure each processed file ends with exactly one
  line-feed.
- **FR-2.3** prim shall normalize line endings to LF, unless `.editorconfig`
  sets `end_of_line = crlf`.
- **FR-2.4** _(scope)_ prim shall process only (a) the parsed formats
  (md/json/jsonc/yaml/toml) and (b) a built-in orphan allowlist of un-owned text
  files. Every other file — recognized source code, unknown types, binaries — is
  left byte-for-byte unchanged.
- **FR-2.5** prim shall identify allowlisted files by filename/extension, not
  content sniffing. Neither test shall require the filename to be valid UTF-8:
  prim shall read the extension before it needs the whole name, and shall match
  the orphan allowlist on the filename's bytes. A file whose name is not valid
  UTF-8 is therefore owned, and formatted, exactly as the same name would be
  were it decodable (#168), whether prim reached it by directory walk or through
  `--since`/`--staged`, and whether it was named on the command line (#173).
  prim still reports such a path lossily (#172).
- **FR-2.6** prim shall strip a leading UTF-8 BOM (`U+FEFF`), unconditionally,
  from every file it processes.
- **FR-2.7** prim shall leave a tool-generated file byte-for-byte unchanged,
  including whitespace hygiene, even when its type is one prim formats. The set
  is a built-in, name-keyed list (AD-0011). A committed `.primignore` negation
  re-includes such a file only when its final path segment is a literal file
  name equal to the generated file's name (for example `!package-lock.json`,
  `!**/package-lock.json`, or `!vendor/package-lock.json`) — a broader negation
  such as `!*.json` or `!*` does not, and no negation re-includes a file that a
  directory exclusion above it already covers (FR-4.4). The `.primignore` files
  consulted for a path are bounded as FR-4.4b specifies, so a `.primignore`
  outside the repository cannot disable the built-in list for every repository
  beneath it. `--no-primignore` disables the built-in list along with the rest
  of the `.primignore` stack.

## FR-3 — Style resolution

- **FR-3.1** prim shall apply its built-in canonical style with no config file
  present.
- **FR-3.2** prim shall read `.editorconfig` and honor `indent_style`,
  `indent_size`, `max_line_length`, `end_of_line`, `insert_final_newline`, and
  `trim_trailing_whitespace` through the normal `root=true` chain and per-glob
  sections. (`charset` is out of scope: prim processes UTF-8 only — FR-6.5,
  AD-0002.)
- **FR-3.2a** prim shall also read a small, closed, documented set of namespaced
  `prim_*` keys from the same `.editorconfig` cascade. The current set contains
  three keys: `prim_mdlint_strict = true|false` (default `false`) for Markdown
  lint-tier selection, `prim_mdlint_disable = <rule ids>` (FR-3.2c) for
  excluding named Markdown lint rules from the tier selected for a matching
  path, and `prim_mdlint_report_line_length = true|false` (default `false`,
  FR-3.2d) for reporting Markdown lines longer than the resolved
  `max_line_length`.
- **FR-3.2b** Any other `prim_*` key is ignored silently. Unknown custom keys
  must never error, widen the public configuration surface, or interfere with
  standard EditorConfig-key resolution.
- **FR-3.2c** `prim_mdlint_disable`'s value shall be a comma-separated list of
  rule ids, resolved through the same per-glob `.editorconfig` cascade as
  `prim_mdlint_strict`: EditorConfig's ordinary last-match-wins resolution
  applies per section, so a narrower section's value replaces a wider section's
  list rather than merging with it. Rule ids shall match case-insensitively. The
  key is subtract-only: it shall remove named rules from the tier already
  selected for that path and shall never add a rule prim did not already select
  for that tier. A value of `unset` (EditorConfig's own reserved word) or `none`
  shall clear the list rather than name a rule, and shall not be reported as
  unrecognised. An id that names no rule prim can run in either tier disables
  nothing; prim shall report it on stderr, naming the `.editorconfig` file, line
  and section that set it, once per run for each section that carries it,
  without changing the exit code.
- **FR-3.2d** `prim_mdlint_report_line_length = true` shall select MD013 into
  whichever Markdown lint tier FR-5.5b already resolved for that path, measured
  against the `max_line_length` resolved for the same file (80 when unset or
  `off` — the formatter's own fallback, which treats both the same way), so that
  for every width prim can wrap to, no `.editorconfig` cascade makes
  `prim fmt`'s wrap width and `prim lint`'s reported width differ. Two values
  fall outside that range and are documented rather than reconciled:
  `max_line_length = 0`, which the formatter treats as one word per line while
  the rule is clamped to 1, and a value above `u32::MAX`, which the formatter
  truncates. A file carrying rumdl's own `markdownlint-configure-file` directive
  is the one exception, and the same escape hatch AD-0012 already documents: it
  can change MD013's options for that file, the width included, but it cannot
  select MD013 where this key has not. It resolves through the same per-glob
  cascade as `prim_mdlint_strict`. It shall not change which tier applies, and
  `prim_mdlint_disable = MD013` shall remove the rule again like any other.
- **FR-3.3** prim shall expose no other style configuration (no `prim.toml`, no
  per-rule flags, and no way for a repository to configure a rule's options),
  with two named exceptions, both feeding MD013 (FR-3.2d) and no other rule:
  `max_line_length` supplies its `line-length`, and `prim_mdlint_strict`
  supplies its `headings`. Each is a value a repository already sets for another
  purpose — the formatter's wrap width, and the lint tier — reused so that the
  setting and the rule cannot disagree. Neither is a per-rule dial, and no other
  option of any rule is reachable. `prim_mdlint_disable` (FR-3.2c) is not an
  exception either: it only ever narrows the rule set prim already selected,
  never widens it or changes a rule's behaviour.
- **FR-3.4** prim shall never reorder keys, table entries, or array elements.
- **FR-3.5** prim shall provide `prim init [PATH]` as a one-time `.editorconfig`
  scaffolder for Markdown lint-tier placement. With no existing `.editorconfig`,
  it writes exactly:

  ```ini
  root = true
  [*.md]
  prim_mdlint_strict = false
  [docs/**.md]
  prim_mdlint_strict = true
  [docs/wip/**.md]
  prim_mdlint_strict = false
  [docs/archive/**.md]
  prim_mdlint_strict = false
  [**/SUMMARY.md]
  prim_mdlint_strict = false
  ```

  When `PATH/book.toml` exists, the strict glob shall use mdBook's `[book].src`
  directory instead of `docs/`, defaulting to `src/**.md` when the key is absent
  or the TOML is malformed. `[docs/wip/**.md]` and `[docs/archive/**.md]` are
  literals, not derived from the strict glob: they hold Superpowers working
  memory, so the strict tier must not apply to them even when the strict glob
  covers `docs/**` or a custom mdBook `src` directory. Specs and plans under
  `docs/wip/` are transient; gardening moves their raw originals to
  `docs/archive/`. That move is not an edit, so it must not change a document's
  tier — without the second exemption, filing work away is what would make a
  repository's own CI start failing on it. An exemption whose directory the
  strict glob is rooted at, or inside, is not written: there the exemption would
  be the broader glob and would turn the whole book back off.

  When `PATH/.editorconfig` already exists, `prim init` shall merge minimally
  and never reorder or rewrite unrelated bytes, with one exception: it shall
  write the whole file under one line ending, the `end_of_line` that resolves
  for `.editorconfig` once the write has been made (FR-2.3). Composing LF
  additions into a CRLF file would otherwise leave mixed endings, and carrying
  the existing endings through would leave a uniformly CRLF file that
  `prim fmt --check` reports. The ending must be resolved after the write and
  not before, because the `root = true` `prim init` writes stops EditorConfig's
  upward walk, so an `end_of_line` an ancestor declared before the write no
  longer reaches the file afterwards. Otherwise: leave an existing top-level
  `root = ...` untouched, otherwise prepend `root = true` plus one blank line;
  for `[*.md]`, the detected strict glob, `[docs/wip/**.md]`, and
  `[**/SUMMARY.md]`, leave any existing explicit `prim_mdlint_strict = ...`
  untouched, append the missing key inside an existing section, and insert any
  missing section without moving existing bytes so the final relative order
  still reads `[*.md]` → strict glob → `[docs/wip/**.md]` → `[**/SUMMARY.md]`
  (appending at end-of-file only when no later prim section needs to stay after
  it). `prim init` shall check each write against the file it would produce
  before making it. For one representative path per canonical section it places
  — a top-level file, a file under the strict glob, a file under `docs/wip/`
  where that section applies, and a `SUMMARY.md` — it shall resolve
  `prim_mdlint_strict` through EditorConfig's last-match-wins section order, and
  shall make a write only when the path that write is meant to place resolves to
  the value prim intended and every other representative path resolves exactly
  as it did before the run. A write that fails that check shall not be made:
  `prim init` shall print a warning naming the path that would resolve
  differently and the value it would take, and shall still make whatever other
  writes pass the check.

  `root = true` stops EditorConfig's upward walk for every key and every file
  type, not just prim's own, so writing it into a nested directory drops
  whatever a parent configured. Whenever `prim init` writes `root = true` —
  scaffolding a new file or prepending to an existing one — and the target
  directory currently reaches an `.editorconfig` above it that sets at least one
  key in a section, it shall warn, naming those files and the keys that no
  longer reach the directory. An ancestor that sets nothing but `root` is left
  out: cutting the walk off from it loses nothing. Keys written before any
  section header are left out too, because EditorConfig does not apply them and
  prim never resolved them. The write is still made; the prepend is mandated and
  `prim init` cannot know whether the author wants the parent cascade, so this
  is a warning and never a refusal.

  When a section is missing and the file's own existing section order already
  contradicts the canonical order — for example an existing `[**/SUMMARY.md]`
  written before the strict glob — no position satisfies that order; `prim init`
  shall leave that section out and print a warning naming the two conflicting
  sections, the lines they start at, and where the section has to be added by
  hand. `prim init` shall then resolve the file it leaves behind: for every
  canonical section present in it that carries `prim_mdlint_strict`, it shall
  warn when that section does not decide its own representative paths — either
  because a later section overrides it, or because its value is one prim does
  not read as a tier (every value but `true` resolves to `false`). It shall warn
  when the section decides none of its representative paths, not merely when a
  later section happens to set a different value for one of them: a later
  section that agrees on value but still decides the path leaves the earlier
  section deciding none of its own representatives, and that must be reported
  too. To tell that case apart from a person's narrower override that still
  leaves the section deciding other paths, `prim init` resolves one further
  representative per canonical section, never named in any message. That covers
  a section prim planned no write for because the key was already there, and
  equally one that prim's own write has just taken a path away from. It is a
  warning, never a refusal: a person's narrower override is legitimate, and
  `prim init` shall stay able to report on a file it disagrees with. It shall
  never reorder sections a person wrote. An occurrence of a canonical glob that
  neither sets `prim_mdlint_strict` nor receives it — an ordinary
  `[*.md] max_line_length = 80` — shall take no part in that ordering. Running
  `prim init` twice shall be a byte-identical no-op on the second run.

## FR-4 — File discovery

- **FR-4.1** prim shall default to the current working directory, recursively,
  when given no paths.
- **FR-4.2** prim shall respect `.gitignore`, `.git/info/exclude`, global
  gitignore rules, and `.ignore` (via the `ignore` crate) without invoking git,
  and shall function in non-git directories.
- **FR-4.2a** `--no-ignore` shall disable only the git-family ignore rules from
  FR-4.2 (`.gitignore`, global gitignore, `.git/info/exclude`). It shall not
  disable `.primignore`, CLI `--exclude` globs, or the `.git/`
  metadata-directory prune.
- **FR-4.2b** `--since <REF>` and `--staged` shall further restrict the matched
  file set through git-native diffs, and they shall be mutually exclusive.
  `--since <REF>` uses plain two-way `git diff --name-only <REF>` semantics:
  paths that differ between `<REF>` and the current working tree, including both
  staged and unstaged changes, with no merge-base (`...`) comparison. `--staged`
  uses `git diff --name-only --cached` semantics: paths staged in the index
  relative to `HEAD`. Both intersect with FR-4.2/4.2a/4.3/4.4/4.5 filtering
  rather than replacing it, silently drop paths git reports as deletions, and
  shall raise a usage error (exit `2`) if git is unavailable, the current
  working directory is not inside a git working tree, or `<REF>` is invalid.
- **FR-4.2c** When `--staged` selects the file set and prim writes at least one
  file, prim shall emit exactly one warning on stderr, reporting how many files
  it wrote to the working tree and that `--staged` does not update the index
  (#159). The warning shall name `git diff`, which shows the unstaged result it
  just produced, and shall not name `git diff --cached`, which compares the
  index against `HEAD` and so cannot show that result at all.
- **FR-4.2d** prim shall not re-stage a file it wrote under `--staged`, and
  shall not state what the index holds. prim never reads the staged blob: for a
  partially staged file that blob may already be canonical, and `git add` would
  also stage the remainder the author kept out of the commit. Re-staging belongs
  to the hook runner.
- **FR-4.2e** No git configuration and no path spelling shall change which files
  `--since` or `--staged` select. prim shall read raw NUL-separated paths, so no
  quoting form hides a path (#164), and shall pin `diff.relative=false`, so the
  paths git reports are the ones prim resolves (#165). prim shall treat those
  paths as bytes rather than as text, so on a platform whose filenames are byte
  strings a name that is not valid UTF-8 selects, classifies and formats like
  any other (#168). prim reports such a path lossily, so the machine-readable
  list on stdout may name a file that cannot be opened (#172). Where a path
  cannot be represented at all — a platform whose filenames are Unicode, given
  output that is not valid UTF-8 — prim shall raise a usage error (exit `2`)
  naming that path, rather than drop it. prim shall raise a usage error (exit
  `2`), rather than resolve an empty file set, when git reports output that is
  not NUL-separated. Output that is empty is a legitimately empty selection and
  shall stay a clean run.

- **FR-4.2f** No `<REF>` shall reach git as anything but a revision. prim shall
  exit `2` for a `<REF>` git will not resolve, including one beginning with `-`
  (#167), one naming an existing file, and `--` itself, and shall never let such
  a `<REF>` take effect as a git option or as a pathspec. `--since` therefore
  requires git 2.24 or newer, which is when `--end-of-options` reached the
  revision parser.
- **FR-4.2g** prim shall ask git for the status of each changed path and shall
  classify every one before passing over any (#169, AD-0015). A path that
  resolves on disk shall be selected whatever its status, so a staged deletion
  of a file still present is still formatted. A path that is absent shall be
  passed over in silence when git reports it as a deletion, when something other
  than a regular file is at it, or when the index will not materialise it —
  `skip-worktree`, which is how sparse checkout is implemented, or
  `assume-unchanged`. A path that is absent, tracked, and of a type prim owns
  shall raise a usage error (exit `2`) within the paths prim was pointed at. A
  path git reports but does not track shall raise a usage error wherever prim
  was pointed, because no repository state produces one.
- **FR-4.3** prim shall process explicit file/directory path arguments.
- **FR-4.4** prim shall respect a committed `.primignore` (gitignore syntax) for
  every path it is given, whether reached by a directory walk or named on the
  command line (AD-0009). `--no-primignore` shall process ignored paths anyway.
  gitignore's re-inclusion rule shall hold on both routes: a `!` rule shall not
  re-include a path when a directory holding it is excluded, however near that
  path the rule is written, including in a `.primignore` beneath the excluded
  directory (#114).
- **FR-4.4a** prim shall report on stderr each path that was named on the
  command line and skipped, whether because `.primignore` covers it or because
  it matches the built-in generated-file list (FR-2.7, AD-0011). Skipping a path
  reached by a directory walk shall be silent for either reason. Where no path
  is named, the working directory is the path prim was pointed at, and skipping
  it shall be reported the same way.
- **FR-4.4b** The `.primignore` files that apply to a path shall be those from
  its own directory up to a bound, and none above that bound. The bound shall be
  the root of the repository containing whatever prim was pointed at — a path
  named on the command line, or the root of a directory walk — resolved once and
  applied to every path considered under it. A repository root is the nearest
  directory holding a `.git` entry, which is a directory in an ordinary clone
  and a file in a git worktree. Only where no repository is found shall the
  bound be the current working directory instead. Consequently a nested checkout
  is governed by the enclosing repository's `.primignore` when prim is pointed
  at the enclosing repository, and by its own when prim is pointed at the
  checkout. Where the working directory is the bound, the bound shall be the
  outermost directory holding the path that resolves inside the working
  directory with every directory between it and the path resolving inside it
  too, named as the path names it (#113). Matching itself shall stay lexical — a
  rule naming a symlinked directory covers the paths written through it — so a
  path is never judged under a spelling other than the one prim was given, and a
  `.primignore` that the path as spelled does not pass shall not apply to it
  however that path resolves.
- **FR-4.4c** In a gate mode — one whose exit code gates on pending findings:
  `fmt --check`, `fix --check`, `fix --diff`, `fmt --check-idempotence`, `lint`
  — prim shall exit `2` when every path it was pointed at was skipped (FR-4.4a):
  each path named on the command line, or the working directory when none was
  named. A `0` there would report a clean run over files prim never looked at,
  which is the assurance a CI gate reads. The modes that write (`fmt`, `fix`)
  and the preview mode (`fmt --diff`) shall exit `0` in the same case, so a
  pre-commit hook handed a staged list of ignored paths still passes (AD-0009).
  Skipping only some of the paths given shall not raise the exit code in any
  mode.
- **FR-4.5** prim shall accept CLI exclude globs; a malformed glob is a usage
  error (exit `2`).
- **FR-4.6** prim shall handle an explicitly named path strictly: a path that
  does not exist shall be reported as an error (exit `2`); an existing path
  whose type prim does not own shall be reported as a warning and left unchanged
  (exit `0`). An unowned path reached only by directory walking shall be skipped
  silently (FR-2.4).
- **FR-4.6a** A symbolic link is a path type prim does not own (AD-0016). prim
  shall not write to one. Where a link is itself the file prim would read or
  write — a path named on the command line in any verb or gate, a path a
  changed-file scope reports, or the `.editorconfig` `prim init` would write —
  the link shall be left byte-for-byte unchanged and the file it points at shall
  be neither read nor written. Naming one shall be reported (FR-4.6) and shall
  not count toward FR-4.4c. This overrides FR-4.6's missing-path rule for a
  **dangling** link: it is declined for its type, so it exits `0` rather than
  `2`. `prim init` is the one route that exits `2`, because it was asked to
  write one named file and did not.
- **FR-4.6b** Two shapes are outside FR-4.6a, both because they end at a regular
  file and so destroy no link. A path that passes **through** a symlinked
  directory shall be processed normally, and the `.primignore` reach limit that
  follows from FR-4.4b's lexical matching stands as decided. A **symlinked
  directory named directly** shall likewise be walked: refusing it would refuse
  `prim fmt /tmp/...` on a platform where `/tmp` is itself a link. A walk shall
  not descend into a symlinked directory in either case (FR-2.4), so naming one
  and walking past it answer about different trees. The answer follows what prim
  was pointed at — the same rule FR-4.4b applies to a nested checkout.

## FR-5 — Operating modes (CLI)

prim exposes three verbs (AD-0007): `fmt`, `lint`, `fix`. Bare `prim [PATH]...`
is a permanent alias for `prim fmt [PATH]...` — no verb is required for the
default, format-in-place action.

- **FR-5.1** _(default)_ `prim fmt` (and its bare alias) shall format matched
  files in place.
- **FR-5.2** `prim fmt --check` (also `fix --check`) shall write nothing, exit
  `0` when all files are already formatted, exit non-zero when any file would
  change, and list the files that would change. A run that examined nothing
  exits `2` instead of `0` (FR-4.4c).
- **FR-5.3** `prim fmt --diff` shall print a unified diff of pending changes and
  write nothing; it shall exit `0` whether or not changes are pending (`--check`
  is the CI gate). `prim fix --diff` shares `fix --check`'s gated contract
  instead (FR-5.2): it also prints the diff and writes nothing, but exits
  non-zero when a fixable finding is pending, since `fix`'s `--check` and
  `--diff` are both format-drift gates, unlike `fmt --diff`'s preview-only
  behaviour (AD-0007 §4). Being gates, both also exit `2` when the run examined
  nothing (FR-4.4c); `fmt --diff` does not.
- **FR-5.3a** `prim fmt --check-idempotence` (also reachable as bare
  `prim --check-idempotence` through the permanent `fmt` alias) shall write
  nothing and verify FR-6.1 across the matched corpus: for each discovered file
  prim owns, it formats the current bytes in memory, formats that output a
  second time with the same resolved `.editorconfig` style, and exits `1` if any
  second pass still changes bytes. It lists each failing file on stdout, exits
  `0` when every second pass is stable (`2` when it examined nothing, FR-4.4c),
  and uses the normal discovery/classify rules (structured formats plus the
  orphan hygiene allowlist only).
- **FR-5.4** With `--stdin-filepath <path>` (valid on `fmt`, `lint`, and `fix`),
  prim shall read stdin and, for `fmt`/`fix`, write the formatted result to
  stdout. The flag is mutually exclusive with `--check` and `--diff`.
- **FR-5.5** `prim lint` shall report hygiene and content violations without
  ever rewriting a file; it has neither `--check` nor `--diff` (report-only is
  its only mode). Being report-only makes it a gate throughout: a `lint` run
  that examined nothing exits `2` (FR-4.4c).
  - **FR-5.5a** _(hygiene diagnostics, story B1)_ For the un-owned-text
    allowlist (the orphan set, shell excluded — same scope as FR-2.4/2.5),
    `prim lint` shall report each whitespace-hygiene violation individually: a
    leading BOM, a line ending that does not match the resolved `end_of_line`,
    trailing whitespace, an indentation character that contradicts the resolved
    `indent_style`, and a missing final newline (when `insert_final_newline` is
    set). Each finding carries a stable, namespaced diagnostic code
    (`hygiene::bom`, `hygiene::eol`, `hygiene::trailing-whitespace`,
    `hygiene::indent`, `hygiene::final-newline`) and a 1-indexed `file:line:col`
    (`prim_fmt::line_col`, AD-0008), printed as `path:line:col: message [code]`.
    JSON/JSONC/TOML/YAML keep the coarser format-drift finding until their own
    content diagnostics land (D2).
  - **FR-5.5b** _(Markdown content diagnostics, stories G2/G3)_ For Markdown
    files, `prim lint` shall run `rumdl_lib::lint()` in Standard flavor through
    `prim_fmt::lint_markdown`, filtering `rumdl_lib::rules::all_rules(&cfg)` to
    prim's active rule subset by `Rule::name()`. The per-file `.editorconfig`
    key `prim_mdlint_strict = true|false` (default `false`) is resolved through
    the normal EditorConfig cascade; `false` runs the always-on floor tier of 12
    defect rules, `true` adds 13 convention rules on top. Every rule prim runs,
    at either tier, is an error: there is no warning severity for Markdown, so a
    finding's presence is its severity. Each finding carries rumdl's rule code
    verbatim and a 1-indexed `path:line:col`, printed as
    `path:line:col: message [MD0xx]`. This path is lint-only: prim shall never
    invoke rumdl's formatter or auto-fix Markdown findings, and `prim fix` does
    not yet auto-fix these rules.
    - **Floor tier — defect rules (always on, error at floor and strict):**
      MD011, MD034, MD042, MD045, MD051, MD052, MD056, MD062, MD066, MD068,
      MD070, MD075. Each reports something objectively broken — a dead link, a
      dangling reference, a malformed table — independent of what the author
      intended, so it can gate every repository with no opt-in. MD051 is
      corrected before it is reported: rumdl retains U+00A7 in a heading's
      computed slug as an artifact of its own emoji sentinel, where GitHub
      strips it. For a document holding that character which MD051 reported
      against, prim lints a second copy with the character substituted, drops
      the fragment findings the second pass does not report, and keeps every
      other finding. It is the one place prim lints a document twice. The
      correction is skipped — leaving the upstream false positive in place
      rather than risking a real finding — when the second pass cannot run, or
      when the document already holds every substitute character prim can safely
      use (AD-0018, #180).
    - **Strict tier — convention rules (`prim_mdlint_strict = true` only, error
      when active):** MD001, MD024, MD025 (SUMMARY-safe via `.editorconfig`;
      front-matter title excluded by default, see below), MD026, MD033, MD036,
      MD040, MD041, MD053, MD059, MD067, MD073, MD080. Each is decidable but
      fires on documents that are otherwise fine, so it gates only once a
      repository opts in.
    - **Never linted (formatter territory):** MD003-005, MD007, MD009, MD010,
      MD012, MD018-023, MD027-032, MD035, MD037-039, MD046-050, MD055, MD058,
      MD060, MD064, MD065, MD071, MD076, MD077.
    - **Off in both tiers:** MD014, MD043, MD044, MD054, MD061, MD063, MD069,
      MD072 (frontmatter key sorting would violate prim's semantics-preserving
      guardrail), MD074, MD078, MD079, MD081, MD057 (dropped from
      `ACTIVE_RULES`: whether a link that leaves the file resolves depends on
      the renderer, so a file-existence check is the wrong question at this
      layer; see AD-0013), and MD082 (dropped from `ACTIVE_RULES` entirely:
      absent from markdownlint, opt-in in rumdl, no fix by design, and —
      measured across six public documentation sites — 569 of 573 findings flag
      a parent heading immediately followed by a deeper one, an ordinary outline
      shape rather than an empty section; see AD-0012).
    - **Selected by `prim_mdlint_report_line_length` (FR-3.2d), off otherwise:**
      MD013. It sits outside the tier model — the key decides whether it runs,
      and the tier decides only whether it examines headings. prim shall set
      five of its options and expose none of them: `code-blocks = false` and
      `code-spans = false`, because prim preserves fenced content verbatim
      (FR-1.6) and keeps an unbreakable inline span on one line (FR-1.1a);
      `tables = false`, pinned rather than inherited from rumdl, because a table
      row cannot carry a line break; `line-length` from the resolved
      `max_line_length`, written to both rumdl's global and the rule's own key
      so their precedence rule cannot change the answer; and
      `headings =
      false` at the floor tier, `true` at the strict tier. A
      heading is the one construct prim cannot wrap but an author can shorten,
      which is the strict tier's definition of a finding. The tier used is the
      one in force after FR-5.5c's file-level override, so the directive moves
      the heading check with it.
    - **Exit-code implication:** floor-tier findings and strict-tier findings
      alike raise `prim lint`'s exit code to `1`; no Markdown rule emits a
      warning.
    - **Rule configuration prim owns:** prim passes rumdl a `Config` with
      MD025's `front-matter-title` option emptied, so a page's front-matter
      `title:` is treated as metadata rather than a heading, and — when FR-3.2d
      selected it — MD013's five options as listed above. These are prim
      choosing its own canonical defaults for rules it runs, not a configuration
      surface a repository can reach: there is still no way for a repository to
      configure a rule's options, save the one named path FR-3.3 records. See
      AD-0012 for the evidence behind the MD025 choice.
  - **FR-5.5c** _(override surface, story G5)_ A standalone
    `<!-- prim-mdlint-strict: true|false -->` line anywhere in a Markdown file
    (the whole line, once trimmed) overrides FR-5.5b's `.editorconfig`-resolved
    `prim_mdlint_strict` for that file only. When several such lines are
    present, the last one (top-to-bottom) wins; an unrecognized value falls back
    to the `.editorconfig`-resolved tier rather than erroring the lint run.
    rumdl's own inline directives (`rumdl-disable`/`rumdl-enable`,
    `markdownlint-disable`/`-enable`, and their line/next-line/file-scoped
    forms) require no additional wiring: `rumdl_lib::lint` applies them
    internally before returning findings, independent of the `source_file` prim
    passes (`None`). No second config source is introduced — the override
    surface is the strict boolean plus these two inline mechanisms, never a
    per-rule matrix.
- **FR-5.6** _(exit codes)_ `0` = nothing to do / already clean · `1` =
  actionable — format drift (`fmt`/`fix --check`) or a lint finding · `2` = prim
  could not do its job (parse/IO/usage error, or a gate that examined nothing —
  FR-4.4c). Warnings never raise the exit code; only errors do.
- **FR-5.6a** _(argv survives a byte prim cannot decode)_ prim shall read its
  command line with `std::env::args_os`, so a **path** that is not valid UTF-8
  reaches the filesystem as the bytes prim was given — satisfying FR-2.5 for a
  named path, not only a discovered one (AD-0017). A non-UTF-8 value for an
  option that is not a path — an `--exclude` glob, a `--since` ref — remains a
  usage error (exit `2`, FR-4.5): those are matched and resolved as text, and
  prim has no meaning for bytes that are not.
- **FR-5.6b** _(a contained fault keeps the contract)_ prim shall not exit
  outside FR-5.6 for a panic raised inside a third-party formatter or linter. It
  shall contain such a panic per file: the file shall be reported as an error
  with its path named and left byte-for-byte unchanged (FR-6.3), the run shall
  continue to the next file, and the process shall exit `2`. A panic shall be an
  error whether the file was named or walked, because it is a fault in prim
  rather than in the input. Where prim holds a buffer it does not own it shall
  return that buffer unchanged: `fmt`/`fix --stdin-filepath` shall echo stdin to
  stdout and exit `2`, and the LSP shall return no edits, no diagnostics, and
  keep the session. `lint --stdin-filepath` writes a report to stdout rather
  than a buffer, so it echoes nothing. A `--format` run shall still emit its
  report document, since `--format` governs stdout alone.
- **FR-5.7** _(deprecated top-level flags)_ The top-level `--check`, `--diff`,
  and `--stdin-filepath` flags remain accepted directly on bare `prim` as
  deprecated sugar for the `fmt` forms; the first use in a run emits a one-line
  deprecation warning to stderr. They are scheduled for removal in v2.0; the
  bare alias itself is not deprecated.
- **FR-5.8** _(machine-readable reports, story D2)_ `--format <json|sarif>`
  shall be accepted only on `prim fmt --check` and `prim lint`. It changes only
  stdout for those report-only modes: write behaviour and exit codes are
  unchanged, and warnings/errors remain on stderr. Without `--format`, the
  existing plain-text stdout for `fmt --check` and `lint` remains unchanged.
  - **FR-5.8a** `--format json` shall emit a stable JSON document of the form
    `{ "version": 1, "mode": "fmt-check"|"lint", "findings": [...] }`. Each
    finding includes `path`, `code`, and `message`; positioned findings also
    include 1-indexed `line` and `column`. `fmt --check` reports one
    `format::drift` finding per file that would change, with the message
    `"would be reformatted"`. `prim lint` reports the existing coarse structured
    format drift as `format::drift`, plus the B1 hygiene diagnostics for orphan
    files with their stable `hygiene::*` codes and positions.
  - **FR-5.8b** `--format sarif` shall emit a valid SARIF 2.1.0 log with one
    result per finding. Each result's `ruleId` shall match the stable `code`,
    `artifactLocation.uri` shall be the reported file path, and
    `region.startLine` / `region.startColumn` shall be present whenever the
    finding has a known position.

## FR-6 — Correctness & safety

- **FR-6.1** _(idempotency)_ Running prim on its own output shall produce zero
  further changes. `prim fmt --check-idempotence` is the CLI-facing verification
  surface for this guarantee and never writes to disk.
- **FR-6.2** _(semantic preservation)_ Formatting shall not change the parsed
  data model of a JSON/JSONC/YAML/TOML document.
- **FR-6.3** _(fail-safe)_ An unparseable file shall be left byte-for-byte
  unchanged and reported as an error (exit `2`).
- **FR-6.4** _(atomic write)_ prim shall write via a temporary file and atomic
  rename, preserving permission bits.
- **FR-6.5** prim shall process only UTF-8 text; it shall leave non-UTF-8 files
  unchanged and report them.

## NFR — non-functional (targets, tunable)

- **NFR-1** One statically linked binary, zero runtime dependencies.
- **NFR-2** Linux/macOS/Windows on `amd64` + `arm64`.
- **NFR-3** _(determinism)_ identical input → byte-identical output on every
  supported platform. Parallel file processing must preserve discovery order for
  reports and deferred warnings/errors so output stays byte-identical across
  runs.
- **NFR-4** _(throughput)_ format a 5,000-file repository in under 2 s on an
  8-core machine with warm cache, parallelized across files. prim satisfies this
  by parallelizing the per-file read → style-resolution → format pipeline across
  discovered files.
- **NFR-5** _(footprint)_ peak memory scales with the largest single file, not
  repository size.

## Style stability

The canonical style is a compatibility contract. Any change to prim's output for
already-canonical input — including a change inherited from a formatter
dependency upgrade (`dprint-plugin-json`, `dprint-plugin-markdown`, `taplo`,
`pretty_yaml`) — is a versioned, release-noted event: a **minor** version bump
while prim is pre-1.0, a **major** bump once prim reaches 1.0. The release notes
must call out the changed output explicitly so downstream `prim --check` gates
upgrade deliberately. The fixture harness
(`crates/prim-fmt/tests/correctness/fixtures/`) is prim's **golden corpus**: its
`spec_cases_format_as_expected` test byte-compares formatter output against each
fixture's committed `-- expected --` section, so canonical-output drift fails
the build until it is reverted, or deliberately regenerated with
`PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness
spec_cases_format_as_expected`,
reviewed in the diff, and released as above. CI runs the plain, ungated
`cargo test --workspace` (no `PRIM_SPEC_UPDATE`), so an unreviewed golden-corpus
regeneration can never merge silently.

Because releases are generated from Conventional Commits (`convco`), the policy
above only holds if commit types match intent: a commit that changes a golden
fixture's `-- expected --` section (or otherwise changes canonical output) must
be typed `feat` (or `feat!` for a breaking, post-1.0 change) — never `fix`,
`refactor`, or `chore` — so the generated `CHANGELOG.md` surfaces it under the
right heading and `convco`'s version bump matches the compatibility contract
above. A reviewer who sees a fixture's `-- expected --` section change in a
non-`feat` commit should request re-typing before merge.

## Non-goals

- No source-code formatting (Rust/JS/TS/Python/Go/…).
- No plugins or user-facing extensibility API.
- No schema validation or generalized lint framework beyond the documented
  whitespace-hygiene and Markdown-content checks.
- No style knobs beyond `.editorconfig`.

## Phase 2 — roadmap (not v1)

- prim _may_ format shell scripts (`*.sh`/`*.bash`) by embedding `shfmt`
  compiled to WebAssembly. This brushes the "no plugins" non-goal and is to be
  decided deliberately at Phase 2 start: prim has no plugin _system_ (no
  user-supplied formatters), but _may embed_ specific curated wasm formatters
  internally.
