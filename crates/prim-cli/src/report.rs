//! Machine-readable report rendering for `fmt --check` and `lint`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::cli::OutputFormat;

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
    path: String,
    code: String,
    message: String,
    line: Option<usize>,
    column: Option<usize>,
}

impl Finding {
    /// Build an unpositioned finding for `path`.
    pub fn new(path: &Path, code: &str, message: &str) -> Self {
        Self {
            path: path.display().to_string(),
            code: code.to_string(),
            message: message.to_string(),
            line: None,
            column: None,
        }
    }

    /// Build a positioned finding from a structured hygiene diagnostic.
    pub fn diagnostic(path: &Path, diagnostic: &prim_fmt::Diagnostic) -> Self {
        Self {
            path: path.display().to_string(),
            code: diagnostic.code.to_string(),
            message: diagnostic.message.clone(),
            line: Some(diagnostic.line),
            column: Some(diagnostic.column),
        }
    }

    /// Build a positioned finding from a rumdl Markdown content diagnostic
    /// (story G2). The rule code is passed through verbatim (e.g. `"MD034"`).
    pub fn markdown(path: &Path, diagnostic: &prim_fmt::MdDiagnostic) -> Self {
        Self {
            path: path.display().to_string(),
            code: diagnostic.rule.clone(),
            message: diagnostic.message.clone(),
            line: Some(diagnostic.line),
            column: Some(diagnostic.column),
        }
    }
}

/// Render `findings` in the requested machine-readable `format`.
pub fn render(format: OutputFormat, mode: ReportMode, findings: &[Finding]) -> String {
    match format {
        OutputFormat::Json => render_json(mode, findings),
        OutputFormat::Sarif => render_sarif(findings),
    }
}

fn render_json(mode: ReportMode, findings: &[Finding]) -> String {
    let report = JsonReport {
        version: 1,
        mode: mode.as_str(),
        findings: findings
            .iter()
            .map(|finding| JsonFinding {
                path: &finding.path,
                code: &finding.code,
                message: &finding.message,
                line: finding.line,
                column: finding.column,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&report).expect("JSON report serialization should succeed") + "\n"
}

fn render_sarif(findings: &[Finding]) -> String {
    let rules = findings
        .iter()
        .fold(BTreeMap::new(), |mut rules, finding| {
            rules
                .entry(finding.code.as_str())
                .or_insert(&finding.message);
            rules
        })
        .into_iter()
        .map(|(code, message)| SarifRule {
            id: code,
            name: code,
            short_description: SarifMessage { text: message },
        })
        .collect();
    let results = findings
        .iter()
        .map(|finding| SarifResult {
            rule_id: &finding.code,
            level: "error",
            message: SarifMessage {
                text: &finding.message,
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation { uri: &finding.path },
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
        .collect();
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
}

#[derive(Serialize)]
struct JsonFinding<'a> {
    path: &'a str,
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
    locations: Vec<SarifLocation<'a>>,
}

#[derive(Serialize)]
struct SarifMessage<'a> {
    text: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation<'a> {
    physical_location: SarifPhysicalLocation<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation<'a> {
    artifact_location: SarifArtifactLocation<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<SarifRegion>,
}

#[derive(Serialize)]
struct SarifArtifactLocation<'a> {
    uri: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    fn json_report_omits_missing_location_fields() {
        let report = render(
            OutputFormat::Json,
            ReportMode::FmtCheck,
            &[Finding::new(
                Path::new("doc.json"),
                "format::drift",
                "would be reformatted",
            )],
        );
        let value: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(
            value,
            json!({
                "version": 1,
                "mode": "fmt-check",
                "findings": [
                    {
                        "path": "doc.json",
                        "code": "format::drift",
                        "message": "would be reformatted"
                    }
                ]
            })
        );
    }

    #[test]
    fn sarif_rules_are_deduplicated_by_code() {
        let findings = vec![
            Finding::new(Path::new("a.json"), "format::drift", "would be reformatted"),
            Finding::new(Path::new("b.json"), "format::drift", "would be reformatted"),
        ];
        let report = render(OutputFormat::Sarif, ReportMode::FmtCheck, &findings);
        let value: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(
            value["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(value["runs"][0]["results"].as_array().unwrap().len(), 2);
    }
}
