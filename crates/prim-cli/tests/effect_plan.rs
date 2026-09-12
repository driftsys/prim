//! Black-box acceptance tests for the exact no-write formatting effect plan.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn output_json(assert: &assert_cmd::assert::Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).expect("stdout is valid JSON")
}

fn validate_plan(plan: &Value) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/prim-effect-plan-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).expect("effect-plan schema compiles");
    if let Err(error) = validator.validate(plan) {
        panic!("effect plan should validate: {error}");
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn fixture(root: &Path) -> Vec<&'static str> {
    write(
        root,
        ".editorconfig",
        "root = true\n[*]\nend_of_line = lf\ntrim_trailing_whitespace = true\ninsert_final_newline = true\nindent_style = space\nindent_size = 2\nmax_line_length = 80\n",
    );
    let files = vec![
        "README.md",
        "config.json",
        "config.jsonc",
        "config.yaml",
        "config.toml",
        "notes.txt",
        "path with spaces.md",
    ];
    for (path, contents) in [
        ("README.md", "#    Title\n"),
        ("config.json", "{\"a\":1}\n"),
        ("config.jsonc", "{\n// note\n\"a\":1,\n}\n"),
        ("config.yaml", "a:    1\n"),
        ("config.toml", "a=1\n"),
        ("notes.txt", "note  \n"),
        ("path with spaces.md", "#    Space\n"),
    ] {
        write(root, path, contents);
    }
    files
}

#[test]
fn dry_run_requires_json_and_rejects_other_modes() {
    let invalid = [
        ["fmt", "--dry-run", "doc.md"].as_slice(),
        ["fmt", "--dry-run", "--format", "sarif", "doc.md"].as_slice(),
        ["fmt", "--dry-run", "--format", "json", "--check", "doc.md"].as_slice(),
        ["fmt", "--dry-run", "--format", "json", "--diff", "doc.md"].as_slice(),
        [
            "fmt",
            "--dry-run",
            "--format",
            "json",
            "--check-idempotence",
            "doc.md",
        ]
        .as_slice(),
        [
            "fmt",
            "--dry-run",
            "--format",
            "json",
            "--stdin-filepath",
            "doc.md",
        ]
        .as_slice(),
    ];

    for args in invalid {
        prim().args(args).assert().code(2).stdout("");
    }
}

#[test]
fn fix_accepts_a_json_dry_run_without_writing() {
    let repository = tempfile::tempdir().unwrap();
    let file = repository.path().join("doc.txt");
    let original = "drift  \n";
    fs::write(&file, original).unwrap();

    prim()
        .args(["fix", "--dry-run", "--format", "json"])
        .arg(&file)
        .assert()
        .code(1);

    assert_eq!(fs::read_to_string(file).unwrap(), original);
}

