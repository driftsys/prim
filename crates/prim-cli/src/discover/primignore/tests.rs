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
fn a_nested_negation_does_not_re_include_a_file_under_an_ignored_directory() {
    // gitignore's rule: "it is not possible to re-include a file if a
    // parent directory of that file is excluded". The negation sits in a
    // `.primignore` below the excluded directory, so it is the nearest
    // rule — and still loses to the exclusion above it (#114).
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "docs/\n");
    write(&repo.path().join("docs/.primignore"), "!notes.md\n");
    let notes = repo.path().join("docs/notes.md");
    write(&notes, "#  Notes\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&notes, false, bound_for(&notes).as_deref()),
        Verdict::Ignored,
        "an excluded parent directory takes the file with it"
    );
}

#[test]
fn a_negation_beside_the_exclusion_does_not_re_include_it_either() {
    // The same rule where both lines live in one `.primignore`: the walk
    // never descends into `build/`, so naming the file must not either.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "build/\n!build/keep.md\n");
    let keep = repo.path().join("build/keep.md");
    write(&keep, "#  Keep\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&keep, false, bound_for(&keep).as_deref()),
        Verdict::Ignored,
        "a negation cannot re-include a file under an excluded directory"
    );
}

#[test]
fn an_excluded_directory_covers_a_file_nested_well_below_it() {
    // The rule is about every directory holding the path, not only the one
    // that holds it directly: `build/` covers `build/sub` too, so the
    // negation two levels down never gets to speak.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "build/\n");
    write(&repo.path().join("build/sub/.primignore"), "!keep.md\n");
    let keep = repo.path().join("build/sub/keep.md");
    write(&keep, "#  Keep\n");
    let bound = bound_for(&keep);

    assert_eq!(
        PrimignoreCache::default().verdict(&keep, false, bound.as_deref()),
        Verdict::Ignored,
        "an exclusion any distance above the file covers it"
    );
    assert_eq!(
        PrimignoreCache::default().verdict(&repo.path().join("build/sub"), true, bound.as_deref()),
        Verdict::Ignored,
        "naming the directory between them reaches the same rule"
    );
}

#[test]
fn a_re_included_directory_does_not_stop_the_file_under_it() {
    // Only an ancestor its own stack *excludes* stops the path. One a `!`
    // rule re-includes is what gitignore calls a re-included directory, and
    // the search must carry on into it — `git` treats `docs/keep.md` here
    // as an ordinary tracked-able file.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(
        &repo.path().join(".primignore"),
        "*\n!docs\n!docs/keep.md\n",
    );
    let keep = repo.path().join("docs/keep.md");
    write(&keep, "#  Keep\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&keep, false, bound_for(&keep).as_deref()),
        Verdict::Whitelisted(true),
        "a directory `*` excluded and `!docs` re-included holds a file the \
         next negation re-includes"
    );
}

#[test]
fn a_nearer_negation_naming_a_parent_does_not_answer_for_the_file() {
    // `!c` re-includes the directory `c`, and says nothing about the file
    // inside it. Folding the parents into the file's own match would let that
    // nearer rule answer first and re-include `d.md`, which the rule at the
    // repository root names outright — and `git check-ignore` reports that
    // root file as the one that decides it.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "b/c/d.md\n");
    write(&repo.path().join("b/.primignore"), "!c\n");
    let doc = repo.path().join("b/c/d.md");
    write(&doc, "#  Doc\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&doc, false, bound_for(&doc).as_deref()),
        Verdict::Ignored,
        "the rule naming the file decides it, not the nearer rule naming \
         its directory"
    );
}

