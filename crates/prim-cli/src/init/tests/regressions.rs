//! Regression tests for every known way `prim init` has reported success
//! while the `.editorconfig` it left behind resolved differently from what
//! prim intended.
//!
//! Each test pins the **outcome** — the `prim_mdlint_strict` a representative
//! path actually resolves to before and after the run, through the real
//! `.editorconfig` cascade — rather than the bytes prim happened to write.
//! Text assertions have missed every one of these in turn; resolution
//! assertions cannot.

use std::fs;
use std::path::Path;

use crate::init::run;
use crate::mdlint_policy;

/// The tier `relative` resolves to under `dir`'s `.editorconfig`.
fn strict_for(dir: &Path, relative: &str) -> bool {
    mdlint_policy::resolve(&dir.join(relative)).strict
}

fn fixture(content: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), content).unwrap();
    dir
}

fn editorconfig(dir: &Path) -> String {
    fs::read_to_string(dir.join(".editorconfig")).unwrap()
}

/// The warnings `merge` would report for `dir`'s `.editorconfig`, joined —
/// `run` prints them to stderr, which a unit test cannot read.
fn warnings_of(dir: &Path) -> String {
    warnings_of_glob(dir, "docs/**.md")
}

fn warnings_of_glob(dir: &Path, strict_glob: &str) -> String {
    crate::init::merge(&editorconfig(dir), strict_glob)
        .warnings
        .join("\n")
}

#[test]
fn route_1_a_missing_section_is_left_out_when_its_anchor_is_out_of_order() {
    // `[**/SUMMARY.md]` was written before the strict glob and `docs/wip` is
    // absent, so there is no position where the missing exemption both
    // follows the strict glob and precedes `[**/SUMMARY.md]`. prim must leave
    // it out rather than pick a losing position.
    let dir = fixture(
        "root = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n",
    );
    let before = [
        strict_for(dir.path(), "README.md"),
        strict_for(dir.path(), "docs/guide.md"),
        strict_for(dir.path(), "docs/wip/plan.md"),
        strict_for(dir.path(), "docs/SUMMARY.md"),
    ];

    run(dir.path()).unwrap();

    assert!(
        !editorconfig(dir.path()).contains("docs/wip"),
        "the exemption must not be inserted in a losing position"
    );
    assert!(
        editorconfig(dir.path()).contains("[*.md]\nprim_mdlint_strict = false"),
        "the write that was safe must still happen: {}",
        editorconfig(dir.path())
    );
    assert_eq!(
        [
            strict_for(dir.path(), "README.md"),
            strict_for(dir.path(), "docs/guide.md"),
            strict_for(dir.path(), "docs/wip/plan.md"),
            strict_for(dir.path(), "docs/SUMMARY.md"),
        ],
        before,
        "prim changed how an existing path resolves"
    );
}

#[test]
fn route_2_every_section_present_but_misordered_are_reported_not_rewritten() {
    // Every canonical section already carries an explicit value, so each
    // per-spec check finds nothing to write — but `[docs/**.md]`, written
    // after `[docs/wip/**.md]`, wins under last-match-wins and defeats the
    // exemption. prim must not report plain success over that.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);
    assert!(
        strict_for(dir.path(), "docs/wip/plan.md"),
        "precondition: the file as written already resolves docs/wip to strict"
    );

    let outcome = run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        content,
        "prim must not reorder or rewrite sections a person wrote"
    );
    assert!(
        !outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
    assert!(
        strict_for(dir.path(), "docs/wip/plan.md"),
        "prim did not claim to fix what it left alone"
    );
}

#[test]
fn route_3_a_keyless_section_in_a_losing_position_is_not_written_into() {
    // `[docs/wip/**.md]` exists but is keyless, before `[docs/**.md]` which
    // already has its key. Inserting `prim_mdlint_strict = false` into it
    // would report an update while the freshly-written value still loses to
    // `[docs/**.md]` under last-match-wins.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);

    let outcome = run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        content,
        "the file must be left exactly as written"
    );
    assert!(
        !outcome.message.contains("updated"),
        "got: {}",
        outcome.message
    );
    assert!(
        strict_for(dir.path(), "docs/wip/plan.md"),
        "prim must not claim an exemption it could not actually place"
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains(
            "docs/wip/plan.md would still resolve to prim_mdlint_strict = true, not \
                           false"
        ),
        "the warning must name the path and the value it would take; got: {warnings}"
    );
}

#[test]
fn route_4_a_write_never_changes_a_path_prim_did_not_intend_to_change() {
    // `[docs/**.md]` sets an unrelated key and sits after `[**/SUMMARY.md]`.
    // Writing `prim_mdlint_strict = true` into it would place SUMMARY.md
    // under the strict tier — a file that was correctly at the floor tier
    // before prim ran, and that prim was never asked to touch.
    let dir = fixture(
        "root = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/**.md]\nmax_line_length = 100\n",
    );
    assert!(
        !strict_for(dir.path(), "docs/SUMMARY.md"),
        "precondition: SUMMARY.md starts at the floor tier"
    );

    run(dir.path()).unwrap();

    assert!(
        !strict_for(dir.path(), "docs/SUMMARY.md"),
        "prim moved an unrelated file to the strict tier: {}",
        editorconfig(dir.path())
    );
    assert!(
        !editorconfig(dir.path()).contains("max_line_length = 100\nprim_mdlint_strict"),
        "prim wrote into the section whose value would lose anyway: {}",
        editorconfig(dir.path())
    );
    assert!(
        editorconfig(dir.path()).contains("[*.md]\nprim_mdlint_strict = false"),
        "the write that was safe must still happen: {}",
        editorconfig(dir.path())
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains("for docs/SUMMARY.md from false to true"),
        "the warning must name the path that would move and the value it would take; \
         got: {warnings}"
    );
    assert!(
        warnings.contains("[**/SUMMARY.md]"),
        "the warning must name the section that is meant to decide that path; got: {warnings}"
    );
}

