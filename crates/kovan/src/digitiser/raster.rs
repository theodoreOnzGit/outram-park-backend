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
/// A quarter-turn, for a figure printed sideways on the page.
///
/// Closed set, enum-dispatched per the workspace Rust design rules. Only
/// multiples of 90 degrees: those are lossless index permutations, whereas an
/// arbitrary angle needs resampling and would blur a scanned plot's axis
/// ticks -- the very features a digitisation is read against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quarter {
    /// As scanned.
    #[default]
    None,
    /// 90 degrees clockwise.
    Clockwise,
    /// 90 degrees counter-clockwise.
    CounterClockwise,
    /// 180 degrees.
    Half,
}

impl Quarter {
    /// The next turn clockwise, cycling None -> CW -> Half -> CCW -> None.
    pub fn next_clockwise(self) -> Self {
        match self {
            Self::None => Self::Clockwise,
            Self::Clockwise => Self::Half,
            Self::Half => Self::CounterClockwise,
            Self::CounterClockwise => Self::None,
        }
    }

    /// Degrees clockwise, for display and for the provenance record.
    pub fn degrees(self) -> u32 {
        match self {
            Self::None => 0,
            Self::Clockwise => 90,
            Self::Half => 180,
            Self::CounterClockwise => 270,
        }
    }
}

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

    /// A quarter-turn of the image, for a figure printed sideways.
    ///
    /// Scanned reports routinely set a wide plot rotated 90 degrees on the
    /// page, and digitising one in that orientation means every axis is
    /// transposed -- the calibration would have to be entered against the
    /// wrong axes and every exported point would come out swapped
    /// (maintainer, 2026-09-24: "the last 3 plots are rotated 90 degrees").
    ///
    /// **Lossless.** A quarter turn is a permutation of pixel indices, not a
    /// resample: every source pixel appears exactly once in the result, with
    /// its colour unchanged. Width and height swap.
    ///
    /// The `source_sha256` is **kept**, because it identifies the bytes this
    /// raster was decoded from and that is still true after a turn -- the
    /// provenance question it answers is "which file did this come from",
    /// not "has it been transformed since". Any rotation applied is recorded
    /// separately, as a processing step (`DATA_POLICY.md`).
    pub fn rotated(&self, turn: Quarter) -> Self {
        let (w, h) = (self.width, self.height);
        let (nw, nh) = match turn {
            Quarter::None | Quarter::Half => (w, h),
            Quarter::Clockwise | Quarter::CounterClockwise => (h, w),
        };
        let mut pixels = Vec::with_capacity(self.pixels.len());
        for y in 0..nh {
            for x in 0..nw {
                // Map destination (x, y) back to the source pixel.
                let (sx, sy) = match turn {
                    Quarter::None => (x, y),
                    // Clockwise: the source's bottom-left becomes the
                    // destination's top-left.
                    Quarter::Clockwise => (y, h - 1 - x),
                    Quarter::CounterClockwise => (w - 1 - y, x),
                    Quarter::Half => (w - 1 - x, h - 1 - y),
                };
                pixels.push(self.pixels[(sy * w + sx) as usize]);
            }
        }
        Self {
            width: nw,
            height: nh,
            pixels,
            source_sha256: self.source_sha256.clone(),
        }
    }

    /// The largest fine-deskew angle offered, in degrees either way.
    ///
    /// Past roughly this the operator is looking at a quarter-turn problem,
    /// not a skew, and [`Self::rotated`] handles that losslessly.
    pub const MAX_DESKEW_DEGREES: f64 = 20.0;

    /// A small rotation about the image centre, to straighten a scan that
    /// went through the feeder crooked.
    ///
    /// # This RESAMPLES, unlike [`Self::rotated`]
    ///
    /// A quarter turn permutes pixel indices and loses nothing. An arbitrary
    /// angle cannot: destination pixels fall between source pixels, so this
    /// bilinearly interpolates, and every pass softens the image slightly.
    /// **Apply it once, from the original**, never repeatedly — the form
    /// holds an angle and re-derives from the unrotated raster for exactly
    /// that reason.
    ///
    /// # Consider the parallelogram calibration instead
    ///
    /// [`super::calibration::PlotCalibration::Parallelogram`] handles a
    /// skewed plot **without touching a single pixel**: four dragged corners
    /// define the skewed frame and a projective transform maps them onto a
    /// rectilinear data rectangle. That is strictly more accurate than
    /// straightening the image first, because it introduces no interpolation
    /// at all. This exists because a straightened picture is easier to place
    /// points on by eye, not because it is the more correct route.
    ///
    /// Dimensions are preserved and anything rotating in from outside the
    /// original is filled white, which is the page colour of a scanned plot.
    /// `angle_degrees` is clamped to ±[`Self::MAX_DESKEW_DEGREES`];
    /// positive is clockwise.
    pub fn deskewed(&self, angle_degrees: f64) -> Self {
        let angle = angle_degrees.clamp(-Self::MAX_DESKEW_DEGREES, Self::MAX_DESKEW_DEGREES);
        if angle == 0.0 {
            return self.clone();
        }
        let (w, h) = (self.width as f64, self.height as f64);
        let (cx, cy) = (w / 2.0, h / 2.0);
        // Rotate the SAMPLE point by -angle: we walk destination pixels and
        // ask where each came from.
        let (sin, cos) = (-angle.to_radians()).sin_cos();
        let mut pixels = Vec::with_capacity(self.pixels.len());
        for y in 0..self.height {
            for x in 0..self.width {
                let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                let sx = cx + dx * cos - dy * sin - 0.5;
                let sy = cy + dx * sin + dy * cos - 0.5;
                pixels.push(self.bilinear(sx, sy));
            }
        }
        Self {
            width: self.width,
            height: self.height,
            pixels,
            source_sha256: self.source_sha256.clone(),
        }
    }

    /// Bilinear sample at a fractional pixel position; white outside the
    /// image, which is a scanned page's own background.
    fn bilinear(&self, sx: f64, sy: f64) -> [u8; 3] {
        const WHITE: [u8; 3] = [255, 255, 255];
        if sx < -1.0 || sy < -1.0 || sx > self.width as f64 || sy > self.height as f64 {
            return WHITE;
        }
        let x0 = sx.floor();
        let y0 = sy.floor();
        let (fx, fy) = (sx - x0, sy - y0);
        let at = |x: f64, y: f64| -> [f64; 3] {
            if x < 0.0 || y < 0.0 || x >= self.width as f64 || y >= self.height as f64 {
                return [255.0, 255.0, 255.0];
            }
            let p = self.pixels[(y as u32 * self.width + x as u32) as usize];
            [p[0] as f64, p[1] as f64, p[2] as f64]
        };
        let (p00, p10) = (at(x0, y0), at(x0 + 1.0, y0));
        let (p01, p11) = (at(x0, y0 + 1.0), at(x0 + 1.0, y0 + 1.0));
        let mut out = [0u8; 3];
        for c in 0..3 {
            let top = p00[c] * (1.0 - fx) + p10[c] * fx;
            let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
            out[c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
        }
        out
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
///
/// `pub(crate)`: [`crate::relation`]'s relation-id generator hashes a
/// timestamp+counter with the same `sha2` dependency and reuses this rather
/// than a second hand-rolled hex formatter.
pub(crate) fn hex_lower(bytes: &[u8]) -> String {
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

    /// A quarter turn is a **permutation**, not a resample: every source
    /// pixel appears exactly once, unchanged. Anything else would blur the
    /// axis ticks a digitisation is read against.
    #[test]
    fn a_quarter_turn_is_lossless() {
        // Distinct colour per pixel, so any duplication or loss shows up.
        let r = PlotRaster::from_rgb_fn(3, 2, |x, y| [x as u8, y as u8, 7]);
        for turn in [
            Quarter::None,
            Quarter::Clockwise,
            Quarter::Half,
            Quarter::CounterClockwise,
        ] {
            let t = r.rotated(turn);
            let mut before: Vec<[u8; 3]> = (0..2)
                .flat_map(|y| (0..3).map(move |x| [x as u8, y as u8, 7]))
                .collect();
            let mut after: Vec<[u8; 3]> = (0..t.height())
                .flat_map(|y| (0..t.width()).map(move |x| (x, y)))
                .map(|(x, y)| t.rgb(x, y))
                .collect();
            before.sort();
            after.sort();
            assert_eq!(before, after, "{turn:?} lost or duplicated a pixel");
        }
    }

    /// Width and height swap on a quarter turn and are preserved on a half
    /// turn -- the thing that makes a sideways plot digitisable at all.
    #[test]
    fn a_quarter_turn_swaps_the_dimensions() {
        let r = PlotRaster::from_rgb_fn(8, 3, |_, _| [0, 0, 0]);
        assert_eq!(
            (
                r.rotated(Quarter::Clockwise).width(),
                r.rotated(Quarter::Clockwise).height()
            ),
            (3, 8)
        );
        assert_eq!(
            (
                r.rotated(Quarter::CounterClockwise).width(),
                r.rotated(Quarter::CounterClockwise).height()
            ),
            (3, 8)
        );
        assert_eq!(
            (
                r.rotated(Quarter::Half).width(),
                r.rotated(Quarter::Half).height()
            ),
            (8, 3)
        );
        assert_eq!(
            (
                r.rotated(Quarter::None).width(),
                r.rotated(Quarter::None).height()
            ),
            (8, 3)
        );
    }

    /// Clockwise takes the source's **bottom-left** corner to the
    /// destination's **top-left**. Getting this backwards mirrors the plot,
    /// which still looks like a plot -- so it needs pinning by corner, not
    /// by eye.
    #[test]
    fn clockwise_moves_the_corners_the_right_way() {
        // A 2x2 with a unique colour per corner.
        //   (0,0)=TL  (1,0)=TR
        //   (0,1)=BL  (1,1)=BR
        let r = PlotRaster::from_rgb_fn(2, 2, |x, y| [x as u8, y as u8, 0]);
        let tl = [0, 0, 0];
        let tr = [1, 0, 0];
        let bl = [0, 1, 0];
        let br = [1, 1, 0];

        let cw = r.rotated(Quarter::Clockwise);
        assert_eq!(cw.rgb(0, 0), bl, "CW: bottom-left must land top-left");
        assert_eq!(cw.rgb(1, 0), tl, "CW: top-left must land top-right");
        assert_eq!(cw.rgb(1, 1), tr);
        assert_eq!(cw.rgb(0, 1), br);

        let ccw = r.rotated(Quarter::CounterClockwise);
        assert_eq!(ccw.rgb(0, 0), tr, "CCW: top-right must land top-left");
    }

    /// Four clockwise turns return the original exactly, and CW then CCW is
    /// the identity -- so an operator who over-rotates loses nothing.
    #[test]
    fn rotations_compose_back_to_the_original() {
        let r = PlotRaster::from_rgb_fn(5, 3, |x, y| [x as u8, y as u8, (x + y) as u8]);
        let four = r
            .rotated(Quarter::Clockwise)
            .rotated(Quarter::Clockwise)
            .rotated(Quarter::Clockwise)
            .rotated(Quarter::Clockwise);
        assert_eq!(four, r, "four quarter turns is the identity");
        assert_eq!(
            r.rotated(Quarter::Clockwise)
                .rotated(Quarter::CounterClockwise),
            r,
            "a turn and its inverse cancel"
        );
        assert_eq!(r.rotated(Quarter::Half).rotated(Quarter::Half), r);
    }

    /// The turn cycle and its degree labels, which the UI shows and the
    /// provenance record stores.
    #[test]
    fn the_turn_cycle_goes_round_once() {
        let mut q = Quarter::None;
        let mut seen = vec![q.degrees()];
        for _ in 0..4 {
            q = q.next_clockwise();
            seen.push(q.degrees());
        }
        assert_eq!(seen, vec![0, 90, 180, 270, 0]);
    }

    /// The source hash survives a turn: it identifies the FILE this was
    /// decoded from, which a rotation does not change. The rotation itself
    /// is recorded separately as a processing step.
    #[test]
    fn the_source_hash_survives_a_turn() {
        let png = PlotRaster::from_rgb_fn(4, 2, |_, _| [1, 2, 3])
            .to_png_bytes()
            .expect("encode");
        let r = PlotRaster::from_bytes(&png).expect("decode");
        let hash = r.source_sha256().map(str::to_string);
        assert!(
            hash.is_some(),
            "precondition: decoded from bytes, so hashed"
        );
        assert_eq!(
            r.rotated(Quarter::Clockwise)
                .source_sha256()
                .map(str::to_string),
            hash
        );
    }

    /// Zero degrees is the identity, exactly -- no resampling pass, so no
    /// softening for an operator who nudges the slider and puts it back.
    #[test]
    fn a_zero_deskew_is_the_identity() {
        let r = PlotRaster::from_rgb_fn(6, 4, |x, y| [x as u8 * 30, y as u8 * 40, 9]);
        assert_eq!(r.deskewed(0.0), r);
    }

    /// Dimensions are preserved, and the angle is clamped to the offered
    /// range rather than silently accepting a quarter-turn-sized value that
    /// `rotated` would do losslessly.
    #[test]
    fn deskew_preserves_dimensions_and_clamps_the_angle() {
        let r = PlotRaster::from_rgb_fn(9, 5, |_, _| [10, 20, 30]);
        let d = r.deskewed(7.5);
        assert_eq!((d.width(), d.height()), (9, 5));
        // Past the limit, clamped -- 90 must not be honoured here.
        assert_eq!(r.deskewed(90.0), r.deskewed(PlotRaster::MAX_DESKEW_DEGREES));
        assert_eq!(
            r.deskewed(-90.0),
            r.deskewed(-PlotRaster::MAX_DESKEW_DEGREES)
        );
    }

    /// A uniform image stays uniform in its interior: interpolation between
    /// equal neighbours must return that value, not drift. Only the corners
    /// rotate in from outside and go white.
    #[test]
    fn deskew_does_not_shift_a_uniform_interior() {
        let r = PlotRaster::from_rgb_fn(40, 40, |_, _| [64, 128, 192]);
        let d = r.deskewed(10.0);
        // Well inside, away from anything that rotated in from outside.
        for (x, y) in [(20, 20), (15, 25), (25, 15)] {
            assert_eq!(
                d.rgb(x, y),
                [64, 128, 192],
                "interior drifted at ({x}, {y})"
            );
        }
    }

    /// Positive is CLOCKWISE. A single dark pixel above the centre must move
    /// to the RIGHT of vertical. Getting the sign backwards straightens a
    /// skew the wrong way, doubling it -- and the result still looks like a
    /// plot, so it needs pinning by direction.
    #[test]
    fn a_positive_deskew_turns_clockwise() {
        let (w, h) = (41u32, 41u32);
        // One dark pixel directly above centre.
        let r = PlotRaster::from_rgb_fn(w, h, |x, y| {
            if x == 20 && y == 8 {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        });
        let d = r.deskewed(15.0);
        // Find the darkest pixel in the top half.
        let mut best = (0u32, 0u32, 255i32);
        for y in 0..h / 2 {
            for x in 0..w {
                let v = d.rgb(x, y)[0] as i32;
                if v < best.2 {
                    best = (x, y, v);
                }
            }
        }
        assert!(best.2 < 200, "the mark should survive the resample");
        assert!(
            best.0 > 20,
            "clockwise must move a mark above centre to the RIGHT; it landed at x={}",
            best.0
        );
    }

    /// The source hash survives, as it does for a quarter turn: it
    /// identifies the file decoded from, not the pixels as they now stand.
    #[test]
    fn the_source_hash_survives_a_deskew() {
        let png = PlotRaster::from_rgb_fn(6, 4, |x, _| [x as u8, 0, 0])
            .to_png_bytes()
            .expect("encode");
        let r = PlotRaster::from_bytes(&png).expect("decode");
        assert_eq!(
            r.deskewed(5.0).source_sha256().map(str::to_string),
            r.source_sha256().map(str::to_string)
        );
    }
}
