//! Unit tests for ancestor-cascade detection.

use std::fs;

use super::*;

/// A parent that sets a key prim does not own, so the assertions are about
/// the whole cascade rather than about prim's own keys.
const PARENT: &str = "root = true\n[*.md]\nmax_line_length = 120\n";

/// A line that is neither a section header nor a `key = value` pair, which is
/// what `ec4rs` rejects an `.editorconfig` for.
const MALFORMED: &str = "[*.md]\nthis line has no equals sign\n";

/// The inheritance `dir` has, for the tests that expect a readable cascade.
/// `None` means nothing above sets anything.
fn inheritance(dir: &Path) -> Option<Inheritance> {
    match from_ancestors(dir) {
        Ancestry::Nothing => None,
        Ancestry::Inherits(inherited) => Some(inherited),
        Ancestry::Malformed { path, .. } => {
            panic!("unexpected malformed ancestor: {}", path.display())
        }
        Ancestry::Unopenable { path } => {
            panic!("unexpected unopenable ancestor: {}", path.display())
        }
    }
}

#[test]
fn a_directory_with_nothing_above_it_inherits_nothing() {
    let dir = tempfile::tempdir().unwrap();
    assert!(inheritance(dir.path()).is_none());
}

#[test]
fn a_directorys_own_editorconfig_is_not_something_it_inherits() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    assert!(inheritance(dir.path()).is_none());
}

#[test]
fn a_parent_that_sets_a_key_is_reported_with_that_key() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();

    let inherited = inheritance(&sub).expect("the parent sets max_line_length");
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

    assert!(inheritance(&sub).is_none());
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
    assert!(inheritance(&sub).is_none());
}

#[test]
fn every_contributing_ancestor_is_named_in_the_order_editorconfig_applies_them() {
    let dir = tempfile::tempdir().unwrap();
    let middle = dir.path().join("middle");
    let sub = middle.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();
    fs::write(middle.join(".editorconfig"), "[*.md]\nindent_size = 4\n").unwrap();

    let inherited = inheritance(&sub).expect("both ancestors set a key");
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

    let inherited = inheritance(&sub).unwrap();
    assert!(inherited.keys.contains("max_line_length"));
    assert!(!inherited.keys.contains("indent_size"));
}

#[test]
fn the_warning_names_the_directory_the_file_and_the_keys() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), PARENT).unwrap();

    let warning = severing_warning(&sub, &from_ancestors(&sub)).expect("the parent sets a key");
    assert!(warning.contains("root = true"), "{warning}");
    assert!(warning.contains("max_line_length"), "{warning}");
    assert!(
        warning.contains(&dir.path().join(".editorconfig").display().to_string()),
        "{warning}"
    );
}

#[test]
fn a_malformed_ancestor_is_reported_rather_than_passed_over_in_silence() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), MALFORMED).unwrap();

    let ancestry = from_ancestors(&sub);
    let Ancestry::Malformed { ref path, .. } = ancestry else {
        panic!("a parent prim cannot parse must not read as an ordinary answer");
    };
    assert_eq!(path, &dir.path().join(".editorconfig"));

    let warning = severing_warning(&sub, &ancestry).expect("a malformed ancestor is reported");
    assert!(
        warning.contains("ignoring malformed .editorconfig"),
        "the same opening sentence the resolution path uses: {warning}"
    );
    assert!(
        warning.contains(&dir.path().join(".editorconfig").display().to_string()),
        "the warning names the file prim could not read: {warning}"
    );
    assert!(
        warning.contains("root = true"),
        "the warning says what prim wrote: {warning}"
    );
}

#[test]
fn a_malformed_ancestor_beyond_the_walk_boundary_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let middle = dir.path().join("middle");
    let sub = middle.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(dir.path().join(".editorconfig"), MALFORMED).unwrap();
    fs::write(middle.join(".editorconfig"), "root = true\n").unwrap();

    // `middle` bounds the walk, so the grandparent is already out of reach:
    // `root = true` here cuts off nothing that was still connected.
    assert!(inheritance(&sub).is_none());
}
