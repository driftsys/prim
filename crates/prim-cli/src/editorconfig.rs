//! Resolve prim's [`Style`] from the `.editorconfig` cascade (FR-3).
//!
//! Walking the directory tree and reading files is I/O, so resolution lives in
//! the CLI crate; the engine consumes only the resolved [`Style`]. A missing
//! `.editorconfig` yields the built-in canonical style (FR-3.1); an unreadable
//! or malformed one falls back to it with a warning.
//!
//! [`Resolver`] caches the parsed cascade per directory: a repository with many
//! files under the same tree parses each `.editorconfig` once instead of once
//! per file. Per-glob sections still resolve per file (two files in one
//! directory can differ), so only the file-reading and parsing is cached, never
//! the glob matching — the result is byte-identical to an uncached resolve.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ec4rs::property::{
    EndOfLine, FinalNewline, IndentSize, IndentStyle, MaxLineLen, TabWidth, TrimTrailingWs,
};
use ec4rs::{ConfigFiles, Properties, PropertiesSource, Section};
use prim_fmt::{Indent, LineEnding, Style};

use crate::ui;

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

    /// Resolve the [`Style`] that applies to `path`, reusing the cached cascade
    /// for its directory when one is present.
    pub fn resolve(&mut self, path: &Path) -> Style {
        let dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let cascade = self
            .cache
            .entry(dir.clone())
            .or_insert_with(|| build_cascade(&dir));
        style_from(apply(cascade, path))
    }
}

/// One-shot resolution without caching — for `--stdin-filepath` (a single file)
/// and unit tests.
pub fn resolve(path: &Path) -> Style {
    Resolver::new().resolve(path)
}

/// Parse the `.editorconfig` cascade that applies to files in `dir`, once.
///
/// Uses [`ConfigFiles::open`] to find and open the applicable configs (walking
/// up to the nearest `root = true`), then collects each config's sections into
/// an owned, reusable list. The iteration yields configs root-first, which is
/// the order [`apply`] must replay. A config that cannot be read or parsed is
/// reported once (per directory, not per file) and dropped.
fn build_cascade(dir: &Path) -> Cascade {
    // A probe filename makes `ConfigFiles` walk upward from `dir`; the configs
    // found depend only on the directory, so the probe's name never matters.
    let probe = dir.join(".editorconfig");
    let files = match ConfigFiles::open(&probe, Option::<&Path>::None) {
        Ok(files) => files,
        Err(err) => {
            ui::warning(&format!(
                "{}: ignoring unreadable .editorconfig ({err}); using canonical style",
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
                    // A malformed config anywhere in the chain discards the whole
                    // cascade and falls back to canonical style, matching
                    // `ec4rs::properties_of` — which fails the entire resolution
                    // on a parse error, not just the offending file.
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
    cascade
}

/// Apply a cascade to `path`, mirroring `ec4rs`: each config's sections are
/// matched against the path made relative to that config's directory, applied
/// root-first so nearer configs win, then EditorConfig fallbacks are filled in.
fn apply(cascade: &Cascade, path: &Path) -> Properties {
    let mut props = Properties::new();
    for cfg in cascade {
        let relative = path.strip_prefix(&cfg.dir).unwrap_or(path);
        for section in &cfg.sections {
            // Applying a pre-parsed section only matches a glob and writes keys;
            // it cannot fail the way parsing can.
            let _ = section.apply_to(&mut props, relative);
        }
    }
    props.use_fallbacks();
    props
}

/// Map resolved [`Properties`] onto prim's [`Style`], keeping canonical defaults
/// for any key the cascade did not set.
fn style_from(cfg: Properties) -> Style {
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
fn resolve_indent(cfg: &Properties, default: Indent) -> Indent {
    match cfg.get::<IndentStyle>() {
        Ok(IndentStyle::Tabs) => Indent::Tab,
        Ok(IndentStyle::Spaces) => Indent::Spaces(indent_width(cfg).unwrap_or(2)),
        Err(_) => default,
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
        // A valid root config with a malformed child: `ec4rs::properties_of`
        // fails the entire resolution, so old prim fell back to canonical style.
        // The cache must match that — dropping just the broken child (and
        // keeping the root's `indent_style=tab`) would change output bytes.
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
        // Two files in the same directory share the cascade but keep their own
        // per-glob resolution.
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
        // One directory cached.
        assert_eq!(resolver.cache.len(), 1);
    }
}
