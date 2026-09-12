//! A pinned consumer accepts additive v1 metadata while validating known fields.

use serde_json::{Value, json};

#[test]
fn published_v1_schemas_accept_additive_metadata() {
    let schemas = [
        (
            include_str!("../../../schemas/prim-registry-v1.schema.json"),
            json!({
                "schema_version": 1, "tool": {"name": "prim", "version": "0.9.0", "new_tool_field": true},
            "diagnostics": [{"code":"MD041","description":"heading","category":"markdown",
                "default_severity":"error","formats":["markdown"],"enabled_by":"always",
                "configuration_keys":[],"inline_controls":[],"can_disable":true,"new_field":42}],
            "aliases": [{"code":"old","replacement":"MD041","new_field":42}],
            "retired": [{"code":"retired","description":"no longer emitted","new_field":42}], "new_field": 42
            }),
        ),
        (
            include_str!("../../../schemas/prim-effect-plan-v1.schema.json"),
            json!({
                "schema_version": 1, "tool": {"name": "prim", "version": "0.9.0", "new_tool_field": true},
            "operation": "fmt", "effects": [{"path":"a.txt","kind":"orphan","operation":"replace_contents",
                "before":{"sha256":"a".repeat(64),"bytes":3,"new_field":42},
                "after":{"sha256":"b".repeat(64),"bytes":2,"new_field":42},
                "configuration":{"end_of_line":"lf","trim_trailing_whitespace":true,
                    "insert_final_newline":true,"indent_style":"space","indent_size":2,
                    "max_line_length":null,"new_field":42},"new_field":42}],
            "errors": [{"code":"input::read","message":"unreadable","new_field":42}], "new_field": 42
            }),
        ),
    ];
    for (source, mut document) in schemas {
        let schema: Value = serde_json::from_str(source).unwrap();
        let validator = jsonschema::validator_for(&schema).unwrap();
        assert!(
            validator.is_valid(&document),
            "additive metadata must be accepted: {document}"
        );
        document["schema_version"] = json!(2);
        assert!(!validator.is_valid(&document));
        document["schema_version"] = json!(1);
        document["tool"]["name"] = json!("another-tool");
        assert!(!validator.is_valid(&document));
    }
}
