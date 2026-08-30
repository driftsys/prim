//! Resolve prim's [`Style`] and documented `prim_*` keys from the
//! `.editorconfig` cascade (FR-3).
//!
//! Resolution lives in the CLI crate because it does I/O; `prim-fmt` consumes
//! only the resolved [`Style`]. [`Resolver`] caches parsed cascades per
//! directory, but still resolves glob sections per file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ec4rs::property::{
    EndOfLine, FinalNewline, IndentSize, IndentStyle, MaxLineLen, TabWidth, TrimTrailingWs,
};
use ec4rs::{ConfigFiles, Properties, PropertiesSource, Section};
use prim_fmt::{Indent, LineEnding, Style};

use crate::ui;

pub(crate) mod ancestors;
pub(crate) mod line;

/// One parsed `.editorconfig` in a cascade: the directory that contains it
/// (globs match relative to it) and its sections, parsed once and reused.
struct CachedConfig {
    dir: PathBuf,
    sections: Vec<Section>,
}

/// A directory's `.editorconfig` cascade, ordered root-first so that nearer
/// configs, applied last, override farther ones (EditorConfig last-write-wins).
type Cascade = Vec<CachedConfig>;

/// Resolves [`Style`] from `.editorconfig`, caching each directory's parsed
/// cascade so a repository's files parse each `.editorconfig` only once.
#[derive(Default)]
pub struct Resolver {
    cache: HashMap<PathBuf, Cascade>,
}

impl Resolver {
    /// A resolver with an empty cache.
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn properties_for(&mut self, path: &Path) -> Properties {
        let dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let cascade = self
            .cache
            .entry(dir.clone())
            .or_insert_with(|| build_cascade(&dir));
        apply(cascade, path)
    }

    /// Resolve the [`Style`] that applies to `path`, reusing the cached cascade
    /// for its directory when one is present.
    pub fn resolve(&mut self, path: &Path) -> Style {
        style_from(self.properties_for(path))
    }

    /// Resolve the whole Markdown lint policy for `path`, reusing the cached
    /// cascade for its directory when one is present.
    pub fn resolve_mdlint_policy(&mut self, path: &Path) -> crate::mdlint_policy::MdLintPolicy {
        let props = self.properties_for(path);
        crate::mdlint_policy::policy_from(&props)
    }
}

/// Parse the `.editorconfig` cascade that applies to files in `dir`, once.
fn build_cascade(dir: &Path) -> Cascade {
    // The probe name is irrelevant; only the directory controls the walk.
    let probe = dir.join(".editorconfig");
    let files = match ConfigFiles::open(&probe, Option::<&Path>::None) {
        Ok(files) => files,
        // Not an unreadable `.editorconfig`: `ec4rs` skips a file it cannot
        // open and carries on, so one never reaches here. This is the walk
        // itself failing to start, which for a relative probe means the
        // working directory could not be determined.
        Err(err) => {
            ui::warning(&format!(
                "{}: cannot search for .editorconfig ({err}); using canonical style",
                dir.display()
            ));
            return Vec::new();
        }
    };

    let mut cascade = Vec::new();
    for mut file in files {
        let config_dir = file.path.parent().unwrap_or(Path::new("")).to_path_buf();
        let mut sections = Vec::new();
        for section in file.by_ref() {
            match section {
                Ok(section) => sections.push(section),
                Err(err) => {
                    // Match `ec4rs::properties_of`: one malformed config drops
                    // the whole cascade back to canonical style.
                    ui::warning(&format!(
                        "{}: ignoring malformed .editorconfig ({err}); using canonical style",
                        file.path.display()
                    ));
                    return Vec::new();
                }
            }
        }
        cascade.push(CachedConfig {
            dir: config_dir,
            sections,
        });
    }

    // `ec4rs` skipped any `.editorconfig` in this ancestry it could not open,
    // which silently changes what resolves here (#153). Named last, after the
    // section loop has had its chance to return: where a malformed section
    // drops the whole cascade, that outcome supersedes this one, and saying
    // "resolving as if it were absent" about one file while every file has
    // just stopped applying would be the more misleading of the two.
    ancestors::report_unopenable_in_cascade(dir);

    cascade
}

