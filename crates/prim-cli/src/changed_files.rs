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
}

impl ChangedFiles {
    /// Resolve the current git-derived changed-file filter, if any.
    pub(crate) fn resolve(scope: &ChangedFilesScope) -> Result<Self, Error> {
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
        let repo_root =
            std::fs::canonicalize(repo_root.trim()).map_err(|err| Error::GitCommandFailed {
                flag,
                command: "git rev-parse --show-toplevel",
                detail: err.to_string(),
            })?;
        let output = run_git(&current_dir, flag, diff_command, &diff_args)?;

        let paths = output
            .lines()
            .filter(|line| !line.is_empty())
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
    fn git_diff_command(&self) -> Option<(&'static str, &'static str, Vec<&str>)> {
        match self {
            Self::All => None,
            Self::Since(reference) => Some((
                "--since",
                "git diff --name-only <REF>",
                vec!["diff", "--name-only", reference.as_str()],
            )),
            Self::Staged => Some((
                "--staged",
                "git diff --name-only --cached",
                vec!["diff", "--name-only", "--cached"],
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
