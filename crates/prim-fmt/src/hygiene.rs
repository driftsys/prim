//! Whitespace hygiene (FR-2.1/2.2/2.3): the format-agnostic pass applied to
//! every file prim owns.

/// Apply whitespace hygiene to `source`:
///
/// - normalise CRLF and lone CR line endings to LF (FR-2.3),
/// - strip trailing whitespace from each line (FR-2.1),
/// - end non-empty content with exactly one line-feed; leave empty (or
///   whitespace-only) content empty (FR-2.2).
///
/// The pass is idempotent (FR-6.1).
pub fn hygiene(source: &str) -> String {
    // FR-2.3: normalise CRLF and lone CR to LF.
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");

    // FR-2.1: strip trailing whitespace from each line.
    let mut out = String::with_capacity(normalized.len());
    for line in normalized.split('\n') {
        out.push_str(line.trim_end());
        out.push('\n');
    }

    // FR-2.2: exactly one final LF for non-empty content; empty stays empty.
    let body = out.trim_end_matches('\n');
    if body.is_empty() {
        String::new()
    } else {
        let mut result = String::with_capacity(body.len() + 1);
        result.push_str(body);
        result.push('\n');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_trailing_whitespace_per_line() {
        assert_eq!(hygiene("a  \nb\t\n"), "a\nb\n");
    }

    #[test]
    fn preserves_leading_and_inner_whitespace() {
        assert_eq!(hygiene("  a  b  \n"), "  a  b\n");
    }

    #[test]
    fn ensures_single_final_newline() {
        assert_eq!(hygiene("a"), "a\n");
        assert_eq!(hygiene("a\n\n\n"), "a\n");
    }

    #[test]
    fn normalizes_crlf_and_cr_to_lf() {
        assert_eq!(hygiene("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn empty_or_whitespace_only_stays_empty() {
        assert_eq!(hygiene(""), "");
        assert_eq!(hygiene("   \n  \n"), "");
    }

    #[test]
    fn is_idempotent() {
        for input in ["a  \r\nb\n\n", "", "x", "  keep\nlead  \n", "   \n"] {
            let once = hygiene(input);
            assert_eq!(hygiene(&once), once, "not idempotent for {input:?}");
        }
    }
}