/// Apply a cascade to `path`, matching `ec4rs`.
fn apply(cascade: &Cascade, path: &Path) -> Properties {
    let mut props = Properties::new();
    for cfg in cascade {
        let relative = path.strip_prefix(&cfg.dir).unwrap_or(path);
        for section in &cfg.sections {
            let _ = section.apply_to(&mut props, relative);
        }
    }
    props.use_fallbacks();
    props
}

/// Map resolved [`Properties`] onto prim's [`Style`].
pub(crate) fn style_from(cfg: Properties) -> Style {
    let mut style = Style::default();
    if let Ok(eol) = cfg.get::<EndOfLine>() {
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

/// Read one of prim's documented `prim_*` `.editorconfig` keys as a bare
/// boolean: the raw value `true` (case-insensitively) resolves to `true`,
/// any other set value resolves to `false`, and an unset key resolves to
/// `None`. There is no generic custom-key lookup here — `key` is always one
/// of the specific, hard-coded `prim_*` keys a caller (e.g.
/// [`crate::mdlint_policy`]) already knows about; prim keeps a closed
/// allowlist of documented keys rather than exposing arbitrary custom
/// properties.
pub(crate) fn prim_bool_from(cfg: &Properties, key: &str) -> Option<bool> {
    cfg.get_raw_for_key(key)
        .into_option()
        .map(|value| value.eq_ignore_ascii_case("true"))
}

/// Map `indent_style` + `indent_size`/`tab_width` onto [`Indent`].
///
/// EditorConfig treats the two properties as independent, so `indent_size`
/// applies on its own (issue #104). An explicit `indent_style` is the stronger
/// statement and wins: `indent_style = tab` with `indent_size = 4` means
/// tab-indented, not four spaces. With neither key set, prim's canonical
/// default stands.
fn resolve_indent(cfg: &Properties, default: Indent) -> Indent {
    match cfg.get::<IndentStyle>() {
        Ok(IndentStyle::Tabs) => Indent::Tab,
        Ok(IndentStyle::Spaces) => Indent::Spaces(indent_width(cfg).unwrap_or(2)),
        // No style given: `indent_size = tab` still asks for tabs, a number
        // asks for that many spaces, and anything else leaves the default.
        Err(_) => match cfg.get::<IndentSize>() {
            Ok(IndentSize::UseTabWidth) => Indent::Tab,
            Ok(IndentSize::Value(width)) => Indent::Spaces(width),
            Err(_) => default,
        },
    }
}

fn indent_width(cfg: &Properties) -> Option<usize> {
    match cfg.get::<IndentSize>() {
        Ok(IndentSize::Value(n)) => Some(n),
        Ok(IndentSize::UseTabWidth) => match cfg.get::<TabWidth>() {
            Ok(TabWidth::Value(n)) => Some(n),
            _ => None,
        },
        Err(_) => None,
    }
}

/// One-shot resolution without caching — for `--stdin-filepath` (a single file)
/// and unit tests.
pub fn resolve(path: &Path) -> Style {
    Resolver::new().resolve(path)
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

    /// The reference resolution prim's cache must match: `ec4rs::properties_of`
    /// applied directly, mapped through the same [`style_from`].
    fn oracle(path: &Path) -> Style {
        let mut props = ec4rs::properties_of(path).expect("properties");
        props.use_fallbacks();
        style_from(props)
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
    fn indent_size_alone_sets_the_width() {
        // Issue #104: EditorConfig treats indent_size and indent_style as
        // independent properties, so a size with no style still applies.
        for cfg in [
            "root=true\n[*.toml]\nindent_size=4\n",
            "root=true\n[*]\nindent_size=4\n",
            "root=true\n[*.{toml,json}]\nindent_size=4\n",
            "root=true\n[x.toml]\nindent_size=4\n",
        ] {
            let (_d, style) = resolve_in(cfg, "x.toml");
            assert_eq!(style.indent, Indent::Spaces(4), "cfg: {cfg:?}");
        }
    }

    #[test]
    fn indent_size_tab_alone_indents_with_tabs() {
        let (_d, style) = resolve_in("root=true\n[*.toml]\nindent_size=tab\n", "x.toml");
        assert_eq!(style.indent, Indent::Tab);
    }

    #[test]
    fn indent_style_still_wins_over_a_bare_size() {
        // An explicit style is the stronger statement: tabs with a size of 4
        // means tab-indented, not four spaces.
        let (_d, style) = resolve_in(
            "root=true\n[*.toml]\nindent_style=tab\nindent_size=4\n",
            "x.toml",
        );
        assert_eq!(style.indent, Indent::Tab);
    }

    #[test]
    fn unset_indent_keys_keep_the_canonical_default() {
        let (_d, style) = resolve_in("root=true\n[*.toml]\nmax_line_length=100\n", "x.toml");
        assert_eq!(style.indent, Indent::Spaces(2));
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

    #[test]
    fn cached_resolution_matches_ec4rs_across_cases() {
        // A nested tree with an overriding child config and per-glob sections.
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root=true\n[*]\nindent_style=space\nindent_size=2\nend_of_line=lf\n[*.md]\nmax_line_length=80\n",
        )
        .unwrap();
        let sub = dir.path().join("pkg");
        fs::create_dir(&sub).unwrap();
        fs::write(
            sub.join(".editorconfig"),
            "[*.md]\nmax_line_length=100\n[*.toml]\nindent_size=4\n",
        )
        .unwrap();

        let mut resolver = Resolver::new();
        for rel in [
            "top.md",
            "top.toml",
            "top.yaml",
            "pkg/child.md",
            "pkg/child.toml",
            "pkg/child.json",
            "pkg/noext",
        ] {
            let path = dir.path().join(rel);
            assert_eq!(
                resolver.resolve(&path),
                oracle(&path),
                "cached resolve diverged from ec4rs for {rel}"
            );
            // A second call (now served from cache) must be identical.
            assert_eq!(
                resolver.resolve(&path),
                oracle(&path),
                "cache hit diverged for {rel}"
            );
        }
    }

    #[test]
    fn malformed_config_in_cascade_discards_the_whole_cascade() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root=true\n[*]\nindent_style=tab\n",
        )
        .unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join(".editorconfig"), "[*]\nindent_size=4\ngarbage\n").unwrap();

        let target = sub.join("a.md");
        assert!(
            ec4rs::properties_of(&target).is_err(),
            "precondition: ec4rs fails the whole resolution on the malformed child"
        );
        assert_eq!(
            resolve(&target),
            Style::default(),
            "a malformed config in the chain must fall back to canonical style, not the root's tab indent"
        );
    }

    #[test]
    fn cache_serves_same_directory_from_one_parse() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(".editorconfig"),
            "root=true\n[*.md]\nmax_line_length=80\n[*.toml]\nindent_style=tab\n",
        )
        .unwrap();
        let mut resolver = Resolver::new();
        let md = resolver.resolve(&dir.path().join("a.md"));
        let toml = resolver.resolve(&dir.path().join("b.toml"));
        assert_eq!(md.max_line_length, Some(80));
        assert_eq!(md.indent, Indent::Spaces(2));
        assert_eq!(toml.indent, Indent::Tab);
        assert_eq!(toml.max_line_length, None);
        assert_eq!(resolver.cache.len(), 1);
    }

    // --- Custom `prim_*` keys + glob-section precedence ---

    #[test]
    fn unknown_prim_keys_are_ignored_without_affecting_known_keys_or_style() {
        let (_d, style) = resolve_in(
            "root = true\n\
             [*.md]\n\
             prim_totally_made_up_key = true\n\
             prim_mdlint_strict = true\n\
             max_line_length = 100\n",
            "a.md",
        );
        let path = _d.path().join("a.md");
        assert!(crate::mdlint_policy::resolve(&path).strict);
        assert_eq!(style.max_line_length, Some(100));
        assert_eq!(
            style,
            Style {
                max_line_length: Some(100),
                ..Style::default()
            },
            "unknown prim_* keys are ignored; only standard keys shape Style"
        );
    }

    #[test]
    fn standard_and_documented_prim_keys_resolve_together_for_the_same_file() {
        let (_d, style) = resolve_in(
            "root = true\n\
             [*.md]\n\
             indent_style = space\n\
             indent_size = 4\n\
             end_of_line = crlf\n\
             insert_final_newline = false\n\
             trim_trailing_whitespace = false\n\
             max_line_length = 120\n\
             prim_mdlint_strict = true\n",
            "guide.md",
        );
        let path = _d.path().join("guide.md");
        assert!(crate::mdlint_policy::resolve(&path).strict);
        assert_eq!(
            style,
            Style {
                end_of_line: LineEnding::CrLf,
                trim_trailing_whitespace: false,
                insert_final_newline: false,
                indent: Indent::Spaces(4),
                max_line_length: Some(120),
            }
        );
    }
}
