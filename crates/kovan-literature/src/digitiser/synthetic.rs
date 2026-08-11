//! Synthetic plot rendering — deterministic ground-truth fixtures.
//!
//! Belongs here: [`SyntheticPlotSpec`] and [`render_synthetic_plot`], which
//! draw a *known analytic curve* into a [`PlotRaster`] at known pixel
//! positions and return the exact [`PlotCalibration`] used. The
//! self-consistency tests (`tests/digitiser_synthetic.rs`) digitise these
//! images and compare the recovered points against the analytic function —
//! the only ground truth available until the maintainer-supplied golden
//! oracle (bead `op-amfh`) lands. Keeping the renderer public also lets that
//! future oracle comparison reuse the same tolerance machinery.
//!
//! Does not belong here: any digitising. This module only *makes* images.

use super::calibration::{AxisCalibration, AxisRef, AxisScale, PlotCalibration};
use super::detect::PixelRect;
use super::raster::PlotRaster;
use super::DigitiserError;

/// Description of a synthetic plot: image size, frame placement, axis ranges
/// and scales, and the curve to draw.
///
/// The curve is a plain function pointer (`fn(f64) -> f64`), not a closure
/// trait object, per the workspace no-trait-objects rule; every fixture curve
/// is a free function anyway.
#[derive(Debug, Clone, Copy)]
pub struct SyntheticPlotSpec {
    /// Total image width in pixels.
    pub width: u32,
    /// Total image height in pixels.
    pub height: u32,
    /// Where the axis frame is drawn. Must fit inside the image with at
    /// least 1 px margin.
    pub frame: PixelRect,
    /// x-axis scale and the data values at the frame's left and right edges.
    pub x_scale: AxisScale,
    /// Data x at `frame.left`.
    pub x_min: f64,
    /// Data x at `frame.right`.
    pub x_max: f64,
    /// y-axis scale and the data values at the frame's bottom and top edges.
    pub y_scale: AxisScale,
    /// Data y at `frame.bottom` (rows grow downward, so the bottom edge is
    /// the *smaller* y for a conventional plot).
    pub y_min: f64,
    /// Data y at `frame.top`.
    pub y_max: f64,
    /// The curve to draw: `y = curve(x)` in data units.
    pub curve: fn(f64) -> f64,
    /// Half-thickness of the drawn curve in pixels (the drawn band spans
    /// `centre ± half`, so thickness is `2*half + 1`). 1 gives a 3-px line,
    /// typical of published figures.
    pub curve_half_thickness: u32,
}

/// Render the spec to an image, returning the raster **and the exact
/// calibration** implied by the frame/ranges (which is also the ground-truth
/// calibration a digitising test should use).
///
/// **Method (deterministic).** White background; 1-px black frame on the
/// spec's rectangle; then for every pixel column strictly inside the frame,
/// `x = cal.x.value_at(col)` and the curve pixel row is
/// `cal.y.pixel_at(curve(x))`. A vertical band of `2*half+1` px is inked at
/// the rounded row, and consecutive columns are connected by filling the row
/// interval between them, so steep curves have no gaps. Curve values that
/// fall outside the frame (or are non-finite / non-positive on a log axis)
/// are simply not drawn for that column.
///
/// # Errors
///
/// [`DigitiserError::Calibration`] if the axis ranges are invalid for their
/// scale (via [`AxisCalibration::new`]), or the frame does not fit in the
/// image.
pub fn render_synthetic_plot(
    spec: &SyntheticPlotSpec,
) -> Result<(PlotRaster, PlotCalibration), DigitiserError> {
    let f = spec.frame;
    if f.right >= spec.width || f.bottom >= spec.height || f.left >= f.right || f.top >= f.bottom {
        return Err(DigitiserError::Calibration(format!(
            "frame {f:?} does not fit in {}x{} image",
            spec.width, spec.height
        )));
    }
    let cal = PlotCalibration {
        x: AxisCalibration::new(
            spec.x_scale,
            AxisRef {
                pixel: f.left as f64,
                value: spec.x_min,
            },
            AxisRef {
                pixel: f.right as f64,
                value: spec.x_max,
            },
        )?,
        y: AxisCalibration::new(
            spec.y_scale,
            AxisRef {
                pixel: f.bottom as f64,
                value: spec.y_min,
            },
            AxisRef {
                pixel: f.top as f64,
                value: spec.y_max,
            },
        )?,
    };

    // Ink mask, row-major. Frame first.
    let w = spec.width as usize;
    let mut ink = vec![false; w * spec.height as usize];
    for x in f.left..=f.right {
        ink[f.top as usize * w + x as usize] = true;
        ink[f.bottom as usize * w + x as usize] = true;
    }
    for y in f.top..=f.bottom {
        ink[y as usize * w + f.left as usize] = true;
        ink[y as usize * w + f.right as usize] = true;
    }

    // Curve: centre row per column, then band + inter-column connection.
    let interior = (f.left + 1)..f.right; // strictly inside the frame
    let row_margin = (f.top + 1, f.bottom - 1);
    let mut prev_row: Option<i64> = None;
    for col in interior {
        let x = cal.x.value_at(col as f64);
        let y = (spec.curve)(x);
        let row = match cal.y.pixel_at(y) {
            Some(r) if r.is_finite() => r.round() as i64,
            _ => {
                prev_row = None;
                continue;
            }
        };
        if row < row_margin.0 as i64 || row > row_margin.1 as i64 {
            prev_row = None;
            continue;
        }
        // Vertical extent to ink this column: the band around `row`, plus the
        // span to the previous column's row so steep segments stay connected.
        let half = spec.curve_half_thickness as i64;
        let (mut lo, mut hi) = (row - half, row + half);
        if let Some(pr) = prev_row {
            if pr < lo {
                lo = pr + 1; // connect upward
            }
            if pr > hi {
                hi = pr - 1; // connect downward
            }
        }
        for r in lo.max(row_margin.0 as i64)..=hi.min(row_margin.1 as i64) {
            ink[r as usize * w + col as usize] = true;
        }
        prev_row = Some(row);
    }

    let raster = PlotRaster::from_rgb_fn(spec.width, spec.height, |x, y| {
        if ink[y as usize * w + x as usize] {
            [0, 0, 0]
        } else {
            [255, 255, 255]
        }
    });
    Ok((raster, cal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_x: f64) -> f64 {
        5.0
    }

    #[test]
    fn renders_frame_and_curve_where_the_calibration_says() {
        let spec = SyntheticPlotSpec {
            width: 200,
            height: 150,
            frame: PixelRect {
                left: 20,
                right: 180,
                top: 10,
                bottom: 140,
            },
            x_scale: AxisScale::Linear,
            x_min: 0.0,
            x_max: 10.0,
            y_scale: AxisScale::Linear,
            y_min: 0.0,
            y_max: 10.0,
            curve: flat,
            curve_half_thickness: 1,
        };
        let (img, cal) = render_synthetic_plot(&spec).unwrap();
        // Frame corners are ink.
        assert_eq!(img.rgb(20, 10), [0, 0, 0]);
        assert_eq!(img.rgb(180, 140), [0, 0, 0]);
        // The curve y=5 must sit at the calibrated row, mid-frame.
        let row = cal.y.pixel_at(5.0).unwrap().round() as u32;
        assert_eq!(img.rgb(100, row), [0, 0, 0]);
        // And nowhere near the top edge interior.
        assert_eq!(img.rgb(100, 15), [255, 255, 255]);
    }
}
