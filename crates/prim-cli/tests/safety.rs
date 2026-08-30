// Safety behaviours (FR-6.4/6.5): atomic in-place writes preserve permission
// bits, and owned files that aren't valid UTF-8 are reported, not silently
// dropped or fatal when merely discovered.

use assert_cmd::Command;
#[cfg(unix)]
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[cfg(unix)]
#[test]
fn in_place_format_preserves_file_mode() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("doc.md");
    std::fs::write(&file, "title  \n").unwrap(); // needs hygiene
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "title\n");
    let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o640, "atomic write must preserve permission bits");
}

#[test]
fn walked_owned_non_utf8_file_is_reported_but_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("data.json"); // owned type, invalid UTF-8
    std::fs::write(&bad, [0xFFu8, 0xFE, 0x00]).unwrap();
    std::fs::write(dir.path().join("ok.md"), "# Hi\n").unwrap();

    prim()
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("data.json"));

    // Left byte-for-byte unchanged.
    assert_eq!(std::fs::read(&bad).unwrap(), [0xFFu8, 0xFE, 0x00]);
}

// A symlink is a path type prim does not own (FR-4.6, AD-0016). Naming one
// must leave it intact: the atomic write of FR-6.4 is a temp file plus
// rename, which would replace the link with a regular file and leave the file
// git actually tracks still drifting (#166).

#[cfg(unix)]
#[test]
fn named_symlink_is_left_intact_and_reported() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.md");
    let link = dir.path().join("link.md");
    std::fs::write(&target, "title  \n").unwrap(); // needs hygiene
    std::os::unix::fs::symlink("target.md", &link).unwrap();

    prim()
        .arg(&link)
        .assert()
        .success()
        .stderr(predicates::str::contains("is a symbolic link"));

    assert!(
        std::fs::symlink_metadata(&link).unwrap().is_symlink(),
        "naming a symlink must not replace it with a regular file"
    );
    assert_eq!(
        std::fs::read_to_string(&target).unwrap(),
        "title  \n",
        "the symlink's target must be left byte-for-byte unchanged"
    );
}

#[cfg(unix)]
#[test]
fn named_symlink_gives_the_same_answer_as_the_walk_under_check() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target.md");
    let link = dir.path().join("link.md");
    std::fs::write(&target, "title  \n").unwrap(); // drifts
    std::os::unix::fs::symlink("target.md", &link).unwrap();

    // The walk never offers the link, so the gate must not report it either:
    // one question, one answer (AD-0009).
    prim()
        .args(["fmt", "--check"])
        .arg(dir.path())
        .assert()
        .code(1)
        .stdout(predicates::str::contains("link.md").not());

    prim()
        .args(["fmt", "--check"])
        .arg(&link)
        .assert()
        .code(0)
        .stdout(predicates::str::contains("link.md").not());
}

// AD-0016 point 5 / #152: a path that merely goes *through* a symlinked
// directory is processed normally. It ends at a regular file, so the atomic
// rename destroys nothing. Refusing it, as `git` does, would refuse every
// `/tmp/...` and `$TMPDIR/...` path on macOS, where `/tmp` and `/var` are
// themselves symlinks. Pinned so the limit reads as decided, not as a defect.
#[cfg(unix)]
#[test]
fn a_path_through_a_symlinked_directory_is_formatted_normally() {
    let dir = tempfile::tempdir().unwrap();
    let inner = dir.path().join("inner");
    std::fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    std::fs::write(&doc, "title  \n").unwrap(); // needs hygiene
    let link_dir = dir.path().join("linkdir");
    std::os::unix::fs::symlink("inner", &link_dir).unwrap();

    prim().arg(link_dir.join("doc.md")).assert().success();

    assert_eq!(
        std::fs::read_to_string(&doc).unwrap(),
        "title\n",
        "a path through a symlinked directory must still be formatted"
    );
    assert!(
        std::fs::symlink_metadata(&link_dir).unwrap().is_symlink(),
        "the traversed directory link must survive"
    );
}

/// A drifting file plus a symlink to it, in a fresh directory.
#[cfg(unix)]
fn link_to_drifting_file(dir: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let target = dir.join("target.md");
    let link = dir.join("link.md");
    std::fs::write(&target, "title  \n").unwrap();
    std::os::unix::fs::symlink("target.md", &link).unwrap();
    (target, link)
}

// AD-0016 point 4: the rule holds in every verb, not only `fmt`. `fix` is the
// one that also writes, so a guard that missed it would still destroy a link.
#[cfg(unix)]
#[test]
fn every_verb_declines_a_named_symlink() {
    for args in [
        vec!["fix"],
        vec!["lint"],
        vec!["fmt", "--diff"],
        vec!["fmt", "--check-idempotence"],
    ] {
        let dir = tempfile::tempdir().unwrap();
        let (target, link) = link_to_drifting_file(dir.path());

        prim()
            .args(&args)
            .arg(&link)
            .assert()
            .success()
            .stdout(predicates::str::contains("link.md").not())
            .stderr(predicates::str::contains("is a symbolic link"));

        assert!(
            std::fs::symlink_metadata(&link).unwrap().is_symlink(),
            "`prim {args:?}` must not replace the symlink"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "title  \n",
            "`prim {args:?}` must not write through the symlink"
        );
    }
}

