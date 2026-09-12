use std::collections::BTreeSet;

use assert_cmd::Command;
use serde_json::{Value, json};

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn registry() -> Value {
    let output = prim()
        .args(["registry", "--format", "json"])
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).expect("registry stdout is one JSON document")
}

fn diagnostic<'a>(registry: &'a Value, code: &str) -> &'a Value {
    registry["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == code)
        .unwrap_or_else(|| panic!("{code} is registered"))
}

#[test]
fn registry_is_schema_valid_versioned_and_deterministic() {
    let document = registry();
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/prim-registry-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).expect("registry schema compiles");
    if let Err(error) = validator.validate(&document) {
        panic!("registry should validate: {error}");
    }

    assert_eq!(document["schema_version"], 1);
    assert_eq!(document["tool"]["name"], "prim");
    assert_eq!(document["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(document["aliases"], json!([]));
    assert_eq!(document["retired"], json!([]));

    let codes = document["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(codes.len(), 37);
    assert!(codes.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        codes.iter().copied().collect::<BTreeSet<_>>().len(),
        codes.len()
    );
}

#[test]
fn registry_describes_the_inline_configuration_that_changes_rule_behavior() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join(".editorconfig"),
        "root = true\n[*.md]\nprim_mdlint_strict = true\n",
    )
    .unwrap();
    for (source, expected) in [
        ("## Heading\n", 1),
        (
            "<!-- markdownlint-configure-file {\"MD041\":{\"level\":2}} -->\n## Heading\n",
            0,
        ),
    ] {
        prim()
            .current_dir(directory.path())
            .args(["lint", "--format", "json", "--stdin-filepath", "guide.md"])
            .write_stdin(source)
            .assert()
            .code(expected);
    }
    let document = registry();
    assert!(
        diagnostic(&document, "MD041")["inline_controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|control| control == "markdownlint-configure-file")
    );
}

#[test]
fn registry_projects_the_engine_catalogs_and_policy_metadata() {
    let document = registry();
    for definition in prim_fmt::hygiene_diagnostic_definitions() {
        let entry = diagnostic(&document, definition.code);
        assert_eq!(entry["description"], definition.description);
        assert_eq!(entry["category"], "hygiene");
        assert_eq!(entry["formats"], json!(["orphan"]));
    }
    for definition in prim_fmt::markdown_rule_definitions() {
        let entry = diagnostic(&document, definition.code);
        assert_eq!(entry["description"], definition.description);
        assert_eq!(entry["category"], "markdown");
        assert_eq!(entry["formats"], json!(["markdown"]));
        assert_eq!(entry["can_disable"], true);
        assert!(
            entry["inline_controls"]
                .as_array()
                .unwrap()
                .iter()
                .any(|control| control == "markdownlint-configure-file"),
            "{}",
            definition.code
        );
        let expected = match definition.tier {
            prim_fmt::MarkdownTier::Floor => "always",
            prim_fmt::MarkdownTier::Strict => "prim_mdlint_strict",
            prim_fmt::MarkdownTier::LineLength => "prim_mdlint_report_line_length",
        };
        assert_eq!(entry["enabled_by"], expected, "{}", definition.code);
    }
    for (code, enabled, keys, can_disable) in [
        ("hygiene::bom", "always", vec![], false),
        ("hygiene::eol", "editorconfig", vec!["end_of_line"], false),
        (
            "hygiene::final-newline",
            "editorconfig",
            vec!["insert_final_newline"],
            true,
        ),
        (
            "hygiene::indent",
            "editorconfig",
            vec!["indent_size", "indent_style"],
            false,
        ),
        (
            "hygiene::trailing-whitespace",
            "editorconfig",
            vec!["trim_trailing_whitespace"],
            true,
        ),
    ] {
        let entry = diagnostic(&document, code);
        assert_eq!(entry["enabled_by"], enabled, "{code}");
        assert_eq!(entry["configuration_keys"], json!(keys), "{code}");
        assert_eq!(entry["can_disable"], can_disable, "{code}");
        assert_eq!(entry["inline_controls"], json!([]), "{code}");
    }

    assert_eq!(
        diagnostic(&document, "MD041")["enabled_by"],
        "prim_mdlint_strict"
    );
    assert_eq!(
        diagnostic(&document, "MD041")["configuration_keys"],
        json!(["prim_mdlint_disable", "prim_mdlint_strict"])
    );
    assert_eq!(
        diagnostic(&document, "MD041")["inline_controls"],
        json!([
            "markdownlint-configure-file",
            "markdownlint-disable",
            "prim-mdlint-strict",
            "rumdl-disable"
        ])
    );
    assert_eq!(diagnostic(&document, "MD045")["enabled_by"], "always");
    assert_eq!(
        diagnostic(&document, "hygiene::indent")["configuration_keys"],
        json!(["indent_size", "indent_style"])
    );
    assert_eq!(
        diagnostic(&document, "format::drift")["configuration_keys"],
        json!([
            "end_of_line",
            "indent_size",
            "indent_style",
            "insert_final_newline",
            "max_line_length",
            "tab_width",
            "trim_trailing_whitespace"
        ])
    );
    assert_eq!(
        diagnostic(&document, "MD013")["configuration_keys"],
        json!([
            "max_line_length",
            "prim_mdlint_disable",
            "prim_mdlint_report_line_length",
            "prim_mdlint_strict"
        ])
    );
}

#[test]
fn every_machine_diagnostic_code_has_exactly_one_registry_entry() {
    let document = registry();
    let codes = document["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["code"].as_str().unwrap())
        .collect::<Vec<_>>();

    let expected = [
        "format::drift",
        "input::read",
        "format::parse",
        "internal::panic",
        "scope::empty",
        "scope::resolve",
    ]
    .into_iter()
    .chain(
        prim_fmt::hygiene_diagnostic_definitions()
            .iter()
            .map(|definition| definition.code),
    )
    .chain(
        prim_fmt::markdown_rule_definitions()
            .into_iter()
            .map(|definition| definition.code),
    );

    for code in expected {
        assert_eq!(
            codes
                .iter()
                .filter(|registered| **registered == code)
                .count(),
            1,
            "{code} must resolve exactly once"
        );
    }
}

#[test]
fn registry_does_not_inspect_the_repository_and_rejects_other_formats() {
    let baseline = prim()
        .args(["registry", "--format", "json"])
        .assert()
        .success()
        .stderr("")
        .get_output()
        .stdout
        .clone();
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join(".editorconfig"), "not valid").unwrap();
    std::fs::write(directory.path().join("bad.json"), "{ not valid").unwrap();
    prim()
        .current_dir(directory.path())
        .args(["registry", "--format", "json", "--exclude", "[", "--staged"])
        .assert()
        .success()
        .stderr("")
        .stdout(baseline);

    prim().arg("registry").assert().code(2).stdout("");
    prim()
        .args(["registry", "--format", "sarif"])
        .assert()
        .code(2)
        .stdout("");
}
