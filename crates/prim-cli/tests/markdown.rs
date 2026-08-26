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
fn keeps_prose_off_an_html_comment_closing_line() {
    // Issue #97: joining the prose onto the `-->` line makes CommonMark parse
    // the rest of that line as raw HTML, so the code span stops rendering.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nindent_size = 2\ntrim_trailing_whitespace = false\n",
    )
    .unwrap();
    let file = dir.path().join("r.md");
    fs::write(
        &file,
        "# T\n\n- A list item whose prose runs on for a while before the note.\n  <!-- A correction note that spans\n  more than one line and ends here. -->\n  As-built `rest.rs` omits the two count fields.\n",
    )
    .unwrap();

    prim().arg(&file).assert().success();

    let out = fs::read_to_string(&file).unwrap();
    assert!(
        out.contains("more than one line and ends here. -->\n  As-built `rest.rs`"),
        "prose must start its own line after the comment: {out:?}"
    );

    // Formatting again must not move it back.
    prim().arg("--check").arg(&file).assert().success();
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

#[test]
fn whitespace_inside_a_word_does_not_panic() {
    // Issue #115: dprint-plugin-markdown asserts, in debug builds only, that
    // a word holds no whitespace other than U+00A0 — see AD-0006 for the full
    // condition and the dev-profile override that silences it. This pins the
    // file path end to end; the whole character set is covered by
    // prim-fmt's markdown::tests::whitespace_inside_a_word_does_not_panic.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.md");
    let content = "a \u{2009}b\n";
    fs::write(&file, content).unwrap();

    prim().arg("fmt").arg(&file).assert().success();

    assert_eq!(fs::read(&file).unwrap(), content.as_bytes());
}
