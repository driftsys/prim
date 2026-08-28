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
//!
//! # The re-inclusion rule
//!
//! Matching here rather than in the walker means this module owns a second rule
//! the walker would have applied for free. Under gitignore semantics a `!` rule
//! cannot re-include a path when a directory holding it is excluded; a walk
//! gets that by pruning the excluded directory and never descending, and a
//! named path has no walk to prune. So [`PrimignoreCache::verdict`] decides the
//! directories holding a path first, each against the stack that governs it,
//! and matches the rules naming the path only where every one of them survived.
//! That is also why the path itself is matched with `Gitignore::matched` rather
//! than `matched_path_or_any_parents`: the latter folds a path's parents into
//! its own match, at the wrong precedence and against the wrong stack (#114).

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
/// Answers are cached because both callers ask about many paths in the same
/// directory: a hook hands prim its whole staged-file list at once, and a walk
/// yields a directory's entries together. The cache is split by bound, since
/// the same directory yields a different stack under a different one — and
/// because a per-bound map is keyed by directory alone, which a `&Path` probes
/// without allocating. Selecting the scope still allocates one key per
/// question; the directory lookups inside it no longer do.
#[derive(Default)]
pub(crate) struct PrimignoreCache {
    scopes: BTreeMap<Option<PathBuf>, Scope>,
}

/// What is remembered for one bound.
#[derive(Default)]
struct Scope {
    /// The `.primignore` stack that governs a directory.
    matchers: BTreeMap<PathBuf, Vec<Gitignore>>,
    /// Whether a directory, or one above it, is excluded. Kept apart from the
    /// matchers because it is a property of the directory: every file in it
    /// shares one answer, and a walk asks about the same directory once per
    /// entry it holds.
    covered_directories: BTreeMap<PathBuf, bool>,
}

impl PrimignoreCache {
    /// The verdict on `path`, searching no higher than `bound`.
    ///
    /// An excluded directory takes everything under it with it: under gitignore
    /// semantics a `!` rule cannot re-include a file whose parent directory is
    /// excluded, however near the file that rule is written. The directories
    /// holding `path` are therefore decided first, and only a path whose every
    /// parent below the bound survived is matched against the rules that name
    /// it (#114). A walk gets the same answer by pruning the excluded directory
    /// before it descends, so both routes agree.
    pub(crate) fn verdict(&mut self, path: &Path, is_dir: bool, bound: Option<&Path>) -> Verdict {
        let Some(absolute) = normalized(path) else {
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

        let scope = self.scopes.entry(bound.map(Path::to_path_buf)).or_default();

        // Every directory holding `absolute` — each one below `bound`, and not
        // `absolute` itself — must survive before its own rules are consulted.
        if absolute
            .parent()
            .is_some_and(|directory| scope.is_covered(directory, bound))
        {
            return Verdict::Ignored;
        }
        scope.own_verdict(&absolute, is_dir, bound)
    }
}

impl Scope {
    /// Whether `directory` is excluded, or lies under a directory that is.
    ///
    /// Each directory is judged on its own, against the `.primignore` stack
    /// that governs it, exactly as the walk judges it before deciding whether
    /// to descend. The answer is remembered, so a walk pays for a directory
    /// once rather than once per file in it.
    fn is_covered(&mut self, directory: &Path, bound: Option<&Path>) -> bool {
        // The search stops at the bound, so nothing at or above it decides
        // what lies below.
        if bound == Some(directory) {
            return false;
        }
        if let Some(&known) = self.covered_directories.get(directory) {
            return known;
        }
        let Some(parent) = directory.parent() else {
            return false;
        };

        // The directory above first: an exclusion there covers this directory
        // whatever its own rules say, which is the rule being applied.
        let covered = self.is_covered(parent, bound)
            || self.own_verdict(directory, true, bound) == Verdict::Ignored;
        self.covered_directories
            .insert(directory.to_path_buf(), covered);
        covered
    }

    /// What the `.primignore` stack says about `absolute` itself, with no
    /// regard for the directories holding it. `absolute` must already be
    /// normalized, and must sit strictly below `bound`.
    fn own_verdict(&mut self, absolute: &Path, is_dir: bool, bound: Option<&Path>) -> Verdict {
        let Some(directory) = absolute.parent() else {
            return Verdict::Unmatched;
        };

        let file_name = absolute.file_name().and_then(|name| name.to_str());
        if !self.matchers.contains_key(directory) {
            self.matchers
                .insert(directory.to_path_buf(), matchers_between(directory, bound));
        }
        let matchers = &self.matchers[directory];

        // Nearest `.primignore` first, so a closer whitelist (`!name`) beats a
        // more distant ignore — the precedence gitignore semantics give a
        // nested ignore file.
        for matcher in matchers.iter() {
            // `matched`, not `matched_path_or_any_parents`: the directories
            // holding the path are decided one at a time by [`Scope::is_covered`],
            // where each gets the stack that governs it rather than being
            // folded into the file's own match.
            match matcher.matched(absolute, is_dir) {
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
mod tests;