#[test]
fn an_unrelated_section_that_sets_no_prim_key_does_not_block_a_second_run() {
    // A person appends an ordinary `[*.md] max_line_length = 80` after the
    // map prim wrote. That occurrence sets no prim key, so it takes no part
    // in prim's ordering: the second run must stay the documented
    // byte-identical no-op rather than refuse to work for ever.
    let dir = tempfile::tempdir().unwrap();
    run(dir.path()).unwrap();
    let mut content = editorconfig(dir.path());
    content.push_str("[*.md]\nmax_line_length = 80\n");
    fs::write(dir.path().join(".editorconfig"), &content).unwrap();

    let outcome = run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        content,
        "an unrelated section must not make prim rewrite the file"
    );
    assert!(
        outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
}

#[test]
fn a_keyless_occurrence_does_not_anchor_a_section_a_keyed_one_already_decides() {
    // The other half of the same rule: `[*.md]` carries the key near the top
    // and an ordinary `[*.md] max_line_length = 80` was appended at the end.
    // The keyed occurrence is the one that decides `[*.md]`'s place in the
    // map, so the missing exemption still has a position — between the strict
    // glob and `[**/SUMMARY.md]`. Anchoring on the trailing occurrence instead
    // would put the floor section after everything and leave no position at
    // all.
    let dir = fixture(
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[*.md]\nmax_line_length = 80\n",
    );

    run(dir.path()).unwrap();

    assert_eq!(
        editorconfig(dir.path()),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[*.md]\nmax_line_length = 80\n",
        "the exemption belongs between the strict glob and [**/SUMMARY.md]"
    );
    assert!(
        !strict_for(dir.path(), "docs/wip/plan.md"),
        "and it has to actually resolve to the floor tier"
    );
}

#[test]
fn a_keyless_section_between_two_keyed_ones_does_not_hide_their_conflict() {
    // `[**/SUMMARY.md]` and `[docs/**.md]` both carry the key and are in the
    // wrong relative order, but `[docs/wip/**.md]` — canonically between them
    // and present without the key — separates them. Comparing only
    // canonically adjacent pairs skips both pairs that touch the keyless
    // section and never compares the two that are actually broken, so the run
    // reported plain success over a file whose SUMMARY exemption is defeated.
    let dir = fixture(
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nmax_line_length = 100\n",
    );
    assert!(
        strict_for(dir.path(), "docs/SUMMARY.md"),
        "precondition: the file as written already defeats the SUMMARY exemption"
    );

    let outcome = run(dir.path()).unwrap();
    let warnings = warnings_of(dir.path());

    assert!(
        warnings.contains("[docs/**.md]") && warnings.contains("[**/SUMMARY.md]"),
        "the conflicting pair must be reported; got: {warnings}"
    );
    assert!(
        !outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
}

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
fn route_8_a_missing_canonical_section_is_added_when_an_existing_header_has_interior_whitespace() {
    // `[ docs/**.md ]` — with spaces — matches nothing, because EditorConfig
    // does not trim inside the brackets. prim's own scan used to trim, so it
    // read that header as its canonical `docs/**.md` and appended the key to
    // a section that decided nothing (issue #117).
    //
    // prim now reads the header the way `ec4rs` does, so the spaced section
    // is somebody else's, the canonical one is simply missing, and prim adds
    // it in canonical order. Mutation evidence: restoring
    // `Line::Section(glob.trim())` in the header scan (the pre-fix trimming
    // behaviour) makes this test fail, which is what pins it to the new
    // parsing rather than to the retired self-reference guard it used to be
    // named for.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[ docs/**.md ]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let dir = fixture(content);

    let outcome = run(dir.path()).unwrap();

    assert!(
        outcome.message.contains("added [docs/**.md]"),
        "the spaced header is not prim's section, so prim's is missing and gets \
         added; got: {}",
        outcome.message
    );
    assert!(
        strict_for(dir.path(), "docs/a.md"),
        "and it takes effect: {}",
        editorconfig(dir.path())
    );

    let warnings = warnings_of(dir.path());
    assert!(
        !warnings.contains("comes after it and wins"),
        "nothing was defeated, so nothing is reported as defeating it; got: {warnings}"
    );
}

#[test]
fn route_9_a_summary_under_docs_wip_is_checked_too() {
    // One representative per section is not enough for `[**/SUMMARY.md]`: a
    // summary under `docs/wip/` is decided by a different set of sections
    // from one under the strict glob, and it was the docs/wip one the retired
    // section-order check used to catch.
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = true\n";
    let dir = fixture(content);
    assert!(
        strict_for(dir.path(), "docs/wip/SUMMARY.md"),
        "precondition: the summary under docs/wip is strict as written"
    );

    let outcome = run(dir.path()).unwrap();

    assert!(
        !outcome.message.contains("already contains"),
        "got: {}",
        outcome.message
    );
    let warnings = warnings_of(dir.path());
    assert!(
        warnings.contains("applies to docs/wip/SUMMARY.md"),
        "got: {warnings}"
    );
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
