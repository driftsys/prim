// Behavioural acceptance tests for `prim_mdlint_report_line_length`: the
// `.editorconfig` key that selects MD013 into the tier a path already runs.
//
// The governing rule is that prim reports only the long lines that can be
// shortened. Prose is reported — `prim fmt` wraps it to the same limit, so a
// formatted repository has none left. Table rows, fenced code and an inline
// code span are never reported, because a line break cannot be inserted into
// them without changing what the document means. A
// heading is reported at the strict tier only: prim cannot wrap one, but an
// author can shorten it.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

/// Write `.editorconfig` + `doc.md` into a fresh temp dir.
fn project(editorconfig: &str, markdown: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".editorconfig"), editorconfig).unwrap();
    let file = dir.path().join("doc.md");
    std::fs::write(&file, markdown).unwrap();
    (dir, file)
}

const LONG_PROSE: &str = "This prose line is written past the eighty column budget on purpose so that MD013 has something to report.\n";

#[test]
fn reports_nothing_when_the_key_is_unset() {
    let (_dir, file) = project("root = true\n[*.md]\nmax_line_length = 80\n", LONG_PROSE);

    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn reports_a_long_prose_line_when_the_key_is_true() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\n",
        LONG_PROSE,
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn measures_against_the_resolved_max_line_length_not_rumdls_default() {
    // The line is 106 columns: over rumdl's own MD013 default of 80, under the
    // 120 this project resolves. prim wraps prose to 120 here, so reporting it
    // would fail prim's own formatter output.
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 120\nprim_mdlint_report_line_length = true\n",
        LONG_PROSE,
    );

    prim().arg("lint").arg(&file).assert().code(0);
}

const KEY_ON: &str =
    "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\n";

#[test]
fn a_long_table_row_is_never_reported() {
    let (_dir, file) = project(
        KEY_ON,
        "\
| a column heading that is deliberately long | another column heading that is long |
| ------------------------------------------ | ----------------------------------- |
| a | b |
",
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD013]").not());
}

#[test]
fn a_long_line_inside_fenced_code_is_never_reported() {
    let (_dir, file) = project(
        KEY_ON,
        "```text\nalpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho\n```\n",
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD013]").not());
}

#[test]
fn a_line_long_only_because_of_an_inline_code_span_is_never_reported() {
    // The span must sit inside breakable prose. A line that is one unbreakable
    // token is already exempt for a different reason, so it would pass whether
    // `code-spans` was set or not and would pin nothing.
    let (_dir, file) = project(
        KEY_ON,
        "Prose before `a code span with internal spaces that pushes this line past the budget` end.\n",
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD013]").not());
}

#[test]
fn an_explicit_false_reports_nothing() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = false\n",
        LONG_PROSE,
    );

    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn an_unset_max_line_length_falls_back_to_eighty() {
    // FR-3.2d: the fallback matches the formatter's own, so a repository that
    // sets only the key still gets the width prim wraps to.
    let (_dir, file) = project(
        "root = true\n[*.md]\nprim_mdlint_report_line_length = true\n",
        LONG_PROSE,
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn reads_the_key_on_the_stdin_path_too() {
    let (dir, _file) = project(KEY_ON, "placeholder\n");

    prim()
        .arg("lint")
        .arg("--stdin-filepath")
        .arg(dir.path().join("doc.md"))
        .write_stdin(LONG_PROSE)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn a_long_heading_is_silent_at_the_floor_tier() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\nprim_mdlint_strict = false\n",
        "# A heading written well past the eighty column budget so that MD013 could report it\n",
    );

    prim().arg("lint").arg(&file).assert().code(0);
}

#[test]
fn a_long_heading_is_reported_at_the_strict_tier() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\nprim_mdlint_strict = true\n",
        "# A heading written well past the eighty column budget so that MD013 could report it\n",
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn prim_mdlint_disable_turns_the_rule_off_again() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\nprim_mdlint_disable = MD013\n",
        LONG_PROSE,
    );

    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .code(0)
        .stderr(predicates::str::contains("MD013").not());
}

#[test]
fn explain_reports_the_resolved_key() {
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\n",
        LONG_PROSE,
    );

    prim()
        .arg("explain")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains(
            "prim_mdlint_report_line_length = true",
        ))
        .stdout(predicates::str::contains(".editorconfig:4 [*.md]"));
}

