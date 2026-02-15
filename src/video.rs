use anyhow::{Context, Result, bail};
use image::GrayImage;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// Check that ffmpeg is available
pub fn check_ffmpeg() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("ffmpeg not found. Please install ffmpeg.")?;

    if !output.status.success() {
        bail!("ffmpeg is not working properly");
    }

    Ok(())
}

/// Encode a sequence of grayscale images into an MP4 video file by piping
/// raw pixel data directly to ffmpeg stdin. No temporary files needed.
///
/// Uses ffmpeg with settings optimized for:
/// - Maximum QR code readability after compression
/// - Minimal file size
/// - Compatibility with common video platforms
pub fn encode_frames_to_video(
    frames: &[GrayImage],
    output_path: &Path,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<()> {
    let size_str = format!("{}x{}", width, height);

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "gray",
            "-video_size",
            &size_str,
            "-framerate",
            &fps.to_string(),
            "-i",
            "pipe:0", // read raw frames from stdin
            "-c:v",
            "libx264",
            "-crf",
            "18",
            "-preset",
            "slow",
            "-pix_fmt",
            "yuv420p",
            "-tune",
            "stillimage",
            "-profile:v",
            "high",
            "-level",
            "4.1",
            "-movflags",
            "+faststart",
            output_path.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn ffmpeg for encoding")?;

    {
        let stdin = child.stdin.as_mut().context("Failed to open ffmpeg stdin")?;
        for frame in frames {
            // Write raw grayscale pixels directly
            stdin
                .write_all(frame.as_raw())
                .context("Failed to write frame to ffmpeg")?;
        }
    } // stdin is dropped here, closing the pipe

    let output = child.wait_with_output().context("Failed to wait for ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg encoding failed:\n{}", stderr);
    }

    Ok(())
}

/// Extract all frames from an MP4 video as in-memory grayscale images by
/// piping ffmpeg output directly. No temporary files needed.
pub fn decode_video_to_frames(input_path: &Path) -> Result<Vec<GrayImage>> {
    // First probe for resolution and frame count
    let probe_output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,nb_frames",
            "-of",
            "csv=p=0",
            input_path.to_str().unwrap(),
        ])
        .output()
        .context("Failed to run ffprobe")?;

    let probe_str = String::from_utf8_lossy(&probe_output.stdout);
    let parts: Vec<&str> = probe_str.trim().split(',').collect();
    if parts.len() < 3 {
        bail!(
            "Failed to parse video info from ffprobe: '{}'",
            probe_str.trim()
        );
    }

    let width: u32 = parts[0].parse().context("Failed to parse width")?;
    let height: u32 = parts[1].parse().context("Failed to parse height")?;
    let frame_count: usize = parts[2].parse().unwrap_or(0);

    let frame_size = (width * height) as usize;

    // Pipe ffmpeg raw grayscale output
    let mut child = Command::new("ffmpeg")
        .args([
            "-i",
            input_path.to_str().unwrap(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "gray",
            "-v",
            "error",
            "pipe:1", // write raw frames to stdout
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn ffmpeg for decoding")?;

    let stdout = child
        .stdout
        .take()
        .context("Failed to open ffmpeg stdout")?;

    let mut reader = std::io::BufReader::new(stdout);
    let mut frames = if frame_count > 0 {
        Vec::with_capacity(frame_count)
    } else {
        Vec::new()
    };

    loop {
        let mut buf = vec![0u8; frame_size];
        match reader.read_exact(&mut buf) {
            Ok(()) => {
                let img = GrayImage::from_raw(width, height, buf)
                    .context("Failed to create image from raw pixels")?;
                frames.push(img);
            }
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e).context("Failed to read frame from ffmpeg"),
        }
    }

    let output = child.wait_with_output().context("Failed to wait for ffmpeg")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            bail!("ffmpeg decoding failed:\n{}", stderr);
        }
    }

    Ok(frames)
}
