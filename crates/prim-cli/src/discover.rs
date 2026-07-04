//! File discovery (FR-4): turn CLI path arguments into the concrete set of
//! files to format.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

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

/// Collect the set of files to process.
///
/// With no `paths`, walks the current directory recursively. Explicit file
/// arguments are taken directly; explicit directories (and the cwd) are walked,
/// honoring `.gitignore`, `.ignore`, `.primignore`, and `--exclude` globs.
/// Results are sorted and de-duplicated; a path reached both explicitly and via
/// a walk is marked explicit.
/// Fails when an `--exclude` glob is malformed (FR-4.5): a typo'd filter must
/// be a usage error, not a silently ignored one.
pub fn collect(paths: &[PathBuf], excludes: &[String]) -> Result<Vec<Discovered>, ignore::Error> {
    validate_excludes(excludes)?;
    // BTreeMap keeps results sorted by path and de-duplicated; the bool is the
    // `explicit` flag, OR-ed so explicit provenance wins over a walk.
    let mut selected: BTreeMap<PathBuf, bool> = BTreeMap::new();

    if paths.is_empty() {
        walk_into(Path::new("."), excludes, &mut selected);
    } else {
        for path in paths {
            if path.is_dir() {
                walk_into(path, excludes, &mut selected);
            } else {
                // A file, or a non-existent path: include it as explicit and
                // let the caller surface any read error (FR-6 fail-safe).
                mark(&mut selected, path.clone(), true);
            }
        }
    }

    Ok(selected
        .into_iter()
        .map(|(path, explicit)| Discovered { path, explicit })
        .collect())
}

/// Reject malformed exclude globs up front; `walk_into` re-builds the same
/// set per walk root, which cannot fail after this check.
fn validate_excludes(excludes: &[String]) -> Result<(), ignore::Error> {
    let mut builder = OverrideBuilder::new(".");
    for glob in excludes {
        builder.add(&format!("!{glob}"))?;
    }
    builder.build()?;
    Ok(())
}

/// Walk `root` recursively, adding every regular file with walked provenance.
fn walk_into(root: &Path, excludes: &[String], selected: &mut BTreeMap<PathBuf, bool>) {
    let mut walker = WalkBuilder::new(root);
    walker
        // Honor .gitignore/.ignore even outside a git repo, without invoking
        // git (FR-4.2).
        .standard_filters(true)
        .require_git(false)
        // Include dotfiles so allowlisted ones (.gitignore, .editorconfig,
        // .env, …) are reachable; the VCS metadata directory is pruned below.
        .hidden(false)
        // The committed escape hatch (FR-4.4).
        .add_custom_ignore_filename(".primignore")
        .filter_entry(|entry| entry.file_name() != ".git");

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
            mark(selected, entry.into_path(), false);
        }
    }
}

/// Record `path`, OR-ing in its explicit provenance.
fn mark(selected: &mut BTreeMap<PathBuf, bool>, path: PathBuf, explicit: bool) {
    *selected.entry(path).or_insert(false) |= explicit;
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

        let found = collect(&[dir.path().to_path_buf()], &[]).unwrap();
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

        let found = collect(&[dir.path().to_path_buf()], &[]).unwrap();
        let got = names(&found);
        assert!(got.contains(&"kept.md".to_string()));
        assert!(!got.contains(&"ignored.md".to_string()));
    }

    #[test]
    fn respects_primignore() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".primignore"), "skip.json\n");
        write(&dir.path().join("skip.json"), "{}\n");
        write(&dir.path().join("keep.json"), "{}\n");

        let found = collect(&[dir.path().to_path_buf()], &[]).unwrap();
        let got = names(&found);
        assert!(got.contains(&"keep.json".to_string()));
        assert!(!got.contains(&"skip.json".to_string()));
    }

    #[test]
    fn respects_exclude_glob() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("keep.md"), "x\n");
        write(&dir.path().join("drop.log"), "x\n");

        let found = collect(&[dir.path().to_path_buf()], &["*.log".to_string()]).unwrap();
        let got = names(&found);
        assert!(got.contains(&"keep.md".to_string()));
        assert!(!got.contains(&"drop.log".to_string()));
    }

    #[test]
    fn explicit_file_is_marked_explicit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("named.toml");
        write(&file, "x = 1\n");

        let found = collect(std::slice::from_ref(&file), &[]).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].explicit);
        assert_eq!(found[0].path, file);
    }

    #[test]
    fn nonexistent_explicit_path_is_included_as_explicit() {
        let found = collect(&[PathBuf::from("/no/such/prim/fixture.md")], &[]).unwrap();
        assert_eq!(found.len(), 1);
        assert!(found[0].explicit);
    }

    #[test]
    fn includes_allowlisted_dotfiles() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join(".editorconfig"), "root = true\n");
        write(&dir.path().join("a.md"), "x\n");

        let found = collect(&[dir.path().to_path_buf()], &[]).unwrap();
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

        let found = collect(&[dir.path().to_path_buf()], &[]).unwrap();
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
    fn results_are_sorted_and_deduped_with_explicit_winning() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("a.md"), "a\n");
        write(&dir.path().join("b.md"), "b\n");
        let a = dir.path().join("a.md");

        // a.md reached both via the walk and named explicitly.
        let found = collect(&[dir.path().to_path_buf(), a.clone()], &[]).unwrap();

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
