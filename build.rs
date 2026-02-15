use std::env;
use std::fs;
use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::generate_to;
use clap_complete::shells::{Bash, Fish, Zsh};
use clap_mangen::Man;

// Mirror of src/cli.rs for build-time generation
// Keep in sync with src/cli.rs when changing CLI args

#[derive(Parser)]
#[command(
    name = "qrdv",
    version,
    about = "Encode data into QR code videos with optional encryption"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encode a file into a QR code video
    Encode(EncodeArgs),
    /// Decode a QR code video back into a file
    Decode(DecodeArgs),
}

#[derive(Parser, Clone)]
struct EncodeArgs {
    /// Input file to encode
    #[arg(short, long)]
    input: PathBuf,

    /// Output MP4 file
    #[arg(short, long)]
    output: PathBuf,

    /// Video resolution
    #[arg(short, long, default_value = "720p")]
    resolution: Resolution,

    /// Encryption key (optional)
    #[arg(short, long)]
    key: Option<String>,

    /// Error correction level (higher = more compression resilient, less data per frame)
    #[arg(short, long, default_value = "high")]
    ec_level: EcLevel,

    /// Frames per second (lower = smaller file, but more frames needed)
    #[arg(long, default_value = "2")]
    fps: u32,

    /// Processing mode
    #[arg(short, long, default_value = "parallel")]
    mode: ProcessingMode,
}

#[derive(Parser, Clone)]
struct DecodeArgs {
    /// Input MP4 file to decode
    #[arg(short, long)]
    input: PathBuf,

    /// Output file
    #[arg(short, long)]
    output: PathBuf,

    /// Decryption key (must match encoding key)
    #[arg(short, long)]
    key: Option<String>,

    /// Processing mode
    #[arg(short, long, default_value = "parallel")]
    mode: ProcessingMode,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Resolution {
    #[value(name = "480p")]
    R480p,
    #[value(name = "720p")]
    R720p,
    #[value(name = "1080p")]
    R1080p,
    #[value(name = "1440p")]
    R1440p,
    #[value(name = "4k")]
    R4k,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum EcLevel {
    Low,
    Medium,
    Quartile,
    High,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
enum ProcessingMode {
    Standard,
    Parallel,
}

fn main() {
    let outdir = env::var("OUT_DIR").unwrap();
    let outdir = PathBuf::from(&outdir);

    // Also generate into a known directory for easy access
    let completions_dir = PathBuf::from("completions");
    fs::create_dir_all(&completions_dir).unwrap();

    let mut cmd = Cli::command();

    // Generate shell completions
    generate_to(Bash, &mut cmd, "qrdv", &completions_dir).unwrap();
    generate_to(Zsh, &mut cmd, "qrdv", &completions_dir).unwrap();
    generate_to(Fish, &mut cmd, "qrdv", &completions_dir).unwrap();

    // Also copy to OUT_DIR
    generate_to(Bash, &mut cmd, "qrdv", &outdir).unwrap();
    generate_to(Zsh, &mut cmd, "qrdv", &outdir).unwrap();
    generate_to(Fish, &mut cmd, "qrdv", &outdir).unwrap();

    // Generate man page
    let man = Man::new(cmd);
    let mut buffer = Vec::new();
    man.render(&mut buffer).unwrap();

    fs::write(completions_dir.join("qrdv.1"), &buffer).unwrap();
    fs::write(outdir.join("qrdv.1"), &buffer).unwrap();

    // Generate subcommand man pages
    let cmd = Cli::command();
    for subcmd in cmd.get_subcommands() {
        let subcmd_man = Man::new(subcmd.clone());
        let mut buffer = Vec::new();
        subcmd_man.render(&mut buffer).unwrap();

        let name = format!("qrdv-{}.1", subcmd.get_name());
        fs::write(completions_dir.join(&name), &buffer).unwrap();
        fs::write(outdir.join(&name), &buffer).unwrap();
    }

    println!("cargo:rerun-if-changed=src/cli.rs");
    println!("cargo:rerun-if-changed=build.rs");
}
