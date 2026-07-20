//! Track where a resolved `.editorconfig` setting's value came from — a
//! specific `.editorconfig` file/line/section, or prim's built-in default —
//! for `prim explain` (story C2).
//!
//! This is a distinct concern from [`crate::editorconfig`]'s job of merging
//! the cascade into one effective [`Style`]: `explain` needs the same merged
//! result, plus per-key provenance that `ec4rs`'s `track-source` feature
//! exposes via [`ec4rs::rawvalue::RawValue::source`].

use std::path::{Path, PathBuf};

use ec4rs::Properties;
use prim_fmt::{FileKind, Indent, LineEnding};

use crate::editorconfig::{self, MDLINT_STRICT_KEY, Resolver};

impl Resolver {
    /// Resolve every `.editorconfig`-recognized setting that applies to
    /// `kind` at `path`, alongside where its effective value came from.
    /// Settings irrelevant to `kind` (indent and max-line-length for
    /// [`FileKind::Orphan`], `prim_mdlint_strict` outside
    /// [`FileKind::Markdown`]) are omitted rather than shown as inapplicable.
    pub fn explain(&mut self, path: &Path, kind: FileKind) -> Vec<ResolvedSetting> {
        let props = self.properties_for(path);
        let style = editorconfig::style_from(props.clone());

        let mut settings = vec![
            ResolvedSetting {
                key: "end_of_line",
                value: match style.end_of_line {
                    LineEnding::Lf => "lf".to_string(),
                    LineEnding::CrLf => "crlf".to_string(),
                },
                origin: origin_of(&props, "end_of_line"),
            },
            ResolvedSetting {
                key: "trim_trailing_whitespace",
                value: style.trim_trailing_whitespace.to_string(),
                origin: origin_of(&props, "trim_trailing_whitespace"),
            },
            ResolvedSetting {
                key: "insert_final_newline",
                value: style.insert_final_newline.to_string(),
                origin: origin_of(&props, "insert_final_newline"),
            },
        ];

        if kind != FileKind::Orphan {
            settings.push(ResolvedSetting {
                key: "indent_style",
                value: match style.indent {
                    Indent::Spaces(_) => "space".to_string(),
                    Indent::Tab => "tab".to_string(),
                },
                origin: origin_of(&props, "indent_style"),
            });
            settings.push(ResolvedSetting {
                key: "indent_size",
                value: match style.indent {
                    Indent::Spaces(n) => n.to_string(),
                    Indent::Tab => "n/a (indent_style = tab)".to_string(),
                },
                origin: indent_size_origin(&props),
            });
            settings.push(ResolvedSetting {
                key: "max_line_length",
                value: style
                    .max_line_length
                    .map_or_else(|| "unset".to_string(), |n| n.to_string()),
                origin: origin_of(&props, "max_line_length"),
            });
        }

        if kind == FileKind::Markdown {
            settings.push(ResolvedSetting {
                key: MDLINT_STRICT_KEY,
                value: editorconfig::prim_bool_from(&props, MDLINT_STRICT_KEY)
                    .unwrap_or(false)
                    .to_string(),
                origin: origin_of(&props, MDLINT_STRICT_KEY),
            });
        }

        settings
    }
}

/// One `.editorconfig`-recognized setting resolved for a single file: its
/// effective value and where that value came from.
pub struct ResolvedSetting {
    /// The `.editorconfig` key name (for example `end_of_line`).
    pub key: &'static str,
    /// The effective value, formatted the way it would appear in
    /// `.editorconfig` (for example `lf`, `2`, `true`).
    pub value: String,
    /// Where `value` came from.
    pub origin: SettingOrigin,
}

/// Where a [`ResolvedSetting`]'s value came from.
pub enum SettingOrigin {
    /// No `.editorconfig` entry set this key; prim's built-in canonical
    /// default applies.
    Default,
    /// Set by an entry in `file` at `line` (1-indexed), inside the section
    /// whose header text is `section` when it could be recovered.
    EditorConfig {
        file: PathBuf,
        line: usize,
        section: Option<String>,
    },
}

/// Where `key`'s effective value in `props` came from: the `.editorconfig`
/// file/line that set it (via `ec4rs`'s `track-source` feature), or prim's
/// built-in default when the key was never set (including when
/// `Properties::use_fallbacks` synthesized a value with no source of its
/// own — see [`indent_size_origin`] for the one case prim attributes better).
fn origin_of(props: &Properties, key: &str) -> SettingOrigin {
    let raw = props.get_raw_for_key(key);
    if raw.into_option().is_none() {
        return SettingOrigin::Default;
    }
    match raw.source() {
        Some((file, line)) => SettingOrigin::EditorConfig {
            file: file.to_path_buf(),
            line,
            section: section_header_before(file, line),
        },
        None => SettingOrigin::Default,
    }
}

