//! Resolve prim's Markdown lint policy for one file: which tier applies, and
//! which rules that path excludes.
//!
//! Lives in the CLI crate because it reads `.editorconfig`; `prim-fmt` takes
//! the resolved policy as data and stays pure.

use std::path::Path;

use ec4rs::Properties;

use crate::editorconfig::{self, Resolver};

pub(crate) const MDLINT_STRICT_KEY: &str = "prim_mdlint_strict";

/// Read `prim_mdlint_strict` out of already-resolved properties. Unset or
/// non-`true` values mean the floor tier.
pub(crate) fn strict_from(props: &Properties) -> bool {
    editorconfig::prim_bool_from(props, MDLINT_STRICT_KEY).unwrap_or(false)
}

/// One-shot resolution without caching — used by `lint --stdin-filepath` and
/// unit tests.
pub fn resolve_strict(path: &Path) -> bool {
    let mut resolver = Resolver::new();
    strict_from(&resolver.properties_for(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Test-only helper over the production resolver.
    fn strict_for(dir: &Path, relative: &str) -> bool {
        resolve_strict(&dir.join(relative))
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
}
