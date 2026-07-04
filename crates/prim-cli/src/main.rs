use std::io::IsTerminal;

use clap::{CommandFactory, Parser};

mod app;
mod cli;
mod diff;
mod discover;
mod editorconfig;
mod ui;
mod write;

use cli::Cli;

fn main() {
    let cli = Cli::parse();

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

    std::process::exit(app::run(&cli));
}
