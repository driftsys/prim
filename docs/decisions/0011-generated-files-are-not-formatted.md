# AD-0011 — Generated files are not formatted

## Status

Accepted. New behavior: prim no longer treats `package-lock.json`,
`npm-shrinkwrap.json`, `pnpm-lock.yaml`, and `packages.lock.json` as ordinary
JSON/YAML input. `fmt`, `lint`, `fix`, `--stdin-filepath`, and the LSP
formatting request all leave a listed file byte-for-byte unchanged by default.

## Context

prim formats **authored** files. A generated file belongs to the tool that
generates it, and prim rewriting it is pure churn: the generator overwrites
prim's output again on the next run, so the only lasting effect is diff noise
and, for a file a pre-commit hook re-stages, a committed change nobody wrote.

Measured on prim 0.3.0, before this change:

- `pnpm-lock.yaml` was actively mangled: prim rewrote `'9.0'` to `"9.0"` and
  exploded pnpm's flow mappings across several lines, turning a single-line
  `resolution: {integrity: sha512-CQpnWPr...==}` into a multi-line block.
- `package-lock.json` was rewritten whenever a repository's `.editorconfig`
  style did not match prim's default. Under `[*.json] indent_size = 4`, a
  92-line generated lockfile produced a 178-line diff — every line reindented.
  Under prim's default 2-space style the same file is a no-op, because npm also
  writes 2-space, which is what made the exposure easy to miss.
- The protection that did hold was accidental. `Cargo.lock`, `uv.lock`,
  `poetry.lock`, `deno.lock`, `composer.lock`, `yarn.lock`, `flake.lock`,
  `bun.lock`, `Pipfile.lock`, and `.terraform.lock.hcl` were skipped only
  because `.lock` and `.hcl` are not extensions prim owns (FR-2.4). Nothing
  stated the intent, so a future extension of the format surface could start
  formatting them without anyone deciding it should.

This is the same principle that produced the `docs/recipes.md` recommendation to
add a generated `CHANGELOG.md` to `.primignore`, and the AD-0009 decision that
`.primignore` binds however prim is invoked. The question here is whether that
protection should stay a per-repository opt-in or ship built in for the files
where the answer is always the same.

## Options

**A. Keep relying on a per-repository `.primignore` entry, as `docs/recipes.md`
already recommends for `CHANGELOG.md`.** A `CHANGELOG.md` is right to leave as
an opt-in: some projects generate it, many hand-author it (Keep a Changelog), so
only the repository knows which applies. A lockfile named `package-lock.json` is
not repository-specific — it means the same thing in every npm project — so the
fix and the harm are both universal. Leaving it to each repository to notice and
add four names to `.primignore` means some fraction of them hit the measured
harm above before anyone does. Rejected.

**B. Ship a built-in, name-keyed list that behaves as the outermost
`.primignore` layer.** Reuses the layering AD-0009 already established — weaker
than any `.primignore` the repository commits, disabled wholesale by
`--no-primignore` — instead of adding a second mechanism or a second flag.
Chosen.

## Decision

1. A new pure predicate in the engine, `prim_fmt::generated_by`, answers "which
   tool generates this file":

   ```rust
   pub fn generated_by(path: &Path) -> Option<&'static str>
   ```

   Keyed on the final path component, exact and case-sensitive — the same
   matching `classify()` uses (FR-2.5) — and no I/O, keeping the engine's
   pure-crate boundary (AD-0001).

   The predicate is deliberately **not** folded into `classify()`.
   `package-lock.json` genuinely is JSON; `classify()` answering `None` for it
   would make `prim explain package-lock.json` report that the file is not a
   type prim formats, which is false. "What is this file" and "may prim write to
   it" are separate questions, and only the second one generated files answer
   differently from their ordinary type.

2. A file joins the built-in list only when all three hold:
   1. The generating tool's own documentation describes the file as generated
      and not hand-edited.
   2. The file is conventionally committed to the repository — an uncommitted
      file is already out of reach.
   3. The file is inside prim's format surface, so listing it changes behavior.

   The list today:

   | File                  | Generator |
   | --------------------- | --------- |
   | `package-lock.json`   | npm       |
   | `npm-shrinkwrap.json` | npm       |
   | `pnpm-lock.yaml`      | pnpm      |
   | `packages.lock.json`  | NuGet     |

