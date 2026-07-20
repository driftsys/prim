use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

fn git(repo: &Path, args: &[&str]) {
    let output = git_command(repo, args)
        .output()
        .unwrap_or_else(|err| panic!("git {args:?} failed to start: {err}"));
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_command(repo: &Path, args: &[&str]) -> StdCommand {
    let mut command = StdCommand::new("git");
    command
        .current_dir(repo)
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

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init"]);
    git(dir.path(), &["config", "user.name", "Prim Test"]);
    git(dir.path(), &["config", "user.email", "prim@example.com"]);
    dir
}

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "."]);
    git(repo, &["commit", "-m", message]);
}

#[test]
fn since_limits_check_to_staged_and_unstaged_changes_against_the_ref() {
    let repo = init_repo();
    write(&repo.path().join("staged.txt"), "staged\n");
    write(&repo.path().join("unstaged.txt"), "unstaged\n");
    write(&repo.path().join("unchanged.txt"), "unchanged  \n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("staged.txt"), "staged  \n");
    write(&repo.path().join("unstaged.txt"), "unstaged  \n");
    git(repo.path(), &["add", "staged.txt"]);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("staged.txt")
                .and(predicates::str::contains("unstaged.txt"))
                .and(predicates::str::contains("unchanged.txt").not()),
        );
}

#[test]
fn staged_limits_check_to_index_changes_only() {
    let repo = init_repo();
    write(&repo.path().join("staged.txt"), "staged\n");
    write(&repo.path().join("unstaged.txt"), "unstaged\n");
    write(&repo.path().join("unchanged.txt"), "unchanged  \n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("staged.txt"), "staged  \n");
    write(&repo.path().join("unstaged.txt"), "unstaged  \n");
    git(repo.path(), &["add", "staged.txt"]);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--staged"])
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("staged.txt")
                .and(predicates::str::contains("unstaged.txt").not())
                .and(predicates::str::contains("unchanged.txt").not()),
        );
}

#[test]
fn since_resolves_git_root_paths_when_run_from_a_subdirectory() {
    let repo = init_repo();
    write(&repo.path().join("docs/guide.txt"), "guide\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("docs/guide.txt"), "guide  \n");

    prim()
        .current_dir(repo.path().join("docs"))
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("guide.txt"));
}

#[test]
fn changed_file_filters_intersect_with_path_arguments_and_excludes() {
    let repo = init_repo();
    write(&repo.path().join("docs/included.txt"), "included\n");
    write(&repo.path().join("docs/excluded.txt"), "excluded\n");
    write(&repo.path().join("notes/outside.txt"), "outside\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("docs/included.txt"), "included  \n");
    write(&repo.path().join("docs/excluded.txt"), "excluded  \n");
    write(&repo.path().join("notes/outside.txt"), "outside  \n");
    git(
        repo.path(),
        &[
            "add",
            "docs/included.txt",
            "docs/excluded.txt",
            "notes/outside.txt",
        ],
    );

    prim()
        .current_dir(repo.path())
        .args([
            "fmt",
            "--check",
            "--staged",
            "--exclude",
            "excluded.txt",
            "docs",
        ])
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("docs/included.txt")
                .and(predicates::str::contains("docs/excluded.txt").not())
                .and(predicates::str::contains("notes/outside.txt").not()),
        );
}

#[test]
fn changed_file_filters_compose_with_no_ignore() {
    let repo = init_repo();
    write(&repo.path().join(".gitignore"), "hidden.txt\n");
    write(&repo.path().join("hidden.txt"), "hidden\n");
    git(repo.path(), &["add", ".gitignore"]);
    git(repo.path(), &["add", "-f", "hidden.txt"]);
    git(repo.path(), &["commit", "-m", "baseline"]);

    write(&repo.path().join("hidden.txt"), "hidden  \n");
    git(repo.path(), &["add", "-f", "hidden.txt"]);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--staged"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    prim()
        .current_dir(repo.path())
        .args(["--no-ignore", "fmt", "--check", "--staged"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("hidden.txt"));
}

#[test]
fn since_also_composes_with_no_ignore() {
    let repo = init_repo();
    write(&repo.path().join(".gitignore"), "hidden.txt\n");
    write(&repo.path().join("hidden.txt"), "hidden\n");
    git(repo.path(), &["add", ".gitignore"]);
    git(repo.path(), &["add", "-f", "hidden.txt"]);
    git(repo.path(), &["commit", "-m", "baseline"]);

    write(&repo.path().join("hidden.txt"), "hidden  \n");

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    prim()
        .current_dir(repo.path())
        .args(["--no-ignore", "fmt", "--check", "--since", "HEAD"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("hidden.txt"));
}

#[test]
fn deleted_paths_reported_by_git_are_dropped_silently() {
    let repo = init_repo();
    write(&repo.path().join("deleted.txt"), "deleted\n");
    commit_all(repo.path(), "baseline");

    std::fs::remove_file(repo.path().join("deleted.txt")).unwrap();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::is_empty());
}

#[test]
fn changed_file_scopes_require_a_git_working_tree() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("doc.txt"), "doc  \n");

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("git").and(
            predicates::str::contains("working tree").or(predicates::str::contains("repository")),
        ));

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "--staged"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("git").and(
            predicates::str::contains("working tree").or(predicates::str::contains("repository")),
        ));
}

#[test]
fn bad_since_ref_is_a_usage_error() {
    let repo = init_repo();
    write(&repo.path().join("doc.txt"), "doc\n");
    commit_all(repo.path(), "baseline");

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "not-a-real-ref-xyz"])
        .assert()
        .code(2)
        .stderr(
            predicates::str::contains("--since")
                .and(predicates::str::contains("not-a-real-ref-xyz"))
                .and(predicates::str::contains("git")),
        );
}

#[test]
fn since_and_staged_conflict_at_the_clap_layer() {
    prim()
        .args(["fmt", "--check", "--since", "HEAD", "--staged"])
        .assert()
        .code(2)
        .stderr(
            predicates::str::contains("--since")
                .and(predicates::str::contains("--staged"))
                .and(predicates::str::contains("cannot be used")),
        );
}

#[test]
fn changed_file_queries_ignore_inherited_git_repo_env() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("doc.txt"), "doc  \n");

    let worktree_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let git_dir_output = git_command(worktree_root, &["rev-parse", "--git-dir"])
        .output()
        .unwrap();
    assert!(git_dir_output.status.success());
    let git_dir = String::from_utf8(git_dir_output.stdout).unwrap();

    prim()
        .current_dir(dir.path())
        .env("GIT_DIR", git_dir.trim())
        .env("GIT_WORK_TREE", worktree_root)
        .args(["fmt", "--check", "--since", "HEAD"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("git").and(
            predicates::str::contains("working tree").or(predicates::str::contains("repository")),
        ));
}
