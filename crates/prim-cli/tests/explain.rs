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
fn explain_names_a_section_with_a_trailing_comment_by_its_glob_not_the_raw_line() {
    // Reproduction (issue #117, shape 2): prim's own scanner required a
    // trimmed line to end in ']', so a header with a trailing comment was not
    // recognized at all, and `explain` could not name the section that set
    // the value. It must now recognize the header and show the glob alone,
    // without the comment that played no part in matching.
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join(".editorconfig");
    fs::write(
        &config,
        "root = true\n[*.md] # markdown files\nmax_line_length = 100\n",
    )
    .unwrap();
    let file = dir.path().join("doc.md");

    let assert = prim().arg("explain").arg(&file).assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let max_line_length_line = stdout
        .lines()
        .find(|line| line.contains("max_line_length"))
        .expect("max_line_length line present in output");
    assert!(
        max_line_length_line.contains("= 100"),
        "got: {max_line_length_line}"
    );
    assert!(
        max_line_length_line.ends_with(&format!("{}:3 [*.md])", config.display())),
        "got: {max_line_length_line}"
    );
    assert!(
        !max_line_length_line.contains("markdown files"),
        "the comment plays no part in matching and must not be shown: {max_line_length_line}"
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

#[test]
fn explain_shows_the_disabled_rules_and_their_origin() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD033, MD041\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n").unwrap();

    prim()
        .current_dir(dir.path())
        .args(["explain", "a.md"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("prim_mdlint_disable")
                .and(predicates::str::contains("MD033, MD041"))
                .and(predicates::str::contains(".editorconfig:3")),
        );
}

#[test]
fn explain_warns_about_an_unrecognised_disabled_id_instead_of_claiming_unset() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n").unwrap();

    let assert = prim()
        .current_dir(dir.path())
        .args(["explain", "a.md"])
        .assert()
        .success()
        .stderr(predicates::str::contains("MD999"))
        .stdout(
            predicates::str::contains("prim_mdlint_disable")
                .and(predicates::str::contains(".editorconfig:3")),
        );

    // `max_line_length` legitimately prints "unset" elsewhere in the same
    // output, so scope the "no unset" check to the prim_mdlint_disable line
    // itself rather than the whole stdout.
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let disable_line = stdout
        .lines()
        .find(|line| line.contains("prim_mdlint_disable"))
        .expect("prim_mdlint_disable line present in output");
    assert!(
        !disable_line.contains("unset"),
        "prim_mdlint_disable has a real origin here, so its line must not claim unset: {disable_line}"
    );
}

/// The rendered `prim explain` line for `key`, for assertions that must not
/// be satisfied by some other setting's value.
fn line_for(output: &[u8], key: &str) -> String {
    String::from_utf8_lossy(output)
        .lines()
        .find(|line| line.contains(key))
        .unwrap_or_else(|| panic!("no {key} line in output"))
        .to_string()
}

#[test]
fn explain_reports_an_unset_disable_key_as_prims_default() {
    // Nothing in the cascade sets `prim_mdlint_disable`, so there is no
    // `.editorconfig` line to blame and nothing is excluded: `unset` is the
    // whole truth.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    let assert = prim()
        .current_dir(dir.path())
        .args(["explain", "a.md"])
        .assert()
        .success();

    let disable = line_for(&assert.get_output().stdout, "prim_mdlint_disable");
    assert!(disable.contains("unset"), "got: {disable}");
    assert!(disable.contains("prim's default"), "got: {disable}");
}

#[test]
fn explain_reports_a_deliberately_cleared_disable_key_as_none() {
    // `= none` and `= unset` both clear the list on purpose. The key has a
    // real `.editorconfig` origin, so the value beside it must say what prim
    // applies — no rules — rather than being blank or claiming the key was
    // never set.
    for value in ["none", "unset"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            format!("root = true\n[*.md]\nprim_mdlint_disable = {value}\n"),
        )
        .unwrap();

        let assert = prim()
            .current_dir(dir.path())
            .args(["explain", "a.md"])
            .assert()
            .success()
            .stderr(predicate::str::contains("not a rule prim runs").not());

        let disable = line_for(&assert.get_output().stdout, "prim_mdlint_disable");
        assert!(
            disable.contains("= none"),
            "value {value:?} resolves to no exclusions; got: {disable}"
        );
        assert!(
            disable.contains(".editorconfig:3"),
            "value {value:?} was set on a real line; got: {disable}"
        );
    }
}
