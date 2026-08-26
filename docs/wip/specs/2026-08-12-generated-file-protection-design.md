# Generated-file protection — design

Status: approved for planning, 2026-08-12.

prim formats **authored** files. A generated file belongs to the tool that
generates it, so prim leaves it byte-for-byte alone.

## Problem

prim currently formats four committed, tool-generated files, and the files it
does not format are safe only by accident.

Measured on `prim 0.3.0`:

- **`pnpm-lock.yaml` is actively mangled.** prim rewrites `'9.0'` to `"9.0"` and
  explodes pnpm's flow mappings across several lines:

  ```yaml
  # pnpm writes
  resolution: {integrity: sha512-CQpnWPr...==}
  # prim rewrites to
  resolution: {
    integrity: sha512-CQpnWPr...==,
  ```

  pnpm regenerates the file wholesale on every install, so the change is pure
  churn that returns on the next `pnpm install`.

- **`package-lock.json` is rewritten whenever the repo's style is not prim's.**
  With `[*.json] indent_style = space, indent_size = 4` in `.editorconfig`, a
  92-line generated lockfile produced a diff of 178 changed lines — every line
  reindented. Under prim's default 2-space style it is a no-op, because npm also
  writes 2-space, which is what makes the exposure easy to miss.

- **The protection that does hold is accidental.** `Cargo.lock`, `uv.lock`,
  `poetry.lock`, `deno.lock`, `composer.lock`, `yarn.lock`, `flake.lock`,
  `bun.lock`, `Pipfile.lock`, and `.terraform.lock.hcl` are skipped only because
  `.lock` and `.hcl` are not extensions prim owns. Nothing states the intent, so
  a future extension to the format surface would silently start formatting them.

## Principle

Do not fight the tool that owns a file. For a _style_ disagreement the owning
tool has a convention prim should match; for a _generated_ file the tool owns
every byte, so prim declines entirely. This is the same principle that produced
the `.primignore` recipe for a generated `CHANGELOG.md` and the AD-0009 decision
that `.primignore` binds however prim is invoked.

## Scope

### Admission rule

A file joins the built-in list only when all three hold:

1. The generating tool's own documentation describes the file as generated and
   not hand-edited.
2. The file is conventionally committed to the repository (an uncommitted file
   is already out of reach).
3. The file is inside prim's format surface, so listing it changes behaviour.

### The list

| File                  | Generator |
| --------------------- | --------- |
| `package-lock.json`   | npm       |
| `npm-shrinkwrap.json` | npm       |
| `pnpm-lock.yaml`      | pnpm      |
| `packages.lock.json`  | NuGet     |

Matching is on the final path component, exact and case-sensitive, consistent
with `classify()`.

### Deliberate exclusions

Recorded so they are not re-litigated:

- **Lockfiles outside the format surface** (`Cargo.lock`, `uv.lock`,
  `poetry.lock`, `deno.lock`, `composer.lock`, `yarn.lock`, `flake.lock`,
  `bun.lock`, `Pipfile.lock`, `.terraform.lock.hcl`). Listing them would be a
  no-op today. They are safe by classification, not by this list, and the
  decision record must say so.
- **`CHANGELOG.md`.** Generated in some workflows, hand-authored in many others
  (Keep a Changelog). A built-in ignore would silently stop formatting a file a
  large share of users do author. It stays a `docs/recipes.md` `.primignore`
  recommendation.
- **`pnpm-workspace.yaml`.** Authored configuration, not generated.
- **Generated directories** (`node_modules/`, `target/`, `dist/`). Already
  skipped through `.gitignore` in practice. Directory-level ignores are a
  different risk profile and are out of scope for v1.

## Design

### Component: the predicate

A new pure module in the engine, `prim-fmt/src/generated.rs`:

```rust
/// True when `path` names a file its generating tool owns outright, which prim
/// must leave byte-for-byte unchanged.
pub fn is_generated(path: &Path) -> bool
```

No I/O, keyed on the final path component — the same shape as `classify()`, and
it keeps AD-0001's pure-engine boundary intact.

It is deliberately _not_ folded into `classify()`. `package-lock.json` genuinely
is JSON; `classify()` answering `None` would misstate the file's type and would
make `prim explain` report "not a file type prim formats", which is false. The
two questions are separate: _what is this file_ and _may prim touch it_.

### Component: enforcement at every entry point

`is_generated` is consulted at all three places a file reaches the formatter, so
the guarantee does not depend on how prim was invoked:

1. **Discovery** (`prim-cli/src/discover.rs`) — as a built-in ignore layer.
2. **stdin** (`--stdin-filepath`) — input echoes to stdout unchanged.
3. **LSP** (`prim-cli/src/lsp/`) — a formatting request returns no edits.

