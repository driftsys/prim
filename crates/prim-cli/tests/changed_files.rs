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

/// #164: git C-quotes non-ASCII paths when `core.quotePath` is on, which is
/// the default and which this test pins explicitly. A line-based reader then
/// joins a quoted literal onto the repo root, fails to canonicalize it, and
/// drops the path, so the gate passes over a file it never examined.
#[test]
fn a_non_ascii_path_survives_changed_file_selection() {
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let repo = init_repo();
        write(&repo.path().join("café.txt"), "x\n");
        commit_all(repo.path(), "baseline");

        write(&repo.path().join("café.txt"), "x  \n");
        git(repo.path(), &["add", "."]);
        // Pin the default rather than inherit it: with core.quotePath false
        // this test would pass without ever reproducing #164.
        git(repo.path(), &["config", "core.quotePath", "true"]);

        let mut command = prim();
        command.current_dir(repo.path()).arg("fmt").arg("--check");
        command
            .args(scope)
            .arg(".")
            .assert()
            .code(1)
            .stdout(predicates::str::contains("café.txt"));
    }
}

/// #165: `diff.relative=true` makes git print paths relative to the current
/// directory, but prim joins them onto the repository root. Run from a
/// subdirectory the whole selection empties and the gate passes.
#[test]
fn diff_relative_does_not_empty_the_selection_from_a_subdirectory() {
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let repo = init_repo();
        write(&repo.path().join("sub/a.txt"), "a\n");
        commit_all(repo.path(), "baseline");

        write(&repo.path().join("sub/a.txt"), "a  \n");
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["config", "diff.relative", "true"]);

        let mut command = prim();
        command
            .current_dir(repo.path().join("sub"))
            .arg("fmt")
            .arg("--check");
        command
            .args(scope)
            .arg(".")
            .assert()
            .code(1)
            .stdout(format!("{}\n", Path::new(".").join("a.txt").display()));
    }
}

/// `core.quotePath` governs only the non-ASCII range; git C-quotes a control
/// character whatever that setting says. So a path holding a newline or a tab
/// was dropped for the same reason `café.txt` was — a quoted literal that does
/// not resolve — and it stays dropped for a repository that sets
/// `core.quotePath false` to work around #164. Reading raw NUL-separated paths
/// is what makes every quoting form moot, and that is what this pins.
#[cfg(unix)]
#[test]
fn a_path_git_would_quote_survives_changed_file_selection() {
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let repo = init_repo();
        // core.quotePath off, so this cannot pass by way of the #164 fix.
        git(repo.path(), &["config", "core.quotePath", "false"]);
        let quoted = ["we\nird.txt", "ta\tb.txt"].map(|name| repo.path().join(name));
        for path in &quoted {
            write(path, "x\n");
        }
        commit_all(repo.path(), "baseline");

        for path in &quoted {
            write(path, "x  \n");
        }
        git(repo.path(), &["add", "."]);

        let mut command = prim();
        command.current_dir(repo.path()).arg("fmt");
        command.args(scope).arg(".").assert().code(0);

        for path in &quoted {
            assert_eq!(
                std::fs::read_to_string(path).unwrap(),
                "x\n",
                "{scope:?}: {path:?} was selected and formatted"
            );
        }
    }
}

/// A `<REF>` is data. Without `--end-of-options` git reads one beginning with
/// `-` as its own option: `--since=--output=<path>` made git write the path
/// list into that file, truncating it, and hand prim an empty selection that
/// exited 0. Refs routinely come from a variable in CI.
#[test]
fn a_since_ref_cannot_smuggle_an_option_into_git() {
    let repo = init_repo();
    write(&repo.path().join("a.txt"), "x\n");
    commit_all(repo.path(), "baseline");

    let victim = repo.path().join("victim.txt");
    write(&victim, "IMPORTANT\n");

    prim()
        .current_dir(repo.path())
        .args([
            "fmt",
            "--check",
            &format!("--since=--output={}", victim.display()),
            ".",
        ])
        .assert()
        .code(2);

    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        "IMPORTANT\n",
        "the ref must never be parsed as one of git's own options"
    );

    // A hostile ref with no file to truncate: exit 2 is the only observable,
    // so this catches a fix that special-cased `--output=` alone.
    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since=--exit-code", "."])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--since"));
}

