//! YAML formatting (FR-1.4) via `pretty_yaml`.

use pretty_yaml::config::{FormatOptions, LayoutOptions, LineBreak};

use crate::hygiene::hygiene;
use crate::{FormatError, Indent, Style};

/// Format `source` as YAML under `style`, then apply whitespace hygiene for the
/// configured line ending and final newline.
///
/// `pretty_yaml` canonicalizes layout while preserving comments, anchors/aliases,
/// and block (literal `|` / folded `>`) scalar styles, and never reorders
/// (FR-1.4/3.4/6.2). It returns a `SyntaxError` on invalid YAML, mapped to
/// [`FormatError::Parse`] (FR-6.3). YAML forbids tab indentation, so
/// [`Indent::Tab`] falls back to two spaces.
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let options = FormatOptions {
        layout: LayoutOptions {
            print_width: style.max_line_length.unwrap_or(80),
            indent_width: match style.indent {
                Indent::Spaces(width) => width,
                Indent::Tab => 2,
            },
            line_break: LineBreak::Lf, // hygiene owns the configured line ending
        },
        ..FormatOptions::default()
    };
    match pretty_yaml::format_text(source, &options) {
        Ok(printed) => Ok(hygiene(&printed, style)),
        Err(err) => Err(FormatError::Parse(err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineEnding;

    #[test]
    fn canonicalizes_key_spacing() {
        let out = format("a:    1\n", &Style::default()).unwrap();
        assert!(out.contains("a: 1"), "{out:?}");
    }

    #[test]
    fn preserves_comments() {
        let out = format("# keep me\na: 1\n", &Style::default()).unwrap();
        assert!(out.contains("# keep me"), "{out:?}");
    }

    #[test]
    fn preserves_anchors_and_aliases() {
        let out = format("a: &id 1\nb: *id\n", &Style::default()).unwrap();
        assert!(out.contains("&id"), "anchor kept: {out:?}");
        assert!(out.contains("*id"), "alias kept: {out:?}");
    }

    #[test]
    fn preserves_block_scalar_style() {
        let src = "a: |\n  line one\n  line two\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains('|'), "block scalar indicator kept: {out:?}");
        assert!(out.contains("line one"), "block content kept: {out:?}");
    }

    #[test]
    fn preserves_key_order() {
        let out = format("b: 2\na: 1\n", &Style::default()).unwrap();
        let b = out.find("b:").unwrap();
        let a = out.find("a:").unwrap();
        assert!(b < a, "order preserved (b before a): {out:?}");
    }

    #[test]
    fn indents_nested_mapping_to_style_width() {
        let style = Style {
            indent: Indent::Spaces(4),
            ..Style::default()
        };
        let out = format("a:\n  b: 1\n", &style).unwrap();
        assert!(out.contains("\n    b:"), "4-space nested key: {out:?}");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        let out = format("a: 1\n", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn invalid_yaml_is_a_parse_error() {
        assert!(matches!(
            format("a: [1, 2\n", &Style::default()),
            Err(FormatError::Parse(_))
        ));
    }

    #[test]
    fn is_idempotent() {
        let src = "b:  2\n# c\na: 1\nlist:\n  - 1\n  - 2\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
