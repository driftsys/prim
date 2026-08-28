// Behavioural acceptance tests for recursive file discovery (FR-4), exercised
// against real temp directories. With the no-op formatter, discovery's
// observable effects are: directories/cwd get walked (no longer an error), and
// walked non-UTF-8 files are skipped rather than failing the run.

use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

#[test]
fn directory_argument_is_walked() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/a.md"), "# Hi\n").unwrap();

    prim().arg(dir.path()).assert().success();
}

#[test]
fn no_args_walks_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# Hi\n").unwrap();

    prim().current_dir(dir.path()).assert().success();
}

#[test]
fn a_walk_reports_neighbours_of_a_file_that_used_to_panic() {
    // Issue #115: a debug assertion in dprint-plugin-markdown panicked (exit
    // 101) and took the whole walk down, so the drifty files beside it were
    // never reported. `fmt --check` makes that observable: the two drifty
    // neighbours must be listed and the exit code must be 1, not 101.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "#   Hi\n").unwrap();
    std::fs::write(dir.path().join("b.md"), "#   There\n").unwrap();
    let bad_content = "a \u{2009}b\n";
    std::fs::write(dir.path().join("bad.md"), bad_content).unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("a.md").and(predicates::str::contains("b.md")));

    // The triggering file is already canonical, so it is not reported and not
    // rewritten.
    assert_eq!(
        std::fs::read(dir.path().join("bad.md")).unwrap(),
        bad_content.as_bytes()
    );
}

#[test]
fn walked_binary_is_skipped_not_errored() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("logo.bin"), [0xFFu8, 0xFE, 0x00, 0x01]).unwrap();
    std::fs::write(dir.path().join("ok.md"), "# Hi\n").unwrap();

    prim().arg(dir.path()).assert().success();
}

#[test]
fn explicit_non_owned_file_is_left_unchanged() {
    // A file prim does not own (here a binary) is skipped with a warning,
    // not an error, when named explicitly (FR-2.4).
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("logo.bin");
    std::fs::write(&bin, [0xFFu8, 0xFE, 0x00]).unwrap();

    prim().arg(&bin).assert().success();
}

#[test]
fn explicit_owned_file_that_is_not_utf8_errors() {
    // An owned file type (.json) that cannot be read as UTF-8 is reported as an
    // error when named explicitly (exit 2).
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("data.json");
    std::fs::write(&bad, [0xFFu8, 0xFE, 0x00]).unwrap();

    prim().arg(&bad).assert().code(2);
}

/// A repository shaped like the one `docs/recipes.md` describes: a generated
/// file and a byte-exact fixture directory, both protected by `.primignore`.
fn ignored_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(".primignore"), "/CHANGELOG.md\nfixtures/\n").unwrap();
    // Non-canonical on purpose: prim would rewrite both if it processed them.
    std::fs::write(dir.path().join("CHANGELOG.md"), "#  Changelog\n").unwrap();
    std::fs::create_dir_all(dir.path().join("fixtures")).unwrap();
    std::fs::write(dir.path().join("fixtures/golden.json"), "{\"a\" :1}\n").unwrap();
    std::fs::write(dir.path().join("kept.json"), "{\"a\" :1}\n").unwrap();
    dir
}

#[test]
fn explicit_path_matching_primignore_is_not_formatted() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "#  Changelog\n",
        "an ignored file named explicitly must stay byte-for-byte unchanged"
    );
}

#[test]
fn explicit_path_inside_an_ignored_directory_is_not_formatted() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "fixtures/golden.json"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("fixtures/golden.json")).unwrap(),
        "{\"a\" :1}\n",
        "the ignore covers a directory, so files under it are covered too"
    );
}

#[test]
fn skipping_an_explicit_path_is_reported_on_stderr() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "CHANGELOG.md"])
        .assert()
        .success()
        .stderr(predicates::str::contains(".primignore"));
}

#[test]
fn skipping_a_walked_path_is_silent() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("kept.json"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[test]
fn lint_and_fix_honor_primignore_on_an_explicit_path() {
    // `fix` writes, so skipping every named path is an ordinary no-op; `lint`
    // gates, so the same run examined nothing and exits 2 (FR-4.4c). Either
    // way the ignored file is left byte-for-byte unchanged.
    for (verb, code) in [("lint", 2), ("fix", 0)] {
        let dir = ignored_repo();

        prim()
            .current_dir(dir.path())
            .args([verb, "CHANGELOG.md"])
            .assert()
            .code(code)
            .stderr(predicates::str::contains(".primignore"));

        assert_eq!(
            std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
            "#  Changelog\n",
            "{verb} must leave an ignored file alone"
        );
    }
}

#[test]
fn no_primignore_processes_the_ignored_path_anyway() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["--no-primignore", "fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "# Changelog\n",
        "--no-primignore restores the explicit-path override"
    );
}

#[test]
fn explicit_paths_and_a_walk_agree_about_what_is_ignored() {
    // The contract in one sentence: naming a file cannot make prim do something
    // walking to it would not. The reported answer is the same empty one the
    // walk gives; the exit code says separately that every named path was
    // skipped, so the gate examined nothing (FR-4.4c).
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "CHANGELOG.md", "fixtures/golden.json"])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty());
}

#[test]
fn no_ignore_still_does_not_bypass_primignore_on_an_explicit_path() {
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["--no-ignore", "fmt", "CHANGELOG.md"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
        "#  Changelog\n",
        "--no-ignore covers VCS ignore files only"
    );
}

