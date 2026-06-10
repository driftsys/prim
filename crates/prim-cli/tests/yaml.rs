//! Behavioural tests: prim formats YAML and fails safe on invalid input.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn reformats_messy_yaml_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.yaml");
    fs::write(&file, "a:    1\nb:  2\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("a: 1"), "{out:?}");
    assert!(out.contains("b: 2"), "{out:?}");
}

#[test]
fn check_flags_noncanonical_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.yaml");
    fs::write(&file, "a:    1\n").unwrap();

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn yml_extension_is_formatted() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.yml");
    fs::write(&file, "a:    1\n").unwrap();

    prim().arg(&file).assert().success();

    assert!(fs::read_to_string(&file).unwrap().contains("a: 1"));
}

#[test]
fn editorconfig_indent_size_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nindent_style=space\nindent_size=4\n",
    )
    .unwrap();
    let file = dir.path().join("a.yaml");
    fs::write(&file, "a:\n  b: 1\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\n    b:"), "4-space nested key: {out:?}");
}

#[test]
fn comments_anchors_and_block_scalars_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.yaml");
    fs::write(
        &file,
        "# note\nbase: &id 1\nref: *id\nblock: |\n  line one\n  line two\n",
    )
    .unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("# note"), "comment: {out:?}");
    assert!(out.contains("&id"), "anchor: {out:?}");
    assert!(out.contains("*id"), "alias: {out:?}");
    assert!(out.contains('|'), "block scalar: {out:?}");
    assert!(out.contains("line one"), "block content: {out:?}");
}

#[test]
fn invalid_yaml_explicit_path_errors_and_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.yaml");
    fs::write(&file, "a: [1, 2").unwrap();

    prim().arg(&file).assert().failure().code(2);

    assert_eq!(fs::read_to_string(&file).unwrap(), "a: [1, 2");
}

#[test]
fn invalid_yaml_discovered_warns_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.yaml"), "a: [1, 2").unwrap();

    prim().arg(dir.path()).assert().success();
    assert_eq!(
        fs::read_to_string(dir.path().join("bad.yaml")).unwrap(),
        "a: [1, 2"
    );
}

#[test]
fn stdin_invalid_yaml_echoes_original_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.yaml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a: [1, 2")
        .assert()
        .failure()
        .code(2)
        .stdout("a: [1, 2");
}

#[test]
fn stdin_roundtrips_valid_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.yaml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a:    1\n")
        .assert()
        .success();
}
