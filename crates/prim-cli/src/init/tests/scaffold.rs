//! Tests for the map `prim init` writes into a repository that has no
//! `.editorconfig` yet: which sections it contains for a given strict glob,
//! where that glob comes from, and the check prim runs over that map before
//! writing it.

use super::super::*;

/// The map prim writes for `strict_glob`, which is only obtainable already
/// checked — so every use of this helper also asserts that prim's own map
/// holds for that glob.
fn scaffold(strict_glob: &str) -> String {
    map::checked_scaffold(strict_glob).expect("prim's own map holds")
}

#[test]
fn scaffold_matches_the_default_contract() {
    assert_eq!(
        scaffold("docs/**.md"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn scaffold_places_every_section_in_order_for_a_custom_strict_glob() {
    // The working memory exemptions are literals, not derived from the strict
    // glob, so they must appear even when book.toml points the strict tier at a
    // non-default mdBook `src` directory such as `guide`.
    assert_eq!(
        scaffold("guide/**.md"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[guide/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn scaffold_skips_the_docs_wip_exemption_when_the_strict_glob_is_docs_wip() {
    // A mdBook with `src = "docs/wip"` derives a strict glob identical to
    // the literal docs/wip exemption. Emitting both would write
    // `[docs/wip/**.md] = true` then `[docs/wip/**.md] = false` — the
    // exemption, written after, wins under last-match-wins and silently
    // defeats the strict tier for the whole book, even though the author
    // asked for that directory to be strict.
    let content = scaffold("docs/wip/**.md");

    assert_eq!(
        content,
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        content.matches("[docs/wip/**.md]").count(),
        1,
        "the exemption section must not duplicate the strict section with a conflicting value"
    );
}

#[test]
fn book_toml_custom_src_changes_the_strict_glob() {
    assert_eq!(
        strict_glob_from_book_toml("[book]\nsrc = \"guide\"\n"),
        "guide/**.md"
    );
}

#[test]
fn book_toml_src_is_normalized_before_becoming_a_glob() {
    assert_eq!(
        strict_glob_from_book_toml("[book]\nsrc = \"./guide/\"\n"),
        "guide/**.md"
    );
}

#[test]
fn book_toml_without_src_defaults_to_src_directory() {
    assert_eq!(
        strict_glob_from_book_toml("[book]\ntitle = \"prim\"\n"),
        "src/**.md"
    );
}

#[test]
fn malformed_book_toml_also_defaults_to_src_directory() {
    assert_eq!(strict_glob_from_book_toml("[book]\nsrc =\n"), "src/**.md");
}

#[test]
fn a_book_src_inside_docs_wip_keeps_its_strict_tier() {
    // An mdBook whose `src` is nested inside `docs/wip` derives a strict glob
    // the literal exemption completely covers. Writing the exemption after it
    // turns the strict tier off for the whole book under last-match-wins —
    // and the author asked for that tree to be strict, so the exemption is
    // what has to go, exactly as when the two globs are equal.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("book.toml"),
        "[book]\nsrc = \"docs/wip/sub\"\n",
    )
    .unwrap();

    run(dir.path()).unwrap();

    assert!(
        crate::mdlint_policy::resolve(&dir.path().join("docs/wip/sub/guide.md"))
            .selection
            .strict,
        "the book prim was pointed at must be strict: {}",
        fs::read_to_string(dir.path().join(".editorconfig")).unwrap()
    );
}

#[test]
fn a_strict_glob_that_covers_every_directory_has_no_floor_section() {
    // An mdBook with `src = "."` yields `[**.md]`, which covers every
    // Markdown file there is — a `[*.md] = false` section above it would
    // decide nothing at all, which is how the docs/wip exemption went wrong
    // twice. prim leaves it out of both the map it writes and the sections
    // it checks, so `README.md` is strict because the author asked for the
    // whole repository to be.
    assert_eq!(
        scaffold("**.md"),
        "root = true\n[**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );

    let merged = merge("root = true\n[**.md]\n", "**.md");

    assert_eq!(merged.contents, scaffold("**.md"));
    assert!(merged.warnings.is_empty(), "{:?}", merged.warnings);
    assert_eq!(merged.actions.len(), 4, "{:?}", merged.actions);
}

#[test]
fn the_scaffold_self_check_catches_a_map_that_does_not_resolve_the_way_it_reads() {
    // The check that runs over prim's own map before it is written, driven
    // here with a deliberately broken one: the exemption is written before
    // the strict glob, so `docs/wip/plan.md` ends up strict. Intent comes
    // from each section's declared value, never from this text, which is what
    // lets the text be wrong.
    let broken = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";

    let flaws = map::map_flaws(&map::canonical_specs("docs/**.md"), broken);

    assert_eq!(flaws.len(), 2, "{flaws:?}");
    assert!(
        flaws
            .iter()
            .any(|f| f.contains("docs/wip/plan.md") && f.contains("not false")),
        "got: {flaws:?}"
    );
    assert!(
        flaws
            .iter()
            .any(|f| f.contains("docs/archive/plan.md") && f.contains("not false")),
        "got: {flaws:?}"
    );
    // And prim's real map passes its own check, for every strict glob it can
    // derive — including the ones the collapses exist for: a book rooted at
    // each exempt directory, and one rooted inside it.
    let mut globs = vec!["docs/**.md", "guide/**.md", "**.md"];
    let exemptions: Vec<String> = WORKING_MEMORY
        .iter()
        .flat_map(|memory| [memory.glob.to_string(), format!("{}/sub/**.md", memory.dir)])
        .collect();
    globs.extend(exemptions.iter().map(String::as_str));
    for glob in globs {
        assert!(
            map::checked_scaffold(glob).is_ok(),
            "prim's own map for [{glob}] does not hold: {:?}",
            map::checked_scaffold(glob).unwrap_err()
        );
    }
}

#[test]
fn each_working_memory_glob_matches_its_directory() {
    // The glob is spelled out beside the directory rather than derived, so
    // that it can stay a `&'static str`. This is what holds the two together.
    for memory in WORKING_MEMORY {
        assert_eq!(memory.glob, format!("{}/**.md", memory.dir));
    }
}

#[test]
fn the_created_message_names_exactly_the_sections_that_were_written() {
    // The summary is read off the same list the file is built from, so it can
    // never advertise a section prim decided to skip.
    for src in ["docs/wip/sub", "."] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("book.toml"),
            format!("[book]\nsrc = \"{src}\"\n"),
        )
        .unwrap();

        let message = run(dir.path()).unwrap().message;
        let written = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();

        for glob in [
            "*.md",
            "docs/wip/**.md",
            "docs/archive/**.md",
            "**/SUMMARY.md",
        ] {
            assert_eq!(
                message.contains(&format!("[{glob}]")),
                written.contains(&format!("[{glob}]\n")),
                "src {src:?}: the summary and the file disagree about [{glob}]\n{message}\n{written}"
            );
        }
    }
}

#[test]
fn a_directory_name_prim_cannot_build_a_probe_for_is_still_scaffolded() {
    // A glob and a representative path are built from the same `src` string
    // by concatenation, so a directory whose name contains glob syntax —
    // `docs[1]`, `book?` — or a `./` segment yields a path its own glob does
    // not match. That is prim failing to construct a probe, not prim's map
    // being wrong, and it must not turn a legal directory name into a
    // refusal.
    for src in ["docs[1]", "book?", "./docs/./guide", "{a,b}", "*"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("book.toml"),
            format!("[book]\nsrc = \"{src}\"\n"),
        )
        .unwrap();

        let outcome = run(dir.path()).expect(src);

        assert!(outcome.message.contains("created"), "src {src:?}");
        assert!(
            dir.path().join(".editorconfig").exists(),
            "src {src:?}: nothing was written"
        );
    }
}
