//! Operating-mode dispatch over the prim formatting engine.

use std::io::Read;
use std::path::Path;

use crate::cli::Cli;
use crate::discover;
use crate::ui;

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
        Some(kind) => print!("{}", prim_fmt::format(kind, &input)),
        None => print!("{input}"),
    }
    EXIT_OK
}

/// Discover the target files and format each according to the selected mode.
fn run_paths(cli: &Cli) -> i32 {
    let mut had_error = false;
    let mut any_would_change = false;

    for file in discover::collect(&cli.paths, &cli.exclude) {
        let Some(kind) = prim_fmt::classify(&file.path) else {
            // A file prim does not own — left byte-for-byte unchanged (FR-2.4),
            // even when named explicitly.
            continue;
        };

        let original = match std::fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(err) => {
                // An explicitly named owned file that can't be read is an error
                // (FR-6 fail-safe); a walked file that isn't UTF-8 is skipped.
                if file.explicit {
                    ui::error(&format!("{}: {err}", file.path.display()));
                    had_error = true;
                }
                continue;
            }
        };

        let formatted = prim_fmt::format(kind, &original);
        if formatted == original {
            continue;
        }
        any_would_change = true;

        if cli.check {
            ui::would_reformat(&file.path);
        } else if cli.diff {
            // Unified-diff rendering arrives with real formatting; the
            // scaffold no-op never produces a pending change to show.
        } else if let Err(err) = std::fs::write(&file.path, &formatted) {
            // Atomic write (FR-6.4) is a follow-up milestone; this branch is
            // unreached until structured formatting yields a change.
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
