//! Exact, no-write formatting effect plans for delegated `fmt` and `fix`.

use serde::Serialize;
use sha2::{Digest, Sha256};

use super::load::FormattedFile;
use crate::run_diagnostic::{MachineDiagnostic, RunDiagnostic};
use prim_fmt::{FileKind, Indent, LineEnding, Style};

#[derive(Clone, Copy)]
pub(super) enum PlannedOperation {
    Fmt,
    Fix,
}

impl PlannedOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Fmt => "fmt",
            Self::Fix => "fix",
        }
    }
}

pub(super) fn render(
    operation: PlannedOperation,
    files: &[FormattedFile],
    errors: &[RunDiagnostic],
) -> String {
    let plan = EffectPlan {
        schema_version: 1,
        tool: Tool {
            name: "prim",
            version: env!("CARGO_PKG_VERSION"),
        },
        operation: operation.as_str(),
        effects: files
            .iter()
            .filter(|file| file.4 != file.5)
            .map(Effect::from_file)
            .collect(),
        errors: errors.iter().map(RunDiagnostic::machine).collect(),
    };

    serde_json::to_string_pretty(&plan).expect("effect-plan serialization should succeed") + "\n"
}

#[derive(Serialize)]
struct EffectPlan<'a> {
    schema_version: u8,
    tool: Tool<'a>,
    operation: &'a str,
    effects: Vec<Effect>,
    errors: Vec<MachineDiagnostic<'a>>,
}

#[derive(Serialize)]
struct Tool<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
struct Effect {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_encoded: Option<String>,
    kind: &'static str,
    operation: &'static str,
    before: Content,
    after: Content,
    configuration: Configuration,
}

impl Effect {
    fn from_file(file: &FormattedFile) -> Self {
        let (path, kind, style, _markdown_policy, original, formatted) = file;
        Self {
            path: crate::machine_path::display(path),
            path_encoded: crate::machine_path::encoded(path),
            kind: kind_name(*kind),
            operation: "replace_contents",
            before: Content::new(original),
            after: Content::new(formatted),
            configuration: Configuration::from(*style),
        }
    }
}

#[derive(Serialize)]
struct Content {
    sha256: String,
    bytes: usize,
}

impl Content {
    fn new(contents: &str) -> Self {
        Self {
            sha256: format!("{:x}", Sha256::digest(contents.as_bytes())),
            bytes: contents.len(),
        }
    }
}

#[derive(Serialize)]
struct Configuration {
    end_of_line: &'static str,
    trim_trailing_whitespace: bool,
    insert_final_newline: bool,
    indent_style: &'static str,
    indent_size: Option<usize>,
    max_line_length: Option<usize>,
}

impl From<Style> for Configuration {
    fn from(style: Style) -> Self {
        let (indent_style, indent_size) = match style.indent {
            Indent::Spaces(size) => ("space", Some(size)),
            Indent::Tab => ("tab", None),
        };
        Self {
            end_of_line: match style.end_of_line {
                LineEnding::Lf => "lf",
                LineEnding::CrLf => "crlf",
            },
            trim_trailing_whitespace: style.trim_trailing_whitespace,
            insert_final_newline: style.insert_final_newline,
            indent_style,
            indent_size,
            max_line_length: style.max_line_length,
        }
    }
}

fn kind_name(kind: FileKind) -> &'static str {
    match kind {
        FileKind::Markdown => "markdown",
        FileKind::Json => "json",
        FileKind::Jsonc => "jsonc",
        FileKind::Yaml => "yaml",
        FileKind::Toml => "toml",
        FileKind::Orphan => "orphan",
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;
    use crate::mdlint_policy::MdLintPolicy;

    #[test]
    fn an_undecodable_effect_path_keeps_its_exact_bytes() {
        let path = PathBuf::from(OsStr::from_bytes(b"caf\xe9.txt"));
        let files = vec![(
            path,
            FileKind::Orphan,
            Style::default(),
            MdLintPolicy::default(),
            "drift  \n".to_string(),
            "drift\n".to_string(),
        )];

        let rendered = render(PlannedOperation::Fmt, &files, &[]);
        let plan: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(plan["effects"][0]["path"], "caf�.txt");
        assert_eq!(plan["effects"][0]["path_encoded"], "caf%E9.txt");
    }
}
