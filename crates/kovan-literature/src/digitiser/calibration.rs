//! Axis calibration — mapping pixel coordinates to data coordinates.
//!
//! Belongs here: [`AxisScale`], [`AxisRef`], [`AxisCalibration`],
//! [`PlotCalibration`], and the pixel ↔ data-value maps. Logarithmic axes are
//! interpolated in **log10 space** — the pixel position of a value on a log
//! axis is affine in `log10(value)`, not in the value itself, and getting
//! this wrong is the classic digitisation error this module exists to avoid.
//!
//! Does not belong here: image handling ([`super::raster`]), curve extraction
//! ([`super::trace`]), output formats ([`super::dataset`]).

use serde::{Deserialize, Serialize};

use super::DigitiserError;

/// Whether an axis is linear or logarithmic. Closed set — enum-dispatched per
/// the workspace Rust design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AxisScale {
    /// Value is an affine function of pixel position.
    Linear,
    /// `log10(value)` is an affine function of pixel position (decade-ruled
    /// axis). Reference values must be strictly positive.
    Logarithmic,
}

impl std::fmt::Display for AxisScale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AxisScale::Linear => write!(f, "linear"),
            AxisScale::Logarithmic => write!(f, "log"),
        }
    }
}

/// One axis reference point: a pixel coordinate along the axis direction
/// (column index for the x axis, row index for the y axis) paired with the
/// data value the figure assigns to that pixel.
///
/// `pixel` is an `f64` because reference points may be placed with sub-pixel
/// precision (e.g. the centre of a 2-px-thick axis line). `value` is in
/// *document units* — whatever the source figure's axis label says (see the
/// module doc of [`super`] for why `uom` is not used here).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisRef {
    /// Pixel coordinate along this axis (x axis → column, y axis → row;
    /// image rows increase downward).
    pub pixel: f64,
    /// Data value at that pixel, in the figure's own units.
    pub value: f64,
}

/// Calibration of a single axis from two reference points.
///
/// Construct with [`AxisCalibration::new`], which validates the references;
/// the fields stay public so a deserialised calibration can be inspected, but
/// prefer the constructor for anything built at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AxisCalibration {
    /// Linear or logarithmic interpolation between the reference points.
    pub scale: AxisScale,
    /// First reference point.
    pub r1: AxisRef,
    /// Second reference point. Must differ from `r1` in both pixel and value.
    pub r2: AxisRef,
}

impl AxisCalibration {
    /// Build a validated axis calibration.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Calibration`] if the reference pixels are closer
    /// than 1 px (the map would be ill-conditioned), the reference values
    /// coincide, any value is non-finite, or the scale is
    /// [`AxisScale::Logarithmic`] and either value is `<= 0`.
    pub fn new(scale: AxisScale, r1: AxisRef, r2: AxisRef) -> Result<Self, DigitiserError> {
        if !r1.pixel.is_finite()
            || !r2.pixel.is_finite()
            || !r1.value.is_finite()
            || !r2.value.is_finite()
        {
            return Err(DigitiserError::Calibration(
                "reference pixels/values must be finite".to_string(),
            ));
        }
        if (r1.pixel - r2.pixel).abs() < 1.0 {
            return Err(DigitiserError::Calibration(format!(
                "reference pixels {} and {} are less than 1 px apart",
                r1.pixel, r2.pixel
            )));
        }
        if r1.value == r2.value {
            return Err(DigitiserError::Calibration(format!(
                "reference values are both {}",
                r1.value
            )));
        }
        if scale == AxisScale::Logarithmic && (r1.value <= 0.0 || r2.value <= 0.0) {
            return Err(DigitiserError::Calibration(format!(
                "logarithmic axis needs strictly positive reference values, got {} and {}",
                r1.value, r2.value
            )));
        }
        Ok(Self { scale, r1, r2 })
    }

    /// Data value at pixel coordinate `pixel`, in the figure's own units.
    ///
    /// Linear axes interpolate/extrapolate the value directly; logarithmic
    /// axes interpolate `log10(value)` and exponentiate. Pixels outside the
    /// reference span extrapolate on the same rule.
    pub fn value_at(&self, pixel: f64) -> f64 {
        let t = (pixel - self.r1.pixel) / (self.r2.pixel - self.r1.pixel);
        match self.scale {
            AxisScale::Linear => self.r1.value + t * (self.r2.value - self.r1.value),
            AxisScale::Logarithmic => {
                let l1 = self.r1.value.log10();
                let l2 = self.r2.value.log10();
                10f64.powf(l1 + t * (l2 - l1))
            }
        }
    }

