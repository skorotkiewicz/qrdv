use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;

use crate::cli::EncodeArgs;
use crate::crypto;
use crate::protocol::{DataFrame, FLAG_COMPRESSED, FLAG_ENCRYPTED, Header};
use crate::qr;
use crate::video;

pub fn run(args: EncodeArgs) -> Result<()> {
    // Check ffmpeg availability
    video::check_ffmpeg()?;

    let ec_level = args.ec_level.to_qrcode_ec();
    let (width, height) = args.resolution.dimensions();

    // Read input file
    println!("  {} {}", "reading:".dimmed(), args.input.display());
    let raw_data = fs::read(&args.input)
        .with_context(|| format!("Failed to read input file: {}", args.input.display()))?;
    let original_size = raw_data.len() as u64;

    println!(
        "  {} {} ({:.2} MB)",
        "size:".dimmed(),
        format!("{} bytes", original_size).white(),
        original_size as f64 / 1_048_576.0
    );

    // CRC32 of original data
    let checksum = crc32fast::hash(&raw_data);

    // Compress the data
    println!("  {} {}", "compress:".dimmed(), "deflate...".cyan());
    let compressed = compress_data(&raw_data)?;
    let compressed_size = compressed.len();
    let compression_ratio = (1.0 - compressed_size as f64 / raw_data.len() as f64) * 100.0;
    println!(
        "  {} {} ({:.1}% reduction)",
        "compressed:".dimmed(),
        format!("{} bytes", compressed_size).white(),
        compression_ratio
    );

    let mut flags: u8 = FLAG_COMPRESSED;
    let mut salt = None;
    let mut nonce = None;

    // Encrypt if key provided
    let payload = if let Some(ref key) = args.key {
        println!("  {} {}", "encrypt:".dimmed(), "aes-256-gcm".cyan());
        let s = crypto::generate_salt();
        let n = crypto::generate_nonce();
        let encrypted = crypto::encrypt(&compressed, key, &s, &n)?;
        salt = Some(s);
        nonce = Some(n);
        flags |= FLAG_ENCRYPTED;
        println!(
            "  {} {}",
            "encrypted:".dimmed(),
            format!("{} bytes", encrypted.len()).white()
        );
        encrypted
    } else {
        compressed
    };

    // Get filename from input path
    let filename = args
        .input
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Dynamically find the actual QR capacity
    println!(
        "  {} {}",
        "calibrate:".dimmed(),
        "testing QR capacity...".cyan()
    );
    let max_capacity = qr::find_max_capacity(ec_level);
    let data_frame_overhead = 8; // frame_index(4) + chunk_crc(4)
    let chunk_size = max_capacity - data_frame_overhead;

    // Calculate number of data frames needed
    let num_data_frames = (payload.len() + chunk_size - 1) / chunk_size;
    let total_frames = num_data_frames as u32 + 1; // +1 for header frame

    println!();
    println!(
        "  {} {}x{}",
        "resolution:".dimmed(),
        width.to_string().white(),
        height.to_string().white()
    );
    println!(
        "  {} {}",
        "ec level:".dimmed(),
        format!("{:?}", args.ec_level).cyan()
    );
    println!(
        "  {} {}",
        "qr capacity:".dimmed(),
        format!("{} bytes", max_capacity).white()
    );
    println!(
        "  {} {}",
        "chunk size:".dimmed(),
        format!("{} bytes", chunk_size).white()
    );
    println!(
        "  {} {} ({} header + {} data)",
        "total frames:".dimmed(),
        total_frames.to_string().yellow(),
        "1".white(),
        num_data_frames.to_string().white()
    );
    println!("  {} {}", "fps:".dimmed(), args.fps.to_string().white());
    println!(
        "  {} {}",
        "duration:".dimmed(),
        format!("{:.1}s", total_frames as f64 / args.fps as f64).yellow()
    );

    // Generate header frame
    let is_parallel = args.mode == crate::cli::ProcessingMode::Parallel;
    let mode_label = if is_parallel { "parallel" } else { "standard" };

    println!();
    println!(
        "  {} {}",
        "generate:".dimmed(),
        format!("QR code frames ({})...", mode_label).cyan()
    );

    let header = Header {
        flags,
        total_frames,
        original_size,
        checksum,
        filename,
        salt,
        nonce,
    };

    let header_data = header.serialize()?;
    let header_img = qr::generate_qr_image(&header_data, ec_level, width, height)
        .context("Failed to generate header QR code")?;

    // Generate data frames
    let pb = ProgressBar::new(num_data_frames as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "  [{bar:40.cyan/blue}] {pos}/{len} frames ({eta} remaining)",
        )?
        .progress_chars("━╸ "),
    );

    let generate_frame = |i: usize| -> Result<image::GrayImage> {
        let start = i * chunk_size;
        let end = ((i + 1) * chunk_size).min(payload.len());
        let chunk = &payload[start..end];

        let chunk_crc = crc32fast::hash(chunk);

        let data_frame = DataFrame {
            frame_index: (i + 1) as u32,
            chunk_crc,
            data: chunk.to_vec(),
        };

        let frame_data = data_frame.serialize()?;
        let frame_img = qr::generate_qr_image(&frame_data, ec_level, width, height)
            .with_context(|| format!("Failed to generate QR code for frame {}", i + 1))?;

        pb.inc(1);
        Ok(frame_img)
    };

    let data_frames: Result<Vec<_>> = if is_parallel {
        (0..num_data_frames).into_par_iter().map(generate_frame).collect()
    } else {
        (0..num_data_frames).map(generate_frame).collect()
    };

    let data_frames = data_frames?;
    pb.finish_and_clear();

    // Combine header + data frames in order
    let mut frames = Vec::with_capacity(total_frames as usize);
    frames.push(header_img);
    frames.extend(data_frames);

    println!(
        "  {} {}",
        "frames:".dimmed(),
        format!("{} generated", total_frames).green()
    );

    // Pipe frames directly to ffmpeg (no disk I/O)
    println!(
        "  {} {}",
        "encode:".dimmed(),
        "piping to ffmpeg...".cyan()
    );
    video::encode_frames_to_video(&frames, &args.output, width, height, args.fps)?;

    // Report output file size
    let output_size = fs::metadata(&args.output)
        .with_context(|| format!("Failed to stat output: {}", args.output.display()))?
        .len();

    println!();
    println!("  {}", "done!".green().bold());
    println!(
        "  {} {}",
        "output:".dimmed(),
        args.output.display().to_string().white()
    );
    println!(
        "  {} {} ({:.2} MB)",
        "video size:".dimmed(),
        format!("{} bytes", output_size).white(),
        output_size as f64 / 1_048_576.0
    );
    println!(
        "  {} {}",
        "overhead:".dimmed(),
        format!("{:.1}x", output_size as f64 / original_size as f64).yellow()
    );

    Ok(())
}

fn compress_data(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use std::io::Write;

    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}
