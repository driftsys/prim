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
    let Some(config) = unreadable_ancestor(root.path()) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(format!(
            "{}: ignoring unreadable .editorconfig",
            config.display()
        )));
}

#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_editorconfig_is_reported_while_formatting() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    fs::write(inner.join("doc.md"), "text\n").unwrap();
    let Some(config) = unreadable_ancestor(root.path()) else {
        return;
    };

    prim()
        .arg(&inner)
        .assert()
        .success()
        .stderr(predicates::str::contains(format!(
            "{}: ignoring unreadable .editorconfig",
            config.display()
        )));
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

// The load-bearing property of #153, and the one the earlier tests all
// missed: reporting a skipped file must not change what resolves. A file
// `ec4rs` could not open drops only itself; every readable config in the
// cascade still applies. That is what separates this from the
// section-iteration warning beside it, which drops the whole cascade to
// canonical style (AD-0002).
#[cfg(unix)]
#[test]
fn an_unreadable_ancestor_does_not_discard_the_readable_ones() {
    let outer = tempfile::tempdir().unwrap();
    let readable = outer.path().join(".editorconfig");
    fs::write(&readable, "[*]\nmax_line_length = 120\n").unwrap();
    let middle = outer.path().join("middle");
    fs::create_dir(&middle).unwrap();
    let inner = middle.join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let Some(_bad) = unreadable_ancestor(&middle) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        // Still 120, still attributed to the file that set it.
        .stdout(predicates::str::contains("max_line_length"))
        .stdout(predicates::str::contains("120"))
        .stdout(predicates::str::contains(readable.display().to_string()));
}

// `ec4rs` skips an unopenable file and keeps climbing, so the probe must too:
// stopping at the first one would leave the second unnamed while it still
// affects resolution.
#[cfg(unix)]
#[test]
fn every_unreadable_ancestor_in_the_climb_is_named() {
    let outer = tempfile::tempdir().unwrap();
    let middle = outer.path().join("middle");
    fs::create_dir(&middle).unwrap();
    let inner = middle.join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let (Some(high), Some(low)) = (
        unreadable_ancestor(outer.path()),
        unreadable_ancestor(&middle),
    ) else {
        return;
    };

    let output = prim().arg("explain").arg(&doc).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    for path in [&high, &low] {
        assert_eq!(
            stderr.matches(&path.display().to_string()).count(),
            1,
            "{} must be named exactly once\nstderr:\n{stderr}",
            path.display()
        );
    }
}

// The dedup key is the path. A global one-shot would let the first bad file
// suppress every other one for the whole run.
#[cfg(unix)]
#[test]
fn two_unreadable_ancestors_are_each_named_once_across_many_files() {
    let outer = tempfile::tempdir().unwrap();
    let middle = outer.path().join("middle");
    fs::create_dir(&middle).unwrap();
    for sub in 0..12 {
        let dir = middle.join(format!("sub{sub}"));
        fs::create_dir(&dir).unwrap();
        // Enough files to make rayon actually use several threads: the claim
        // is "once per run", and one resolver is built per thread.
        for name in 0..24 {
            fs::write(dir.join(format!("doc{name}.md")), "text\n").unwrap();
        }
    }
    let (Some(high), Some(low)) = (
        unreadable_ancestor(outer.path()),
        unreadable_ancestor(&middle),
    ) else {
        return;
    };

    let output = prim().arg(&middle).output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    for path in [&high, &low] {
        // Count the cascade report specifically: the walk also meets
        // `middle/.editorconfig` as a file it cannot read, which is a
        // separate, correct message about the same path.
        let report = format!("{}: ignoring unreadable", path.display());
        assert_eq!(
            stderr.matches(&report).count(),
            1,
            "{} must be named once per run, not once per directory or thread\nstderr:\n{stderr}",
            path.display()
        );
    }
}

// The file's own directory counts for resolution — unlike `prim init`, which
// owns the file it is about to write.
#[cfg(unix)]
#[test]
fn an_unreadable_editorconfig_beside_the_file_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let Some(config) = unreadable_ancestor(dir.path()) else {
        return;
    };

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(config.display().to_string()));
}

// `ec4rs` absolutizes a relative probe against the working directory before
// it climbs, so the probe has to as well or it walks a different ancestry.
// `prim` invoked on a relative path is the ordinary case, not an edge one.
#[cfg(unix)]
#[test]
fn a_relative_invocation_still_names_the_unreadable_ancestor() {
    let outer = tempfile::tempdir().unwrap();
    let inner = outer.path().join("inner");
    fs::create_dir(&inner).unwrap();
    fs::write(inner.join("doc.md"), "text\n").unwrap();
    let Some(config) = unreadable_ancestor(outer.path()) else {
        return;
    };

    prim()
        .current_dir(outer.path())
        .arg("explain")
        .arg("inner/doc.md")
        .assert()
        .success()
        .stderr(predicates::str::contains(config.display().to_string()));
}

