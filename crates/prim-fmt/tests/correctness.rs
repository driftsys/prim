//! Cross-cutting correctness harness (FR-6.1 idempotency, FR-6.2 semantic
//! preservation). Every formatter is run through both checks over a shared
//! corpus, using independent parsers for the semantic comparison.

use prim_fmt::{FileKind, Style, format};

/// Representative + edge-case inputs per format.
const CORPUS: &[(FileKind, &str)] = &[
    (FileKind::Json, "{\"a\":1,\"b\":[1,2,3],\"c\":{\"d\":true}}"),
    (FileKind::Json, "[]"),
    (FileKind::Jsonc, "{\n// a comment\n\"a\": 1,\n\"b\": 2,\n}"),
    (
        FileKind::Toml,
        "a=1\nb = \"x\"\n[t]\nc=[1,2]\nd = {e=1}\n# comment",
    ),
    (
        FileKind::Yaml,
        "a: 1\nb:\n  - 1\n  - 2\nbase: &id 1\nref: *id\nblock: |\n  l1\n  l2\n# comment",
    ),
    (
        FileKind::Markdown,
        "#  Heading\n\nSome   prose with `inline code` that runs on and on and on well past the wrap.\n\n- one\n- two\n",
    ),
    (FileKind::Orphan, "trailing  \nlines\n\n\n"),
];

#[test]
fn formatting_is_idempotent() {
    let style = Style::default();
    for (kind, input) in CORPUS {
        let once = format(*kind, input, &style).expect("formats");
        let twice = format(*kind, &once, &style).expect("formats");
        assert_eq!(once, twice, "not idempotent for {kind:?}: {input:?}");
    }
}

fn json_value(text: &str) -> serde_json::Value {
    jsonc_parser::parse_to_serde_value::<serde_json::Value>(
        text,
        &jsonc_parser::ParseOptions::default(),
    )
    .expect("parses")
}

fn fmt(kind: FileKind, input: &str) -> String {
    format(kind, input, &Style::default()).expect("formats")
}

#[test]
fn json_and_jsonc_data_model_preserved() {
    for (kind, input) in CORPUS
        .iter()
        .filter(|(k, _)| matches!(k, FileKind::Json | FileKind::Jsonc))
    {
        assert_eq!(
            json_value(input),
            json_value(&fmt(*kind, input)),
            "JSON data model changed: {input:?}"
        );
    }
}

#[test]
fn toml_data_model_preserved() {
    for (_, input) in CORPUS.iter().filter(|(k, _)| matches!(k, FileKind::Toml)) {
        let before: toml::Table = input.parse().expect("parses");
        let after: toml::Table = fmt(FileKind::Toml, input).parse().expect("parses");
        assert_eq!(before, after, "TOML data model changed: {input:?}");
    }
}

#[test]
fn yaml_data_model_preserved() {
    use yaml_rust2::YamlLoader;
    for (_, input) in CORPUS.iter().filter(|(k, _)| matches!(k, FileKind::Yaml)) {
        let before = YamlLoader::load_from_str(input).expect("parses");
        let after = YamlLoader::load_from_str(&fmt(FileKind::Yaml, input)).expect("parses");
        assert_eq!(before, after, "YAML data model changed: {input:?}");
    }
}
