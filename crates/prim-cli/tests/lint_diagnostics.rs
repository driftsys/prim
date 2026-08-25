// Behavioural acceptance tests for story B1 (`prim lint` diagnostics
// mode): each whitespace-hygiene violation on the un-owned-text allowlist
// (the same set A1's BOM strip covers) reports a stable `code` and a
// 1-indexed `file:line:col`, never rewriting the file. Markdown now also gets
// itemized rumdl content diagnostics; JSON/YAML/TOML still keep the coarser
// format-drift finding until their own content diagnostics land (D2).

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
fn json_keeps_the_coarse_format_drift_finding() {
    // Markdown now has its own itemized content diagnostics (story G2), but
    // JSON/YAML/TOML still keep the pre-existing single "format drift"
    // finding, not itemized codes.
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
fn markdown_reports_rumdl_rule_codes_with_positions_instead_of_coarse_drift() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "#Title\n\nSee https://example.com.\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains("README.md:3:")
            .and(predicates::str::contains("[MD034]"))
            .and(predicates::str::contains("prim fmt").not()),
    );

    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "#Title\n\nSee https://example.com.\n"
    );
}

#[test]
fn markdown_floor_defect_raises_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "# Title\n\n![](hero.png)\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains("README.md:3:")
            .and(predicates::str::contains("[MD045]"))
            .and(predicates::str::contains("Image missing alt text")),
    );
}

#[test]
fn markdown_convention_rules_are_silent_until_strict() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(&file, "# Title\n\n```\ncode\n```\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD040]").not());

    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD040]"));
}

#[test]
fn file_level_directive_drops_a_strict_glob_back_to_floor() {
    // Story G5 (#61): a per-file `<!-- prim-mdlint-strict: false -->`
    // overrides the .editorconfig-resolved strict tier for this file only.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(
        &file,
        "<!-- prim-mdlint-strict: false -->\nIntro\n\n# Title\n",
    )
    .unwrap();

    // MD041 (first-line-heading) is strict-only; the directive drops this
    // file back to floor, so it must not fire despite the strict glob.
    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD041]").not());
}

#[test]
fn file_level_directive_raises_a_floor_glob_to_strict() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    std::fs::write(
        &file,
        "<!-- prim-mdlint-strict: true -->\nIntro\n\n# Title\n",
    )
    .unwrap();

    // No .editorconfig at all (floor by default); the directive raises this
    // file to strict on its own. MD041 is a convention rule that only runs in
    // strict, and every rule that runs is an error.
    prim().arg("lint").arg(&file).assert().code(1).stdout(
        predicates::str::contains("[MD041]").and(predicates::str::contains("level 1 heading")),
    );
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

#[test]
fn disable_key_subtracts_a_rule_from_the_strict_tier() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("docs/guide.md");
    std::fs::write(&file, "# Title\n\nPress <kbd>Ctrl</kbd>.\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD033]").not());
}

#[test]
fn disable_key_does_not_reach_other_globs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let outside = dir.path().join("README.md");
    std::fs::write(&outside, "# Title\n\nPress <kbd>Ctrl</kbd>.\n").unwrap();

    prim()
        .arg("lint")
        .arg(&outside)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD033]"));
}

#[test]
fn an_unknown_disabled_rule_warns_without_changing_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    let file = dir.path().join("README.md");
    // A genuine floor-tier violation (MD045) alongside the unknown id: if the
    // unknown-id path ever swallowed a real finding, this would still pass at
    // exit 0 with the fixture from before. Asserting the `1` this violation
    // earns proves the unknown id changed nothing.
    std::fs::write(&file, "# Title\n\n![](hero.png)\n").unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD045]"))
        .stderr(predicates::str::contains("MD999"));
}

#[test]
fn an_unknown_disabled_rule_warns_once_per_run_not_once_per_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    for name in ["a.md", "b.md", "c.md"] {
        std::fs::write(dir.path().join(name), "# Title\n\nText.\n").unwrap();
    }

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert_eq!(
        stderr.matches("MD999").count(),
        1,
        "an unknown id must warn once per run, not once per matching file:\n{stderr}"
    );
}

#[test]
fn fmt_never_warns_about_an_unknown_disabled_rule() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    // `fmt` resolves the policy (to know the tier for a later lint pass) but
    // never reads `.disabled`/`.unknown` — it must stay silent about this key.
    let assert = prim().arg("fmt").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        !stderr.contains("MD999"),
        "prim fmt must not warn about prim_mdlint_disable, it never consumes it:\n{stderr}"
    );
}
