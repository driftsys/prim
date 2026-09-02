use std::fs;

use assert_cmd::Command;
use assert_cmd::assert::Assert;
use serde_json::{Value, json};

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn stdout_json(assert: &Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).expect("stdout is valid JSON")
}

fn sort_findings(findings: &[Value]) -> Vec<Value> {
    let mut sorted = findings.to_vec();
    sorted.sort_by(|left, right| {
        let left_key = (
            left["path"].as_str().unwrap_or_default(),
            left["code"].as_str().unwrap_or_default(),
        );
        let right_key = (
            right["path"].as_str().unwrap_or_default(),
            right["code"].as_str().unwrap_or_default(),
        );
        left_key.cmp(&right_key)
    });
    sorted
}

fn validate_sarif(report: &Value) {
    let schema: Value =
        serde_json::from_str(include_str!("fixtures/sarif-schema-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    if let Err(error) = validator.validate(report) {
        panic!("SARIF output should validate: {error}");
    }
}

#[test]
fn fmt_check_json_reports_files_that_would_change_without_writing() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    let original = "{\"a\":1}\n";
    fs::write(&file, original).unwrap();

    let output = prim()
        .args(["fmt", "--check", "--format", "json"])
        .arg(&file)
        .assert()
        .code(1);
    let report = stdout_json(&output);

    assert_eq!(
        report,
        json!({
            "version": 1,
            "mode": "fmt-check",
            "findings": [
                {
                    "path": file.display().to_string(),
                    "code": "format::drift",
                    "message": "would be reformatted"
                }
            ]
        })
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn fmt_check_sarif_validates_against_the_official_schema() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    let original = "{\"a\":1}\n";
    fs::write(&file, original).unwrap();

    let output = prim()
        .args(["fmt", "--check", "--format", "sarif"])
        .arg(&file)
        .assert()
        .code(1);
    let report = stdout_json(&output);
    validate_sarif(&report);

    assert_eq!(report["version"], json!("2.1.0"));
    assert_eq!(
        report["runs"][0]["results"][0]["ruleId"],
        json!("format::drift")
    );
    assert_eq!(
        report["runs"][0]["results"][0]["message"]["text"],
        json!("would be reformatted")
    );
    assert_eq!(
        report["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        json!(file.display().to_string())
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn lint_json_reports_hygiene_diagnostics_and_structured_format_drift() {
    let dir = tempfile::tempdir().unwrap();
    let orphan = dir.path().join("notes.txt");
    let structured = dir.path().join("doc.json");
    fs::write(&orphan, "title  \n").unwrap();
    fs::write(&structured, "{\"a\":1}\n").unwrap();

    let output = prim()
        .args(["lint", "--format", "json"])
        .arg(&orphan)
        .arg(&structured)
        .assert()
        .code(1);
    let report = stdout_json(&output);
    let findings = sort_findings(report["findings"].as_array().unwrap());

    assert_eq!(report["version"], json!(1));
    assert_eq!(report["mode"], json!("lint"));
    assert_eq!(
        findings,
        vec![
            json!({
                "path": structured.display().to_string(),
                "code": "format::drift",
                "message": "does not match prim's canonical format (run `prim fmt` to fix)"
            }),
            json!({
                "path": orphan.display().to_string(),
                "code": "hygiene::trailing-whitespace",
                "line": 1,
                "column": 6,
                "message": "trailing whitespace"
            })
        ]
    );
    assert_eq!(fs::read_to_string(&orphan).unwrap(), "title  \n");
    assert_eq!(fs::read_to_string(&structured).unwrap(), "{\"a\":1}\n");
}

#[test]
fn lint_sarif_validates_and_keeps_precise_regions_for_positioned_findings() {
    let dir = tempfile::tempdir().unwrap();
    let orphan = dir.path().join("notes.txt");
    let structured = dir.path().join("doc.json");
    fs::write(&orphan, "title  \n").unwrap();
    fs::write(&structured, "{\"a\":1}\n").unwrap();

    let output = prim()
        .args(["lint", "--format", "sarif"])
        .arg(&orphan)
        .arg(&structured)
        .assert()
        .code(1);
    let report = stdout_json(&output);
    validate_sarif(&report);

    let results = report["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(
        report["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(results.iter().any(|result| {
        result["ruleId"] == json!("hygiene::trailing-whitespace")
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                == json!(orphan.display().to_string())
            && result["locations"][0]["physicalLocation"]["region"]["startLine"] == json!(1)
            && result["locations"][0]["physicalLocation"]["region"]["startColumn"] == json!(6)
    }));
    assert!(results.iter().any(|result| {
        result["ruleId"] == json!("format::drift")
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                == json!(structured.display().to_string())
    }));
    assert_eq!(fs::read_to_string(&orphan).unwrap(), "title  \n");
    assert_eq!(fs::read_to_string(&structured).unwrap(), "{\"a\":1}\n");
}

#[test]
fn lint_json_reports_markdown_rumdl_findings() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    fs::write(&file, "#Title\n\nSee https://example.com.\n").unwrap();

    let output = prim()
        .args(["lint", "--format", "json"])
        .arg(&file)
        .assert()
        .code(1);
    let report = stdout_json(&output);

    assert_eq!(report["version"], json!(1));
    assert_eq!(report["mode"], json!("lint"));
    let findings = report["findings"].as_array().unwrap();
    assert!(findings.iter().any(|finding| {
        finding["path"] == json!(file.display().to_string())
            && finding["code"] == json!("MD034")
            && finding["line"] == json!(3)
    }));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "#Title\n\nSee https://example.com.\n"
    );
}

#[test]
fn lint_sarif_marks_floor_markdown_defects_as_errors() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("README.md");
    fs::write(&file, "# Title\n\n![](hero.png)\n").unwrap();

    let output = prim()
        .args(["lint", "--format", "sarif"])
        .arg(&file)
        .assert()
        .code(1);
    let report = stdout_json(&output);
    validate_sarif(&report);

    let results = report["runs"][0]["results"].as_array().unwrap();
    assert!(results.iter().any(|result| {
        result["ruleId"] == json!("MD045")
            && result["level"] == json!("error")
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                == json!(file.display().to_string())
    }));
}

