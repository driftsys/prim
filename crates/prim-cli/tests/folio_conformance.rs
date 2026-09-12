//! Black-box conformance tests for Folio's explicit-file delegation boundary.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

fn prim() -> Command {
    Command::cargo_bin("prim").expect("prim binary builds")
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn seed_repository(root: &Path) {
    write(
        root,
        ".editorconfig",
        "root = true\n[*]\nindent_style = space\nindent_size = 2\n\n[docs/*.md]\nmax_line_length = 20\n",
    );
    write(root, "README.md", "#    Prim\n");
    write(root, "config.json", "{\"answer\":42}\n");
    write(root, "config.jsonc", "{\n// retained\n\"answer\":42,\n}\n");
    write(root, "config.yaml", "answer:    42\n");
    write(root, "config.toml", "answer=42\n");
    write(root, "notes.txt", "delegated  \n");
    write(
        root,
        "docs/path with spaces.md",
        "#    Delegated\n\none two three four five six seven\n",
    );
    write(root, "script.sh", "echo delegated  \n");
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else if path.is_file() {
                files.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn explicit_batch_formats_only_named_owned_non_ignored_files() {
    let repository = tempfile::tempdir().unwrap();
    let root = repository.path();
    seed_repository(root);
    write(root, ".primignore", "/ignored.md\n");
    write(root, "ignored.md", "#    Ignored\n");
    write(root, "package-lock.json", "{\"generated\":true}");
    write(root, "unselected.md", "#    Unselected\n");
    write(root, "symlink-target.md", "#    Symlink target\n");

    #[cfg(unix)]
    std::os::unix::fs::symlink("symlink-target.md", root.join("linked.md")).unwrap();

    let selected = vec![
        "README.md",
        "config.json",
        "config.jsonc",
        "config.yaml",
        "config.toml",
        "notes.txt",
        "docs/path with spaces.md",
        "ignored.md",
        "package-lock.json",
        "script.sh",
        #[cfg(unix)]
        "linked.md",
    ];

    prim()
        .current_dir(root)
        .arg("fmt")
        .args(selected)
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(root.join("README.md")).unwrap(),
        "# Prim\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("config.json")).unwrap(),
        "{ \"answer\": 42 }\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("config.jsonc")).unwrap(),
        "{\n  // retained\n  \"answer\": 42\n}\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("config.yaml")).unwrap(),
        "answer: 42\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("config.toml")).unwrap(),
        "answer = 42\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "delegated\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("docs/path with spaces.md")).unwrap(),
        "# Delegated\n\none two three four\nfive six seven\n"
    );

    assert_eq!(
        fs::read_to_string(root.join("ignored.md")).unwrap(),
        "#    Ignored\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("package-lock.json")).unwrap(),
        "{\"generated\":true}"
    );
    assert_eq!(
        fs::read_to_string(root.join("unselected.md")).unwrap(),
        "#    Unselected\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("script.sh")).unwrap(),
        "echo delegated  \n"
    );
    assert_eq!(
        fs::read_to_string(root.join("symlink-target.md")).unwrap(),
        "#    Symlink target\n"
    );
}

#[test]
fn explicit_batch_and_directory_walk_produce_identical_bytes() {
    let walked = tempfile::tempdir().unwrap();
    let delegated = tempfile::tempdir().unwrap();
    seed_repository(walked.path());
    seed_repository(delegated.path());

    prim()
        .current_dir(walked.path())
        .args(["fmt", "."])
        .assert()
        .success();

    prim()
        .current_dir(delegated.path())
        .arg("fmt")
        .args([
            ".editorconfig",
            "README.md",
            "config.json",
            "config.jsonc",
            "config.yaml",
            "config.toml",
            "notes.txt",
            "docs/path with spaces.md",
        ])
        .assert()
        .success();

    assert_eq!(snapshot(walked.path()), snapshot(delegated.path()));
}

#[test]
fn verification_lint_and_fix_do_not_widen_an_explicit_file_list() {
    let check_repository = tempfile::tempdir().unwrap();
    write(check_repository.path(), "selected.json", "{\"a\":1}\n");
    write(check_repository.path(), "unselected.json", "{\"b\":2}\n");
    let output = prim()
        .current_dir(check_repository.path())
        .args(["fmt", "--check", "--format", "json", "selected.json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"][0]["path"], "selected.json");
    assert_eq!(
        fs::read_to_string(check_repository.path().join("unselected.json")).unwrap(),
        "{\"b\":2}\n"
    );

    let lint_repository = tempfile::tempdir().unwrap();
    write(lint_repository.path(), "selected.txt", "selected  \n");
    write(lint_repository.path(), "unselected.txt", "unselected  \n");
    let output = prim()
        .current_dir(lint_repository.path())
        .args(["lint", "--format", "json", "selected.txt"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(report["findings"].as_array().unwrap().len(), 1);
    assert_eq!(report["findings"][0]["path"], "selected.txt");

    let fix_repository = tempfile::tempdir().unwrap();
    write(fix_repository.path(), "selected.txt", "selected  \n");
    write(fix_repository.path(), "unselected.txt", "unselected  \n");
    prim()
        .current_dir(fix_repository.path())
        .args(["fix", "selected.txt"])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(fix_repository.path().join("selected.txt")).unwrap(),
        "selected\n"
    );
    assert_eq!(
        fs::read_to_string(fix_repository.path().join("unselected.txt")).unwrap(),
        "unselected  \n"
    );
}

#[test]
fn directory_exclusions_use_the_resolved_parent_of_dotdot_paths() {
    let repository = tempfile::tempdir().unwrap();
    write(
        repository.path(),
        "docs/nested/excluded.txt",
        "excluded  \n",
    );
    write(repository.path(), "selected.txt", "selected  \n");
    for relative in ["docs/../selected.txt", "docs/nested/../../selected.txt"] {
        for path in [PathBuf::from(relative), repository.path().join(relative)] {
            let output = prim()
                .current_dir(repository.path())
                .args(["fmt", "--dry-run", "--format", "json", "--exclude", "docs/"])
                .arg(path)
                .assert()
                .code(1)
                .get_output()
                .stdout
                .clone();
            let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(report["effects"].as_array().unwrap().len(), 1);
            assert_eq!(report["errors"], serde_json::json!([]));
        }
    }
}

#[test]
#[cfg(unix)]
fn parent_resolution_follows_directory_symlinks_before_excluding() {
    let repository = tempfile::tempdir().unwrap();
    write(repository.path(), "docs/nested/file.txt", "nested\n");
    write(repository.path(), "docs/excluded.txt", "excluded  \n");
    std::os::unix::fs::symlink("docs/nested", repository.path().join("link")).unwrap();
    prim()
        .current_dir(repository.path())
        .args([
            "fmt",
            "--dry-run",
            "--format",
            "json",
            "--exclude",
            "docs/",
            "link/../excluded.txt",
        ])
        .assert()
        .code(2);
    assert_eq!(
        fs::read_to_string(repository.path().join("docs/excluded.txt")).unwrap(),
        "excluded  \n"
    );
}

#[test]
fn directory_excludes_filter_explicit_descendants() {
    for verb in ["fmt", "fix", "lint"] {
        for is_absolute in [false, true] {
            for exclude in ["docs/", "/docs/", "/docs/nested/path with spaces.md"] {
                let repository = tempfile::tempdir().unwrap();
                let relative = "docs/nested/path with spaces.md";
                let original = "#    Excluded\n";
                write(repository.path(), relative, original);
                let path = if is_absolute {
                    repository.path().join(relative)
                } else {
                    PathBuf::from(relative)
                };
                let mut command = prim();
                command.current_dir(repository.path()).args([
                    verb,
                    "--exclude",
                    exclude,
                    "--format",
                    "json",
                ]);
                if verb != "lint" {
                    command.arg("--dry-run");
                }
                let output = command
                    .arg(&path)
                    .assert()
                    .code(2)
                    .get_output()
                    .stdout
                    .clone();
                let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
                assert_eq!(report["errors"][0]["code"], "scope::empty");
                assert_eq!(
                    fs::read_to_string(repository.path().join(relative)).unwrap(),
                    original
                );
                prim()
                    .current_dir(repository.path())
                    .args(["fmt", "--exclude", exclude])
                    .arg(path)
                    .assert()
                    .success();
                assert_eq!(
                    fs::read_to_string(repository.path().join(relative)).unwrap(),
                    original
                );
            }
        }
    }
}

#[test]
fn exclude_globs_filter_explicit_files_as_well_as_walked_files() {
    let repository = tempfile::tempdir().unwrap();
    write(repository.path(), "excluded.json", "{\"excluded\":true}\n");
    write(repository.path(), "included.txt", "included  \n");

    prim()
        .current_dir(repository.path())
        .args([
            "fmt",
            "--exclude",
            "*.json",
            "excluded.json",
            "included.txt",
        ])
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(repository.path().join("excluded.json")).unwrap(),
        "{\"excluded\":true}\n"
    );
    assert_eq!(
        fs::read_to_string(repository.path().join("included.txt")).unwrap(),
        "included\n"
    );
}
