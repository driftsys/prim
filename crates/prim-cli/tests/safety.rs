// Safety behaviours (FR-6.4/6.5): atomic in-place writes preserve permission
// bits, and owned files that aren't valid UTF-8 are reported, not silently
// dropped or fatal when merely discovered.

use assert_cmd::Command;

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
