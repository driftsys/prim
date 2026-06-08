use std::io::IsTerminal;

use clap::{CommandFactory, Parser};

mod app;
mod cli;
mod discover;
mod editorconfig;
mod ui;
mod write;

use cli::{Cli, ColorWhen};

fn main() {
    let cli = Cli::parse();

    // Configure yansi colour output based on the --color flag.
    match cli.color {
        ColorWhen::Always => yansi::enable(),
        ColorWhen::Never => yansi::disable(),
        ColorWhen::Auto => {
            if !std::io::stdout().is_terminal() {
                yansi::disable();
            }
        }
    }

    // Handle --completions before any file processing so it works standalone.
    if let Some(shell) = cli.completions {
        let mut cmd = Cli::command();
        clap_complete::generate(shell, &mut cmd, "prim", &mut std::io::stdout());
        return;
    }

    std::process::exit(app::run(&cli));
}
