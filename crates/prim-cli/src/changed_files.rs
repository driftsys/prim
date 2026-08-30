//! Git-backed changed-file selection for `--since` / `--staged`.

mod decode;

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use self::decode::{diff_entries, index_entries, repo_root_from_bytes, trim_line_terminator};

/// Which git-derived changed-file scope, if any, restricts discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChangedFilesScope {
    /// No git-derived restriction: discover all matched files as usual.
    All,
    /// Limit to `git diff --name-status <ref>`.
    Since(String),
    /// Limit to `git diff --name-status --cached`.
    Staged,
}

/// The resolved changed-file filter for one CLI invocation.
#[derive(Debug)]
pub(crate) struct ChangedFiles {
    current_dir: PathBuf,
    paths: Option<HashSet<PathBuf>>,
}

#[derive(Debug)]
pub(crate) enum Error {
    CurrentDirectory(std::io::Error),
    GitUnavailable {
        flag: &'static str,
        source: std::io::Error,
    },
    NotGitRepository {
        flag: &'static str,
        detail: String,
    },
    GitCommandFailed {
        flag: &'static str,
        command: &'static str,
        detail: String,
    },
    SeparatorAsReference {
        flag: &'static str,
    },
    UndecodablePath {
        flag: &'static str,
        command: &'static str,
        path: String,
    },
    UnresolvablePaths {
        flag: &'static str,
        paths: Vec<String>,
    },
    MisreadPaths {
        flag: &'static str,
        paths: Vec<String>,
    },
}

impl ChangedFiles {
    /// Resolve the current git-derived changed-file filter, if any.
    pub(crate) fn resolve(scope: &ChangedFilesScope, requested: &[PathBuf]) -> Result<Self, Error> {
        // `--` is git's revision/pathspec separator. Passed through, it would
        // pair with the trailing `--` in `git_diff_command`, leaving git zero
        // revisions and a pathspec matching nothing — an empty selection and a
        // clean exit, which is the silent pass FR-4.2f forbids.
        if let ChangedFilesScope::Since(reference) = scope
            && reference == "--"
        {
            return Err(Error::SeparatorAsReference { flag: "--since" });
        }

        let current_dir = std::env::current_dir().map_err(Error::CurrentDirectory)?;
        let Some((flag, diff_command, diff_args)) = scope.git_diff_command() else {
            return Ok(Self {
                current_dir,
                paths: None,
            });
        };

        let repo_root = run_git(
            &current_dir,
            flag,
            "git rev-parse --show-toplevel",
            &["rev-parse", "--show-toplevel"],
        )?;
        let repo_root = repo_root_from_bytes(&repo_root).ok_or_else(|| Error::UndecodablePath {
            flag,
            command: "git rev-parse --show-toplevel",
            path: String::from_utf8_lossy(trim_line_terminator(&repo_root)).into_owned(),
        })?;
        let repo_root =
            std::fs::canonicalize(repo_root).map_err(|err| Error::GitCommandFailed {
                flag,
                command: "git rev-parse --show-toplevel",
                detail: err.to_string(),
            })?;
        let output = run_git(&current_dir, flag, diff_command, &diff_args)?;

        // `-z` (see `git_diff_command`) separates entries with NUL, so split on
        // that rather than on lines: a path may legally contain a newline.
        //
        // If a `git` on PATH ignored `-z`, splitting on NUL would yield one
        // bogus entry holding every path joined by newlines, which would fail
        // to canonicalize and empty the selection — the silent pass this whole
        // requirement exists to prevent. Refuse instead.
        if !output.is_empty() && !output.contains(&0) {
            return Err(Error::GitCommandFailed {
                flag,
                command: diff_command,
                detail: "git ignored -z: output is not NUL-separated".to_string(),
            });
        }

        let entries = diff_entries(&output).map_err(|path| Error::UndecodablePath {
            flag,
            command: diff_command,
            path,
        })?;

        // Classify every reported path before excusing any. Asking git to hide
        // deletions instead (`--diff-filter=d`) made two mistakes at once: it
        // dropped a staged deletion of a file that still exists and drifts,
        // and it left every other absence indistinguishable from one (#169).
        let mut paths = HashSet::new();
        let mut absent = Vec::new();
        for entry in entries {
            let joined = repo_root.join(&entry.path);

            // A symlink git reported is a path prim does not own (AD-0016).
            // Resolving it would put its *target's* identity in the changed
            // set, so staging only the link would make prim format a file git
            // never staged — the same question answered two ways again.
            // Tested before `canonicalize`, which is what resolves it away.
            if crate::symlink::is_symlink(&joined) {
                continue;
            }
            // On disk is on disk. A staged deletion of a file still present —
            // `git rm --cached` — is a file prim must still format.
            if let Ok(canonical) = std::fs::canonicalize(&joined) {
                paths.insert(canonical);
                continue;
            }
            // Really gone, and git says so: the case FR-4.2b has always passed
            // over in silence.
            if entry.is_deletion {
                continue;
            }
            // Something is there that prim does not format — a dangling
            // symlink, a directory. Discovery admits only regular files, so it
            // has already declined this one.
            if std::fs::symlink_metadata(&joined).is_ok() {
                continue;
            }
            absent.push(entry.path);
        }

        if !absent.is_empty() {
            // Run from the repository root, and ask for root-relative names
            // over the whole tree: from a subdirectory `git ls-files` lists
            // only that subtree, in names relative to it, and the diff paths
            // are root-relative — the two key spaces would never meet.
            let index = run_git(
                &repo_root,
                flag,
                "git ls-files -v",
                &["ls-files", "-v", "--full-name", "-z", "--", ":/"],
            )?;
            let index = index_entries(&index);
            let requested_roots = requested_roots(requested, &current_dir);

            let mut misread = Vec::new();
            let mut unresolvable = Vec::new();
            for path in absent {
                // Absent on purpose: sparse checkout and skip-worktree both
                // tell git not to put the file there.
                if index.not_materialised.contains(&path) {
                    continue;
                }
                // git named something it does not track. No repository state
                // produces that, so prim misread git's output — the shape of
                // #164, #165 and #167. Reported whatever prim was pointed at,
                // because the fault is prim's, not the caller's.
                if !index.tracked.contains(&path) {
                    misread.push(path.display().to_string());
                } else if prim_fmt::classify(&path).is_some()
                    && is_requested(&repo_root.join(&path), &requested_roots)
                {
                    unresolvable.push(path.display().to_string());
                }
            }

            if !misread.is_empty() {
                return Err(Error::MisreadPaths {
                    flag,
                    paths: misread,
                });
            }
            if !unresolvable.is_empty() {
                return Err(Error::UnresolvablePaths {
                    flag,
                    paths: unresolvable,
                });
            }
        }

        Ok(Self {
            current_dir,
            paths: Some(paths),
        })
    }

