//! Files a generating tool owns outright (FR-2.7).

use std::path::Path;

/// Files whose generating tool rewrites them wholesale, paired with that tool.
///
/// Admission requires all three: the tool's own documentation calls the file
/// generated and not hand-edited, the file is conventionally committed, and
/// the file is inside prim's format surface so listing it changes behaviour.
/// See AD-0011 for the candidates that were rejected and why.
const GENERATED: &[(&str, &str)] = &[
    ("npm-shrinkwrap.json", "npm"),
    ("package-lock.json", "npm"),
    ("packages.lock.json", "NuGet"),
    ("pnpm-lock.yaml", "pnpm"),
];

/// The tool that generates `path`, or `None` when prim may format it.
///
/// Keyed on the final path component, exact and case-sensitive, the same way
/// [`crate::classify()`] works — never by sniffing content (FR-2.5).
pub fn generated_by(path: &Path) -> Option<&'static str> {
    let name = path.file_name()?.to_str()?;
    GENERATED
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, tool)| *tool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn names_the_generator_for_each_listed_file() {
        for (name, tool) in [
            ("package-lock.json", "npm"),
            ("npm-shrinkwrap.json", "npm"),
            ("pnpm-lock.yaml", "pnpm"),
            ("packages.lock.json", "NuGet"),
        ] {
            assert_eq!(generated_by(Path::new(name)), Some(tool), "{name}");
        }
    }

    #[test]
    fn authored_files_have_no_generator() {
        for name in [
            "package.json",
            "pnpm-workspace.yaml",
            "Cargo.toml",
            "deno.json",
            "pyproject.toml",
        ] {
            assert_eq!(generated_by(Path::new(name)), None, "{name}");
        }
    }

    #[test]
    fn matching_is_on_the_final_component_only() {
        // A directory that happens to share the name must not match, and a
        // listed name nested in a path must.
        assert_eq!(generated_by(Path::new("package-lock.json/x.json")), None);
        assert_eq!(
            generated_by(Path::new("web/app/package-lock.json")),
            Some("npm")
        );
    }

    #[test]
    fn matching_is_case_sensitive() {
        assert_eq!(generated_by(Path::new("Package-Lock.json")), None);
    }
}
