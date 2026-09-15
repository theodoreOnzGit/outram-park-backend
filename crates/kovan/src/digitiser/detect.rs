//! Automatic plot-frame detection — finding the axis box in pixel space.
//!
//! Belongs here: [`PixelRect`], [`DetectConfig`], and
//! [`detect_plot_frame`], which locates the rectangle bounded by the plot's
//! axis lines by scanning for long dark horizontal/vertical pixel runs.
//! Deterministic; no ML, no OCR — it finds *where* the axes are, never what
//! their tick labels say (the caller supplies the numeric axis values, see
//! the [`super`] module doc).
//!
//! Does not belong here: calibration values ([`super::calibration`]) or curve
//! pixels ([`super::trace`]).

use serde::{Deserialize, Serialize};

use super::raster::PlotRaster;
use super::DigitiserError;

/// An axis-aligned pixel rectangle, inclusive on all four edges.
///
/// Rows increase downward, so `top < bottom` numerically while `top` is the
/// visually upper edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelRect {
    /// Leftmost column (inclusive).
    pub left: u32,
    /// Rightmost column (inclusive). Always `> left`.
    pub right: u32,
    /// Topmost row (inclusive; visually the upper edge).
    pub top: u32,
    /// Bottommost row (inclusive; visually the lower edge). Always `> top`.
    pub bottom: u32,
}

impl PixelRect {
    /// Width in pixels (inclusive of both edges).
    pub fn width(&self) -> u32 {
        self.right - self.left + 1
    }

    /// Height in pixels (inclusive of both edges).
    pub fn height(&self) -> u32 {
        self.bottom - self.top + 1
    }
}

/// Tuning knobs for [`detect_plot_frame`]. [`DetectConfig::default`] suits
/// typical black-on-white published figures.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DetectConfig {
    /// A pixel with Rec. 709 luminance strictly below this counts as "dark"
    /// (axis-line ink). Default 128 — the midpoint, tolerant of grey
    /// anti-aliasing and scan noise.
    pub dark_threshold: u8,
    /// A row/column is an axis-line candidate when its longest contiguous
    /// dark run covers at least this fraction of the image's
    /// width/height. Default 0.4 — axis lines span most of a cropped figure;
    /// curve segments and tick marks do not.
    pub min_line_fraction: f64,
}

impl Default for DetectConfig {
    fn default() -> Self {
        Self {
            dark_threshold: 128,
            min_line_fraction: 0.4,
        }
    }
}

/// Detect the plot frame (axis box) of a black-on-white figure.
///
/// **Method (deterministic).** Every row's and column's longest contiguous
/// dark run is measured. Rows/columns whose run covers at least
/// [`DetectConfig::min_line_fraction`] of the image dimension are axis-line
/// candidates. With two or more candidate rows *and* columns (a fully boxed
/// plot), the frame is their outermost members. With exactly one of either
/// (an L-shaped plot: one x axis, one y axis), the missing top/right edges
/// are taken from the dark extent of the detected axis lines themselves.
///
/// # Errors
///
/// [`DigitiserError::Detection`] when no candidate row or column exists, or
/// the resulting rectangle is degenerate (under 10 px in either direction) —
/// in that case supply explicit pixel reference points instead (see
/// [`super::auto::AxisPixelRefs`]).
pub fn detect_plot_frame(
    raster: &PlotRaster,
    config: &DetectConfig,
) -> Result<PixelRect, DigitiserError> {
    let w = raster.width();
    let h = raster.height();
    if w < 16 || h < 16 {
        return Err(DigitiserError::Detection(format!(
            "image {w}x{h} too small to hold a plot frame"
        )));
    }

    // Longest contiguous dark run in each row / column.
    let row_run = |y: u32| longest_dark_run(w, |x| raster.luminance(x, y) < config.dark_threshold);
    let col_run = |x: u32| longest_dark_run(h, |y| raster.luminance(x, y) < config.dark_threshold);

    let min_row_run = (config.min_line_fraction * w as f64) as u32;
    let min_col_run = (config.min_line_fraction * h as f64) as u32;

    let line_rows: Vec<u32> = (0..h).filter(|&y| row_run(y).len >= min_row_run).collect();
    let line_cols: Vec<u32> = (0..w).filter(|&x| col_run(x).len >= min_col_run).collect();

    if line_rows.is_empty() || line_cols.is_empty() {
        return Err(DigitiserError::Detection(format!(
            "no axis lines found ({} candidate rows, {} candidate columns); \
             lower `min_line_fraction`/raise `dark_threshold`, or give explicit pixel refs",
            line_rows.len(),
            line_cols.len()
        )));
    }

    // Outermost candidates. For an L-shaped plot (axis lines only on the
    // bottom/left), a thick axis line yields several *adjacent* candidates,
    // which would collapse the frame to the line thickness — detect that and
    // fall back to the dark extents of the axis lines themselves.
    let bottom = *line_rows.iter().max().expect("non-empty");
    let mut top = *line_rows.iter().min().expect("non-empty");
    let left = *line_cols.iter().min().expect("non-empty");
    let mut right = *line_cols.iter().max().expect("non-empty");

    let contiguous = |v: &[u32]| v.windows(2).all(|p| p[1] - p[0] <= 2) && v.len() <= 6;

    if contiguous(&line_rows) {
        // Only the x axis found: top edge = top of the y-axis line's dark run.
        top = col_run(left).start;
    }
    if contiguous(&line_cols) {
        // Only the y axis found: right edge = end of the x-axis line's run.
        right = row_run(bottom).start + row_run(bottom).len - 1;
    }

    if right <= left + 10 || bottom <= top + 10 {
        return Err(DigitiserError::Detection(format!(
            "detected frame is degenerate: left {left}, right {right}, top {top}, bottom {bottom}"
        )));
    }

    Ok(PixelRect {
        left,
        right,
        top,
        bottom,
    })
}