    /// Report whether `path` survives the git-derived changed-file filter.
    pub(crate) fn contains(&self, path: &Path) -> bool {
        match &self.paths {
            None => true,
            Some(paths) => self
                .canonical_candidate(path)
                .is_some_and(|canonical| paths.contains(&canonical)),
        }
    }

    fn canonical_candidate(&self, path: &Path) -> Option<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.current_dir.join(path)
        };
        std::fs::canonicalize(absolute).ok()
    }
}

/// The paths prim was pointed at, made absolute and canonical.
///
/// A requested path may itself be the one that is missing, so canonicalizing
/// it outright would fail and silently judge it out of scope — the most
/// explicit invocation being the one that stayed quiet. Canonicalize the
/// nearest existing ancestor instead and re-attach the rest.
fn requested_roots(requested: &[PathBuf], current_dir: &Path) -> Vec<PathBuf> {
    requested
        .iter()
        .filter_map(|path| {
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                current_dir.join(path)
            };

            let mut tail = Vec::new();
            let mut probe = absolute.as_path();
            loop {
                if let Ok(canonical) = std::fs::canonicalize(probe) {
                    let mut root = canonical;
                    root.extend(tail.iter().rev());
                    return Some(root);
                }
                tail.push(probe.file_name()?.to_os_string());
                probe = probe.parent()?;
            }
        })
        .collect()
}

/// Whether `candidate` lies under one of the paths prim was pointed at. The
/// caller resolves "no path arguments" to `.` first, so the two spellings of
/// the same invocation cannot disagree (FR-4.4c).
fn is_requested(candidate: &Path, requested_roots: &[PathBuf]) -> bool {
    requested_roots
        .iter()
        .any(|root| candidate.starts_with(root))
}

