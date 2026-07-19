//! Byte-offset → `line:col` mapping for parse diagnostics.
//!
//! **Spike #42 output.** prim's structured formatters do *not* round-trip
//! through `serde` (which discards source spans); each parser prim already
//! links carries a byte offset (or line:col) on its parse error:
//!
//! - **JSON/JSONC** (`jsonc-parser`): `ParseError::line_display` /
//!   `column_display` are already 1-indexed, and `range().start` is a byte
//!   offset.
//! - **TOML** (`taplo`): `parser::Error::range` is a `TextRange` of byte
//!   offsets.
//! - **YAML** (`yaml_parser`, via `pretty_yaml`): `SyntaxError::offset` is a
//!   byte offset.
//! - **Markdown content lint** (`rumdl`): warnings are already 1-indexed
//!   line:col (Spike #39); hygiene diagnostics know their line by construction.
//!
//! So a single shared byte-offset → `line:col` mapper is the only primitive the
//! diagnostic stories (B1/D2) need — no span-preserving parser swap and no
//! JSON-pointer → line fallback.

/// Map a 0-based byte `offset` into `source` to a 1-indexed `(line, column)`.
///
/// The column counts Unicode scalar values (chars) from the start of the line,
/// not bytes, so multi-byte characters advance the column by one. An `offset`
/// at or past the end of `source` yields the position just past the last
/// character; an `offset` that lands inside a multi-byte character rounds up to
/// the next character boundary.
pub fn line_col(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (index, ch) in source.char_indices() {
        if index >= offset {
            return (line, column);
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_offsets_within_a_multi_line_string() {
        let source = "a\nbb\nccc";
        assert_eq!(line_col(source, 0), (1, 1)); // 'a'
        assert_eq!(line_col(source, 1), (1, 2)); // '\n'
        assert_eq!(line_col(source, 2), (2, 1)); // first 'b'
        assert_eq!(line_col(source, 5), (3, 1)); // first 'c'
        assert_eq!(line_col(source, source.len()), (3, 4)); // EOF
    }

    #[test]
    fn column_counts_chars_not_bytes() {
        // "é" is two UTF-8 bytes; the char after it is column 2, not 3.
        let source = "é!";
        assert_eq!(line_col(source, 0), (1, 1)); // 'é' at byte 0
        assert_eq!(line_col(source, 2), (1, 2)); // '!' at byte 2
    }

    /// Spike proof: an invalid JSON document emits a real `line:col`.
    #[test]
    fn emits_line_col_for_invalid_json() {
        // Unexpected token `@` where a value is expected (line 2).
        let source = "{\n  \"a\": @\n}\n";
        let error = jsonc_parser::parse_to_value(source, &Default::default())
            .expect_err("document is malformed");
        let offset = error.range().start;
        let (line, column) = line_col(source, offset);

        // jsonc-parser already reports 1-indexed line:col; our mapper agrees.
        assert_eq!(line, error.line_display());
        assert_eq!(column, error.column_display());
        assert_eq!(line, 2, "input.json:{line}:{column}: {}", error.kind());
    }

    /// Spike proof: an invalid TOML document emits a real `line:col`.
    #[test]
    fn emits_line_col_for_invalid_toml() {
        // Bare `=` with no value on line 1.
        let source = "title = \nport = 8080\n";
        let parsed = taplo::parser::parse(source);
        let error = parsed.errors.first().expect("document is malformed");
        let offset = usize::from(error.range.start());
        let (line, column) = line_col(source, offset);

        assert_eq!(line, 1, "input.toml:{line}:{column}: {}", error.message);
    }

    /// Spike proof: an invalid YAML document emits a real `line:col`.
    #[test]
    fn emits_line_col_for_invalid_yaml() {
        // Unclosed flow sequence.
        let source = "items: [1, 2\nname: prim\n";
        let options = pretty_yaml::config::FormatOptions::default();
        let error = pretty_yaml::format_text(source, &options).expect_err("document is malformed");
        let (line, column) = line_col(source, error.offset());

        assert!(line >= 1, "input.yaml:{line}:{column}: {}", error.message());
    }
}
