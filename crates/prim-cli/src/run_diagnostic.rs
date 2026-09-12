//! Stable operational diagnostics shared by reports, plans, and the registry.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Static metadata for one operational diagnostic code.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Definition {
    pub(crate) code: &'static str,
    pub(crate) description: &'static str,
}

pub(crate) const FORMAT_DRIFT: Definition = Definition {
    code: "format::drift",
    description: "file does not match prim's canonical format",
};

pub(crate) const INPUT_READ: Definition = Definition {
    code: "input::read",
    description: "prim could not read a selected input",
};
pub(crate) const FORMAT_PARSE: Definition = Definition {
    code: "format::parse",
    description: "a selected structured input could not be parsed",
};
pub(crate) const INTERNAL_PANIC: Definition = Definition {
    code: "internal::panic",
    description: "prim contained an internal formatter or linter panic",
};
pub(crate) const SCOPE_EMPTY: Definition = Definition {
    code: "scope::empty",
    description: "every selected path was excluded from processing",
};
pub(crate) const SCOPE_RESOLVE: Definition = Definition {
    code: "scope::resolve",
    description: "prim could not resolve the requested file scope",
};

const OPERATIONAL_DEFINITIONS: &[Definition] = &[
    FORMAT_PARSE,
    INPUT_READ,
    INTERNAL_PANIC,
    SCOPE_EMPTY,
    SCOPE_RESOLVE,
];

pub(crate) fn operational_definitions() -> &'static [Definition] {
    OPERATIONAL_DEFINITIONS
}

/// One failure encountered during a command invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunDiagnostic {
    pub(crate) code: &'static str,
    description: &'static str,
    pub(crate) path: Option<PathBuf>,
    pub(crate) message: String,
}

impl RunDiagnostic {
    pub(crate) fn at(definition: &'static Definition, path: &Path, message: String) -> Self {
        Self {
            code: definition.code,
            description: definition.description,
            path: Some(path.to_path_buf()),
            message,
        }
    }

    pub(crate) fn run_wide(definition: &'static Definition, message: impl Into<String>) -> Self {
        Self {
            code: definition.code,
            description: definition.description,
            path: None,
            message: message.into(),
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        self.description
    }

    pub(crate) fn machine(&self) -> MachineDiagnostic<'_> {
        MachineDiagnostic {
            code: self.code,
            message: &self.message,
            path: self.path.as_deref().map(crate::machine_path::display),
            path_encoded: self.path.as_deref().and_then(crate::machine_path::encoded),
        }
    }
}

/// Serializable projection that preserves a non-UTF-8 path's exact bytes.
#[derive(Serialize)]
pub(crate) struct MachineDiagnostic<'a> {
    code: &'static str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_encoded: Option<String>,
}
