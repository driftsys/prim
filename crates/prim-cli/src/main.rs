use std::io::IsTerminal;

use clap::{CommandFactory, Parser};

mod app;
mod argv;
mod changed_files;
mod cli;
mod diff;
mod discover;
mod editorconfig;
mod init;
mod report;
mod ui;
mod write;

use cli::{Cli, Verb};

fn main() {
    let (args, verb_injected) = argv::inject_default_verb(std::env::args().collect());
    let cli = Cli::parse_from(args);

    // Colour policy (clig.dev): --color wins; auto honors NO_COLOR and keys
    // off stderr, where all human-readable output goes.
    let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    if ui::resolve_color(cli.color, std::io::stderr().is_terminal(), no_color) {
        yansi::enable();
    } else {
        yansi::disable();
    }

    // Handle --completions before any file processing so it works standalone.
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "prim", &mut std::io::stdout());
        return;
    }

    // AD-0007 §3: the top-level `--check`/`--diff`/`--stdin-filepath` spelling
    // is deprecated sugar for `prim fmt ...`. Warn once, on stderr only, so
    // the stdout machine channel (and CI gates reading it) stay untouched.
    if verb_injected
        && let Verb::Fmt(fmt_args) = &cli.verb
        && let Some(flag) = argv::deprecated_flag(fmt_args)
    {
        ui::warning(&format!(
            "'prim {flag}' is deprecated; use 'prim fmt {flag}' (removed in v2.0)"
        ));
    }

    std::process::exit(app::run(&cli));
}
