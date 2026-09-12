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

fn validate_sarif(report: &Value) {
    let schema: Value =
        serde_json::from_str(include_str!("fixtures/sarif-schema-2.1.0.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    if let Err(error) = validator.validate(report) {
        panic!("SARIF output should validate: {error}");
    }
}

#[test]
fn json_reports_preserve_findings_and_code_each_operational_failure() {
    for verb in ["fmt", "lint"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("good.json"), "{\"good\":true}\n").unwrap();
        fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();
        fs::write(dir.path().join("boom.md"), "#    Boom\n").unwrap();
        let originals = ["good.json", "bad.json", "boom.md"]
            .map(|path| (path, fs::read(dir.path().join(path)).unwrap()));

        let mut command = prim();
        command
            .current_dir(dir.path())
            .env("PRIM_PANIC_INJECT", "boom.md")
            .arg(verb);
        if verb == "fmt" {
            command.arg("--check");
        }
        let output = command
            .args([
                "--format",
                "json",
                "good.json",
                "bad.json",
                "missing.json",
                "boom.md",
            ])
            .assert()
            .code(2);
        let report = stdout_json(&output);

        assert_eq!(report["findings"].as_array().unwrap().len(), 1, "{verb}");
        assert_eq!(report["findings"][0]["path"], "good.json", "{verb}");
        let errors = report["errors"].as_array().unwrap();
        assert_eq!(
            errors
                .iter()
                .map(|error| (&error["code"], &error["path"]))
                .collect::<Vec<_>>(),
            vec![
                (&json!("format::parse"), &json!("bad.json")),
                (&json!("internal::panic"), &json!("boom.md")),
                (&json!("input::read"), &json!("missing.json")),
            ],
            "{verb}: {report}"
        );
        assert!(
            errors[0]["message"]
                .as_str()
                .unwrap()
                .starts_with("bad.json: ")
        );
        assert_eq!(
            errors[1]["message"],
            "boom.md: panicked while processing this file; it is unchanged — please report it at https://github.com/driftsys/prim/issues"
        );
        assert!(
            errors[2]["message"]
                .as_str()
                .unwrap()
                .starts_with("missing.json: ")
        );
        assert!(
            errors
                .iter()
                .all(|error| error.get("path_encoded").is_none())
        );
        for (path, original) in originals {
            assert_eq!(
                fs::read(dir.path().join(path)).unwrap(),
                original,
                "{verb} must remain no-write through a partial failure"
            );
        }
    }
}

#[test]
fn json_reports_emit_a_complete_document_when_every_input_fails() {
    for verb in ["fmt", "lint"] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();
        fs::write(dir.path().join("boom.md"), "# Boom\n").unwrap();

        let mut command = prim();
        command
            .current_dir(dir.path())
            .env("PRIM_PANIC_INJECT", "boom.md")
            .arg(verb);
        if verb == "fmt" {
            command.arg("--check");
        }
        let output = command
            .args(["--format", "json", "bad.json", "missing.json", "boom.md"])
            .assert()
            .code(2);
        let report = stdout_json(&output);

        assert_eq!(report["findings"], json!([]), "{verb}: {report}");
        assert_eq!(
            report["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["code"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["format::parse", "internal::panic", "input::read"],
            "{verb}: {report}"
        );
    }
}