#[test]
fn json_plan_describes_an_exact_effect_without_writing() {
    let repository = tempfile::tempdir().unwrap();
    write(
        repository.path(),
        ".editorconfig",
        "root = true\n[*.json]\nend_of_line = crlf\ntrim_trailing_whitespace = true\ninsert_final_newline = true\nindent_style = space\nindent_size = 4\nmax_line_length = 100\n",
    );
    let relative = "path with spaces.json";
    let original = b"{\"answer\":42}\n";
    fs::write(repository.path().join(relative), original).unwrap();

    let output = prim()
        .current_dir(repository.path())
        .args(["fmt", "--dry-run", "--format", "json", relative])
        .assert()
        .code(1);
    let plan = output_json(&output);
    validate_plan(&plan);

    assert_eq!(plan["schema_version"], 1);
    assert_eq!(plan["tool"]["name"], "prim");
    assert_eq!(plan["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(plan["operation"], "fmt");
    assert_eq!(plan["errors"], json!([]));
    assert_eq!(plan["effects"].as_array().unwrap().len(), 1);

    let effect = &plan["effects"][0];
    assert_eq!(effect["path"], relative);
    assert!(effect.get("path_encoded").is_none());
    assert_eq!(effect["kind"], "json");
    assert_eq!(effect["operation"], "replace_contents");
    assert_eq!(effect["before"]["sha256"], digest(original));
    assert_eq!(effect["before"]["bytes"], original.len());
    assert_eq!(effect["configuration"]["end_of_line"], "crlf");
    assert_eq!(effect["configuration"]["trim_trailing_whitespace"], true);
    assert_eq!(effect["configuration"]["insert_final_newline"], true);
    assert_eq!(effect["configuration"]["indent_style"], "space");
    assert_eq!(effect["configuration"]["indent_size"], 4);
    assert_eq!(effect["configuration"]["max_line_length"], 100);
    assert_eq!(
        fs::read(repository.path().join(relative)).unwrap(),
        original
    );
}

#[test]
fn fmt_and_fix_plans_match_the_bytes_applied_for_every_file_kind() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        let mut files = fixture(repository.path());
        files.reverse();
        let before = files
            .iter()
            .map(|path| {
                (
                    (*path).to_string(),
                    fs::read(repository.path().join(path)).unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let output = prim()
            .current_dir(repository.path())
            .args([verb, "--dry-run", "--format", "json"])
            .args(&files)
            .assert()
            .code(1);
        let plan = output_json(&output);
        validate_plan(&plan);
        assert_eq!(plan["operation"], verb);
        assert_eq!(plan["effects"].as_array().unwrap().len(), files.len());
        let effect_paths = plan["effects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|effect| effect["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            effect_paths,
            vec![
                "README.md",
                "config.json",
                "config.jsonc",
                "config.toml",
                "config.yaml",
                "notes.txt",
                "path with spaces.md",
            ]
        );
        for (path, bytes) in &before {
            assert_eq!(fs::read(repository.path().join(path)).unwrap(), *bytes);
        }

        prim()
            .current_dir(repository.path())
            .arg(verb)
            .args(&files)
            .assert()
            .success();

        for effect in plan["effects"].as_array().unwrap() {
            let relative = effect["path"].as_str().unwrap();
            let original = &before[relative];
            assert_eq!(effect["before"]["bytes"], original.len());
            assert_eq!(effect["before"]["sha256"], digest(original));
            let expected_kind = match relative {
                "README.md" | "path with spaces.md" => "markdown",
                "config.json" => "json",
                "config.jsonc" => "jsonc",
                "config.yaml" => "yaml",
                "config.toml" => "toml",
                "notes.txt" => "orphan",
                unexpected => panic!("unexpected effect path: {unexpected}"),
            };
            assert_eq!(effect["kind"], expected_kind);
            let actual = fs::read(repository.path().join(relative)).unwrap();
            assert_eq!(effect["after"]["bytes"], actual.len());
            assert_eq!(effect["after"]["sha256"], digest(&actual));
        }
    }
}

#[test]
fn a_clean_dry_run_has_no_effects_and_exits_zero() {
    let repository = tempfile::tempdir().unwrap();
    write(repository.path(), "clean.txt", "clean\n");

    let output = prim()
        .current_dir(repository.path())
        .args(["fmt", "--dry-run", "--format", "json", "clean.txt"])
        .assert()
        .success();
    let plan = output_json(&output);
    validate_plan(&plan);
    assert_eq!(plan["effects"], json!([]));
    assert_eq!(plan["errors"], json!([]));
}

#[test]
fn both_dry_run_verbs_preserve_effects_and_code_mixed_failures() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        write(repository.path(), "good.txt", "good  \n");
        write(repository.path(), "bad.json", "{ not valid");
        write(repository.path(), "boom.md", "#    Boom\n");
        let originals = ["good.txt", "bad.json", "boom.md"]
            .map(|path| (path, fs::read(repository.path().join(path)).unwrap()));

        let output = prim()
            .current_dir(repository.path())
            .env("PRIM_PANIC_INJECT", "boom.md")
            .args([verb, "--dry-run", "--format", "json"])
            .args(["good.txt", "bad.json", "missing.json", "boom.md"])
            .assert()
            .code(2);
        let plan = output_json(&output);
        validate_plan(&plan);
        assert_eq!(plan["effects"].as_array().unwrap().len(), 1, "{verb}");
        assert_eq!(plan["effects"][0]["path"], "good.txt", "{verb}");
        let errors = plan["errors"].as_array().unwrap();
        assert_eq!(
            errors
                .iter()
                .map(|error| (
                    error["code"].as_str().unwrap(),
                    error["path"].as_str().unwrap()
                ))
                .collect::<Vec<_>>(),
            [
                ("format::parse", "bad.json"),
                ("internal::panic", "boom.md"),
                ("input::read", "missing.json"),
            ],
            "{verb}: {plan}"
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
        for (path, original) in originals {
            assert_eq!(fs::read(repository.path().join(path)).unwrap(), original);
        }
    }
}

#[test]
fn both_dry_run_verbs_code_an_empty_scope() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        write(repository.path(), ".primignore", "ignored.md\n");
        write(repository.path(), "ignored.md", "#    Ignored\n");

        let output = prim()
            .current_dir(repository.path())
            .args([verb, "--dry-run", "--format", "json", "ignored.md"])
            .assert()
            .code(2);
        let plan = output_json(&output);
        validate_plan(&plan);
        assert_eq!(plan["operation"], verb);
        assert_eq!(plan["effects"], json!([]));
        assert_eq!(plan["errors"].as_array().unwrap().len(), 1);
        assert_eq!(plan["errors"][0]["code"], "scope::empty");
        assert!(plan["errors"][0].get("path").is_none());
        assert_eq!(
            fs::read_to_string(repository.path().join("ignored.md")).unwrap(),
            "#    Ignored\n"
        );
    }
}

