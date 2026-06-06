//! Atomic file writes (FR-6.4): write via a temp file in the target's own
//! directory, preserve its permission bits, then atomically rename into place.
//! A failure leaves the original file byte-for-byte unchanged.

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

/// Atomically replace `path` with `contents`.
pub fn atomic(path: &Path, contents: &str) -> io::Result<()> {
    // The temp file must share a filesystem with the target for the rename to
    // be atomic, so create it in the target's own directory.
    let dir = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };

    let mut tmp = NamedTempFile::new_in(dir)?;
    tmp.write_all(contents.as_bytes())?;
    tmp.flush()?;

    // Preserve the original file's permission bits, when it already exists.
    if let Ok(meta) = fs::metadata(path) {
        fs::set_permissions(tmp.path(), meta.permissions())?;
    }

    // Atomic rename over the target; on error the original is untouched.
    tmp.persist(path).map_err(|err| err.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_existing_file_contents() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "old\n").unwrap();

        atomic(&file, "new\n").unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "new\n");
    }

    #[test]
    fn creates_file_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("fresh.txt");

        atomic(&file, "hello\n").unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "hello\n");
    }

    #[test]
    fn leaves_no_temp_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "x\n").unwrap();

        atomic(&file, "y\n").unwrap();

        let entries = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(entries, 1, "only the target file should remain");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "x\n").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();

        atomic(&file, "y\n").unwrap();

        assert_eq!(fs::read_to_string(&file).unwrap(), "y\n");
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }
}
