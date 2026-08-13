//! File discovery (FR-4): turn CLI path arguments into the concrete set of
//! files to format.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use ignore::Match;
use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use ignore::overrides::OverrideBuilder;

use crate::changed_files::{self, ChangedFilesScope};

/// A file selected for processing, tagged with how it was reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Discovered {
    /// Path to the file (relative to the walk root or as named on the CLI).
    pub path: PathBuf,
    /// True when the path was named directly on the command line. Explicit
    /// files are processed strictly (read failures are reported as errors);
    /// walked files are processed leniently (unreadable files are skipped).
    pub explicit: bool,
}

/// Which ignore sources apply to a run. Two independent switches, because
/// `--no-ignore` is about someone else's ignore files and `--no-primignore` is
/// about prim's own.
#[derive(Debug, Clone, Copy)]
pub struct IgnoreSettings {
    /// `.gitignore`, the global gitignore, and `.git/info/exclude`.
    pub vcs: bool,
    /// `.primignore`, for walked *and* explicitly named paths alike (AD-0009).
    pub primignore: bool,
}

impl Default for IgnoreSettings {
    fn default() -> Self {
        Self {
            vcs: true,
            primignore: true,
        }
    }
}

/// Why a path named on the command line was dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IgnoreReason {
    /// A committed `.primignore` covers it (AD-0009).
    Primignore,
    /// The named tool generates it, so prim declines outright (AD-0011).
    Generated(&'static str),
}

/// A path prim was given and chose not to process, with the reason to report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ignored {
    pub path: PathBuf,
    pub reason: IgnoreReason,
}

/// The outcome of discovery: the files to process, plus the paths that were
/// named on the command line and dropped. Those are reported (FR-4.4a) so an
/// ignored path never fails silently.
#[derive(Debug, Default)]
pub struct Discovery {
    pub files: Vec<Discovered>,
    pub ignored: Vec<Ignored>,
}

#[derive(Debug)]
pub(crate) enum Error {
    Exclude(ignore::Error),
    ChangedFiles(changed_files::Error),
}

/// Collect the set of files to process.
///
/// With no `paths`, walks the current directory recursively. Explicit file
/// arguments are taken directly; explicit directories (and the cwd) are walked,
/// honoring `.ignore`, `.primignore`, `--exclude` globs, and (unless disabled)
/// git-family ignore files like `.gitignore` and `.git/info/exclude`.
/// Results are sorted and de-duplicated; a path reached both explicitly and via
/// a walk is marked explicit.
///
/// `.primignore` covers explicitly named paths too (AD-0009): naming a file
/// cannot make prim touch something walking to it would leave alone, so the
/// "left byte-for-byte unchanged" promise holds however prim is invoked. The
/// dropped paths come back in [`Discovery::ignored`] for the caller to report.
///
/// Fails when an `--exclude` glob is malformed (FR-4.5): a typo'd filter must
/// be a usage error, not a silently ignored one.
pub fn collect(
    paths: &[PathBuf],
    excludes: &[String],
    ignores: IgnoreSettings,
    changed_files_scope: &ChangedFilesScope,
) -> Result<Discovery, Error> {
    validate_excludes(excludes)?;
    let changed_files = changed_files::ChangedFiles::resolve(changed_files_scope)?;
    // BTreeMap keeps results sorted by path and de-duplicated; the bool is the
    // `explicit` flag, OR-ed so explicit provenance wins over a walk.
    let mut selected: BTreeMap<PathBuf, bool> = BTreeMap::new();
    let mut ignored = Vec::new();
    let mut primignore = PrimignoreCache::default();

    if paths.is_empty() {
        walk_into(
            Path::new("."),
            excludes,
            ignores,
            &changed_files,
            &mut primignore,
            &mut selected,
        );
    } else {
        for path in paths {
            let is_dir = path.is_dir();
            let verdict = if ignores.primignore {
                primignore.verdict(path, is_dir)
            } else {
                Verdict::Unmatched
            };

            if verdict == Verdict::Ignored {
                ignored.push(Ignored {
                    path: path.clone(),
                    reason: IgnoreReason::Primignore,
                });
                continue;
            }
            // The built-in list is the weakest layer: a committed `!name` overrides it,
            // and `--no-primignore` disables it along with everything else.
            if verdict != Verdict::Whitelisted
                && ignores.primignore
                && let Some(tool) = prim_fmt::generated_by(path)
            {
                ignored.push(Ignored {
                    path: path.clone(),
                    reason: IgnoreReason::Generated(tool),
                });
                continue;
            }

            if is_dir {
                walk_into(
                    path,
                    excludes,
                    ignores,
                    &changed_files,
                    &mut primignore,
                    &mut selected,
                );
            } else {
                // A file, or a non-existent path: include it as explicit and
                // let the caller surface any read error (FR-6 fail-safe).
                mark_if_changed(&mut selected, path.clone(), true, &changed_files);
            }
        }
    }

    Ok(Discovery {
        files: selected
            .into_iter()
            .map(|(path, explicit)| Discovered { path, explicit })
            .collect(),
        ignored,
    })
}

