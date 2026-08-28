//! Regression tests for what prim says about the file it leaves behind:
//! whether a warning fires exactly when a section prim did not (or could
//! not) rewrite fails to decide its own representative path.
//!
//! Each test pins the **outcome** — the `prim_mdlint_strict` a representative
//! path actually resolves to before and after the run, through the real
//! `.editorconfig` cascade — rather than the bytes prim happened to write.
//! Text assertions have missed every one of these in turn; resolution
//! assertions cannot.

use std::fs;

use crate::init::run;

use super::{editorconfig, fixture, strict_for, warnings_of, warnings_of_glob};

#[test]
fn route_5_a_narrower_section_that_defeats_the_map_is_reported() {
    // A complete, correctly ordered map with a narrower section appended
    // after it that turns the exemption back off. prim plans no write — every
    // canonical section already carries its key — so nothing it writes can be
    // checked; the file still does not do what the map means.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/wip/*.md]\nprim_mdlint_strict = true\n";
    let dir = fixture(content);
    assert!(
        strict_for(dir.path(), "docs/wip/plan.md"),
        "precondition: the appended section defeats the exemption"
    );

    let outcome = run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        content,
        "a person's own narrower section is theirs to keep"
    );
    assert!(
        !outcome.message.contains("already contains"),
        "prim must not report plain success over a map that does not hold; got: {}",
        outcome.message
    );
    // The whole message, not a substring either half could supply: the
    // defeated section and its line, the path and what it resolves to, and
    // the section that actually wins — which is a glob prim never writes, so
    // naming it takes EditorConfig's own matcher rather than prim's list.
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains(
            "[docs/wip/**.md] (line 6) sets prim_mdlint_strict = false, but [docs/wip/*.md] \
             (line 12) comes after it and wins, so prim_mdlint_strict = true applies to \
             docs/wip/plan.md instead"
        ),
        "got: {warnings}"
    );
}

#[test]
fn route_6_a_value_that_is_neither_true_nor_false_is_reported() {
    // `prim_mdlint_strict = maybe` counts as the key being present, so prim
    // leaves the section alone — and every value but `true` resolves to
    // `false`, so the strict tier is silently dead.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = maybe\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);
    assert!(
        !strict_for(dir.path(), "docs/guide.md"),
        "precondition: nothing is strict anywhere"
    );

    let outcome = run(dir.path()).unwrap();

    assert!(
        !outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains(
            "[docs/**.md] (line 4) sets prim_mdlint_strict = maybe, which is neither true nor \
             false"
        ),
        "the warning must quote the value and the line it is written on; got: {warnings}"
    );
    assert!(
        warnings.contains("so the floor tier applies to docs/guide.md"),
        "and name the path that silently lost its tier; got: {warnings}"
    );
}

#[test]
fn a_defeated_sections_line_numbers_are_lines_of_the_file_prim_leaves_behind() {
    // prim adds the missing `[*.md]` floor at the top in the same run, which
    // moves every section below it down two lines. A warning that quoted the
    // lines prim read would send the reader to the wrong ones.
    //
    // This is also the narrow-override guard: `[docs/*.md]` agrees with
    // `[docs/**.md]` for docs/guide.md, so `[docs/**.md]` must stay silent —
    // its witness in a subdirectory `[docs/*.md]` cannot reach is still
    // decided by `[docs/**.md]` itself. `warnings.len() == 1` below is
    // load-bearing for that too.
    let existing = "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/*.md]\nprim_mdlint_strict = true\n";
    let merged = crate::init::merge(existing, "docs/**.md");

    assert!(
        merged
            .actions
            .iter()
            .any(|action| action.contains("added [*.md]")),
        "{:?}",
        merged.actions
    );
    // [**/SUMMARY.md] is line 8 as prim read it and line 10 as prim leaves it;
    // [docs/*.md] is line 10 read and line 12 left.
    assert_eq!(merged.warnings.len(), 1, "{:?}", merged.warnings);
    assert!(
        merged.warnings[0].contains(
            "[**/SUMMARY.md] (line 10) sets prim_mdlint_strict = false, but [docs/*.md] (line 12) \
             comes after it and wins, so prim_mdlint_strict = true applies to docs/SUMMARY.md"
        ),
        "got: {}",
        merged.warnings[0]
    );
}

