//! File classification (FR-2.4/2.5): decide whether prim owns a file, and what
//! kind it is, from its name/extension alone — never by sniffing content.

use std::ffi::OsStr;
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
    // Extension first, and deliberately before decoding the whole name: the
    // extension decides on its own, and a name that is not valid UTF-8 must
    // not stop it (#168). On a platform whose filenames are byte strings,
    // `caf\xe9.txt` is a text file like any other.
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

    is_orphan(path.file_name()?).then_some(FileKind::Orphan)
}

/// Whether `name` is on the curated orphan allowlist (documented in
/// `docs/USAGE.md`). `.env` files are deliberately excluded: their values are
/// data and may be whitespace-sensitive.
fn is_orphan(name: &OsStr) -> bool {
    const EXACT: &[&str] = &[
        ".gitignore",
        ".gitattributes",
        ".gitmodules",
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

    // Compared as bytes, not as text. `as_encoded_bytes` is self-consistent on
    // every platform and is documented as safe to compare against ASCII, which
    // each entry and prefix below is. Decoding the name first would drop a
    // filename that is not valid UTF-8 (#168), so `Dockerfile.d\xe9v` would be
    // left alone while `Dockerfile.dev` was formatted, for no reason a user
    // could act on.
    let name = name.as_encoded_bytes();

    EXACT.iter().any(|entry| name == entry.as_bytes())
        || name.starts_with(b"Dockerfile.") // Dockerfile.*
        || name.starts_with(b"LICENSE") // LICENSE*
}

#[cfg(test)]
mod tests {
    /// #168: on a platform whose filenames are byte strings, a name that is
    /// not valid UTF-8 is still a file prim owns when its extension says so.
    /// Deciding on the whole name first dropped it, which made the changed-file
    /// selection fix a no-op: the path was selected and then discarded here.
    #[cfg(unix)]
    #[test]
    fn an_extension_classifies_a_name_that_is_not_valid_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        for (bytes, expected) in [
            (b"caf\xe9.txt".as_slice(), super::FileKind::Orphan),
            (b"caf\xe9.md".as_slice(), super::FileKind::Markdown),
            (b"caf\xe9.json".as_slice(), super::FileKind::Json),
        ] {
            let path = Path::new(OsStr::from_bytes(bytes));
            assert_eq!(super::classify(path), Some(expected), "{path:?}");
        }
    }

    /// The allowlist matches bytes, so its prefix rules reach a name that is
    /// not valid UTF-8 exactly as they reach a decodable one. Comparing text
    /// would claim `Dockerfile.dev` and quietly skip `Dockerfile.d\xe9v`.
    #[cfg(unix)]
    #[test]
    fn the_orphan_allowlist_reaches_a_name_that_is_not_valid_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        for bytes in [b"Dockerfile.d\xe9v".as_slice(), b"LICENSE\xe9".as_slice()] {
            let path = Path::new(OsStr::from_bytes(bytes));
            assert_eq!(
                super::classify(path),
                Some(super::FileKind::Orphan),
                "{path:?}"
            );
        }
    }

    /// A name that is neither on the allowlist nor prefixed by one of its
    /// rules stays unowned, decodable or not.
    #[cfg(unix)]
    #[test]
    fn an_undecodable_name_off_the_allowlist_is_not_claimed() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        use std::path::Path;

        let path = Path::new(OsStr::from_bytes(b".gitign\xe9re"));

        assert_eq!(super::classify(path), None);
    }

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
            ".gitmodules",
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
        assert_eq!(k(".gitconfig"), None); // user/machine-local, not committed.
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
