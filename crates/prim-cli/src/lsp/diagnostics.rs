//! Compute LSP diagnostics from prim's own lint findings — whitespace
//! hygiene (story B1) and Markdown content via rumdl (story G2) — reprojected
//! onto LSP's range/severity shape. Kept separate from [`super::server`] so
//! the message-dispatch state machine stays focused on one concern (story
//! G5's follow-up, issue #83).

use std::path::Path;

use crate::editorconfig::Resolver;
use crate::lsp::protocol::{self, Diagnostic};

/// Compute prim's own diagnostics for `text` at `path`/`kind`, resolving
/// `.editorconfig` style/strictness through `resolver` (the same cached
/// resolver `textDocument/formatting` uses). Structured formats (JSON/JSONC/
/// YAML/TOML) have no itemized findings yet — `prim lint`'s own coarser
/// format-drift finding for those kinds carries no position, so it is not
/// surfaced here either.
pub fn compute(
    resolver: &mut Resolver,
    path: &Path,
    kind: prim_fmt::FileKind,
    text: &str,
) -> Vec<Diagnostic> {
    match kind {
        prim_fmt::FileKind::Orphan => {
            let style = resolver.resolve(path);
            prim_fmt::hygiene_diagnostics(text, &style)
                .iter()
                .map(|diagnostic| Diagnostic {
                    range: protocol::point_range(text, diagnostic.line, diagnostic.column),
                    severity: protocol::SEVERITY_ERROR,
                    code: diagnostic.code.to_string(),
                    source: "prim",
                    message: diagnostic.message.clone(),
                })
                .collect()
        }
        prim_fmt::FileKind::Markdown => {
            let strict = resolver.resolve_mdlint_strict(path);
            prim_fmt::lint_markdown(text, strict)
                .iter()
                .map(|diagnostic| Diagnostic {
                    range: protocol::point_range(text, diagnostic.line, diagnostic.column),
                    severity: if diagnostic.is_error {
                        protocol::SEVERITY_ERROR
                    } else {
                        protocol::SEVERITY_WARNING
                    },
                    code: diagnostic.rule.clone(),
                    source: "prim",
                    message: diagnostic.message.clone(),
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn orphan_hygiene_findings_are_error_severity() {
        let mut resolver = Resolver::new();
        let diagnostics = compute(
            &mut resolver,
            Path::new("notes.txt"),
            prim_fmt::FileKind::Orphan,
            "a  \n",
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "hygiene::trailing-whitespace");
        assert_eq!(diagnostics[0].severity, protocol::SEVERITY_ERROR);
        assert_eq!(diagnostics[0].source, "prim");
    }

    #[test]
    fn markdown_findings_map_rumdl_severity() {
        let mut resolver = Resolver::new();
        let diagnostics = compute(
            &mut resolver,
            Path::new("README.md"),
            prim_fmt::FileKind::Markdown,
            "# Title\n\n![](hero.png)\n",
        );
        let md045 = diagnostics
            .iter()
            .find(|d| d.code == "MD045")
            .expect("MD045 reported");
        assert_eq!(md045.severity, protocol::SEVERITY_WARNING, "floor tier");
    }

    #[test]
    fn structured_formats_have_no_itemized_diagnostics_yet() {
        let mut resolver = Resolver::new();
        let diagnostics = compute(
            &mut resolver,
            Path::new("a.json"),
            prim_fmt::FileKind::Json,
            "{\"a\":1}\n",
        );
        assert!(diagnostics.is_empty());
    }
}