    /// Pixel coordinate at which `value` sits on this axis — the inverse of
    /// [`AxisCalibration::value_at`].
    ///
    /// Returns `None` when the value cannot be placed: non-finite input, or a
    /// non-positive value on a logarithmic axis.
    pub fn pixel_at(&self, value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        let t = match self.scale {
            AxisScale::Linear => (value - self.r1.value) / (self.r2.value - self.r1.value),
            AxisScale::Logarithmic => {
                if value <= 0.0 {
                    return None;
                }
                let l1 = self.r1.value.log10();
                let l2 = self.r2.value.log10();
                (value.log10() - l1) / (l2 - l1)
            }
        };
        Some(self.r1.pixel + t * (self.r2.pixel - self.r1.pixel))
    }
}

/// Full two-axis calibration of a plot: an [`AxisCalibration`] for x (pixel
/// columns) and one for y (pixel rows; rows increase *downward*, which the
/// two-point form handles with no special casing — the bottom-of-plot
/// reference simply has the larger row index).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlotCalibration {
    /// Horizontal axis (pixel columns → data x).
    pub x: AxisCalibration,
    /// Vertical axis (pixel rows → data y).
    pub y: AxisCalibration,
}

impl PlotCalibration {
    /// Map an image pixel `(column, row)` to data coordinates `(x, y)`.
    pub fn point_at(&self, x_px: f64, y_px: f64) -> (f64, f64) {
        (self.x.value_at(x_px), self.y.value_at(y_px))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(p1: f64, v1: f64, p2: f64, v2: f64) -> AxisCalibration {
        AxisCalibration::new(
            AxisScale::Linear,
            AxisRef {
                pixel: p1,
                value: v1,
            },
            AxisRef {
                pixel: p2,
                value: v2,
            },
        )
        .unwrap()
    }

    fn log(p1: f64, v1: f64, p2: f64, v2: f64) -> AxisCalibration {
        AxisCalibration::new(
            AxisScale::Logarithmic,
            AxisRef {
                pixel: p1,
                value: v1,
            },
            AxisRef {
                pixel: p2,
                value: v2,
            },
        )
        .unwrap()
    }

    #[test]
    fn linear_axis_interpolates_and_extrapolates() {
        let c = lin(100.0, 0.0, 600.0, 10.0);
        assert!((c.value_at(100.0) - 0.0).abs() < 1e-12);
        assert!((c.value_at(600.0) - 10.0).abs() < 1e-12);
        assert!((c.value_at(350.0) - 5.0).abs() < 1e-12);
        assert!((c.value_at(700.0) - 12.0).abs() < 1e-12); // extrapolation
    }

    #[test]
    fn log_axis_midpixel_is_geometric_mean_not_arithmetic() {
        // The decisive log-axis property: halfway in pixels between decades
        // 1 and 100 is sqrt(1*100) = 10, NOT (1+100)/2 = 50.5.
        let c = log(0.0, 1.0, 200.0, 100.0);
        let mid = c.value_at(100.0);
        assert!((mid - 10.0).abs() < 1e-9, "got {mid}");
    }

    #[test]
    fn log_axis_round_trips_pixel_value_pixel() {
        let c = log(50.0, 1e-2, 850.0, 1e6);
        for px in [50.0, 123.4, 400.0, 850.0] {
            let v = c.value_at(px);
            let back = c.pixel_at(v).unwrap();
            assert!((back - px).abs() < 1e-9, "px {px} -> {v} -> {back}");
        }
    }

    #[test]
    fn y_axis_downward_rows_need_no_special_case() {
        // Row 500 is the bottom (y=0), row 100 is the top (y=40).
        let c = lin(500.0, 0.0, 100.0, 40.0);
        assert!((c.value_at(300.0) - 20.0).abs() < 1e-12);
    }

    #[test]
    fn invalid_calibrations_are_rejected() {
        let r = |p, v| AxisRef { pixel: p, value: v };
        assert!(AxisCalibration::new(AxisScale::Linear, r(10.0, 0.0), r(10.5, 1.0)).is_err());
        assert!(AxisCalibration::new(AxisScale::Linear, r(0.0, 3.0), r(90.0, 3.0)).is_err());
        assert!(AxisCalibration::new(AxisScale::Logarithmic, r(0.0, 0.0), r(90.0, 1.0)).is_err());
        assert!(AxisCalibration::new(AxisScale::Logarithmic, r(0.0, -1.0), r(90.0, 1.0)).is_err());
        assert!(AxisCalibration::new(AxisScale::Linear, r(f64::NAN, 0.0), r(90.0, 1.0)).is_err());
    }

    #[test]
    fn pixel_at_rejects_nonpositive_on_log() {
        let c = log(0.0, 1.0, 100.0, 10.0);
        assert!(c.pixel_at(0.0).is_none());
        assert!(c.pixel_at(-5.0).is_none());
        assert!(c.pixel_at(f64::NAN).is_none());
    }
}
