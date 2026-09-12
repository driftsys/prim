//! Additional black-box contract tests for effect-plan fidelity.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::Value;
use sha2::{Digest, Sha256};

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn write(root: &Path, relative: &str, contents: &str) {
    fs::write(root.join(relative), contents).unwrap();
}

fn plan(root: &Path, verb: &str, files: &[&str]) -> Value {
    let output = prim()
        .current_dir(root)
        .args([verb, "--dry-run", "--format", "json"])
        .args(files)
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).expect("effect plan is JSON")
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn effect<'a>(document: &'a Value, path: &str) -> &'a Value {
    document["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|candidate| candidate["path"] == path)
        .unwrap_or_else(|| panic!("missing effect for {path}"))
}

fn validate_plan(document: &Value) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/prim-effect-plan-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).expect("effect-plan schema compiles");
    if let Err(error) = validator.validate(document) {
        panic!("effect plan should validate: {error}");
    }
}

#[test]
fn mixed_batches_omit_unchanged_files_from_both_plans() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        write(repository.path(), "clean.txt", "clean\n");
        write(repository.path(), "dirty.txt", "dirty  \n");
        let document = plan(repository.path(), verb, &["clean.txt", "dirty.txt"]);
        assert_eq!(document["effects"].as_array().unwrap().len(), 1);
        assert_eq!(document["effects"][0]["path"], "dirty.txt");
        assert_eq!(
            fs::read_to_string(repository.path().join("clean.txt")).unwrap(),
            "clean\n"
        );
        assert_eq!(
            fs::read_to_string(repository.path().join("dirty.txt")).unwrap(),
            "dirty  \n"
        );
    }
}

#[test]
fn plan_serializes_asymmetric_and_non_default_configuration_with_byte_counts() {
    let repository = tempfile::tempdir().unwrap();
    write(
        repository.path(),
        ".editorconfig",
        "root = true\n[trim.txt]\nend_of_line = lf\ntrim_trailing_whitespace = true\ninsert_final_newline = false\nindent_style = tab\nindent_size = tab\n\n[final.txt]\nend_of_line = lf\ntrim_trailing_whitespace = false\ninsert_final_newline = true\nindent_style = space\nindent_size = 3\n",
    );
    let trim_before = "é  \n";
    let trim_after = "é";
    let final_before = "é  ";
    let final_after = "é  \n";
    write(repository.path(), "trim.txt", trim_before);
    write(repository.path(), "final.txt", final_before);
    let entries_before = fs::read_dir(repository.path()).unwrap().count();

    let document = plan(repository.path(), "fmt", &["trim.txt", "final.txt"]);

    let trim = effect(&document, "trim.txt");
    assert_eq!(trim["before"]["bytes"], trim_before.len());
    assert_eq!(trim["before"]["sha256"], digest(trim_before.as_bytes()));
    assert_eq!(trim["after"]["bytes"], trim_after.len());
    assert_eq!(trim["after"]["sha256"], digest(trim_after.as_bytes()));
    assert_eq!(trim["configuration"]["end_of_line"], "lf");
    assert_eq!(trim["configuration"]["trim_trailing_whitespace"], true);
    assert_eq!(trim["configuration"]["insert_final_newline"], false);
    assert_eq!(trim["configuration"]["indent_style"], "tab");
    assert_eq!(trim["configuration"]["indent_size"], Value::Null);
    assert_eq!(trim["configuration"]["max_line_length"], Value::Null);

    let final_effect = effect(&document, "final.txt");
    assert_eq!(final_effect["before"]["bytes"], final_before.len());
    assert_eq!(final_effect["after"]["bytes"], final_after.len());
    assert_eq!(
        final_effect["configuration"]["trim_trailing_whitespace"],
        false
    );
    assert_eq!(final_effect["configuration"]["insert_final_newline"], true);
    assert_eq!(final_effect["configuration"]["indent_style"], "space");
    assert_eq!(final_effect["configuration"]["indent_size"], 3);

    assert_eq!(
        fs::read_to_string(repository.path().join("trim.txt")).unwrap(),
        trim_before
    );
    assert_eq!(
        fs::read_to_string(repository.path().join("final.txt")).unwrap(),
        final_before
    );
    assert_eq!(
        fs::read_dir(repository.path()).unwrap().count(),
        entries_before
    );
}

#[cfg(unix)]
#[test]
fn planning_does_not_change_file_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let repository = tempfile::tempdir().unwrap();
    let path = repository.path().join("mode.txt");
    fs::write(&path, "drift  \n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();

    let document = plan(repository.path(), "fmt", &["mode.txt"]);

    assert_eq!(document["effects"].as_array().unwrap().len(), 1);
    assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o640);
}

#[test]
fn fmt_and_fix_plan_the_same_markdown_bytes_even_with_content_findings() {
    let repository = tempfile::tempdir().unwrap();
    write(
        repository.path(),
        "doc.md",
        "#    Heading\n\nhttps://example.com\n",
    );

    let mut fmt = plan(repository.path(), "fmt", &["doc.md"]);
    let mut fix = plan(repository.path(), "fix", &["doc.md"]);
    fmt.as_object_mut().unwrap().remove("operation");
    fix.as_object_mut().unwrap().remove("operation");

    assert_eq!(fmt, fix);
}

#[test]
fn zero_width_editorconfig_values_still_produce_a_schema_valid_plan() {
    let repository = tempfile::tempdir().unwrap();
    write(
        repository.path(),
        ".editorconfig",
        "root = true\n[*]\nindent_size = 0\nmax_line_length = 0\n",
    );
    write(repository.path(), "notes.txt", "drift  \n");

    let document = plan(repository.path(), "fmt", &["notes.txt"]);

    assert_eq!(document["effects"][0]["configuration"]["indent_size"], 0);
    assert_eq!(
        document["effects"][0]["configuration"]["max_line_length"],
        0
    );
    validate_plan(&document);

    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/prim-effect-plan-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).expect("effect-plan schema compiles");
    let mut negative_indent = document.clone();
    negative_indent["effects"][0]["configuration"]["indent_size"] = (-1).into();
    assert!(!validator.is_valid(&negative_indent));
    let mut negative_width = document;
    negative_width["effects"][0]["configuration"]["max_line_length"] = (-1).into();
    assert!(!validator.is_valid(&negative_width));
}
