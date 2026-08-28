use super::*;
use std::fs;
use std::path::Path;

/// Test-only helper over the production resolver.
fn strict_for(dir: &Path, relative: &str) -> bool {
    resolve(&dir.join(relative)).selection.strict
}

#[test]
fn prim_custom_key_resolves_per_glob_more_specific_later_wins() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n\
         [*.md]\n\
         prim_mdlint_strict = false\n\
         [docs/**.md]\n\
         prim_mdlint_strict = true\n\
         [**/SUMMARY.md]\n\
         prim_mdlint_strict = false\n",
    )
    .unwrap();

    assert!(
        !strict_for(dir.path(), "README.md"),
        "top-level doc is floor"
    );
    assert!(
        strict_for(dir.path(), "docs/guide.md"),
        "docs/ doc is strict"
    );
    assert!(
        !strict_for(dir.path(), "docs/SUMMARY.md"),
        "SUMMARY.md is floor (SUMMARY-safe)"
    );
}

#[test]
fn nearer_config_overrides_prim_key_from_a_farther_one() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = false\n",
    )
    .unwrap();
    let sub = dir.path().join("pkg");
    fs::create_dir(&sub).unwrap();
    fs::write(
        sub.join(".editorconfig"),
        "[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();

    assert!(
        strict_for(dir.path(), "pkg/child.md"),
        "nearer config overrides the farther one for custom keys"
    );
}

#[test]
fn unset_prim_key_is_none() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nindent_size = 2\n",
    )
    .unwrap();
    assert!(
        !strict_for(dir.path(), "a.md"),
        "an unset custom key resolves to the floor tier (false)"
    );
}

#[test]
fn disable_list_splits_trims_and_uppercases() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD033 , md041\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(policy.selection.disabled, vec!["MD033", "MD041"]);
}

#[test]
fn a_narrower_section_replaces_the_wider_list() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD033\n[docs/**.md]\nprim_mdlint_disable = MD041\n",
    )
    .unwrap();

    assert_eq!(
        resolve(&dir.path().join("a.md")).selection.disabled,
        vec!["MD033"]
    );
    assert_eq!(
        resolve(&dir.path().join("docs/g.md")).selection.disabled,
        vec!["MD041"],
        "EditorConfig replaces a value, it does not merge lists"
    );
}

#[test]
fn an_unknown_rule_id_is_dropped_rather_than_matched() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD999, MD033\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(
        policy.selection.disabled,
        vec!["MD033"],
        "an unknown id is dropped from the exclusion set"
    );
    assert_eq!(
        policy.unknown,
        vec!["MD999"],
        "an unknown id is still surfaced for the caller to report"
    );
}

#[test]
fn an_empty_or_unset_key_disables_nothing() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), "root = true\n[*.md]\n").unwrap();
    assert!(
        resolve(&dir.path().join("a.md"))
            .selection
            .disabled
            .is_empty()
    );
}

#[test]
fn a_value_of_unset_or_none_disables_nothing_and_is_not_reported_as_unknown() {
    // `prim_mdlint_disable = unset` (EditorConfig's own reserved word)
    // and `= none` (prim's accepted spelling of the same intent) must
    // clear the list on purpose, not by accident of failing to match a
    // known rule id — and neither should trigger the "is not a rule
    // prim runs" warning that a genuine typo gets.
    for value in ["unset", "UNSET", "none", "None"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            format!("root = true\n[*.md]\nprim_mdlint_disable = {value}\n"),
        )
        .unwrap();

        let policy = resolve(&dir.path().join("a.md"));
        assert!(policy.selection.disabled.is_empty(), "value: {value:?}");
        assert!(
            policy.unknown.is_empty(),
            "value {value:?} must not be reported as an unrecognised rule id"
        );
    }
}
