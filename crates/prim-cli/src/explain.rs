//! Render `prim explain`'s output: the `.editorconfig` settings that apply
//! to one file, and where each came from (story C2).

use std::path::Path;

use crate::provenance::{ResolvedSetting, SettingOrigin};

/// Render `settings` for `path` as plain-text `key = value  (origin)` lines,
/// one per setting, in the order [`crate::editorconfig::Resolver::explain`]
/// produced them.
pub fn render(path: &Path, settings: &[ResolvedSetting]) -> String {
    let width = settings
        .iter()
        .map(|setting| setting.key.len())
        .max()
        .unwrap_or(0);

    let mut out = format!("{}\n", path.display());
    for setting in settings {
        out.push_str(&format!(
            "  {:width$} = {:<10} ({})\n",
            setting.key,
            setting.value,
            render_origin(&setting.origin),
            width = width
        ));
    }
    out
}

fn render_origin(origin: &SettingOrigin) -> String {
    match origin {
        SettingOrigin::Default => "prim's default".to_string(),
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
