#[path = "correctness/spec_parser.rs"]
mod spec_parser;

// Cross-cutting correctness harness (FR-6.1 idempotency, FR-6.2 semantic
// preservation) plus format-equality assertions, all driven from the
// plain-text fixtures under `tests/correctness/fixtures/`. Adding coverage
// means adding a `.txt` fixture — see
// `docs/wip/plans/2026-07-02-spec-test-harness-plan.md` for the format.

use prim_fmt::{FileKind, format};
use spec_parser::{SpecCase, discover, parse_spec_file, rewrite_expected};
use std::path::{Path, PathBuf};

fn fixtures_root() -> PathBuf {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/correctness/fixtures"
    ))
    .to_path_buf()
}

fn load_cases() -> Vec<(FileKind, SpecCase)> {
    discover(&fixtures_root())
        .into_iter()
        .map(|(kind, path)| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let name = path.display().to_string();
            (kind, parse_spec_file(&name, &text))
        })
        .collect()
}

#[test]
fn spec_cases_format_as_expected() {
    let update = std::env::var_os("PRIM_SPEC_UPDATE").is_some();
    let mut failures = Vec::new();
    for (kind, case) in load_cases() {
        let actual = format(kind, &case.input, &case.style).expect("formats");
        if actual != case.expected {
            if update {
                rewrite_expected(Path::new(&case.name), &actual);
            } else {
                failures.push(case.name.clone());
            }
        }
    }
    assert!(
        failures.is_empty(),
        "spec cases produced unexpected output: {failures:?}\n\
         run `PRIM_SPEC_UPDATE=1 cargo test -p prim-fmt --test correctness \
         spec_cases_format_as_expected` to regenerate, then review the diff \
         before committing"
    );
}

#[test]
fn spec_cases_are_idempotent() {
    for (kind, case) in load_cases() {
        let once = format(kind, &case.input, &case.style).expect("formats");
        let twice = format(kind, &once, &case.style).expect("formats");
        assert_eq!(once, twice, "not idempotent: {}", case.name);
    }
}

fn json_value(text: &str) -> serde_json::Value {
    jsonc_parser::parse_to_serde_value::<serde_json::Value>(
        text,
        &jsonc_parser::ParseOptions::default(),
    )
    .expect("parses")
}

#[test]
fn spec_cases_preserve_json_data_model() {
    for (kind, case) in load_cases() {
        if !matches!(kind, FileKind::Json | FileKind::Jsonc) {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        assert_eq!(
            json_value(&case.input),
            json_value(&actual),
            "JSON data model changed: {}",
            case.name
        );
    }
}

#[test]
fn spec_cases_preserve_toml_data_model() {
    for (kind, case) in load_cases() {
        if kind != FileKind::Toml {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        let before: toml::Table = case.input.parse().expect("parses");
        let after: toml::Table = actual.parse().expect("parses");
        assert_eq!(before, after, "TOML data model changed: {}", case.name);
    }
}

#[test]
fn spec_cases_preserve_yaml_data_model() {
    use yaml_rust2::YamlLoader;
    for (kind, case) in load_cases() {
        if kind != FileKind::Yaml {
            continue;
        }
        let actual = format(kind, &case.input, &case.style).expect("formats");
        let before = YamlLoader::load_from_str(&case.input).expect("parses");
        let after = YamlLoader::load_from_str(&actual).expect("parses");
        assert_eq!(before, after, "YAML data model changed: {}", case.name);
    }
}

#[test]
fn formatted_markdown_never_reports_its_own_line_length() {
    // FR-3.2d's load-bearing promise: `prim_mdlint_report_line_length` measures
    // against the width the formatter wrapped to, so `prim fmt` output cannot
    // fail `prim lint`. Every other test for this feature uses a hand-written
    // fixture; this one runs the whole Markdown corpus through the real
    // formatter and lints the result, so a change in how dprint wraps — a
    // dependency bump, a new construct — is caught here rather than in a
    // user's CI.
    //
    // At the floor tier nothing prim produced may be reported. At the strict
    // tier a long heading may be, and only a heading: prim cannot wrap one, so
    // it is the one construct the formatter knowingly leaves over the limit.
    for (kind, case) in load_cases() {
        if kind != FileKind::Markdown {
            continue;
        }
        let formatted = format(kind, &case.input, &case.style).expect("formats");
        let width = case.style.max_line_length.unwrap_or(80);

        let floor = prim_fmt::lint_markdown(&formatted, false, &[], Some(width));
        let offenders: Vec<_> = floor.iter().filter(|d| d.rule == "MD013").collect();
        assert!(
            offenders.is_empty(),
            "{}: prim fmt output reports its own line length at width {width}: {offenders:?}",
            case.name
        );

        let strict = prim_fmt::lint_markdown(&formatted, true, &[], Some(width));
        for diagnostic in strict.iter().filter(|d| d.rule == "MD013") {
            let line = formatted
                .lines()
                .nth(diagnostic.line.saturating_sub(1))
                .unwrap_or_default();
            assert!(
                line.trim_start().starts_with('#'),
                "{}: strict tier reports a non-heading line prim produced: {line:?}",
                case.name
            );
        }
    }
}