/// `--` is git's revision separator, not a revision. prim's own trailing `--`
/// would pair with it, leaving git zero revisions and a pathspec matching
/// nothing — an empty selection reported as a clean run. The `--end-of-options`
/// guard does not catch it, because `--` is not an option.
#[test]
fn a_since_ref_of_the_separator_is_a_usage_error() {
    let repo = init_repo();
    write(&repo.path().join("a.txt"), "x\n");
    commit_all(repo.path(), "baseline");
    write(&repo.path().join("a.txt"), "x  \n");
    git(repo.path(), &["add", "."]);

    // Control: the same fixture is a finding under a real ref.
    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "HEAD", "."])
        .assert()
        .code(1);

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since=--", "."])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--since"))
        .stdout(predicates::str::is_empty());
}

/// FR-4.2b requires exit `2` for an invalid `<REF>`. Without the trailing `--`
/// git reads a ref naming an existing file as a pathspec, so prim reported a
/// narrowed file set and exited `1` — a gate that silently examined less than
/// it was asked to.
#[test]
fn a_since_ref_naming_a_file_is_a_usage_error_not_a_pathspec() {
    let repo = init_repo();
    write(&repo.path().join("a.txt"), "x\n");
    commit_all(repo.path(), "baseline");
    write(&repo.path().join("a.txt"), "x  \n");

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "--since", "a.txt", "."])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--since"));
}

/// Put a `git` on PATH that ignores `-z` and answers with newline-separated
/// paths. Splitting that on NUL yields one bogus entry holding every path, so
/// the selection would empty and the gate would pass. prim must refuse
/// instead. Nothing else can reach this branch: real git always honours `-z`.
#[cfg(unix)]
fn repo_with_a_git_shim(diff_output: &str) -> (tempfile::TempDir, tempfile::TempDir) {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_repo();
    write(&repo.path().join("a.txt"), "x\n");
    commit_all(repo.path(), "baseline");
    write(&repo.path().join("a.txt"), "x  \n");
    git(repo.path(), &["add", "."]);

    let bin = tempfile::tempdir().unwrap();
    let shim = bin.path().join("git");
    std::fs::write(
        &shim,
        format!(
            "#!/bin/sh\n\
             for a in \"$@\"; do\n\
             \tif [ \"$a\" = rev-parse ]; then printf '%s\\n' '{root}'; exit 0; fi\n\
             done\n\
             printf '%s' '{out}'\n",
            root = repo.path().display(),
            out = diff_output
        ),
    )
    .unwrap();
    std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    (repo, bin)
}

#[cfg(unix)]
#[test]
fn git_output_that_is_not_nul_separated_is_a_usage_error() {
    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let (repo, bin) = repo_with_a_git_shim("a.txt\nb.txt\n");
        let path = format!(
            "{}:{}",
            bin.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );

        let mut command = prim();
        command
            .current_dir(repo.path())
            .env("PATH", &path)
            .arg("fmt")
            .arg("--check");
        command.args(scope).arg(".").assert().code(2).stderr(
            predicates::str::contains("NUL-separated").and(predicates::str::contains(scope[0])),
        );
    }
}

/// The guard's escape: no output at all is a legitimate empty selection, not a
/// broken git, so it must stay a clean exit rather than an error.
#[cfg(unix)]
#[test]
fn empty_git_output_stays_an_empty_selection() {
    let (repo, bin) = repo_with_a_git_shim("");
    let path = format!(
        "{}:{}",
        bin.path().display(),
        std::env::var("PATH").unwrap_or_default()
    );

    prim()
        .current_dir(repo.path())
        .env("PATH", &path)
        .args(["fmt", "--check", "--staged", "."])
        .assert()
        .code(0);
}