3. `generated_by` is consulted at all five places a file reaches the formatter
   or its diagnostics, so the guarantee does not depend on how prim was invoked:
   directory discovery (`prim-cli/src/discover.rs`), `--stdin-filepath` for
   `fmt`/`fix` (`prim-cli/src/app.rs`), `--stdin-filepath` for `lint`
   (`prim-cli/src/app.rs`), the LSP formatting request
   (`prim-cli/src/lsp/server.rs`), and the LSP `didOpen`/`didChange` diagnostics
   notification (`prim-cli/src/lsp/server.rs`). The LSP formatting path is not
   optional: without it, an editor configured to format `package-lock.json` on
   save with prim would still rewrite the file, silently, which is the most
   damaging of the five paths because nothing prints a warning to notice it by.
   The LSP diagnostics path guards a narrower but still real failure: an editor
   open on a generated file must never show findings the user can never clear,
   because `formatting` correctly returns no edits for it.

4. For the two path-based entry points (directory discovery and an explicitly
   named path, both in `discover.rs`), the built-in list sits inside the
   `.primignore` stack rather than beside it. A committed `!package-lock.json`
   line re-includes the file — the negation must name the file specifically: its
   final path segment, after the `!`, must be a literal equal to the file's
   name, containing none of the glob metacharacters `*`, `?`, `[`, `{` (so
   `!package-lock.json`, `!**/package-lock.json`, and
   `!vendor/package-lock.json` all re-include it; `!*.json` and `!*` — the
   latter a no-op negation under gitignore semantics, since nothing precedes it
   to re-include — do not, even though the `ignore` crate reports all four as
   `Match::Whitelist`). This mirrors AD-0009's rule that a path must be _named_,
   not merely matched, to take precedence, and applies only to the
   generated-list override — ordinary `.primignore` whitelisting of a
   non-generated file is unaffected. `--no-primignore` disables the built-in
   list along with the rest of the `.primignore` stack. There is no separate
   flag. `--stdin-filepath` and the LSP formatting request never consult
   `.primignore` at all, so neither escape applies to them: on those two paths,
   `generated_by` is unconditional, and there is no way to make prim format a
   listed file over stdin or through the LSP. The `.primignore` files consulted
   for a path named on the command line are those from its directory up to the
   repository root (the nearest ancestor containing `.git`), or up to the
   current working directory when no repository is found — never beyond it, so a
   `.primignore` outside the repository (for example one left in `$HOME`) cannot
   silently disable the built-in list for every repository beneath it.

5. Reporting follows AD-0009's rule exactly, because the surprise is the same
   shape: reached by a directory walk, a generated file is skipped silently —
   filtering is what a walk is for. Named explicitly, it is skipped with a
   warning:

   ```text
   package-lock.json: generated by npm; skipped (prim formats authored files,
   use --no-primignore to process it)
   ```

   Warnings never raise the exit code, so a hook that passes a staged-file list,
   lockfile included, still passes.

6. No whitespace hygiene applies to a generated file. It is skipped before it is
   ever read, so the bytes — including a missing final newline — are left
   completely alone. This is stricter than FR-2.4's "not owned" byte-for-byte
   promise applying to it by omission: a generated file **is** a type prim owns,
   and the byte-for-byte promise applies to it anyway, by this decision.

7. Deliberate exclusions, recorded so they are not re-litigated:
   - **Lockfiles outside the format surface** (`Cargo.lock`, `uv.lock`,
     `poetry.lock`, `deno.lock`, `composer.lock`, `yarn.lock`, `flake.lock`,
     `bun.lock`, `Pipfile.lock`, `.terraform.lock.hcl`). Listing them today
     would be a no-op: they are safe because prim does not parse `.lock` or
     `.hcl` at all (FR-2.4), not because of this list. If the format surface
     ever grows to cover one of these extensions, that same change must add the
     corresponding name here.
   - **`CHANGELOG.md`.** Generated in some workflows, hand-authored in many
     others. A built-in entry would silently stop formatting a file a large
     share of users do author, so it stays a `docs/recipes.md` `.primignore`
     recommendation rather than a built-in one.
   - **`pnpm-workspace.yaml`.** Authored configuration, not generated by pnpm —
     it is the input pnpm reads, not output it writes.
   - **Generated directories** (`node_modules/`, `target/`, `dist/`). Already
     skipped through `.gitignore` in practice. A directory-level entry is a
     different risk profile — matching a path prefix rather than an exact name —
     and is out of scope for this decision.

