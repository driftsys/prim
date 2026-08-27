//! Whether `prim init`'s mandated `root = true` (FR-3.5) would cut this
//! directory off from an `.editorconfig` above it that it currently inherits
//! from.
//!
//! `root = true` stops EditorConfig's upward walk, and it does so for every
//! key and every file type, not just prim's own. In a nested directory that
//! silently drops whatever a parent configured, so `init` reports it.
//! Detection asks `ec4rs` for the same cascade prim resolves with, rather
//! than walking parent directories a second time.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ec4rs::ConfigFiles;

use super::EDITORCONFIG_NAME;

/// What a directory currently inherits from `.editorconfig` files above it.
pub(super) struct Inheritance {
    /// The ancestor files that set at least one key, nearest first. A file
    /// that sets nothing — one carrying only `root = true`, say — is left
    /// out: cutting the walk off from it loses nothing to report.
    files: Vec<PathBuf>,
    /// Every key those files set, deduplicated and ordered so the message is
    /// stable between runs.
    keys: BTreeSet<String>,
}

/// What `dir` inherits from above it today, or `None` when nothing up there
/// sets anything — the ordinary case of running `prim init` at the top of a
/// repository, where the warning would be pure noise.
///
/// The walk stops where EditorConfig's own does, so an ancestor already
/// marked `root = true` bounds the answer rather than appearing beyond it.
pub(super) fn from_ancestors(dir: &Path) -> Option<Inheritance> {
    // The probe need not exist; only its directory steers the walk. Matches
    // `editorconfig::build_cascade`.
    let probe = dir.join(EDITORCONFIG_NAME);
    let mut files = Vec::new();
    let mut keys = BTreeSet::new();

    for mut file in ConfigFiles::open(&probe, Option::<&Path>::None).ok()? {
        let path = file.path.clone();
        // `dir`'s own `.editorconfig` is the file prim is about to write, not
        // something it inherits.
        if path.parent() == Some(dir) {
            continue;
        }
        let mut sets_anything = false;
        for section in file.by_ref() {
            // A malformed ancestor is nothing this warning can speak for, and
            // `build_cascade` already reports it on the resolution path.
            let Ok(section) = section else { return None };
            for (key, _) in section.props().iter() {
                sets_anything = true;
                keys.insert(key.to_string());
            }
        }
        if sets_anything {
            files.push(path);
        }
    }

    (!files.is_empty()).then_some(Inheritance { files, keys })
}

/// The text `init` prints when it is about to write `root = true` over
/// `inherited`. Names the files that become unreachable and the keys they
/// set, because "a cascade was severed" does not tell anybody what they lost.
///
/// The keys listed are everything those files set, not everything that
/// applied to any particular file in `dir` — a section whose glob matches
/// nothing here is still listed. Narrowing it would need a representative
/// path, and a directory has no single representative file.
pub(super) fn severing_warning(dir: &Path, inherited: &Inheritance) -> String {
    let files = inherited
        .files
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let keys = inherited
        .keys
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}: prim wrote root = true, which EditorConfig requires here, so files under this \
         directory no longer inherit from {files} — the keys set there ({keys}) stop applying. \
         Delete the root = true line to keep inheriting them.",
        dir.display()
    )
}

#[cfg(test)]
mod tests;
