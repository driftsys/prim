// Behavioural acceptance tests for the `prim` operating modes, exercised
// against real temp files and real stdin (no mocks). At the scaffold stage
// the formatter is a no-op, so "already formatted" means "any input".
//
// These live in the bin crate (not the `spec` crate) so cargo provides
// `CARGO_BIN_EXE_prim` for reliable binary resolution.

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn stdin_filepath_applies_whitespace_hygiene_to_stdout() {
    // Trailing whitespace is trimmed; the formatted result goes to stdout.
    prim()
        .args(["--stdin-filepath", "doc.md"])
        .write_stdin("# Title\n\nbody text  \n")
        .assert()
        .success()
        .stdout("# Title\n\nbody text\n");
}

#[test]
fn stdin_for_unowned_filetype_passes_through_unchanged() {
    // prim does not own .rs, so stdin is emitted verbatim.
    let input = "fn main()  {}  \n";
    prim()
        .args(["--stdin-filepath", "main.rs"])
        .write_stdin(input)
        .assert()
        .success()
        .stdout(input.to_string());
}

#[test]
fn check_on_already_formatted_file_exits_zero() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("data.json");
    std::fs::write(&file, "{ \"a\": 1 }\n").unwrap();

    prim().arg("--check").arg(&file).assert().success();
}

#[test]
fn default_in_place_leaves_already_formatted_file_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.md");
    let original = "# Heading\n\ntext\n";
    std::fs::write(&file, original).unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn unreadable_path_reports_error_and_exits_two() {
    prim().arg("/no/such/prim/fixture.md").assert().code(2);
}