#[test]
fn malformed_exclude_glob_is_a_usage_error() {
    let dir = tempfile::tempdir().unwrap();
    prim()
        .current_dir(dir.path())
        .args(["--exclude", "{unclosed"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("--exclude"));
}

/// A repository carrying a generated lockfile with non-canonical content:
/// pnpm's single-quoted scalars and flow mappings, which prim would rewrite.
fn generated_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("pnpm-lock.yaml"),
        "lockfileVersion: '9.0'\npackages:\n  is-odd@3.0.1:\n    engines: {node: '>=4'}\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("package-lock.json"), "{\"name\" :\"x\"}\n").unwrap();
    // No trailing newline, on purpose: AD-0011 promises no hygiene applies to
    // a generated file, including a missing final newline.
    std::fs::write(dir.path().join("npm-shrinkwrap.json"), "{\"name\" :\"x\"}").unwrap();
    std::fs::write(dir.path().join("authored.json"), "{\"a\" :1}\n").unwrap();
    dir
}

#[test]
fn a_walk_skips_generated_files_silently() {
    let dir = generated_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("authored.json"))
        .stdout(predicates::str::contains("pnpm-lock.yaml").not())
        .stdout(predicates::str::contains("package-lock.json").not())
        .stdout(predicates::str::contains("npm-shrinkwrap.json").not())
        .stderr(predicates::str::contains("generated").not());
}

#[test]
fn a_generated_file_survives_formatting_byte_for_byte() {
    let dir = generated_repo();
    let lock = dir.path().join("pnpm-lock.yaml");
    let before = std::fs::read_to_string(&lock).unwrap();

    prim().current_dir(dir.path()).arg("fmt").assert().success();

    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        before,
        "pnpm's flow mappings and quoting must survive untouched"
    );
}

#[test]
fn a_generated_file_with_no_trailing_newline_survives_unchanged() {
    // AD-0011 promises no whitespace hygiene applies to a generated file,
    // including a missing final newline — unlike every other fixture in
    // `generated_repo()`, which ends with one.
    let dir = generated_repo();
    let lock = dir.path().join("npm-shrinkwrap.json");
    let before = std::fs::read_to_string(&lock).unwrap();
    assert!(
        !before.ends_with('\n'),
        "fixture must lack a trailing newline"
    );

    prim().current_dir(dir.path()).arg("fmt").assert().success();

    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        before,
        "whitespace hygiene must not add a trailing newline to a generated file"
    );
}

#[test]
fn an_explicit_generated_path_is_skipped_and_names_its_generator() {
    let dir = generated_repo();
    let lock = dir.path().join("package-lock.json");
    let before = std::fs::read_to_string(&lock).unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "package-lock.json"])
        .assert()
        .success()
        .stderr(predicates::str::contains("generated by npm"));

    assert_eq!(std::fs::read_to_string(&lock).unwrap(), before);
}

#[test]
fn a_primignore_outside_the_repository_does_not_reinclude_a_generated_file() {
    // A stray `.primignore` above the repository (for example one left in
    // $HOME) must not be able to disable the built-in generated-file list
    // for every repository beneath it.
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join(".primignore"), "!package-lock.json\n").unwrap();
    let repo = outer.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("package-lock.json"), "{\"a\":1}\n").unwrap();

    prim()
        .current_dir(&repo)
        .args(["fmt", "--check", "."])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

#[test]
fn a_nonexistent_generated_path_errors_and_exits_two() {
    // FR-4.6: an explicitly named path that does not exist is always a usage
    // error, even when its name matches the built-in generated-file list.
    let dir = tempfile::tempdir().unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "typo/package-lock.json"])
        .assert()
        .code(2);
}

#[test]
fn a_directory_named_like_a_generated_file_is_walked_not_skipped() {
    // AD-0009's invariant, restated by the module doc: naming a path must
    // never make prim skip what walking to it would process.
    // `Path::file_name()` matches a directory named `package-lock.json` too,
    // so the generated check must not fire for one.
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("dirlock/package-lock.json")).unwrap();
    std::fs::write(
        dir.path().join("dirlock/package-lock.json/inner.json"),
        "{\"a\" :1}\n",
    )
    .unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "dirlock/package-lock.json"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("inner.json"));
}

#[test]
fn no_primignore_processes_a_generated_file() {
    let dir = generated_repo();
    let lock = dir.path().join("package-lock.json");

    prim()
        .current_dir(dir.path())
        .args(["--no-primignore", "fmt", "package-lock.json"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "{ \"name\": \"x\" }\n",
        "--no-primignore disables the built-in list too"
    );
}

#[test]
fn a_primignore_negation_re_includes_a_generated_file() {
    let dir = generated_repo();
    std::fs::write(dir.path().join(".primignore"), "!package-lock.json\n").unwrap();
    let lock = dir.path().join("package-lock.json");

    prim()
        .current_dir(dir.path())
        .args(["fmt", "package-lock.json"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "{ \"name\": \"x\" }\n",
        "a committed negation beats the built-in list"
    );
}

#[test]
fn lint_also_skips_a_generated_file() {
    let dir = generated_repo();

    prim()
        .current_dir(dir.path())
        .args(["lint", "package-lock.json"])
        .assert()
        // The skip leaves the gate with nothing to report on (FR-4.4c).
        .code(2)
        .stderr(predicates::str::contains("generated by npm"));
}

#[test]
fn no_ignore_includes_git_info_exclude_matches_in_fmt_check() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".git/info")).unwrap();
    std::fs::write(dir.path().join(".git/info/exclude"), "hidden.json\n").unwrap();
    std::fs::write(dir.path().join("hidden.json"), "{\"a\":1}\n").unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    prim()
        .current_dir(dir.path())
        .args(["--no-ignore", "fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("hidden.json"));
}

#[test]
fn stdin_echoes_a_generated_file_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("package-lock.json");

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--stdin-filepath"])
        .arg(&target)
        .write_stdin("{\"name\" :\"x\"}\n")
        .assert()
        .success()
        .stdout("{\"name\" :\"x\"}\n");
}

