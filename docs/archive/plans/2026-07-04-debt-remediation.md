# Debt Remediation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the contract and documentation debt found in the post-v1 design
review: a style-stability contract, no silent CLI failures, docs that match the
implementation, and a curated orphan allowlist.

**Architecture:** Five independent, single-focus PRs plus one local (no-PR)
cleanup. No new crates, no new runtime dependencies. The only new public API is
`ui::resolve_color`; `discover::collect` becomes fallible; PR 5 fixes the FR-1.6
markdown-fence bug inside `prim-fmt`.

**Tech Stack:** Rust (cargo workspace), `assert_cmd`/`predicates`/`tempfile` for
behavioural tests, `trycmd` for CLI snapshots, `just` for gates.

**Spec:** `docs/wip/specs/2026-07-04-debt-remediation-design.md`

## Global Constraints

- Conventional Commits, imperative mood (`feat`, `fix`, `docs`, `test`,
  `chore`). git-std allows only the scopes `prim-cli`, `prim-fmt`, `release` —
  or no scope. Every commit message ends with the trailer:
  `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`
- Run `just fmt` before every commit (formats Rust + Markdown).
- Zero warnings: `cargo clippy --workspace --all-targets -- -D warnings`.
- `prim-fmt` stays pure: no clap, I/O, or terminal dependencies.
- PR-based workflow — never push to `main`. Run `just verify` before each PR.
- Merge order: PR 1 → PR 2 → PR 3 → PR 4 → PR 5 (they touch overlapping doc
  files, and PR 5 re-blesses the PR 2 corpus; rebase each branch on `main`
  before opening its PR).
- Doc comments on all public items.

---

## PR 1 — docs truth-sync (branch `docs/debt-remediation`, already created)

### Task 1: Fix the stale status docs

**Files:**

- Modify: `README.md:16-20` (status blockquote)
- Modify: `crates/prim-fmt/src/lib.rs:13-16` (module doc)
- Modify: `crates/prim-fmt/src/classify.rs:6-8` (`FileKind` doc)

**Interfaces:** none (docs only).

- [ ] **Step 1: Replace the README status block**

Old (README.md, the `> **Status:** early. …` blockquote):

```markdown
> **Status:** early. prim does recursive discovery and applies **whitespace
> hygiene** (trailing-whitespace removal, single final line-feed, LF endings) to
> the parsed formats and the orphan allowlist. Structured per-format formatting
> (JSON/YAML/TOML/Markdown) and `.editorconfig` resolution land in later
> milestones. See [docs/SPEC.md](docs/SPEC.md).
```

New:

```markdown
> **Status:** v1 complete — all v1 requirements (FR-1 through FR-6) are
> implemented: recursive discovery, whitespace hygiene, `.editorconfig` style
> resolution, structured JSON/JSONC, TOML, YAML, and Markdown formatting (with
> prose-wrap guardrails), `--check` / `--diff` / `--stdin-filepath` modes, and
> atomic writes. See [docs/SPEC.md](docs/SPEC.md).
```

- [ ] **Step 2: Replace the stale `lib.rs` module-doc paragraph**

Old (crates/prim-fmt/src/lib.rs):

```rust
//! At this stage [`format`] applies only the format-agnostic **whitespace
//! hygiene** pass (trailing-whitespace removal, single final line-feed, LF line
//! endings). Structured per-format canonicalisation (Markdown wrapping, JSON
//! re-indentation, …) is added per [`FileKind`] in later milestones.
```

New:

```rust
//! [`format`] applies structured canonicalisation to the parsed formats —
//! JSON/JSONC via `dprint-plugin-json`, TOML via `taplo`, YAML via
//! `pretty_yaml`, Markdown via `dprint-plugin-markdown` — followed by the
//! format-agnostic **whitespace hygiene** pass (trailing-whitespace removal,
//! single final line-feed, configured line endings). `Orphan` files receive
//! hygiene only.
```

- [ ] **Step 3: Fix the `FileKind` doc comment**

Old (crates/prim-fmt/src/classify.rs):

```rust
/// The kind of file prim recognises. Parsed formats gain structured
/// canonicalisation in later milestones; `Orphan` files (the un-owned text
/// allowlist) only ever receive whitespace hygiene.
```

New:

```rust
/// The kind of file prim recognises. Parsed formats receive structured
/// canonicalisation plus whitespace hygiene; `Orphan` files (the un-owned text
/// allowlist) only ever receive whitespace hygiene.
```

- [ ] **Step 4: Verify**

Run: `cargo test -p prim-fmt && just fmt && git diff --stat` Expected: tests
pass; `just fmt` changes nothing beyond your edits.

- [ ] **Step 5: Commit**

```bash
git add README.md crates/prim-fmt/src/lib.rs crates/prim-fmt/src/classify.rs
git commit -m "docs(prim-fmt): sync status docs with the implemented v1 reality" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 2: SPEC truth fixes

**Files:**

- Modify: `docs/SPEC.md` (FR-1.3, FR-2.1, FR-3.2, FR-5.3)

**Interfaces:** none (docs only).

- [ ] **Step 1: FR-2.1 — record the `.editorconfig` precedence**

Old:

```markdown
- **FR-2.1** For every file it processes, prim shall remove trailing whitespace
  from each line.
```

New:

```markdown
- **FR-2.1** For every file it processes, prim shall remove trailing whitespace
  from each line, unless `.editorconfig` sets `trim_trailing_whitespace = false`
  (FR-3.2 takes precedence).
