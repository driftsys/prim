//! Behavioural tests: prim formats JSON/JSONC and fails safe on invalid input.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn reformats_messy_json_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\n\"a\":1,\n\"b\":   2\n}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\"a\": 1"), "{out:?}");
    assert!(out.contains("\"b\": 2"), "{out:?}");
    assert!(out.ends_with('\n'));
}

#[test]
fn check_flags_noncanonical_json() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\"a\":1}\n").unwrap(); // missing space after colon

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn editorconfig_indent_size_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nindent_style=space\nindent_size=4\n",
    )
    .unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\n\"a\": {\n\"b\": 1\n}\n}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\n    \"a\":"), "4-space top key: {out:?}");
    assert!(
        out.contains("\n        \"b\":"),
        "8-space nested key: {out:?}"
    );
}

#[test]
fn jsonc_comments_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.jsonc");
    fs::write(&file, "{\n// note\n\"a\": 1\n}\n").unwrap();

    prim().arg(&file).assert().success();

    assert!(fs::read_to_string(&file).unwrap().contains("// note"));
}

#[test]
fn invalid_json_explicit_path_errors_and_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.json");
    fs::write(&file, "{ not valid").unwrap();

    prim().arg(&file).assert().failure().code(2);

    assert_eq!(fs::read_to_string(&file).unwrap(), "{ not valid");
}

#[test]
fn invalid_json_discovered_warns_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();

    // Discovered (directory walk), not explicitly named: warn, exit 0, untouched.
    prim().arg(dir.path()).assert().success();
    assert_eq!(
        fs::read_to_string(dir.path().join("bad.json")).unwrap(),
        "{ not valid"
    );
}

#[test]
fn stdin_invalid_json_echoes_original_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.json");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("{ not valid")
        .assert()
        .failure()
        .code(2)
        .stdout("{ not valid");
}

#[test]
fn stdin_roundtrips_valid_json() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.json");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("{\"a\":1}")
        .assert()
        .success();
}
