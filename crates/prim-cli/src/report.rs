//! Machine-readable report rendering for `fmt --check` and `lint`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
        self.path.display().to_string()
    }

    /// The path's bytes percent-encoded, and `None` when there is nothing to
    /// add: the path is valid UTF-8, or the platform has no bytes to offer.
    ///
    /// Returning `None` for the decodable case is what keeps this additive:
    /// every path on a platform whose filenames are Unicode, and nearly every
    /// path elsewhere, renders exactly as it did before (#172).
    fn encoded_path(&self) -> Option<String> {
        if self.path.to_str().is_some() {
            return None;
        }

        Some(percent_encode(path_bytes(&self.path)?))
    }
}

/// The bytes a path is made of, where the platform has them.
///
/// On unix a path is an arbitrary byte string, so the bytes are the name.
/// Elsewhere a path is Unicode: one that is not valid UTF-8 cannot be
/// represented at all, so there is no exact form to offer and the caller adds
/// no field. Encoding `Path::display`'s output there would percent-encode the
/// U+FFFD this split exists to avoid, and promise a round-trip it cannot keep.
#[cfg(unix)]
fn path_bytes(path: &Path) -> Option<&[u8]> {
    use std::os::unix::ffi::OsStrExt;

    Some(path.as_os_str().as_bytes())
}

#[cfg(not(unix))]
fn path_bytes(_path: &Path) -> Option<&[u8]> {
    None
}

/// Percent-encode `bytes` so a name that is not valid UTF-8 survives a format
/// whose strings must be.
///
/// `/` is left as itself so the result still reads as a path — a unix filename
/// component cannot contain one — and every byte outside the unreserved set of
/// RFC 3986 is escaped, `%` included, so the encoding round-trips.
fn percent_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                encoded.push(char::from(byte));
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }

    encoded
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
                path: finding.display_path(),
                path_encoded: finding.encoded_path(),
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
            // The "warning" arm is unreachable today: every `Finding` sets
            // `is_error: true` (see `Finding::new`/`diagnostic`/`markdown`
            // above, and AD-0012). Kept for a future non-Markdown content
            // rule that might legitimately report a warning.
            level: if finding.is_error { "error" } else { "warning" },
            message: SarifMessage {
                text: &finding.message,
            },
            locations: vec![SarifLocation {
                physical_location: SarifPhysicalLocation {
                    artifact_location: SarifArtifactLocation {
                        // A SARIF uri is a URI reference, so the encoded form
                        // is the one the format wants for a path that cannot
                        // be a Unicode string (#172).
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
