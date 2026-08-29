# Usage

```text
prim [fmt|lint|fix] [OPTIONS] [PATH]...
prim init [PATH]
prim explain <PATH>
prim lsp
```

prim exposes three formatting verbs (AD-0007) plus three utilities: `init` (repo
setup), `explain` (config introspection), and `lsp` (a format-on-save language
server). Bare `prim [PATH]...` is a permanent alias for `prim fmt [PATH]...` —
no verb is required for the common case.

| Command   | Writes?               | Purpose                                                                                    |
| --------- | --------------------- | ------------------------------------------------------------------------------------------ |
| `fmt`     | yes (in place)        | Format the parsed formats + whitespace hygiene. Default action.                            |
| `lint`    | never                 | Report hygiene and content violations only.                                                |
| `fix`     | yes (in place)        | `fmt` plus autofixable content rules (none yet, so `fix` is currently identical to `fmt`). |
| `init`    | `.editorconfig` only  | Scaffold or minimally merge prim's Markdown strict-glob map.                               |
| `explain` | never                 | Print the `.editorconfig` settings that apply to one file, and where each came from.       |
| `lsp`     | never (returns edits) | Run an LSP formatting server over stdio for editor format-on-save.                         |

## Arguments

| Argument    | Description                                                                                                                                                                                                            |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `[PATH]...` | Files or directories to process. Directories are searched recursively (honoring `.gitignore`/`.git/info/exclude`/global gitignore/`.ignore`/`.primignore` by default); defaults to the current directory when omitted. |

## Options

| Flag                            | Verbs                 | Description                                                                                                                                                                                               |
| ------------------------------- | --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `--check`                       | `fmt`, `fix`          | Write nothing; exit non-zero if any file would change, and list it.                                                                                                                                       |
| `--diff`                        | `fmt`, `fix`          | Print a unified diff of pending changes; write nothing. Exit `0` on `fmt` regardless of pending changes; exit non-zero on `fix` if a fixable finding is pending (shares `fix --check`'s gated contract).  |
| `--check-idempotence`           | `fmt`                 | Write nothing; for each matched prim-owned file, format it in memory twice with the resolved `.editorconfig` style and exit non-zero if the second pass still changes bytes.                              |
| `--format <json\|sarif>`        | `fmt --check`, `lint` | Emit machine-readable findings to stdout instead of the default plain-text report. Valid only on `fmt --check` and `lint`.                                                                                |
| `--stdin-filepath <PATH>`       | `fmt`, `lint`, `fix`  | Read stdin and process it (format-on-save for `fmt`/`fix`; report for `lint`). Mutually exclusive with `--check`/`--diff`.                                                                                |
| `--exclude <GLOB>`              | all                   | Exclude paths matching the glob (repeatable). A malformed glob is a usage error.                                                                                                                          |
| `--no-ignore`                   | `fmt`, `lint`, `fix`  | Disable only VCS ignore files (`.gitignore`, global gitignore, `.git/info/exclude`). `.primignore`, `--exclude`, and the `.git/` directory prune still apply.                                             |
| `--no-primignore`               | `fmt`, `lint`, `fix`  | Disable `.primignore`, including for paths named on the command line. VCS ignore files, `--exclude`, and the `.git/` directory prune still apply.                                                         |
| `--since <REF>`                 | `fmt`, `lint`, `fix`  | Limit the file set to `git diff --name-only <REF>`: paths that differ between `<REF>` and the current working tree, including staged and unstaged changes (plain two-way diff, no merge-base comparison). |
| `--staged`                      | `fmt`, `lint`, `fix`  | Limit the file set to `git diff --name-only --cached`: paths staged in the git index relative to `HEAD`. Mutually exclusive with `--since`.                                                               |
| `--color <auto\|always\|never>` | all                   | When to use coloured output (default `auto`; `auto` honors `NO_COLOR`).                                                                                                                                   |
| `--completions <SHELL>`         | global                | Generate a shell completion script and print it to stdout.                                                                                                                                                |
| `-h, --help`                    | global                | Print help.                                                                                                                                                                                               |
| `-V, --version`                 | global                | Print version.                                                                                                                                                                                            |

The top-level `--check`, `--diff`, and `--stdin-filepath` flags remain accepted
directly on bare `prim` as **deprecated sugar** for the `fmt` forms: the first
use in a run prints a one-line deprecation warning to stderr. They are scheduled
for removal in v2.0 — the bare `fmt` alias itself is not deprecated.

## Exit codes

| Code | Meaning                                                             |
| ---- | ------------------------------------------------------------------- |
| `0`  | Nothing to do, or already clean.                                    |
| `1`  | Actionable: format drift (`fmt`/`fix --check`) or a `lint` finding. |
| `2`  | prim could not do its job (parse, I/O, or usage error).             |

Warnings never raise the exit code; only errors do. A gate that was pointed only
at skipped paths examined nothing, and exits `2` rather than `0` (FR-4.4c).

## Operating modes

- **`fmt` (default)** — format the given files in place.
- **`fmt --check`** (also `fix --check`) — a CI gate: exit `1` and list the
  files that would change. Add `--format json` or `--format sarif` to emit the
  same findings as a machine-readable report instead of the default path list.
- **`--no-ignore`** — keep prim's own filters (`.primignore`, `--exclude`, and
  `.git/` pruning) but ignore VCS ignore files so paths hidden by `.gitignore`,
  global gitignore, or `.git/info/exclude` are walked again.
