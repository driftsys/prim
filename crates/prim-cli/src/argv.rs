//! The bare-`prim`-is-`fmt` argv preprocessor (AD-0007 §1).
//!
//! clap cannot cleanly disambiguate an optional subcommand from a leading
//! positional `PATH`, so dispatch is resolved before [`clap::Parser::parse`]:
//! if the first non-flag argument is a known verb (`fmt`/`lint`/`fix`) or a
//! global help/version flag, the argv is left as-is; otherwise an implicit
//! `fmt` is inserted right after the program name. This keeps `prim README.md`,
//! `prim fmt README.md`, and the deprecated `prim --check` all working.

use crate::cli::FmtArgs;

const VERBS: &[&str] = &["fmt", "lint", "fix"];
const GLOBAL_ONLY_FLAGS: &[&str] = &["-h", "--help", "-V", "--version"];

/// Insert an implicit `fmt` verb into `args` (a full argv, including the
/// program name at index 0) when the caller did not name a verb. Returns the
/// (possibly adjusted) argv and whether `fmt` was injected.
pub fn inject_default_verb(args: Vec<String>) -> (Vec<String>, bool) {
    let first = args.get(1).map(String::as_str);
    let needs_fmt = match first {
        None => true,
        Some(token) if VERBS.contains(&token) => false,
        Some(token) if GLOBAL_ONLY_FLAGS.contains(&token) => false,
        Some(_) => true,
    };

    if !needs_fmt {
        return (args, false);
    }

    let mut adjusted = args;
    adjusted.insert(1, "fmt".to_string());
    (adjusted, true)
}

/// Which deprecated top-level flag (if any) `args` was parsed from, when
/// `fmt` was injected implicitly. `None` means the run used none of the
/// deprecated top-level spellings (e.g. bare `prim README.md`, which is the
/// permanent, non-deprecated `fmt` alias).
pub fn deprecated_flag(args: &FmtArgs) -> Option<&'static str> {
    if args.check {
        Some("--check")
    } else if args.diff {
        Some("--diff")
    } else if args.stdin_filepath.is_some() {
        Some("--stdin-filepath")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(rest: &[&str]) -> Vec<String> {
        std::iter::once("prim")
            .chain(rest.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn bare_invocation_injects_fmt() {
        let (adjusted, injected) = inject_default_verb(argv(&[]));
        assert_eq!(adjusted, vec!["prim", "fmt"]);
        assert!(injected);
    }

    #[test]
    fn positional_path_injects_fmt_before_it() {
        let (adjusted, injected) = inject_default_verb(argv(&["README.md"]));
        assert_eq!(adjusted, vec!["prim", "fmt", "README.md"]);
        assert!(injected);
    }

    #[test]
    fn deprecated_top_level_flags_inject_fmt() {
        for flag in ["--check", "--diff", "--color"] {
            let (adjusted, injected) = inject_default_verb(argv(&[flag]));
            assert_eq!(adjusted, vec!["prim", "fmt", flag], "flag: {flag}");
            assert!(injected, "flag: {flag}");
        }
    }

    #[test]
    fn known_verbs_are_left_alone() {
        for verb in VERBS {
            let (adjusted, injected) = inject_default_verb(argv(&[verb]));
            assert_eq!(adjusted, vec!["prim".to_string(), verb.to_string()]);
            assert!(!injected, "verb: {verb}");
        }
    }

    #[test]
    fn global_help_and_version_flags_are_left_alone() {
        for flag in GLOBAL_ONLY_FLAGS {
            let (adjusted, injected) = inject_default_verb(argv(&[flag]));
            assert_eq!(adjusted, vec!["prim".to_string(), flag.to_string()]);
            assert!(!injected, "flag: {flag}");
        }
    }

    #[test]
    fn a_file_literally_named_fmt_is_shadowed_by_the_verb() {
        // Documented edge case (AD-0007 §1): `prim fmt` disambiguates with
        // `prim fmt fmt`.
        let (adjusted, injected) = inject_default_verb(argv(&["fmt"]));
        assert_eq!(adjusted, vec!["prim", "fmt"]);
        assert!(!injected);
    }

    #[test]
    fn deprecated_flag_reports_the_one_flag_in_use() {
        let mut args = FmtArgs {
            paths: vec![],
            check: false,
            diff: false,
            stdin_filepath: None,
        };
        assert_eq!(deprecated_flag(&args), None);

        args.check = true;
        assert_eq!(deprecated_flag(&args), Some("--check"));

        args.check = false;
        args.diff = true;
        assert_eq!(deprecated_flag(&args), Some("--diff"));

        args.diff = false;
        args.stdin_filepath = Some("a.md".into());
        assert_eq!(deprecated_flag(&args), Some("--stdin-filepath"));
    }
}
