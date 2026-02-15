use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::{GrayImage, Luma};
use qrcode::QrCode;
use rqrr::PreparedImage;

/// Generate a QR code image from binary data at the specified error correction level.
/// The data is base64-encoded first so it survives QR text-mode roundtrips.
/// The image is sized to fit within the given dimensions with proper quiet zone.
pub fn generate_qr_image(
    data: &[u8],
    ec_level: qrcode::EcLevel,
    width: u32,
    height: u32,
) -> Result<GrayImage> {
    // Base64 encode so binary data survives QR text-mode decode
    let encoded = BASE64.encode(data);

    let code = QrCode::with_error_correction_level(encoded.as_bytes(), ec_level)
        .context("Failed to create QR code (data may be too large for a single frame)")?;

    let qr_size = code.width() as u32;

    // Calculate module size to fit within dimensions
    // Leave quiet zone of 4 modules on each side
    let available = width.min(height);
    let module_size = available / (qr_size + 8);
    let module_size = module_size.max(1);

    let total_qr_pixels = qr_size * module_size;
    let offset_x = (width - total_qr_pixels) / 2;
    let offset_y = (height - total_qr_pixels) / 2;

    let mut img = GrayImage::from_pixel(width, height, Luma([255u8]));

    for (y, row) in code.to_colors().chunks(qr_size as usize).enumerate() {
        for (x, &color) in row.iter().enumerate() {
            let pixel_val = match color {
                qrcode::Color::Dark => 0u8,
                qrcode::Color::Light => 255u8,
            };

            for dy in 0..module_size {
                for dx in 0..module_size {
                    let px = offset_x + (x as u32) * module_size + dx;
                    let py = offset_y + (y as u32) * module_size + dy;
                    if px < width && py < height {
                        img.put_pixel(px, py, Luma([pixel_val]));
                    }
                }
            }
        }
    }

    Ok(img)
}

/// Decode a QR code from a grayscale image, returning the raw binary data.
/// The QR content is base64-decoded back to binary.
pub fn decode_qr_image(img: &GrayImage) -> Result<Vec<u8>> {
    let mut prepared = PreparedImage::prepare(img.clone());
    let grids = prepared.detect_grids();

    if grids.is_empty() {
        anyhow::bail!("No QR code detected in frame");
    }

    let (_meta, content) = grids[0]
        .decode()
        .map_err(|e| anyhow::anyhow!("Failed to decode QR code: {:?}", e))?;

    // Base64 decode back to binary
    let data = BASE64
        .decode(&content)
        .context("Failed to base64-decode QR content")?;

    Ok(data)
}

/// Dynamically find the maximum binary data capacity that fits in a QR code
/// at the given error correction level. This accounts for base64 overhead
/// and any crate-specific limitations.
pub fn find_max_capacity(ec_level: qrcode::EcLevel) -> usize {
    let mut low: usize = 1;
    let mut high: usize = 3000;

    while low < high {
        let mid = (low + high + 1) / 2;
        // Test with representative binary data (not all zeros - use pattern)
        let test_data: Vec<u8> = (0..mid).map(|i| (i % 256) as u8).collect();
        let encoded = BASE64.encode(&test_data);
        if QrCode::with_error_correction_level(encoded.as_bytes(), ec_level).is_ok() {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    low
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qr_roundtrip() {
        let data = b"Hello, QR World! This is a test payload.";
        let img = generate_qr_image(data, qrcode::EcLevel::H, 720, 720).unwrap();
        let decoded = decode_qr_image(&img).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_qr_binary_data() {
        let data: Vec<u8> = (0..=255).collect();
        let img = generate_qr_image(&data, qrcode::EcLevel::M, 1080, 1080).unwrap();
        let decoded = decode_qr_image(&img).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_find_capacity() {
        let cap = find_max_capacity(qrcode::EcLevel::H);
        assert!(cap > 500, "H capacity should be > 500, got {}", cap);
        assert!(cap < 2000, "H capacity should be < 2000, got {}", cap);
    }
}
