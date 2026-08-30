//! Turning git's raw output into paths.
//!
//! Split out of [`super`] so the decoding is reachable from a unit test: a
//! filename that is not valid UTF-8 cannot be created on APFS or HFS+, so an
//! end-to-end test of one runs only on Linux, and the wiring would otherwise
//! be unguarded on the machines prim is developed on (#168).

use std::collections::HashSet;
use std::path::PathBuf;

/// Rebuild a path from the raw bytes git reported.
///
/// On unix a path is an arbitrary byte string, so the bytes *are* the path.
/// Decoding through `str` turns a name that is not valid UTF-8 into U+FFFD,
/// which then resolves to nothing and vanishes from the selection — a file the
/// gate never examines and never mentions (#168).
///
/// Under `-z` git writes the bytes the tree stores, on every platform, so a
/// repository committed elsewhere can hand a Windows client a name Windows
/// cannot represent. There a path is Unicode, so prim refuses rather than
/// inventing one.
#[cfg(unix)]
pub(super) fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    Some(PathBuf::from(std::ffi::OsStr::from_bytes(bytes)))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    utf8_path_from_bytes(bytes)
}

/// The decode used where a filename is Unicode.
///
/// Compiled unconditionally, and on unix reached only from tests: CI runs the
/// test suite on Linux alone, so this is the one way the non-unix behaviour —
/// and the usage error FR-4.2e requires of it — is pinned at all.
#[cfg_attr(unix, allow(dead_code, reason = "unix reaches this only from tests"))]
fn utf8_path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    std::str::from_utf8(bytes).ok().map(PathBuf::from)
}

/// The repository root `git rev-parse --show-toplevel` reported. It is a path
/// as much as any diff entry, and decoding it through `str` would corrupt the
/// root that every reported path is joined onto — losing the whole selection
/// rather than one entry.
pub(super) fn repo_root_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
    path_from_bytes(trim_line_terminator(bytes))
}

/// Strip the one line terminator git appends, and no more: a directory name
/// may legally end in a newline, which is the same reason the diff is read as
/// bytes.
pub(super) fn trim_line_terminator(bytes: &[u8]) -> &[u8] {
    let Some(rest) = bytes.strip_suffix(b"\n") else {
        return bytes;
    };

    // Only where git may write CRLF. On unix it writes a bare LF, so a CR
    // before it is part of the directory name, and stripping it retargets the
    // repository root at a sibling directory — an empty selection and a clean
    // exit over a drifting file, which is #168 again by another route.
    #[cfg(not(unix))]
    let rest = rest.strip_suffix(b"\r").unwrap_or(rest);

    rest
}

/// One record of `git diff --name-status --no-renames -z`: the status letter
/// git gave the path, and the path itself.
///
/// Asking for the status rather than filtering deletions out is what lets the
/// caller classify every reported path before excusing any. `--diff-filter=d`
/// hid deletions instead, which both dropped a staged deletion of a file that
/// still exists and drifts, and left every other absence indistinguishable
/// (#169).
#[derive(Debug)]
pub(super) struct DiffEntry {
    /// git reported the path as deleted, which FR-4.2b has always allowed prim
    /// to pass over in silence when it is really gone.
    pub(super) is_deletion: bool,
    pub(super) path: PathBuf,
}

/// `Err` holds the offending field, rendered lossily, when a path cannot be
/// represented on this platform, or when git's records do not pair up.
pub(super) fn diff_entries(output: &[u8]) -> Result<Vec<DiffEntry>, String> {
    let mut fields = output.split(|byte| *byte == 0).filter(|f| !f.is_empty());
    let mut entries = Vec::new();

    while let Some(status) = fields.next() {
        let Some(path) = fields.next() else {
            return Err(format!(
                "a status with no path: {}",
                String::from_utf8_lossy(status)
            ));
        };
        let path =
            path_from_bytes(path).ok_or_else(|| String::from_utf8_lossy(path).into_owned())?;

        entries.push(DiffEntry {
            // `--no-renames` decomposes a rename into a delete plus an add, so
            // a status is one letter, optionally followed by a similarity
            // score prim does not read.
            is_deletion: status.first() == Some(&b'D'),
            path,
        });
    }

    Ok(entries)
}

pub(super) struct IndexEntries {
    /// Every path the index tracks. A reported path missing from this set is
    /// not a repository state at all — git named something it does not have,
    /// which means prim misread git's output.
    pub(super) tracked: HashSet<PathBuf>,
    /// The subset git will not put in the working tree, so absent on purpose.
    pub(super) not_materialised: HashSet<PathBuf>,
}

