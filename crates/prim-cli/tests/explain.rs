//! Behavioural tests: `prim explain <PATH>` prints the `.editorconfig`
//! settings that apply to a file and where each came from (story C2).

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn explain_reports_configured_settings_with_their_editorconfig_source() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".editorconfig");
    fs::write(
        &config,
        "root = true\n[*]\nend_of_line = crlf\n[*.md]\nmax_line_length = 80\n",
    )
    .unwrap();
    let file = dir.path().join("doc.md");
    fs::write(&file, "# hi\n").unwrap();

    prim().arg("explain").arg(&file).assert().success().stdout(
        predicate::str::contains("end_of_line")
            .and(predicate::str::contains("crlf"))
            .and(predicate::str::contains(config.display().to_string()))
            .and(predicate::str::contains(":3"))
            .and(predicate::str::contains("[*]"))
            .and(predicate::str::contains("max_line_length"))
            .and(predicate::str::contains("80"))
            .and(predicate::str::contains(":5"))
            .and(predicate::str::contains("[*.md]")),
    );
}

#[test]
fn explain_reports_prims_default_when_no_editorconfig_sets_a_key() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");

    prim().arg("explain").arg(&file).assert().success().stdout(
        predicate::str::contains("end_of_line")
            .and(predicate::str::contains("lf"))
            .and(predicate::str::contains("prim's default")),
    );
}

#[test]
fn explain_omits_indent_and_max_line_length_for_orphan_files() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*]\nindent_style = space\nindent_size = 4\nmax_line_length = 80\n",
    )
    .unwrap();
    let file = dir.path().join("NOTES.txt");

    prim().arg("explain").arg(&file).assert().success().stdout(
        predicate::str::contains("end_of_line")
            .and(predicate::str::contains("indent_style").not())
            .and(predicate::str::contains("indent_size").not())
            .and(predicate::str::contains("max_line_length").not()),
    );
}

#[test]
fn explain_reports_prim_mdlint_strict_only_for_markdown() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();
    let markdown = dir.path().join("doc.md");
    let json = dir.path().join("doc.json");

    prim()
        .arg("explain")
        .arg(&markdown)
        .assert()
        .success()
        .stdout(
            predicate::str::contains("prim_mdlint_strict").and(predicate::str::contains("true")),
        );

    prim()
        .arg("explain")
        .arg(&json)
        .assert()
        .success()
        .stdout(predicate::str::contains("prim_mdlint_strict").not());
}

#[test]
fn explain_attributes_indent_size_to_tab_width_when_derived_from_it() {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".editorconfig");
    fs::write(&config, "root = true\n[*]\ntab_width = 4\n").unwrap();
    let file = dir.path().join("a.json");

    prim().arg("explain").arg(&file).assert().success().stdout(
        predicate::str::contains("indent_size")
            .and(predicate::str::contains(config.display().to_string())),
    );
}

#[test]
fn explain_warns_and_exits_zero_for_a_file_type_prim_does_not_format() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("script.sh");

    prim()
        .arg("explain")
        .arg(&file)
        .assert()
        .success()
        .stderr(predicate::str::contains("not a file type prim formats"));
}

#[test]
fn explain_does_not_require_the_target_file_to_exist() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("does-not-exist.md");

    prim()
        .arg("explain")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicate::str::contains("end_of_line"));
}