- **`--no-primignore`** — the opposite switch: keep VCS ignore files but drop
  `.primignore`. Needed only to process a path `.primignore` covers, since that
  file is honoured however prim is invoked — walked to or named on the command
  line (AD-0009), and under gitignore's own rules: a `!` line cannot re-include
  a file when a directory holding it is excluded, so `fixtures/` followed by
  `!fixtures/keep.md` leaves `keep.md` covered. This same flag also disables the
  built-in generated-file list (AD-0011), which behaves as the outermost
  `.primignore` layer. Naming an ignored path without this flag prints a warning
  and changes nothing; warnings never raise the exit code. The one case that
  does is a gate — `fmt --check`, `fix --check`, `fix --diff`,
  `fmt --check-idempotence`, or `lint` — where _every_ path prim was pointed at
  is skipped: prim then examined nothing, so it reports an error and exits `2`
  instead of reporting a clean run (FR-4.4c).
- **`--since <REF>`** — limit discovery to the paths
  `git diff --name-only <REF>` reports: files that differ between `<REF>` and
  the current working tree, including both staged and unstaged changes. prim
  uses the plain two-way `git diff <REF>` semantics here — no merge-base (`...`)
  comparison.
- **`--staged`** — limit discovery to the paths `git diff --name-only --cached`
  reports: files staged in the git index relative to `HEAD`. It chooses paths
  only: a write mode still writes the working tree and never touches the index,
  so prim warns on stderr when it writes under `--staged` and leaves re-staging
  to the hook runner (FR-4.2c, FR-4.2d). The warning reports what prim wrote,
  not what the index holds — prim never reads the staged blob — and points at
  `git diff`, where the unstaged result shows up. It does not raise the exit
  code.
- **Changed-file filters** — `--since` and `--staged` are mutually exclusive.
  They compose by intersection with `--check`, `--diff`, `lint`, `fix`, explicit
  path arguments, `.primignore`, `--exclude`, `--no-ignore`, and
  `--no-primignore`. Deleted paths reported by git are skipped silently, and
  both flags require the current working directory to be inside a git working
  tree.
- **`fmt --diff`** — preview pending changes without writing; always exits `0`
  (`--check` is the CI gate).
- **`fmt --check-idempotence`** — a formatter self-check: prim formats each
  matched file in memory, reformats that output with the same resolved style,
  and exits `1` only if the second pass still changes bytes. It never writes to
  disk, even when the original file is not already in canonical form. Bare
  `prim --check-idempotence [PATH]...` works too through the permanent `fmt`
  alias.
- **`fix --diff`** — preview pending changes without writing, like `fmt --diff`,
  but exits `1` if a fixable finding is pending — `fix`'s `--check` and `--diff`
  share one gated contract (AD-0007 §4), unlike `fmt --diff`'s preview-only
  behaviour.
