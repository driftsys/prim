// Behavioural acceptance tests for the hook-shim contract (story D3).
//
// git-std and the pre-commit framework both resolve the changed-file list
// themselves and invoke prim once with the resulting paths as explicit
// arguments (git-std: a glob-filtered `$@` of staged files; pre-commit:
// `entry`'s argv narrowed by the hook's `types`). prim must accept that
// mixed, arbitrary-order file list in a single invocation, format only the
// files it owns, and warn-skip (never fail) on anything it doesn't — a
// hook must never abort a commit just because an unrelated staged file
// (a `.rs`, a `.sh` script, a binary asset) was swept up in the same list.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn fmt_over_an_explicit_mixed_file_list_formats_owned_files_and_skips_the_rest() {
    // Simulates `git-std hook run pre-commit` / a pre-commit-framework
    // invocation: several staged files of different types, passed together
    // in one argv, in the order git would report them (not alphabetical).
    let dir = tempfile::tempdir().unwrap();
    let markdown = dir.path().join("README.md");
    let json = dir.path().join("data.json");
    let source = dir.path().join("main.rs");
    let script = dir.path().join("setup.sh");

    std::fs::write(&markdown, "# Title  \n").unwrap();
    std::fs::write(&json, "{\"a\":1}").unwrap();
    std::fs::write(&source, "fn main()  {}\n").unwrap();
    std::fs::write(&script, "echo hi  \n").unwrap();

    prim()
        .arg("fmt")
        .arg(&script)
        .arg(&markdown)
        .arg(&source)
        .arg(&json)
        .assert()
        .success()
        .stderr(
            predicates::str::contains("main.rs: not a file type prim formats").and(
                predicates::str::contains("setup.sh: not a file type prim formats"),
            ),
        );

    // Owned files are reformatted...
    assert_eq!(std::fs::read_to_string(&markdown).unwrap(), "# Title\n");
    assert_eq!(std::fs::read_to_string(&json).unwrap(), "{ \"a\": 1 }\n");
    // ...unowned/source files are left byte-for-byte untouched, never touched
    // by the whitespace-hygiene pass either — prim's hygiene allowlist is
    // curated, not "every text file it's handed".
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "fn main()  {}\n");
    assert_eq!(std::fs::read_to_string(&script).unwrap(), "echo hi  \n");
}

#[test]
fn fmt_check_over_an_explicit_mixed_file_list_reports_only_owned_drift() {
    // The `--check` gate form of the same contract, as a CI-side hook
    // (rather than a local commit-time hook) would invoke it.
    let dir = tempfile::tempdir().unwrap();
    let markdown = dir.path().join("README.md");
    let source = dir.path().join("main.rs");

    std::fs::write(&markdown, "# Title  \n").unwrap();
    std::fs::write(&source, "fn main()  {}\n").unwrap();

    prim()
        .args(["fmt", "--check"])
        .arg(&source)
        .arg(&markdown)
        .assert()
        .code(1)
        .stdout(predicates::str::contains("README.md"))
        .stdout(predicates::str::contains("main.rs").not());

    // --check never writes, regardless of ownership.
    assert_eq!(std::fs::read_to_string(&markdown).unwrap(), "# Title  \n");
    assert_eq!(std::fs::read_to_string(&source).unwrap(), "fn main()  {}\n");
}
