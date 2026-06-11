//! Behavioural tests: prim formats Markdown with prose wrap and guardrails.

use std::fs;

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

fn max_line_width(s: &str) -> usize {
    s.lines().map(|l| l.chars().count()).max().unwrap_or(0)
}

#[test]
fn normalizes_heading_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "#    Title\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "# Title\n");
}

#[test]
fn check_flags_noncanonical_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, "#    Title\n").unwrap();

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn markdown_extension_is_formatted() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.markdown");
    fs::write(&file, "#    Title\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "# Title\n");
}

#[test]
fn editorconfig_max_line_length_drives_wrap() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*.md]\nmax_line_length=40\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    fs::write(&file, format!("{}\n", "word ".repeat(40))).unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(max_line_width(&out) <= 40, "wrapped to 40: {out:?}");
}

#[test]
fn fenced_code_and_link_preserved_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    let long = "word ".repeat(30);
    fs::write(
        &file,
        format!("{long}[link](https://example.com/a/very/long/path)\n\n```js\nconst x=1\n```\n"),
    )
    .unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(
        out.contains("https://example.com/a/very/long/path"),
        "URL intact: {out:?}"
    );
    assert!(out.contains("const x=1"), "fenced code verbatim: {out:?}");
}
