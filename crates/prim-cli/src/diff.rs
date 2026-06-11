//! Unified-diff rendering for `--diff` (FR-5.3).

use std::path::Path;

use similar::TextDiff;

/// Render a unified diff of `original` → `formatted` for `path`, with the
/// conventional `a/<path>` and `b/<path>` headers. Returns an empty string when
/// the two are identical.
pub fn unified(path: &Path, original: &str, formatted: &str) -> String {
    if original == formatted {
        return String::new();
    }
    let display = path.display();
    let diff = TextDiff::from_lines(original, formatted);
    let mut rendered = diff.unified_diff();
    rendered.header(&format!("a/{display}"), &format!("b/{display}"));
    rendered.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_inputs_render_nothing() {
        assert_eq!(unified(Path::new("a.json"), "x\n", "x\n"), "");
    }

    #[test]
    fn renders_headers_and_changed_lines() {
        let out = unified(Path::new("a.json"), "old line\n", "new line\n");
        assert!(out.contains("--- a/a.json"), "old header: {out:?}");
        assert!(out.contains("+++ b/a.json"), "new header: {out:?}");
        assert!(out.contains("-old line"), "removed line: {out:?}");
        assert!(out.contains("+new line"), "added line: {out:?}");
    }
}
