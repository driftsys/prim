// Behavioural acceptance tests for recursive file discovery (FR-4), exercised
// against real temp directories. With the no-op formatter, discovery's
// observable effects are: directories/cwd get walked (no longer an error), and
// walked non-UTF-8 files are skipped rather than failing the run.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn directory_argument_is_walked() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/a.md"), "# Hi\n").unwrap();

    prim().arg(dir.path()).assert().success();
}

#[test]
fn no_args_walks_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# Hi\n").unwrap();

    prim().current_dir(dir.path()).assert().success();
}

#[test]
fn walked_binary_is_skipped_not_errored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("logo.bin"), [0xFFu8, 0xFE, 0x00, 0x01]).unwrap();
    std::fs::write(dir.path().join("ok.md"), "# Hi\n").unwrap();

    prim().arg(dir.path()).assert().success();
}

#[test]
fn explicit_non_owned_file_is_left_unchanged() {
    // A file prim does not own (here a binary) is skipped with a warning,
    // not an error, when named explicitly (FR-2.4).
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("logo.bin");
    std::fs::write(&bin, [0xFFu8, 0xFE, 0x00]).unwrap();

    prim().arg(&bin).assert().success();
}

#[test]
fn explicit_owned_file_that_is_not_utf8_errors() {
    // An owned file type (.json) that cannot be read as UTF-8 is reported as an
    // error when named explicitly (exit 2).
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("data.json");
    std::fs::write(&bad, [0xFFu8, 0xFE, 0x00]).unwrap();

    prim().arg(&bad).assert().code(2);
}

/// A repository shaped like the one `docs/recipes.md` describes: a generated
/// file and a byte-exact fixture directory, both protected by `.primignore`.
fn ignored_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".primignore"), "/CHANGELOG.md\nfixtures/\n").unwrap();
    // Non-canonical on purpose: prim would rewrite both if it processed them.
    std::fs::write(dir.path().join("CHANGELOG.md"), "#  Changelog\n").unwrap();
    std::fs::create_dir_all(dir.path().join("fixtures")).unwrap();
    std::fs::write(dir.path().join("fixtures/golden.json"), "{\"a\" :1}\n").unwrap();
    std::fs::write(dir.path().join("kept.json"), "{\"a\" :1}\n").unwrap();
    dir
}

#[test]
fn explicit_path_matching_primignore_is_not_formatted() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "#  Changelog\n",
        "an ignored file named explicitly must stay byte-for-byte unchanged"
    );
}

#[test]
fn explicit_path_inside_an_ignored_directory_is_not_formatted() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "fixtures/golden.json"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("fixtures/golden.json")).unwrap(),
        "{\"a\" :1}\n",
        "the ignore covers a directory, so files under it are covered too"
    );
}

#[test]
fn skipping_an_explicit_path_is_reported_on_stderr() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "CHANGELOG.md"])
        .assert()
        .success()
        .stderr(predicates::str::contains(".primignore"));
}

#[test]
fn skipping_a_walked_path_is_silent() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("kept.json"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[test]
fn lint_and_fix_honor_primignore_on_an_explicit_path() {
    for verb in ["lint", "fix"] {
        let dir = ignored_repo();

        prim()
            .current_dir(dir.path())
            .args([verb, "CHANGELOG.md"])
            .assert()
            .success()
            .stderr(predicates::str::contains(".primignore"));

        assert_eq!(
            std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
            "#  Changelog\n",
            "{verb} must leave an ignored file alone"
        );
    }
}

#[test]
fn no_primignore_processes_the_ignored_path_anyway() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["--no-primignore", "fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "# Changelog\n",
        "--no-primignore restores the explicit-path override"
    );
}

#[test]
fn explicit_paths_and_a_walk_agree_about_what_is_ignored() {
    // The contract in one sentence: naming a file cannot make prim do something
    // walking to it would not.
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "CHANGELOG.md", "fixtures/golden.json"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

#[test]
fn no_ignore_still_does_not_bypass_primignore_on_an_explicit_path() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["--no-ignore", "fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "#  Changelog\n",
        "--no-ignore covers VCS ignore files only"
    );
}

#[test]
fn malformed_exclude_glob_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    prim()
        .current_dir(dir.path())
        .args(["--exclude", "{unclosed"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--exclude"));
}

#[test]
fn no_ignore_includes_git_info_exclude_matches_in_fmt_check() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git/info")).unwrap();
    std::fs::write(dir.path().join(".git/info/exclude"), "hidden.json\n").unwrap();
    std::fs::write(dir.path().join("hidden.json"), "{\"a\":1}\n").unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    prim()
        .current_dir(dir.path())
        .args(["--no-ignore", "fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("hidden.json"));
}