#[test]
fn explain_reports_the_key_as_false_when_it_is_unset() {
    let (_dir, file) = project("root = true\n[*.md]\nmax_line_length = 80\n", LONG_PROSE);

    prim()
        .arg("explain")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains(
            "prim_mdlint_report_line_length = false",
        ))
        .stdout(predicates::str::contains("(prim's default)"));
}

#[test]
fn a_narrower_glob_can_turn_the_key_off_again() {
    // FR-3.2d resolves through the same per-glob cascade as
    // `prim_mdlint_strict`: last match wins, so a narrower section replaces a
    // wider one rather than merging with it.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\n[quiet/**.md]\nprim_mdlint_report_line_length = false\n",
    )
    .unwrap();
    std::fs::create_dir(dir.path().join("quiet")).unwrap();
    let loud = dir.path().join("loud.md");
    let quiet = dir.path().join("quiet").join("doc.md");
    std::fs::write(&loud, LONG_PROSE).unwrap();
    std::fs::write(&quiet, LONG_PROSE).unwrap();

    prim().arg("lint").arg(&loud).assert().code(1);
    prim().arg("lint").arg(&quiet).assert().code(0);
}

/// Exactly `columns` characters of breakable prose, never ending on a space —
/// trailing whitespace would be stripped by hygiene and change the width.
fn prose_of(columns: usize) -> String {
    let mut line = String::new();
    while line.len() < columns {
        line.push(if line.len() % 5 == 4 { ' ' } else { 'w' });
    }
    if line.ends_with(' ') {
        line.pop();
        line.push('w');
    }
    assert_eq!(line.len(), columns);
    format!("{line}\n")
}

#[test]
fn the_limit_is_exactly_max_line_length() {
    // The boundary is the whole point: prim wraps to `max_line_length`, so a
    // line of exactly that width is output prim produced and must be clean,
    // while one character more must be reported. An off-by-one either way
    // makes the formatter and the linter disagree.
    let config =
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_report_line_length = true\n";

    let (_at, at_limit) = project(config, &prose_of(80));
    prim().arg("lint").arg(&at_limit).assert().code(0);

    let (_over, over_limit) = project(config, &prose_of(81));
    prim()
        .arg("lint")
        .arg(&over_limit)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn the_unset_fallback_is_exactly_eighty() {
    // Pins the documented 80, not merely "some value below the fixture width".
    let config = "root = true\n[*.md]\nprim_mdlint_report_line_length = true\n";

    let (_at, at_limit) = project(config, &prose_of(80));
    prim().arg("lint").arg(&at_limit).assert().code(0);

    let (_over, over_limit) = project(config, &prose_of(81));
    prim()
        .arg("lint")
        .arg(&over_limit)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("[MD013]"));
}

#[test]
fn the_strict_tier_alone_does_not_select_md013() {
    // FR-3.2d: the key selects the rule, the tier does not. Opting into
    // conventions must not silently start reporting line length.
    let (_dir, file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\nprim_mdlint_strict = true\n",
        &format!("# Title\n\n{LONG_PROSE}"),
    );

    // The strict tier fires its own rules on this file; the assertion is that
    // MD013 is not among them.
    prim()
        .arg("lint")
        .arg(&file)
        .assert()
        .stdout(predicates::str::contains("[MD013]").not());
}

#[test]
fn the_stdin_path_stays_silent_when_the_key_is_unset() {
    let (dir, _file) = project(
        "root = true\n[*.md]\nmax_line_length = 80\n",
        "placeholder\n",
    );

    prim()
        .arg("lint")
        .arg("--stdin-filepath")
        .arg(dir.path().join("doc.md"))
        .write_stdin(LONG_PROSE)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("[MD013]").not());
}

#[test]
fn explain_omits_the_key_for_a_file_that_is_not_markdown() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_report_line_length = true\n",
    )
    .unwrap();
    let file = dir.path().join("data.json");
    std::fs::write(&file, "{}\n").unwrap();

    prim()
        .arg("explain")
        .arg(&file)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("prim_mdlint_report_line_length").not());
}
