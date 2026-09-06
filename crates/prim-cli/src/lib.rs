//! Internal library target for the `prim` binary.
//!
//! `prim-cli` is a binary crate: the supported surface is the `prim` command
//! line, and the reusable engine is `prim-fmt` (AD-0001). This library target
//! exists so the binary's own modules can be linked by benchmarks and
//! unit-level tests, which cannot depend on a `[[bin]]` target. Nothing here
//! carries an API stability promise (AD-0020).

pub mod app;
pub mod argv;
pub(crate) mod changed_files;
pub mod cli;
pub(crate) mod diff;
pub(crate) mod discover;
pub mod editorconfig;
pub(crate) mod explain;
pub(crate) mod formatting;
pub(crate) mod init;
pub(crate) mod lsp;
pub(crate) mod mdlint_policy;
pub(crate) mod provenance;
pub(crate) mod report;
pub(crate) mod symlink;
pub mod ui;
pub(crate) mod write;
