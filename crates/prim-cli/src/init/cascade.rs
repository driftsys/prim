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

/// What a directory currently inherits from `.editorconfig` files above it,
/// or the reason prim cannot say.
pub(super) enum Ancestry {
    /// Nothing above the directory sets anything — the ordinary case of
    /// running `prim init` at the top of a repository, where a warning would
    /// be pure noise.
    Nothing,
    /// The ancestors that set at least one key, and the keys they set.
    Inherits(Inheritance),
    /// An ancestor `.editorconfig` could not be parsed, so prim cannot say
    /// what `root = true` cuts this directory off from.
    Malformed { path: PathBuf, error: String },
}

/// What a directory currently inherits from `.editorconfig` files above it.
pub(super) struct Inheritance {
    /// The ancestor files that set at least one key in a section, in the
    /// order EditorConfig applies them: farthest ancestor first. A file that
    /// sets nothing in a section — one carrying only `root = true`, say — is
    /// left out, because cutting the walk off from it loses nothing to
    /// report. Keys written before any section header are left out too, for
    /// the same reason: `ec4rs` does not apply them, so prim never resolved
    /// them either.
    files: Vec<PathBuf>,
    /// Every key those files set, deduplicated and ordered so the message is
    /// stable between runs.
    keys: BTreeSet<String>,
}

/// What `dir` inherits from above it today.
///
/// The walk stops where EditorConfig's own does, so an ancestor already
/// marked `root = true` bounds the answer rather than appearing beyond it.
pub(super) fn from_ancestors(dir: &Path) -> Ancestry {
    // The probe need not exist; only its directory steers the walk. Matches
    // `editorconfig::build_cascade`.
    let probe = dir.join(EDITORCONFIG_NAME);
    // `ec4rs` absolutizes a relative probe against the working directory, so
    // the paths it reports back are absolute. Comparing them against a
    // relative `dir` would never match, and `dir`'s own `.editorconfig` —
    // the file prim is about to write — would be mistaken for an ancestor.
    // `prim init` with no PATH argument passes `.`, so that is the common
    // case, not an edge one.
    let own_dir = if dir.is_relative() {
        let Ok(cwd) = std::env::current_dir() else {
            return Ancestry::Nothing;
        };
        cwd.join(dir)
    } else {
        dir.to_path_buf()
    };
    // Not an unreadable ancestor: `ec4rs` skips a file it cannot open and
    // carries on, so one never reaches here. This is the walk failing to
    // start, which leaves prim with nothing to say about what it inherits.
    let Ok(config_files) = ConfigFiles::open(&probe, Option::<&Path>::None) else {
        return Ancestry::Nothing;
    };
    let mut files = Vec::new();
    let mut keys = BTreeSet::new();

    for mut file in config_files {
        let path = file.path.clone();
        if path.parent() == Some(own_dir.as_path()) {
            continue;
        }
        let mut sets_anything = false;
        for section in file.by_ref() {
            // A malformed ancestor cannot be summarized, so prim stops here
            // and names the file instead. `editorconfig::build_cascade`
            // reports it when prim next resolves a file, but `prim init`
            // never builds a resolver, so without this the reader hears
            // nothing at all during this command.
            let section = match section {
                Ok(section) => section,
                Err(error) => {
                    return Ancestry::Malformed {
                        path,
                        error: error.to_string(),
                    };
                }
            };
            for (key, _) in section.props().iter() {
                sets_anything = true;
                keys.insert(key.to_string());
            }
        }
        if sets_anything {
            files.push(path);
        }
    }

    if files.is_empty() {
        Ancestry::Nothing
    } else {
        Ancestry::Inherits(Inheritance { files, keys })
    }
}

/// The text `init` prints once it has written `root = true` in `dir`, or
/// `None` when there is nothing above `dir` worth reporting.
pub(super) fn severing_warning(dir: &Path, ancestry: &Ancestry) -> Option<String> {
    match ancestry {
        Ancestry::Nothing => None,
        Ancestry::Inherits(inherited) => Some(severed_cascade(dir, inherited)),
        Ancestry::Malformed { path, error } => Some(unreadable_ancestor(dir, path, error)),
    }
}

/// The text for an ancestor prim could not parse. It opens with the same
/// sentence `editorconfig::build_cascade` prints on the resolution path, so a
/// reader who has met one of them recognizes the other, and it says only what
/// prim can stand behind: prim cannot describe a cascade it could not read.
/// It still says the write happened, because another EditorConfig reader —
/// most are more tolerant than prim — may be applying that file today.
fn unreadable_ancestor(dir: &Path, path: &Path, error: &str) -> String {
    format!(
        "{}: ignoring malformed .editorconfig ({error}); using canonical style. prim wrote \
         root = true in {}, so files under that directory no longer inherit from this file, and \
         prim cannot say what that cuts off, because it could not read it. prim resolves this \
         file as absent either way, but other EditorConfig readers are more tolerant and may \
         still be applying it.",
        path.display(),
        dir.display()
    )
}

/// The text `init` prints when it is about to write `root = true` over
/// `inherited`. Names the files that become unreachable and the keys they
/// set, because "a cascade was severed" does not tell anybody what they lost.
///
/// The keys listed are everything those files set, not everything that
/// applied to any particular file in `dir` — a section whose glob matches
/// nothing here is still listed. Narrowing it would need a representative
/// path, and a directory has no single representative file. The wording says
/// the keys no longer reach the directory rather than that they stop
/// applying, because the second would claim more than this can check.
fn severed_cascade(dir: &Path, inherited: &Inheritance) -> String {
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
         directory no longer inherit from {files} — the keys set there ({keys}) no longer reach \
         this directory. Delete the root = true line to keep inheriting them.",
        dir.display()
    )
}

#[cfg(test)]
mod tests;