The LSP path is not optional. Without it, opening `package-lock.json` in an
editor configured to format on save with prim would still rewrite the file,
which is the most damaging path of the three because it is silent.

### Precedence

The built-in list behaves as the outermost `.primignore` layer — weaker than any
`.primignore` the repository commits, so a nearer rule always wins:

```text
built-in generated list   <   .primignore (any depth)
```

`--no-primignore` is not a layer but a switch: it disables the whole stack,
built-in list included.

So a `!package-lock.json` line in a committed `.primignore` re-includes the
file, and `--no-primignore` processes it without any edit to the repository.
This reuses the layering AD-0009 established rather than adding a second
mechanism and a second flag.

### Reporting

Follows AD-0009's rule exactly, because the surprise is the same shape:

- Reached by a directory walk — skipped silently. Filtering is what a walk is
  for.
- Named on the command line — skipped, with a warning naming the reason:

  ```text
  warning: package-lock.json: generated by npm; skipped (prim formats authored
  files, use --no-primignore to process it)
  ```

Warnings never raise the exit code, so hooks that pass a staged-file list
continue to pass.

### Error handling

There is no new failure mode. `is_generated` is a total function over a path and
performs no I/O, so it cannot fail. A generated file that is also unreadable or
malformed is skipped before it is ever read, which strictly reduces the error
surface.

## Non-goals

- No new configuration surface: no `prim.toml`, no per-file flags, no
  `--no-generated`. FR-3.3 stays intact; `.primignore` negation and
  `--no-primignore` are the escape hatches.
- No whitespace hygiene on generated files. Bytes are left completely alone,
  including a missing final newline.
- No content sniffing or "looks generated" heuristics. Name-based only, matching
  FR-2.5.
- No directory-level entries.

## Testing

| Layer                          | Case                                                          |
| ------------------------------ | ------------------------------------------------------------- |
| `prim-fmt` unit                | `is_generated` true for each listed name                      |
| `prim-fmt` unit                | false for `package.json`, `pnpm-workspace.yaml`, `Cargo.toml` |
| `prim-fmt` unit                | false for a path merely _containing_ a listed name            |
| `prim-cli` behavioural         | a walk skips the file and stays silent                        |
| `prim-cli` behavioural         | an explicit path warns, exits `0`, leaves bytes identical     |
| `prim-cli` behavioural         | `--no-primignore` processes it                                |
| `prim-cli` behavioural         | `!package-lock.json` in `.primignore` re-includes it          |
| `prim-cli` behavioural         | `fmt --check` on an explicit generated path exits `0`         |
| `prim-cli` LSP                 | a formatting request returns no edits                         |
| `prim-cli` behavioural (stdin) | `--stdin-filepath package-lock.json` echoes input unchanged   |

The regression that motivated this work gets a named test: a `pnpm-lock.yaml`
fixture carrying flow mappings and single-quoted scalars must survive `prim fmt`
byte-for-byte.

## Artifacts

- `docs/decisions/0011-generated-files-are-not-formatted.md` — the principle,
  the admission rule, the list, and the exclusions with their reasons.
- `docs/SPEC.md` — a new requirement under FR-2 (scope), cross-referencing
  FR-4.4. FR-2.4 already says every file prim does not own is left byte-for-byte
  unchanged; this extends that to files prim owns but declines.
- `docs/USAGE.md` — the list, the precedence, and how to override.
- `docs/recipes.md` — a note that generated files need no `.primignore` entry,
  and that `CHANGELOG.md` still does.

## Alternatives considered

- **Add the names to `classify()`'s not-owned path.** Rejected: it would make
  `prim explain package-lock.json` report the file is not a type prim formats,
  which is untrue, and it conflates file type with permission to write.
- **A `--no-generated` flag.** Rejected: a second flag for a second ignore layer
  when `--no-primignore` already means "process what prim would skip". YAGNI,
  and it widens the surface FR-3.3 exists to keep narrow.
- **Pattern matching (`*.lock.json`, `*-lock.yaml`).** Rejected for v1:
  over-matches authored files that happen to be named that way, and the
  evidence-based admission rule is what keeps the list defensible.
- **Format generated files but only apply hygiene.** Rejected: a final-newline
  fixup on a file the generator rewrites wholesale is still churn, and it makes
  the rule "prim leaves it alone, except sometimes".

## Follow-ups, not in this spec

- `resolve_indent` drops `indent_size` when `indent_style` is absent
  ([`editorconfig.rs:166`](../../../crates/prim-cli/src/editorconfig.rs)) — an
  independent bug, prerequisite for the ecosystem-profiles work.
- Ecosystem profiles: `Cargo.toml` at 100/4 per the Rust Style Guide, and
  `pyproject.toml` array collapsing, which fights `uv add`. Separate spec.
