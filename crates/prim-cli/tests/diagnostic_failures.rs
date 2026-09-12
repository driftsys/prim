//! Operational failure codes at the diagnostic and stdin boundaries.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn assert_error(report: &Value, format: &str, code: &str, path: &str) {
    if format == "json" {
        assert_eq!(report["errors"].as_array().unwrap().len(), 1);
        assert_eq!(report["errors"][0]["code"], code);
        assert_eq!(report["errors"][0]["path"], path);
    } else {
        let matches = report["runs"][0]["results"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|finding| finding["ruleId"] == code)
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            path
        );
    }
}

#[test]
fn lint_stage_panics_preserve_neighbor_findings_and_report_the_path() {
    for format in ["json", "sarif"] {
        for path in ["boom.txt", "boom.md"] {
            let directory = tempfile::tempdir().unwrap();
            fs::write(directory.path().join(path), "panic fixture  \n").unwrap();
            fs::write(directory.path().join("good.json"), "{\"good\":true}\n").unwrap();
            let output = prim()
                .current_dir(directory.path())
                .env("PRIM_DIAGNOSTIC_PANIC_INJECT", "boom")
                .args(["lint", "--format", format, path, "good.json"])
                .assert()
                .code(2)
                .get_output()
                .stdout
                .clone();
            let report: Value = serde_json::from_slice(&output).unwrap();
            assert_error(&report, format, "internal::panic", path);
            if format == "json" {
                assert_eq!(report["findings"].as_array().unwrap().len(), 1);
                assert_eq!(report["findings"][0]["path"], "good.json");
            } else {
                assert_eq!(report["runs"][0]["results"].as_array().unwrap().len(), 2);
            }
            assert_eq!(
                fs::read_to_string(directory.path().join(path)).unwrap(),
                "panic fixture  \n"
            );
        }
    }
}

#[test]
fn stdin_lint_distinguishes_read_failures_and_panics() {
    for format in ["json", "sarif"] {
        for (input, code, has_panic) in [
            (vec![0xff], "input::read", false),
            (b"fixture\n".to_vec(), "internal::panic", true),
        ] {
            let mut command = prim();
            command.args(["lint", "--format", format, "--stdin-filepath", "boom.txt"]);
            if has_panic {
                command.env("PRIM_PANIC_INJECT", "boom.txt");
            }
            let output = command
                .write_stdin(input)
                .assert()
                .code(2)
                .get_output()
                .stdout
                .clone();
            let report: Value = serde_json::from_slice(&output).unwrap();
            assert_error(&report, format, code, "boom.txt");
        }
    }
}