- **`lint`** — report-only: prints one finding per violation and never rewrites.
  - For the un-owned-text allowlist (BOM, line endings, trailing whitespace,
    indentation, missing final newline — same set `.editorconfig`/hygiene
    covers), each finding is a coded, positioned diagnostic:
    `path:line:col: message [code]` (e.g.
    `notes.txt:1:6: trailing whitespace
    [hygiene::trailing-whitespace]`).
  - For Markdown, `prim lint` runs rumdl in Standard flavor against prim's own
    curated rule set, placed into two bands and selected per file through
    `.editorconfig` `prim_mdlint_strict = true|false` (default `false`). `false`
    runs the always-on floor tier of 12 defect rules; `true` adds 13 convention
    rules on top. Every rule prim runs, at either tier, is an error — there is
    no warning severity for Markdown, so a finding's presence is its severity.
    prim prints each finding as `path:line:col: message
    [MD0xx]`, passes
    rumdl's rule codes through verbatim, never invokes rumdl's formatter/fixer,
    and does not auto-fix these findings in `fix` yet.
    - **Floor tier — defect rules** (always on, error at floor and strict):
      MD011, MD034, MD042, MD045, MD051, MD052, MD056, MD062, MD066, MD068,
      MD070, MD075. Each reports something objectively broken — a dead link, a
      dangling reference, a malformed table — so it gates every repository with
      no opt-in.
    - **Strict tier — convention rules** (`prim_mdlint_strict = true` only,
      error when active): MD001, MD024, MD025 (SUMMARY-safe via `.editorconfig`;
      front-matter title excluded by default, see below), MD026, MD033, MD036,
      MD040, MD041, MD053, MD059, MD067, MD073, MD080. Each is decidable but
      fires on documents that are otherwise fine, so it gates only once a
      repository opts in.
    - **Never linted (formatter territory):** MD003-005, MD007, MD009, MD010,
      MD012, MD018-023, MD027-032, MD035, MD037-039, MD046-050, MD055, MD058,
      MD060, MD064, MD065, MD071, MD076, MD077.
    - **Off in both tiers:** MD014, MD043, MD044, MD054, MD061, MD063, MD069,
      MD072 (frontmatter key sorting stays off because prim must remain
      semantics-preserving), MD074, MD078, MD079, MD081, MD057 (dropped — a
      cross-file link's target depends on the renderer, so prim does not check
      it; see AD-0013), and MD082 (dropped entirely — see AD-0012).
    - **Line length (`prim_mdlint_report_line_length`):** `max_line_length`
      (default 80) already controls how prim wraps Markdown prose. Setting
      `prim_mdlint_report_line_length = true` additionally makes `prim lint`
      report lines that exceed it.

      prim reports only the long lines that can actually be shortened. A long
      prose line is reported — `prim fmt` wraps prose to the same limit, so a
      repository that formats with prim sees no prose findings. Table rows,
      fenced code, and an inline code span with no internal space are never
      reported: a line break cannot be inserted into any of them without
      changing what the document means, so there is nothing to fix.

      Headings are the one case decided by the tier. prim cannot wrap a heading
      — a line break would end the heading and turn the rest into a paragraph —
      but you can shorten the wording. So a long heading is reported when
      `prim_mdlint_strict = true`, and silent otherwise, like every other
      convention-tier check. Note that rumdl measures a heading only up to its
      last whitespace, so the effective slack is the length of the final word:
      at an 80-column limit an 84-column heading is silent and an 86-column one
      is reported. Do not adopt this key expecting headings held to exactly
      `max_line_length`.

      The limit is whatever `max_line_length` resolves to for that file, so no
      `.editorconfig` cascade can leave the formatter and the linter disagreeing
      about the width. If your headings are long by convention — numbered
      decision records, for example — keep those files at the floor tier.

      | Line content                                 | Floor    | Strict   |
      | -------------------------------------------- | -------- | -------- |
      | Prose                                        | reported | reported |
      | Heading                                      | silent   | reported |
      | Table row                                    | silent   | silent   |
      | Fenced or indented code                      | silent   | silent   |
      | Long only because of unbreakable inline code | silent   | silent   |

      This is MD013, the one rule prim excludes from both tiers by default and
      lets a repository switch on, because it is the only rule whose options
      must track the formatter's own line width. prim sets five of MD013's
      options and exposes none of them: `line-length` from the resolved
      `max_line_length`, `code-blocks`, `code-spans` and `tables` off, and
      `headings` on at the strict tier only. Every other MD013 option keeps
      rumdl's default — including `ignore-link-urls`, which is why a line that
      is long only because of a link URL is usually not reported; prim does not
      pin that one, so treat it as rumdl's behaviour rather than prim's
      guarantee.

      There is no `.editorconfig` key for any of these options. A single file
      can still override them with rumdl's own `markdownlint-configure-file`
      directive, the same per-file escape hatch every other rule prim runs
      already has (AD-0012). It can change the width or re-enable the table and
      code checks for that one file; it cannot switch MD013 on where
      `prim_mdlint_report_line_length` has not. `prim_mdlint_disable = MD013`
      turns the rule off again for a narrower glob, the same way it removes any
      other rule.
    - Floor-tier and strict-tier findings alike raise `prim lint`'s exit code to
      `1`; no Markdown rule emits a warning.
    - **Rule options prim sets for itself:** prim configures MD025's
      `front-matter-title` option to an empty string, so a page's front-matter
      `title:` is treated as metadata rather than a heading, and — when
      `prim_mdlint_report_line_length` selects it — five of MD013's options.
      These are prim's own canonical defaults for rules it runs. A repository
      reaches only two of them, both MD013's and both named by FR-3.3:
      `max_line_length` supplies `line-length`, and `prim_mdlint_strict`
      supplies `headings`. No other option of any rule is reachable — see
      "Configuration" below, AD-0012 and AD-0014.
    - **Per-file override (story G5):** a standalone
      `<!-- prim-mdlint-strict: true|false -->` line anywhere in the file
      overrides `.editorconfig`'s resolved tier for that file only — an escape
      hatch for the rare file that needs to differ from its glob (e.g. opt a
      legacy doc out of a `docs/**.md` strict glob, or opt one file into strict
      without a matching glob). The line must be the whole line once trimmed; if
      several are present, the last one wins; an unrecognized value (anything
      but `true`/`false`, case-insensitive) is ignored and falls back to the
      `.editorconfig`-resolved tier.
    - **Per-glob rule exclusion:** `.editorconfig` `prim_mdlint_disable` removes
      named rules from whichever tier a path already runs — see "Configuration"
      below.
    - **rumdl's own inline directives** work as-is — prim calls
      `rumdl_lib::lint` directly, which applies them before returning findings,
      so no prim-side wiring was needed:

      | Directive                                         | Scope    |
      | ------------------------------------------------- | -------- |
      | `<!-- markdownlint-disable-file MD033 -->`        | file     |
      | `<!-- markdownlint-disable MD033 -->` … `-enable` | block    |
      | `<!-- markdownlint-disable-next-line MD033 -->`   | one line |
      | `<!-- rumdl-disable-file MD033 -->`               | file     |

  - JSON/JSONC/YAML/TOML still report the coarser format drift `fmt --check`
    would report; their own content diagnostics are future work.
  - Add `--format json` or `--format sarif` to switch stdout from the plain-text
    report above to a machine-readable document carrying the same findings
    (hygiene, Markdown, and format-drift alike).
