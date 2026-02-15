mod cli;
mod crypto;
mod encode;
mod decode;
mod qr;
mod video;
mod protocol;

use anyhow::Result;
use cli::{Cli, Commands};
use clap::Parser;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Encode(args) => encode::run(args)?,
        Commands::Decode(args) => decode::run(args)?,
    }

    Ok(())
}
