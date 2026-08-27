//! Behavioural tests for the warning `prim init` prints when writing
//! `root = true` would cut a cascade the target directory currently inherits
//! from (issue #118).

use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

/// A parent `.editorconfig` that sets a key prim does not own, so the test
/// asserts about the whole cascade rather than about prim's own keys.
const PARENT: &str = "root = true\n[*.md]\nmax_line_length = 120\n";

#[test]
fn merging_into_a_nested_directory_reports_the_parent_it_cuts_off() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(sub.join(".editorconfig"), "[*.txt]\nindent_size = 3\n").unwrap();

    prim().arg("init").arg(&sub).assert().success().stderr(
        predicates::str::contains("root = true").and(predicates::str::contains("max_line_length")),
    );

    let contents = fs::read_to_string(sub.join(".editorconfig")).unwrap();
    assert!(
        contents.starts_with("root = true\n"),
        "the write itself is unchanged: {contents}"
    );
}

#[test]
fn scaffolding_into_a_nested_directory_reports_the_parent_it_cuts_off() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();

    prim().arg("init").arg(&sub).assert().success().stderr(
        predicates::str::contains("root = true").and(predicates::str::contains("max_line_length")),
    );

    assert!(
        sub.join(".editorconfig").exists(),
        "the file is still created"
    );
}

#[test]
fn an_editorconfig_that_already_declares_root_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(
        sub.join(".editorconfig"),
        "root = true\n[*.txt]\nindent_size = 3\n",
    )
    .unwrap();

    prim()
        .arg("init")
        .arg(&sub)
        .assert()
        .success()
        .stderr(predicates::str::contains("no longer inherit").not());
}

#[test]
fn root_false_keeps_the_cascade_and_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    // `root = false` is still a `root` key, so prim prepends nothing — but it
    // does not stop EditorConfig's walk either, so the parent stays in reach.
    // Reporting a cut here would name a write prim never made.
    fs::write(
        sub.join(".editorconfig"),
        "root = false\n[*.txt]\nindent_size = 3\n",
    )
    .unwrap();

    prim()
        .arg("init")
        .arg(&sub)
        .assert()
        .success()
        .stderr(predicates::str::contains("no longer inherit").not());

    let contents = fs::read_to_string(sub.join(".editorconfig")).unwrap();
    assert!(
        contents.starts_with("root = false\n"),
        "prim left the root key alone: {contents}"
    );
}

#[test]
fn a_directory_with_no_parent_editorconfig_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();

    prim()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("no longer inherit").not());
}

#[test]
fn a_relative_path_does_not_report_the_directorys_own_editorconfig() {
    // `ec4rs` absolutizes a relative probe, so comparing its answers against
    // a relative directory once made prim name the file it was writing as an
    // ancestor it had cut itself off from. `prim init` with no PATH passes
    // `.`, so this is the ordinary invocation, not an edge case.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "[*.md]\nmax_line_length = 120\n",
    )
    .unwrap();

    prim()
        .current_dir(dir.path())
        .arg("init")
        .assert()
        .success()
        .stderr(predicates::str::contains("no longer inherit").not());
}

#[test]
fn a_relative_subdirectory_reports_only_the_real_parent() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(sub.join(".editorconfig"), "[*.txt]\nindent_size = 3\n").unwrap();

    let stderr = String::from_utf8(
        prim()
            .current_dir(dir.path())
            .args(["init", "sub"])
            .assert()
            .success()
            .get_output()
            .stderr
            .clone(),
    )
    .unwrap();

    assert!(stderr.contains("max_line_length"), "{stderr}");
    assert!(
        !stderr.contains("indent_size"),
        "the directory's own file is not an ancestor: {stderr}"
    );
}

#[test]
fn a_cascade_an_intervening_root_already_cut_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let middle = dir.path().join("middle");
    let sub = middle.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(middle.join(".editorconfig"), "root = true\n").unwrap();

    prim()
        .arg("init")
        .arg(&sub)
        .assert()
        .success()
        .stderr(predicates::str::contains("no longer inherit").not());
}