- **`--stdin-filepath`** — editor format-on-save: stdin in, formatted stdout out
  (`fmt`/`fix`), or a report (`lint`).
- Naming a path explicitly is strict: a missing file is an error (exit `2`); an
  existing file prim does not own is skipped with a warning.

## `prim init`

`prim init [PATH]` scaffolds or minimally merges `.editorconfig` in `PATH`
(default `.`). It writes no other file.

With no existing `.editorconfig`, prim writes this exact placement map when no
mdBook is detected:

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

Section order is part of the contract: EditorConfig has no specificity ranking,
so the broader `[*.md]` floor must appear before the stricter middle section,
`[docs/wip/**.md]` and `[docs/archive/**.md]` must come after it to opt working
memory back down, and `[**/SUMMARY.md]` must come last to opt mdBook summaries
back down. `[docs/wip/**.md]` and `[docs/archive/**.md]` are literals, not
derived from the strict glob: Superpowers specs and plans live under `docs/wip/`
while a branch is open, and gardening moves the raw originals to
`docs/archive/`. That move is not an edit, so it must not change a document's
lint tier — otherwise filing work away is what makes a repository's own CI start
failing on it. (A repository that does want its archive linted can write
`prim_mdlint_strict = true` under that section; `prim init` leaves an explicit
choice alone on every later run.)

If `PATH/book.toml` exists, prim reads `[book].src` and uses that directory for
the strict middle glob instead of `docs/`; for example, `src = "guide"` yields
`[guide/**.md]`. If `book.toml` is present but omits `src`, or is malformed,
prim falls back to mdBook's conventional `src/**.md`.

If `.editorconfig` already exists, prim merges in place without reordering
unrelated content:

- leaves an existing top-level `root = ...` untouched; otherwise prepends
  `root = true` and a blank line
- for `[*.md]`, the detected strict glob, `[docs/wip/**.md]`,
  `[docs/archive/**.md]`, and `[**/SUMMARY.md]`, leaves an existing explicit
  `prim_mdlint_strict = ...` untouched
- if one of those sections exists but lacks the key, appends the key inside that
  section immediately before the next section (or end-of-file)
- if one of those sections is missing entirely, inserts a new block without
  moving existing bytes so the final prim-managed order still reads `[*.md]` →
  strict glob → `[docs/wip/**.md]` → `[docs/archive/**.md]` → `[**/SUMMARY.md]`
  (falling back to end-of-file only when no later prim-managed section needs to
  stay after it)
- if a section is missing and the file's own existing section order already
  contradicts that canonical order — for example an existing `[**/SUMMARY.md]`
  written before the strict glob — there is no position left for the missing
  section; prim leaves it out and prints a warning naming the two conflicting
  sections, the lines they start at, and where to add the section by hand

Before it writes anything, prim resolves the file it would produce. For one
representative path per canonical section it places — a top-level file, a file
under the strict glob, a file under `docs/wip/` where that section applies, a
file under `docs/archive/` where that section applies, and a `SUMMARY.md` — it
applies EditorConfig's last-match-wins section order and compares the result
with the value it intended and with the value that path resolved to before the
run. prim makes a write only when the path that write is meant to place lands on
the intended value and none of the other representative paths moves. (The check
is over prim's own canonical globs: a glob you wrote that none of those paths
stands for is not covered.) A write that would fail that check is not made: prim
warns, naming the path and the value it would take, and still makes whatever
other writes are safe. prim never reorders sections a person wrote, so a file
whose own order contradicts the canonical one is reported and left for you to
fix. An existing `.editorconfig` prim cannot parse at all is reported and left
untouched — with no resolution there is nothing to check a change against.

prim also resolves every canonical section the file already carries
`prim_mdlint_strict` for, not only the ones it plans to write, checking each
against the same representative paths. A section fails that check three ways: a
later section sets a different value and wins, so the wrong tier applies to the
section's own paths; a later section sets the same value and wins anyway, so the
earlier section decides none of those paths even though nothing resolves
incorrectly today; or the value itself is not `true` or `false`, which
EditorConfig reads as `false` silently. Each is a warning naming the section,
the line it starts at, and the section that decides instead — never a refusal,
since a person's own narrower override is legitimate and prim will not reorder
sections a person wrote. When any of these warnings fires, the summary line
reads `.editorconfig left unchanged — see the warning(s) above` instead of
`.editorconfig already contains the Markdown strict-glob map`, even on a run
that writes nothing, because the two are different outcomes.

`prim init` writes the whole file under one line ending: the `end_of_line` that
resolves for `.editorconfig` once the write has been made, which is LF unless
the file prim writes itself sets `end_of_line = crlf` (FR-2.3). It does not
carry an existing file's line endings through — merging LF additions into a CRLF
file would leave mixed endings, and a uniformly CRLF file with no `end_of_line`
key is a file that `prim fmt --check` reports.

