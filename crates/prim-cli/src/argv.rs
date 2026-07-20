//! The bare-`prim`-is-`fmt` argv preprocessor (AD-0007 §1).
//!
//! clap cannot cleanly disambiguate an optional subcommand from a leading
//! positional `PATH`, so dispatch is resolved before [`clap::Parser::parse`]:
//! the argv is scanned, skipping over recognized global flags (and their
//! values), for the first token that is either a known verb (`fmt`/`lint`/
//! `fix`/`init`) or a global help/version flag — either way the argv is left
//! as-is.
//! Anything else (a bare path, or an unrecognized flag) means no verb was
//! given, so an implicit `fmt` is inserted right after the program name.
//! This keeps `prim README.md`, `prim fmt README.md`,
//! `prim --color=always fmt README.md`, and the deprecated `prim --check`
//! all working, regardless of where global flags like `--color`/`--exclude`/
//! `--completions`/`--no-ignore`/`--since`/`--staged` (declared `global = true`
//! on `Cli`) appear
//! relative to the verb.

use crate::cli::FmtArgs;

const VERBS: &[&str] = &["fmt", "lint", "fix", "init", "explain", "lsp"];
const GLOBAL_ONLY_FLAGS: &[&str] = &["-h", "--help", "-V", "--version"];
const GLOBAL_BOOL_FLAGS: &[&str] = &["--no-ignore", "--staged"];
/// Global flags that consume a value — either as a separate following token
/// (`--color always`) or attached with `=` (`--color=always`) — and so must
/// be skipped over, value and all, while scanning for a verb.
const GLOBAL_VALUE_FLAGS: &[&str] = &["--exclude", "--color", "--completions", "--since"];

/// Insert an implicit `fmt` verb into `args` (a full argv, including the
/// program name at index 0) when the caller did not name a verb. Returns the
/// (possibly adjusted) argv and whether `fmt` was injected.
pub fn inject_default_verb(args: Vec<String>) -> (Vec<String>, bool) {
    let mut index = 1;
    let mut leave_unchanged = false;

    while index < args.len() {
        let token = args[index].as_str();

        if VERBS.contains(&token) || GLOBAL_ONLY_FLAGS.contains(&token) {
            leave_unchanged = true;
            break;
        }

        if GLOBAL_BOOL_FLAGS.contains(&token) {
            index += 1;
            continue;
        }

        let flag_name = token.split('=').next().unwrap_or(token);
        if GLOBAL_VALUE_FLAGS.contains(&flag_name) {
            // `--flag=value` is one token; `--flag value` is two.
            index += if token.contains('=') { 1 } else { 2 };
            continue;
        }

        // The first token that isn't a recognized global flag: either a bare
        // path, an unrecognized flag, or (per clap's `--` convention) the
        // start of the positional tail. No verb precedes it.
        break;
    }

    if leave_unchanged {
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
    if args.write.check {
        Some("--check")
    } else if args.write.diff {
        Some("--diff")
    } else if args.write.stdin_filepath.is_some() {
        Some("--stdin-filepath")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::WriteArgs;

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
    fn global_value_flag_before_an_explicit_verb_does_not_shadow_it() {
        // Regression: `--color`/`--exclude`/`--completions` are `global =
        // true` (valid before or after the verb); the preprocessor must scan
        // past them, both in `--flag value` and `--flag=value` form, to find
        // the real verb rather than injecting a second, spurious `fmt`.
        let cases: &[&[&str]] = &[
            &["--color", "always", "fmt", "doc.txt"],
            &["--color=always", "fmt", "doc.txt"],
            &["--color", "always", "lint", "doc.txt"],
            &["--exclude", "*.md", "fmt", "doc.txt"],
            &["--exclude", "*.md", "--exclude", "*.json", "fmt", "doc.txt"],
            &["--completions", "bash", "fmt"],
        ];
        for rest in cases {
            let (adjusted, injected) = inject_default_verb(argv(rest));
            let expected: Vec<String> = std::iter::once("prim".to_string())
                .chain(rest.iter().map(|s| s.to_string()))
                .collect();
            assert_eq!(adjusted, expected, "rest: {rest:?}");
            assert!(!injected, "rest: {rest:?}");
        }
    }

    #[test]
    fn global_boolean_flag_before_an_explicit_verb_does_not_shadow_it() {
        let (adjusted, injected) = inject_default_verb(argv(&["--no-ignore", "lint", "doc.txt"]));
        assert_eq!(adjusted, argv(&["--no-ignore", "lint", "doc.txt"]));
        assert!(!injected);
    }

    #[test]
    fn staged_before_an_explicit_verb_does_not_shadow_it() {
        let (adjusted, injected) = inject_default_verb(argv(&["--staged", "lint", "doc.txt"]));
        assert_eq!(adjusted, argv(&["--staged", "lint", "doc.txt"]));
        assert!(!injected);
    }

    #[test]
    fn global_boolean_flag_without_a_verb_still_injects_fmt() {
        let (adjusted, injected) = inject_default_verb(argv(&["--no-ignore", "doc.txt"]));
        assert_eq!(adjusted, argv(&["fmt", "--no-ignore", "doc.txt"]));
        assert!(injected);
    }

    #[test]
    fn staged_without_a_verb_still_injects_fmt() {
        let (adjusted, injected) = inject_default_verb(argv(&["--staged", "doc.txt"]));
        assert_eq!(adjusted, argv(&["fmt", "--staged", "doc.txt"]));
        assert!(injected);
    }

    #[test]
    fn global_value_flag_alone_still_injects_fmt() {
        // No verb anywhere in argv: `fmt` must still be injected even though
        // scanning has to step over a global value flag first.
        let (adjusted, injected) = inject_default_verb(argv(&["--color", "always", "doc.txt"]));
        assert_eq!(
            adjusted,
            vec!["prim", "fmt", "--color", "always", "doc.txt"]
        );
        assert!(injected);
    }

    #[test]
    fn since_before_an_explicit_verb_does_not_shadow_it() {
        for rest in [
            ["--since", "main", "fmt", "--check"].as_slice(),
            ["--since=main", "lint", "doc.txt"].as_slice(),
        ] {
            let (adjusted, injected) = inject_default_verb(argv(rest));
            assert_eq!(adjusted, argv(rest), "rest: {rest:?}");
            assert!(!injected, "rest: {rest:?}");
        }
    }

    #[test]
    fn since_without_a_verb_still_injects_fmt() {
        let (adjusted, injected) = inject_default_verb(argv(&["--since", "main", "doc.txt"]));
        assert_eq!(adjusted, argv(&["fmt", "--since", "main", "doc.txt"]));
        assert!(injected);
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
            write: WriteArgs {
                paths: vec![],
                check: false,
                diff: false,
                stdin_filepath: None,
            },
            check_idempotence: false,
            format: None,
        };
        assert_eq!(deprecated_flag(&args), None);

        args.write.check = true;
        assert_eq!(deprecated_flag(&args), Some("--check"));

        args.write.check = false;
        args.write.diff = true;
        assert_eq!(deprecated_flag(&args), Some("--diff"));

        args.write.diff = false;
        args.write.stdin_filepath = Some("a.md".into());
        assert_eq!(deprecated_flag(&args), Some("--stdin-filepath"));
    }
}