#[test]
fn stdin_lint_reports_no_findings_for_a_generated_file() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("package-lock.json");

    prim()
        .current_dir(dir.path())
        .args(["lint", "--stdin-filepath"])
        .arg(&target)
        .write_stdin("{\"a\" :1}\n")
        .assert()
        .success()
        .stdout(predicates::str::is_empty());
}

/// A repository nested inside another, where the outer one's `.primignore`
/// names the directory the inner checkout sits in.
///
/// `marker` is what makes the inner directory a repository root: a `.git`
/// directory for an ordinary clone, a `.git` file for a git worktree.
/// `outer_pattern` is the enclosing rule, which decides whether the outer
/// repository would prune the checkout by its directory component or by
/// matching the files under it.
fn nested_repository(outer_pattern: &str, marker: fn(&std::path::Path)) -> tempfile::TempDir {
    let outer = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outer.path().join(".git")).unwrap();
    std::fs::write(outer.path().join(".primignore"), outer_pattern).unwrap();

    let inner = outer.path().join("build/inner");
    std::fs::create_dir_all(&inner).unwrap();
    marker(&inner);
    // Non-canonical on purpose: prim rewrites this as soon as it processes it.
    std::fs::write(inner.join("doc.md"), "#  Doc\n").unwrap();
    outer
}

fn git_directory(inner: &std::path::Path) {
    std::fs::create_dir_all(inner.join(".git")).unwrap();
}

fn git_worktree_file(inner: &std::path::Path) {
    std::fs::write(
        inner.join(".git"),
        "gitdir: /elsewhere/.git/worktrees/inner\n",
    )
    .unwrap();
}

