//! Readback sanity checks shared by the optional GPU runner and CPU-only tests.
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct CapturePixelStats {
    pub width: u32,
    pub height: u32,
    pub visible_nonblack_pixels: usize,
    pub visible_channel_range: u8,
}

/// Reject an empty readback, including a flat central scene with valid black
/// bars. This is a failure guard, not a pixel-fidelity or complete-draw test.
pub fn validate_capture_pixels(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<CapturePixelStats, String> {
    if width != 512 || height != 512 || rgba.len() != 512 * 512 * 4 {
        return Err("Capture must be a complete 512x512 RGBA image".into());
    }
    let mut minima = [u8::MAX; 3];
    let mut maxima = [0; 3];
    let mut nonblack = 0;
    for pixel in rgba[144 * 512 * 4..368 * 512 * 4].chunks_exact(4) {
        if pixel[..3].iter().any(|&v| v > 4) {
            nonblack += 1;
        }
        for channel in 0..3 {
            minima[channel] = minima[channel].min(pixel[channel]);
            maxima[channel] = maxima[channel].max(pixel[channel]);
        }
    }
    let range = (0..3).map(|i| maxima[i] - minima[i]).max().unwrap_or(0);
    if nonblack < 256 || range < 8 {
        return Err(format!(
            "Capture is blank or flat in the visible scene: {nonblack} nonblack pixels, channel range {range}"
        ));
    }
    Ok(CapturePixelStats {
        width,
        height,
        visible_nonblack_pixels: nonblack,
        visible_channel_range: range,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_black_and_flat_scene_readbacks() {
        assert!(validate_capture_pixels(512, 512, &[]).is_err());
        let mut rgba = vec![0; 512 * 512 * 4];
        assert!(validate_capture_pixels(512, 512, &rgba).is_err());
        for pixel in rgba[144 * 512 * 4..368 * 512 * 4].chunks_exact_mut(4) {
            pixel.copy_from_slice(&[128, 32, 9, 255]);
        }
        assert!(validate_capture_pixels(512, 512, &rgba).is_err());
        for (i, pixel) in rgba[144 * 512 * 4..368 * 512 * 4]
            .chunks_exact_mut(4)
            .enumerate()
        {
            pixel[0] = (i % 256) as u8;
        }
        assert_eq!(
            validate_capture_pixels(512, 512, &rgba)
                .unwrap()
                .visible_channel_range,
            255
        );
    }
}
