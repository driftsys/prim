//! Parse one `.editorconfig` line the way `ec4rs` does.
//!
//! `ec4rs` resolves every setting prim reads, but its own per-line parser
//! (`parse_line` in its `src/linereader.rs`) lives in a module `lib.rs`
//! declares as `mod linereader;` — not `pub` — so prim cannot call it
//! directly. prim's `.editorconfig`-writing and `.editorconfig`-explaining
//! code ([`crate::init::sections`], [`crate::provenance`]) each hand-rolled
//! their own line scanner instead, and both diverged from what `ec4rs`
//! actually does with a section header's brackets and trailing comments
//! (issue #117). This module is the one place that scanning happens now.
//!
//! [`parse`] MUST stay a byte-for-byte reimplementation of `ec4rs` 1.2.0's
//! `parse_line`. The differential test below is what holds it to that: it
//! drives real `ec4rs` through its public `ConfigParser` and checks this
//! module's verdict against it for a table of line shapes. There is no
//! compiler check that would catch the two parsers drifting apart, so bump
//! that differential test, not just this file, whenever `ec4rs` is bumped —
//! re-read its `linereader.rs` and update both if it has changed.

/// What one `.editorconfig` line means, mirroring `ec4rs`'s private
/// `linereader::Line` plus an explicit `Invalid` case for a line `ec4rs`
/// would reject with `ParseError::InvalidLine`.
///
/// This is an enum rather than a `Result`: none of prim's call sites need to
/// propagate *why* a line failed to parse — `ec4rs`'s own `ParseError` carries
/// no detail beyond "invalid" — they only ever need to know whether a line is
/// the header/pair they are looking for, and if not, to skip it exactly as
/// prim's line scanners already tolerate (see `map.rs`'s note that prim's
/// scanner is more forgiving than an EditorConfig parser). Matching a plain
/// enum reads better at every call site than unwrapping a `Result` whose
/// `Err` payload nobody consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Line<'a> {
    /// A comment or a blank (after trimming) line.
    Nothing,
    /// A section header, e.g. `[*.md]`, holding the exact text between the
    /// brackets. Not trimmed: `[ *.md ]` yields `" *.md "`, spaces included,
    /// because `ec4rs` does not trim inside the brackets either.
    Section(&'a str),
    /// A `key = value` pair, both sides trimmed of the whitespace directly
    /// around the `=`.
    Pair(&'a str, &'a str),
    /// A line `ec4rs` would reject with `ParseError::InvalidLine`: `[]`, a
    /// line with no closing bracket, `key =` with nothing after the `=`, and
    /// so on.
    Invalid,
}

/// Parse one `.editorconfig` line, matching `ec4rs`'s private `parse_line`
/// exactly (see the module doc comment for why this cannot just call it).
///
/// A trailing comment is stripped only when the line contains a `]` and a
/// comment character (`;` or `#`) after it — `key = value # not a comment`
/// keeps the `# not a comment` as part of the value, because that is what
/// `ec4rs` does for a line with no `]` in it. This is a deliberate `ec4rs`
/// quirk prim must agree with, not a bug to fix here.
pub(crate) fn parse(line: &str) -> Line<'_> {
    let mut rest = line.trim_start();
    if rest.starts_with(is_comment) {
        return Line::Nothing;
    }

    // A trailing comment after a closing bracket is stripped; one before it,
    // or with no closing bracket on the line at all, is left in place.
    if let (Some(bracket), Some(comment)) = (rest.rfind(']'), rest.rfind(is_comment))
        && comment > bracket
    {
        rest = &rest[..comment];
    }

    rest = rest.trim_end();
    if rest.is_empty() {
        return Line::Nothing;
    }

    if let Some(glob) = rest.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return if glob.is_empty() {
            Line::Invalid
        } else {
            Line::Section(glob)
        };
    }

    match rest.split_once('=') {
        Some((key, value)) => {
            let key = key.trim_end();
            let value = value.trim_start();
            if key.is_empty() || value.is_empty() {
                Line::Invalid
            } else {
                Line::Pair(key, value)
            }
        }
        None => Line::Invalid,
    }
}

fn is_comment(c: char) -> bool {
    c == ';' || c == '#'
}

