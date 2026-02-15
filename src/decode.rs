use anyhow::{Result, Context, bail};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;

use crate::cli::DecodeArgs;
use crate::crypto;
use crate::protocol::{Header, DataFrame};
use crate::qr;
use crate::video;

pub fn run(args: DecodeArgs) -> Result<()> {
    // Check ffmpeg availability
    video::check_ffmpeg()?;

    println!("  {} {}", "input:".dimmed(), args.input.display());

    // Create temporary directory for extracted frames
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let frames_dir = temp_dir.path();

    // Extract frames from video
    println!("  {} {}", "extract:".dimmed(), "frames via ffmpeg...".cyan());
    let frame_count = video::decode_video_to_frames(&args.input, frames_dir)?;
    println!("  {} {}", "frames:".dimmed(),
        format!("{} extracted", frame_count).white());

    if frame_count == 0 {
        bail!("No frames found in video");
    }

    // Read header frame (first frame)
    println!("  {} {}", "header:".dimmed(), "reading...".cyan());
    let header_path = frames_dir.join("frame_000001.png");
    let header_img = video::load_frame(&header_path)
        .context("Failed to load header frame")?;
    let header_data = qr::decode_qr_image(&header_img)
        .context("Failed to decode header QR code")?;
    let header = Header::deserialize(&header_data)
        .context("Failed to parse header")?;

    println!("  {} {}", "filename:".dimmed(), header.filename.white());
    println!("  {} {} ({:.2} MB)", "original size:".dimmed(),
        format!("{} bytes", header.original_size).white(),
        header.original_size as f64 / 1_048_576.0);
    println!("  {} {}", "total frames:".dimmed(),
        header.total_frames.to_string().white());
    println!("  {} {}", "encrypted:".dimmed(),
        if header.is_encrypted() { "yes".yellow() } else { "no".dimmed() });
    println!("  {} {}", "compressed:".dimmed(),
        if header.is_compressed() { "yes".cyan() } else { "no".dimmed() });

    // Validate encryption key requirement
    if header.is_encrypted() && args.key.is_none() {
        bail!("This video is encrypted. Please provide the decryption key with --key");
    }

    if !header.is_encrypted() && args.key.is_some() {
        println!("  {} {}", "warning:".yellow(),
            "key provided but video is not encrypted (ignoring)".dimmed());
    }

    // Read data frames
    println!();
    println!("  {} {}", "decode:".dimmed(), "reading data frames...".cyan());
    let num_data_frames = (header.total_frames - 1) as usize;

    let pb = ProgressBar::new(num_data_frames as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "  [{bar:40.cyan/blue}] {pos}/{len} frames ({eta} remaining)"
        )?
        .progress_chars("━╸ "),
    );

    let mut chunks: Vec<Option<Vec<u8>>> = vec![None; num_data_frames];
    let mut errors = Vec::new();

    for i in 0..num_data_frames {
        let frame_path = frames_dir.join(format!("frame_{:06}.png", i + 2));
        match decode_data_frame(&frame_path) {
            Ok(data_frame) => {
                let idx = data_frame.frame_index as usize;
                if idx == 0 || idx > num_data_frames {
                    errors.push(format!("Frame {} has invalid index {}", i + 2, idx));
                } else {
                    // Verify chunk CRC
                    let computed_crc = crc32fast::hash(&data_frame.data);
                    if computed_crc != data_frame.chunk_crc {
                        errors.push(format!(
                            "Frame {} (index {}) CRC mismatch: expected {:08x}, got {:08x}",
                            i + 2, idx, data_frame.chunk_crc, computed_crc
                        ));
                    } else {
                        chunks[idx - 1] = Some(data_frame.data);
                    }
                }
            }
            Err(e) => {
                errors.push(format!("Frame {}: {}", i + 2, e));
            }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();

    // Report errors
    if !errors.is_empty() {
        println!("  {} {}", "errors:".red(),
            format!("{} frame(s) failed", errors.len()).red());
        for (i, err) in errors.iter().enumerate().take(10) {
            println!("    {} {}", format!("{}.", i + 1).dimmed(), err.dimmed());
        }
        if errors.len() > 10 {
            println!("    {} {}", "...".dimmed(),
                format!("and {} more", errors.len() - 10).dimmed());
        }
    } else {
        println!("  {} {}", "frames:".dimmed(),
            format!("{} decoded", num_data_frames).green());
    }

    // Check for missing frames
    let missing: Vec<usize> = chunks.iter().enumerate()
        .filter(|(_, c)| c.is_none())
        .map(|(i, _)| i + 1)
        .collect();

    if !missing.is_empty() {
        bail!(
            "Missing {} data frames: {:?}. Video may be too corrupted to decode.",
            missing.len(),
            &missing[..missing.len().min(20)]
        );
    }

    // Reassemble payload
    println!("  {} {}", "reassemble:".dimmed(), "joining chunks...".cyan());
    let mut payload = Vec::new();
    for chunk in chunks {
        payload.extend(chunk.unwrap());
    }
    println!("  {} {}", "payload:".dimmed(),
        format!("{} bytes", payload.len()).white());

    // Decrypt if needed
    let decrypted = if header.is_encrypted() {
        println!("  {} {}", "decrypt:".dimmed(), "aes-256-gcm...".cyan());
        let salt = header.salt.as_ref().unwrap();
        let nonce = header.nonce.as_ref().unwrap();
        let key = args.key.as_ref().unwrap();
        crypto::decrypt(&payload, key, salt, nonce)
            .context("Decryption failed - wrong key or corrupted data")?
    } else {
        payload
    };

    // Decompress if needed
    let original = if header.is_compressed() {
        println!("  {} {}", "decompress:".dimmed(), "inflate...".cyan());
        decompress_data(&decrypted)?
    } else {
        decrypted
    };

    // Verify checksum
    let computed_checksum = crc32fast::hash(&original);
    if computed_checksum != header.checksum {
        bail!(
            "Data integrity check failed! CRC32 mismatch: expected {:08x}, got {:08x}",
            header.checksum, computed_checksum
        );
    }

    // Verify size
    if original.len() as u64 != header.original_size {
        bail!(
            "Size mismatch: expected {} bytes, got {} bytes",
            header.original_size, original.len()
        );
    }

    // Write output file
    fs::write(&args.output, &original)
        .with_context(|| format!("Failed to write output file: {}", args.output.display()))?;

    println!();
    println!("  {}", "done!".green().bold());
    println!("  {} {}", "output:".dimmed(), args.output.display().to_string().white());
    println!("  {} {} ({:.2} MB)", "size:".dimmed(),
        format!("{} bytes", original.len()).white(),
        original.len() as f64 / 1_048_576.0);
    println!("  {} {} {}", "checksum:".dimmed(),
        format!("{:08x}", header.checksum).white(), "verified".green());

    Ok(())
}

fn decode_data_frame(frame_path: &std::path::Path) -> Result<DataFrame> {
    let img = video::load_frame(frame_path)?;
    let data = qr::decode_qr_image(&img)?;
    DataFrame::deserialize(&data)
}

fn decompress_data(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::DeflateDecoder;
    use std::io::Read;

    let mut decoder = DeflateDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)
        .context("Decompression failed - data may be corrupted")?;
    Ok(decompressed)
}