// AD-0016 Consequences: a dangling link is declined for its type, not for the
// far end. Before, it reached `classify` and then the read, and was reported
// as a missing file with exit 2.
#[cfg(unix)]
#[test]
fn a_dangling_symlink_is_declined_as_a_symlink_not_as_a_missing_file() {
    for args in [vec!["fmt"], vec!["fmt", "--check"], vec!["lint"]] {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink("nowhere.md", &link).unwrap();

        prim()
            .args(&args)
            .arg(&link)
            .assert()
            .code(0)
            .stderr(predicates::str::contains("is a symbolic link"))
            .stderr(predicates::str::contains("No such file").not());

        assert!(
            std::fs::symlink_metadata(&link).unwrap().is_symlink(),
            "the dangling link must survive `prim {args:?}`"
        );
    }
}

// `prim explain` already declines a type prim does not format; a symlink is
// such a type, and answering for one would describe settings prim will never
// apply to it.
#[cfg(unix)]
#[test]
fn explain_declines_a_named_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let (_target, link) = link_to_drifting_file(dir.path());

    prim()
        .arg("explain")
        .arg(&link)
        .assert()
        .success()
        .stdout(predicates::str::contains("end_of_line").not())
        .stderr(predicates::str::contains("is a symbolic link"));
}

// `prim init` never reaches the formatting path, so the same rename destroyed
// a symlinked `.editorconfig` — the shared config it pointed at was left
// unchanged while the link became a regular file.
#[cfg(unix)]
#[test]
fn init_declines_a_symlinked_editorconfig() {
    let dir = tempfile::tempdir().unwrap();
    let real_dir = dir.path().join("real");
    std::fs::create_dir(&real_dir).unwrap();
    let real = real_dir.join(".editorconfig");
    std::fs::write(&real, "root = true\n[*]\nindent_size = 4\n").unwrap();
    let link = dir.path().join(".editorconfig");
    std::os::unix::fs::symlink("real/.editorconfig", &link).unwrap();

    prim()
        .arg("init")
        .arg(dir.path())
        .assert()
        .code(2)
        .stderr(predicates::str::contains("is a symbolic link"));

    assert!(
        std::fs::symlink_metadata(&link).unwrap().is_symlink(),
        "prim init must not replace a symlinked .editorconfig"
    );
    assert_eq!(
        std::fs::read_to_string(&real).unwrap(),
        "root = true\n[*]\nindent_size = 4\n",
        "and must not write through it either"
    );
}

// #173: `std::env::args()` panics on an argument that is not valid UTF-8, so
// every entry point carrying a path exited 101 — outside FR-5.6's 0/1/2
// contract. A hook of the shape `prim fmt "$@"`, which both shipped recipes
// use, hit it. The exit code matters more than which of 0/1/2 it is: 101 is
// not an answer any caller can interpret.
#[cfg(unix)]
#[test]
fn an_undecodable_argument_does_not_panic_on_any_entry_point() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let undecodable = || OsString::from_vec(vec![0xE9, b'b', b'a', b'd', b'.', b't', b'x', b't']);
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("doc.md"), "# hi\n").unwrap();

    let cases: Vec<(Vec<&str>, Vec<OsString>)> = vec![
        (vec!["fmt", "--check"], vec![undecodable()]),
        (vec!["explain"], vec![undecodable()]),
        (vec!["lint"], vec![undecodable()]),
        (
            vec!["fmt", "--exclude"],
            vec![undecodable(), dir.path().into()],
        ),
        (
            vec!["fmt", "--stdin-filepath"],
            vec![OsString::from_vec(vec![0xE9, b'a', b'.', b'm', b'd'])],
        ),
        // No verb in front of it: the argv preprocessor has to decide whether
        // this token is a verb without being able to decode it.
        (vec![], vec![undecodable()]),
    ];

    for (flags, tail) in cases {
        let assert = prim()
            .args(&flags)
            .args(&tail)
            .write_stdin("x  \n")
            .assert();
        let output = assert.get_output();
        let code = output.status.code().expect("prim exited normally");

        assert!(
            (0..=2).contains(&code),
            "`prim {flags:?} {tail:?}` exited {code}, outside the 0/1/2 contract\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("panicked"),
            "`prim {flags:?} {tail:?}` panicked\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

// #125: prim formats through a rayon pool with no `catch_unwind`, so one
// panicking dependency took the whole process to exit 101 and the files beside
// it produced no output either. The two inputs known to panic are pinned in
// `prim-fmt` and stay quiet only because of the `[profile.dev.package.*]`
// overrides AD-0006 records, so these drive the debug-build fault injector
// instead: `PRIM_PANIC_INJECT` panics inside the contained region for any path
// containing its value. That is what lets one file panic while its neighbours
// do not.

fn tree_with_a_panicking_file() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("boom.md"), "# Doc  \n").unwrap();
    std::fs::write(dir.path().join("ok.md"), "# Fine  \n").unwrap();
    std::fs::write(dir.path().join("note.txt"), "text  \n").unwrap();
    dir
}

