use super::*;

mod property;
mod regressions;
mod scaffold;

#[test]
fn merge_prepends_root_and_appends_missing_sections_without_reordering_existing_content() {
    let existing = "[*]\nindent_style = space\nindent_size = 2\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n\n[*]\nindent_style = space\nindent_size = 2\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        merged.actions,
        vec![
            "added top-level root = true",
            "added [*.md] with prim_mdlint_strict = false",
            "added [docs/**.md] with prim_mdlint_strict = true",
            "added [docs/wip/**.md] with prim_mdlint_strict = false",
            "added [docs/archive/**.md] with prim_mdlint_strict = false",
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
        "root = true\n[*.md]\nmax_line_length = 100\nprim_mdlint_strict = false\n[*.txt]\nindent_style = space\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_inserts_a_missing_floor_before_an_existing_strict_section() {
    let existing = "root = true\n[docs/**.md]\nprim_mdlint_strict = true\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
}

#[test]
fn merge_leaves_an_existing_explicit_choice_untouched() {
    let existing =
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\n[**/SUMMARY.md]\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents,
        "root = true\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
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
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n"
    );
    assert_eq!(
        merged.actions,
        vec![
            "added [docs/wip/**.md] with prim_mdlint_strict = false",
            "added [docs/archive/**.md] with prim_mdlint_strict = false",
        ]
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
    assert!(
        !merged.contents.contains("docs/archive"),
        "the exemption must not be inserted in a losing position"
    );
    // Three distinct problems, three warnings: the section that is already
    // there and no longer decides its own path, and the two sections prim
    // therefore cannot add.
    assert_eq!(merged.warnings.len(), 3, "{:?}", merged.warnings);
    let refusal = merged
        .warnings
        .iter()
        .find(|warning| warning.contains("[docs/wip/**.md]"))
        .expect("a refusal naming the section prim did not add");
    // The line numbers are what make this message actionable, and they must
    // be lines of the file prim leaves behind: prim inserts [*.md] above
    // both sections in the same run, moving them down by two lines.
    assert!(
        refusal.contains("[docs/**.md] (line 6) comes after [**/SUMMARY.md] (line 4)"),
        "got: {refusal}"
    );
    assert!(
        refusal.contains("put [docs/**.md] before [**/SUMMARY.md] yourself"),
        "the advice must be possible to follow; got: {refusal}"
    );
}

#[test]
fn merge_warns_when_every_section_is_present_but_docs_wip_precedes_the_strict_glob() {
    // Reproduction (whole-branch review, Important 1): every canonical
    // section already carries an explicit value, so each per-spec iteration
    // hits the already-has-the-key early-continue and, before this fix, took
    // no action and emitted no warning — `prim init` reported success even
    // though
    // `[docs/**.md]`, written after `[docs/wip/**.md]`, wins under
    // EditorConfig's last-match-wins resolution and defeats the docs/wip
    // exemption for every file under it.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
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
        "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n",
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
fn merge_does_not_write_into_a_keyless_section_a_later_one_overrides() {
    // Reproduction (whole-branch re-review): `[docs/wip/**.md]` exists but is
    // KEYLESS, written before `[docs/**.md]` which already has its key.
    // Inserting the key into it would report "updated" over a file that still
    // resolves to the wrong tier — the freshly-inserted `false` loses to
    // `[docs/**.md]`'s `true` under last-match-wins, so `docs/wip/**.md`
    // stays strict. The outcome check is what refuses the write now.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.contents, existing,
        "a section whose freshly-written value would lose must not be written into"
    );
    assert!(
        merged.actions.is_empty(),
        "no action may claim an update the file would not actually resolve to; got: {:?}",
        merged.actions
    );
    assert_eq!(merged.warnings.len(), 1, "exactly one conflict is expected");
    assert!(merged.warnings[0].contains("[docs/**.md]"));
    assert!(merged.warnings[0].contains("[docs/wip/**.md]"));
}

#[test]
fn run_reports_no_update_for_a_keyless_section_a_later_one_overrides() {
    // Same reproduction as
    // `merge_does_not_write_into_a_keyless_section_a_later_one_overrides`,
    // checked end-to-end through `run`: the outcome message must not say
    // "updated" (which `merge`'s now-empty `actions` already rules out) and
    // the file on disk must be byte-identical to what was written.
    let dir = tempfile::tempdir().unwrap();
    let content = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/wip/**.md]\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
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
fn merge_skips_the_docs_wip_exemption_when_the_strict_glob_is_docs_wip() {
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/wip/**.md");

    assert_eq!(
        merged.actions,
        vec![
            "added [docs/wip/**.md] with prim_mdlint_strict = true",
            "added [docs/archive/**.md] with prim_mdlint_strict = false",
        ],
        "only the strict section is added; no separate false exemption for the same glob"
    );
    assert_eq!(
        merged.contents.matches("[docs/wip/**.md]").count(),
        1,
        "the exemption section must not duplicate the strict section with a conflicting value"
    );
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

#[test]
fn merge_makes_no_change_to_an_editorconfig_it_cannot_parse() {
    // prim's own section scanner tolerates a line an EditorConfig parser
    // rejects. Without a resolution there is nothing to check a write
    // against, so prim reports the file and leaves it alone rather than
    // writing blind.
    let existing = "root = true\ngarbage\n[*.md]\nindent_size = 2\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(merged.contents, existing);
    assert!(merged.actions.is_empty());
    assert_eq!(merged.warnings.len(), 1, "{:?}", merged.warnings);
    assert!(
        merged.warnings[0].contains("does not parse"),
        "got: {}",
        merged.warnings[0]
    );
}

#[test]
fn a_later_occurrence_of_the_same_glob_is_the_persons_override_and_is_not_reported() {
    // Two `[*.md]` sections, both with the key, both before the rest of the
    // map: the last one is what EditorConfig reads and what prim must judge
    // the map by. Reading the first would report a section as defeated when
    // the person deliberately replaced it.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[*.md]\nprim_mdlint_strict = true\n[docs/**.md]\nprim_mdlint_strict = true\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert!(
        merged.warnings.is_empty(),
        "a deliberate later override is not a defeat: {:?}",
        merged.warnings
    );
}

#[test]
fn a_key_written_twice_in_one_section_is_read_the_way_editorconfig_reads_it() {
    // EditorConfig keeps the last value in a section; prim must report on the
    // same one, or it names a value the file does not actually use.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\nprim_mdlint_strict = mabye\n[docs/wip/**.md]\nprim_mdlint_strict = false\n[docs/archive/**.md]\nprim_mdlint_strict = false\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(merged.warnings.len(), 1, "{:?}", merged.warnings);
    assert!(
        merged.warnings[0].contains("mabye"),
        "the last value in the section is the one that counts; got: {}",
        merged.warnings[0]
    );
}

#[test]
fn a_defeated_section_is_reported_while_the_writes_that_are_safe_still_happen() {
    // A warning about a section prim cannot fix must not turn into a refusal
    // to make the changes it can: `[docs/*.md]`, appended last, defeats the
    // SUMMARY exemption, and the docs/wip exemption is still missing.
    let existing = "root = true\n[*.md]\nprim_mdlint_strict = false\n[docs/**.md]\nprim_mdlint_strict = true\n[**/SUMMARY.md]\nprim_mdlint_strict = false\n[docs/*.md]\nprim_mdlint_strict = true\n";
    let merged = merge(existing, "docs/**.md");

    assert_eq!(
        merged.actions,
        vec![
            "added [docs/wip/**.md] with prim_mdlint_strict = false",
            "added [docs/archive/**.md] with prim_mdlint_strict = false",
        ],
        "the safe write still happens alongside the warning"
    );
    assert_eq!(merged.warnings.len(), 1, "{:?}", merged.warnings);
    assert!(
        merged.warnings[0].contains("[**/SUMMARY.md]")
            && merged.warnings[0].contains("[docs/*.md]"),
        "got: {}",
        merged.warnings[0]
    );
}
