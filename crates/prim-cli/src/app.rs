//! Operating-mode dispatch over the prim formatting engine.

use std::io::Read;
use std::path::Path;

use crate::cli::Cli;
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
fn run_stdin(_path: &Path) -> i32 {
    let mut input = String::new();
    if std::io::stdin().read_to_string(&mut input).is_err() {
        ui::error("could not read stdin as UTF-8");
        return EXIT_ERROR;
    }
    print!("{}", prim_fmt::format(&input));
    EXIT_OK
}

/// Format each explicit path argument according to the selected mode.
fn run_paths(cli: &Cli) -> i32 {
    let mut had_error = false;
    let mut any_would_change = false;

    for path in &cli.paths {
        let original = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) => {
                ui::error(&format!("{}: {err}", path.display()));
                had_error = true;
                continue;
            }
        };

        let formatted = prim_fmt::format(&original);
        if formatted == original {
            continue;
        }
        any_would_change = true;

        if cli.check {
            ui::would_reformat(path);
        } else if cli.diff {
            // Unified-diff rendering arrives with real formatting; the
            // scaffold no-op never produces a pending change to show.
        } else if let Err(err) = std::fs::write(path, &formatted) {
            // Atomic write (FR-6.4) is a follow-up milestone; this branch is
            // unreached until structured formatting yields a change.
            ui::error(&format!("{}: {err}", path.display()));
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
