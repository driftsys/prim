//! Regression tests for every known way `prim init` has reported success
//! while the `.editorconfig` it left behind resolved differently from what
//! prim intended.
//!
//! Split by what each test pins: [`placement`] is about where prim puts a
//! write, [`reporting`] is about what prim says about the file it leaves
//! behind. Both share the helpers below, and both carry the same rule: each
//! test pins the **outcome** — the `prim_mdlint_strict` a representative
//! path actually resolves to before and after the run, through the real
//! `.editorconfig` cascade — rather than the bytes prim happened to write.
//! Text assertions have missed every one of these in turn; resolution
//! assertions cannot.

use std::fs;
use std::path::Path;

use crate::mdlint_policy;

mod placement;
mod reporting;

/// The tier `relative` resolves to under `dir`'s `.editorconfig`.
fn strict_for(dir: &Path, relative: &str) -> bool {
    mdlint_policy::resolve(&dir.join(relative)).strict
}

fn fixture(content: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".editorconfig"), content).unwrap();
    dir
}

fn editorconfig(dir: &Path) -> String {
    fs::read_to_string(dir.join(".editorconfig")).unwrap()
}

/// The warnings `merge` would report for `dir`'s `.editorconfig`, joined —
/// `run` prints them to stderr, which a unit test cannot read.
fn warnings_of(dir: &Path) -> String {
    warnings_of_glob(dir, "docs/**.md")
}

fn warnings_of_glob(dir: &Path, strict_glob: &str) -> String {
    crate::init::merge(&editorconfig(dir), strict_glob)
        .warnings
        .join("\n")
}
