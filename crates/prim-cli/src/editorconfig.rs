//! Resolve prim's [`Style`] from the `.editorconfig` cascade (FR-3).
//!
//! Walking the directory tree and reading files is I/O, so resolution lives in
//! the CLI crate; the engine consumes only the resolved [`Style`]. A missing
//! `.editorconfig` yields the built-in canonical style (FR-3.1); an unreadable
//! or malformed one falls back to it with a warning.

use std::path::Path;

use ec4rs::property::{
    EndOfLine, FinalNewline, IndentSize, IndentStyle, MaxLineLen, TabWidth, TrimTrailingWs,
};
use prim_fmt::{Indent, LineEnding, Style};

use crate::ui;

/// Resolve the [`Style`] that applies to `path` from the `.editorconfig`
/// cascade rooted at its directory.
pub fn resolve(path: &Path) -> Style {
    let mut cfg = match ec4rs::properties_of(path) {
        Ok(cfg) => cfg,
        Err(err) => {
            ui::warning(&format!(
                "{}: ignoring unreadable .editorconfig ({err}); using canonical style",
                path.display()
            ));
            return Style::default();
        }
    };
    cfg.use_fallbacks();

    let mut style = Style::default();
    if let Ok(eol) = cfg.get::<EndOfLine>() {
        // FR-2.3 carves out crlf only; deprecated bare `cr` falls back to LF.
        style.end_of_line = match eol {
            EndOfLine::CrLf => LineEnding::CrLf,
            EndOfLine::Lf | EndOfLine::Cr => LineEnding::Lf,
        };
    }
    if let Ok(TrimTrailingWs::Value(trim)) = cfg.get::<TrimTrailingWs>() {
        style.trim_trailing_whitespace = trim;
    }
    if let Ok(FinalNewline::Value(insert)) = cfg.get::<FinalNewline>() {
        style.insert_final_newline = insert;
    }
    style.indent = resolve_indent(&cfg, style.indent);
    if let Ok(max) = cfg.get::<MaxLineLen>() {
        style.max_line_length = match max {
            MaxLineLen::Value(n) => Some(n),
            MaxLineLen::Off => None,
        };
    }
    style
}

/// Map `indent_style` + `indent_size`/`tab_width` onto [`Indent`], keeping the
/// canonical default when `indent_style` is unset.
fn resolve_indent(cfg: &ec4rs::Properties, default: Indent) -> Indent {
    match cfg.get::<IndentStyle>() {
        Ok(IndentStyle::Tabs) => Indent::Tab,
        Ok(IndentStyle::Spaces) => Indent::Spaces(indent_width(cfg).unwrap_or(2)),
        Err(_) => default,
    }
}

fn indent_width(cfg: &ec4rs::Properties) -> Option<usize> {
    match cfg.get::<IndentSize>() {
        Ok(IndentSize::Value(n)) => Some(n),
        Ok(IndentSize::UseTabWidth) => match cfg.get::<TabWidth>() {
            Ok(TabWidth::Value(n)) => Some(n),
            _ => None,
        },
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Write `.editorconfig` `content` into a fresh temp dir and resolve the
    /// style for `relative` (a path under that dir).
    fn resolve_in(content: &str, relative: &str) -> (tempfile::TempDir, Style) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), content).unwrap();
        let style = resolve(&dir.path().join(relative));
        (dir, style)
    }

    #[test]
    fn no_editorconfig_yields_canonical_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(resolve(&dir.path().join("a.md")), Style::default());
    }

    #[test]
    fn honors_end_of_line_crlf() {
        let (_d, style) = resolve_in("root=true\n[*]\nend_of_line=crlf\n", "a.md");
        assert_eq!(style.end_of_line, LineEnding::CrLf);
    }

    #[test]
    fn honors_trim_and_final_newline_disabled() {
        let cfg = "root=true\n[*]\ntrim_trailing_whitespace=false\ninsert_final_newline=false\n";
        let (_d, style) = resolve_in(cfg, "a.md");
        assert!(!style.trim_trailing_whitespace);
        assert!(!style.insert_final_newline);
    }

    #[test]
    fn per_glob_sections_select_indent_and_width() {
        let cfg = "root=true\n[*]\nindent_style=space\nindent_size=2\n[*.md]\nmax_line_length=80\n[*.rs]\nindent_size=4\n";
        let (_d, md) = resolve_in(cfg, "doc.md");
        assert_eq!(md.indent, Indent::Spaces(2));
        assert_eq!(md.max_line_length, Some(80));
        let (_d2, rs) = resolve_in(cfg, "main.rs");
        assert_eq!(rs.indent, Indent::Spaces(4));
        assert_eq!(rs.max_line_length, None);
    }

    #[test]
    fn honors_tab_indent_style() {
        let (_d, style) = resolve_in("root=true\n[Makefile]\nindent_style=tab\n", "Makefile");
        assert_eq!(style.indent, Indent::Tab);
    }

    #[test]
    fn root_chain_stops_at_root_true() {
        // Inner root=true must shadow an outer config that sets crlf.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root=true\n[*]\nend_of_line=crlf\n",
        )
        .unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(
            sub.join(".editorconfig"),
            "root=true\n[*]\nend_of_line=lf\n",
        )
        .unwrap();
        let style = resolve(&sub.join("a.md"));
        assert_eq!(style.end_of_line, LineEnding::Lf);
    }
}
