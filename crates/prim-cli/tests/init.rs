//! Behavioural tests for `prim init`: scaffold or merge `.editorconfig`
//! without disturbing unrelated content, and ensure the generated placement
//! map resolves through prim's existing Markdown strict-tier reader.

use std::fs;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn init_scaffolds_the_default_map_and_lint_resolves_it_end_to_end() {
    let dir = tempfile::tempdir().unwrap();

    prim()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success()
        .stderr(
            predicates::str::contains("created").and(predicates::str::contains(".editorconfig")),
        );

    assert_eq!(
        fs::read_to_string(dir.path().join(".editorconfig")).unwrap(),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
    );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "README.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .success()
        .stdout(
            predicates::str::contains("README.md:3:")
                .and(predicates::str::contains("[MD045]"))
                .and(predicates::str::contains("Image missing alt text")),
        );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/guide.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("docs/guide.md:3:")
                .and(predicates::str::contains("[MD045]"))
                .and(predicates::str::contains("Image missing alt text")),
        );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/SUMMARY.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .success()
        .stdout(
            predicates::str::contains("docs/SUMMARY.md:3:")
                .and(predicates::str::contains("[MD045]"))
                .and(predicates::str::contains("Image missing alt text")),
        );
}

#[test]
fn init_keeps_an_existing_strict_section_strict_when_it_backfills_the_floor() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    prim().arg("init").arg(dir.path()).assert().success();

    assert_eq!(
        fs::read_to_string(dir.path().join(".editorconfig")).unwrap(),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
    );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/guide.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD045]"));
}
