//! Resolve prim's Markdown lint policy for one file: which tier applies, and
//! which rules that path excludes.
//!
//! Lives in the CLI crate because it reads `.editorconfig`; `prim-fmt` takes
//! the resolved policy as data and stays pure.

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
}

/// Parse `prim_mdlint_disable` out of already-resolved properties.
///
/// The value is a comma-separated list of rule ids. Entries are trimmed and
/// uppercased. An id prim does not run is reported once and dropped, so a typo
/// is visible rather than silently excluding nothing; per AD-0007 that warning
/// does not raise the exit code.
fn disabled_from(props: &Properties, path: &Path) -> Vec<String> {
    let Some(raw) = props.get_raw_for_key(MDLINT_DISABLE_KEY).into_option() else {
        return Vec::new();
    };

    raw.split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .filter_map(|entry| {
            if prim_fmt::is_known_rule(entry) {
                Some(entry.to_ascii_uppercase())
            } else {
                ui::warning(&format!(
                    "{}: {MDLINT_DISABLE_KEY} lists '{entry}', which is not a rule prim runs — ignoring it",
                    path.display()
                ));
                None
            }
        })
        .collect()
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve(path: &Path) -> MdLintPolicy {
    Resolver::new().resolve_mdlint_policy(path)
}

pub(crate) fn policy_from(props: &Properties, path: &Path) -> MdLintPolicy {
    MdLintPolicy {
        strict: strict_from(props),
        disabled: disabled_from(props, path),
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
            "unknown ids are reported and dropped"
        );
    }

    #[test]
    fn an_empty_or_unset_key_disables_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), "root = true\n[*.md]\n").unwrap();
        assert!(resolve(&dir.path().join("a.md")).disabled.is_empty());
    }
}
