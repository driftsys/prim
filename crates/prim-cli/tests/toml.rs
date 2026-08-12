//! Behavioural tests: prim formats TOML and fails safe on invalid input.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn reformats_messy_toml_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a=1\nb   =   2\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("a = 1"), "{out:?}");
    assert!(out.contains("b = 2"), "{out:?}");
}

#[test]
fn check_flags_noncanonical_toml() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a=1\n").unwrap();

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn editorconfig_indent_size_is_honored() {
    let dir = tempfile::tempdir().unwrap();
    // A tiny max_line_length forces taplo to expand the array so the per-element
    // indentation (from indent_size) is observable.
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nindent_style=space\nindent_size=4\nmax_line_length=1\n",
    )
    .unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "arr = [1, 2]\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("\n    1,"), "4-space array element: {out:?}");
}

#[test]
fn cargo_manifest_feature_array_is_kept_within_max_line_length() {
    // Issue #96: a dependency's feature list must not be collapsed past the
    // configured width just because it sits inside an inline table.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.toml]\nindent_size = 2\nmax_line_length = 80\n",
    )
    .unwrap();
    let file = dir.path().join("Cargo.toml");
    fs::write(
        &file,
        "[workspace.dependencies]\nweb-sys = { version = \"0.3.103\", features = [\n  \"Document\",\n  \"DomRect\",\n  \"Element\",\n  \"Headers\",\n  \"HtmlCanvasElement\",\n  \"Request\",\n  \"RequestInit\",\n  \"Response\",\n  \"Window\",\n] }\n",
    )
    .unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    let widest = out.lines().map(|line| line.chars().count()).max().unwrap();
    assert!(
        widest <= 80,
        "no line exceeds 80 columns, got {widest}: {out:?}"
    );
    assert!(
        out.contains("features = [\n  \"Document\",\n"),
        "feature list stays expanded: {out:?}"
    );
}

#[test]
fn pyproject_dependency_layout_survives_formatting() {
    // Issue #105: `uv add` writes this array one entry per line and re-expands
    // it on every edit. Collapsing it means prim and uv overwrite each other.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("pyproject.toml");
    let written = "[project]\nname = \"probe\"\nversion = \"0.1.0\"\ndependencies = [\n  \"idna>=3.18\",\n  \"packaging\",\n]\n";
    fs::write(&file, written).unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        written,
        "the dependency list must keep its one-per-line layout"
    );
    // And the result is a fixed point, so a hook never reports pending changes.
    prim().arg("--check").arg(&file).assert().success();
}

#[test]
fn inline_table_and_comments_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "# note\nx = {a=1}\n").unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(out.contains("# note"), "{out:?}");
    assert!(out.contains("{ a = 1 }"), "{out:?}");
}

#[test]
fn invalid_toml_explicit_path_errors_and_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bad.toml");
    fs::write(&file, "a = = 1").unwrap();

    prim().arg(&file).assert().failure().code(2);

    assert_eq!(fs::read_to_string(&file).unwrap(), "a = = 1");
}

#[test]
fn invalid_toml_discovered_warns_and_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.toml"), "a = = 1").unwrap();

    prim().arg(dir.path()).assert().success();
    assert_eq!(
        fs::read_to_string(dir.path().join("bad.toml")).unwrap(),
        "a = = 1"
    );
}

#[test]
fn stdin_invalid_toml_echoes_original_and_exits_two() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.toml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a = = 1")
        .assert()
        .failure()
        .code(2)
        .stdout("a = = 1");
}

#[test]
fn stdin_roundtrips_valid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("x.toml");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a=1\n")
        .assert()
        .success();
}