impl ChangedFilesScope {
    /// The flag name, a human-readable command for error messages, and the
    /// argument vector prim actually runs.
    ///
    /// Three arguments are defences rather than part of the query, and each
    /// closes a gate that used to pass over files it never examined:
    ///
    /// - `-c diff.relative=false` (#165). The diff runs from the process
    ///   working directory but every path it reports is joined onto the
    ///   repository root, so a repository or user config setting
    ///   `diff.relative=true` made git print paths relative to the current
    ///   directory and emptied the selection from any subdirectory. The
    ///   `GIT_*` scrubbing in [`git_command`] does not cover this: those
    ///   variables pin which repository and index git reads, not how it prints
    ///   a path.
    /// - `--end-of-options` and a trailing `--`, both on the `--since` arm,
    ///   which is the only arm carrying user data. `<REF>` is data, and git
    ///   will otherwise reinterpret it two ways. Without `--end-of-options` a
    ///   ref beginning with `-` is read as one of git's own options:
    ///   `--since=--output=<path>` made git write the path list into that
    ///   file, truncating it, and hand prim an empty selection that exited
    ///   `0`. Without the trailing `--` a ref naming an existing file is read
    ///   as a pathspec, so `--since a.txt` silently narrowed the gate and
    ///   exited `1` where FR-4.2b requires `2`. Refs routinely come from a
    ///   variable (`prim fmt --since "$BASE_REF"`), so both are reachable.
    /// - `-z` (#164). Without it git C-quotes, so `café.txt` arrived as the
    ///   17-byte literal `"caf\303\251.txt"` — quotes and backslash escapes
    ///   included — and was dropped. `core.quotePath` governs only the
    ///   non-ASCII range and is on by default; a control character such as a
    ///   newline or a tab is quoted whatever that setting says, so turning it
    ///   off is not a workaround. `-z` emits raw NUL-separated paths, which
    ///   makes every quoting form moot.
    ///
    /// The human-readable string names the query, not these defences: a user
    /// comparing prim's report against their own `git diff` wants the query.
    /// The one place a defence does belong in an error is where it is the
    /// failure — see the `-z` guard in [`ChangedFiles::resolve`].
    fn git_diff_command(&self) -> Option<(&'static str, &'static str, Vec<&str>)> {
        match self {
            Self::All => None,
            Self::Since(reference) => Some((
                "--since",
                "git diff --name-status <REF>",
                vec![
                    "-c",
                    "diff.relative=false",
                    "diff",
                    "--name-status",
                    "--no-renames",
                    "-z",
                    "--end-of-options",
                    reference.as_str(),
                    "--",
                ],
            )),
            Self::Staged => Some((
                "--staged",
                "git diff --name-status --cached",
                vec![
                    "-c",
                    "diff.relative=false",
                    "diff",
                    "--name-status",
                    "--no-renames",
                    "-z",
                    "--cached",
                ],
            )),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDirectory(err) => {
                write!(f, "could not determine the current directory: {err}")
            }
            Self::GitUnavailable { flag, source }
                if source.kind() == std::io::ErrorKind::NotFound =>
            {
                write!(
                    f,
                    "{flag} requires the `git` executable, but it was not found"
                )
            }
            Self::GitUnavailable { flag, source } => {
                write!(f, "{flag}: could not run git: {source}")
            }
            Self::NotGitRepository { flag, detail } => {
                write!(f, "{flag} requires a git working tree: {detail}")
            }
            Self::GitCommandFailed {
                flag,
                command,
                detail,
            } => write!(f, "{flag}: {command} failed: {detail}"),
            Self::SeparatorAsReference { flag } => write!(
                f,
                "{flag} requires a revision, and `--` is git's revision separator, not one"
            ),
            Self::UndecodablePath {
                flag,
                command,
                path,
            } => write!(
                f,
                "{flag}: {command} reported a path this platform cannot represent, because it is not valid UTF-8: {path}"
            ),
            Self::MisreadPaths { flag, paths } => write!(
                f,
                "{flag}: prim read {} out of git's output that git does not track: {}. That is a defect in prim rather than a state of the repository — please report it.",
                if paths.len() == 1 {
                    "a path".to_string()
                } else {
                    format!("{} paths", paths.len())
                },
                paths.join(", ")
            ),
            Self::UnresolvablePaths { flag, paths } => write!(
                f,
                "{flag}: git reported {} that {} not on disk and that git will still put there, so prim could not examine {}: {}. Restore {}, or stage the removal as well.",
                if paths.len() == 1 {
                    "1 path".to_string()
                } else {
                    format!("{} paths", paths.len())
                },
                if paths.len() == 1 { "is" } else { "are" },
                if paths.len() == 1 { "it" } else { "them" },
                paths.join(", "),
                if paths.len() == 1 { "it" } else { "them" }
            ),
        }
    }
}