#[test]
fn a_negation_still_re_includes_a_file_whose_parents_are_not_excluded() {
    // The rule is about excluded parents, not about negations: where
    // nothing above the file is excluded, a `!` rule keeps working, which
    // is what the documented `!package-lock.json` recipe relies on.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "*.md\n!keep.md\n");
    let keep = repo.path().join("docs/keep.md");
    write(&keep, "#  Keep\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&keep, false, bound_for(&keep).as_deref()),
        Verdict::Whitelisted(true),
        "`!keep.md` names the file and no parent directory is excluded"
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

/// The shapes a directory exclusion can take, each paired with the verdict
/// `git check-ignore` gives for `build/keep.md` under it. The negation is
/// always `!build/keep.md`, so what varies is only how the directory is named —
/// which is where gitignore semantics bite, and where a change to
/// `Scope::is_covered` would show up first.
const DIRECTORY_SHAPES: [(&str, Verdict); 6] = [
    // A pattern that names the directory excludes it, and the negation under
    // it cannot take the file back.
    ("build/\n!build/keep.md\n", Verdict::Ignored),
    ("build\n!build/keep.md\n", Verdict::Ignored),
    // These name the directory's *contents*, so the directory itself is never
    // excluded and the negation still works — including `build/*`, which is
    // the migration path `docs/recipes.md` prescribes for a `.primignore` this
    // rule breaks.
    ("build/**\n!build/keep.md\n", Verdict::Whitelisted(true)),
    ("**/build/**\n!build/keep.md\n", Verdict::Whitelisted(true)),
    ("build/*\n!build/keep.md\n", Verdict::Whitelisted(true)),
    // A directory `*` excluded and a later `!` line put back is not excluded.
    ("*\n!build\n!build/keep.md\n", Verdict::Whitelisted(true)),
];

#[test]
fn every_directory_shape_answers_the_way_git_does() {
    for (rules, expected) in DIRECTORY_SHAPES {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        write(&repo.path().join(".primignore"), rules);
        let keep = repo.path().join("build/keep.md");
        write(&keep, "#  Keep\n");

        assert_eq!(
            PrimignoreCache::default().verdict(&keep, false, bound_for(&keep).as_deref()),
            expected,
            "`build/keep.md` under {rules:?}"
        );
    }
}

#[test]
fn excluding_a_directorys_contents_still_covers_its_other_files() {
    // The other half of the `build/*` migration path: the file the negation
    // names comes back, and everything beside it stays covered.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(
        &repo.path().join(".primignore"),
        "build/*\n!build/keep.md\n",
    );
    let drop = repo.path().join("build/drop.md");
    write(&drop, "#  Drop\n");

    assert_eq!(
        PrimignoreCache::default().verdict(&drop, false, bound_for(&drop).as_deref()),
        Verdict::Ignored,
        "only the file the negation names comes back"
    );
}

#[test]
fn one_cache_answers_for_a_sibling_after_an_excluded_directory() {
    // The memo is keyed by the directory it describes. Keyed by anything else —
    // its parent, say — the first question would poison the answer for every
    // sibling of the excluded directory, which is what a hook handing prim a
    // whole staged list would hit.
    let repo = tempfile::tempdir().unwrap();
    fs::create_dir_all(repo.path().join(".git")).unwrap();
    write(&repo.path().join(".primignore"), "a/b/\n");
    let inner = repo.path().join("a/b/inner.json");
    write(&inner, "{\"a\" :1}\n");
    let other = repo.path().join("a/other.json");
    write(&other, "{\"a\" :1}\n");
    let bound = bound_for(&inner);

    let mut cache = PrimignoreCache::default();
    assert_eq!(
        cache.verdict(&inner, false, bound.as_deref()),
        Verdict::Ignored,
        "`a/b/` covers the file under it"
    );
    assert_eq!(
        cache.verdict(&other, false, bound.as_deref()),
        Verdict::Unmatched,
        "the sibling beside `a/b/` is untouched by that answer"
    );
}

/// A directory reachable two ways: its own resolved path, and a symlink to it.
#[cfg(unix)]
fn directory_reachable_two_ways() -> (PathBuf, PathBuf, tempfile::TempDir, tempfile::TempDir) {
    let temp = tempfile::tempdir().unwrap();
    let real = fs::canonicalize(temp.path()).unwrap();
    write(&real.join("sub/file.md"), "#  File\n");

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    (real, link, temp, elsewhere)
}

#[cfg(unix)]
#[test]
fn a_symlink_into_the_working_directory_keeps_every_rule_between() {
    // The symlink may point anywhere under the working directory rather than
    // at it, and the search must still read every `.primignore` between the
    // path and that point. Comparing an ancestor for equality with the working
    // directory finds no bound at all here, and the search stops at the
    // pointed-at directory — short of the rule protecting the file (#113).
    let temp = tempfile::tempdir().unwrap();
    let working = fs::canonicalize(temp.path()).unwrap();
    write(&working.join("inner/build/doc.md"), "#  Doc\n");

    let elsewhere = tempfile::tempdir().unwrap();
    let link = elsewhere.path().join("link");
    std::os::unix::fs::symlink(working.join("inner"), &link).unwrap();

    assert_eq!(
        bound_from(&link.join("build/doc.md"), Some(working)).as_deref(),
        Some(link.as_path()),
        "the bound is the outermost ancestor still inside the working \
         directory, so `link/.primignore` is still read"
    );
}

#[cfg(unix)]
#[test]
fn a_working_directory_that_is_not_reported_resolved_still_bounds_the_search() {
    // Both sides of the comparison are resolved, not just the path's. On Unix
    // `std::env::current_dir` already reports a resolved path, so this stands
    // in for the platform where it does not: on Windows it reports `C:\...`
    // while resolving one yields the verbatim `\\?\C:\...` form, and comparing
    // the two spellings would lose the bound exactly as #113 did. That target
    // is built by CI but never tested on, so the seam is checked here instead.
    let (real, link, _temp, _elsewhere) = directory_reachable_two_ways();

    assert_eq!(
        bound_from(&real.join("sub/file.md"), Some(link)).as_deref(),
        Some(real.as_path()),
        "the working directory bounds the search however it is reported"
    );
}

#[cfg(unix)]
#[test]
fn the_bound_is_named_the_way_the_path_under_it_is() {
    // The answer has to be an ancestor of the path *as spelled*, because that
    // is what `matchers_between` climbs. Handing back the resolved spelling
    // would be the same defect from the other side.
    let (real, link, _temp, _elsewhere) = directory_reachable_two_ways();

    assert_eq!(
        bound_from(&link.join("sub/file.md"), Some(real)).as_deref(),
        Some(link.as_path()),
        "the bound is spelled the way the search will walk"
    );
}