#[test]
fn a_named_repository_root_is_not_covered_by_the_enclosing_primignore() {
    // Named from the enclosing repository, so the working directory cannot be
    // what decides this: only the `.git` entry at `build/inner` can (#110).
    let outer = nested_repository("build/\n", git_directory);

    prim()
        .current_dir(outer.path())
        .args(["fmt", "--check", "build/inner"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[test]
fn a_named_worktree_root_is_not_covered_by_the_enclosing_primignore() {
    // A git worktree marks its root with a `.git` *file* rather than a
    // directory. It is the case #110 was reported from: agent worktrees under
    // a directory the enclosing repository's `.primignore` names. Named from
    // the enclosing repository so only the marker can decide it.
    let outer = nested_repository("build/\n", git_worktree_file);

    prim()
        .current_dir(outer.path())
        .args(["fmt", "--check", "build/inner"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[test]
fn working_inside_a_nested_checkout_ignores_the_enclosing_primignore() {
    // The reported invocation: standing in the worktree and naming `.`.
    let outer = nested_repository("build/\n", git_worktree_file);

    prim()
        .current_dir(outer.path().join("build/inner"))
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[test]
fn a_walk_inside_a_nested_checkout_ignores_the_enclosing_primignore() {
    // `build/**` matches the files rather than the directory component, so the
    // enclosing rule reaches the walk instead of the named root. Bounding only
    // the named path left this route skipping every file and exiting 0 with no
    // warning at all.
    let outer = nested_repository("build/**\n", git_worktree_file);

    prim()
        .current_dir(outer.path().join("build/inner"))
        .args(["fmt", "--check"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"));
}

#[test]
fn pointing_prim_at_the_enclosing_repository_still_prunes_the_nested_checkout() {
    // The other direction, which the bound must not break: pointed at the outer
    // repository, prim keeps the outer rules, so the checkout beneath it is
    // still pruned.
    //
    // Both pattern forms, because only the second reaches the nested checkout.
    // `build/` prunes at the directory component, so the walk never descends
    // far enough to meet a second repository root; `build/**` matches the files
    // inside it, so it is the form that shows the bound stayed fixed at the
    // outer repository instead of being re-derived for each entry.
    for pattern in ["build/\n", "build/**\n"] {
        let outer = nested_repository(pattern, git_worktree_file);

        prim()
            .current_dir(outer.path())
            .args(["fmt", "--check", "."])
            .assert()
            .success()
            .stdout(predicates::str::is_empty());
    }
}

#[test]
fn a_walk_and_a_named_path_keep_their_own_bounds_in_one_invocation() {
    // Two bounds are live at once: the walk is bounded at the outer repository,
    // and the named path at the nested checkout it points into. Were the
    // matcher cache keyed by directory alone, whichever argument came first
    // would decide for both, and the enclosing rule would reach across into the
    // checkout — bug #110's shape again.
    for order in [
        ["fmt", "--check", ".", "build/inner/doc.md"],
        ["fmt", "--check", "build/inner/doc.md", "."],
    ] {
        let outer = nested_repository("**/doc.md\n", git_worktree_file);

        prim()
            .current_dir(outer.path())
            .args(order)
            .assert()
            .code(1)
            .stdout(predicates::str::contains("build/inner/doc.md"));
    }
}

/// A repository that excludes a directory and then tries to re-include one
/// file inside it — `nested` decides whether the `!` rule sits in the same
/// `.primignore` or in one beneath the excluded directory.
fn repository_negating_inside_an_ignored_directory(nested: bool) -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    std::fs::create_dir_all(repo.path().join("build")).unwrap();
    if nested {
        std::fs::write(repo.path().join(".primignore"), "build/\n").unwrap();
        std::fs::write(repo.path().join("build/.primignore"), "!keep.md\n").unwrap();
    } else {
        std::fs::write(repo.path().join(".primignore"), "build/\n!build/keep.md\n").unwrap();
    }
    // Non-canonical on purpose: prim would rewrite all three if it processed
    // them. `outside.md` sits beyond the exclusion, so a walk that reports
    // nothing at all is distinguishable from one that pruned `build/`.
    std::fs::write(repo.path().join("build/keep.md"), "#  Keep\n").unwrap();
    std::fs::write(repo.path().join("build/other.md"), "#  Other\n").unwrap();
    std::fs::write(repo.path().join("outside.md"), "#  Outside\n").unwrap();
    repo
}

#[test]
fn a_negation_does_not_re_include_a_file_under_an_ignored_directory() {
    // gitignore's rule, which prim applies itself rather than delegating to the
    // walker: a `!` rule cannot re-include a file whose parent directory is
    // excluded. It holds only because the walk tells the matcher whether each
    // entry is a directory, so `build/` covers the directory itself.
    for nested in [false, true] {
        let repo = repository_negating_inside_an_ignored_directory(nested);

        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check", "."])
            .assert()
            .code(1)
            .stdout(
                predicates::str::contains("outside.md")
                    .and(predicates::str::contains("keep.md").not()),
            );
    }
}

#[test]
fn naming_the_negated_file_gets_the_same_answer_as_the_walk() {
    // #114: the two routes, over one tree, in one test — which is the claim
    // AD-0009 makes and the shape that broke it. Matching a named path used to
    // stop at the nearest `.primignore`, so a negation written there — or
    // beside the exclusion — re-included a file the walk would never offer, and
    // `prim fmt build/keep.md` rewrote what `prim fmt .` left alone.
    for nested in [false, true] {
        let repo = repository_negating_inside_an_ignored_directory(nested);

        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check", "."])
            .assert()
            .code(1)
            .stdout(
                predicates::str::contains("outside.md")
                    .and(predicates::str::contains("keep.md").not()),
            );

        prim()
            .current_dir(repo.path())
            .args(["fmt", "build/keep.md"])
            .assert()
            .success()
            .stderr(predicates::str::contains("matched by .primignore"));

        // The gate form of the same question, which is how #114 reproduces.
        // Both routes report the file the same way — neither lists it — while
        // the exit code separates them: this run was pointed only at a skipped
        // path, so FR-4.4c applies.
        prim()
            .current_dir(repo.path())
            .args(["fmt", "--check", "build/keep.md"])
            .assert()
            .code(2)
            .stdout(predicates::str::is_empty());

        assert_eq!(
            std::fs::read_to_string(repo.path().join("build/keep.md")).unwrap(),
            "#  Keep\n",
            "a file under an excluded directory must stay byte-for-byte \
             unchanged however prim is pointed at it (nested = {nested})"
        );
    }
}

#[test]
fn a_path_beside_an_excluded_directory_is_still_reported() {
    // One invocation, two paths sharing the directory above the excluded one:
    // the answer about `a/b/` must not carry over to `a/`. This is the shape a
    // hook produces when it hands prim a whole staged list.
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    std::fs::write(repo.path().join(".primignore"), "a/b/\n").unwrap();
    std::fs::create_dir_all(repo.path().join("a/b")).unwrap();
    std::fs::write(repo.path().join("a/b/inner.json"), "{\"a\" :1}\n").unwrap();
    std::fs::write(repo.path().join("a/other.json"), "{\"a\" :1}\n").unwrap();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "a/b/inner.json", "a/other.json"])
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("a/other.json")
                .and(predicates::str::contains("inner.json").not()),
        )
        .stderr(predicates::str::contains("matched by .primignore"));
}

#[test]
fn a_generated_file_negated_under_an_ignored_directory_is_still_skipped() {
    // AD-0011 item 4's override rides on the `.primignore` stack rather than
    // sitting beside it, so the directory exclusion above the negation decides
    // the path before the built-in list is consulted.
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    std::fs::create_dir_all(repo.path().join("vendor")).unwrap();
    std::fs::write(
        repo.path().join(".primignore"),
        "vendor/\n!vendor/package-lock.json\n",
    )
    .unwrap();
    let lock = repo.path().join("vendor/package-lock.json");
    std::fs::write(&lock, "{\"a\" :1}\n").unwrap();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "vendor/package-lock.json"])
        .assert()
        .success()
        // The directory exclusion decides it, so this is not the "generated by
        // npm" skip — the two reasons must stay distinguishable.
        .stderr(
            predicates::str::contains("matched by .primignore")
                .and(predicates::str::contains("generated by").not()),
        );

    assert_eq!(
        std::fs::read_to_string(&lock).unwrap(),
        "{\"a\" :1}\n",
        "the exclusion above the negation keeps the file unformatted"
    );
}

#[test]
fn a_primignore_above_the_working_directory_does_not_cover_a_named_directory() {
    // Outside a repository the search stops at the working directory, so a
    // stray `.primignore` in a parent directory cannot reach the tree prim was
    // pointed at (AD-0011).
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join(".primignore"), "build/\n").unwrap();
    let inner = outer.path().join("build/inner");
    std::fs::create_dir_all(&inner).unwrap();
    std::fs::write(inner.join("doc.md"), "#  Doc\n").unwrap();

    prim()
        .current_dir(&inner)
        .args(["fmt", "--check", "."])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

/// A repository whose `.primignore` names `fixtures/`, holding the byte-exact
/// fixture that entry exists to protect — the shape `docs/recipes.md`
/// recommends, and prim's own.
fn repository_ignoring_fixtures() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    std::fs::write(repo.path().join(".primignore"), "fixtures/\n").unwrap();
    std::fs::create_dir_all(repo.path().join("fixtures")).unwrap();
    std::fs::write(repo.path().join("fixtures/golden.json"), "{\"a\" :1}\n").unwrap();
    repo
}

#[test]
fn naming_an_ignored_directory_in_its_own_repository_still_skips_it() {
    // AD-0009 point 4, for a directory rather than a file.
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "--check", "fixtures"])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains(".primignore"));
}