#[test]
fn route_7_a_write_that_takes_a_path_off_an_explicit_section_says_so() {
    // `src = "."` makes the whole repository the book, so the strict section
    // prim adds legitimately covers top-level files — including ones a person
    // put under an explicit `[*.md] prim_mdlint_strict = false`. prim does
    // what the book layout asks and reports what that costs; what it must not
    // do is move README.md to the strict tier without a word.
    let dir = fixture("root = true\n[*.md]\nprim_mdlint_strict = false\n");
    fs::write(dir.path().join("book.toml"), "[book]\nsrc = \".\"\n").unwrap();

    let outcome = run(dir.path()).unwrap();

    assert!(
        outcome.message.contains("added [**.md]"),
        "the section the book layout asks for is still written; got: {}",
        outcome.message
    );
    assert!(
        strict_for(dir.path(), "README.md"),
        "and it takes effect: {}",
        editorconfig(dir.path())
    );
    let warnings = warnings_of_glob(dir.path(), "**.md");
    assert!(
        warnings.contains(
            "[*.md] (line 2) sets prim_mdlint_strict = false, but [**.md] (line 4) comes after it \
             and wins, so prim_mdlint_strict = true applies to README.md instead"
        ),
        "got: {warnings}"
    );
}

#[test]
fn route_9_a_summary_under_each_working_memory_directory_is_checked_too() {
    // One representative per section is not enough for `[**/SUMMARY.md]`: a
    // summary inside an exempt directory is decided by a different set of
    // sections from one under the strict glob, and it was the docs/wip one the
    // retired section-order check used to catch. Every exempt directory needs
    // its own probe, or the section that overrides it goes unnoticed.
    for dir_name in ["docs/wip", "docs/archive"] {
        let content = format!(
            "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = \
             true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[{dir_name}/**.md]\nprim_mdlint_strict = true\n"
        );
        let summary = format!("{dir_name}/SUMMARY.md");
        let dir = fixture(&content);
        assert!(
            strict_for(dir.path(), &summary),
            "precondition: the summary under {dir_name} is strict as written"
        );

        let outcome = run(dir.path()).unwrap();

        assert!(
            !outcome.message.contains("already contains"),
            "got: {}",
            outcome.message
        );
        let warnings = warnings_of(dir.path());
        assert!(
            warnings.contains(&format!("applies to {summary}")),
            "got: {warnings}"
        );
    }
}

#[test]
fn route_10_a_non_boolean_value_reports_the_tier_the_file_actually_gives() {
    // The value prim cannot read is one problem; which tier the path ends up
    // in is a separate question, and a later section can answer it. Claiming
    // the floor tier without asking would be the same defect as any other
    // message that states an outcome it never checked.
    let content = "root = true\n[docs/**.md]\nprim_mdlint_strict = 1\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[*.md]\nprim_mdlint_strict = true\n";
    let dir = fixture(content);
    assert!(
        strict_for(dir.path(), "docs/guide.md"),
        "precondition: the trailing [*.md] = true wins for docs/guide.md"
    );

    run(dir.path()).unwrap();

    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains("so the strict tier applies to docs/guide.md"),
        "the tier reported must be the one the file gives; got: {warnings}"
    );
}