#[test]
fn both_dry_run_verbs_emit_a_document_on_full_input_failure() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        write(repository.path(), "bad.json", "{ not valid");

        let output = prim()
            .current_dir(repository.path())
            .args([verb, "--dry-run", "--format", "json", "bad.json"])
            .assert()
            .code(2);
        let plan = output_json(&output);
        validate_plan(&plan);
        assert_eq!(plan["effects"], json!([]));
        assert_eq!(plan["errors"].as_array().unwrap().len(), 1);
        assert_eq!(plan["errors"][0]["code"], "format::parse");
        assert_eq!(plan["errors"][0]["path"], "bad.json");
        assert_eq!(
            fs::read_to_string(repository.path().join("bad.json")).unwrap(),
            "{ not valid"
        );
    }
}

#[test]
fn discovery_failures_still_emit_effect_plan_documents() {
    for verb in ["fmt", "fix"] {
        let repository = tempfile::tempdir().unwrap();
        write(repository.path(), "a.json", "{\"a\":1}\n");

        let output = prim()
            .current_dir(repository.path())
            .args([
                verb,
                "--dry-run",
                "--format",
                "json",
                "--exclude",
                "[",
                "a.json",
            ])
            .assert()
            .code(2);
        let plan = output_json(&output);
        validate_plan(&plan);
        assert_eq!(plan["effects"], json!([]));
        assert_eq!(plan["errors"][0]["code"], "scope::resolve");
        assert!(plan["errors"][0].get("path").is_none());
        assert_eq!(
            fs::read_to_string(repository.path().join("a.json")).unwrap(),
            "{\"a\":1}\n"
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
fn an_undecodable_effect_path_has_an_exact_encoded_form() {
    use std::os::unix::ffi::OsStringExt;

    use std::ffi::OsString;

    let repository = tempfile::tempdir().unwrap();
    let relative = OsString::from_vec(b"caf\xe9.txt".to_vec());
    fs::write(repository.path().join(&relative), "drift  \n").unwrap();

    let output = prim()
        .current_dir(repository.path())
        .args(["fmt", "--dry-run", "--format", "json"])
        .arg(&relative)
        .assert()
        .code(1);
    let plan = output_json(&output);
    validate_plan(&plan);
    assert_eq!(plan["effects"][0]["path"], "caf�.txt");
    assert_eq!(plan["effects"][0]["path_encoded"], "caf%E9.txt");
}

#[cfg(target_os = "linux")]
#[test]
fn an_undecodable_error_path_has_an_exact_encoded_form() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let repository = tempfile::tempdir().unwrap();
    let relative = OsString::from_vec(b"bad\xe9.json".to_vec());
    fs::write(repository.path().join(&relative), "{ not valid").unwrap();

    let output = prim()
        .current_dir(repository.path())
        .args(["fmt", "--dry-run", "--format", "json"])
        .arg(&relative)
        .assert()
        .code(2);
    let plan = output_json(&output);
    validate_plan(&plan);
    assert_eq!(plan["effects"], json!([]));
    assert_eq!(plan["errors"][0]["code"], "format::parse");
    assert_eq!(plan["errors"][0]["path"], "bad�.json");
    assert_eq!(plan["errors"][0]["path_encoded"], "bad%E9.json");
}