#[test]
fn format_is_rejected_outside_fmt_check_and_lint() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("notes.txt");
    fs::write(&file, "title  \n").unwrap();

    prim()
        .args(["fmt", "--format", "json"])
        .arg(&file)
        .assert()
        .code(2);
    prim()
        .args(["fmt", "--diff", "--format", "json"])
        .arg(&file)
        .assert()
        .code(2);
    prim()
        .args(["fix", "--check", "--format", "json"])
        .arg(&file)
        .assert()
        .code(2);
    prim()
        .args(["fix", "--format", "json"])
        .arg(&file)
        .assert()
        .code(2);
}

#[test]
fn a_gate_that_examined_nothing_still_emits_its_report() {
    // FR-5.8: `--format` changes stdout alone. A gate pointed only at skipped
    // paths exits 2 (FR-4.4c), and the machine-readable document is still the
    // one a pipeline uploads — an empty findings list, not an empty file.
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".primignore"), "CHANGELOG.md\n").unwrap();
    fs::write(dir.path().join("CHANGELOG.md"), "#  Changelog\n").unwrap();

    for (verb, mode) in [("fmt", "fmt-check"), ("lint", "lint")] {
        let mut command = prim();
        command.current_dir(dir.path()).arg(verb);
        if verb == "fmt" {
            command.arg("--check");
        }
        let output = command
            .args(["--format", "json", "CHANGELOG.md"])
            .assert()
            .code(2);

        assert_eq!(
            stdout_json(&output),
            json!({ "version": 1, "mode": mode, "findings": [] }),
            "{verb} must still emit its report"
        );
    }
}

/// #172 end to end through the report formats. A filename that is not valid
/// UTF-8 is legal on Linux and cannot exist on APFS or HFS+, so this runs only
/// where it is reachable — CI's ubuntu runner. The unit tests reach `render`
/// directly; this is what proves the bytes survive the whole way from
/// discovery through `app::emit_report`.
#[cfg(target_os = "linux")]
#[test]
fn a_report_carries_the_exact_bytes_of_an_undecodable_path() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    let odd = dir.path().join(OsStr::from_bytes(b"caf\xe9.md"));
    fs::write(&odd, "# Title  \n").unwrap();

    let json = prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "--format", "json", "."])
        .assert()
        .code(1);
    let report = stdout_json(&json);
    // A walked path carries its walk root, so this is `./caf%E9.md`.
    let encoded = report["findings"][0]["path_encoded"]
        .as_str()
        .unwrap_or_else(|| panic!("path_encoded is present and a string: {report}"));
    assert!(
        encoded.ends_with("caf%E9.md") && !encoded.contains("%EF%BF%BD"),
        "the exact bytes reach the JSON report: {encoded}"
    );

    let sarif = prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "--format", "sarif", "."])
        .assert()
        .code(1);
    let log = stdout_json(&sarif);
    validate_sarif(&log);
    let uri = log["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]
        ["uri"]
        .as_str()
        .unwrap_or_else(|| panic!("the SARIF uri is a string: {log}"));
    assert!(
        uri.ends_with("caf%E9.md") && !uri.contains("%EF%BF%BD"),
        "the exact bytes reach the SARIF uri: {uri}"
    );
}

/// The plain-text `lint` half of FR-5.9, which the `--check` list does not
/// cover. `run_lint`'s arms are mutually exclusive, so two fixtures are needed:
/// an orphan reaches `ui::lint_positioned`, and a parsed format with drift
/// reaches the coarse `ui::lint_finding`. Reverting either wrapper to
/// `Path::display` passes every test outside this one.
#[cfg(target_os = "linux")]
#[test]
fn a_plain_text_lint_finding_names_the_file_by_its_bytes() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    let dir = tempfile::tempdir().unwrap();
    // An orphan: a hygiene finding carries a position, so this drives
    // `lint_positioned`.
    let odd = dir.path().join(OsStr::from_bytes(b"caf\xe9.txt"));
    fs::write(&odd, "x  \n").unwrap();
    // A parsed format with drift: the coarse shape, which is the only route to
    // `lint_finding`.
    let odd_json = dir.path().join(OsStr::from_bytes(b"caf\xe9.json"));
    fs::write(&odd_json, "{\"a\":1}").unwrap();

    let assert = prim()
        .current_dir(dir.path())
        .args(["lint", "."])
        .assert()
        .code(1);

    let reported = &assert.get_output().stdout;
    assert!(
        contains_bytes(reported, b"caf\xe9.txt:"),
        "the positioned finding names the file by its real bytes: {reported:?}"
    );
    assert!(
        contains_bytes(reported, b"caf\xe9.json:"),
        "the coarse finding names the file by its real bytes: {reported:?}"
    );
    assert!(
        !contains_bytes(reported, "caf\u{FFFD}.txt".as_bytes()),
        "and never by the lossy rendering: {reported:?}"
    );
}
