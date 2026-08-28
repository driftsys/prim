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
    let reached: Vec<_> = policy
        .rejected
        .iter()
        .map(|reject| (reject.id.as_str(), reject.reach))
        .collect();
    assert_eq!(
        reached,
        vec![("MD999", prim_fmt::RuleReach::Unknown)],
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
    // prim knows" warning that a genuine typo gets.
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
            policy.rejected.is_empty(),
            "value {value:?} must not be reported as an unrecognised rule id"
        );
    }
}

#[test]
fn the_enable_list_splits_trims_and_uppercases() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD013 , md014\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(policy.selection.enabled, vec!["MD013", "MD014"]);
}

#[test]
fn a_narrower_section_replaces_the_wider_enable_list() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join("docs")).unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD013\n[docs/**.md]\nprim_mdlint_enable = MD014\n",
    )
    .unwrap();

    assert_eq!(
        resolve(&dir.path().join("a.md")).selection.enabled,
        vec!["MD013"]
    );
    assert_eq!(
        resolve(&dir.path().join("docs/g.md")).selection.enabled,
        vec!["MD014"],
        "EditorConfig replaces a value, it does not merge lists"
    );
}

#[test]
fn an_enable_value_of_unset_or_none_enables_nothing_and_is_not_rejected() {
    for value in ["unset", "UNSET", "none", "None"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            format!("root = true\n[*.md]\nprim_mdlint_enable = {value}\n"),
        )
        .unwrap();

        let policy = resolve(&dir.path().join("a.md"));
        assert!(policy.selection.enabled.is_empty(), "value: {value:?}");
        assert!(policy.rejected.is_empty(), "value: {value:?}");
    }
}

#[test]
fn a_withheld_id_is_rejected_separately_from_a_typo() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD072, MD999, MD013\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert_eq!(policy.selection.enabled, vec!["MD013"]);
    let reached: Vec<_> = policy
        .rejected
        .iter()
        .map(|reject| (reject.id.as_str(), reject.reach, reject.key))
        .collect();
    assert_eq!(
        reached,
        vec![
            ("MD072", prim_fmt::RuleReach::Withheld, MDLINT_ENABLE_KEY),
            ("MD999", prim_fmt::RuleReach::Unknown, MDLINT_ENABLE_KEY),
        ]
    );
}

#[test]
fn each_key_attributes_its_own_rejects_to_its_own_line() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_enable = MD999\nprim_mdlint_disable = MD998\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    let line_of = |id: &str| match &policy
        .rejected
        .iter()
        .find(|reject| reject.id == id)
        .expect("rejected id present")
        .origin
    {
        SettingOrigin::EditorConfig { line, .. } => *line,
        SettingOrigin::Default => panic!("{id} must be attributed to .editorconfig"),
    };
    assert_eq!(line_of("MD999"), 3, "the enable key is on line 3");
    assert_eq!(line_of("MD998"), 4, "the disable key is on line 4");
}

#[test]
fn a_selectable_id_the_path_does_not_run_is_not_rejected() {
    // MD013 is opt-in, so disabling it without enabling it changes nothing —
    // but it is a real rule prim can run, not a typo, and must not be reported
    // as one.
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_disable = MD013\n",
    )
    .unwrap();

    let policy = resolve(&dir.path().join("a.md"));
    assert!(policy.rejected.is_empty());
    assert_eq!(policy.selection.disabled, vec!["MD013"]);
}