## Consequences

- prim no longer rewrites the four listed files, regardless of the repository's
  `.editorconfig` style. The `pnpm-lock.yaml` mangling and the
  `package-lock.json` reindentation measured in Context stop happening on every
  affected repository, not just ones that remembered to add `.primignore`
  entries.
- `prim explain package-lock.json` continues to report the file's true type and
  the `.editorconfig` settings that would apply to it. Only `fmt`, `lint`,
  `fix`, `--stdin-filepath`, and the LSP decline to act on that type; the
  predicate does not change what `classify()` reports.
- The `.lock`/`.hcl` lockfiles remain safe by classification, not by this list.
  A future PR that extends the format surface to a `.lock` or `.hcl` extension
  must add the corresponding name to `GENERATED` in the same change, or the
  protection this decision documents silently stops applying to it.
- `CHANGELOG.md` still needs a per-repository `.primignore` entry; this decision
  does not extend the built-in list to it, on purpose.
- For a path reached by directory discovery or named explicitly, anyone who
  wants prim to format a listed file anyway has two ways to say so without a
  code change: a `!name` line in a committed `.primignore`, or `--no-primignore`
  for a one-off run. Neither works over `--stdin-filepath` or the LSP: those two
  paths never consult `.primignore`, so the decline there is unconditional
  (Decision item 4).
- Matching is case-sensitive, the same rule `classify()` uses (Decision item 1,
  FR-2.5). On a case-insensitive filesystem (for example, the default macOS APFS
  configuration), a differently cased name on disk does not match `GENERATED`:
  `prim fmt Package-Lock.json` in a directory that also contains
  `package-lock.json` formats the file, because the filesystem treats the two
  names as the same file while the name comparison does not. This is the
  opposite failure mode from `classify()`, where a miss leaves the file alone;
  here a miss writes to it. Accepted because the four generated names are
  written by their tools in exactly one case, so a differently cased name on
  disk means a human renamed the file — a case the pure, no-I/O predicate
  (Decision item 1) has no filesystem access to detect.

## Alternatives considered

- **Fold the four names into `classify()`'s not-owned path.** Rejected:
  `package-lock.json` genuinely is JSON. Answering `None` for it would make
  `prim explain package-lock.json` report that the file is not a type prim
  formats, which is false, and it would conflate "what is this file" with "may
  prim write to it" — the two questions Decision item 1 keeps separate.
- **A `--no-generated` flag.** Rejected: `--no-primignore` already means
  "process what prim would otherwise skip," and the built-in list is designed to
  sit inside that same stack for exactly that reason. A second flag for a second
  ignore layer duplicates it for no new capability and widens the surface FR-3.3
  exists to keep narrow.
- **Pattern matching, for example `*.lock.json` or `*-lock.yaml`.** Rejected for
  v1: a glob over-matches authored files that happen to share the shape of a
  generated name. The three-part admission rule in Decision item 2 is what keeps
  a short, explicit list defensible; a pattern trades that away for a
  convenience nobody asked for.
- **Format generated files but apply only whitespace hygiene.** Rejected: the
  generating tool rewrites the file wholesale on every run, so even a
  final-newline fixup is pure churn that returns on the next install. It would
  also leave the rule as "prim leaves generated files alone, except sometimes,"
  which is harder to state and to test than "prim leaves them alone."

---

Satisfies: FR-2.7. Related: AD-0001 (pure-engine crate boundary), AD-0009
(`.primignore` layering — this list sits inside the same stack), FR-2.4/FR-2.5
(format surface and name-based matching), `crates/prim-fmt/src/generated.rs`,
`crates/prim-cli/src/discover.rs`, `crates/prim-cli/src/app.rs`,
`crates/prim-cli/src/lsp/server.rs`.
