//! The committed escape hatch (FR-4.4, AD-0009): which `.primignore` files
//! apply to a path, and what they say about it.
//!
//! # The bound
//!
//! A `.primignore` governs the repository that holds it and nothing else. The
//! search for the files that apply therefore stops at a **bound**: the root of
//! the repository containing whatever prim was pointed at — a path named on the
//! command line, or the root of a directory walk. Outside a repository the
//! bound is the current working directory instead, so a stray `.primignore` in
//! a parent directory cannot reach the tree prim was pointed at (AD-0011).
//!
//! The bound is resolved once, from the path prim was given, and then applies
//! to every path considered under it. That is what makes a walk and a named
//! path agree: pointing prim at `outer` bounds the whole walk at `outer`, so a
//! nested checkout beneath it is still pruned by `outer`'s rules, while
//! pointing prim at the nested checkout bounds it at that checkout, so
//! `outer`'s rules no longer reach it.
//!
//! prim matches `.primignore` here rather than registering it with the `ignore`
//! crate's walker, because the walker's ancestor stack has no bound: it reads
//! ignore files from every directory up to the filesystem root.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use ignore::Match;
use ignore::gitignore::{Gitignore, GitignoreBuilder, Glob};

/// What the `.primignore` stack says about a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// A rule excludes it.
    Ignored,
    /// A `!` rule re-includes it. The payload is whether that rule names the
    /// file specifically enough to also override the built-in generated-file
    /// list — see [`whitelist_names_file`]. Ordinary (non-generated)
    /// `.primignore` behaviour does not consult the payload: either way, the
    /// path is not [`Verdict::Ignored`].
    Whitelisted(bool),
    /// No rule mentions it.
    Unmatched,
}

/// Where the `.primignore` search stops for anything prim is pointed at, as
/// described in the module documentation. Inclusive: a `.primignore` sitting at
/// the bound still applies.
///
/// The repository root is the nearest directory, at or above `path`, holding a
/// `.git` entry. That entry is a directory in an ordinary clone and a **file**
/// in a git worktree, so both are found by testing for existence rather than
/// for a directory — a worktree is the shape #110 was reported from.
///
/// The repository and the working directory are ordered, not alternatives.
/// Standing in a directory that its own repository ignores must not stop the
/// search before it reaches the `.primignore` that names it, or the escape
/// hatch stops protecting the files it was added for.
pub(crate) fn bound_for(path: &Path) -> Option<PathBuf> {
    // A file cannot hold a `.git` entry, and its own directory is where the
    // search starts, so resolve the bound from a directory either way.
    let absolute = normalized(path)?;
    let start = if absolute.is_dir() {
        absolute
    } else {
        absolute.parent()?.to_path_buf()
    };

    if let Some(repository) = start.ancestors().find(|dir| dir.join(".git").exists()) {
        return Some(repository.to_path_buf());
    }

    // No repository. Stop at the working directory when prim was pointed
    // somewhere beneath it, and otherwise at the pointed-at directory itself: a
    // bound the search can never reach is no bound at all, and would let it
    // climb out into an unrelated tree.
    match std::env::current_dir() {
        Ok(cwd) if start.starts_with(&cwd) => Some(cwd),
        _ => Some(start),
    }
}

/// `path`, made absolute and then normalized lexically: `.` components dropped
/// and `..` resolved against the component before it.
///
/// `std::path::absolute` leaves `..` in place on Unix. A `..` left in place
/// makes the directory it points out of a lexical ancestor of the result, so
/// `prim fmt ../sibling` would be governed by the `.primignore` of the
/// repository the caller happens to be standing in.
fn normalized(path: &Path) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in std::path::absolute(path).ok()?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other),
        }
    }
    Some(normalized)
}

/// Answers "does `.primignore` cover this path?" within one bound.
///
/// Matchers are cached because both callers ask about many paths in the same
/// directory: a hook hands prim its whole staged-file list at once, and a walk
/// yields a directory's entries together. The bound is part of the key, since
/// the same directory yields a different stack under a different bound.
#[derive(Default)]
pub(crate) struct PrimignoreCache {
    by_scope: BTreeMap<(Option<PathBuf>, PathBuf), Vec<Gitignore>>,
}

