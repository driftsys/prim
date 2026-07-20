use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn bare_check_idempotence_exits_zero_for_a_clean_structured_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.json");
    fs::write(&file, "{ \"a\": 1 }\n").unwrap();

    prim()
        .arg("--check-idempotence")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn check_idempotence_covers_hygiene_only_files_without_writing_them() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    let original = "title  \n";
    fs::write(&file, original).unwrap();

    prim()
        .args(["fmt", "--check-idempotence"])
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());

    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn check_idempotence_ignores_hidden_environment_overrides() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.json");
    let original = "{ \"a\": 1 }\n";
    fs::write(&file, original).unwrap();

    prim()
        .args(["fmt", "--check-idempotence"])
        .env("PRIM_TEST_FORCE_IDEMPOTENCE_FAILURE", &file)
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());

    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}