The ending is resolved after the write rather than before, because the
`root = true` that `prim init` writes stops EditorConfig's upward walk: an
`end_of_line` declared by an ancestor no longer reaches the file once prim has
written it. So a scaffold placed under a CRLF-declaring parent is written LF,
which is what `prim fmt` then resolves for it. A run that writes nothing — the
map is already present — leaves the file exactly as it found it, line endings
included.

Running `prim init` twice is idempotent: once the map is present, the second run
reports a no-op and leaves `.editorconfig` byte-identical. An occurrence of one
of those globs that neither sets `prim_mdlint_strict` nor receives it — an
ordinary `[*.md] max_line_length = 80` appended after a map that already sets
the key — takes no part in prim's ordering, so it never makes `prim init` refuse
to work.

### Running `prim init` in a subdirectory

EditorConfig requires `root = true` at the top of the file prim writes, and
`root = true` stops EditorConfig's upward walk — for every key and every file
type, not just prim's own. In a subdirectory of a repository that already has an
`.editorconfig`, that means everything the parent configured stops reaching the
files below:

```console
$ prim explain sub/a.md | grep max_line_length
  max_line_length = 120  (.editorconfig:3 [*.md])
$ prim init sub
warning: sub: prim wrote root = true, which EditorConfig requires here, so files
under this directory no longer inherit from .editorconfig — the keys set there
(max_line_length) no longer reach this directory. Delete the root = true line to
keep inheriting them.
$ prim explain sub/a.md | grep max_line_length
  max_line_length = unset  (prim's default)
```

prim still makes the write — the `root = true` line is mandated, and `prim init`
cannot know whether you wanted the parent cascade. Delete that line if you did.

The warning names every ancestor `.editorconfig` that becomes unreachable and
every key those files set in a section. It lists what those files set, not what
applied to any particular file below, so a section whose glob matches nothing
here is still named. An ancestor that carries only `root = true` is left out,
because cutting the walk off from it loses nothing, and so is a key written
before the first section header, because EditorConfig never applied it. Running
`prim init` at the top of a repository, where there is nothing above to inherit
from, prints no warning.

## `prim explain`

`prim explain <PATH>` prints every `.editorconfig` setting that applies to
`PATH`, its effective value, and where that value came from: a specific
`.editorconfig` file and line (with the `[glob]` section it came from, when one
could be recovered), or `prim's default` when no `.editorconfig` entry set it.
`PATH` need not exist — resolution is name/extension-based, the same
classification `fmt`/`lint`/`fix` use, so `explain` also works for a
not-yet-created file to preview what settings it would get.

Given a `.editorconfig` such as:

```ini
root = true

[*]
charset = utf-8
end_of_line = lf
insert_final_newline = true
trim_trailing_whitespace = true
indent_style = space
indent_size = 2

[*.md]
max_line_length = 80

[docs/**.md]
prim_mdlint_strict = true
prim_mdlint_disable = MD033, MD041
```

`prim explain` prints:

```console
$ prim explain docs/USAGE.md
docs/USAGE.md
  end_of_line                    = lf         (/repo/.editorconfig:5 [*])
  trim_trailing_whitespace       = true       (/repo/.editorconfig:7 [*])
  insert_final_newline           = true       (/repo/.editorconfig:6 [*])
  indent_style                   = space      (/repo/.editorconfig:8 [*])
  indent_size                    = 2          (/repo/.editorconfig:9 [*])
  max_line_length                = 80         (/repo/.editorconfig:12 [*.md])
  prim_mdlint_strict             = true       (/repo/.editorconfig:15 [docs/**.md])
  prim_mdlint_report_line_length = false      (prim's default)
  prim_mdlint_disable            = MD033, MD041 (/repo/.editorconfig:16 [docs/**.md])
```

(prim's own `.editorconfig` sets `prim_mdlint_strict` in several sections and
sets neither `prim_mdlint_disable` nor `prim_mdlint_report_line_length`, so
running this against the repository prim ships in prints the tier resolved for
that path and `unset`/`false (prim's default)` for the other two — see below.)

