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
fn untracked_paths_are_invisible_until_they_reach_the_index() {
    let repo = init_repo();
    write(&repo.path().join("modified.txt"), "modified\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("modified.txt"), "modified  \n");
    git(repo.path(), &["add", "modified.txt"]);
    write(&repo.path().join("newcomer.txt"), "newcomer  \n");

    // `git diff` never reports an untracked path, so neither scope can select
    // one. docs/recipes.md warns readers that a changed-file gate therefore
    // misses a brand-new file an unfiltered run would report.
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check"])
            .args(scope)
            .assert()
            .code(1)
            .stdout(
                predicates::str::contains("modified.txt")
                    .and(predicates::str::contains("newcomer.txt").not()),
            )
            .stderr(predicates::str::is_empty());
    }

    // Control: without a scope prim does report it, so the assertions above
    // pin the filter rather than the file being undiscoverable to begin with.
    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("newcomer.txt"));

    git(repo.path(), &["add", "newcomer.txt"]);

    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check"])
            .args(scope)
            .assert()
            .code(1)
            .stdout(predicates::str::contains("newcomer.txt"));
    }
}

#[test]
fn a_changed_file_gate_passes_when_only_an_untracked_path_drifts() {
    let repo = init_repo();
    write(&repo.path().join("clean.txt"), "clean\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("newcomer.txt"), "newcomer  \n");

    // The outcome docs/recipes.md promises: the gate is clean even though the
    // tree is not, which is why the recipe warns about the gap at all.
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check"])
            .args(scope)
            .assert()
            .success()
            .stdout(predicates::str::is_empty())
            .stderr(predicates::str::is_empty());
    }

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("newcomer.txt"));
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

fn index_content(repo: &Path, path: &str) -> String {
    let output = git_command(repo, &["show", &format!(":{path}")])
        .output()
        .unwrap_or_else(|err| panic!("git show :{path} failed to start: {err}"));
    assert!(output.status.success(), "git show :{path} failed");
    String::from_utf8(output.stdout).expect("index blob is UTF-8")
}

/// One staged file holding trailing whitespace, committed clean first so that
/// `git diff --cached` reports it.
fn repo_with_one_drifting_staged_file() -> tempfile::TempDir {
    let repo = init_repo();
    write(&repo.path().join("staged.txt"), "staged\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("staged.txt"), "staged  \n");
    git(repo.path(), &["add", "staged.txt"]);
    repo
}

fn stderr_of(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stderr.clone()).expect("stderr is UTF-8")
}

/// Issue #159: `--staged` selects paths from the index, but `fmt` writes the
/// working tree. Re-staging is deliberately left to the hook runner (prim
/// cannot do it safely for a partially staged file), so the write mode has to
/// report what it wrote.
///
/// The whole line is asserted, including the `warning:` prefix `ui::warning`
/// adds. Anything appended to it — a staleness claim, a `git add` suggestion —
/// violates FR-4.2c, and a `contains` assertion would not see it.
#[test]
fn staged_write_reports_what_it_wrote_and_leaves_the_index_alone() {
    let repo = repo_with_one_drifting_staged_file();

    let assertion = prim()
        .current_dir(repo.path())
        .args(["fmt", "--staged", "."])
        .assert()
        .code(0)
        // Human output belongs on stderr; a `--format` pipeline reads stdout.
        .stdout(predicates::str::is_empty());

    assert_eq!(
        stderr_of(assertion),
        "warning: 1 file was formatted in the working tree, but --staged does \
         not update the index. Run git diff to see what is not staged.\n",
        "the whole warning, verbatim"
    );

    assert_eq!(
        std::fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
        "staged\n",
        "the working tree is formatted"
    );
    assert_eq!(
        index_content(repo.path(), "staged.txt"),
        "staged  \n",
        "the index is deliberately left alone"
    );
}

