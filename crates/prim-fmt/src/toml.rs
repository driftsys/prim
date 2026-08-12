//! TOML formatting (FR-1.5) via `taplo`.

use taplo::formatter::{Options, format_syntax};
use taplo::parser::parse;

use crate::hygiene::hygiene;
use crate::{FormatError, Indent, Style};

/// Format `source` as TOML under `style`, then apply whitespace hygiene for the
/// configured line ending and final newline.
///
/// taplo canonicalizes spacing/indentation and preserves comments, and with
/// `reorder_* = false` never reorders (FR-3.4/6.2). Malformed input is
/// detected via `parse().errors` and returned as [`FormatError::Parse`]
/// (FR-6.3) — taplo's formatter is otherwise lenient and would skip invalid
/// parts.
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let parsed = parse(source);
    if !parsed.errors.is_empty() {
        let message = parsed
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(FormatError::Parse(message));
    }

    let indent_string = match style.indent {
        Indent::Spaces(width) => " ".repeat(width),
        Indent::Tab => "\t".to_string(),
    };
    let options = Options {
        indent_string,
        column_width: style.max_line_length.unwrap_or(80),
        // Despite its name, this option does not put an inline table on more
        // than one line — taplo always writes `{ a = 1, b = 2 }` inline, so
        // inline-table style is preserved either way (FR-1.5). What it controls
        // is whether an over-width entry's "expand me" signal reaches nested
        // values. With `false`, an array inside an inline table can never
        // expand and `column_width` is silently ignored for it (issue #96).
        inline_table_expand: true,
        reorder_keys: false,          // FR-3.4
        reorder_arrays: false,        // FR-3.4
        reorder_inline_tables: false, // FR-3.4
        ..Options::default()
    };

    let printed = format_syntax(parsed.into_syntax(), options);
    Ok(hygiene(&printed, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineEnding;

    #[test]
    fn canonicalizes_key_value_spacing() {
        let out = format("a=1\n", &Style::default()).unwrap();
        assert!(out.contains("a = 1"), "{out:?}");
    }

    #[test]
    fn preserves_inline_table_style() {
        let out = format("x = {a=1}\n", &Style::default()).unwrap();
        assert!(out.contains("{ a = 1 }"), "inline table kept: {out:?}");
        assert!(!out.contains("[x]"), "not expanded to a table: {out:?}");
    }

    #[test]
    fn preserves_comments() {
        let out = format("# keep me\na = 1\n", &Style::default()).unwrap();
        assert!(out.contains("# keep me"), "{out:?}");
    }

    #[test]
    fn preserves_key_order() {
        let out = format("b = 2\na = 1\n", &Style::default()).unwrap();
        let b = out.find("b =").unwrap();
        let a = out.find("a =").unwrap();
        assert!(b < a, "order preserved (b before a): {out:?}");
    }

    #[test]
    fn indents_expanded_array_to_style_width() {
        // A tiny column width forces taplo to expand the array; each element is
        // then indented with the Style's indent string.
        let style = Style {
            indent: Indent::Spaces(4),
            max_line_length: Some(1),
            ..Style::default()
        };
        let out = format("arr = [1, 2]\n", &style).unwrap();
        assert!(out.contains("\n    1,"), "4-space array element: {out:?}");
    }

    #[test]
    fn expands_an_over_width_array_nested_in_an_inline_table() {
        // The Cargo-manifest case: a dependency whose feature list pushes the
        // entry past `max_line_length`. The array expands one element per line;
        // the inline table itself stays on one line (FR-1.5).
        let src = "[workspace.dependencies]\nweb-sys = { version = \"0.3.103\", features = [\"Document\", \"DomRect\", \"Element\", \"Headers\", \"HtmlCanvasElement\", \"Request\", \"RequestInit\", \"Response\", \"Window\"] }\n";
        let out = format(src, &Style::default()).unwrap();
        assert!(
            out.contains("features = [\n  \"Document\",\n"),
            "array expanded one element per line: {out:?}"
        );
        assert!(
            out.contains("web-sys = { version = \"0.3.103\", features = ["),
            "inline table stays on one line: {out:?}"
        );
        assert!(
            out.lines().all(|line| line.chars().count() <= 80),
            "no line exceeds the width: {out:?}"
        );
    }

    #[test]
    fn keeps_a_fitting_array_inline_inside_an_inline_table() {
        let src = "x = { version = \"1\", features = [\n  \"a\",\n  \"b\",\n] }\n";
        let out = format(src, &Style::default()).unwrap();
        assert_eq!(out, "x = { version = \"1\", features = [\"a\", \"b\"] }\n");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        let out = format("a = 1\n", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn invalid_toml_is_a_parse_error() {
        assert!(matches!(
            format("a = = 1\n", &Style::default()),
            Err(FormatError::Parse(_))
        ));
    }

    #[test]
    fn is_idempotent() {
        let src = "b=2\n# c\na   =   1\narr=[1,2]\n";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
