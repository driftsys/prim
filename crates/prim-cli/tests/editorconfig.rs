//! Behavioural tests: prim honors `.editorconfig` (FR-3).

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn crlf_end_of_line_is_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*]\nend_of_line = crlf\n",
    )
    .unwrap();
    // A `.txt` orphan is hygiene-only, isolating the end_of_line setting from any
    // per-format structured pass.
    let file = dir.path().join("notes.txt");
    fs::write(&file, "a\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\r\nb\r\n");
}

#[test]
fn insert_final_newline_false_strips_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ninsert_final_newline=false\n",
    )
    .unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{}\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
}

#[test]
fn trim_disabled_keeps_trailing_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ntrim_trailing_whitespace=false\n",
    )
    .unwrap();
    // A `.txt` orphan stays hygiene-only (never structurally formatted), so it
    // isolates the trim_trailing_whitespace setting from any per-format pass.
    let file = dir.path().join("notes.txt");
    fs::write(&file, "a  \n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a  \n");
}

#[test]
fn check_mode_flags_crlf_when_config_demands_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nend_of_line=crlf\n",
    )
    .unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a = 1\n").unwrap(); // LF on disk, config wants CRLF

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn stdin_filepath_honors_sibling_editorconfig() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nend_of_line=crlf\n",
    )
    .unwrap();
    let target = dir.path().join("x.txt");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a\nb\n")
        .assert()
        .success()
        .stdout("a\r\nb\r\n");
}

#[test]
fn no_editorconfig_leaves_canonical_behaviour() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "a  \r\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\nb\n");
}
