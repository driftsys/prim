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
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
    );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "README.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .code(1)
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
        .code(1)
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
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
    );

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/guide.md"])
        .write_stdin("# Title\n\n![](hero.png)\n")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD045]"));
}

#[test]
fn init_scaffold_resolves_docs_wip_to_the_floor_tier_despite_the_strict_glob() {
    // MD041 (first-line-heading) is a convention rule that only runs in the
    // strict tier. A file under docs/wip/ must stay at the floor tier even
    // though the strict glob covers docs/**, so the same non-heading-first
    // content must trigger MD041 under docs/ but not under docs/wip/. Text
    // assertions on the scaffold alone would not catch the section being
    // written in the wrong order — this pins the actual resolution.
    let dir = tempfile::tempdir().unwrap();

    prim().arg("init").arg(dir.path()).assert().success();

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/guide.md"])
        .write_stdin("Intro\n\n# Title\n")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD041]"));

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/wip/plan.md"])
        .write_stdin("Intro\n\n# Title\n")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD041]").not());
}

#[test]
fn init_adds_a_working_floor_section_when_an_existing_header_has_interior_whitespace() {
    // Reproduction (issue #117, shape 1): `[ *.md ]` is not prim's canonical
    // `[*.md]` — `ec4rs` does not trim inside the brackets, so the real glob
    // is " *.md " and matches nothing. prim must not treat that bracket
    // content as already covering `*.md` and write into it; it must add a
    // working `[*.md]` section of its own, next to the section it leaves
    // untouched.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[ *.md ]\n[docs/**.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    prim()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success()
        .stderr(
            predicates::str::contains("added [*.md]")
                .and(predicates::str::contains("set prim_mdlint_strict = false in [*.md]").not()),
        );

    let contents = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();
    assert!(
        contents.contains("[ *.md ]\n[*.md]\nprim_mdlint_strict = false\n"),
        "the malformed header must be left exactly as written, with a real [*.md] \
         section added next to it rather than a key written inside it: {contents}"
    );

    // Pin the resolved outcome through `explain`, not by re-reading the bytes
    // prim wrote: `prim_mdlint_strict` for README.md must be attributed to
    // the newly added `[*.md]` section, at the line prim actually wrote it.
    let assert = prim()
        .current_dir(dir.path())
        .args(["explain", "README.md"])
        .assert()
        .success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    let strict_line = stdout
        .lines()
        .find(|line| line.contains("prim_mdlint_strict"))
        .expect("prim_mdlint_strict line present in output");
    assert!(strict_line.contains("= false"), "got: {strict_line}");
    assert!(
        strict_line.ends_with(".editorconfig:4 [*.md])"),
        "got: {strict_line}"
    );
}

#[test]
fn init_adds_its_own_section_when_an_existing_header_has_a_bracket_in_its_trailing_comment() {
    // `[docs/**.md] # see [docs]` has a `]` inside its trailing comment,
    // after the header's own closing bracket. `ec4rs` looks for the *last*
    // `]` on the line to decide whether a trailing comment follows a
    // header — here it does not, so the comment is not stripped, and the
    // header's real glob is not `docs/**.md`, it is the whole bracket
    // interior including the stray `] # see [docs`. prim must not mistake
    // this line for its own canonical `[docs/**.md]`; it must add a clean
    // one of its own, and the strict tier must still apply.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[docs/**.md] # see [docs]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    prim()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("added [docs/**.md]"));

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath", "docs/guide.md"])
        .write_stdin("Intro\n\n# Title\n")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD041]"));
}

#[test]
fn init_recognizes_a_section_header_with_a_trailing_comment_instead_of_duplicating_it() {
    // Reproduction (issue #117, shape 2): prim's own scanner required a
    // trimmed line to end in ']', so `[docs/**.md] # book docs` was not
    // recognized as a header at all. `ec4rs` does honour a trailing comment
    // after the closing bracket, so the section was already real; prim must
    // set the key in it, not create a second, unrelated `[docs/**.md]`.
    // Parameterized over both comment characters `ec4rs` recognizes: `#` and
    // `;`.
    for comment_char in ["#", ";"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            format!("root = true\n[docs/**.md] {comment_char} book docs\n"),
        )
        .unwrap();

        prim()
            .arg("init")
            .arg(dir.path())
            .assert()
            .success()
            .stderr(predicates::str::contains(
                "set prim_mdlint_strict = true in [docs/**.md]",
            ));

        let contents = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();
        assert_eq!(
            contents.matches("[docs/**.md]").count(),
            1,
            "the header with its comment must not be duplicated: {contents}"
        );
        assert!(
            contents.contains(&format!(
                "[docs/**.md] {comment_char} book docs\nprim_mdlint_strict = true\n"
            )),
            "the key must land right after the person's own header, comment kept as written: \
             {contents}"
        );

        prim()
            .current_dir(dir.path())
            .args(["lint", "--stdin-filepath", "docs/guide.md"])
            .write_stdin("Intro\n\n# Title\n")
            .assert()
            .code(1)
            .stdout(predicates::str::contains("[MD041]"));

        prim()
            .current_dir(dir.path())
            .args(["lint", "--stdin-filepath", "docs/wip/plan.md"])
            .write_stdin("Intro\n\n# Title\n")
            .assert()
            .code(0)
            .stdout(predicates::str::contains("[MD041]").not());
    }
}