/// #168 end to end. A filename that is not valid UTF-8 is legal on Linux and
/// cannot exist on APFS or HFS+, so this runs only where it is reachable —
/// CI's ubuntu runner. Before the fix the path decoded to U+FFFD, resolved to
/// nothing, and left the gate reporting a clean run over a drifting file.
#[cfg(target_os = "linux")]
#[test]
fn a_path_that_is_not_valid_utf8_is_selected() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let repo = init_repo();
        let odd = repo.path().join(OsStr::from_bytes(b"caf\xe9.txt"));
        // A parsed format as well as an orphan: this one also resolves its
        // `.editorconfig` section through a glob match on a byte-string name.
        let odd_md = repo.path().join(OsStr::from_bytes(b"caf\xe9.md"));
        write(&odd, "x\n");
        write(&odd_md, "# Title\n");
        commit_all(repo.path(), "baseline");

        // Committed clean and left drifting without being staged or changed
        // against HEAD: it must not be selected. Without it this test passes
        // even if the changed-file filter stopped filtering.
        let unchanged = repo.path().join("unchanged.txt");
        write(&unchanged, "y  \n");
        commit_all(repo.path(), "drifting but unchanged");

        write(&odd, "x  \n");
        write(&odd_md, "# Title  \n");
        git(repo.path(), &["add", odd.to_str().unwrap_or(".")]);
        git(repo.path(), &["add", "."]);

        // `Path::display` is lossy, so a correct run names the file this way.
        // Asserting the name rather than "something was printed" separates
        // this from a run that selected some other file, or none.
        let mut command = prim();
        command.current_dir(repo.path()).arg("fmt").arg("--check");
        command.args(scope).arg(".").assert().code(1).stdout(
            predicates::str::contains("caf\u{FFFD}.txt")
                .and(predicates::str::contains("unchanged.txt").not()),
        );

        // Write mode: a dropped path meant the file silently kept its drift.
        let mut command = prim();
        command.current_dir(repo.path()).arg("fmt");
        command.args(scope).arg(".").assert().code(0);

        assert_eq!(
            std::fs::read_to_string(&odd).unwrap(),
            "x\n",
            "{scope:?}: the orphan file was formatted, not skipped"
        );
        assert_eq!(
            std::fs::read_to_string(&odd_md).unwrap(),
            "# Title\n",
            "{scope:?}: the Markdown file was formatted, not skipped"
        );
    }
}

/// The repository root is a path too, and `git rev-parse --show-toplevel`
/// appends exactly one newline to it. Stripping more than that terminator
/// mangles a root whose own name ends in one, and every reported path is
/// joined onto that root — so the whole selection is lost, not one entry.
/// Creatable on APFS, so unlike the byte-decode half this pins the call site
/// on the machines prim is developed on.
#[cfg(unix)]
#[test]
fn a_repository_root_ending_in_a_newline_still_resolves() {
    a_repository_root_named("weird\n");
}

/// A CR is not a terminator on unix, where git writes a bare LF. Stripping it
/// pointed the root at the sibling directory created below, so the selection
/// came out empty and the gate passed over a drifting file.
#[cfg(unix)]
#[test]
fn a_repository_root_ending_in_a_carriage_return_still_resolves() {
    a_repository_root_named("weird\r");
}

#[cfg(unix)]
fn a_repository_root_named(name: &str) {
    let parent = tempfile::tempdir().unwrap();
    // The sibling the truncated spelling would resolve to.
    std::fs::create_dir(parent.path().join("weird")).unwrap();
    let root = parent.path().join(name);
    std::fs::create_dir(&root).unwrap();

    git(&root, &["init"]);
    git(&root, &["config", "user.name", "Prim Test"]);
    git(&root, &["config", "user.email", "prim@example.com"]);
    write(&root.join("a.txt"), "x\n");
    commit_all(&root, "baseline");
    write(&root.join("a.txt"), "x  \n");
    git(&root, &["add", "."]);

    for scope in [["--since", "HEAD"].as_slice(), ["--staged"].as_slice()] {
        let mut command = prim();
        command.current_dir(&root).arg("fmt").arg("--check");
        command
            .args(scope)
            .arg(".")
            .assert()
            .code(1)
            .stdout(predicates::str::contains("a.txt"));
    }
}