/// Answers "does `.primignore` cover this path?" for paths named on the command
/// line, which the directory walk never sees.
///
/// The walker reads `.primignore` files as it descends; a named path has no
/// descent, so the matchers are built by climbing from the path's own directory
/// to the filesystem root. Matchers are cached per directory because a hook
/// hands prim its whole staged-file list at once.
#[derive(Default)]
struct PrimignoreCache {
    by_directory: BTreeMap<PathBuf, Vec<Gitignore>>,
}

/// What the `.primignore` stack says about a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// A rule excludes it.
    Ignored,
    /// A `!` rule re-includes it, which also overrides the built-in list.
    Whitelisted,
    /// No rule mentions it.
    Unmatched,
}

impl PrimignoreCache {
    fn verdict(&mut self, path: &Path, is_dir: bool) -> Verdict {
        let Ok(absolute) = std::path::absolute(path) else {
            return Verdict::Unmatched;
        };
        let Some(directory) = absolute.parent() else {
            return Verdict::Unmatched;
        };

        let matchers = self
            .by_directory
            .entry(directory.to_path_buf())
            .or_insert_with(|| matchers_above(directory));

        // Nearest `.primignore` first, so a closer whitelist (`!name`) beats a
        // more distant ignore — the same precedence the walker applies.
        for matcher in matchers.iter() {
            match matcher.matched_path_or_any_parents(&absolute, is_dir) {
                Match::Ignore(_) => return Verdict::Ignored,
                Match::Whitelist(_) => return Verdict::Whitelisted,
                Match::None => {}
            }
        }
        Verdict::Unmatched
    }
}

/// Every `.primignore` from `directory` up to the filesystem root, nearest
/// first, each rooted at the directory that holds it so anchored patterns
/// (`/CHANGELOG.md`, `fixtures/`) resolve the way gitignore syntax says.
fn matchers_above(directory: &Path) -> Vec<Gitignore> {
    directory
        .ancestors()
        .filter_map(|ancestor| {
            let candidate = ancestor.join(".primignore");
            if !candidate.is_file() {
                return None;
            }
            let mut builder = GitignoreBuilder::new(ancestor);
            builder.add(&candidate).is_none().then_some(())?;
            builder.build().ok()
        })
        .collect()
}

/// Reject malformed exclude globs up front; `walk_into` re-builds the same
/// set per walk root, which cannot fail after this check.
fn validate_excludes(excludes: &[String]) -> Result<(), Error> {
    let mut builder = OverrideBuilder::new(".");
    for glob in excludes {
        // Validate the glob as the user typed it. `walk_into` later negates it
        // with a `!` prefix, but the marker does not affect glob parsing, so
        // validating the raw form catches the same errors while keeping the
        // user's exact input in any error message.
        builder.add(glob).map_err(Error::Exclude)?;
    }
    builder.build().map_err(Error::Exclude)?;
    Ok(())
}

