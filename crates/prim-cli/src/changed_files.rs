//! Git-backed changed-file selection for `--since` / `--staged`.

use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Which git-derived changed-file scope, if any, restricts discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ChangedFilesScope {
    /// No git-derived restriction: discover all matched files as usual.
    All,
    /// Limit to `git diff --name-only <ref>`.
    Since(String),
    /// Limit to `git diff --name-only --cached`.
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
}

impl ChangedFiles {
    /// Resolve the current git-derived changed-file filter, if any.
    pub(crate) fn resolve(scope: &ChangedFilesScope) -> Result<Self, Error> {
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
        let repo_root = repo_root.trim_end_matches(['\n', '\r']);
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
        if !output.is_empty() && !output.contains('\0') {
            return Err(Error::GitCommandFailed {
                flag,
                command: diff_command,
                detail: "git ignored -z: output is not NUL-separated".to_string(),
            });
        }

        let paths = output
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .filter_map(|relative| std::fs::canonicalize(repo_root.join(relative)).ok())
            .collect();

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
                "git diff --name-only <REF>",
                vec![
                    "-c",
                    "diff.relative=false",
                    "diff",
                    "--name-only",
                    "-z",
                    "--end-of-options",
                    reference.as_str(),
                    "--",
                ],
            )),
            Self::Staged => Some((
                "--staged",
                "git diff --name-only --cached",
                vec![
                    "-c",
                    "diff.relative=false",
                    "diff",
                    "--name-only",
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
        }
    }
}

fn run_git(
    cwd: &Path,
    flag: &'static str,
    command: &'static str,
    args: &[&str],
) -> Result<String, Error> {
    let output = git_command(cwd, args)
        .output()
        .map_err(|source| Error::GitUnavailable { flag, source })?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
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
    use super::ChangedFilesScope;

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
                &args[..5],
                &["-c", "diff.relative=false", "diff", "--name-only", "-z"],
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

        assert_eq!(&args[5..], &["--end-of-options", "HEAD", "--"]);
    }

    #[test]
    fn staged_takes_no_reference_and_so_needs_no_fence() {
        let (_, _, args) = ChangedFilesScope::Staged
            .git_diff_command()
            .expect("a git-backed scope");

        assert_eq!(&args[5..], &["--cached"]);
    }
}