#[test]
fn sarif_reports_operational_failures_as_error_results() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();
    fs::write(dir.path().join("boom.md"), "# Boom\n").unwrap();
    let originals =
        ["bad.json", "boom.md"].map(|path| (path, fs::read(dir.path().join(path)).unwrap()));

    let output = prim()
        .current_dir(dir.path())
        .env("PRIM_PANIC_INJECT", "boom.md")
        .args([
            "lint",
            "--format",
            "sarif",
            "bad.json",
            "missing.json",
            "boom.md",
        ])
        .assert()
        .code(2);
    let report = stdout_json(&output);
    validate_sarif(&report);
    let results = report["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(
        results
            .iter()
            .map(|result| {
                (
                    result["ruleId"].as_str().unwrap(),
                    result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                        .as_str()
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>(),
        [
            ("format::parse", "bad.json"),
            ("internal::panic", "boom.md"),
            ("input::read", "missing.json"),
        ]
    );
    assert!(results.iter().all(|result| result["level"] == "error"));
    assert!(
        results[0]["message"]["text"]
            .as_str()
            .unwrap()
            .starts_with("bad.json: ")
    );
    assert_eq!(
        results[1]["message"]["text"],
        "boom.md: panicked while processing this file; it is unchanged — please report it at https://github.com/driftsys/prim/issues"
    );
    assert!(
        results[2]["message"]["text"]
            .as_str()
            .unwrap()
            .starts_with("missing.json: ")
    );
    let rules = report["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap();
    for (code, description) in [
        (
            "format::parse",
            "a selected structured input could not be parsed",
        ),
        ("input::read", "prim could not read a selected input"),
        (
            "internal::panic",
            "prim contained an internal formatter or linter panic",
        ),
    ] {
        assert!(
            rules.iter().any(|rule| {
                rule["id"] == code && rule["shortDescription"]["text"] == description
            })
        );
    }
    for (path, original) in originals {
        assert_eq!(fs::read(dir.path().join(path)).unwrap(), original);
    }
}

#[test]
fn discovery_failures_still_emit_json_and_sarif_documents() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.json");
    fs::write(&file, "{\"a\":1}\n").unwrap();

    for (verb, format, mode) in [
        ("fmt", "json", "fmt-check"),
        ("fmt", "sarif", "fmt-check"),
        ("lint", "json", "lint"),
        ("lint", "sarif", "lint"),
    ] {
        let mut command = prim();
        command.current_dir(dir.path()).arg(verb);
        if verb == "fmt" {
            command.arg("--check");
        }
        let output = command
            .args(["--format", format, "--exclude", "[", "a.json"])
            .assert()
            .code(2);
        let report = stdout_json(&output);
        if format == "json" {
            assert_eq!(report["mode"], mode);
            assert_eq!(report["findings"], json!([]));
            assert_eq!(report["errors"][0]["code"], "scope::resolve");
            assert!(report["errors"][0].get("path").is_none());
            assert!(
                report["errors"][0]["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("--exclude: ")
            );
        } else {
            validate_sarif(&report);
            assert_eq!(report["runs"][0]["results"][0]["ruleId"], "scope::resolve");
            assert!(report["runs"][0]["results"][0].get("locations").is_none());
            let rules = report["runs"][0]["tool"]["driver"]["rules"]
                .as_array()
                .unwrap();
            assert_eq!(rules.len(), 1);
            assert_eq!(rules[0]["id"], "scope::resolve");
            assert_eq!(
                rules[0]["shortDescription"]["text"],
                "prim could not resolve the requested file scope"
            );
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), "{\"a\":1}\n");
    }
}

#[test]
fn sarif_partial_failure_preserves_successful_findings() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("good.json"), "{\"good\":true}\n").unwrap();
    fs::write(dir.path().join("bad.json"), "{ not valid").unwrap();

    let output = prim()
        .current_dir(dir.path())
        .args(["lint", "--format", "sarif", "good.json", "bad.json"])
        .assert()
        .code(2);
    let report = stdout_json(&output);
    validate_sarif(&report);
    let results = report["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0]["ruleId"], "format::drift");
    assert_eq!(results[1]["ruleId"], "format::parse");
    assert_eq!(
        fs::read_to_string(dir.path().join("good.json")).unwrap(),
        "{\"good\":true}\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("bad.json")).unwrap(),
        "{ not valid"
    );
}

#[test]
fn sarif_run_wide_errors_have_no_fake_location() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".primignore"), "ignored.md\n").unwrap();
    fs::write(dir.path().join("ignored.md"), "#    Ignored\n").unwrap();

    let output = prim()
        .current_dir(dir.path())
        .args(["fmt", "--check", "--format", "sarif", "ignored.md"])
        .assert()
        .code(2);
    let report = stdout_json(&output);
    validate_sarif(&report);
    let result = &report["runs"][0]["results"][0];
    assert_eq!(result["ruleId"], "scope::empty");
    assert!(result.get("locations").is_none(), "{report}");
}

#[test]
fn stdin_lint_parse_failures_are_structured() {
    let output = prim()
        .args(["lint", "--stdin-filepath", "bad.json", "--format", "json"])
        .write_stdin("{ not valid")
        .assert()
        .code(2);
    let report = stdout_json(&output);
    assert_eq!(report["findings"], json!([]));
    assert_eq!(report["errors"].as_array().unwrap().len(), 1);
    assert_eq!(report["errors"][0]["code"], "format::parse");
    assert_eq!(report["errors"][0]["path"], "bad.json");
}