```

- [ ] **Step 2: FR-1.3 — record the lenient `.json` handling (AD-0003)**

Old:

```markdown
- **FR-1.3** prim shall format JSONC, preserving all comments in position.
  (JSON5 excluded.)
```

New:

```markdown
- **FR-1.3** prim shall format JSONC, preserving all comments in position.
  `.json` files are parsed with the same lenient JSONC parser: comments and
  trailing commas are accepted on input and never emitted (AD-0003). (JSON5
  excluded.)
```

- [ ] **Step 3: FR-3.2 — drop `charset` (aligns with AD-0002)**

Old:

```markdown
- **FR-3.2** prim shall read `.editorconfig` and honor `indent_style`,
  `indent_size`, `max_line_length`, `end_of_line`, `charset`,
  `insert_final_newline`, `trim_trailing_whitespace` — including the `root=true`
  chain and per-glob sections.
```

New:

```markdown
- **FR-3.2** prim shall read `.editorconfig` and honor `indent_style`,
  `indent_size`, `max_line_length`, `end_of_line`, `insert_final_newline`,
  `trim_trailing_whitespace` — including the `root=true` chain and per-glob
  sections. (`charset` is out of scope: prim processes UTF-8 only — FR-6.5,
  AD-0002.)
```

- [ ] **Step 4: FR-5.3 — record the `--diff` exit code**

Old:

```markdown
- **FR-5.3** `--diff` shall print a unified diff of pending changes and write
  nothing.
```

New:

```markdown
- **FR-5.3** `--diff` shall print a unified diff of pending changes and write
  nothing; it shall exit `0` whether or not changes are pending (`--check` is
  the CI gate).
```

- [ ] **Step 5: Verify and commit**

Run: `just fmt && cargo run -q -p prim-cli -- --check docs/SPEC.md` Expected: no
output (SPEC is canonical).

```bash
git add docs/SPEC.md
git commit -m "docs(prim-cli): record charset scope, trim precedence, --diff exit code, JSON leniency" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 3: Recipes, USAGE format note, and archive ignores

**Files:**

- Modify: `docs/recipes.md` (new section after "Excluding files")
- Modify: `docs/USAGE.md` (new "Format notes" section after "Configuration")
- Modify: `.gitignore` (after the `docs/wip/` entry)
- Modify: `.markdownlintignore`

**Interfaces:** none (docs only).

- [ ] **Step 1: Add the golden-files recipe**

Insert into `docs/recipes.md`, between the "Excluding files" and "Using prim
with git-std" sections:

````markdown
## Protecting golden files

Test fixtures and golden files often contain deliberate formatting violations
(trailing whitespace, missing final newlines, non-canonical indentation) that
must stay byte-exact. Add those directories to `.primignore` — prim's own
repository does this for its test fixtures:

```gitignore
# .primignore
crates/prim-fmt/tests/correctness/fixtures/
```

Note: `--exclude` and `.primignore` apply to directory walks; a file named
explicitly on the command line is always processed.
````

- [ ] **Step 2: Add the USAGE format note**

Insert into `docs/USAGE.md`, after the "Configuration" section:

```markdown
## Format notes

- `.json` files are parsed leniently as JSONC: comments and trailing commas are
  accepted on input (trailing commas are removed on output). prim never rejects
  a `.json` file for containing comments (AD-0003).
```

- [ ] **Step 3: Ignore `docs/archive/`**

In `.gitignore`, extend the working-memory block:

```gitignore
# Superpowers working memory (specs/plans) — private/local working memory
docs/wip/
docs/archive/
```

In `.markdownlintignore`, add the line `docs/archive/` after `docs/wip/`.

- [ ] **Step 4: Verify and commit**

Run: `just fmt && npx markdownlint-cli docs/recipes.md docs/USAGE.md` Expected:
clean.

```bash
git add docs/recipes.md docs/USAGE.md .gitignore .markdownlintignore
git commit -m "docs(prim-cli): add golden-file recipe, JSON leniency note, and archive ignores" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 4: PR 1 gate

- [ ] **Step 1:** Run `just verify` — commit lint, tests, install tests, lint
      all green.
- [ ] **Step 2:** Open the PR:

```bash
git push -u origin docs/debt-remediation
gh pr create --title "docs: sync docs with implemented v1 reality" --body "$(cat <<'EOF'
Truth-syncs the docs after the post-v1 design review: README status, stale
module docs, SPEC corrections (charset scope, trim precedence, --diff exit
code, JSON leniency), a golden-files recipe, and gitignored docs/archive/.

Docs only — no behavior change.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## PR 2 — style-stability contract (branch `feat/style-stability` from `main`)

### Task 5: Corpus inputs and ignore entries

**Files:**

- Create: `crates/prim-fmt/tests/corpus/input/sample.md`
- Create: `crates/prim-fmt/tests/corpus/input/sample.json`
- Create: `crates/prim-fmt/tests/corpus/input/sample.jsonc`
- Create: `crates/prim-fmt/tests/corpus/input/sample.yaml`
- Create: `crates/prim-fmt/tests/corpus/input/sample.toml`
- Modify: `.primignore`, `.markdownlintignore`

**Interfaces:**

- Produces: the corpus layout Task 6's test walks — `input/<name>` pairs with
  `expected/<name>`, kinds selected by real file extensions.

- [ ] **Step 1: Create the branch**

```bash
git switch main && git pull && git switch -c feat/style-stability
```

- [ ] **Step 2: Write the five input files (deliberately non-canonical)**

