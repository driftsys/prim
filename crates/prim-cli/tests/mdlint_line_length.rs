// Behavioural acceptance tests for `prim_mdlint_report_line_length`: the
// `.editorconfig` key that selects MD013 into the tier a path already runs.
//
// The governing rule is that prim reports only the long lines that can be
// shortened. Prose is reported — `prim fmt` wraps it to the same limit, so a
// formatted repository has none left. Table rows, fenced code and an inline
// code span with no internal space are never reported, because a line break
// cannot be inserted into them without changing what the document means. A
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
        "```rust\nlet x = \"a line inside a fenced code block that runs past the eighty column budget\";\n```\n",
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
