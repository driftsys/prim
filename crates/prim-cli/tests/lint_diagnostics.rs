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

#[test]
fn an_unknown_disabled_rule_names_the_editorconfig_line_the_typo_is_on() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains(".editorconfig:3 [*.md]"),
        "the typo is in .editorconfig, not in the Markdown file that inherits it:\n{stderr}"
    );
    assert!(
        !stderr.contains("a.md:"),
        "naming the Markdown file sends the reader to a file with nothing to fix:\n{stderr}"
    );
}

#[test]
fn the_same_unknown_id_in_two_sections_warns_about_each_one() {
    // Two sections, two separate typos to fix. Deduplicating by rule id alone
    // reports one of them and names a line the other one is not on.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("docs")).unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999\n[docs/**.md]\nprim_mdlint_disable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();
    std::fs::write(dir.path().join("docs/g.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains(".editorconfig:3 [*.md]"),
        "missing the warning for the first section:\n{stderr}"
    );
    assert!(
        stderr.contains(".editorconfig:5 [docs/**.md]"),
        "missing the warning for the second section:\n{stderr}"
    );
    assert_eq!(
        stderr.matches("MD999").count(),
        2,
        "one warning per section that carries the typo:\n{stderr}"
    );
}

#[test]
fn enabling_an_opt_in_rule_makes_it_gate() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nmax_line_length = 40\nprim_mdlint_enable = MD013\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    // A heading is the one over-width thing prim's formatter will not wrap,
    // so this finding survives `prim fmt`.
    std::fs::write(
        &file,
        "# A heading far longer than the forty columns this repository asked for\n\nText.\n",
    )
    .unwrap();

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn an_enabled_md013_uses_the_width_the_formatter_wrapped_to() {
    // rumdl's own MD013 default is 80. A repository asking for 120 and
    // enabling the rule must not have its own formatted prose reported.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nmax_line_length = 120\nprim_mdlint_enable = MD013\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    let prose = "word ".repeat(40);
    std::fs::write(&file, format!("# Title\n\n{prose}\n")).unwrap();

    prim().arg("fmt").arg(&file).assert().code(0);
    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn an_enabled_convention_rule_gates_without_the_strict_tier() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    // Opening with prose rather than a heading is an MD041 violation, so the
    // negative assertion below has something to catch: if enabling MD033 pulled
    // in its whole band, MD041 would report here.
    std::fs::write(
        &file,
        "Intro\n\n# Title\n\nText with <span>inline HTML</span>.\n",
    )
    .unwrap();

    let assert = prim().arg("lint").arg(&file).assert().code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("[MD033]"), "{stdout}");
    assert!(
        !stdout.contains("[MD041]"),
        "enabling one convention rule must not pull in the rest of its band:\n{stdout}"
    );
}

#[test]
fn disabling_beats_enabling_for_the_same_rule() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD033\nprim_mdlint_disable = MD033\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    std::fs::write(&file, "# Title\n\nText with <span>inline HTML</span>.\n").unwrap();

    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn an_enabled_rule_survives_a_file_level_strict_opt_out() {
    // The directive moves the tier; it does not cancel an enable.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\nprim_mdlint_enable = MD013\nmax_line_length = 40\n",
    )
    .unwrap();
    let file = dir.path().join("a.md");
    std::fs::write(
        &file,
        "<!-- prim-mdlint-strict: false -->\n\n# A heading far longer than the forty columns asked for\n",
    )
    .unwrap();

    let assert = prim().arg("lint").arg(&file).assert().code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).into_owned();
    assert!(stdout.contains("[MD013]"), "{stdout}");
}

#[test]
fn a_withheld_enabled_rule_warns_that_prim_does_not_run_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD072\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("prim_mdlint_enable"), "{stderr}");
    assert!(stderr.contains("MD072"), "{stderr}");
    assert!(
        stderr.contains("does not run"),
        "a withheld rule is a deliberate refusal, not a typo:\n{stderr}"
    );
    assert!(stderr.contains(".editorconfig:3 [*.md]"), "{stderr}");
}

#[test]
fn an_unknown_enabled_rule_warns_as_a_typo() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("lint").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(stderr.contains("not a rule prim knows"), "{stderr}");
}

#[test]
fn fmt_never_warns_about_an_enabled_rule() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("a.md"), "# Title\n\nText.\n").unwrap();

    let assert = prim().arg("fmt").arg(dir.path()).assert().code(0);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        !stderr.contains("MD999"),
        "prim fmt never consumes prim_mdlint_enable:\n{stderr}"
    );
}