#[test]
fn an_ignored_directory_named_from_inside_itself_is_still_skipped() {
    // The working directory is a bound only when there is no repository. Were
    // it an alternative rather than a fallback, standing in the fixtures
    // directory would stop the search short of the `.primignore` naming it.
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path().join("fixtures"))
        .args(["fmt", "--check", "."])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains(".primignore"));
}

#[test]
fn an_ignored_file_named_from_inside_its_own_directory_is_still_skipped() {
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path().join("fixtures"))
        .args(["fmt", "--check", "golden.json"])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains(".primignore"));
}

#[test]
fn a_walk_started_inside_an_ignored_directory_rewrites_nothing() {
    // The destructive form of the same gap: the walk root is the one entry the
    // `ignore` crate never offers to a filter, so a bare `prim fmt` here used
    // to rewrite the byte-exact corpus the `.primignore` entry protects.
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path().join("fixtures"))
        .arg("fmt")
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(repo.path().join("fixtures/golden.json")).unwrap(),
        "{\"a\" :1}\n",
        "a `.primignore`d fixture must stay byte-for-byte unchanged"
    );
}

#[test]
fn a_bound_the_search_cannot_reach_still_bounds_it() {
    // Outside a repository the bound is the working directory — but only when
    // prim was pointed somewhere beneath it. Pointed at a sibling tree, the
    // working directory is not an ancestor of anything being considered, so a
    // bound that is merely never reached would leave the search climbing to the
    // filesystem root and reading `.primignore` files belonging to neither
    // tree. That is the hazard AD-0011 records.
    let outer = tempfile::tempdir().unwrap();
    std::fs::write(outer.path().join(".primignore"), "docs/\n").unwrap();
    // The working directory is a repository; the tree prim is pointed at is not.
    let here = outer.path().join("here");
    std::fs::create_dir_all(here.join(".git")).unwrap();
    let sibling = outer.path().join("sibling/docs");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(sibling.join("x.json"), "{\"a\" :1}\n").unwrap();

    prim()
        .current_dir(&here)
        .args(["fmt", "--check"])
        .arg(outer.path().join("sibling"))
        .assert()
        .code(1)
        .stdout(predicates::str::contains("x.json"));
}

#[test]
fn a_parent_directory_component_does_not_hand_a_sibling_tree_the_repositorys_rules() {
    // `..` is not normalized away by `std::path::absolute` on Unix, so
    // `../sibling` keeps the repository as a lexical ancestor. Left alone, the
    // repository's `.primignore` would govern a tree outside it, and the same
    // tree would get opposite answers depending on how it was spelled.
    let outer = tempfile::tempdir().unwrap();
    let repo = outer.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(repo.join(".primignore"), "docs/\n").unwrap();
    let sibling = outer.path().join("sibling/docs");
    std::fs::create_dir_all(&sibling).unwrap();
    std::fs::write(sibling.join("x.json"), "{\"a\" :1}\n").unwrap();

    prim()
        .current_dir(&repo)
        .args(["fmt", "--check", "../sibling"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("x.json"));
}

/// Every mode whose exit code gates on pending findings (FR-5.2/5.3/5.3a/5.5).
/// These are the modes whose `0` asserts something about the paths prim was
/// given, so they are the ones that must not report success after examining
/// nothing (FR-4.4c).
const GATE_INVOCATIONS: [&[&str]; 5] = [
    &["fmt", "--check"],
    &["fmt", "--check-idempotence"],
    &["fix", "--check"],
    &["fix", "--diff"],
    &["lint"],
];

/// The modes that write, or only preview: doing nothing is the correct outcome
/// there, and a hook handed a staged list of ignored paths must still pass.
const NON_GATE_INVOCATIONS: [&[&str]; 3] = [&["fmt"], &["fix"], &["fmt", "--diff"]];

#[test]
fn a_gate_exits_two_when_every_named_path_was_skipped() {
    // #112: `0` from a gate reads as "I looked, and there is nothing to do".
    // With every named path skipped it would mean "I looked at nothing".
    for invocation in GATE_INVOCATIONS {
        let dir = ignored_repo();

        prim()
            .current_dir(dir.path())
            .args(invocation)
            .args(["CHANGELOG.md", "fixtures/golden.json"])
            .assert()
            .code(2)
            .stdout(predicates::str::is_empty())
            // Each skipped path is still named, as FR-4.4a requires; the
            // summary line is what carries the exit code.
            .stderr(
                predicates::str::contains("CHANGELOG.md")
                    .and(predicates::str::contains("golden.json"))
                    .and(predicates::str::contains("nothing was examined")),
            );
    }
}

#[test]
fn a_gate_exits_normally_when_one_named_path_was_processed() {
    // Only a run that examined nothing is an error. A hook handing prim a
    // staged list where one path is ignored still reports on the rest, which
    // is the normal shape and must keep its ordinary exit code — and must
    // still name the path it skipped.
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "CHANGELOG.md", "kept.json"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("kept.json"))
        .stderr(predicates::str::contains("CHANGELOG.md"));
}

