//! File classification (FR-2.4/2.5): decide whether prim owns a file, and what
//! kind it is, from its name/extension alone — never by sniffing content.

use std::path::Path;

/// The kind of file prim recognises. Parsed formats receive structured
/// canonicalisation plus whitespace hygiene; `Orphan` files (the un-owned text
/// allowlist) only ever receive whitespace hygiene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Markdown,
    Json,
    Jsonc,
    Yaml,
    Toml,
    /// An un-owned text file on the curated allowlist (e.g. `.gitignore`).
    Orphan,
}

/// Classify `path` by its final component. Returns `None` for anything prim
/// does not own (source code, unknown types, binaries) — those are left
/// byte-for-byte unchanged.
pub fn classify(path: &Path) -> Option<FileKind> {
    let name = path.file_name()?.to_str()?;

    // Parsed formats and the extension-based orphan patterns (*.txt, *.text).
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        match ext.to_ascii_lowercase().as_str() {
            "md" | "markdown" => return Some(FileKind::Markdown),
            "json" => return Some(FileKind::Json),
            "jsonc" => return Some(FileKind::Jsonc),
            "yaml" | "yml" => return Some(FileKind::Yaml),
            "toml" => return Some(FileKind::Toml),
            "txt" | "text" => return Some(FileKind::Orphan),
            _ => {}
        }
    }

    is_orphan(name).then_some(FileKind::Orphan)
}

/// Whether `name` is on the curated orphan allowlist (documented in
/// `docs/USAGE.md`). `.env` files are deliberately excluded: their values are
/// data and may be whitespace-sensitive.
fn is_orphan(name: &str) -> bool {
    const EXACT: &[&str] = &[
        ".gitignore",
        ".gitattributes",
        ".dockerignore",
        ".npmignore",
        ".eslintignore",
        ".prettierignore",
        ".primignore",
        ".helmignore",
        ".editorconfig",
        ".containerignore",
        ".mailmap",
        "CODEOWNERS",
        "Dockerfile",
        "Containerfile",
        "AUTHORS",
        "CONTRIBUTORS",
        "NOTICE",
        "COPYING",
    ];

    EXACT.contains(&name)
        || name.starts_with("Dockerfile.") // Dockerfile.*
        || name.starts_with("LICENSE") // LICENSE*
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(p: &str) -> Option<FileKind> {
        classify(Path::new(p))
    }

    #[test]
    fn parsed_formats_by_extension() {
        assert_eq!(k("a.md"), Some(FileKind::Markdown));
        assert_eq!(k("a.markdown"), Some(FileKind::Markdown));
        assert_eq!(k("a.json"), Some(FileKind::Json));
        assert_eq!(k("a.jsonc"), Some(FileKind::Jsonc));
        assert_eq!(k("a.yaml"), Some(FileKind::Yaml));
        assert_eq!(k("a.yml"), Some(FileKind::Yaml));
        assert_eq!(k("a.toml"), Some(FileKind::Toml));
    }

    #[test]
    fn orphan_allowlist_dotfiles() {
        for name in [
            ".gitignore",
            ".gitattributes",
            ".dockerignore",
            ".npmignore",
            ".eslintignore",
            ".prettierignore",
            ".primignore",
            ".helmignore",
            ".editorconfig",
            ".containerignore",
            ".mailmap",
        ] {
            assert_eq!(k(name), Some(FileKind::Orphan), "{name}");
        }
    }

    #[test]
    fn orphan_allowlist_patterns_and_names() {
        assert_eq!(k("Dockerfile"), Some(FileKind::Orphan));
        assert_eq!(k("Dockerfile.dev"), Some(FileKind::Orphan));
        assert_eq!(k("Containerfile"), Some(FileKind::Orphan));
        assert_eq!(k("CODEOWNERS"), Some(FileKind::Orphan));
        assert_eq!(k("LICENSE"), Some(FileKind::Orphan));
        assert_eq!(k("LICENSE.txt"), Some(FileKind::Orphan));
        assert_eq!(k("AUTHORS"), Some(FileKind::Orphan));
        assert_eq!(k("CONTRIBUTORS"), Some(FileKind::Orphan));
        assert_eq!(k("NOTICE"), Some(FileKind::Orphan));
        assert_eq!(k("COPYING"), Some(FileKind::Orphan));
        assert_eq!(k("notes.txt"), Some(FileKind::Orphan));
        assert_eq!(k("readme.text"), Some(FileKind::Orphan));
    }

    #[test]
    fn non_owned_returns_none() {
        assert_eq!(k("main.rs"), None);
        assert_eq!(k("script.py"), None);
        assert_eq!(k("logo.png"), None);
        assert_eq!(k(".env"), None); // data values, not metadata — excluded.
        assert_eq!(k(".env.local"), None);
        assert_eq!(k("Makefile"), None); // Make is out of v1 scope.
        assert_eq!(k("run.sh"), None); // Shell is deferred to Phase 2.
        assert_eq!(k("noext"), None);
    }

    #[test]
    fn classifies_by_final_component_of_a_path() {
        assert_eq!(k("src/docs/guide.md"), Some(FileKind::Markdown));
        assert_eq!(k("/etc/project/.gitignore"), Some(FileKind::Orphan));
    }
}
