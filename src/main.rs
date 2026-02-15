mod cli;
mod crypto;
mod decode;
mod encode;
mod protocol;
mod qr;
mod video;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode(args) => encode::run(args)?,
        Commands::Decode(args) => decode::run(args)?,
    }

    Ok(())
}
