use super::*;

#[test]
fn scaffold_matches_the_default_contract() {
    assert_eq!(
        scaffold("docs/**.md"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn scaffold_places_all_four_sections_in_order_for_a_custom_strict_glob() {
    // The docs/wip exemption is a literal, not derived from the strict glob,
    // so it must appear even when book.toml points the strict tier at a
    // non-default mdBook `src` directory such as `guide`.
    assert_eq!(
        scaffold("guide/**.md"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[guide/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_prepends_root_and_appends_missing_sections_without_reordering_existing_content() {
    let existing = "[*]\nindent_style = space\nindent_size = 2\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n\n[*]\nindent_style = space\nindent_size = 2\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        merged.actions,
        vec![
            "added top-level root = true",
            "added [*.md] with prim_mdlint_strict = false",
            "added [docs/**.md] with prim_mdlint_strict = true",
            "added [docs/wip/**.md] with prim_mdlint_strict = false",
            "added [**/SUMMARY.md] with prim_mdlint_strict = false",
        ]
    );
}

#[test]
fn merge_adds_the_missing_key_in_place_for_an_existing_section() {
    let existing = "root = true\n[*.md]\nmax_line_length = 100\n[*.txt]\nindent_style = space\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nmax_line_length = 100\nprim_mdlint_strict = false\n[*.txt]\nindent_style = space\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_inserts_a_missing_floor_before_an_existing_strict_section() {
    let existing = "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_leaves_an_existing_explicit_choice_untouched() {
    let existing =
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\n[**/SUMMARY.md]\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_inserts_docs_wip_between_the_strict_glob_and_summary_for_a_canonically_ordered_file() {
    // The upgrade path: a file the old three-section `prim init` wrote,
    // missing only the new `docs/wip` exemption. Its existing sections are
    // already in canonical order, so no conflict exists and the new section
    // must land exactly between the strict glob and `SUMMARY.md`.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        merged.actions,
        vec!["added [docs/wip/**.md] with prim_mdlint_strict = false"]
    );
    assert!(
        merged.warnings.is_empty(),
        "a canonically-ordered file must not warn"
    );
}

#[test]
fn merge_refuses_to_insert_docs_wip_when_existing_sections_are_out_of_canonical_order() {
    // Reproduction: `[**/SUMMARY.md]` was written before `[docs/**.md]`, so
    // the canonical order docs/wip would need to be anchored to — strict
    // glob, then docs/wip, then SUMMARY.md — is contradicted by the file's
    // own order. Inserting docs/wip anywhere would silently pick a losing
    // position under EditorConfig's last-match-wins resolution, so prim must
    // leave it out and warn instead of reordering what the author wrote.
    let existing = "root = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n",
        "the pre-existing sections stay exactly where the author put them, \
         and docs/wip is not inserted anywhere"
    );
    assert!(
        !merged.contents.contains("docs/wip"),
        "the exemption must not be inserted in a losing position"
    );
    assert_eq!(merged.warnings.len(), 1, "exactly one conflict is expected");
    assert!(merged.warnings[0].contains("[docs/wip/**.md]"));
    assert!(merged.warnings[0].contains("[docs/**.md]"));
    assert!(merged.warnings[0].contains("[**/SUMMARY.md]"));
}

#[test]
fn merge_warns_when_all_four_sections_are_present_but_docs_wip_precedes_the_strict_glob() {
    // Reproduction (whole-branch review, Important 1): every canonical
    // section already carries an explicit value, so each per-spec iteration
    // hits the has_key early-continue and, before this fix, took no action
    // and emitted no warning — `prim init` reported success even though
    // `[docs/**.md]`, written after `[docs/wip/**.md]`, wins under
    // EditorConfig's last-match-wins resolution and defeats the docs/wip
    // exemption for every file under it.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents, existing,
        "prim must not reorder or rewrite sections a person wrote"
    );
    assert!(
        merged.actions.is_empty(),
        "nothing needed inserting; every section already had its key"
    );
    assert_eq!(merged.warnings.len(), 1, "exactly one conflict is expected");
    assert!(merged.warnings[0].contains("[docs/**.md]"));
    assert!(merged.warnings[0].contains("[docs/wip/**.md]"));
}

#[test]
fn run_does_not_claim_the_map_is_present_when_a_conflict_blocks_an_update() {
    // Reproduction (whole-branch review, Important 2): before this fix,
    // emitting a refusal warning still fell through to the
    // `actions.is_empty()` branch, which unconditionally claimed the map was
    // already present — a scripted caller or a skimming reader takes that
    // line, not the warning above it, as the outcome.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
    )
    .unwrap();

    let outcome = run(dir.path()).unwrap();

    assert!(
        !outcome.message.contains("already contains"),
        "a refusal warning must not be followed by a false claim of success; got: {}",
        outcome.message
    );
    assert!(
        outcome.message.contains("left unchanged"),
        "got: {}",
        outcome.message
    );
}

#[test]
fn merge_does_not_write_into_a_section_the_order_pre_pass_flagged() {
    // Reproduction (whole-branch re-review): `[docs/wip/**.md]` exists but is
    // KEYLESS, written before `[docs/**.md]` which already has its key. The
    // pre-pass correctly flags the pair as out of canonical order, but
    // before this fix the per-spec loop never consulted that finding: it
    // only tests `has_key`, saw docs/wip's section has no key, and inserted
    // one into it anyway — writing into a position it had just warned was
    // broken, and reporting "updated" over a file that still resolves to
    // the wrong tier (the freshly-inserted `false` loses to `[docs/**.md]`'s
    // `true` under last-match-wins, so `docs/wip/**.md` stays strict).
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents, existing,
        "a section on either side of a flagged conflict must not be written into"
    );
    assert!(
        merged.actions.is_empty(),
        "no action may claim an update for a section the pre-pass just flagged; got: {:?}",
        merged.actions
    );
    assert_eq!(merged.warnings.len(), 1, "exactly one conflict is expected");
    assert!(merged.warnings[0].contains("[docs/**.md]"));
    assert!(merged.warnings[0].contains("[docs/wip/**.md]"));
}

#[test]
fn run_reports_no_update_for_a_keyless_section_the_order_pre_pass_flagged() {
    // Same reproduction as `merge_does_not_write_into_a_section_the_order_pre_pass_flagged`,
    // checked end-to-end through `run`: the outcome message must not say
    // "updated" (which `merge`'s now-empty `actions` already rules out) and
    // the file on disk must be byte-identical to what was written.
    let dir = tempfile::tempdir().unwrap();
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let editorconfig = dir.path().join(".editorconfig");
    fs::write(&editorconfig, content).unwrap();

    let outcome = run(dir.path()).unwrap();

    assert!(
        !outcome.message.contains("updated"),
        "got: {}",
        outcome.message
    );
    assert_eq!(
        fs::read_to_string(&editorconfig).unwrap(),
        content,
        "the file must be left exactly as written"
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
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        content.matches("[docs/wip/**.md]").count(),
        1,
        "the exemption section must not duplicate the strict section with a conflicting value"
    );
}

#[test]
fn merge_skips_the_docs_wip_exemption_when_the_strict_glob_is_docs_wip() {
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/wip/**.md");

    assert_eq!(
        merged.actions,
        vec!["added [docs/wip/**.md] with prim_mdlint_strict = true"],
        "only the strict section is added; no separate false exemption for the same glob"
    );
    assert_eq!(
        merged.contents.matches("[docs/wip/**.md]").count(),
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
fn running_twice_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();

    let first = run(dir.path()).unwrap();
    let once = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();
    let second = run(dir.path()).unwrap();
    let twice = fs::read_to_string(dir.path().join(".editorconfig")).unwrap();

    assert!(first.message.contains("created"));
    assert!(second.message.contains("already contains"));
    assert_eq!(once, twice);
}

#[test]
fn non_utf8_editorconfig_is_reported_and_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".editorconfig");
    fs::write(&path, [0xFFu8, 0xFE, 0x00]).unwrap();

    let err = run(dir.path()).unwrap_err();

    assert!(matches!(err, Error::ReadEditorConfig { .. }));
    assert_eq!(fs::read(&path).unwrap(), [0xFFu8, 0xFE, 0x00]);
}
