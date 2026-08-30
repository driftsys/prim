//! Reporting for an `.editorconfig` that `ec4rs` passed over in silence.
//!
//! `ec4rs` skips a config file it cannot open and carries on climbing
//! (`ec4rs::ConfigFiles::open`: `if let Ok(file) = ConfigFile::open(...)`).
//! prim inherits that wherever it resolves a cascade, so an unreadable
//! ancestor changed which settings applied with nothing said: an ancestor at
//! mode `000` turned `max_line_length = 120` into `unset` and no command
//! mentioned it (#153).
//!
//! This is a diagnostic only. Resolution is unchanged — prim still resolves
//! such a file as absent, which is what `ec4rs` does and what every other
//! EditorConfig reader with the same permissions would do. What changes is
//! that prim now names the file instead of leaving the reader to guess.

use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use ec4rs::{ConfigFile, ParseError};

use crate::ui;

const EDITORCONFIG_NAME: &str = ".editorconfig";

/// Report every `.editorconfig` that `ec4rs` passed over while resolving a
/// file in `dir`, `dir`'s own included.
pub(crate) fn report_unopenable_in_cascade(dir: &Path) {
    report(dir, true);
}

/// Report every `.editorconfig` that `ec4rs` passed over *above* `dir`.
///
/// `prim init` owns `dir`'s own `.editorconfig` — it is the file being
/// written or merged, not one `dir` inherits from — so an unopenable file
/// there is that command's business rather than an ancestor to report. The
/// walk still starts at `dir`, because a readable `root = true` there bounds
/// what lies above it.
///
/// Returns every path it found, reported or already reported, so `prim init`
/// can also say what its own `root = true` just cut the directory off from.
pub(crate) fn report_unopenable_above(dir: &Path) -> Vec<PathBuf> {
    report(dir, false)
}

fn report(dir: &Path, include_own: bool) -> Vec<PathBuf> {
    let found = unopenable_in_ancestry(dir, include_own);
    let paths = found.iter().map(|(path, _)| path.clone()).collect();
    for (path, fault) in found {
        if is_first_report(&path) {
            // The two faults read very differently to whoever has to fix
            // them, and prim already spells the second one this way when it
            // meets it during section iteration. Only the tail is shared:
            // unlike that case, a file skipped here leaves the rest of the
            // cascade in force rather than dropping it to canonical style.
            let kind = match fault {
                Fault::Unreadable(_) => "unreadable",
                Fault::Malformed(_) => "malformed",
            };
            ui::warning(&format!(
                "{}: ignoring {kind} .editorconfig ({fault}); resolving as if it were absent",
                path.display()
            ));
        }
    }
    paths
}

/// Why `ec4rs` passed a file over: it could not read the bytes, or it read
/// them and they were not `.editorconfig`.
enum Fault {
    Unreadable(String),
    Malformed(String),
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(detail) | Self::Malformed(detail) => f.write_str(detail),
        }
    }
}

impl From<ParseError> for Fault {
    fn from(error: ParseError) -> Self {
        let detail = error.to_string();
        match error {
            // `ec4rs` reports a file that is not valid UTF-8 as an I/O error
            // (`ErrorKind::InvalidData`), but prim read those bytes fine —
            // they were not `.editorconfig`. Without this, the same fault got
            // called `unreadable` here and `malformed` from the section loop,
            // and the position of the bad bytes decided which.
            ParseError::Io(ref io) if io.kind() != ErrorKind::InvalidData => {
                Self::Unreadable(detail)
            }
            _ => Self::Malformed(detail),
        }
    }
}

/// Walk the ancestry `ec4rs` walks, collecting the `.editorconfig` files it
/// would have passed over.
///
/// This mirrors `ec4rs::ConfigFiles::open` deliberately: the same climb, the
/// same `ConfigFile::open`, and the same stop at the first `root = true` that
/// opens. Anything above such a file never reached resolution, so naming it
/// would be noise rather than a diagnosis.
fn unopenable_in_ancestry(dir: &Path, include_own: bool) -> Vec<(PathBuf, Fault)> {
    // `ec4rs` joins a relative path onto the working directory before it
    // climbs, so a relative `dir` must be absolutized the same way or this
    // would walk a different ancestry than the one that was resolved.
    let absolute = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        let Ok(cwd) = std::env::current_dir() else {
            // `build_cascade` already reports a working directory it cannot
            // determine; there is no second thing to say about it here.
            return Vec::new();
        };
        cwd.join(dir)
    };

    // Drop `.` components, so the reported path does not depend on how the
    // caller spelled it: `prim explain ./doc.md` and `prim explain doc.md`
    // resolve the same ancestry and must name it the same way.
    let absolute: PathBuf = absolute.components().collect();

    let mut found = Vec::new();
    let mut current = Some(absolute.as_path());
    let mut is_own = true;
    while let Some(directory) = current {
        let candidate = directory.join(EDITORCONFIG_NAME);
        match ConfigFile::open(&candidate) {
            // `ec4rs` stops at the first `root = true` it manages to read.
            Ok(file) => {
                if file.reader.is_root {
                    break;
                }
            }
            Err(error) => {
                // Absent is the ordinary case — every ancestor up to the
                // filesystem root is one — and silence is right for it.
                // Everything else is a file `ec4rs` passed over without being
                // able to say so.
                //
                // Testing the error rather than stat-ing the candidate is
                // both cheaper and wider: an ancestor *directory* prim cannot
                // search fails here with `EACCES` while a `stat` of the
                // candidate fails too, so the stat-based guard hid the very
                // case #153 is about, one level up. A dangling symlink comes
                // back `NotFound` and stays silent, which is right — there is
                // no config there to have applied.
                let is_absent = matches!(
                    &error,
                    ParseError::Io(io) if io.kind() == ErrorKind::NotFound
                );
                if (include_own || !is_own) && !is_absent {
                    found.push((candidate, error.into()));
                }
            }
        }
        is_own = false;
        current = directory.parent();
    }
    found
}

/// Whether this is the first time `path` has been reported in this run.
///
/// The cascade cache is per directory and `load_discovered` builds one
/// resolver per rayon thread, so a single unreadable ancestor is met once per
/// directory per thread. The reader needs to hear about it once.
fn is_first_report(path: &Path) -> bool {
    static REPORTED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

    REPORTED
        .get_or_init(|| Mutex::new(HashSet::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(path.to_path_buf())
}