impl PrimignoreCache {
    /// The verdict on `path`, searching no higher than `bound`.
    pub(crate) fn verdict(&mut self, path: &Path, is_dir: bool, bound: Option<&Path>) -> Verdict {
        let Some(absolute) = normalized(path) else {
            return Verdict::Unmatched;
        };
        let Some(directory) = absolute.parent() else {
            return Verdict::Unmatched;
        };

        // A path that is its own bound has nothing above it that may apply, and
        // a directory's own `.primignore` cannot ignore that directory —
        // gitignore patterns are relative to the directory holding the file. So
        // the stack is empty. Only a directory can be its own bound: a file is
        // neither a repository root nor the working directory.
        if bound == Some(absolute.as_path()) {
            return Verdict::Unmatched;
        }

        let file_name = absolute.file_name().and_then(|name| name.to_str());
        let key = (bound.map(Path::to_path_buf), directory.to_path_buf());
        let matchers = self
            .by_scope
            .entry(key)
            .or_insert_with(|| matchers_between(directory, bound));

        // Nearest `.primignore` first, so a closer whitelist (`!name`) beats a
        // more distant ignore — the precedence gitignore semantics give a
        // nested ignore file.
        for matcher in matchers.iter() {
            match matcher.matched_path_or_any_parents(&absolute, is_dir) {
                Match::Ignore(_) => return Verdict::Ignored,
                Match::Whitelist(glob) => {
                    let specific = file_name.is_some_and(|name| whitelist_names_file(glob, name));
                    return Verdict::Whitelisted(specific);
                }
                Match::None => {}
            }
        }
        Verdict::Unmatched
    }
}

/// Every `.primignore` from `directory` up to `bound` inclusive, nearest first,
/// each rooted at the directory that holds it so anchored patterns
/// (`/CHANGELOG.md`, `fixtures/`) resolve the way gitignore syntax says.
///
/// A bound that is not an ancestor of `directory` — prim pointed at a tree
/// outside the working directory, with no repository anywhere above it — leaves
/// the search unbounded, which is the only case where a `.primignore` above the
/// working directory can still apply.
fn matchers_between(directory: &Path, bound: Option<&Path>) -> Vec<Gitignore> {
    let mut matchers = Vec::new();
    for ancestor in directory.ancestors() {
        let candidate = ancestor.join(".primignore");
        if candidate.is_file() {
            let mut builder = GitignoreBuilder::new(ancestor);
            if builder.add(&candidate).is_none()
                && let Ok(built) = builder.build()
            {
                matchers.push(built);
            }
        }
        if bound == Some(ancestor) {
            break;
        }
    }
    matchers
}