#[test]
fn route_11_a_write_a_later_broader_section_swallows_is_reported() {
    // The issue's own reproduction: `[*.md]` has no key yet, so prim writes
    // one, but `[**.md]` already sets the same value and comes after —
    // agreeing today, but [*.md] itself decides nothing. prim must not claim
    // a write it did not effectively make.
    let content = "root = true\n[*.md]\n[**.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);

    let outcome = run(dir.path()).unwrap();

    assert!(
        editorconfig(dir.path()).contains("[*.md]\nprim_mdlint_strict = false\n"),
        "the key is still written into [*.md]: {}",
        editorconfig(dir.path())
    );
    assert!(
        outcome
            .message
            .contains("set prim_mdlint_strict = false in [*.md]"),
        "the write is still named as an action; got: {}",
        outcome.message
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains("[*.md]") && warnings.contains("[**.md]"),
        "both sections must be named; got: {warnings}"
    );
    // The property that actually matters: [*.md] holds nothing, and [**.md]
    // is what decides top-level Markdown, exactly as the issue reports.
    assert!(
        !strict_for(dir.path(), "README.md"),
        "nothing resolves wrongly today: {}",
        editorconfig(dir.path())
    );
    assert!(
        strict_for(dir.path(), "docs/guide.md"),
        "the strict section prim adds still holds: {}",
        editorconfig(dir.path())
    );
}

#[test]
fn route_12_a_section_prim_creates_is_reported_when_a_later_catch_all_swallows_it() {
    // A shape the issue's own fixture never reaches: no pre-existing keyless
    // section at all. prim inserts a brand-new [*.md] in canonical order, and
    // a person's [*] — the most common section in any real .editorconfig —
    // comes after it and swallows it.
    let dir = fixture(
        "root = true\n[guide/**.md]\nprim_mdlint_strict = false\n[*]\nprim_mdlint_strict = false\n",
    );
    fs::write(dir.path().join("book.toml"), "[book]\nsrc = \"guide\"\n").unwrap();

    let outcome = run(dir.path()).unwrap();

    assert!(
        outcome
            .message
            .contains("added [*.md] with prim_mdlint_strict = false"),
        "the new section is still created; got: {}",
        outcome.message
    );
    let warnings = warnings_of_glob(dir.path(), "guide/**.md");
    assert!(
        warnings.contains("[*.md]") && warnings.contains("[*]"),
        "got: {warnings}"
    );
    assert!(
        !strict_for(dir.path(), "README.md"),
        "nothing resolves wrongly today: {}",
        editorconfig(dir.path())
    );
}

#[test]
fn route_11_is_idempotent_under_the_warning() {
    // The warning must not turn "nothing left to do" into "let me try again":
    // a second run changes nothing and repeats the same warning, not a
    // different one.
    let content = "root = true\n[*.md]\n[**.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);

    let first = run(dir.path()).unwrap();
    let first_editorconfig = editorconfig(dir.path());
    let first_warnings = warnings_of(dir.path());

    let second = run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        first_editorconfig,
        "a second run must be byte-identical"
    );
    assert!(
        second.message.contains("left unchanged"),
        "the summary must say the file was left unchanged, not that it already \
         contains the map; got: {}",
        second.message
    );
    assert_eq!(
        warnings_of(dir.path()),
        first_warnings,
        "the same warning is repeated, not a new one"
    );
    assert!(!first.message.is_empty() && !second.message.is_empty());
}

#[test]
fn route_13_a_working_memory_exemption_swallowed_by_a_broader_override_is_reported() {
    // The same shape over Superpowers working memory: a person's broader
    // `[docs/**]` (no `.md` suffix — it reaches every file, not only
    // Markdown) agrees with the docs/wip exemption's value and comes after
    // it, so the exemption decides nothing of its own.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);

    let outcome = run(dir.path()).unwrap();

    assert!(
        !outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains("[docs/wip/**.md]") && warnings.contains("[docs/**]"),
        "got: {warnings}"
    );
    assert!(
        !strict_for(dir.path(), "docs/wip/plan.md"),
        "nothing resolves wrongly today: {}",
        editorconfig(dir.path())
    );
}
