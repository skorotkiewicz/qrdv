use anyhow::{Context, Result, bail};
use image::GrayImage;
use std::path::Path;
use std::process::Command;

/// Check that ffmpeg is available
pub fn check_ffmpeg() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-version")
        .output()
        .context("ffmpeg not found. Please install ffmpeg.")?;

    if !output.status.success() {
        bail!("ffmpeg is not working properly");
    }

    Ok(())
}

/// Encode a sequence of grayscale images into an MP4 video file.
///
/// Uses ffmpeg with settings optimized for:
/// - Maximum QR code readability after compression
/// - Minimal file size
/// - Compatibility with common video platforms
pub fn encode_frames_to_video(
    frames_dir: &Path,
    output_path: &Path,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<()> {
    let input_pattern = frames_dir.join("frame_%06d.png");

    // Use ffmpeg to encode frames to MP4
    // Settings chosen for QR code readability after compression:
    // - High quality CRF (Constant Rate Factor) of 18 for sharp edges
    // - yuv420p pixel format for maximum compatibility
    // - Baseline profile for wide compatibility
    // - High bitrate to preserve QR code sharpness
    // - Nearest-neighbor scaling would be ideal but ffmpeg default is fine for clean input
    let output = Command::new("ffmpeg")
        .args([
            "-y", // overwrite output
            "-framerate",
            &fps.to_string(),
            "-i",
            input_pattern.to_str().unwrap(),
            "-c:v",
            "libx264",
            "-crf",
            "18", // high quality
            "-preset",
            "slow", // better compression
            "-pix_fmt",
            "yuv420p", // compatibility
            "-vf",
            &format!("scale={}:{}:flags=neighbor", width, height), // nearest-neighbor scaling
            "-tune",
            "stillimage", // optimize for static content
            "-profile:v",
            "high",
            "-level",
            "4.1",
            "-movflags",
            "+faststart", // streaming-friendly
            output_path.to_str().unwrap(),
        ])
        .output()
        .context("Failed to run ffmpeg for encoding")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg encoding failed:\n{}", stderr);
    }

    Ok(())
}

/// Extract frames from an MP4 video file as grayscale PNG images.
pub fn decode_video_to_frames(input_path: &Path, frames_dir: &Path) -> Result<u32> {
    // First, get the total number of frames
    let probe_output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "csv=p=0",
            input_path.to_str().unwrap(),
        ])
        .output()
        .context("Failed to run ffprobe")?;

    let frame_count_str = String::from_utf8_lossy(&probe_output.stdout);
    let frame_count: u32 = frame_count_str
        .trim()
        .parse()
        .context("Failed to parse frame count from ffprobe")?;

    // Extract frames as grayscale PNG
    let output_pattern = frames_dir.join("frame_%06d.png");
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input_path.to_str().unwrap(),
            "-vf",
            "format=gray", // convert to grayscale
            "-vsync",
            "0", // preserve frame timing
            output_pattern.to_str().unwrap(),
        ])
        .output()
        .context("Failed to run ffmpeg for frame extraction")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ffmpeg frame extraction failed:\n{}", stderr);
    }

    Ok(frame_count)
}

/// Load a grayscale image from a PNG file
pub fn load_frame(path: &Path) -> Result<GrayImage> {
    let img =
        image::open(path).with_context(|| format!("Failed to open frame: {}", path.display()))?;
    Ok(img.into_luma8())
}
