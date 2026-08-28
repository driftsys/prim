//! Regression tests for where prim puts a write: whether a section lands in
//! a position that lets its value actually decide the path it is for.
//!
//! Each test pins the **outcome** — the `prim_mdlint_strict` a representative
//! path actually resolves to before and after the run, through the real
//! `.editorconfig` cascade — rather than the bytes prim happened to write.
//! Text assertions have missed every one of these in turn; resolution
//! assertions cannot.

use std::fs;

use crate::init::run;

use super::{editorconfig, fixture, strict_for, warnings_of};

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
