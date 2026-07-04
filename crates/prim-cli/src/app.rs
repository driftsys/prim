//! Operating-mode dispatch over the prim formatting engine.

use std::io::Read;
use std::path::Path;

use crate::cli::Cli;
use crate::diff;
use crate::discover;
use crate::editorconfig;
use crate::ui;
use crate::write;

/// Operating-mode exit codes (FR-5.5).
const EXIT_OK: i32 = 0;
const EXIT_CHANGES: i32 = 1;
const EXIT_ERROR: i32 = 2;

/// Process the parsed CLI and return the process exit code.
pub fn run(cli: &Cli) -> i32 {
    if let Some(path) = cli.stdin_filepath.as_deref() {
        return run_stdin(path);
    }
    run_paths(cli)
}

/// Read stdin, format it, and write the result to stdout (format-on-save).
///
/// The path selects the formatter; if prim does not own that file type, the
/// input is passed through unchanged.
fn run_stdin(path: &Path) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    match prim_fmt::classify(path) {
        Some(kind) => {
            let style = editorconfig::resolve(path);
            match prim_fmt::format(kind, &input, &style) {
                Ok(text) => print!("{text}"),
                Err(err) => {
                    // Preserve the editor buffer on a parse failure: echo the
                    // original to stdout and report on stderr (FR-6.3).
                    ui::error(&format!("{}: {err}", path.display()));
                    print!("{input}");
                    return EXIT_ERROR;
                }
            }
        }
        None => print!("{input}"),
    }
    EXIT_OK
}

/// Discover the target files and format each according to the selected mode.
fn run_paths(cli: &Cli) -> i32 {
    let mut had_error = false;
    let mut any_would_change = false;
    // Caches each directory's `.editorconfig` cascade so a repository parses
    // every config once, not once per file.
    let mut resolver = editorconfig::Resolver::new();

    let files = match discover::collect(&cli.paths, &cli.exclude) {
        Ok(files) => files,
        Err(err) => {
            ui::error(&format!("--exclude: {err}"));
            return EXIT_ERROR;
        }
    };

    for file in files {
        let Some(kind) = prim_fmt::classify(&file.path) else {
            // A file prim does not own is left byte-for-byte unchanged
            // (FR-2.4). Walked files are skipped silently; a named path is
            // answered — a missing one is an error, an unowned one a warning.
            if file.explicit {
                if file.path.exists() {
                    ui::warning(&format!(
                        "{}: not a file type prim formats; skipped",
                        file.path.display()
                    ));
                } else {
                    ui::error(&format!("{}: no such file", file.path.display()));
                    had_error = true;
                }
            }
            continue;
        };

        let original = match std::fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(err) => {
                // An owned file that can't be read as UTF-8 is left unchanged
                // and reported (FR-6.5): an error for an explicitly named file
                // (exit 2), a warning for a discovered one.
                let message = format!("{}: {err}", file.path.display());
                if file.explicit {
                    ui::error(&message);
                    had_error = true;
                } else {
                    ui::warning(&message);
                }
                continue;
            }
        };

        let style = resolver.resolve(&file.path);
        let formatted = match prim_fmt::format(kind, &original, &style) {
            Ok(text) => text,
            Err(err) => {
                // An owned file prim cannot parse is left unchanged and reported
                // (FR-6.3): an error for an explicitly named file (exit 2), a
                // warning for a discovered one.
                let message = format!("{}: {err}", file.path.display());
                if file.explicit {
                    ui::error(&message);
                    had_error = true;
                } else {
                    ui::warning(&message);
                }
                continue;
            }
        };
        if formatted == original {
            continue;
        }
        any_would_change = true;

        if cli.check {
            ui::would_reformat(&file.path);
        } else if cli.diff {
            // Print a unified diff of the pending change; write nothing (FR-5.3).
            print!("{}", diff::unified(&file.path, &original, &formatted));
        } else if let Err(err) = write::atomic(&file.path, &formatted) {
            // Atomic write (FR-6.4): on failure the original is left intact.
            ui::error(&format!("{}: {err}", file.path.display()));
            had_error = true;
        }
    }

    if had_error {
        EXIT_ERROR
    } else if cli.check && any_would_change {
        EXIT_CHANGES
    } else {
        EXIT_OK
    }
}
