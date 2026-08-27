//! Unit tests for ancestor-cascade detection.

use std::fs;

use super::*;

/// A parent that sets a key prim does not own, so the assertions are about
/// the whole cascade rather than about prim's own keys.
const PARENT: &str = "root = true\n[*.md]\nmax_line_length = 120\n";

#[test]
fn a_directory_with_nothing_above_it_inherits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(from_ancestors(dir.path()).is_none());
}

#[test]
fn a_directorys_own_editorconfig_is_not_something_it_inherits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    assert!(from_ancestors(dir.path()).is_none());
}

#[test]
fn a_parent_that_sets_a_key_is_reported_with_that_key() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();

    let inherited = from_ancestors(&sub).expect("the parent sets max_line_length");
    assert_eq!(inherited.files.len(), 1);
    assert!(inherited.keys.contains("max_line_length"));
}

#[test]
fn an_ancestor_that_sets_nothing_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    // Only `root`, which is a walk boundary rather than a setting: cutting
    // the walk off from this file loses nobody anything.
    fs::write(dir.path().join(".editorconfig"), "root = true\n").unwrap();

    assert!(from_ancestors(&sub).is_none());
}

#[test]
fn the_walk_stops_where_editorconfig_stops() {
    let dir = tempfile::tempdir().unwrap();
    let middle = dir.path().join("middle");
    let sub = middle.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(middle.join(".editorconfig"), "root = true\n").unwrap();

    // `middle` bounds the walk and sets nothing, so the grandparent's
    // `max_line_length` is already out of reach and must not be reported.
    assert!(from_ancestors(&sub).is_none());
}

#[test]
fn every_contributing_ancestor_is_named_in_the_order_editorconfig_applies_them() {
    let dir = tempfile::tempdir().unwrap();
    let middle = dir.path().join("middle");
    let sub = middle.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(middle.join(".editorconfig"), "[*.md]\nindent_size = 4\n").unwrap();

    let inherited = from_ancestors(&sub).expect("both ancestors set a key");
    assert_eq!(
        inherited.files,
        vec![
            dir.path().join(".editorconfig"),
            middle.join(".editorconfig")
        ],
        "farthest ancestor first, the order EditorConfig applies them in"
    );
    assert!(inherited.keys.contains("max_line_length"));
    assert!(inherited.keys.contains("indent_size"));
}

#[test]
fn a_key_written_before_any_section_is_left_out() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    // `ec4rs` applies nothing written above the first section header, so
    // prim never resolved `indent_size` here and must not claim it is lost.
    fs::write(
        dir.path().join(".editorconfig"),
        "indent_size = 2\n[*.md]\nmax_line_length = 120\n",
    )
    .unwrap();

    let inherited = from_ancestors(&sub).unwrap();
    assert!(inherited.keys.contains("max_line_length"));
    assert!(!inherited.keys.contains("indent_size"));
}

#[test]
fn the_warning_names_the_directory_the_file_and_the_keys() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();

    let inherited = from_ancestors(&sub).unwrap();
    let warning = severing_warning(&sub, &inherited);
    assert!(warning.contains("root = true"), "{warning}");
    assert!(warning.contains("max_line_length"), "{warning}");
    assert!(
        warning.contains(&dir.path().join(".editorconfig").display().to_string()),
        "{warning}"
    );
}
