// End-to-end whitespace-hygiene behaviour (FR-2): real formatting now changes
// files, so these exercise the --check exit-1 path and the in-place write that
// were dead under the scaffold no-op.

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn check_reports_file_needing_hygiene_with_exit_1() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap(); // trailing whitespace

    prim()
        .arg("--check")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.txt"));
}

#[test]
fn check_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    let original = "title  \n";
    std::fs::write(&file, original).unwrap();

    prim().arg("--check").arg(&file).assert().code(1);

    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn in_place_trims_trailing_whitespace_and_adds_final_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \nbody").unwrap(); // trailing ws + no final LF

    prim().arg(&file).assert().success();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title\nbody\n");
}