/// Walk `root` recursively, adding every regular file with walked provenance.
fn walk_into(
    root: &Path,
    excludes: &[String],
    ignores: IgnoreSettings,
    changed_files: &changed_files::ChangedFiles,
    primignore: &mut PrimignoreCache,
    selected: &mut BTreeMap<PathBuf, bool>,
) {
    let mut walker = WalkBuilder::new(root);
    walker
        // Keep `.ignore` support and parent-directory matching on, but let
        // `--no-ignore` disable only the git-family ignore files.
        .standard_filters(false)
        .parents(true)
        .ignore(true)
        .git_ignore(ignores.vcs)
        .git_global(ignores.vcs)
        .git_exclude(ignores.vcs)
        .require_git(false)
        // Include dotfiles so allowlisted ones (.gitignore, .editorconfig,
        // .mailmap, …) are reachable; the VCS metadata directory is pruned below.
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git");

    // The committed escape hatch (FR-4.4).
    if ignores.primignore {
        walker.add_custom_ignore_filename(".primignore");
    }

    if !excludes.is_empty() {
        let mut overrides = OverrideBuilder::new(root);
        for glob in excludes {
            // In ignore's Override a leading `!` blacklists (ignores) the glob;
            // with no whitelist globs, everything else stays included.
            let _ = overrides.add(&format!("!{glob}"));
        }
        if let Ok(built) = overrides.build() {
            walker.overrides(built);
        }
    }

    for entry in walker.build().flatten() {
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            let path = entry.into_path();
            // `generated_by` is a cheap name comparison, so it short-circuits
            // before the matcher build in `verdict`.
            if ignores.primignore
                && prim_fmt::generated_by(&path).is_some()
                && primignore.verdict(&path, false) != Verdict::Whitelisted
            {
                continue; // Silent: filtering is what a walk is for.
            }
            mark_if_changed(selected, path, false, changed_files);
        }
    }
}

fn mark_if_changed(
    selected: &mut BTreeMap<PathBuf, bool>,
    path: PathBuf,
    explicit: bool,
    changed_files: &changed_files::ChangedFiles,
) {
    if changed_files.contains(&path) {
        mark(selected, path, explicit);
    }
}

/// Record `path`, OR-ing in its explicit provenance.
fn mark(selected: &mut BTreeMap<PathBuf, bool>, path: PathBuf, explicit: bool) {
    *selected.entry(path).or_insert(false) |= explicit;
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exclude(err) => write!(f, "--exclude: {err}"),
            Self::ChangedFiles(err) => err.fmt(f),
        }
    }
}

impl From<changed_files::Error> for Error {
    fn from(value: changed_files::Error) -> Self {
        Self::ChangedFiles(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn names(found: &[Discovered]) -> Vec<String> {
        found
            .iter()
            .map(|d| d.path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn walks_directory_recursively() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.md"), "a\n");
        write(&dir.path().join("sub/b.json"), "{}\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let mut got = names(&found);
        got.sort();
        assert_eq!(got, vec!["a.md", "b.json"]);
        assert!(
            found.iter().all(|d| !d.explicit),
            "walked files are not explicit"
        );
    }

    #[test]
    fn respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".gitignore"), "ignored.md\n");
        write(&dir.path().join("ignored.md"), "x\n");
        write(&dir.path().join("kept.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(got.contains(&"kept.md".to_string()));
        assert!(!got.contains(&"ignored.md".to_string()));
    }

    #[test]
    fn respects_git_info_exclude_by_default() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".git/info/exclude"), "ignored.md\n");
        write(&dir.path().join("ignored.md"), "x\n");
        write(&dir.path().join("kept.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(got.contains(&"kept.md".to_string()));
        assert!(!got.contains(&"ignored.md".to_string()));
    }

    #[test]
    fn no_ignore_reincludes_git_info_exclude_matches() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".git/info/exclude"), "ignored.md\n");
        write(&dir.path().join("ignored.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings {
                vcs: false,
                ..IgnoreSettings::default()
            },
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(got.contains(&"ignored.md".to_string()));
    }