#[test]
fn a_gate_exits_zero_when_a_surviving_path_yields_no_files() {
    // The rule is "every path was skipped", not "nothing was reported". A
    // directory prim looked into and found nothing in is a path that survived,
    // so the partial skip beside it must not raise the exit code (FR-4.4c).
    let dir = ignored_repo();
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "CHANGELOG.md", "empty"])
        .assert()
        .success()
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("CHANGELOG.md"));
}

#[test]
fn a_writing_mode_still_exits_zero_when_every_named_path_was_skipped() {
    // AD-0009 point 2's reason to exist: prim's own hooks pass staged paths,
    // and a commit that stages only ignored files must not be blocked.
    for invocation in NON_GATE_INVOCATIONS {
        let dir = ignored_repo();

        prim()
            .current_dir(dir.path())
            .args(invocation)
            .args(["CHANGELOG.md", "fixtures/golden.json"])
            .assert()
            .success()
            // Nothing to preview, and no gate error: the skip is the whole
            // outcome, and it is still reported.
            .stdout(predicates::str::is_empty())
            .stderr(
                predicates::str::contains("CHANGELOG.md")
                    .and(predicates::str::contains("nothing was examined").not()),
            );

        assert_eq!(
            std::fs::read_to_string(dir.path().join("CHANGELOG.md")).unwrap(),
            "#  Changelog\n",
            "{invocation:?} must leave the ignored file byte-for-byte unchanged"
        );
    }
}

#[test]
fn a_gate_exits_two_when_the_only_named_path_is_generated() {
    // The built-in generated-file list is the other route into examining
    // nothing (AD-0011), and it reaches the same exit code.
    let dir = generated_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "pnpm-lock.yaml"])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains("nothing was examined"));
}

#[test]
fn a_gate_pointed_only_at_unowned_paths_exits_zero() {
    // The sibling boundary of the rule: a path prim does not own is reported
    // under FR-4.6 and leaves the exit code alone. A changed-file list from a
    // Rust-only commit is the common case, and it must pass.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    for verb in [vec!["fmt", "--check"], vec!["lint"]] {
        prim()
            .current_dir(dir.path())
            .args(&verb)
            .arg("main.rs")
            .assert()
            .success()
            .stdout(predicates::str::is_empty())
            .stderr(
                predicates::str::contains("not a file type prim formats")
                    .and(predicates::str::contains("nothing was examined").not()),
            );
    }
}

#[test]
fn a_generated_path_beside_an_authored_one_does_not_raise_the_exit_code() {
    // The generated route's half of "skipping only some of the paths given
    // shall not raise the exit code": the lockfile is skipped, and the
    // authored file beside it still decides the run.
    let dir = generated_repo();

    prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "pnpm-lock.yaml", "authored.json"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("authored.json"))
        .stderr(predicates::str::contains("generated by pnpm"));
}

#[test]
fn no_primignore_reaches_the_file_a_gate_would_otherwise_skip() {
    // `--no-primignore` is the documented way out of the new exit `2`, so the
    // gate must reach the file and report on it rather than gating.
    let dir = ignored_repo();

    prim()
        .current_dir(dir.path())
        .args(["--no-primignore", "fmt", "--check", "CHANGELOG.md"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("CHANGELOG.md"));
}

#[test]
fn a_writing_mode_pointed_at_an_ignored_working_directory_still_reports_it() {
    // AD-0009 point 2 for the implicit root: naming nothing and getting
    // nothing back is the surprise the warning exists for, and the writing
    // modes report it without gating.
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path().join("fixtures"))
        .arg("fmt")
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));
}

#[test]
fn a_gate_pointed_at_an_ignored_working_directory_exits_two() {
    // With no path named, prim is pointed at the working directory. Skipping
    // that is the same event as skipping a named `.`, so it is reported and
    // gated the same way rather than passing as an empty walk.
    let repo = repository_ignoring_fixtures();

    prim()
        .current_dir(repo.path().join("fixtures"))
        .args(["fmt", "--check"])
        .assert()
        .code(2)
        .stdout(predicates::str::is_empty())
        .stderr(predicates::str::contains(".primignore"));
}

/// A tree outside any repository, reachable by two spellings: its own resolved
/// path, and a symlink pointing at it. Returns both, plus the temp dirs that
/// must outlive them.
#[cfg(unix)]
fn tree_reachable_two_ways() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let temp = tempfile::tempdir().unwrap();
    // No `.git` anywhere: this is the working-directory half of the bound.
    let real = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(real.join(".primignore"), "build/\n").unwrap();
    std::fs::create_dir_all(real.join("build/inner")).unwrap();
    // Non-canonical on purpose: prim would rewrite it if it processed it.
    std::fs::write(real.join("build/inner/doc.md"), "#  Doc\n").unwrap();

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    (real, link, temp, elsewhere)
}

