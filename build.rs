use clap::{CommandFactory, ValueEnum};
use clap_complete::{generate_to, shells::Shell};
use std::env;
use std::io::Error;

mod cli {
    include!("src/cli.rs");
}

fn main() -> Result<(), Error> {
    let Some(outdir) = env::var_os("OUT_DIR") else {
        return Ok(());
    };

    let mut cmd = cli::Cli::command();

    for &shell in Shell::value_variants() {
        let path = generate_to(shell, &mut cmd, env!("CARGO_PKG_NAME"), &outdir)?;
        println!("cargo:warning={shell} completion file is generated: {path:?}");
    }

    Ok(())
}