The settings shown depend on the file's kind: un-owned text files (the
[Orphan allowlist](#what-prim-formats)) only get the three universal hygiene
settings (`end_of_line`, `trim_trailing_whitespace`, `insert_final_newline`);
Markdown additionally shows `prim_mdlint_strict`,
`prim_mdlint_report_line_length` and `prim_mdlint_disable`. When
`prim_mdlint_disable` was never set for a path, its value prints `unset` against
`prim's default`. When it was set but resolves to no rules — a deliberate
`prim_mdlint_disable = none` or `= unset`, or a list whose every id was
unrecognised (which also warns on stderr) — the value prints `none` against the
`.editorconfig` line that set it, because that is what prim applies there. A
path prim does not format at all reports a warning
(`not a file type prim formats;
skipped`) and prints no settings, but still
exits `0` — `explain` never gates a build.

## `prim lsp`

`prim lsp` runs a
[Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
server over stdin/stdout, exposing prim's formatter as a **document formatting
provider**. Point an editor's LSP client at it and prim formats prim-owned files
through the editor's native format-on-save, running the exact same engine as
`prim fmt` — an editor save and a CLI run produce byte-identical output.

It is deliberately narrow: it advertises only whole-document formatting and
**Full** document sync (it never splices incremental edits) — plus, as of G5's
follow-up story, `textDocument/publishDiagnostics` for the same findings
`prim lint` already reports: whitespace-hygiene diagnostics (B1) for un-owned
text files and rumdl Markdown content diagnostics (G2), reprojected onto LSP's
range/severity shape. It still does **not** publish completions, hover, or
semantic highlighting — prim is a formatter, not a general language server.
Diagnostics are republished (even as an empty list) on every
`didOpen`/`didChange`, and cleared with an empty list on `didClose`, so stale
findings never linger once a file is fixed or closed. Structured formats
(JSON/JSONC/YAML/TOML) have no itemized diagnostics yet — the same scope
`prim lint` covers for those kinds today.

Requesting to format a file prim does not own, or one that is already canonical,
returns no edits; a file prim cannot parse is left untouched (no edits),
matching `--stdin-filepath`'s fail-safe contract.

The server honors `.editorconfig` exactly as the CLI does; the client's
`FormattingOptions` (tab size, insert-spaces) are ignored in favour of prim's
resolved style.

### VS Code

prim has no bundled extension yet; wire it through a generic LSP client such as
[`vscode-glspc`](https://marketplace.visualstudio.com/items?itemName=eirikpre.vscode-glspc)
or any "generic LSP" bridge, pointing its server command at `prim lsp` and its
document selector at the prim-owned languages (`json`, `jsonc`, `yaml`, `toml`,
`markdown`, `plaintext`). Then enable format-on-save:

```jsonc
// settings.json
{
  "glspc.languageId": "markdown",
  "glspc.serverCommand": "prim",
  "glspc.serverCommandArguments": ["lsp"],
  "editor.formatOnSave": true
}
```

### Neovim (0.8+)

Register prim as a formatting-only server with the built-in LSP client:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = { "json", "jsonc", "yaml", "toml", "markdown", "text" },
  callback = function(args)
    vim.lsp.start({
      name = "prim",
      cmd = { "prim", "lsp" },
      root_dir = vim.fs.dirname(vim.fs.find({ ".editorconfig", ".git" }, { upward = true })[1]),
    }, { bufnr = args.buf })
  end,
})

-- Format the prim-owned buffer on save.
vim.api.nvim_create_autocmd("BufWritePre", {
  pattern = { "*.json", "*.jsonc", "*.yaml", "*.yml", "*.toml", "*.md" },
  callback = function() vim.lsp.buf.format() end,
})
```

### Zed

Add prim as a formatter-only language server and select it per language in
`settings.json`:

```jsonc
{
  "lsp": {
    "prim": { "binary": { "path": "prim", "arguments": ["lsp"] } }
  },
  "languages": {
    "Markdown": { "format_on_save": "on", "language_servers": ["prim"] },
    "JSON": { "format_on_save": "on", "language_servers": ["prim"] },
    "YAML": { "format_on_save": "on", "language_servers": ["prim"] },
    "TOML": { "format_on_save": "on", "language_servers": ["prim"] }
  }
}
```

## Machine-readable output

`--format json` and `--format sarif` are available only on `prim fmt --check`
and `prim lint`. They change only stdout; warnings, parse errors, missing-path
errors, and deprecation warnings still go to stderr exactly as they do in the
default plain-text modes.

### JSON schema

prim's JSON report is intentionally small and stable:

```json
{
  "version": 1,
  "mode": "lint",
  "findings": [
    {
      "path": "doc.json",
      "code": "format::drift",
      "message": "does not match prim's canonical format (run `prim fmt` to fix)"
    },
    {
      "path": "notes.txt",
      "code": "hygiene::trailing-whitespace",
      "message": "trailing whitespace",
      "line": 1,
      "column": 6
    }
  ]
}
```

- `version` is the report-schema version, starting at `1`.
- `mode` is `fmt-check` or `lint`.
- `findings` contains one object per reported finding.
- `line` and `column` appear only when prim has a concrete source position.
- `fmt --check` emits `format::drift` findings with the message
  `would be reformatted`.

### SARIF 2.1.0

`--format sarif` emits a SARIF 2.1.0 log for the same findings. `ruleId` matches
prim's stable finding code, `artifactLocation.uri` is the reported path, and
`region.startLine` / `region.startColumn` are included when prim has a
positioned finding.

```json
{
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "prim"
        }
      },
      "results": [
        {
          "ruleId": "hygiene::trailing-whitespace",
          "message": { "text": "trailing whitespace" },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": { "uri": "notes.txt" },
                "region": { "startLine": 1, "startColumn": 6 }
              }
            }
          ]
        }
      ]
    }
  ]
}
```

### GitHub Actions integration

**SARIF upload** and **problem matchers** are separate GitHub features:

- Use `prim ... --format sarif` when you want to upload a SARIF artifact with
  `github/codeql-action/upload-sarif`.
- Use a problem matcher when you want GitHub Actions to parse prim's default
  plain-text `lint` output from the step log.

Example SARIF upload:

```yaml
- name: Run prim lint as SARIF
  run: prim lint --format sarif . > prim.sarif

- name: Upload prim SARIF
  uses: github/codeql-action/upload-sarif@v3
  with:
    sarif_file: prim.sarif