#[cfg(unix)]
#[test]
fn a_symlinked_spelling_gets_the_same_answer_as_the_resolved_one() {
    // #113: outside a repository the bound is the working directory, which
    // `std::env::current_dir` reports with its symlinks resolved, while the
    // path prim was given kept its own. The prefix test never matched, so the
    // search stopped at the pointed-at directory, short of the rule protecting
    // the file, and the same file got two answers.
    let (real, link, _temp, _elsewhere) = tree_reachable_two_ways();

    for spelling in [&real, &link] {
        prim()
            .current_dir(&real)
            .args(["fmt", "--check"])
            .arg(spelling.join("build/inner/doc.md"))
            .assert()
            .code(2)
            .stdout(predicates::str::is_empty())
            .stderr(predicates::str::contains("matched by .primignore"));
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_spelling_does_not_rewrite_a_protected_file() {
    // The destructive form: the escape hatch has to hold however the path is
    // spelled, which is AD-0009's promise.
    let (real, link, _temp, _elsewhere) = tree_reachable_two_ways();

    prim()
        .current_dir(&real)
        .arg("fmt")
        .arg(link.join("build/inner/doc.md"))
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));

    assert_eq!(
        std::fs::read_to_string(real.join("build/inner/doc.md")).unwrap(),
        "#  Doc\n",
        "a `.primignore`d file must survive being named through a symlink"
    );
}

/// A repository holding a symlink to a tree outside it, nested one level down
/// so a `.primignore` can be planted *above* the repository — where an
/// unbounded lexical climb would actually reach. Returns the repository path,
/// the directory above it, and the outside tree — the last two own the temp
/// directories, which must outlive the first.
#[cfg(unix)]
fn repository_with_a_symlinked_directory()
-> (std::path::PathBuf, tempfile::TempDir, tempfile::TempDir) {
    let above = tempfile::tempdir().unwrap();
    let repo = above.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(outside.path().join("tree")).unwrap();
    // Non-canonical on purpose: prim would rewrite it if it processed it.
    std::fs::write(outside.path().join("tree/b.md"), "#  B\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("tree"), repo.join("link")).unwrap();
    (repo, above, outside)
}

#[cfg(unix)]
#[test]
fn a_rule_naming_a_symlinked_directory_covers_what_is_written_through_it() {
    // Matching is lexical, as git's is: a rule naming a symlinked directory
    // covers the paths spelled through it. Resolving the symlink away would
    // stop the rule matching and rewrite the file it protects — and `git`
    // declines to match through such a path at all rather than resolving it.
    let (repo, _above, outside) = repository_with_a_symlinked_directory();
    std::fs::write(repo.join(".primignore"), "link/\n").unwrap();

    prim()
        .current_dir(&repo)
        .args(["fmt", "link/b.md"])
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));

    assert_eq!(
        std::fs::read_to_string(outside.path().join("tree/b.md")).unwrap(),
        "#  B\n",
        "a rule naming the symlink must still protect what is under it"
    );
}

#[cfg(unix)]
#[test]
fn a_symlinked_directory_named_on_the_command_line_stays_bounded() {
    // The bound has to be reachable from the entries walked beneath it. One
    // spelled differently from those entries is never matched, and the search
    // climbs past the repository into a `.primignore` that does not govern it —
    // here the one planted above the repository.
    let (repo, above, _outside) = repository_with_a_symlinked_directory();
    // The trap sits above the repository, which is where an unbounded lexical
    // climb goes. The repository root is the bound, so it must never be read.
    std::fs::write(above.path().join(".primignore"), "b.md\n").unwrap();

    prim()
        .current_dir(&repo)
        .args(["fmt", "--check", "link"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("b.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[cfg(unix)]
#[test]
fn a_primignore_above_the_working_directory_does_not_reach_a_symlinked_spelling() {
    // A guard against over-correcting: the bound must not be widened past the
    // working directory, and must not be handed back resolved — either would
    // make this search unbounded and skip the file. The symlinked sibling of
    // `a_primignore_above_the_working_directory_does_not_cover_a_named_directory`.
    // (It does not pin #113 itself: the pre-fix fallback bound gave the same
    // answer here.)
    let (real, link, _temp, _elsewhere) = tree_reachable_two_ways();

    prim()
        .current_dir(real.join("build/inner"))
        .args(["fmt", "--check"])
        .arg(link.join("build/inner/doc.md"))
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"))
        .stderr(predicates::str::contains(".primignore").not());
}

#[cfg(unix)]
#[test]
fn a_symlink_into_the_working_directory_does_not_rewrite_a_protected_file() {
    // The symlink points at a *subdirectory* of the working directory, which
    // is the shape a bound compared for equality never finds. The `.primignore`
    // between the file and that point still has to be read.
    let temp = tempfile::tempdir().unwrap();
    let working = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::create_dir_all(working.join("inner/build")).unwrap();
    std::fs::write(working.join("inner/.primignore"), "build/\n").unwrap();
    std::fs::write(working.join("inner/build/doc.md"), "#  Doc\n").unwrap();

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(working.join("inner"), &link).unwrap();

    prim()
        .current_dir(&working)
        .arg("fmt")
        .arg(link.join("build/doc.md"))
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));

    assert_eq!(
        std::fs::read_to_string(working.join("inner/build/doc.md")).unwrap(),
        "#  Doc\n",
        "the rule between the file and the working directory still protects it"
    );
}

#[cfg(unix)]
#[test]
fn a_rule_naming_a_symlink_to_a_file_matches_it_under_its_own_name() {
    // Matching is lexical for a file as much as for a directory: the rule
    // names `link.md`, and that is the name prim judges — not `real.md`, the
    // target it points at.
    let repo = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(repo.path().join(".git")).unwrap();
    std::fs::write(repo.path().join(".primignore"), "link.md\n").unwrap();
    std::fs::write(repo.path().join("real.md"), "#  Real\n").unwrap();
    std::os::unix::fs::symlink("real.md", repo.path().join("link.md")).unwrap();

    prim()
        .current_dir(repo.path())
        .args(["fmt", "link.md"])
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));

    assert_eq!(
        std::fs::read_to_string(repo.path().join("real.md")).unwrap(),
        "#  Real\n",
        "the rule names the link, so the file behind it is left alone"
    );
}