#[test]
fn a_panic_exits_two_leaves_the_file_alone_and_still_formats_its_neighbours() {
    let dir = tree_with_a_panicking_file();

    prim()
        .env("PRIM_PANIC_INJECT", "boom.md")
        .arg(dir.path())
        .assert()
        .code(2)
        .stderr(predicates::str::contains("boom.md"))
        .stderr(predicates::str::contains("please report it"));

    assert_eq!(
        std::fs::read_to_string(dir.path().join("boom.md")).unwrap(),
        "# Doc  \n",
        "the panicking file must be left byte-for-byte unchanged (FR-6.3)"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("ok.md")).unwrap(),
        "# Fine\n",
        "the files beside it must still be formatted — the second half of #125"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("note.txt")).unwrap(),
        "text\n"
    );
}

#[test]
fn every_verb_and_gate_holds_the_contract_through_a_panic() {
    // `lint` is the one that stayed at 101 after the formatter was contained:
    // it reaches rumdl without ever calling `prim_fmt::format`.
    for args in [
        vec!["fmt"],
        vec!["fmt", "--check"],
        vec!["fmt", "--diff"],
        vec!["fmt", "--check-idempotence"],
        vec!["lint"],
        vec!["fix"],
    ] {
        let dir = tree_with_a_panicking_file();
        let output = prim()
            .env("PRIM_PANIC_INJECT", "boom.md")
            .args(&args)
            .arg(dir.path())
            .output()
            .unwrap();
        let code = output.status.code().expect("prim exited normally");

        assert_eq!(
            code,
            2,
            "`prim {args:?}` exited {code}, not 2\nstderr:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("boom.md")).unwrap(),
            "# Doc  \n",
            "`prim {args:?}` must leave the panicking file unchanged"
        );
    }
}

#[test]
fn a_machine_readable_lint_still_emits_its_document_through_a_panic() {
    // `--format` changes stdout alone: a pipeline should get a well-formed
    // document with the failure carried by the exit code, not an empty stream
    // that reads as a parse failure.
    for format in ["json", "sarif"] {
        let dir = tree_with_a_panicking_file();
        let output = prim()
            .env("PRIM_PANIC_INJECT", "boom.md")
            .args(["lint", "--format", format])
            .arg(dir.path())
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "format: {format}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            serde_json::from_str::<serde_json::Value>(&stdout).is_ok(),
            "`lint --format {format}` must still emit a parseable document\nstdout:\n{stdout}"
        );
    }
}

#[test]
fn a_panic_under_stdin_filepath_returns_the_buffer_rather_than_emptying_it() {
    // An editor replaces its buffer with prim's stdout, so printing nothing
    // would empty the document (AD-0017 point 5).
    for args in [vec!["fmt"], vec!["fix"]] {
        let output = prim()
            .env("PRIM_PANIC_INJECT", "buffer.md")
            .args(&args)
            .args(["--stdin-filepath", "buffer.md"])
            .write_stdin("# Draft  \n")
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "args: {args:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "# Draft  \n",
            "`prim {args:?} --stdin-filepath` must echo the buffer back unchanged"
        );
    }

    // The lint route writes nothing to stdout without `--format`, but must
    // still not exit outside the contract.
    prim()
        .env("PRIM_PANIC_INJECT", "buffer.md")
        .args(["lint", "--stdin-filepath", "buffer.md"])
        .write_stdin("# Draft  \n")
        .assert()
        .code(2);
}

/// FR-2.5 end to end through the route #173 opened: a filename that is not
/// valid UTF-8, **named on the command line**, is formatted exactly as a
/// decodable name would be. Such a name is legal on Linux and cannot exist on
/// APFS or HFS+, so this runs only where it is reachable — CI's ubuntu runner,
/// matching `changed_files.rs::a_path_that_is_not_valid_utf8_is_selected`.
///
/// Without it, decoding argv lossily passes every other test here: the six
/// entry points in `an_undecodable_argument_does_not_panic_on_any_entry_point`
/// all name a path that does not exist, so nothing observes that the bytes
/// prim was given are the bytes it used.
#[cfg(target_os = "linux")]
#[test]
fn a_path_that_is_not_valid_utf8_is_formatted_when_named() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    let odd = dir.path().join(OsStr::from_bytes(b"caf\xe9.md"));
    let odd_txt = dir.path().join(OsStr::from_bytes(b"caf\xe9.txt"));
    std::fs::write(&odd, "# Title  \n").unwrap();
    std::fs::write(&odd_txt, "x  \n").unwrap();

    prim().arg(&odd).arg(&odd_txt).assert().success();

    assert_eq!(
        std::fs::read(&odd).unwrap(),
        b"# Title\n",
        "a named path that is not valid UTF-8 must be formatted, not mangled in transit"
    );
    assert_eq!(std::fs::read(&odd_txt).unwrap(), b"x\n");

    // And the gate agrees about it, rather than reporting a clean run over a
    // file it could not name.
    std::fs::write(&odd, "# Title  \n").unwrap();
    prim().args(["fmt", "--check"]).arg(&odd).assert().code(1);
}
