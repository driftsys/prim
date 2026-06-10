//! JSON / JSONC formatting (FR-1.2/1.3) via `dprint-plugin-json`.

use std::path::Path;

use dprint_plugin_json::configuration::{ConfigurationBuilder, TrailingCommaKind};
use dprint_plugin_json::format_text;

use crate::hygiene::hygiene;
use crate::{FormatError, Indent, Style};

/// Format `source` as JSONC under `style`, then apply whitespace hygiene for the
/// configured line ending and final newline.
///
/// JSON is a subset of JSONC, so both `FileKind::Json` and `FileKind::Jsonc`
/// route here; comments are preserved in either (FR-1.3). dprint's defaults give
/// one space after `:` and, with [`TrailingCommaKind::Never`], no trailing
/// commas (FR-1.2); it never reorders keys or elements (FR-3.4/6.2). Invalid
/// input yields [`FormatError::Parse`] (FR-6.3).
pub fn format(source: &str, style: &Style) -> Result<String, FormatError> {
    let mut builder = ConfigurationBuilder::new();
    builder
        .line_width(style.max_line_length.unwrap_or(80) as u32)
        .trailing_commas(TrailingCommaKind::Never);
    match style.indent {
        Indent::Spaces(width) => {
            builder.use_tabs(false).indent_width(width as u8);
        }
        Indent::Tab => {
            builder.use_tabs(true);
        }
    }
    let config = builder.build();

    // A synthetic `.jsonc` path selects dprint's comment-aware mode; no file is
    // read (dprint uses only the extension). The line ending is owned by
    // `hygiene`, so dprint's newline kind is left at its default.
    let printed = match format_text(Path::new("source.jsonc"), source, &config) {
        Ok(Some(text)) => text,
        Ok(None) => source.to_string(),
        Err(err) => return Err(FormatError::Parse(err.to_string())),
    };
    Ok(hygiene(&printed, style))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineEnding;

    #[test]
    fn one_space_after_colon() {
        let out = format("{\"a\":1}", &Style::default()).unwrap();
        assert!(out.contains("\"a\": 1"), "{out:?}");
    }

    #[test]
    fn reindents_nested_object_to_style_width() {
        let src = "{\n\"a\": {\n\"b\": 1\n}\n}";
        let out = format(src, &Style::default()).unwrap(); // 2-space
        assert!(out.contains("\n  \"a\":"), "top key 2 spaces: {out:?}");
        assert!(out.contains("\n    \"b\":"), "nested key 4 spaces: {out:?}");
    }

    #[test]
    fn drops_trailing_comma() {
        let out = format("[\n1,\n2,\n]", &Style::default()).unwrap();
        assert!(!out.contains("2,"), "trailing comma dropped: {out:?}");
        assert!(out.contains('2'), "value kept: {out:?}");
    }

    #[test]
    fn preserves_comments() {
        let src = "{\n// keep me\n\"a\": 1\n}";
        let out = format(src, &Style::default()).unwrap();
        assert!(out.contains("// keep me"), "{out:?}");
    }

    #[test]
    fn tab_indent_from_style() {
        let style = Style {
            indent: Indent::Tab,
            ..Style::default()
        };
        let out = format("{\n\"a\": 1\n}", &style).unwrap();
        assert!(out.contains("\n\t\"a\""), "{out:?}");
    }

    #[test]
    fn crlf_end_of_line_from_style() {
        let style = Style {
            end_of_line: LineEnding::CrLf,
            ..Style::default()
        };
        let out = format("{\n\"a\": 1\n}", &style).unwrap();
        assert!(out.contains("\r\n"), "{out:?}");
    }

    #[test]
    fn invalid_json_is_a_parse_error() {
        assert!(matches!(
            format("{", &Style::default()),
            Err(FormatError::Parse(_))
        ));
    }

    #[test]
    fn is_idempotent() {
        let src = "{\n\"a\":   1,\n  \"b\":2\n}";
        let once = format(src, &Style::default()).unwrap();
        let twice = format(&once, &Style::default()).unwrap();
        assert_eq!(once, twice);
    }
}