#[cfg(unix)]
#[test]
fn a_nonexistent_path_spelled_through_a_symlink_is_still_an_error() {
    // `canonicalize` fails on a path that does not exist, which is the risk
    // the issue named for this fix. The bound falls back rather than failing,
    // so FR-4.6's existence error still comes out. (A missing path the
    // `.primignore` does cover is skipped instead — the rule decides before
    // existence does, as it does for any other spelling.)
    let (real, link, _temp, _elsewhere) = tree_reachable_two_ways();

    prim()
        .current_dir(&real)
        .args(["fmt", "--check"])
        .arg(link.join("missing/doc.md"))
        .assert()
        .code(2)
        .stderr(predicates::str::contains("No such file"));
}

#[cfg(unix)]
#[test]
fn a_directory_spelled_through_a_symlink_is_bounded_and_not_escaped() {
    // The walk route through the fixed code, which every other test here
    // reaches only for a named file. Two things at once: the tree's own
    // `.primignore` must be found (or `prim fmt` rewrites what it protects),
    // and the one beside the symlink must not be, because it sits above the
    // bound and governs nothing here.
    let temp = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(real.join(".primignore"), "build/\n").unwrap();
    std::fs::create_dir_all(real.join("build/inner")).unwrap();
    std::fs::write(real.join("build/inner/doc.md"), "#  Doc\n").unwrap();
    std::fs::write(real.join("kept.md"), "#  Kept\n").unwrap();

    let elsewhere = tempfile::tempdir().unwrap();
    // The trap: read only if the search climbs out past its bound.
    std::fs::write(elsewhere.path().join(".primignore"), "kept.md\n").unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    // The symlink itself is the pointed-at path, so the bound lands on it
    // rather than on the directory holding it — which is what keeps the trap
    // out of the search.
    prim()
        .current_dir(&real)
        .args(["fmt", "--check"])
        .arg(&link)
        .assert()
        .code(1)
        .stdout(
            predicates::str::contains("kept.md").and(predicates::str::contains("doc.md").not()),
        );

    prim()
        .current_dir(&real)
        .arg("fmt")
        .arg(&link)
        .assert()
        .success();

    assert_eq!(
        std::fs::read_to_string(real.join("build/inner/doc.md")).unwrap(),
        "#  Doc\n",
        "a directory named through a symlink is covered by the tree's own rule"
    );
}

#[cfg(unix)]
#[test]
fn a_missing_directory_gets_the_same_answer_in_either_spelling() {
    // A directory that does not exist yet says nothing about where the path
    // sits, so the climb carries on rather than giving up — otherwise the two
    // spellings disagree about a path neither can stat.
    let temp = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(real.join(".primignore"), "missing/\n").unwrap();

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    for spelling in [&real, &link] {
        prim()
            .current_dir(&real)
            .arg("fmt")
            .arg(spelling.join("missing/doc.md"))
            .assert()
            .success()
            .stderr(predicates::str::contains("matched by .primignore"));
    }
}

#[cfg(unix)]
#[test]
fn a_missing_path_under_the_working_directory_is_still_covered_by_its_rules() {
    // The ordinary spelling of the case above: a rule covering a directory
    // that does not exist yet still applies to a path named under it. The
    // climb reaches the working directory whether the plain prefix test
    // answers first or not.
    let temp = tempfile::tempdir().unwrap();
    let real = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(real.join(".primignore"), "missing/\n").unwrap();

    prim()
        .current_dir(&real)
        .arg("fmt")
        .arg(real.join("missing/doc.md"))
        .assert()
        .success()
        .stderr(predicates::str::contains("matched by .primignore"));
}

#[cfg(unix)]
#[test]
fn a_rule_above_the_symlinks_target_is_not_reached_through_that_spelling() {
    // The limit AD-0009 records, pinned so it reads as decided rather than as a
    // live defect. The rule sits above the directory the symlink points at, so
    // the path as spelled never passes it and no bound can put it on the
    // search. Reaching it would mean matching the resolved path against a rule
    // the given path never passes — the option AD-0009 rejects, and the one
    // `git` declines by refusing to answer for such a path at all.
    let temp = tempfile::tempdir().unwrap();
    let working = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(working.join(".primignore"), "build/\n").unwrap();
    std::fs::create_dir_all(working.join("inner/build")).unwrap();
    std::fs::write(working.join("inner/build/doc.md"), "#  Doc\n").unwrap();

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(working.join("inner"), &link).unwrap();

    // Named as it resolves, the rule at the tree root covers it.
    prim()
        .current_dir(&working)
        .args(["fmt", "--check", "inner/build/doc.md"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("matched by .primignore"));

    // Named through the symlink, that rule is above the reachable bound.
    prim()
        .current_dir(&working)
        .args(["fmt", "--check"])
        .arg(link.join("build/doc.md"))
        .assert()
        .code(1)
        .stdout(predicates::str::contains("doc.md"));
}
