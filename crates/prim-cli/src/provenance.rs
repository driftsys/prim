//! Track where a resolved `.editorconfig` setting's value came from — a
//! specific `.editorconfig` file/line/section, or prim's built-in default —
//! for `prim explain` (story C2).
//!
//! This is a distinct concern from [`crate::editorconfig`]'s job of merging
//! the cascade into one effective [`prim_fmt::Style`]: `explain` needs the
//! same merged result, plus per-key provenance that `ec4rs`'s `track-source`
//! feature exposes via [`ec4rs::rawvalue::RawValue::source`].

use std::path::{Path, PathBuf};

use ec4rs::Properties;
use prim_fmt::{FileKind, Indent, LineEnding};

use crate::editorconfig::line;
use crate::editorconfig::{self, Resolver};
use crate::mdlint_policy::{
    self, MDLINT_DISABLE_KEY, MDLINT_REPORT_LINE_LENGTH_KEY, MDLINT_STRICT_KEY, MdLintPolicy,
};

impl Resolver {
    /// Resolve every `.editorconfig`-recognized setting that applies to
    /// `kind` at `path`, alongside where its effective value came from.
    /// Settings irrelevant to `kind` (indent and max-line-length for
    /// [`FileKind::Orphan`], every `prim_mdlint_*` key outside
    /// [`FileKind::Markdown`]) are omitted rather than shown as inapplicable.
    pub fn explain(&mut self, path: &Path, kind: FileKind) -> Explanation {
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

        if kind != FileKind::Markdown {
            return Explanation {
                settings,
                mdlint_policy: None,
            };
        }

        let policy = mdlint_policy::policy_from(&props);
        settings.push(ResolvedSetting {
            key: MDLINT_STRICT_KEY,
            value: policy.strict.to_string(),
            origin: origin_of(&props, MDLINT_STRICT_KEY),
        });
        settings.push(ResolvedSetting {
            key: MDLINT_REPORT_LINE_LENGTH_KEY,
            value: policy.report_line_length.is_some().to_string(),
            origin: origin_of(&props, MDLINT_REPORT_LINE_LENGTH_KEY),
        });
        let disable_origin = origin_of(&props, MDLINT_DISABLE_KEY);
        settings.push(ResolvedSetting {
            key: MDLINT_DISABLE_KEY,
            value: disable_value(&policy, &disable_origin),
            origin: disable_origin,
        });

        Explanation {
            settings,
            mdlint_policy: Some(policy),
        }
    }
}

/// What `prim_mdlint_disable` excludes for this file, rendered the way it
/// would be written back.
///
/// An empty exclusion set has two very different origins, and neither may
/// print an empty value next to a real `.editorconfig` line — a blank value
/// beside a real origin reads as prim having lost the value:
///
/// - the key was never set anywhere in the cascade, which is `unset`;
/// - the key was set and resolved to nothing — a deliberate `= none` or
///   `= unset`, or a list whose every id was unrecognised (reported
///   separately on stderr). prim applies no exclusions in either case, which
///   is exactly what `none` says.
fn disable_value(policy: &MdLintPolicy, origin: &SettingOrigin) -> String {
    if !policy.disabled.is_empty() {
        return policy.disabled.join(", ");
    }
    match origin {
        SettingOrigin::Default => "unset".to_string(),
        SettingOrigin::EditorConfig { .. } => "none".to_string(),
    }
}

/// One file's resolved settings, plus the Markdown lint policy they were read
/// from when the file is Markdown.
///
/// The policy travels with the answer rather than being reported here: an
/// unrecognised `prim_mdlint_disable` id is warned about once per run, and a
/// query that writes to stderr on every call cannot honour that (see
/// [`mdlint_policy::UnknownRuleReporter`]).
pub struct Explanation {
    pub settings: Vec<ResolvedSetting>,
    pub mdlint_policy: Option<MdLintPolicy>,
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SettingOrigin {
    /// No `.editorconfig` entry set this key; prim's built-in canonical
    /// default applies.
    #[default]
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
pub(crate) fn origin_of(props: &Properties, key: &str) -> SettingOrigin {
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

/// Scan `file`'s text backward from `line_number` (1-indexed, inclusive) for
/// the nearest preceding section header, to show `prim explain` which
/// section set a value — `ec4rs` parses globs but does not expose their
/// source text, so this re-reads the (already-open, already-small)
/// `.editorconfig` file directly rather than duplicating glob parsing.
///
/// Displays the header reconstructed from what `ec4rs` actually resolved the
/// glob to — `[` + the exact bracket contents (whitespace kept, any trailing
/// comment stripped) + `]` — rather than the raw source line. `explain`'s job
/// is to say what governed the resolution: for a header like
/// `[docs/**.md] # book docs`, the raw line mixes the glob with a trailing
/// comment that plays no part in matching. For `[ *.md ]`, the reconstruction
/// is byte-identical to the raw line — whitespace is kept exactly, so there
/// is nothing to strip — and the choice only matters for the comment case.
/// The line number `explain` already prints beside this is what sends the
/// reader to the exact line to edit; this reconstruction is what tells them
/// what it means.
fn section_header_before(file: &Path, line_number: usize) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    text.lines()
        .enumerate()
        .take(line_number)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .find_map(
            |(index, candidate)| match line::parse_at(candidate, index) {
                line::Line::Section(glob) => Some(format!("[{glob}]")),
                _ => None,
            },
        )
}

/// Where a setting was written, as `file:line [section]`, for a message that
/// has to send the reader to the line they must edit. Empty when the value
/// has no `.editorconfig` origin to name.
pub(crate) fn location_of(origin: &SettingOrigin) -> String {
    match origin {
        SettingOrigin::Default => String::new(),
        SettingOrigin::EditorConfig {
            file,
            line,
            section,
        } => match section {
            Some(section) => format!("{}:{line} {section}", file.display()),
            None => format!("{}:{line}", file.display()),
        },
    }
}

/// One-shot [`Resolver::explain`] without caching — `prim explain` only ever
/// resolves a single path per invocation, so there is no cascade to reuse.
pub fn explain(path: &Path, kind: FileKind) -> Explanation {
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
        let settings = explain(&dir.path().join(relative), kind).settings;
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
        let settings = explain(&dir.path().join("a.json"), FileKind::Json).settings;
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
