//! The prim formatting engine.
//!
//! prim is an opinionated, near-zero-config formatter for a repository's
//! connective tissue — Markdown, JSON/JSONC, YAML, TOML — plus whitespace
//! hygiene on a curated set of un-owned text files.
//!
//! # Scaffold stage
//!
//! At this stage [`format`] is the identity function: it returns its input
//! unchanged. It exists to prove the architecture end-to-end — the `prim`
//! binary depends on this crate and routes every operating mode through
//! [`format`]. The per-format parsers (Markdown, JSON, YAML, TOML) and the
//! whitespace-hygiene pass replace this no-op in later milestones.

/// Format `source` and return the formatted result.
///
/// # Scaffold stage
///
/// Returns `source` unchanged. Real formatting (structured per-format
/// canonicalisation and whitespace hygiene) lands in later milestones.
pub fn format(source: &str) -> String {
    source.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_is_identity_at_scaffold_stage() {
        let input = "# Title\n\n- item\n  trailing kept verbatim  \n";
        assert_eq!(format(input), input);
    }
}