`crates/prim-fmt/tests/corpus/input/sample.md`:

````markdown
---
title: Corpus sample
---

# Heading with extra spaces

This paragraph is deliberately written as one very long single line so that the
canonical formatter has to hard-wrap it at the configured width of eighty
columns.

A line with a hard break\
stays two lines. Inline `code span stays atomic` and a
[link](https://example.com/a/deliberately/long/path/that/must/never/be/split)
survives.

| a | long column name |
| - | ---------------- |
| 1 | padded           |

```rust
fn main()    {   }
```
````

`crates/prim-fmt/tests/corpus/input/sample.json`:

```json
{
    "name":"corpus",
        "nested": {"a":1,"b":[1,2,3]},
  "long_array": [1,2,3,4,5]
}
```

`crates/prim-fmt/tests/corpus/input/sample.jsonc`:

```jsonc
{
  // leading comment
  "kept": true, /* inline */
  "trailing": [1, 2, 3,],
}
```

`crates/prim-fmt/tests/corpus/input/sample.yaml`:

```yaml
# top comment
defaults: &defaults
    retries: 3
service:
    inherits: *defaults
    name: corpus # trailing comment
block: |
    line one
    line two
folded: >
    folded
    text
list:
    - one
    - two
```

`crates/prim-fmt/tests/corpus/input/sample.toml`:

```toml
# top comment
title="corpus"

[server]
host  =  "localhost"   # trailing comment
ports=[8001,8002,8003]
point={x=1,y=2}

[a.b]
key = "nested table"
```

- [ ] **Step 3: Exclude the corpus from repo-wide formatting and lint**

Append to `.primignore` (inputs are deliberately non-canonical; expected outputs
must change only via the bless workflow):

```gitignore
# Style-stability corpus: inputs are deliberately non-canonical, and expected
# outputs must change only via PRIM_BLESS (see docs/SPEC.md, Style stability).
crates/prim-fmt/tests/corpus/
```

Append `crates/prim-fmt/tests/corpus/` to `.markdownlintignore`.

- [ ] **Step 4: Commit**

```bash
git add crates/prim-fmt/tests/corpus .primignore .markdownlintignore
git commit -m "test(prim-fmt): add pinned style-corpus inputs" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 6: The stability test (TDD via bless)

**Files:**

- Create: `crates/prim-fmt/tests/stability.rs`
- Create: `crates/prim-fmt/tests/corpus/expected/` (generated by bless)

**Interfaces:**

- Consumes: `prim_fmt::{classify, format, Style}` (existing public API) and Task
  5's corpus layout.
- Produces: the re-bless workflow
  `PRIM_BLESS=1 cargo test -p prim-fmt --test stability` cited by the SPEC
  policy in Task 7.

- [ ] **Step 1: Write the test**

`crates/prim-fmt/tests/stability.rs`:

```rust
//! Style-stability contract: canonical output for a pinned corpus must not
//! change unless the change is deliberate — re-blessed with `PRIM_BLESS=1`,
//! released as a minor version bump, and called out in the CHANGELOG (see
//! docs/SPEC.md, "Style stability").

use std::fs;
use std::path::{Path, PathBuf};

use prim_fmt::{Style, classify, format};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

#[test]
fn canonical_output_is_stable_for_the_pinned_corpus() {
    let input_dir = corpus_dir().join("input");
    let expected_dir = corpus_dir().join("expected");
    let bless = std::env::var_os("PRIM_BLESS").is_some();
    let mut checked = 0;

    for entry in fs::read_dir(&input_dir).expect("corpus input dir exists") {
        let input_path = entry.unwrap().path();
        let name = input_path.file_name().unwrap().to_owned();
        let kind = classify(&input_path).expect("corpus files are owned kinds");
        let source = fs::read_to_string(&input_path).unwrap();
        let formatted =
            format(kind, &source, &Style::default()).expect("corpus inputs parse");

        // Canonical output must itself be canonical (FR-6.1).
        let twice = format(kind, &formatted, &Style::default()).unwrap();
        assert_eq!(formatted, twice, "{name:?}: canonical output not idempotent");

        let expected_path = expected_dir.join(&name);
        if bless {
            fs::create_dir_all(&expected_dir).unwrap();
            fs::write(&expected_path, &formatted).unwrap();
        } else {
            let expected = fs::read_to_string(&expected_path).unwrap_or_default();
            assert_eq!(
                formatted, expected,
                "{name:?}: canonical output changed. If deliberate: re-bless \
                 with PRIM_BLESS=1, bump the minor version, and call it out \
                 in the CHANGELOG (docs/SPEC.md, Style stability)."
            );
        }
        checked += 1;
    }
    assert!(checked >= 5, "corpus must cover all five formats, found {checked}");
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p prim-fmt --test stability` Expected: FAIL — expected files
missing, so the byte-compare panics.

- [ ] **Step 3: Bless the expected outputs**

Run: `PRIM_BLESS=1 cargo test -p prim-fmt --test stability` Expected: PASS; five
files appear under `tests/corpus/expected/`.

- [ ] **Step 4: Inspect every expected file — this review IS the deliverable**

Open each `expected/sample.*` and confirm, against the spec:

- `sample.md`: prose wrapped ≤ 80 columns; hard break preserved; the long URL
  unsplit; inline code intact; the fenced block byte-identical to the input;
  front matter preserved.
- `sample.json`: two-space indent, one space after `:`, no trailing commas.
- `sample.jsonc`: both comments preserved, trailing commas gone.
- `sample.yaml`: two-space indent; anchor/alias, block and folded scalars, and
  both comments preserved.
- `sample.toml`: `key = value` spacing normalized; inline table and comments
  preserved.

If anything violates the spec, stop and report — do not commit.

- [ ] **Step 5: Run it again without bless to verify it passes**

Run: `cargo test -p prim-fmt --test stability` Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/prim-fmt/tests/stability.rs crates/prim-fmt/tests/corpus/expected
git commit -m "test(prim-fmt): enforce style stability against a pinned corpus" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 7: SPEC policy + PR 2 gate

**Files:**

- Modify: `docs/SPEC.md` (new section between "NFR" and "Non-goals")

- [ ] **Step 1: Add the policy section**

```markdown
## Style stability

The canonical style is a compatibility contract. Any change to prim's output for
already-canonical input — including a change inherited from a formatter
dependency upgrade — ships as a **minor** version bump and is called out in the
CHANGELOG. The pinned-corpus test (`crates/prim-fmt/tests/corpus/`) enforces
this: when canonical output drifts, the test fails until the drift is reverted,
or deliberately re-blessed with
`PRIM_BLESS=1 cargo test -p prim-fmt --test stability` and released as above.
```

- [ ] **Step 2: Commit, verify, PR**

```bash
git add docs/SPEC.md
git commit -m "docs(prim-fmt): add the style-stability policy" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

Run `just fmt`, then `just verify` (rebase on `main` first if PR 1 merged).
Then:

```bash
git push -u origin feat/style-stability
gh pr create --title "test(prim-fmt): style-stability corpus and policy" --body "$(cat <<'EOF'
Adds a pinned-corpus stability test so a formatter-dependency upgrade that
changes canonical output fails prim's own CI, plus the SPEC policy: output
changes ship as a minor bump, called out in the CHANGELOG.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## PR 3 — CLI contract hardening + color (branch `fix/cli-contract` from `main`)

### Task 8: Explicit-path handling

**Files:**

- Modify: `crates/prim-cli/src/app.rs:61-65` (the `classify` else-branch)
- Test: `crates/prim-cli/tests/modes.rs`

**Interfaces:**

- Produces: stderr messages `"<path>: no such file"` (error) and
  `"<path>: not a file type prim formats; skipped"` (warning) — Task 12's docs
  cite this behavior.

- [ ] **Step 1: Create the branch**

```bash
git switch main && git pull && git switch -c fix/cli-contract
```

- [ ] **Step 2: Write the failing tests** (append to
      `crates/prim-cli/tests/modes.rs`)

```rust
#[test]
fn explicit_nonexistent_unowned_path_errors() {
    // A named path that does not exist is an error even when prim would not
    // own the file type (previously: silent exit 0).
    prim().arg("/no/such/prim/fixture.xyz").assert().code(2);
}

#[test]
fn explicit_unowned_existing_file_warns_and_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("main.rs");
    std::fs::write(&file, "fn main() {}\n").unwrap();

    prim()
        .arg(&file)
        .assert()
        .success()
        .stderr(predicates::str::contains("not a file type prim formats"));
}

#[test]
fn walked_unowned_files_stay_silent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    prim().arg(dir.path()).assert().success().stderr("");
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p prim-cli --test modes` Expected:
`explicit_nonexistent_unowned_path_errors` and
`explicit_unowned_existing_file_warns_and_exits_zero` FAIL (both currently exit
0 silently); `walked_unowned_files_stay_silent` passes.

- [ ] **Step 4: Implement** — in `crates/prim-cli/src/app.rs`, replace:

```rust
let Some(kind) = prim_fmt::classify(&file.path) else {
    // A file prim does not own — left byte-for-byte unchanged (FR-2.4),
    // even when named explicitly.
    continue;
};
```

with:

```rust
let Some(kind) = prim_fmt::classify(&file.path) else {
    // A file prim does not own is left byte-for-byte unchanged
    // (FR-2.4). Walked files are skipped silently; a named path is
    // answered — a missing one is an error, an unowned one a warning.
    if file.explicit {
        if file.path.exists() {
            ui::warning(&format!(
                "{}: not a file type prim formats; skipped",
                file.path.display()
            ));
        } else {
            ui::error(&format!("{}: no such file", file.path.display()));
            had_error = true;
        }
    }
    continue;
};
```

Also in `crates/prim-cli/tests/discovery.rs`, update the comment on
`explicit_non_owned_file_is_left_unchanged`: the file is now "skipped with a
warning", still exit 0 (the test's `.success()` assertion is unchanged).

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p prim-cli` Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
just fmt
git add crates/prim-cli/src/app.rs crates/prim-cli/tests/modes.rs crates/prim-cli/tests/discovery.rs
git commit -m "fix(prim-cli): report explicitly named paths prim cannot process" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 9: `--exclude` glob validation

**Files:**

- Modify: `crates/prim-cli/src/discover.rs` (`collect` becomes fallible)
- Modify: `crates/prim-cli/src/app.rs:60` (handle the `Result`)
- Test: `crates/prim-cli/tests/discovery.rs`, plus existing unit tests in
  `discover.rs`

**Interfaces:**

- Produces:
  `pub fn collect(paths: &[PathBuf], excludes: &[String]) ->
  Result<Vec<Discovered>, ignore::Error>`
  — `app::run_paths` maps `Err` to `ui::error` + exit 2.

- [ ] **Step 1: Write the failing behavioural test** (append to
      `crates/prim-cli/tests/discovery.rs`)

```rust
#[test]
fn malformed_exclude_glob_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    prim()
        .current_dir(dir.path())
        .args(["--exclude", "{unclosed"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--exclude"));
}
```

(`discovery.rs` already has the `fn prim()` helper and drives the binary.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p prim-cli --test discovery` Expected: FAIL — currently exits
0 (the bad glob is silently dropped).

- [ ] **Step 3: Implement** — in `crates/prim-cli/src/discover.rs`:

Three surgical edits; the body of `collect` between them is unchanged.

(a) Extend `collect`'s doc comment with one sentence, change the signature, and
validate globs as the first statement (a bad glob must fail even when only
explicit files are named):

```rust
/// Fails when an `--exclude` glob is malformed (FR-4.5): a typo'd filter must
/// be a usage error, not a silently ignored one.
pub fn collect(
    paths: &[PathBuf],
    excludes: &[String],
) -> Result<Vec<Discovered>, ignore::Error> {
    validate_excludes(excludes)?;
```

(b) Wrap the final expression in `Ok(...)`:

```rust
Ok(selected
    .into_iter()
    .map(|(path, explicit)| Discovered { path, explicit })
    .collect())
```

(c) Add the validator below `collect`:

```rust
/// Reject malformed exclude globs up front; `walk_into` re-builds the same
/// set per walk root, which cannot fail after this check.
fn validate_excludes(excludes: &[String]) -> Result<(), ignore::Error> {
    let mut builder = OverrideBuilder::new(".");
    for glob in excludes {
        builder.add(&format!("!{glob}"))?;
    }
    builder.build()?;
    Ok(())
}
```

In `crates/prim-cli/src/app.rs`, replace the
`for file in
discover::collect(...)` loop header with:

```rust
    let files = match discover::collect(&cli.paths, &cli.exclude) {
        Ok(files) => files,
        Err(err) => {
            ui::error(&format!("--exclude: {err}"));
            return EXIT_ERROR;
        }
    };

    for file in files {
```

Update the existing unit tests in `discover.rs` (and any behavioural callers) to
`collect(...).unwrap()`.

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p prim-cli` Expected: all PASS (including the updated unit
tests).

- [ ] **Step 5: Commit**

```bash
just fmt
git add crates/prim-cli/src/discover.rs crates/prim-cli/src/app.rs crates/prim-cli/tests/discovery.rs
git commit -m "fix(prim-cli): make a malformed --exclude glob a usage error" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 10: `--stdin-filepath` exclusivity

**Files:**

- Modify: `crates/prim-cli/src/cli.rs:40-41`
- Test: `crates/prim-cli/tests/modes.rs`

- [ ] **Step 1: Write the failing test** (append to `modes.rs`)

```rust
#[test]
fn stdin_filepath_conflicts_with_check_and_diff() {
    // clap reports argument conflicts as usage errors (exit 2).
    prim()
        .args(["--stdin-filepath", "a.md", "--check"])
        .write_stdin("x\n")
        .assert()
        .code(2);
    prim()
        .args(["--stdin-filepath", "a.md", "--diff"])
        .write_stdin("x\n")
        .assert()
        .code(2);
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p prim-cli --test modes stdin_filepath_conflicts` Expected:
FAIL — stdin mode currently silently ignores `--check`/`--diff`.

- [ ] **Step 3: Implement** — in `crates/prim-cli/src/cli.rs`:

```rust
/// Read from stdin and write the formatted result to stdout. The path
/// names the file so the right formatter is selected (format-on-save).
/// Mutually exclusive with --check and --diff.
#[arg(long, value_name = "PATH", conflicts_with_all = ["check", "diff"])]
pub stdin_filepath: Option<PathBuf>,
```

- [ ] **Step 4: Run to verify pass**, then commit:

```bash
just fmt
git add crates/prim-cli/src/cli.rs crates/prim-cli/tests/modes.rs
git commit -m "fix(prim-cli): reject --stdin-filepath combined with --check/--diff" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 11: `NO_COLOR` and stderr-keyed auto colour

**Files:**

- Modify: `crates/prim-cli/src/ui.rs` (new `resolve_color` + unit tests)
- Modify: `crates/prim-cli/src/main.rs:18-27`

**Interfaces:**

- Produces:
  `pub fn resolve_color(when: ColorWhen, stderr_is_tty: bool,
  no_color: bool) -> bool`
  in `ui`, consumed by `main`.

- [ ] **Step 1: Write the failing unit tests** (new `#[cfg(test)]` module at the
      bottom of `ui.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ColorWhen;

    #[test]
    fn always_and_never_ignore_the_environment() {
        assert!(resolve_color(ColorWhen::Always, false, true));
        assert!(!resolve_color(ColorWhen::Never, true, false));
    }

    #[test]
    fn auto_needs_a_tty_and_no_color_unset() {
        assert!(resolve_color(ColorWhen::Auto, true, false));
        assert!(!resolve_color(ColorWhen::Auto, false, false));
        assert!(!resolve_color(ColorWhen::Auto, true, true));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p prim-cli resolve_color` Expected: FAIL — `resolve_color` not
defined.

- [ ] **Step 3: Implement** — add to `ui.rs` (below `would_reformat`):

```rust
use crate::cli::ColorWhen;

/// Decide whether coloured output is enabled: an explicit `--color always` /
/// `--color never` wins; `auto` colours only when stderr (the human-output
/// stream) is a terminal and `NO_COLOR` is unset (clig.dev).
pub fn resolve_color(when: ColorWhen, stderr_is_tty: bool, no_color: bool) -> bool {
    match when {
        ColorWhen::Always => true,
        ColorWhen::Never => false,
        ColorWhen::Auto => stderr_is_tty && !no_color,
    }
}
```

In `main.rs`, replace the `match cli.color { ... }` block with:

```rust
// Colour policy (clig.dev): --color wins; auto honors NO_COLOR and keys
// off stderr, where all human-readable output goes.
let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
if ui::resolve_color(cli.color, std::io::stderr().is_terminal(), no_color) {
    yansi::enable();
} else {
    yansi::disable();
}
```

(`use cli::{Cli, ColorWhen};` in `main.rs` may now have an unused `ColorWhen`
import — remove it if the compiler warns.)

- [ ] **Step 4: Run to verify pass**

Run:
`cargo test -p prim-cli && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS, zero warnings.

- [ ] **Step 5: Commit**

```bash
just fmt
git add crates/prim-cli/src/ui.rs crates/prim-cli/src/main.rs
git commit -m "fix(prim-cli): honor NO_COLOR and key auto colour off stderr" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 12: PR 3 docs, snapshot, and gate

**Files:**

- Modify: `docs/SPEC.md` (FR-4.5, FR-5.4), `docs/USAGE.md`
- Create: `spec/tests/cmd/general/nonexistent_path.toml`

- [ ] **Step 1: SPEC updates**

FR-4.5 old:

```markdown
- **FR-4.5** prim shall accept CLI exclude globs.
```

new:

```markdown
- **FR-4.5** prim shall accept CLI exclude globs; a malformed glob is a usage
  error (exit `2`).
```

FR-5.4 old:

```markdown
- **FR-5.4** With `--stdin-filepath <path>`, prim shall read stdin and write the
  formatted result to stdout.
```

new:

```markdown
- **FR-5.4** With `--stdin-filepath <path>`, prim shall read stdin and write the
  formatted result to stdout. The flag is mutually exclusive with `--check` and
  `--diff`.
```

- [ ] **Step 2: USAGE updates**

In the Options table, replace the `--color` row description with:
`When to use coloured output (default`auto`;`auto`honors`NO_COLOR`).`

Append to the "Operating modes" list:

```markdown
- Naming a path explicitly is strict: a missing file is an error (exit `2`); an
  existing file prim does not own is skipped with a warning.
```

- [ ] **Step 3: trycmd snapshot** — create
      `spec/tests/cmd/general/nonexistent_path.toml`:

```toml
bin.name = "prim"
args = ["/no/such/prim/path.xyz"]
status.code = 2
```

Run: `cargo test -p prim-spec` Expected: PASS.

- [ ] **Step 4: Commit, verify, PR**

```bash
just fmt
git add docs/SPEC.md docs/USAGE.md spec/tests/cmd/general/nonexistent_path.toml
git commit -m "docs(prim-cli): record CLI hardening in spec and usage" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

Run `just verify` (rebase on `main` first). Then:

```bash
git push -u origin fix/cli-contract
gh pr create --title "fix(prim-cli): no silent failures — path errors, exclude validation, NO_COLOR" --body "$(cat <<'EOF'
Hardens the CLI contract from the post-v1 review: explicitly named missing
paths error (exit 2), unowned explicit paths warn, malformed --exclude globs
are usage errors, --stdin-filepath is exclusive with --check/--diff, and
colour honors NO_COLOR keyed off stderr.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## PR 4 — allowlist curation (branch `feat/allowlist-curation` from `main`, after PR 1 merges)

### Task 13: Curate the orphan allowlist

**Files:**

- Modify: `crates/prim-fmt/src/classify.rs` (the `EXACT` list, the `.env.`
  prefix rule, the `is_orphan` doc, and the tests)

**Interfaces:**

- Produces: `classify(".env") == None`,
  `classify("CODEOWNERS") ==
  Some(FileKind::Orphan)`,
  `classify(".mailmap") == Some(FileKind::Orphan)` — Task 14 documents exactly
  this set.

- [ ] **Step 1: Update the tests to the new contract**

In `orphan_allowlist_dotfiles`, remove `".env"` from the list and add
`".mailmap"`. In `orphan_allowlist_patterns_and_names`, remove the
`k(".env.local")` assertion and add:

```rust
assert_eq!(k("CODEOWNERS"), Some(FileKind::Orphan));
```

In `non_owned_returns_none`, add:

```rust
assert_eq!(k(".env"), None); // data values, not metadata — excluded.
assert_eq!(k(".env.local"), None);
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p prim-fmt classify` Expected: FAIL — `.env` still classifies
as `Orphan`; `CODEOWNERS` and `.mailmap` classify as `None`.

- [ ] **Step 3: Implement** — in `is_orphan`:

- Remove `".env",` from `EXACT`; add `".mailmap",` and `"CODEOWNERS",`.
- Delete the line `|| name.starts_with(".env.") // .env.*`.
- Replace the doc comment
  `/// Whether`name`is on the curated orphan allowlist (FR-2 table in the spec).`
  with
  `/// Whether`name`is on the curated orphan allowlist (documented in docs/USAGE.md).`

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p prim-fmt` Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
just fmt
git add crates/prim-fmt/src/classify.rs
git commit -m "feat(prim-fmt): curate orphan allowlist — drop .env, add CODEOWNERS and .mailmap" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 14: Document the allowlist + PR 4 gate

**Files:**

- Modify: `docs/USAGE.md` (new section after "Operating modes")

- [ ] **Step 1: Add the section**

```markdown
## What prim formats

Parsed formats (structured canonical formatting plus whitespace hygiene), by
extension: `.md`, `.markdown`, `.json`, `.jsonc`, `.yaml`, `.yml`, `.toml`.

Orphan allowlist (whitespace hygiene only) — un-owned text files matched by
exact name or pattern:

| Kind          | Entries                                                                                                                                             |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Ignore files  | `.gitignore`, `.gitattributes`, `.dockerignore`, `.npmignore`, `.eslintignore`, `.prettierignore`, `.primignore`, `.helmignore`, `.containerignore` |
| Repo metadata | `CODEOWNERS`, `.mailmap`, `.editorconfig`, `AUTHORS`, `CONTRIBUTORS`, `NOTICE`, `COPYING`, `LICENSE*`                                               |
| Containers    | `Dockerfile`, `Dockerfile.*`, `Containerfile`                                                                                                       |
| Plain text    | `*.txt`, `*.text`                                                                                                                                   |

Everything else — source code, unknown types, binaries — is left byte-for-byte
unchanged. `.env` files are deliberately excluded: their values are data and may
be whitespace-sensitive.
```

- [ ] **Step 2: Commit, verify, PR**

```bash
just fmt
git add docs/USAGE.md
git commit -m "docs(prim-cli): document the orphan allowlist in usage" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

Run `just verify` (rebase on `main` first). Then:

```bash
git push -u origin feat/allowlist-curation
gh pr create --title "feat(prim-fmt): curate the orphan allowlist" --body "$(cat <<'EOF'
Drops .env/.env.* from the orphan allowlist (data values, whitespace can be
significant), adds CODEOWNERS and .mailmap, and documents the full allowlist
in USAGE.md as the canonical reference.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Local cleanup — gardening (no PR, after the PRs are open)

### Task 15: Archive landed working memory

`docs/wip/` and `docs/archive/` are both gitignored (private working-memory
mode; the archive entry lands with PR 1), so this is a local move only.

- [ ] **Step 1: Move the eleven landed files**

```bash
mkdir -p docs/archive/specs docs/archive/plans
mv docs/wip/specs/2026-06-*.md docs/archive/specs/
mv docs/wip/plans/2026-06-*.md docs/archive/plans/
```

- [ ] **Step 2: Verify**

Run: `ls docs/wip/specs docs/wip/plans && git status --short` Expected:
`docs/wip/` holds only the two in-flight 2026-07-02 plans and the 2026-07-04
debt-remediation spec/plan; `git status` shows no new entries.

---

## PR 5 — FR-1.6: markdown-tagged fences stay verbatim (branch `fix/markdown-fence-verbatim` from `main`, after PR 2 merges)

Background: `dprint-plugin-markdown` unconditionally recurses into fenced blocks
tagged `markdown`/`md` (`Context::format_text` in the crate's
`generation/gen_types.rs` matches the tag before consulting the code-block
callback), so prim's `Ok(None)` callback cannot protect them — an FR-1.6
violation. Foreign-language tags are proven verbatim, so the fix swaps the fence
language for a sentinel before formatting and restores it after.

### Task 16: Guard markdown-tagged fences

**Files:**

- Modify: `crates/prim-fmt/src/markdown.rs` (guard helpers, `format` wiring, and
  tests)

**Interfaces:**

- Produces: private `guard_markdown_fences(&str) -> String` and
  `unguard_markdown_fences(&str) -> String`; `format`'s observable change is
  that `markdown`/`md`-tagged fence contents and tags survive byte-identical.

- [ ] **Step 1: Create the branch**

```bash
git switch main && git pull && git switch -c fix/markdown-fence-verbatim
```

- [ ] **Step 2: Write the failing tests** (append inside the existing
      `#[cfg(test)] mod tests` in `markdown.rs`)

````rust
    #[test]
    fn preserves_markdown_tagged_fence_verbatim() {
        let src = "```markdown\nThis single line is deliberately much longer than eighty columns so that the formatter would want to wrap it.\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert_eq!(out, src, "markdown fence content and tag must survive");
    }

    #[test]
    fn preserves_md_tagged_fence_and_restores_the_tag() {
        let src = "```md\n#    spaced heading stays exactly as written\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("```md\n"), "{out:?}");
        assert!(out.contains("#    spaced heading"), "{out:?}");
    }

    #[test]
    fn no_sentinel_leaks_into_output() {
        let src = "prose\n\n```markdown\ntext\n```\n\n```md\ntext\n```\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(!out.contains("prim-fence-guard"), "{out:?}");
    }

    #[test]
    fn other_fence_tags_are_untouched_by_the_guard() {
        let src = "```js\nconst x=1\n```\n";
        assert_eq!(guard_markdown_fences(src), src);
    }

    #[test]
    fn guard_handles_tilde_and_blockquote_fences() {
        assert_eq!(
            guard_markdown_fences("~~~markdown\n"),
            "~~~prim-fence-guard-markdown\n"
        );
        assert_eq!(
            guard_markdown_fences("> ```md\n"),
            "> ```prim-fence-guard-md\n"
        );
        // Round-trip is the invariant the fix relies on.
        assert_eq!(
            unguard_markdown_fences(&guard_markdown_fences("> ```md\n")),
            "> ```md\n"
        );
    }
````

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p prim-fmt --test '*' --lib markdown` (or simply
`cargo test -p prim-fmt markdown`) Expected: the first three tests FAIL — the
fence content comes back rewrapped; the two guard tests fail to compile until
the helpers exist (add them as stubs returning the input to see the first three
fail, if preferred).

- [ ] **Step 4: Implement** — in `crates/prim-fmt/src/markdown.rs`:

Add below `format`:

```rust
/// dprint-plugin-markdown unconditionally recurses into fenced blocks tagged
/// `markdown`/`md` (the tag is matched before the code-block callback runs),
/// which would violate FR-1.6. Guard: swap the fence language for a sentinel
/// tag dprint treats as foreign (and therefore preserves verbatim), then
/// restore it after formatting.
const GUARD_MARKDOWN: &str = "prim-fence-guard-markdown";
const GUARD_MD: &str = "prim-fence-guard-md";

fn guard_markdown_fences(source: &str) -> String {
    swap_fence_languages(source, &[("markdown", GUARD_MARKDOWN), ("md", GUARD_MD)])
}

fn unguard_markdown_fences(source: &str) -> String {
    swap_fence_languages(source, &[(GUARD_MARKDOWN, "markdown"), (GUARD_MD, "md")])
}

/// Rewrite the language word of every fenced-code opening line whose language
/// exactly matches a swap source. Lines are inspected structurally: optional
/// indentation and blockquote markers, a run of ≥ 3 backticks or tildes, then
/// the info string. Every rewrite is reversed by the opposite swap after
/// formatting, so a false positive inside verbatim content round-trips
/// unchanged.
fn swap_fence_languages(source: &str, swaps: &[(&str, &str)]) -> String {
    source
        .split_inclusive('\n')
        .map(|line| swap_fence_language_line(line, swaps))
        .collect()
}

fn swap_fence_language_line(line: &str, swaps: &[(&str, &str)]) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Optional indentation and blockquote markers ("  > > ").
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'>') {
        i += 1;
    }
    let fence_char = match bytes.get(i) {
        Some(b'`') => b'`',
        Some(b'~') => b'~',
        _ => return line.to_string(),
    };
    let fence_start = i;
    while i < bytes.len() && bytes[i] == fence_char {
        i += 1;
    }
    if i - fence_start < 3 {
        return line.to_string();
    }
    let lang_start = i;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    let lang = &line[lang_start..i];
    for (from, to) in swaps {
        if lang == *from {
            return format!("{}{}{}", &line[..lang_start], to, &line[i..]);
        }
    }
    line.to_string()
}
```

Rewire `format` to guard before dprint and unguard after (the `Ok(None)` branch
means "no changes to the guarded text", so the original `source` is already
correct there):

```rust
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let config = ConfigurationBuilder::new()
        .line_width(style.max_line_length.unwrap_or(80) as u32)
        .text_wrap(TextWrap::Always)
        .build();

    let guarded = guard_markdown_fences(source);
    let result = format_text(&guarded, &config, |_, _, _| Ok(None));
    match result {
        Ok(Some(formatted)) => Ok(hygiene(&unguard_markdown_fences(&formatted), style)),
        Ok(None) => Ok(hygiene(source, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}
```

- [ ] **Step 5: Run to verify pass**

Run:
`cargo test -p prim-fmt && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all PASS, zero warnings.

- [ ] **Step 6: Commit**

```bash
just fmt
git add crates/prim-fmt/src/markdown.rs
git commit -m "fix(prim-fmt): keep markdown-tagged fenced blocks verbatim (FR-1.6)" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

### Task 17: Pin the fix in the corpus + PR 5 gate

**Files:**

- Modify: `crates/prim-fmt/tests/corpus/input/sample.md`
- Modify: `crates/prim-fmt/tests/corpus/expected/sample.md` (via re-bless)

**Interfaces:**

- Consumes: Task 6's bless workflow
  (`PRIM_BLESS=1 cargo test -p prim-fmt --test stability`).

- [ ] **Step 1: Append a markdown-tagged fence to the corpus input**

Append to `crates/prim-fmt/tests/corpus/input/sample.md` (use a real
triple-backtick fence; shown indented here only to survive this plan's own
formatting):

````text
```markdown
#   A nested markdown example that must stay exactly as written, even though this line is far longer than eighty columns.
```
````

- [ ] **Step 2: Re-bless and inspect**

Run: `PRIM_BLESS=1 cargo test -p prim-fmt --test stability` Then open
`crates/prim-fmt/tests/corpus/expected/sample.md` and confirm the nested fence
content is byte-identical to the input and the `markdown` tag is restored. This
is a deliberate canonical-output change — exactly the minor-bump path the
Style-stability policy describes.

- [ ] **Step 3: Run the full suite**

Run: `cargo test -p prim-fmt` Expected: all PASS.

- [ ] **Step 4: Commit, verify, PR**

```bash
git add crates/prim-fmt/tests/corpus
git commit -m "test(prim-fmt): pin markdown-fence verbatim behavior in the corpus" \
  -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

Run `just verify` (rebase on `main` first — PR 2 must be merged). Then:

```bash
git push -u origin fix/markdown-fence-verbatim
gh pr create --title "fix(prim-fmt): keep markdown-tagged fenced blocks verbatim (FR-1.6)" --body "$(cat <<'EOF'
dprint-plugin-markdown recurses into fenced blocks tagged markdown/md before
consulting the code-block callback, so prim rewrapped their contents — an
FR-1.6 violation. Guard the fence language with a sentinel tag around the
dprint pass so those blocks are preserved verbatim, and pin the behavior in
the style corpus.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```
