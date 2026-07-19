// Behavioural acceptance tests for the `fmt`/`lint`/`fix` verb model
// (AD-0007): the bare-`prim`-is-`fmt` alias, the deprecated top-level flag
// shim, and `lint`'s report-only contract.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn bare_invocation_is_a_permanent_fmt_alias() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim().arg(&file).assert().success().stderr("");

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title\n");
}

#[test]
fn explicit_fmt_behaves_like_the_bare_alias() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim().arg("fmt").arg(&file).assert().success().stderr("");

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title\n");
}

#[test]
fn top_level_check_is_deprecated_sugar_for_fmt_check() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim()
        .arg("--check")
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicates::str::contains(
            "'prim --check' is deprecated; use 'prim fmt --check'",
        ));
}

#[test]
fn explicit_fmt_check_does_not_warn() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim()
        .args(["fmt", "--check"])
        .arg(&file)
        .assert()
        .code(1)
        .stderr(predicates::str::is_empty());
}

#[test]
fn top_level_diff_and_stdin_filepath_also_warn_once() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim()
        .arg("--diff")
        .arg(&file)
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "'prim --diff' is deprecated; use 'prim fmt --diff'",
        ));

    prim()
        .args(["--stdin-filepath", "doc.md"])
        .write_stdin("# Title\n")
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "'prim --stdin-filepath' is deprecated; use 'prim fmt --stdin-filepath'",
        ));
}

#[test]
fn lint_reports_a_coded_diagnostic_without_writing() {
    // Since story B1, orphan files (the un-owned-text allowlist) get
    // itemized `code`/`file:line:col` findings instead of a coarse message.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains("doc.txt:1:6:")
            .and(predicates::str::contains("[hygiene::trailing-whitespace]")),
    );

    // lint never rewrites.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title  \n");
}

#[test]
fn lint_on_already_clean_file_exits_zero_and_reports_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

#[test]
fn lint_stdin_filepath_reports_without_writing_to_stdout() {
    prim()
        .args(["lint", "--stdin-filepath", "doc.txt"])
        .write_stdin("title  \n")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[hygiene::trailing-whitespace]"));
}

#[test]
fn lint_stdin_filepath_reports_markdown_content_findings() {
    prim()
        .args(["lint", "--stdin-filepath", "README.md"])
        .write_stdin("#Title\n\nSee https://example.com.\n")
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("README.md:3:")
                .and(predicates::str::contains("[MD034]"))
                .and(predicates::str::contains("prim fmt").not()),
        );
}

#[test]
fn lint_stdin_filepath_resolves_prim_mdlint_strict_from_editorconfig() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

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
}

#[test]
fn lint_does_not_accept_check_or_diff() {
    // `lint` is inherently report-only (AD-0007 §2); these flags belong to
    // `fmt`/`fix` only, so clap rejects them as a usage error.
    prim().args(["lint", "--check", "x.txt"]).assert().code(2);
    prim().args(["lint", "--diff", "x.txt"]).assert().code(2);
}

#[test]
fn fix_formats_in_place_like_fmt_today() {
    // `fix` has no autofixable content rules yet; until then it is
    // byte-for-byte `fmt`.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim().arg("fix").arg(&file).assert().success();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title\n");
}

#[test]
fn fix_check_and_diff_gate_on_pending_findings() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.txt");
    std::fs::write(&file, "title  \n").unwrap();

    prim()
        .args(["fix", "--check"])
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.txt"));

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title  \n");

    // Unlike `fmt --diff` (always exit 0), `fix --diff` shares `fix
    // --check`'s gated contract (AD-0007 §4): it must exit 1 when a fixable
    // finding is pending, even though it only prints a preview.
    prim()
        .args(["fix", "--diff"])
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.txt"));

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title  \n");

    // Once clean, both modes exit 0.
    std::fs::write(&file, "title\n").unwrap();
    prim().args(["fix", "--check"]).arg(&file).assert().code(0);
    prim().args(["fix", "--diff"]).arg(&file).assert().code(0);
}

#[test]
fn a_file_literally_named_fmt_is_shadowed_by_the_verb_and_needs_disambiguation() {
    let dir = tempfile::tempdir().unwrap();
    // No extension, so prim doesn't own this file type regardless — the
    // point here is purely about argv disambiguation (AD-0007 §1).
    std::fs::write(dir.path().join("fmt"), "irrelevant\n").unwrap();

    // `prim fmt` alone: "fmt" is consumed as the verb, not a path; the
    // directory walk finds the file but it's unowned, so it stays silent.
    prim()
        .current_dir(dir.path())
        .arg("fmt")
        .assert()
        .success()
        .stderr(predicates::str::is_empty());

    // `prim fmt fmt` disambiguates: the second "fmt" is now an explicit path,
    // and an explicit unowned path is a warning, not silence.
    prim()
        .current_dir(dir.path())
        .args(["fmt", "fmt"])
        .assert()
        .success()
        .stderr(predicates::str::contains("not a file type prim formats"));
}