/// A contiguous run of `true` results: start index and length.
struct Run {
    start: u32,
    len: u32,
}

/// Longest contiguous run of indices `0..n` where `is_dark(i)` holds.
/// Returns a zero-length run at 0 when nothing is dark. Ties keep the first
/// (deterministic).
fn longest_dark_run(n: u32, is_dark: impl Fn(u32) -> bool) -> Run {
    let mut best = Run { start: 0, len: 0 };
    let mut cur_start = 0u32;
    let mut cur_len = 0u32;
    for i in 0..n {
        if is_dark(i) {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best.len {
                best = Run {
                    start: cur_start,
                    len: cur_len,
                };
            }
        } else {
            cur_len = 0;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    /// White image with a black rectangle outline drawn at the given rect.
    fn boxed_image(w: u32, h: u32, r: PixelRect) -> PlotRaster {
        PlotRaster::from_rgb_fn(w, h, |x, y| {
            let on_frame = ((y == r.top || y == r.bottom) && (r.left..=r.right).contains(&x))
                || ((x == r.left || x == r.right) && (r.top..=r.bottom).contains(&y));
            if on_frame {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        })
    }

    #[test]
    fn detects_a_full_box() {
        let want = PixelRect {
            left: 40,
            right: 360,
            top: 30,
            bottom: 260,
        };
        let img = boxed_image(400, 300, want);
        let got = detect_plot_frame(&img, &DetectConfig::default()).unwrap();
        assert_eq!(got, want);
    }

    #[test]
    fn detects_an_l_shaped_axis_pair() {
        // Only bottom + left axis lines, no top/right frame.
        let r = PixelRect {
            left: 50,
            right: 350,
            top: 40,
            bottom: 250,
        };
        let img = PlotRaster::from_rgb_fn(400, 300, |x, y| {
            let on_axis = (y == r.bottom && (r.left..=r.right).contains(&x))
                || (x == r.left && (r.top..=r.bottom).contains(&y));
            if on_axis {
                [0, 0, 0]
            } else {
                [255, 255, 255]
            }
        });
        let got = detect_plot_frame(&img, &DetectConfig::default()).unwrap();
        assert_eq!(got, r);
    }

    #[test]
    fn blank_image_is_a_detection_error() {
        let img = PlotRaster::from_rgb_fn(100, 100, |_, _| [255, 255, 255]);
        assert!(matches!(
            detect_plot_frame(&img, &DetectConfig::default()),
            Err(DigitiserError::Detection(_))
        ));
    }
}