/// Parse the line at 0-based physical line `index`, matching what `ec4rs`'s
/// public `ConfigParser` — the thing that actually resolves prim's settings —
/// concludes about that line, rather than what its private `parse_line`
/// concludes about the text in isolation. Callers iterating a file's lines
/// must use this, not [`parse`].
///
/// The two differ only on a first line carrying a UTF-8 BOM (`U+FEFF`), where
/// `ec4rs` contradicts itself. `LineReader::next_line` strips the BOM before
/// classifying line 1, so a BOM'd `key = value` is a valid pair. But when
/// `ConfigParser` sees a section it re-reads the line through
/// `LineReader::reparse` (`parser.rs`), which calls `parse_line` on the stored
/// line with the BOM still on it — so a BOM'd `[*.md]` is not a header at all,
/// and the parser yields `ParseError::InvalidLine` for it.
///
/// prim mirrors that asymmetry deliberately. Treating a BOM'd first-line
/// header as a section would put prim back in the position issue #117 is
/// about: writing a key into a section the resolver does not see.
pub(crate) fn parse_at(line: &str, index: usize) -> Line<'_> {
    if index != 0 {
        return parse(line);
    }
    match line.strip_prefix('\u{FEFF}') {
        None => parse(line),
        // `reparse` re-reads the line with the BOM still attached, so a
        // header does not survive the round trip even though a pair does.
        Some(without_bom) => match parse(without_bom) {
            Line::Section(_) => Line::Invalid,
            other => other,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_glob_is_not_trimmed() {
        assert_eq!(parse("[ *.md ]"), Line::Section(" *.md "));
    }

    #[test]
    fn trailing_comment_after_a_closing_bracket_is_stripped() {
        assert_eq!(parse("[*.md] # c"), Line::Section("*.md"));
        assert_eq!(parse("[*.md] ; c"), Line::Section("*.md"));
        assert_eq!(parse("[*.md]# c"), Line::Section("*.md"));
    }

    #[test]
    fn a_comment_character_inside_the_brackets_is_kept() {
        // The last ']' comes after the last '#', so nothing is trimmed.
        assert_eq!(parse("[foo#bar]"), Line::Section("foo#bar"));
    }

    #[test]
    fn empty_brackets_are_invalid() {
        assert_eq!(parse("[]"), Line::Invalid);
    }

    #[test]
    fn leading_whitespace_before_a_header_is_still_a_header() {
        assert_eq!(parse("  [*.md]"), Line::Section("*.md"));
    }

    #[test]
    fn a_comment_before_a_header_is_not_a_header() {
        assert_eq!(parse("# [*.md]"), Line::Nothing);
        assert_eq!(parse("; [*.md]"), Line::Nothing);
    }

    #[test]
    fn an_unclosed_or_unopened_bracket_is_invalid() {
        assert_eq!(parse("[*.md"), Line::Invalid);
        assert_eq!(parse("*.md]"), Line::Invalid);
    }

    #[test]
    fn a_pair_with_no_closing_bracket_keeps_a_trailing_comment_in_the_value() {
        // ec4rs quirk: comment-stripping only fires when a ']' is present.
        assert_eq!(parse("key = value # c"), Line::Pair("key", "value # c"));
    }

    #[test]
    fn a_pair_whose_value_contains_a_closing_bracket_before_a_comment_is_stripped() {
        assert_eq!(parse("key = a]b # c"), Line::Pair("key", "a]b"));
    }

    #[test]
    fn a_key_or_value_alone_is_invalid() {
        assert_eq!(parse("key ="), Line::Invalid);
        assert_eq!(parse("= value"), Line::Invalid);
    }

    #[test]
    fn blank_and_whitespace_only_lines_are_nothing() {
        assert_eq!(parse(""), Line::Nothing);
        assert_eq!(parse("   "), Line::Nothing);
    }

    #[test]
    fn a_bom_hides_a_first_line_header_but_not_a_first_line_pair() {
        // `ec4rs` strips the BOM to classify line 1, then re-parses the
        // stored line without stripping it when it sees a section — so the
        // header is lost and the pair survives. See `parse_at`.
        assert_eq!(parse_at("\u{FEFF}[*.md]", 0), Line::Invalid);
        assert_eq!(parse_at("\u{FEFF}key = v", 0), Line::Pair("key", "v"));
        assert_eq!(parse_at("[*.md]", 0), Line::Section("*.md"));
        assert_eq!(
            parse_at("\u{FEFF}[*.md]", 1),
            Line::Invalid,
            "not the first line, so the BOM is never stripped either way"
        );
    }
}

/// Checks [`parse`] against real `ec4rs`, driven through its public
/// `ConfigParser` rather than its private `parse_line` (see the module doc
/// comment for why `parse_line` cannot be called directly). Read this
/// alongside that comment before touching either side; this is what holds
/// [`parse`] to actually mirroring `ec4rs`, not just this file's own idea of
/// what `ec4rs` does.
#[cfg(test)]
mod differential {
    use super::*;
    use std::path::Path;

    /// Which of the two ways a line can be checked against real `ec4rs`
    /// through its public API applies to it.
    #[derive(Clone, Copy)]
    enum Shape {
        /// Feed the candidate as the *entire* one-line `.editorconfig`
        /// document, so it is genuinely the file's first physical line, and
        /// see whether `ConfigParser` yields it as a section. This is also
        /// how a BOM-prefixed line is checked: it is only line 1 on this
        /// path.
        Header,
        /// Feed the candidate on the line after a `[*]` header, and inspect
        /// the resulting section's properties — `ConfigParser`'s preamble
        /// scan swallows a bare key/value pair silently (it only looks for
        /// `root`), so a pair can only be observed once it is inside a
        /// section.
        Body,
    }

    /// Probes chosen to expose the two ways a glob can differ without
    /// changing its displayed brackets: interior whitespace (`" a.md "`
    /// matches the glob from `[ *.md ]` but not from `[*.md]`) and a `#`/`;`
    /// inside the brackets that is not a comment (`"foo#bar"` matches the
    /// glob from `[foo#bar]` only when nothing was stripped from it).
    /// [`Section::applies_to`] is the only way the public API exposes a
    /// glob's identity at all, so these are the full extent of what a
    /// [`Line::Section`] payload can be checked against — see the note on
    /// [`Verdict::Section`].
    const PROBES: [&str; 3] = ["a.md", " a.md ", "foo#bar"];

    /// What either parser concluded about one line, reduced to what is
    /// actually observable through `ec4rs`'s public API.
    #[derive(Debug, PartialEq)]
    enum Verdict {
        Nothing,
        /// A section's identity is unobservable directly — there is no
        /// public accessor for the glob text a [`ec4rs::Section`] was built
        /// from — so it is represented here as which of [`PROBES`] it
        /// matches, which is everything [`ec4rs::Section::applies_to`] can
        /// tell us.
        Section([bool; PROBES.len()]),
        Pair(String, String),
        Invalid,
    }

    fn probe_results(section: &ec4rs::Section) -> [bool; PROBES.len()] {
        PROBES.map(|probe| section.applies_to(Path::new(probe)))
    }

    fn prim_verdict(line: &str, shape: Shape) -> Verdict {
        // A `Header`-shape case stands as the file's whole first line, so it
        // is index 0; a `Body`-shape case sits under a wrapping `[*]`, so it
        // is index 1. Only index 0 has any BOM handling to get wrong.
        let index = match shape {
            Shape::Header => 0,
            Shape::Body => 1,
        };
        match parse_at(line, index) {
            Line::Nothing => Verdict::Nothing,
            Line::Invalid => Verdict::Invalid,
            Line::Section(glob) => Verdict::Section(probe_results(&ec4rs::Section::new(glob))),
            Line::Pair(key, value) => Verdict::Pair(key.to_ascii_lowercase(), value.to_string()),
        }
    }

    /// `ec4rs`'s verdict on `line`, observed the way `shape` says it must be:
    /// as the whole document (so a section is visible through the iterator,
    /// and BOM-stripping — a `LineReader` behaviour, not `parse_line`'s —
    /// applies exactly as it would to a real file's first line), or as the
    /// single property line of a wrapping `[*]` section (so a pair's parsed
    /// key and value are visible through `Properties`, which nothing at the
    /// preamble level of `ConfigParser` exposes).
    fn ec4rs_verdict(line: &str, shape: Shape) -> Verdict {
        match shape {
            Shape::Header => {
                let document = format!("{line}\n");
                match ec4rs::ConfigParser::new_buffered(document.as_bytes()) {
                    Err(_) => Verdict::Invalid,
                    Ok(mut parser) => match parser.next() {
                        Some(Ok(section)) => Verdict::Section(probe_results(&section)),
                        Some(Err(_)) => Verdict::Invalid,
                        None => Verdict::Nothing,
                    },
                }
            }
            Shape::Body => {
                let document = format!("[*]\n{line}\n");
                let mut parser = ec4rs::ConfigParser::new_buffered(document.as_bytes())
                    .expect("the wrapping [*] header always parses on its own");
                match parser.next() {
                    Some(Err(_)) => Verdict::Invalid,
                    Some(Ok(section)) if section.props().is_empty() => Verdict::Nothing,
                    Some(Ok(section)) => {
                        let (key, value) = section
                            .props()
                            .iter()
                            .next()
                            .expect("just checked props is not empty");
                        Verdict::Pair(key.to_string(), value.to_string())
                    }
                    None => unreachable!("the wrapping [*] header is a section"),
                }
            }
        }
    }

    /// Every line shape issue #117 and its class of bug were reported over,
    /// paired with how to observe `ec4rs`'s real verdict on it.
    const CASES: &[(&str, Shape)] = &[
        ("[*.md]", Shape::Header),
        ("[ *.md ]", Shape::Header),
        ("[*.md] # c", Shape::Header),
        ("[*.md] ; c", Shape::Header),
        ("[*.md]# c", Shape::Header),
        ("[foo#bar]", Shape::Header),
        ("[]", Shape::Header),
        ("  [*.md]", Shape::Header),
        ("# [*.md]", Shape::Header),
        ("; [*.md]", Shape::Header),
        ("[*.md", Shape::Header),
        ("*.md]", Shape::Header),
        ("key = value", Shape::Body),
        ("key=value", Shape::Body),
        ("key = value # c", Shape::Body),
        ("key = a]b # c", Shape::Body),
        ("key =", Shape::Body),
        ("= value", Shape::Body),
        ("", Shape::Body),
        ("   ", Shape::Body),
        ("\u{FEFF}[*.md]", Shape::Header),
    ];

    #[test]
    fn prim_agrees_with_ec4rs_on_every_reported_line_shape() {
        for (line, shape) in CASES {
            assert_eq!(
                prim_verdict(line, *shape),
                ec4rs_verdict(line, *shape),
                "prim and ec4rs disagree on {line:?}"
            );
        }
    }
}
