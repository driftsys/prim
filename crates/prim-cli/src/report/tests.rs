//! Report-rendering tests, split out to keep `report.rs` inside the
//! module-size limit AGENTS.md sets.

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

/// A name holding a byte that is not valid UTF-8, alongside the bytes the
/// encoder must escape to round-trip — a literal `%` and a space — and the
/// unreserved ones it must leave alone: `/`, `-`, `_` and `~`. It cannot be created on APFS or HFS+, so the
/// end-to-end coverage is Linux-only; this reaches the rendering without
/// touching a filesystem.
#[cfg(unix)]
fn undecodable() -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;

    std::path::PathBuf::from(std::ffi::OsStr::from_bytes(b"docs/a-b_c~d%41 e\xe9.md"))
}

/// The percent-encoded form of [`undecodable`]: `/` survives so the result
/// still reads as a path, and `%` is itself escaped so decoding recovers
/// the original bytes rather than `A`.
#[cfg(unix)]
const ENCODED: &str = "docs/a-b_c~d%2541%20e%E9.md";

/// Every constructor has to carry the bytes through, not just the
/// unpositioned one: `diagnostic` and `markdown` build the whole
/// `hygiene::*` and `MD***` surface that `prim lint --format` reports.
#[cfg(unix)]
fn each_constructor() -> Vec<Finding> {
    vec![
        Finding::new(&undecodable(), "format::drift", "would be reformatted"),
        Finding::diagnostic(
            &undecodable(),
            &prim_fmt::Diagnostic {
                code: "hygiene::trailing-whitespace",
                message: "trailing whitespace".to_string(),
                line: 1,
                column: 6,
            },
        ),
        Finding::markdown(
            &undecodable(),
            &prim_fmt::MdDiagnostic {
                rule: "MD034".to_string(),
                line: 2,
                column: 1,
                is_error: true,
                message: "bare URL".to_string(),
            },
        ),
    ]
}

/// JSON cannot hold a byte that is not valid UTF-8, so `path` stays the
/// lossy rendering a reader can read and an exact form is emitted alongside it.
#[test]
#[cfg(unix)]
fn a_json_report_carries_the_exact_bytes_of_an_undecodable_path() {
    for finding in each_constructor() {
        let report = render(OutputFormat::Json, ReportMode::Lint, &[finding]);
        let value: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(
            value["findings"][0]["path"],
            json!("docs/a-b_c~d%41 e\u{fffd}.md")
        );
        assert_eq!(value["findings"][0]["path_encoded"], json!(ENCODED));
    }
}

/// A SARIF `uri` is a URI reference, so percent-encoding is the form the
/// format already expects — and the only one that names the real file.
#[test]
#[cfg(unix)]
fn a_sarif_report_percent_encodes_an_undecodable_path() {
    for finding in each_constructor() {
        let report = render(OutputFormat::Sarif, ReportMode::Lint, &[finding]);
        let value: Value = serde_json::from_str(&report).unwrap();

        assert_eq!(
            value["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
                ["uri"],
            json!(ENCODED)
        );
    }
}

/// Decoding `path_encoded` recovers the bytes prim was given. Without this
/// the encoder could leave `%` unescaped and every test above would still
/// pass, while a consumer reconstructed the wrong name.
#[test]
#[cfg(unix)]
fn the_encoded_path_decodes_back_to_the_original_bytes() {
    use std::os::unix::ffi::OsStrExt;

    let mut decoded = Vec::new();
    let mut bytes = ENCODED.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let hex: String = [bytes.next().unwrap(), bytes.next().unwrap()]
                .iter()
                .map(|b| char::from(*b))
                .collect();
            decoded.push(u8::from_str_radix(&hex, 16).unwrap());
        } else {
            decoded.push(byte);
        }
    }

    assert_eq!(decoded, undecodable().as_os_str().as_bytes());
}

/// The fix is additive: a path that is valid UTF-8 — every path on a
/// platform whose filenames are Unicode, and nearly every one elsewhere —
/// renders exactly as it did before, in both formats.
#[test]
fn a_decodable_path_renders_exactly_as_before() {
    let findings = [Finding::new(
        Path::new("docs/a b%c.md"),
        "format::drift",
        "would be reformatted",
    )];

    let json: Value =
        serde_json::from_str(&render(OutputFormat::Json, ReportMode::Lint, &findings)).unwrap();
    assert_eq!(json["findings"][0]["path"], json!("docs/a b%c.md"));
    assert!(
        json["findings"][0].get("path_encoded").is_none(),
        "no extra field for a decodable path: {json}"
    );

    let sarif: Value =
        serde_json::from_str(&render(OutputFormat::Sarif, ReportMode::Lint, &findings)).unwrap();
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        json!("docs/a b%c.md")
    );
}

/// Additivity has to hold for every decodable path, not only an ASCII one.
/// Gating the exact form on `is_ascii` instead of `to_str` would give
/// `café.md` a `path_encoded` and a percent-encoded uri — exactly the
/// breaking change AD-0019 rejects as option 3 — and an ASCII-only fixture
/// cannot tell the two apart.
#[test]
fn a_decodable_non_ascii_path_gains_nothing() {
    let findings = [Finding::new(
        Path::new("docs/café — note.md"),
        "format::drift",
        "would be reformatted",
    )];

    let json: Value =
        serde_json::from_str(&render(OutputFormat::Json, ReportMode::Lint, &findings)).unwrap();
    assert_eq!(json["findings"][0]["path"], json!("docs/café — note.md"));
    assert!(json["findings"][0].get("path_encoded").is_none(), "{json}");

    let sarif: Value =
        serde_json::from_str(&render(OutputFormat::Sarif, ReportMode::Lint, &findings)).unwrap();
    assert_eq!(
        sarif["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        json!("docs/café — note.md")
    );
}

/// Each finding's path must be its own. Every other fixture renders one
/// finding, so deriving the encoded path from `findings.first()` would
/// stamp one path onto the whole report and still pass them.
#[test]
#[cfg(unix)]
fn each_finding_carries_its_own_path() {
    let findings = [
        Finding::new(
            Path::new("plain.md"),
            "format::drift",
            "would be reformatted",
        ),
        Finding::new(&undecodable(), "format::drift", "would be reformatted"),
    ];

    let json: Value =
        serde_json::from_str(&render(OutputFormat::Json, ReportMode::Lint, &findings)).unwrap();
    assert_eq!(json["findings"][0]["path"], json!("plain.md"));
    assert!(
        json["findings"][0].get("path_encoded").is_none(),
        "the decodable finding gains nothing: {json}"
    );
    assert_eq!(json["findings"][1]["path_encoded"], json!(ENCODED));

    let sarif: Value =
        serde_json::from_str(&render(OutputFormat::Sarif, ReportMode::Lint, &findings)).unwrap();
    let uri = |index: usize| {
        sarif["runs"][0]["results"][index]["locations"][0]["physicalLocation"]
            ["artifactLocation"]["uri"]
            .clone()
    };
    assert_eq!(uri(0), json!("plain.md"));
    assert_eq!(uri(1), json!(ENCODED));
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
