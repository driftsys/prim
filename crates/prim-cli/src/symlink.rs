//! prim's symbolic-link policy (AD-0016).
//!
//! prim never writes **to** a symlink. Writes go through a temporary file and
//! a rename (FR-6.4), so writing to a link replaces it with a regular file and
//! leaves the bytes prim was asked to format exactly where they were — the
//! data loss reported as #166.
//!
//! A symlink is therefore a path type prim does not own, which is what FR-4.6
//! already says to do with one: report it, leave it byte-for-byte unchanged,
//! and leave the exit code alone. The rule lives here rather than at each call
//! site so the four routes that can reach a link — formatting, the
//! changed-file scopes, `prim init`, and `prim explain` — cannot drift apart
//! on either the test or the wording.
//!
//! A path that merely passes **through** a symlinked directory is not this
//! case and is deliberately not tested for: it ends at a regular file, so no
//! link is destroyed. AD-0016 records why refusing it would cost more than the
//! `.primignore` reach limit it would close.

use std::path::Path;

/// Whether `path` itself is a symbolic link.
///
/// Only the final component is examined. `symlink_metadata` does not follow
/// the link, which is the whole point: `Path::is_file` and `Path::exists` both
/// answer about the far end and so cannot tell a link from what it points at.
/// A path prim cannot stat at all is not a link as far as prim is concerned —
/// the route that reads it reports the real error.
pub(crate) fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink())
}

/// The one wording prim uses when it declines a symlink.
pub(crate) fn skipped(path: &Path) -> String {
    format!(
        "{}: is a symbolic link; skipped (name the file it points at to format it)",
        path.display()
    )
}
