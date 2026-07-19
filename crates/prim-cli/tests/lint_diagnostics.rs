// Behavioural acceptance tests for story B1 (`prim lint` diagnostics
// mode): each whitespace-hygiene violation on the un-owned-text allowlist
// (the same set A1's BOM strip covers) reports a stable `code` and a
// 1-indexed `file:line:col`, never rewriting the file. Structured formats
// (JSON/YAML/TOML/Markdown) keep the coarser format-drift finding — their own
// content diagnostics are future stories (G2/D2).

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn flags_a_leading_bom_with_its_code_and_position() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(".gitignore");
    std::fs::write(&file, "\u{feff}target/\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains(":1:1:").and(predicates::str::contains("[hygiene::bom]")),
    );
    assert!(
        std::fs::read(&file)
            .unwrap()
            .starts_with(&[0xef, 0xbb, 0xbf])
    );
}

#[test]
fn flags_a_non_canonical_line_ending() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "a\r\nb\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains(":1:2:").and(predicates::str::contains("[hygiene::eol]")),
    );
}

#[test]
fn flags_trailing_whitespace_at_its_column() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains(":1:6:")
            .and(predicates::str::contains("[hygiene::trailing-whitespace]")),
    );
}

#[test]
fn flags_a_tab_indent_against_the_editorconfig_space_style() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nindent_style=space\n",
    )
    .unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "a\n\tb\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains(":2:1:").and(predicates::str::contains("[hygiene::indent]")),
    );
}

#[test]
fn flags_a_missing_final_newline() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "title").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains(":1:6:")
            .and(predicates::str::contains("[hygiene::final-newline]")),
    );
}

#[test]
fn reports_every_finding_in_a_file_with_several_violations() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "title  \nbody").unwrap(); // trailing ws + missing final LF

    let output = prim().arg("lint").arg(&file).assert().code(1);
    let stdout = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("[hygiene::trailing-whitespace]"));
    assert!(stdout.contains("[hygiene::final-newline]"));
    assert_eq!(stdout.lines().count(), 2, "one line per finding: {stdout}");
}

#[test]
fn structured_formats_keep_the_coarse_format_drift_finding() {
    // JSON/YAML/TOML/Markdown are out of B1's scope — they still get the
    // pre-existing single "format drift" finding, not itemized codes.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.json");
    std::fs::write(&file, "{\"a\":1}").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.json").and(predicates::str::contains("prim fmt")))
        .stdout(predicates::str::contains("hygiene::").not());
}

#[test]
fn clean_orphan_file_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    std::fs::write(&file, "title\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}
