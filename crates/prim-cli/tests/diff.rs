//! Behavioural tests: `prim --diff` prints a unified diff and writes nothing.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn diff_prints_unified_diff_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    let original = "{\"a\":1}\n"; // missing space after colon
    fs::write(&file, original).unwrap();

    prim()
        .arg("--diff")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::contains("--- a/"))
        .stdout(predicates::str::contains("+++ b/"));

    // --diff writes nothing.
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn diff_on_canonical_file_prints_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{ \"a\": 1 }\n").unwrap();

    prim()
        .arg("--diff")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}