// The existence guard: an absent `.editorconfig` is the ordinary case, and
// every ancestor up to the filesystem root is one. Without the guard prim
// would warn about `/`, `/var`, and everything between on every run.
#[test]
fn a_tree_with_no_editorconfig_says_nothing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("doc.md"), "# hi\n").unwrap();

    prim()
        .arg(dir.path())
        .assert()
        .success()
        .stderr(predicates::str::contains("ignoring").not());
}

// AD-0002 names two shapes that fail before the first section header: an
// invalid header, and an invalid line ahead of one.
#[test]
fn an_invalid_line_before_the_first_header_is_reported_as_malformed() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let config = root.path().join(".editorconfig");
    fs::write(&config, "junk line without equals\n[*]\nindent_size = 4\n").unwrap();

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(format!(
            "{}: ignoring malformed .editorconfig",
            config.display()
        )));
}

// `ec4rs` reports a file that is not valid UTF-8 as an I/O error, but prim
// read those bytes fine — they were not `.editorconfig`. Calling that
// "unreadable" contradicted the classification's own rule, and the same fault
// was already called "malformed" when the bad bytes sat after the first
// section header, so the position of the bytes decided the noun.
#[test]
fn a_non_utf8_editorconfig_is_malformed_not_unreadable() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();
    let config = root.path().join(".editorconfig");
    fs::write(&config, b"# caf\xe9\n[*]\nindent_size = 4\n").unwrap();

    prim()
        .arg("explain")
        .arg(&doc)
        .assert()
        .success()
        .stderr(predicates::str::contains(format!(
            "{}: ignoring malformed .editorconfig",
            config.display()
        )));
}

// #153 one level up: an ancestor *directory* prim cannot search hides whatever
// `.editorconfig` it holds. Resolution changes and, with a guard that stats
// the candidate, nothing is said — the stat fails for the same reason the
// open does.
#[cfg(unix)]
#[test]
fn an_unsearchable_ancestor_directory_is_reported() {
    use std::os::unix::fs::PermissionsExt;

    let outer = tempfile::tempdir().unwrap();
    let middle = outer.path().join("middle");
    fs::create_dir(&middle).unwrap();
    fs::write(
        middle.join(".editorconfig"),
        "root = true\n[*]\nmax_line_length = 77\n",
    )
    .unwrap();
    let inner = middle.join("inner");
    fs::create_dir(&inner).unwrap();
    let doc = inner.join("doc.md");
    fs::write(&doc, "text\n").unwrap();

    fs::set_permissions(&middle, fs::Permissions::from_mode(0o000)).unwrap();
    let searchable = fs::read_to_string(middle.join(".editorconfig")).is_ok();
    let assertion = prim().arg("explain").arg(&doc).assert();
    fs::set_permissions(&middle, fs::Permissions::from_mode(0o755)).unwrap();
    if searchable {
        return; // running as root, or a filesystem without permission bits.
    }

    assertion
        .success()
        .stderr(predicates::str::contains(".editorconfig"))
        .stderr(predicates::str::contains("Permission denied"));
}

// A dangling `.editorconfig` symlink comes back `NotFound`, and there is no
// config there to have applied — so it stays silent, unlike the two faults
// above.
#[cfg(unix)]
#[test]
fn a_dangling_editorconfig_symlink_says_nothing() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    fs::write(inner.join("doc.md"), "# hi\n").unwrap();
    std::os::unix::fs::symlink("nowhere", root.path().join(".editorconfig")).unwrap();

    prim()
        .arg(&inner)
        .assert()
        .success()
        .stderr(predicates::str::contains("ignoring").not());
}

// The reported path must not depend on how the caller spelled the argument.
#[cfg(unix)]
#[test]
fn the_reported_path_does_not_carry_a_dot_component() {
    let root = tempfile::tempdir().unwrap();
    let inner = root.path().join("inner");
    fs::create_dir(&inner).unwrap();
    fs::write(inner.join("doc.md"), "text\n").unwrap();
    let Some(config) = unreadable_ancestor(root.path()) else {
        return;
    };

    let output = prim()
        .current_dir(root.path())
        .arg("explain")
        .arg("./inner/doc.md")
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains(&config.display().to_string()),
        "the ancestor must be named by its plain path\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("/./"),
        "a `.` component must not reach the message\nstderr:\n{stderr}"
    );
}
