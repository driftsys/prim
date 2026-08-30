//! Turning git's raw output into paths.
//!
//! Split out of [`super`] so the decoding is reachable from a unit test: a
//! filename that is not valid UTF-8 cannot be created on APFS or HFS+, so an
//! end-to-end test of one runs only on Linux, and the wiring would otherwise
//! be unguarded on the machines prim is developed on (#168).

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
fn path_from_bytes(bytes: &[u8]) -> Option<PathBuf> {
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

/// The paths in one `git diff --name-only -z` answer.
///
/// Separate from [`ChangedFiles::resolve`] so the decoding is reachable from a
/// unit test: a filename that is not valid UTF-8 cannot be created on APFS or
/// HFS+, so an end-to-end test of one only runs on Linux, and the wiring would
/// otherwise be unguarded on the machines this is developed on.
///
/// `Err` holds the offending entry, rendered lossily, when one cannot be
/// represented as a path on this platform — the user needs to know which file
/// to rename.
pub(super) fn relative_paths(output: &[u8]) -> Result<Vec<PathBuf>, String> {
    output
        // `-z` terminates every entry, so the split leaves a trailing empty
        // one. Joining that onto the repository root would put the root itself
        // into the selection.
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            path_from_bytes(entry).ok_or_else(|| String::from_utf8_lossy(entry).into_owned())
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::{
        path_from_bytes, relative_paths, repo_root_from_bytes, trim_line_terminator,
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
    #[test]
    fn every_diff_entry_keeps_its_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let paths = relative_paths(b"caf\xe9.txt\0plain.txt\0").expect("unix paths decode");

        assert_eq!(paths.len(), 2, "the trailing NUL must not add an entry");
        assert_eq!(paths[0].as_os_str().as_bytes(), b"caf\xe9.txt");
        assert_eq!(paths[1].as_os_str().as_bytes(), b"plain.txt");
    }

    /// An empty entry would join to the repository root and select the whole
    /// tree's root directory.
    #[test]
    fn empty_entries_never_become_paths() {
        assert!(relative_paths(b"").expect("no entries").is_empty());

        let paths = relative_paths(b"a.txt\0\0").expect("one entry");
        assert_eq!(paths, vec![PathBuf::from("a.txt")]);
    }

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
}
