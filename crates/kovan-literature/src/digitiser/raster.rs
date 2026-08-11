//! Plot image loading — an owned RGB pixel buffer decoded with pure Rust.
//!
//! Belongs here: [`PlotRaster`] (the in-memory image the whole digitiser
//! works on) and its constructors. Decoding uses the `image` crate's
//! pure-Rust PNG/JPEG decoders — no C toolchain, no system libraries, so the
//! engine builds natively on Termux/Android.
//!
//! Does not belong here: axis geometry ([`super::detect`]), curve pixels
//! ([`super::trace`]), or any pixel *interpretation* beyond luminance.

use std::path::Path;

use sha2::{Digest, Sha256};

use super::DigitiserError;

/// An owned, row-major RGB8 plot image.
///
/// The public API deliberately does not expose `image`-crate types, so a
/// caller only needs this struct and plain integers to work with the
/// digitiser (workspace "human interface layer" rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlotRaster {
    width: u32,
    height: u32,
    /// Row-major `[r, g, b]` triples, `width * height` of them.
    pixels: Vec<[u8; 3]>,
    /// Lowercase-hex SHA-256 of the *source file bytes* this raster was
    /// decoded from, when it came from a file or byte buffer. `None` for
    /// synthetically generated rasters. Recorded as provenance
    /// (`DATA_POLICY.md`: digitisation is a documented processing step).
    source_sha256: Option<String>,
}

impl PlotRaster {
    /// Decode a plot image from a file on disk (PNG or JPEG).
    ///
    /// Records the SHA-256 of the file bytes for provenance.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Image`] if the file cannot be read or decoded.
    pub fn from_path(path: &Path) -> Result<Self, DigitiserError> {
        let bytes = std::fs::read(path)
            .map_err(|e| DigitiserError::Image(format!("cannot read {}: {e}", path.display())))?;
        Self::from_bytes(&bytes)
    }

    /// Decode a plot image from in-memory encoded bytes (PNG or JPEG).
    ///
    /// Records the SHA-256 of `bytes` for provenance.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Image`] if the bytes are not a decodable image.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DigitiserError> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| DigitiserError::Image(format!("cannot decode image: {e}")))?
            .to_rgb8();
        let sha = {
            let mut h = Sha256::new();
            h.update(bytes);
            hex_lower(&h.finalize())
        };
        let mut r = Self::from_rgb_fn(img.width(), img.height(), |x, y| {
            let p = img.get_pixel(x, y);
            [p.0[0], p.0[1], p.0[2]]
        });
        r.source_sha256 = Some(sha);
        Ok(r)
    }

    /// Build a raster from a pixel generator function — used by
    /// [`super::synthetic`] to render test fixtures. `f(x, y)` returns the
    /// RGB triple for column `x`, row `y`. No source hash is recorded.
    pub fn from_rgb_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Self {
        let mut pixels = Vec::with_capacity(width as usize * height as usize);
        for y in 0..height {
            for x in 0..width {
                pixels.push(f(x, y));
            }
        }
        Self {
            width,
            height,
            pixels,
            source_sha256: None,
        }
    }

    /// Image width in pixels (number of columns).
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Image height in pixels (number of rows).
    pub fn height(&self) -> u32 {
        self.height
    }

    /// RGB triple at column `x`, row `y` (row 0 is the top of the image).
    ///
    /// # Panics
    ///
    /// Panics if `x >= width` or `y >= height`.
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        assert!(x < self.width && y < self.height, "pixel out of bounds");
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    /// Rec. 709 luminance of the pixel at `(x, y)`, 0 (black) – 255 (white).
    ///
    /// This is the single greyscale definition the whole digitiser uses for
    /// "dark" tests (axis lines, default curve selection).
    pub fn luminance(&self, x: u32, y: u32) -> u8 {
        let [r, g, b] = self.rgb(x, y);
        // Integer Rec. 709: Y = 0.2126 R + 0.7152 G + 0.0722 B.
        ((2126 * r as u32 + 7152 * g as u32 + 722 * b as u32) / 10000) as u8
    }

    /// Lowercase-hex SHA-256 of the encoded source bytes, when this raster
    /// was decoded from a file/byte buffer; `None` for synthetic rasters.
    pub fn source_sha256(&self) -> Option<&str> {
        self.source_sha256.as_deref()
    }

    /// Encode this raster as PNG bytes (pure Rust). Used to write synthetic
    /// fixtures to disk for inspection and by tests that exercise the
    /// decode path.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Image`] if PNG encoding fails (should not happen for
    /// a well-formed buffer).
    pub fn to_png_bytes(&self) -> Result<Vec<u8>, DigitiserError> {
        let mut img = image::RgbImage::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                img.put_pixel(x, y, image::Rgb(self.rgb(x, y)));
            }
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png)
            .map_err(|e| DigitiserError::Image(format!("png encode failed: {e}")))?;
        Ok(out.into_inner())
    }
}

/// Lowercase hex of a byte slice (SHA-256 digests here).
fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_round_trip_preserves_pixels_and_records_sha() {
        let r = PlotRaster::from_rgb_fn(8, 4, |x, y| [(x * 30) as u8, (y * 60) as u8, 7]);
        assert!(r.source_sha256().is_none());
        let png = r.to_png_bytes().unwrap();
        let back = PlotRaster::from_bytes(&png).unwrap();
        assert_eq!(back.width(), 8);
        assert_eq!(back.height(), 4);
        assert_eq!(back.rgb(3, 2), r.rgb(3, 2));
        let sha = back.source_sha256().unwrap();
        assert_eq!(sha.len(), 64);
        // Deterministic: same bytes, same hash.
        assert_eq!(
            PlotRaster::from_bytes(&png)
                .unwrap()
                .source_sha256()
                .unwrap(),
            sha
        );
    }

    #[test]
    fn luminance_extremes() {
        let r = PlotRaster::from_rgb_fn(
            2,
            1,
            |x, _| if x == 0 { [0, 0, 0] } else { [255, 255, 255] },
        );
        assert_eq!(r.luminance(0, 0), 0);
        assert!(r.luminance(1, 0) >= 254);
    }
}