/// Git's raw stdout. Paths are bytes, so decoding is the caller's decision.
fn run_git(
    cwd: &Path,
    flag: &'static str,
    command: &'static str,
    args: &[&str],
) -> Result<Vec<u8>, Error> {
    let output = git_command(cwd, args)
        .output()
        .map_err(|source| Error::GitUnavailable { flag, source })?;
    if output.status.success() {
        return Ok(output.stdout);
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.contains("not a git repository") || detail.contains("must be run in a work tree") {
        Err(Error::NotGitRepository { flag, detail })
    } else {
        Err(Error::GitCommandFailed {
            flag,
            command,
            detail,
        })
    }
}

fn git_command(cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(cwd)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_PREFIX");
    command
}

#[cfg(test)]
mod tests {
    use super::{ChangedFilesScope, Error};

    /// The message has to name the file, or the user cannot act on it.
    #[test]
    fn an_undecodable_path_error_names_the_flag_the_command_and_the_path() {
        let rendered = Error::UndecodablePath {
            flag: "--staged",
            command: "git diff --name-status --cached",
            path: "caf\u{FFFD}.txt".to_string(),
        }
        .to_string();

        assert!(rendered.contains("--staged"), "{rendered}");
        assert!(
            rendered.contains("git diff --name-status --cached"),
            "{rendered}"
        );
        assert!(rendered.contains("caf\u{FFFD}.txt"), "{rendered}");
    }

    /// The message has to name the flag and every path, or the user cannot
    /// tell which invocation refused or what to look at.
    /// A name git does not track can only have come from prim, so the message
    /// has to say so rather than blame the repository.
    #[test]
    fn a_misread_paths_error_says_the_fault_is_prims() {
        let rendered = Error::MisreadPaths {
            flag: "--since",
            paths: vec!["ghost.txt".to_string()],
        }
        .to_string();

        assert!(rendered.contains("--since"), "{rendered}");
        assert!(rendered.contains("ghost.txt"), "{rendered}");
        assert!(rendered.contains("defect in prim"), "{rendered}");
    }

    #[test]
    fn an_unresolvable_paths_error_names_the_flag_and_every_path() {
        let rendered = Error::UnresolvablePaths {
            flag: "--staged",
            paths: vec!["one.txt".to_string(), "two.txt".to_string()],
        }
        .to_string();

        assert!(rendered.contains("--staged"), "{rendered}");
        assert!(rendered.contains("one.txt"), "{rendered}");
        assert!(rendered.contains("two.txt"), "{rendered}");
        assert!(rendered.contains("2 paths"), "{rendered}");
        assert!(
            rendered.contains("stage the removal"),
            "the remedy: {rendered}"
        );
    }

    /// `-c` is only honoured before the subcommand: after it, git reads `-c`
    /// as combined-diff. Nothing end-to-end pins that ordering.
    #[test]
    fn both_scopes_carry_the_config_pin_before_the_subcommand() {
        for scope in [
            ChangedFilesScope::Since("HEAD".to_string()),
            ChangedFilesScope::Staged,
        ] {
            let (_, _, args) = scope.git_diff_command().expect("a git-backed scope");
            assert_eq!(
                &args[..6],
                &[
                    "-c",
                    "diff.relative=false",
                    "diff",
                    "--name-status",
                    "--no-renames",
                    "-z"
                ],
                "{scope:?}"
            );
        }
    }

    /// A `<REF>` sits between `--end-of-options` and `--`, so git can read it
    /// neither as an option nor as a pathspec.
    #[test]
    fn a_reference_is_fenced_on_both_sides() {
        let scope = ChangedFilesScope::Since("HEAD".to_string());
        let (_, _, args) = scope.git_diff_command().expect("a git-backed scope");

        assert_eq!(&args[6..], &["--end-of-options", "HEAD", "--"]);
    }

    #[test]
    fn staged_takes_no_reference_and_so_needs_no_fence() {
        let (_, _, args) = ChangedFilesScope::Staged
            .git_diff_command()
            .expect("a git-backed scope");

        assert_eq!(&args[6..], &["--cached"]);
    }
}