#[test]
fn staged_write_reports_exactly_one_warning_for_the_whole_run() {
    let repo = init_repo();
    write(&repo.path().join("one.txt"), "one\n");
    write(&repo.path().join("two.txt"), "two\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("one.txt"), "one  \n");
    write(&repo.path().join("two.txt"), "two  \n");
    git(repo.path(), &["add", "."]);

    let stderr = stderr_of(
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--staged", "."])
            .assert()
            .code(0),
    );

    assert!(
        stderr.contains("2 files were formatted in the working tree"),
        "plural phrasing, got: {stderr}"
    );
    // Count the prefix, not the sentence: an extra per-file warning line
    // repeats `warning:` without necessarily repeating the tail.
    assert_eq!(
        stderr.matches("warning:").count(),
        1,
        "one warning per run, not one per file, got: {stderr}"
    );
}

/// The count is files prim wrote, not files `--staged` selected: both files
/// here are staged, only one of them drifts.
#[test]
fn staged_write_counts_only_the_files_it_wrote() {
    let repo = init_repo();
    write(&repo.path().join("drifting.txt"), "drifting\n");
    write(&repo.path().join("clean.txt"), "clean\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("drifting.txt"), "drifting  \n");
    write(&repo.path().join("clean.txt"), "still clean\n");
    git(repo.path(), &["add", "."]);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--staged", "."])
        .assert()
        .code(0)
        .stderr(predicates::str::contains(
            "1 file was formatted in the working tree",
        ));
}

/// The case the whole design rests on. The index holds canonical content the
/// user staged; the working tree carries an extra unstaged edit that drifts.
/// prim formats the working tree, so the warning fires — but the index was
/// never stale, so the message must not say it was, and must not tell the user
/// to run `git add`, which would stage the remainder they kept out.
#[test]
fn staged_write_says_nothing_about_an_index_it_never_read() {
    let repo = init_repo();
    write(&repo.path().join("partial.txt"), "old\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("partial.txt"), "staged\n");
    git(repo.path(), &["add", "partial.txt"]);
    write(&repo.path().join("partial.txt"), "staged\nunstaged  \n");

    let stderr = stderr_of(
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--staged", "."])
            .assert()
            .code(0),
    );

    assert!(
        stderr.contains("1 file was formatted in the working tree"),
        "the warning fires on the write, got: {stderr}"
    );
    assert!(
        !stderr.contains("stale"),
        "prim never read the staged blob, so it cannot call it stale: {stderr}"
    );
    assert!(
        !stderr.contains("git add"),
        "git add here would stage the unstaged remainder: {stderr}"
    );

    assert_eq!(
        std::fs::read_to_string(repo.path().join("partial.txt")).unwrap(),
        "staged\nunstaged\n",
        "the working tree is formatted"
    );
    assert_eq!(
        index_content(repo.path(), "partial.txt"),
        "staged\n",
        "the staged blob is untouched, and was canonical all along"
    );
}

#[test]
fn staged_write_stays_silent_when_it_formatted_nothing() {
    let repo = init_repo();
    write(&repo.path().join("staged.txt"), "before\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("staged.txt"), "after\n");
    git(repo.path(), &["add", "staged.txt"]);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--staged", "."])
        .assert()
        .code(0)
        .stderr(predicates::str::is_empty());
}

#[test]
fn staged_check_never_warns_about_the_index() {
    let repo = repo_with_one_drifting_staged_file();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--staged", "."])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("does not update the index").not());
}

/// `fmt --diff` is a preview: it writes nothing, so there is no stale index to
/// report. The warning must be gated on the write actually happening, not on
/// the file having drifted.
#[test]
fn staged_diff_never_warns_about_the_index() {
    let repo = repo_with_one_drifting_staged_file();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--diff", "--staged", "."])
        .assert()
        .code(0)
        .stderr(predicates::str::contains("does not update the index").not())
        // Positive control: without this, an empty selection would satisfy
        // every other assertion in this test.
        .stdout(predicates::str::contains("staged.txt"));

    assert_eq!(
        std::fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
        "staged  \n",
        "--diff wrote nothing, so nothing can be re-staged"
    );
}

/// `fix --diff` gates on pending findings (AD-0007 §4) and so exits `1`, but
/// it writes no more than `fmt --diff` does.
#[test]
fn staged_fix_diff_never_warns_about_the_index() {
    let repo = repo_with_one_drifting_staged_file();

    prim()
        .current_dir(repo.path())
        .args(["fix", "--diff", "--staged", "."])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("does not update the index").not());

    assert_eq!(
        std::fs::read_to_string(repo.path().join("staged.txt")).unwrap(),
        "staged  \n",
        "--diff wrote nothing, so nothing can be re-staged"
    );
}

#[test]
fn staged_fix_warns_and_leaves_the_index_alone_like_staged_fmt() {
    let repo = repo_with_one_drifting_staged_file();

    prim()
        .current_dir(repo.path())
        .args(["fix", "--staged", "."])
        .assert()
        .code(0)
        .stderr(
            predicates::str::contains("1 file was formatted in the working tree").and(
                predicates::str::contains("Run git diff to see what is not staged"),
            ),
        );

    assert_eq!(
        index_content(repo.path(), "staged.txt"),
        "staged  \n",
        "fix does not re-stage either"
    );
}

/// `prim [PATH]...` is the permanent alias for `prim fmt [PATH]...`, so the
/// warning has to reach the form most hooks are written in.
#[test]
fn bare_alias_staged_write_warns_like_fmt_staged() {
    let repo = repo_with_one_drifting_staged_file();

    prim()
        .current_dir(repo.path())
        .args(["--staged", "."])
        .assert()
        .code(0)
        .stderr(predicates::str::contains(
            "Run git diff to see what is not staged",
        ));
}

/// The warning is scoped to `--staged`, the flag whose whole purpose is the
/// index. `--since` is a two-way diff and does report staged paths as well
/// (FR-4.2b), so it can leave the index holding unformatted content too, but
/// it never claims to
/// describe a pending commit — issue #159 is about the flag that does. This
/// pins the scoping: an unstaged-only `--since` write says nothing.
#[test]
fn since_write_never_warns_about_the_index() {
    let repo = init_repo();
    write(&repo.path().join("tracked.txt"), "tracked\n");
    commit_all(repo.path(), "baseline");

    write(&repo.path().join("tracked.txt"), "tracked  \n");

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--since", "HEAD", "."])
        .assert()
        .code(0)
        .stderr(predicates::str::contains("does not update the index").not());
}
