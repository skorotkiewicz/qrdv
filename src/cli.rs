use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "qrdv",
    version,
    about = "Encode data into QR code videos with optional encryption"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Encode a file into a QR code video
    Encode(EncodeArgs),
    /// Decode a QR code video back into a file
    Decode(DecodeArgs),
}

#[derive(Parser, Clone)]
pub struct EncodeArgs {
    /// Input file to encode
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output MP4 file
    #[arg(short, long)]
    pub output: PathBuf,

    /// Video resolution
    #[arg(short, long, default_value = "720p")]
    pub resolution: Resolution,

    /// Encryption key (optional)
    #[arg(short, long)]
    pub key: Option<String>,

    /// Error correction level (higher = more compression resilient, less data per frame)
    #[arg(short, long, default_value = "high")]
    pub ec_level: EcLevel,

    /// Frames per second (lower = smaller file, but more frames needed)
    #[arg(long, default_value = "2")]
    pub fps: u32,

    /// Processing mode
    #[arg(short, long, default_value = "parallel")]
    pub mode: ProcessingMode,
}

#[derive(Parser, Clone)]
pub struct DecodeArgs {
    /// Input MP4 file to decode
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output file
    #[arg(short, long)]
    pub output: PathBuf,

    /// Decryption key (must match encoding key)
    #[arg(short, long)]
    pub key: Option<String>,

    /// Processing mode
    #[arg(short, long, default_value = "parallel")]
    pub mode: ProcessingMode,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum Resolution {
    /// 480p (854x480)
    #[value(name = "480p")]
    R480p,
    /// 720p (1280x720)
    #[value(name = "720p")]
    R720p,
    /// 1080p (1920x1080)
    #[value(name = "1080p")]
    R1080p,
    /// 1440p (2560x1440)
    #[value(name = "1440p")]
    R1440p,
    /// 4K (3840x2160)
    #[value(name = "4k")]
    R4k,
}

impl Resolution {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            Resolution::R480p => (854, 480),
            Resolution::R720p => (1280, 720),
            Resolution::R1080p => (1920, 1080),
            Resolution::R1440p => (2560, 1440),
            Resolution::R4k => (3840, 2160),
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum EcLevel {
    /// Low (7% recovery) - maximum data per frame
    Low,
    /// Medium (15% recovery)
    Medium,
    /// Quartile (25% recovery)
    Quartile,
    /// High (30% recovery) - best compression resilience
    High,
}

impl EcLevel {
    pub fn to_qrcode_ec(&self) -> qrcode::EcLevel {
        match self {
            EcLevel::Low => qrcode::EcLevel::L,
            EcLevel::Medium => qrcode::EcLevel::M,
            EcLevel::Quartile => qrcode::EcLevel::Q,
            EcLevel::High => qrcode::EcLevel::H,
        }
    }
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq)]
pub enum ProcessingMode {
    /// Sequential frame processing
    Standard,
    /// Parallel frame processing using all CPU cores
    Parallel,
}