    #[test]
    fn respects_primignore() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".primignore"), "skip.json\n");
        write(&dir.path().join("skip.json"), "{}\n");
        write(&dir.path().join("keep.json"), "{}\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(got.contains(&"keep.json".to_string()));
        assert!(!got.contains(&"skip.json".to_string()));
    }

    #[test]
    fn no_ignore_does_not_bypass_primignore() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".primignore"), "skip.json\n");
        write(&dir.path().join("skip.json"), "{}\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings {
                vcs: false,
                ..IgnoreSettings::default()
            },
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        assert!(!names(&found).contains(&"skip.json".to_string()));
    }

    #[test]
    fn respects_exclude_glob() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("keep.md"), "x\n");
        write(&dir.path().join("drop.log"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &["*.log".to_string()],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(got.contains(&"keep.md".to_string()));
        assert!(!got.contains(&"drop.log".to_string()));
    }

    #[test]
    fn explicit_file_is_marked_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("named.toml");
        write(&file, "x = 1\n");

        let found = collect(
            std::slice::from_ref(&file),
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        assert_eq!(found.len(), 1);
        assert!(found[0].explicit);
        assert_eq!(found[0].path, file);
    }

    #[test]
    fn nonexistent_explicit_path_is_included_as_explicit() {
        let found = collect(
            &[PathBuf::from("/no/such/prim/fixture.md")],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        assert_eq!(found.len(), 1);
        assert!(found[0].explicit);
    }

    #[test]
    fn includes_allowlisted_dotfiles() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".editorconfig"), "root = true\n");
        write(&dir.path().join("a.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let got = names(&found);
        assert!(
            got.contains(&".editorconfig".to_string()),
            "allowlisted dotfiles must be discovered, got {got:?}"
        );
    }

    #[test]
    fn prunes_dot_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".git/config"), "[core]\n");
        write(&dir.path().join("a.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let paths: Vec<String> = found
            .iter()
            .map(|d| d.path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(
            paths.iter().all(|p| !p.contains("/.git/")),
            "must not descend into .git/, got {paths:?}"
        );
    }

    #[test]
    fn no_ignore_still_prunes_dot_git_directory() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".git/info/exclude"), "ignored.md\n");
        write(&dir.path().join(".git/hooks/post-checkout.md"), "# hook\n");
        write(&dir.path().join("kept.md"), "x\n");

        let found = collect(
            &[dir.path().to_path_buf()],
            &[],
            IgnoreSettings {
                vcs: false,
                ..IgnoreSettings::default()
            },
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;
        let paths: Vec<String> = found
            .iter()
            .map(|d| d.path.to_string_lossy().replace('\\', "/"))
            .collect();
        assert!(paths.iter().all(|path| !path.contains("/.git/")));
        assert!(paths.iter().any(|path| path.ends_with("/kept.md")));
    }

    #[test]
    fn results_are_sorted_and_deduped_with_explicit_winning() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.md"), "a\n");
        write(&dir.path().join("b.md"), "b\n");
        let a = dir.path().join("a.md");

        // a.md reached both via the walk and named explicitly.
        let found = collect(
            &[dir.path().to_path_buf(), a.clone()],
            &[],
            IgnoreSettings::default(),
            &ChangedFilesScope::All,
        )
        .unwrap()
        .files;

        // De-duplicated: a.md appears once.
        assert_eq!(found.iter().filter(|d| d.path == a).count(), 1);
        // Explicit provenance wins for a.md.
        assert!(found.iter().find(|d| d.path == a).unwrap().explicit);
        // Sorted by path.
        let mut sorted = found.clone();
        sorted.sort_by(|x, y| x.path.cmp(&y.path));
        assert_eq!(found, sorted);
    }
}