/// Both answers from one `git ls-files -v -z --full-name -- :/`.
///
/// `S` is skip-worktree, which is how sparse checkout is implemented, and a
/// lowercase tag is assume-unchanged. Either means the path is absent by
/// design rather than because prim failed to resolve it (#169).
pub(super) fn index_entries(output: &[u8]) -> IndexEntries {
    let mut tracked = HashSet::new();
    let mut not_materialised = HashSet::new();

    for entry in output.split(|byte| *byte == 0) {
        if entry.len() < 3 || entry[1] != b' ' {
            continue;
        }
        let Some(path) = path_from_bytes(&entry[2..]) else {
            continue;
        };

        if entry[0] == b'S' || entry[0].is_ascii_lowercase() {
            not_materialised.insert(path.clone());
        }
        tracked.insert(path);
    }

    IndexEntries {
        tracked,
        not_materialised,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        diff_entries, index_entries, path_from_bytes, repo_root_from_bytes, trim_line_terminator,
        utf8_path_from_bytes,
    };
    use std::path::{Path, PathBuf};

    /// #168: on unix a filename is an arbitrary byte string. Decoding through
    /// `str` replaces the invalid bytes with U+FFFD, producing a path that
    /// names nothing, so the file drops out of the selection silently.
    #[cfg(unix)]
    #[test]
    fn a_path_that_is_not_valid_utf8_keeps_its_bytes() {
        use std::os::unix::ffi::OsStrExt;

        // `caf\xe9.txt` — Latin-1, legal on Linux, not valid UTF-8.
        let raw = b"caf\xe9.txt";

        let path = path_from_bytes(raw).expect("a unix path is always representable");

        assert_eq!(
            path.as_os_str().as_bytes(),
            raw,
            "the bytes git reported must survive decoding"
        );
    }

    #[test]
    fn a_valid_utf8_path_round_trips() {
        let path = path_from_bytes("café.txt".as_bytes()).expect("valid UTF-8");
        assert_eq!(path.to_str(), Some("café.txt"));
    }

    #[test]
    fn the_line_terminator_comes_off_without_touching_real_whitespace() {
        assert_eq!(trim_line_terminator(b"/repo root \n"), b"/repo root ");
        assert_eq!(trim_line_terminator(b"/repo"), b"/repo");
        assert_eq!(trim_line_terminator(b""), b"");
    }

    /// One terminator, not every trailing newline byte: a directory name may
    /// legally end in one, which is the same reason the diff is read as bytes.
    #[test]
    fn only_one_terminator_comes_off() {
        assert_eq!(trim_line_terminator(b"/re\npo\n"), b"/re\npo");
        assert_eq!(trim_line_terminator(b"/repo\n\n"), b"/repo\n");
        // A bare CR was never a terminator, so it is part of the name.
        assert_eq!(trim_line_terminator(b"/repo\r"), b"/repo\r");
    }

    /// On unix git terminates with a bare LF, so a CR before it belongs to the
    /// directory name. Stripping it pointed the repository root at a sibling
    /// directory, and the selection came out empty — a clean exit over a
    /// drifting file.
    #[cfg(unix)]
    #[test]
    fn a_carriage_return_before_the_terminator_is_part_of_the_name() {
        assert_eq!(trim_line_terminator(b"/repo\r\n"), b"/repo\r");
    }

    /// The wiring, not just the leaf: this is the step that decodes what git
    /// reported, and on APFS no end-to-end test can reach it.
    #[cfg(unix)]
    /// An empty entry would join to the repository root and select the whole
    /// tree's root directory.
    /// Where filenames are Unicode, output prim cannot decode is output it
    /// cannot trust — FR-4.2e requires a usage error rather than a dropped
    /// path, and this is the decode that decides it.
    #[test]
    fn a_platform_with_unicode_paths_refuses_undecodable_bytes() {
        assert_eq!(
            utf8_path_from_bytes(b"plain.txt").as_deref(),
            Some(Path::new("plain.txt"))
        );
        assert!(utf8_path_from_bytes(b"caf\xe9.txt").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_repository_root_keeps_its_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let root = repo_root_from_bytes(b"/tmp/caf\xe9\n").expect("unix paths decode");

        assert_eq!(root.as_os_str().as_bytes(), b"/tmp/caf\xe9");
    }

    /// `git ls-files -v -z` records are `<tag><space><path>`, NUL-terminated.
    /// `S` is skip-worktree, which is how sparse checkout is implemented, and
    /// a lowercase tag is assume-unchanged. `H` is materialised and must not
    /// `S` is skip-worktree, a lowercase tag is assume-unchanged, and `H` is
    /// materialised. Every record is tracked either way.
    #[test]
    fn the_index_answer_separates_tracked_from_not_materialised() {
        let index = index_entries(b"S drop/b.txt\0H keep/a.txt\0h assumed.txt\0");

        assert!(index.not_materialised.contains(Path::new("drop/b.txt")));
        assert!(index.not_materialised.contains(Path::new("assumed.txt")));
        assert!(!index.not_materialised.contains(Path::new("keep/a.txt")));
        assert_eq!(index.not_materialised.len(), 2);
        assert_eq!(index.tracked.len(), 3);
    }

    /// The status is what lets the caller tell a deletion from every other
    /// absence, so it has to survive parsing.
    #[test]
    fn a_diff_record_carries_its_status_and_its_path() {
        let entries = diff_entries(b"M\0kept.md\0D\0gone.md\0").expect("well-formed");

        assert_eq!(entries.len(), 2);
        assert!(!entries[0].is_deletion);
        assert_eq!(entries[0].path, PathBuf::from("kept.md"));
        assert!(entries[1].is_deletion);
        assert_eq!(entries[1].path, PathBuf::from("gone.md"));
    }

    #[test]
    fn a_diff_record_missing_its_path_is_refused() {
        assert!(diff_entries(b"M\0kept.md\0D\0").is_err());
        assert!(diff_entries(b"").expect("no records").is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_diff_record_keeps_a_path_that_is_not_valid_utf8() {
        use std::os::unix::ffi::OsStrExt;

        let entries = diff_entries(b"M\0caf\xe9.txt\0").expect("unix paths decode");

        assert_eq!(entries[0].path.as_os_str().as_bytes(), b"caf\xe9.txt");
    }

    #[test]
    fn a_malformed_ls_files_record_is_ignored_rather_than_trusted() {
        assert!(index_entries(b"").tracked.is_empty());
        assert!(index_entries(b"S\0").tracked.is_empty(), "no separator");
        assert!(
            index_entries(b"SX path\0").tracked.is_empty(),
            "wrong separator"
        );
    }
}
