//! Resolve prim's Markdown lint policy for one file: which tier applies, and
//! which rules that path excludes.
//!
//! Lives in the CLI crate because it reads `.editorconfig`; `prim-fmt` takes
//! the resolved policy as data and stays pure.

use std::collections::HashSet;
use std::path::Path;

use ec4rs::Properties;

use crate::editorconfig::{self, Resolver};
use crate::ui;

pub(crate) const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";
pub(crate) const MDLINT_DISABLE_KEY: &str = "prim_mdlint_disable";

/// Read `prim_mdlint_strict` out of already-resolved properties. Unset or
/// non-`true` values mean the floor tier.
pub(crate) fn strict_from(props: &Properties) -> bool {
    editorconfig::prim_bool_from(props, MDLINT_STRICT_KEY).unwrap_or(false)
}

/// The Markdown lint policy for one file: the tier that applies, and the rules
/// that path excludes from it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MdLintPolicy {
    /// `true` when `prim_mdlint_strict` selected the strict tier.
    pub strict: bool,
    /// Rule ids excluded by `prim_mdlint_disable`, uppercased. Subtract-only:
    /// these are removed from the tier's rule set and can never add to it.
    pub disabled: Vec<String>,
    /// Ids `prim_mdlint_disable` listed that name no rule prim runs in either
    /// tier, uppercased, in the order written. Resolving a policy never
    /// reports these itself — see [`UnknownRuleReporter`] for where and how
    /// often that happens.
    pub unknown: Vec<String>,
}

/// Parse `prim_mdlint_disable` out of already-resolved properties.
///
/// The value is a comma-separated list of rule ids. Entries are trimmed and
/// uppercased, then split into ids prim runs (kept, to exclude from the
/// tier) and ids it does not recognise (returned separately, dropped from
/// the exclusion set either way). This stays pure — reporting an
/// unrecognised id is a caller's job, not resolution's, so a warning fires
/// once per run rather than once per file (see [`UnknownRuleReporter`]).
///
/// Returns `(disabled, unknown)`.
fn disabled_from(props: &Properties) -> (Vec<String>, Vec<String>) {
    let Some(raw) = props.get_raw_for_key(MDLINT_DISABLE_KEY).into_option() else {
        return (Vec::new(), Vec::new());
    };

    let mut disabled = Vec::new();
    let mut unknown = Vec::new();
    for entry in raw
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        // `unset` is EditorConfig's own reserved word for "clear the
        // inherited value"; `none` is prim's own accepted spelling of the
        // same intent. Neither names a rule, so neither belongs in
        // `unknown` — that list feeds a warning that the id "is not a rule
        // prim runs", which would be a misleading way to describe a
        // deliberate clear.
        if entry.eq_ignore_ascii_case("unset") || entry.eq_ignore_ascii_case("none") {
            continue;
        }
        if prim_fmt::is_known_rule(entry) {
            disabled.push(entry.to_ascii_uppercase());
        } else {
            unknown.push(entry.to_ascii_uppercase());
        }
    }
    (disabled, unknown)
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve(path: &Path) -> MdLintPolicy {
    Resolver::new().resolve_mdlint_policy(path)
}

/// Assemble the whole Markdown lint policy from one file's resolved
/// `.editorconfig` properties. Pure — emits no warnings; see
/// [`UnknownRuleReporter`] for reporting `unknown` at the point of use.
pub(crate) fn policy_from(props: &Properties) -> MdLintPolicy {
    let (disabled, unknown) = disabled_from(props);
    MdLintPolicy {
        strict: strict_from(props),
        disabled,
        unknown,
    }
}

/// Warns about each unrecognised `prim_mdlint_disable` id once per run,
/// rather than once per file a matching glob happens to cover. A typo'd rule
/// id is a mistake in one `.editorconfig` line; it deserves one line of
/// stderr output, not a repeat for every file that inherits it.
#[derive(Default)]
pub struct UnknownRuleReporter {
    reported: HashSet<String>,
}

impl UnknownRuleReporter {
    /// A reporter that has not yet warned about anything.
    pub fn new() -> Self {
        Self::default()
    }

    /// Warn about each id in `unknown`, attributed to `path`, skipping any id
    /// this reporter already warned about earlier in the run.
    pub fn report(&mut self, path: &Path, unknown: &[String]) {
        for id in unknown {
            if self.reported.insert(id.clone()) {
                ui::warning(&format!(
                    "{}: {MDLINT_DISABLE_KEY} lists '{id}', which is not a rule prim runs — ignoring it",
                    path.display()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Test-only helper over the production resolver.
    fn strict_for(dir: &Path, relative: &str) -> bool {
        resolve(&dir.join(relative)).strict
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
        assert_eq!(policy.disabled, vec!["MD033", "MD041"]);
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

        assert_eq!(resolve(&dir.path().join("a.md")).disabled, vec!["MD033"]);
        assert_eq!(
            resolve(&dir.path().join("docs/g.md")).disabled,
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
            policy.disabled,
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
        assert!(resolve(&dir.path().join("a.md")).disabled.is_empty());
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
            assert!(policy.disabled.is_empty(), "value: {value:?}");
            assert!(
                policy.unknown.is_empty(),
                "value {value:?} must not be reported as an unrecognised rule id"
            );
        }
    }
}