```

Example problem matcher for plain-text `prim lint` output:

```json
{
  "problemMatcher": [
    {
      "owner": "prim-hygiene",
      "pattern": [
        {
          "regexp": "^([^:]+):(\\d+):(\\d+): (.+) \\[([^\\]]+)\\]$",
          "file": 1,
          "line": 2,
          "column": 3,
          "message": 4,
          "code": 5
        }
      ]
    },
    {
      "owner": "prim-format-drift",
      "pattern": [
        {
          "regexp": "^([^:]+): (does not match prim's canonical format.*)$",
          "file": 1,
          "message": 2
        }
      ]
    }
  ]
}
```

Register it in a workflow step before running prim:

```yaml
- run: echo "::add-matcher::.github/problem-matchers/prim.json"
- run: prim lint .
```

## What prim formats

Parsed formats (structured canonical formatting plus whitespace hygiene), by
extension: `.md`, `.markdown`, `.json`, `.jsonc`, `.yaml`, `.yml`, `.toml`.

Orphan allowlist (whitespace hygiene only) — un-owned text files matched by
exact name or pattern. An entry qualifies when it is committed repository
connective tissue whose syntax is unaffected by the three hygiene operations
(trailing-whitespace removal, one final line-feed, LF endings) — hygiene never
re-indents, so tab-indented files keep their tabs:

| Kind          | Entries                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ignore files  | `.gitignore`, `.gitattributes`, `.dockerignore`, `.npmignore`, `.eslintignore`, `.prettierignore`, `.primignore`, `.helmignore`, `.containerignore` |
| Repo metadata | `CODEOWNERS`, `.mailmap`, `.gitmodules`, `.editorconfig`, `AUTHORS`, `CONTRIBUTORS`, `NOTICE`, `COPYING`, `LICENSE*`                                |
| Containers    | `Dockerfile`, `Dockerfile.*`, `Containerfile`                                                                                                       |
| Plain text    | `*.txt`, `*.text`                                                                                                                                   |

Everything else — source code, unknown types, binaries — is left byte-for-byte
unchanged. `.env` files are deliberately excluded: their values are data and may
be whitespace-sensitive. `.gitconfig` and `.git/config` are excluded for a
different reason — they share `.gitmodules`'s syntax, but are user- and
machine-local rather than committed.

Whitespace hygiene also strips a leading UTF-8 BOM (`U+FEFF`), unconditionally,
from every file prim processes (parsed formats and orphans alike).

## Generated files

prim declines four files outright, regardless of their type, because a tool
generates them and rewrites prim's output again on the next run:

| File                  | Generator |
| --------------------- | --------- |
| `package-lock.json`   | npm       |
| `npm-shrinkwrap.json` | npm       |
| `pnpm-lock.yaml`      | pnpm      |
| `packages.lock.json`  | NuGet     |

A directory walk skips a listed file silently. Naming one explicitly on the
command line skips it too, with a warning on stderr. For those two path-based
cases, the list behaves as the weakest `.primignore` layer (AD-0011): a
committed `!name` line re-includes the file, and `--no-primignore` disables the
built-in list along with the rest of the `.primignore` stack. The `!name` line
works where nothing above the file is excluded, which is the documented recipe
(`!package-lock.json` at the repository root). Where the `.primignore` stack
leaves a directory holding the file excluded, that exclusion wins and the
negation never reaches the built-in list — gitignore's rule that a `!` rule
cannot re-include a path under an excluded directory. A directory a later `!`
line puts back is not excluded, so an override under it still applies.

`--stdin-filepath` and an editor's format-on-save request skip a listed file
without a warning: stdin echoes the input back unchanged, and the LSP formatting
request returns no edits. Neither escape hatch applies there — a `!name` line
and `--no-primignore` have no effect over `--stdin-filepath` or the LSP, because
neither path consults `.primignore` at all.

## Configuration

prim honors [`.editorconfig`](https://editorconfig.org) as its **only** style
configuration — there is no `prim.toml` and there are no per-rule flags to
configure a rule's options or run a rule prim did not select. With no
`.editorconfig` present, prim applies its built-in canonical style (LF endings,
trailing whitespace stripped, exactly one final newline, two-space indent).

Markdown content lint does not add a second config source: `.editorconfig`
remains prim's only user-facing configuration file, including the documented
`prim_*` keys below. There are two documented exceptions to "no per-rule flags".
`prim_mdlint_disable` is subtract-only: it can remove a rule from the tier prim
already selected for a path, but it can never add one or change a rule's
behaviour. `prim_mdlint_report_line_length` selects MD013 into that same tier,
and `max_line_length` — a value the repository already sets for the formatter —
supplies its limit, so the formatter and the linter cannot disagree about the
width. Neither key reaches any other option of any rule. See the scope notes
below.

prim resolves the standard `.editorconfig` cascade for each file: it walks up
the directory tree, stops at the nearest `root = true`, and applies matching
per-glob sections (e.g. `[*.md]`). With `--stdin-filepath`, the cascade is
resolved relative to that path's directory.

Honored keys (standard EditorConfig keys plus prim's closed custom-key set):

| Key                              | Effect                                                                                                                                                |
| -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `end_of_line`                    | `lf` (default) or `crlf`; the emitted line ending.                                                                                                    |
| `trim_trailing_whitespace`       | `true` (default) strips trailing whitespace; `false` preserves it.                                                                                    |
| `insert_final_newline`           | `true` (default) keeps one final newline; `false` strips it.                                                                                          |
| `indent_style`                   | `space`/`tab` — drives JSON/JSONC, TOML, and YAML indentation.                                                                                        |
| `indent_size`                    | indent width for the JSON/JSONC, TOML, and YAML formatters. Applies on its own; `indent_style` is not required.                                       |
| `max_line_length`                | line width for the structured formatters (default 80).                                                                                                |
| `prim_mdlint_strict`             | `false` (default) = floor tier; `true` = add strict tier for Markdown lint.                                                                           |
| `prim_mdlint_disable`            | comma-separated rule ids to exclude from the tier selected for a matching path (Markdown only, unset by default); `none` or `unset` excludes nothing. |
| `prim_mdlint_report_line_length` | `false` (default) or `true`; report Markdown lines longer than `max_line_length` (MD013).                                                             |

Scope notes:

- prim treats files as UTF-8; `charset` values other than `utf-8` are not
  supported (a non-UTF-8 file is left unchanged and reported).
- `end_of_line = cr` (bare carriage return) is treated as `lf`.
- `prim_mdlint_strict`, `prim_mdlint_disable` and
  `prim_mdlint_report_line_length` are currently the **only** documented
  `prim_*` keys.
- `prim_mdlint_report_line_length` resolves through the same per-glob cascade as
  `prim_mdlint_strict`. It selects MD013 into the tier the path already runs; it
  does not change the tier, and it does not let a repository configure MD013's
  options — prim sets those itself, and one of them varies by tier. The limit is
  the same `max_line_length` the formatter wraps to, so enabling the key cannot
  make prim report a line prim itself produced, except a heading the formatter
  was never able to wrap.
- `prim_mdlint_disable` resolves through the same per-glob cascade as
  `prim_mdlint_strict`: EditorConfig's ordinary last-match-wins resolution
  applies per section, so a narrower section's value **replaces** a wider
  section's list — it does not merge with it. Rule ids match case-insensitively.
  The key is subtract-only: it removes rules from the tier prim already selected
  for that path, and can never add a rule prim did not select. A value of `none`
  or `unset` (EditorConfig's own reserved word for clearing an inherited value)
  excludes nothing — use it to drop a wider section's list for a narrower glob.
  An id that names no rule prim runs in either tier disables nothing; prim
  reports it on stderr, naming the `.editorconfig` file, line and section it was
  written in, once per run for each section that carries it, and the exit code
  is unaffected.
- Any other `prim_*` entry is silently ignored. That is intentional: `prim_*` is
  a closed allowlist, not a generic extension hook or a second config file.
- Standard EditorConfig keys and documented `prim_*` keys resolve together for
  the same file; custom keys do not interfere with `Style` resolution.
- prim reports a broken `.editorconfig` when the file has a valid first
  `[section]` header and its first invalid line comes after that header. It then
  warns, and the built-in canonical style applies for the whole cascade —
  including any `.editorconfig` that was read successfully before it.
- Anything that goes wrong earlier in the file is normally silent. An
  `.editorconfig` prim cannot open, and one whose first invalid line is at or
  before its first `[section]` header — an unclosed `[*.md`, for instance — are
  skipped without a warning, and the walk continues past them. A broken section
  header at the top of a file is the common typo, so being malformed is not on
  its own enough to get a file reported; where the broken line sits is what
  decides.
- The exception is a byte-order mark immediately before a first-line `[section]`
  header, which is reported rather than skipped. `ec4rs` strips the mark when it
  first classifies that line but not when it re-reads it, so the file is
  accepted and then rejected on line 1.
- Skipping a file silently can change resolution in both directions. The
  settings that file would have contributed go missing, and if it carried
  `root = true`, prim never sees that boundary and keeps reading `.editorconfig`
  files above it. So a file can resolve with a `max_line_length` back at prim's
  default, or with an `indent_style` inherited from a directory that
  `root = true` was supposed to cut off, and nothing is printed on stderr either
  way.

> **Status:** prim applies whitespace hygiene (trailing-whitespace removal,
> final newline, line endings) — driven by `.editorconfig` — to every file it
> owns, and structured canonical formatting to all of its parsed formats:
> JSON/JSONC (consistent indentation, one space after `:`, no trailing commas),
> TOML (canonical spacing, inline-table style and array line structure
> preserved), YAML (canonical layout with anchors/aliases and block scalar
> styles preserved), and Markdown (ATX headings, normalized lists/tables, and
> prose hard-wrapped to `max_line_length` with guardrails — inline code, links,
> tables, and fenced code are never broken, and fenced code is preserved
> verbatim). All formats preserve comments and never reorder. See the
> [Specification](SPEC.md).

## Format notes

- `.json` files are parsed leniently as JSONC: comments and trailing commas are
  accepted on input (trailing commas are removed on output). prim never rejects
  a `.json` file for containing comments (AD-0003).
