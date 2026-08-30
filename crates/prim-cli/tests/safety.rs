// Safety behaviours (FR-6.4/6.5): atomic in-place writes preserve permission
// bits, and owned files that aren't valid UTF-8 are reported, not silently
// dropped or fatal when merely discovered.

use assert_cmd::Command;
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
        .stderr(predicates::str::contains("link.md"));

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
