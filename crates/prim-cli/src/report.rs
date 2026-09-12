//! Machine-readable report rendering for `fmt --check` and `lint`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::cli::OutputFormat;
use crate::run_diagnostic::{MachineDiagnostic, RunDiagnostic};

const SARIF_SCHEMA_URI: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/os/schemas/sarif-schema-2.1.0.json";
const SARIF_VERSION: &str = "2.1.0";
const TOOL_NAME: &str = "prim";
const TOOL_INFORMATION_URI: &str = "https://github.com/driftsys/prim";

/// The report-producing modes covered by story D2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReportMode {
    FmtCheck,
    Lint,
}

impl ReportMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::FmtCheck => "fmt-check",
            Self::Lint => "lint",
        }
    }
}

/// A machine-readable finding emitted by `fmt --check` or `lint`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    path: PathBuf,
    code: String,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
    is_error: bool,
}

impl Finding {
    /// Build an unpositioned finding for `path`.
    pub fn new(path: &Path, code: &str, message: &str) -> Self {
        Self {
            path: path.to_path_buf(),
            code: code.to_string(),
            message: message.to_string(),
            line: None,
            column: None,
            is_error: true,
        }
    }

    /// Build a positioned finding from a structured hygiene diagnostic.
    pub fn diagnostic(path: &Path, diagnostic: &prim_fmt::Diagnostic) -> Self {
        Self {
            path: path.to_path_buf(),
            code: diagnostic.code.to_string(),
            message: diagnostic.message.clone(),
            line: Some(diagnostic.line),
            column: Some(diagnostic.column),
            is_error: true,
        }
    }

    /// Build a positioned finding from a rumdl Markdown content diagnostic
    /// (story G2). The rule code is passed through verbatim (e.g. `"MD034"`).
    pub fn markdown(path: &Path, diagnostic: &prim_fmt::MdDiagnostic) -> Self {
        Self {
            path: path.to_path_buf(),
            code: diagnostic.rule.clone(),
            message: diagnostic.message.clone(),
            line: Some(diagnostic.line),
            column: Some(diagnostic.column),
            is_error: diagnostic.is_error,
        }
    }

    /// The path as a reader sees it. Lossy for a name that is not valid UTF-8,
    /// which is what [`Self::encoded_path`] exists to sit beside.
    fn display_path(&self) -> String {
        crate::machine_path::display(&self.path)
    }

    /// The path's bytes percent-encoded, and `None` when there is nothing to
    /// add: the path is valid UTF-8, or the platform has no bytes to offer.
    ///
    /// Returning `None` for the decodable case is what keeps this additive:
    /// every path on a platform whose filenames are Unicode, and nearly every
    /// path elsewhere, renders exactly as it did before (#172).
    fn encoded_path(&self) -> Option<String> {
        crate::machine_path::encoded(&self.path)
    }
}

/// Render `findings` in the requested machine-readable `format`.
#[cfg(test)]
pub fn render(format: OutputFormat, mode: ReportMode, findings: &[Finding]) -> String {
    render_with_errors(format, mode, findings, &[])
}

/// Render findings plus operational failures in the requested machine format.
pub(crate) fn render_with_errors(
    format: OutputFormat,
    mode: ReportMode,
    findings: &[Finding],
    errors: &[RunDiagnostic],
) -> String {
    match format {
        OutputFormat::Json => render_json(mode, findings, errors),
        OutputFormat::Sarif => render_sarif(findings, errors),
    }
}

fn render_json(mode: ReportMode, findings: &[Finding], errors: &[RunDiagnostic]) -> String {
    let report = JsonReport {
        version: 1,
        mode: mode.as_str(),
        findings: findings
            .iter()
            .map(|finding| JsonFinding {
                path: finding.display_path(),
                path_encoded: finding.encoded_path(),
                code: &finding.code,
                message: &finding.message,
                line: finding.line,
                column: finding.column,
            })
            .collect(),
        errors: errors.iter().map(RunDiagnostic::machine).collect(),
    };

    serde_json::to_string_pretty(&report).expect("JSON report serialization should succeed") + "\n"
}

fn render_sarif(findings: &[Finding], errors: &[RunDiagnostic]) -> String {
    let rules = errors
        .iter()
        .fold(
            findings.iter().fold(BTreeMap::new(), |mut rules, finding| {
                rules
                    .entry(finding.code.as_str())
                    .or_insert(finding.message.as_str());
                rules
            }),
            |mut rules, error| {
                rules.entry(error.code).or_insert(error.description());
                rules
            },
        )
        .into_iter()
        .map(|(code, message)| SarifRule {
            id: code,
            name: code,
            short_description: SarifMessage { text: message },
        })
        .collect();
    let mut results = findings
        .iter()
        .map(|finding| SarifResult {
            rule_id: &finding.code,
            level: if finding.is_error { "error" } else { "warning" },
            message: SarifMessage {
                text: &finding.message,
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        uri: finding
                            .encoded_path()
                            .unwrap_or_else(|| finding.display_path()),
                    },
                    region: match (finding.line, finding.column) {
                        (Some(line), Some(column)) => Some(SarifRegion {
                            start_line: line,
                            start_column: column,
                        }),
                        _ => None,
                    },
                },
            }],
        })
        .collect::<Vec<_>>();
    results.extend(errors.iter().map(|error| {
        SarifResult {
            rule_id: error.code,
            level: "error",
            message: SarifMessage {
                text: &error.message,
            },
            locations: error
                .path
                .as_deref()
                .map(|path| {
                    vec![SarifLocation {
                        physical_location: SarifPhysicalLocation {
                            artifact_location: SarifArtifactLocation {
                                uri: crate::machine_path::encoded(path)
                                    .unwrap_or_else(|| crate::machine_path::display(path)),
                            },
                            region: None,
                        },
                    }]
                })
                .unwrap_or_default(),
        }
    }));
    let report = SarifLog {
        schema: SARIF_SCHEMA_URI,
        version: SARIF_VERSION,
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: TOOL_NAME,
                    version: env!("CARGO_PKG_VERSION"),
                    information_uri: TOOL_INFORMATION_URI,
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&report).expect("SARIF report serialization should succeed") + "\n"
}

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u8,
    mode: &'a str,
    findings: Vec<JsonFinding<'a>>,
    errors: Vec<MachineDiagnostic<'a>>,
}

#[derive(Serialize)]
struct JsonFinding<'a> {
    path: String,
    /// Present only for a path that is not valid UTF-8 (#172).
    #[serde(skip_serializing_if = "Option::is_none")]
    path_encoded: Option<String>,
    code: &'a str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<usize>,
}

#[derive(Serialize)]
struct SarifLog<'a> {
    #[serde(rename = "$schema")]
    schema: &'a str,
    version: &'a str,
    runs: Vec<SarifRun<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun<'a> {
    tool: SarifTool<'a>,
    results: Vec<SarifResult<'a>>,
}

#[derive(Serialize)]
struct SarifTool<'a> {
    driver: SarifDriver<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver<'a> {
    name: &'a str,
    version: &'a str,
    information_uri: &'a str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rules: Vec<SarifRule<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule<'a> {
    id: &'a str,
    name: &'a str,
    short_description: SarifMessage<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult<'a> {
    rule_id: &'a str,
    level: &'a str,
    message: SarifMessage<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
struct SarifMessage<'a> {
    text: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<SarifRegion>,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
}

#[cfg(test)]
mod tests;
