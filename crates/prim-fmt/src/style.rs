//! Resolved formatting style (FR-3): the single source of configuration the
//! engine consumes. Built by `prim-cli` from `.editorconfig` and passed into
//! [`crate::format`]. [`Style::default`] is prim's built-in canonical style
//! (FR-3.1), applied when no `.editorconfig` is present.

/// The line ending prim emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// `\n` — prim's canonical default.
    Lf,
    /// `\r\n` — only when `.editorconfig` sets `end_of_line = crlf` (FR-2.3).
    CrLf,
}

impl LineEnding {
    /// The byte sequence for this line ending.
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::CrLf => "\r\n",
        }
    }
}

/// Indentation unit. Carried for the per-format parsers (FR-1, #9–12); the
/// whitespace-hygiene pass does not consume it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Indent {
    /// `indent_style = space` with the given `indent_size`.
    Spaces(usize),
    /// `indent_style = tab`.
    Tab,
}

/// The resolved canonical style for one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Line ending to emit (FR-2.3).
    pub end_of_line: LineEnding,
    /// Strip trailing whitespace from each line (FR-2.1).
    pub trim_trailing_whitespace: bool,
    /// When true, end content with exactly one final line ending; when false,
    /// strip any final line ending (FR-2.2 / `insert_final_newline`).
    pub insert_final_newline: bool,
    /// Indentation unit (carried for FR-1 parsers; unused by hygiene).
    pub indent: Indent,
    /// Hard-wrap width (carried for FR-1 Markdown; unused by hygiene). `None`
    /// means unset — the Markdown formatter falls back to 80.
    pub max_line_length: Option<usize>,
}

impl Default for Style {
    /// prim's built-in canonical style (FR-3.1): LF endings, trailing
    /// whitespace stripped, exactly one final newline, two-space indent.
    fn default() -> Self {
        Style {
            end_of_line: LineEnding::Lf,
            trim_trailing_whitespace: true,
            insert_final_newline: true,
            indent: Indent::Spaces(2),
            max_line_length: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_the_canonical_style() {
        let s = Style::default();
        assert_eq!(s.end_of_line, LineEnding::Lf);
        assert!(s.trim_trailing_whitespace);
        assert!(s.insert_final_newline);
        assert_eq!(s.indent, Indent::Spaces(2));
        assert_eq!(s.max_line_length, None);
    }

    #[test]
    fn line_ending_bytes() {
        assert_eq!(LineEnding::Lf.as_str(), "\n");
        assert_eq!(LineEnding::CrLf.as_str(), "\r\n");
    }
}
