//! Behavioural tests: prim honors `.editorconfig` (FR-3).

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

fn prim() -> Command {
    Command::cargo_bin("prim").unwrap()
}

#[test]
fn crlf_end_of_line_is_written() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*]\nend_of_line = crlf\n",
    )
    .unwrap();
    // A `.txt` orphan is hygiene-only, isolating the end_of_line setting from any
    // per-format structured pass.
    let file = dir.path().join("notes.txt");
    fs::write(&file, "a\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\r\nb\r\n");
}

#[test]
fn insert_final_newline_false_strips_trailing_newline() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ninsert_final_newline=false\n",
    )
    .unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{}\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
}

#[test]
fn trim_disabled_keeps_trailing_whitespace() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\ntrim_trailing_whitespace=false\n",
    )
    .unwrap();
    // A `.txt` orphan stays hygiene-only (never structurally formatted), so it
    // isolates the trim_trailing_whitespace setting from any per-format pass.
    let file = dir.path().join("notes.txt");
    fs::write(&file, "a  \n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a  \n");
}

#[test]
fn check_mode_flags_crlf_when_config_demands_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nend_of_line=crlf\n",
    )
    .unwrap();
    let file = dir.path().join("a.toml");
    fs::write(&file, "a = 1\n").unwrap(); // LF on disk, config wants CRLF

    prim().arg("--check").arg(&file).assert().failure().code(1);
}

#[test]
fn stdin_filepath_honors_sibling_editorconfig() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root=true\n[*]\nend_of_line=crlf\n",
    )
    .unwrap();
    let target = dir.path().join("x.txt");

    prim()
        .arg("--stdin-filepath")
        .arg(&target)
        .write_stdin("a\nb\n")
        .assert()
        .success()
        .stdout("a\r\nb\r\n");
}

#[test]
fn no_editorconfig_leaves_canonical_behaviour() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.txt");
    fs::write(&file, "a  \r\nb\n").unwrap();

    prim().arg(&file).assert().success();

    assert_eq!(fs::read_to_string(&file).unwrap(), "a\nb\n");
}

// #153: `ec4rs` skips an `.editorconfig` it cannot open and carries on
// (`src/file.rs`: `if let Ok(file) = ConfigFile::open(...)`). prim inherits
// that everywhere it resolves a cascade, so an unreadable ancestor silently
// changed which settings applied. These pin that prim now says so.

/// An ancestor `.editorconfig` that exists but cannot be opened. Returns
/// `None` where the process can read it anyway — running as root, or a
/// filesystem without permission bits — so the test skips rather than fails.
#[cfg(unix)]
fn unreadable_ancestor(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let config = dir.join(".editorconfig");
    fs::write(&config, "root = true\n[*]\nmax_line_length = 120\n").unwrap();
    fs::set_permissions(&config, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_to_string(&config).is_ok() {
        return None; // readable regardless of mode; nothing to report.
    }
    Some(config)
}

#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_editorconfig_is_reported_by_explain() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let Some(_config) = unreadable_ancestor(root.path()) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(".editorconfig"));
}

#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_editorconfig_is_reported_while_formatting() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    fs::write(inner.join("doc.md"), "text\n").unwrap();
    let Some(_config) = unreadable_ancestor(root.path()) else {
        return;
    };

    prim()
        .arg(&inner)
        .assert()
        .success()
        .stderr(predicates::str::contains(".editorconfig"));
}

#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_editorconfig_is_reported_once_per_run() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    // Several files across several directories, so the per-directory cascade
    // cache is built more than once and the report still lands only once.
    for sub in ["a", "b", "c"] {
        let dir = inner.join(sub);
        fs::create_dir(&dir).unwrap();
        for name in ["one.md", "two.md"] {
            fs::write(dir.join(name), "text\n").unwrap();
        }
    }
    let Some(config) = unreadable_ancestor(root.path()) else {
        return;
    };

    let output = prim().arg(&inner).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let needle = config.display().to_string();
    assert_eq!(
        stderr.matches(&needle).count(),
        1,
        "the unreadable ancestor must be named once per run, not once per \
         directory or once per thread\nstderr:\n{stderr}"
    );
}

#[cfg(unix)]
#[test]
fn an_editorconfig_above_a_root_true_boundary_is_not_reported() {
    // `ec4rs` stops climbing at `root = true`, so an unreadable file above
    // that boundary never affected resolution and naming it would be noise.
    let outer = tempfile::tempdir().unwrap();
    let middle = outer.path().join("middle");
    fs::create_dir(&middle).unwrap();
    fs::write(
        middle.join(".editorconfig"),
        "root = true\n[*]\nmax_line_length = 80\n",
    )
    .unwrap();
    let doc = middle.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let Some(config) = unreadable_ancestor(outer.path()) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(config.display().to_string()).not());
}

// AD-0002 recorded that prim's "report a bad config" posture was only partly
// implemented, and named two silent cases: a file prim cannot open, and one
// whose first invalid line is at or before its first section header — an
// unclosed `[*.md`, the common typo. Both are `ConfigFile::open` failures, so
// both are now reported, with the word that fits each.
#[test]
fn a_malformed_section_header_is_reported_as_malformed_not_unreadable() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    fs::write(
        root.path().join(".editorconfig"),
        "[*.md\nindent_size = 4\n",
    )
    .unwrap();

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "ignoring malformed .editorconfig",
        ))
        .stderr(predicates::str::contains("unreadable").not());
}

#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_is_reported_as_unreadable_not_malformed() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let Some(_config) = unreadable_ancestor(root.path()) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "ignoring unreadable .editorconfig",
        ));
}
