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
    let file = dir.path().join("notes.md");
    fs::write(&file, "a\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\r\nb\r\n");
}