/// `indent_size`'s effective value may be synthesized from `tab_width` by
/// `Properties::use_fallbacks` (spec-mandated cross-derivation), which loses
/// direct source tracking. Attribute the setting to whichever of the two
/// keys was actually written in `.editorconfig`.
fn indent_size_origin(props: &Properties) -> SettingOrigin {
    match origin_of(props, "indent_size") {
        SettingOrigin::Default => origin_of(props, "tab_width"),
        direct => direct,
    }
}

/// Scan `file`'s text backward from `line` (1-indexed, inclusive) for the
/// nearest preceding `[glob]` section header, to show `prim explain` which
/// section set a value — `ec4rs` parses globs but does not expose their
/// source text, so this re-reads the (already-open, already-small)
/// `.editorconfig` file directly rather than duplicating glob parsing.
fn section_header_before(file: &Path, line: usize) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    text.lines()
        .take(line)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(str::trim)
        .find(|candidate| candidate.starts_with('[') && candidate.ends_with(']'))
        .map(str::to_string)
}

/// One-shot [`Resolver::explain`] without caching — `prim explain` only ever
/// resolves a single path per invocation, so there is no cascade to reuse.
pub fn explain(path: &Path, kind: FileKind) -> Vec<ResolvedSetting> {
    Resolver::new().explain(path, kind)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn explain_in(
        content: &str,
        relative: &str,
        kind: FileKind,
    ) -> (tempfile::TempDir, Vec<ResolvedSetting>) {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".editorconfig"), content).unwrap();
        let settings = explain(&dir.path().join(relative), kind);
        (dir, settings)
    }

    fn setting<'a>(settings: &'a [ResolvedSetting], key: &str) -> &'a ResolvedSetting {
        settings
            .iter()
            .find(|setting| setting.key == key)
            .unwrap_or_else(|| panic!("no {key} setting reported"))
    }

    #[test]
    fn unset_key_is_attributed_to_prims_default() {
        let dir = tempfile::tempdir().unwrap();
        let settings = explain(&dir.path().join("a.json"), FileKind::Json);
        assert!(matches!(
            setting(&settings, "end_of_line").origin,
            SettingOrigin::Default
        ));
    }

    #[test]
    fn set_key_is_attributed_to_its_editorconfig_file_and_line() {
        let (dir, settings) = explain_in(
            "root=true\n[*]\nend_of_line=crlf\n",
            "a.md",
            FileKind::Markdown,
        );
        let end_of_line = setting(&settings, "end_of_line");
        assert_eq!(end_of_line.value, "crlf");
        match &end_of_line.origin {
            SettingOrigin::EditorConfig {
                file,
                line,
                section,
            } => {
                assert_eq!(file, &dir.path().join(".editorconfig"));
                assert_eq!(*line, 3);
                assert_eq!(section.as_deref(), Some("[*]"));
            }
            SettingOrigin::Default => panic!("expected an EditorConfig origin"),
        }
    }

    #[test]
    fn indent_size_derived_from_tab_width_is_attributed_to_tab_width() {
        let (_dir, settings) =
            explain_in("root=true\n[*]\ntab_width=4\n", "a.json", FileKind::Json);
        assert!(matches!(
            setting(&settings, "indent_size").origin,
            SettingOrigin::EditorConfig { line: 3, .. }
        ));
    }

    #[test]
    fn orphan_kind_omits_indent_and_max_line_length() {
        let (_dir, settings) = explain_in(
            "root=true\n[*]\nindent_style=space\nindent_size=4\nmax_line_length=80\n",
            "NOTES.txt",
            FileKind::Orphan,
        );
        assert!(settings.iter().all(|setting| setting.key != "indent_style"));
        assert!(settings.iter().all(|setting| setting.key != "indent_size"));
        assert!(
            settings
                .iter()
                .all(|setting| setting.key != "max_line_length")
        );
    }

    #[test]
    fn only_markdown_reports_prim_mdlint_strict() {
        let (_dir, json_settings) = explain_in(
            "root=true\n[*]\nprim_mdlint_strict=true\n",
            "a.json",
            FileKind::Json,
        );
        assert!(
            json_settings
                .iter()
                .all(|setting| setting.key != "prim_mdlint_strict")
        );

        let (_dir, md_settings) = explain_in(
            "root=true\n[*]\nprim_mdlint_strict=true\n",
            "a.md",
            FileKind::Markdown,
        );
        let strict = setting(&md_settings, "prim_mdlint_strict");
        assert_eq!(strict.value, "true");
    }
}