/// Whether a `!` rule's glob names `file_name` specifically enough to
/// override the built-in generated-file list (AD-0011 item 4): the pattern's
/// final path segment, after the leading `!`, must be a literal equal to
/// `file_name` — none of the glob metacharacters `*`, `?`, `[`, `{`. Mirrors
/// AD-0009's rule that a path must be *named*, not merely matched, to take
/// precedence.
///
/// So `!package-lock.json`, `!**/package-lock.json`, and
/// `!vendor/package-lock.json` all override; `!*.json` and `!*` — the latter
/// a no-op negation under gitignore semantics, since nothing precedes it to
/// re-include — do not, even though the `ignore` crate reports all four as
/// `Match::Whitelist`.
///
/// This narrowing applies only to the generated-list override. Ordinary
/// `.primignore` whitelisting of a non-generated file is unaffected: such a
/// path is simply not [`Verdict::Ignored`] regardless of specificity.
fn whitelist_names_file(glob: &Glob, file_name: &str) -> bool {
    let pattern = glob.original().strip_prefix('!').unwrap_or(glob.original());
    let segment = pattern.rsplit('/').next().unwrap_or(pattern);
    segment == file_name && !segment.contains(['*', '?', '[', '{'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    /// A repository whose `.primignore` names `fixtures/`, holding the fixture
    /// it protects.
    fn repository_ignoring_fixtures() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        write(&repo.path().join(".primignore"), "fixtures/\n");
        write(&repo.path().join("fixtures/golden.json"), "{\"a\" :1}\n");
        repo
    }

    #[test]
    fn the_bound_is_the_repository_root_not_the_directory_asked_about() {
        // These tests cannot move the working directory — the suite runs
        // in parallel from one process — so the ordering of the two bounds is
        // pinned at the CLI layer instead, by
        // `an_ignored_directory_named_from_inside_itself_is_still_skipped`.
        // What is checked here is the repository half on its own.
        let repo = repository_ignoring_fixtures();
        let fixtures = repo.path().join("fixtures");

        assert_eq!(
            bound_for(&fixtures).as_deref(),
            Some(repo.path()),
            "the bound is the repository root above the directory, not the \
             directory itself"
        );
    }

    #[test]
    fn an_ignored_directory_is_covered_by_the_primignore_naming_it() {
        // The bound resolved above must actually reach that `.primignore`.
        let repo = repository_ignoring_fixtures();
        let fixtures = repo.path().join("fixtures");
        let bound = bound_for(&fixtures);

        assert_eq!(
            PrimignoreCache::default().verdict(&fixtures, true, bound.as_deref()),
            Verdict::Ignored,
            "`fixtures/` in the repository's own `.primignore` must cover it"
        );
    }

    #[test]
    fn a_file_under_an_ignored_directory_is_covered_by_the_directorys_rule() {
        // `fixtures/` names the directory, so the match is found by walking the
        // file's parents rather than the file's own path.
        let repo = repository_ignoring_fixtures();
        let golden = repo.path().join("fixtures/golden.json");
        let bound = bound_for(&golden);

        assert_eq!(
            PrimignoreCache::default().verdict(&golden, false, bound.as_deref()),
            Verdict::Ignored,
            "a file under an ignored directory is ignored too"
        );
    }

    #[test]
    fn a_worktree_root_bounds_the_search_though_its_git_entry_is_a_file() {
        // A git worktree records its root with a `.git` *file* holding a
        // `gitdir:` pointer, not a directory. It is the shape #110 was reported
        // from, so it must bound the search exactly as a clone does.
        let outer = tempfile::tempdir().unwrap();
        fs::create_dir_all(outer.path().join(".git")).unwrap();
        write(&outer.path().join(".primignore"), "worktree/\n");
        let worktree = outer.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        write(
            &worktree.join(".git"),
            "gitdir: /elsewhere/.git/worktrees/w\n",
        );

        assert_eq!(
            bound_for(&worktree).as_deref(),
            Some(worktree.as_path()),
            "a `.git` file marks a repository root as much as a `.git` directory"
        );
        assert_eq!(
            PrimignoreCache::default().verdict(&worktree, true, bound_for(&worktree).as_deref()),
            Verdict::Unmatched,
            "the enclosing repository's `.primignore` must not reach a separate \
             checkout that happens to sit at that path"
        );
    }

    #[test]
    fn the_enclosing_repository_still_prunes_a_nested_checkout_it_names() {
        // Pointed at the outer repository, prim keeps the outer rules: the
        // bound is the outer root, so `worktree/` still covers the nested
        // checkout. Only pointing prim at the checkout itself changes the bound.
        let outer = tempfile::tempdir().unwrap();
        fs::create_dir_all(outer.path().join(".git")).unwrap();
        write(&outer.path().join(".primignore"), "worktree/\n");
        let worktree = outer.path().join("worktree");
        fs::create_dir_all(worktree.join(".git")).unwrap();

        assert_eq!(
            PrimignoreCache::default().verdict(&worktree, true, bound_for(outer.path()).as_deref()),
            Verdict::Ignored,
            "under the outer repository's bound, its own rules still apply"
        );
    }
}
