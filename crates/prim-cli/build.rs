use std::fs;
use std::path::PathBuf;

use clap::CommandFactory;

#[path = "src/cli.rs"]
mod cli;

fn main() -> std::io::Result<()> {
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let man_dir = out_dir.join("man");
    fs::create_dir_all(&man_dir)?;

    // Generate the man page: prim(1).
    let cmd = cli::Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;
    fs::write(man_dir.join("prim.1"), buf)?;

    println!("cargo:rerun-if-changed=src/cli.rs");
    Ok(())
}
