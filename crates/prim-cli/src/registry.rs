//! Versioned diagnostic registry for CLI orchestrators.

use serde::Serialize;

use crate::mdlint_policy::{MDLINT_DISABLE_KEY, MDLINT_REPORT_LINE_LENGTH_KEY, MDLINT_STRICT_KEY};
use crate::provenance::{
    END_OF_LINE_KEY, INDENT_SIZE_KEY, INDENT_STYLE_KEY, INSERT_FINAL_NEWLINE_KEY,
    MAX_LINE_LENGTH_KEY, TAB_WIDTH_KEY, TRIM_TRAILING_WHITESPACE_KEY,
};
use crate::run_diagnostic::{self, FORMAT_DRIFT};
use prim_fmt::MarkdownTier;

const ALL_FORMATS: &[&str] = &["json", "jsonc", "markdown", "orphan", "toml", "yaml"];
const PARSED_FORMATS: &[&str] = &["json", "jsonc", "markdown", "toml", "yaml"];
const MARKDOWN_FORMAT: &[&str] = &["markdown"];
const ORPHAN_FORMAT: &[&str] = &["orphan"];
const INLINE_DISABLES: &[&str] = &[
    "markdownlint-configure-file",
    "markdownlint-disable",
    "rumdl-disable",
];
const INLINE_STRICT_DISABLES: &[&str] = &[
    "markdownlint-configure-file",
    "markdownlint-disable",
    "prim-mdlint-strict",
    "rumdl-disable",
];

#[derive(Serialize)]
struct Registry {
    schema_version: u8,
    tool: Tool,
    diagnostics: Vec<Diagnostic>,
    aliases: Vec<Alias>,
    retired: Vec<Retired>,
}

#[derive(Serialize)]
struct Tool {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct Diagnostic {
    code: &'static str,
    description: &'static str,
    category: &'static str,
    default_severity: &'static str,
    formats: &'static [&'static str],
    enabled_by: &'static str,
    configuration_keys: Vec<&'static str>,
    inline_controls: &'static [&'static str],
    can_disable: bool,
}

#[derive(Serialize)]
struct Alias {
    code: &'static str,
    replacement: &'static str,
}

#[derive(Serialize)]
struct Retired {
    code: &'static str,
    description: &'static str,
}

/// Render the complete v1 registry as one JSON document.
pub(crate) fn render() -> String {
    let mut diagnostics = Vec::new();
    diagnostics.push(Diagnostic {
        code: FORMAT_DRIFT.code,
        description: FORMAT_DRIFT.description,
        category: "format",
        default_severity: "error",
        formats: ALL_FORMATS,
        enabled_by: "always",
        configuration_keys: vec![
            END_OF_LINE_KEY,
            INDENT_SIZE_KEY,
            INDENT_STYLE_KEY,
            INSERT_FINAL_NEWLINE_KEY,
            MAX_LINE_LENGTH_KEY,
            TAB_WIDTH_KEY,
            TRIM_TRAILING_WHITESPACE_KEY,
        ],
        inline_controls: &[],
        can_disable: false,
    });

    diagnostics.extend(
        prim_fmt::hygiene_diagnostic_definitions()
            .iter()
            .map(hygiene),
    );
    diagnostics.extend(
        prim_fmt::markdown_rule_definitions()
            .into_iter()
            .map(markdown),
    );
    diagnostics.extend(
        run_diagnostic::operational_definitions()
            .iter()
            .map(operation),
    );
    diagnostics.sort_by_key(|diagnostic| diagnostic.code);

    let registry = Registry {
        schema_version: 1,
        tool: Tool {
            name: "prim",
            version: env!("CARGO_PKG_VERSION"),
        },
        diagnostics,
        aliases: Vec::new(),
        retired: Vec::new(),
    };
    serde_json::to_string_pretty(&registry).expect("registry serialization should succeed") + "\n"
}

fn hygiene(definition: &prim_fmt::DiagnosticDefinition) -> Diagnostic {
    let (enabled_by, configuration_keys, can_disable) = match definition.code {
        "hygiene::bom" => ("always", vec![], false),
        "hygiene::eol" => ("editorconfig", vec![END_OF_LINE_KEY], false),
        "hygiene::final-newline" => ("editorconfig", vec![INSERT_FINAL_NEWLINE_KEY], true),
        "hygiene::indent" => (
            "editorconfig",
            vec![INDENT_SIZE_KEY, INDENT_STYLE_KEY],
            false,
        ),
        "hygiene::trailing-whitespace" => {
            ("editorconfig", vec![TRIM_TRAILING_WHITESPACE_KEY], true)
        }
        code => unreachable!("unknown engine hygiene definition {code}"),
    };
    Diagnostic {
        code: definition.code,
        description: definition.description,
        category: "hygiene",
        default_severity: "error",
        formats: ORPHAN_FORMAT,
        enabled_by,
        configuration_keys,
        inline_controls: &[],
        can_disable,
    }
}

fn markdown(definition: prim_fmt::MarkdownRuleDefinition) -> Diagnostic {
    let (enabled_by, configuration_keys, inline_controls) = match definition.tier {
        MarkdownTier::Floor => ("always", vec![MDLINT_DISABLE_KEY], INLINE_DISABLES),
        MarkdownTier::Strict => (
            MDLINT_STRICT_KEY,
            vec![MDLINT_DISABLE_KEY, MDLINT_STRICT_KEY],
            INLINE_STRICT_DISABLES,
        ),
        MarkdownTier::LineLength => (
            MDLINT_REPORT_LINE_LENGTH_KEY,
            vec![
                MAX_LINE_LENGTH_KEY,
                MDLINT_DISABLE_KEY,
                MDLINT_REPORT_LINE_LENGTH_KEY,
                MDLINT_STRICT_KEY,
            ],
            INLINE_STRICT_DISABLES,
        ),
    };
    Diagnostic {
        code: definition.code,
        description: definition.description,
        category: "markdown",
        default_severity: "error",
        formats: MARKDOWN_FORMAT,
        enabled_by,
        configuration_keys,
        inline_controls,
        can_disable: true,
    }
}

fn operation(definition: &run_diagnostic::Definition) -> Diagnostic {
    let formats: &'static [&'static str] = match definition.code {
        "format::parse" => PARSED_FORMATS,
        "input::read" | "internal::panic" => ALL_FORMATS,
        "scope::empty" | "scope::resolve" => &[],
        code => unreachable!("unknown operational definition {code}"),
    };
    Diagnostic {
        code: definition.code,
        description: definition.description,
        category: "operation",
        default_severity: "error",
        formats,
        enabled_by: "runtime",
        configuration_keys: Vec::new(),
        inline_controls: &[],
        can_disable: false,
    }
}
