//! Axis calibration — mapping pixel coordinates to data coordinates.
//!
//! Belongs here: [`AxisScale`], [`AxisRef`], [`AxisCalibration`],
//! [`PlotCalibration`] (with its [`PlotCalibration::AxisAligned`] and
//! [`PlotCalibration::Parallelogram`] variants — op-vyb9), and the pixel ↔
//! data-value maps. Logarithmic axes are interpolated in **log10 space** —
//! the pixel position of a value on a log axis is affine in `log10(value)`,
//! not in the value itself, and getting this wrong is the classic
//! digitisation error this module exists to avoid.
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

/// Full two-axis calibration of a plot — either the original **axis-aligned**
/// form (an [`AxisCalibration`] for x, one for y) or a **parallelogram/skewed**
/// form for a plot photographed or scanned at an angle (op-vyb9, GitHub issue
/// #30: "it may be helpful to have a box or parallelogram (in case graph is
/// off centre)"). Enum-dispatched per the workspace's no-trait-objects rule —
/// the two shapes coexist, selectable, rather than one replacing the other.
///
/// ## Backward compatibility (deliberate, load-bearing)
///
/// `#[serde(untagged)]` makes [`Self::AxisAligned`] serialise to **exactly**
/// the flat `{"x": ..., "y": ...}` shape this type had before it became an
/// enum — byte-identical to every dataset already exported under
/// [`super::dataset::DATASET_SCHEMA_VERSION`] 1. A pre-existing on-disk
/// dataset therefore still deserialises unchanged (untagged deserialisation
/// tries each variant's own shape against the JSON in order, and the old flat
/// shape only matches `AxisAligned`'s field set); a freshly-written
/// `Parallelogram` uses a structurally distinct field set
/// ([`ParallelogramCalibration`]'s own fields) so the two variants can never
/// be confused for one another.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PlotCalibration {
    /// The original shape: horizontal axis (pixel columns → data x) and
    /// vertical axis (pixel rows → data y), independent of each other.
    AxisAligned {
        x: AxisCalibration,
        y: AxisCalibration,
    },
    /// A skewed pixel-space quadrilateral mapped onto a rectilinear data
    /// rectangle — see [`ParallelogramCalibration`].
    Parallelogram(ParallelogramCalibration),
}

impl PlotCalibration {
    /// Map an image pixel `(column, row)` to data coordinates `(x, y)`.
    ///
    /// [`Self::Parallelogram`] returns `(f64::NAN, f64::NAN)` only if its
    /// four pixel corners are degenerate (e.g. three collinear, or a
    /// repeated corner) — [`ParallelogramCalibration::new`] already rejects
    /// that at construction time, so this can only happen for a value
    /// deserialised from hand-edited or corrupted JSON that bypassed the
    /// constructor. This mirrors how a malformed [`AxisCalibration`] would
    /// already have been rejected at its own construction time, rather than
    /// introducing a new fallible path into an otherwise-infallible method.
    pub fn point_at(&self, x_px: f64, y_px: f64) -> (f64, f64) {
        match self {
            Self::AxisAligned { x, y } => (x.value_at(x_px), y.value_at(y_px)),
            Self::Parallelogram(p) => p.point_at(x_px, y_px),
        }
    }

    /// The inverse of [`Self::point_at`]: the pixel `(column, row)` a data
    /// value `(x, y)` would sit at. `None` when the value cannot be placed
    /// (see [`AxisCalibration::pixel_at`]/[`ParallelogramCalibration::pixel_at`]).
    pub fn pixel_at(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        match self {
            Self::AxisAligned { x: xc, y: yc } => Some((xc.pixel_at(x)?, yc.pixel_at(y)?)),
            Self::Parallelogram(p) => p.pixel_at(x, y),
        }
    }
}

/// A pixel-space point (sub-pixel precision, same convention as
/// [`AxisRef::pixel`]).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PixelPoint {
    pub x: f64,
    pub y: f64,
}

/// Calibration from a (possibly skewed) pixel-space quadrilateral to a
/// rectilinear data-value rectangle (op-vyb9) — for a plot photographed or
/// scanned at an angle, where the figure's own axes are no longer
/// pixel-row/column-aligned.
///
/// The four [`Self::pixel_corners`] are mapped to the unit square
/// `[0,1]×[0,1]` by a 2D projective transform (a homography — the standard
/// "straighten a photographed rectangle" map, the same class of transform a
/// document scanner's perspective-correction uses). The resulting `(u, v)`
/// then feeds the **same per-axis linear/log value interpolation**
/// [`AxisCalibration`] already implements and this module already tests
/// (`u`/`v` standing in for the pixel position, `0`/`1` standing in for the
/// two reference pixels) — deliberately reusing that math rather than
/// inventing a second one, so log-axis correctness only has to be verified
/// once.
///
/// Corners are ordered `[top_left, top_right, bottom_right, bottom_left]`
/// — `u = 0` at the left corners, `u = 1` at the right corners, `v = 0` at
/// the top corners (smaller row), `v = 1` at the bottom corners (larger
/// row), matching this crate's existing "rows increase downward"
/// convention.
///
/// The homography is **recomputed on every [`Self::point_at`]/
/// [`Self::pixel_at`] call**, not cached — a deliberate simplicity choice:
/// caching would need either interior mutability (fighting `Copy`/`Eq`) or
/// a second field that must stay in sync with `pixel_corners`, and solving
/// an 8×8 linear system is microseconds, cheap enough for the few hundred
/// to few thousand points a single trace touches. Revisit only if profiling
/// ever shows this mattering in practice.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ParallelogramCalibration {
    pub pixel_corners: [PixelPoint; 4],
    pub x_scale: AxisScale,
    /// Data x value at `u = 0` (left edge).
    pub x_value_at_left: f64,
    /// Data x value at `u = 1` (right edge).
    pub x_value_at_right: f64,
    pub y_scale: AxisScale,
    /// Data y value at `v = 0` (top edge).
    pub y_value_at_top: f64,
    /// Data y value at `v = 1` (bottom edge).
    pub y_value_at_bottom: f64,
}

impl ParallelogramCalibration {
    /// Build a validated parallelogram calibration.
    ///
    /// # Errors
    ///
    /// [`DigitiserError::Calibration`] if any corner/value is non-finite, the
    /// x or y reference values coincide, a logarithmic axis has a
    /// non-positive reference value (same rules as [`AxisCalibration::new`]),
    /// or the four corners are degenerate (collinear/repeated, so no
    /// homography exists).
    pub fn new(
        pixel_corners: [PixelPoint; 4],
        x_scale: AxisScale,
        x_value_at_left: f64,
        x_value_at_right: f64,
        y_scale: AxisScale,
        y_value_at_top: f64,
        y_value_at_bottom: f64,
    ) -> Result<Self, DigitiserError> {
        for c in &pixel_corners {
            if !c.x.is_finite() || !c.y.is_finite() {
                return Err(DigitiserError::Calibration(
                    "parallelogram corners must be finite".to_string(),
                ));
            }
        }
        for v in [x_value_at_left, x_value_at_right, y_value_at_top, y_value_at_bottom] {
            if !v.is_finite() {
                return Err(DigitiserError::Calibration(
                    "parallelogram reference values must be finite".to_string(),
                ));
            }
        }
        if x_value_at_left == x_value_at_right {
            return Err(DigitiserError::Calibration(format!(
                "x reference values are both {x_value_at_left}"
            )));
        }
        if y_value_at_top == y_value_at_bottom {
            return Err(DigitiserError::Calibration(format!(
                "y reference values are both {y_value_at_top}"
            )));
        }
        if x_scale == AxisScale::Logarithmic && (x_value_at_left <= 0.0 || x_value_at_right <= 0.0)
        {
            return Err(DigitiserError::Calibration(
                "logarithmic x axis needs strictly positive reference values".to_string(),
            ));
        }
        if y_scale == AxisScale::Logarithmic && (y_value_at_top <= 0.0 || y_value_at_bottom <= 0.0)
        {
            return Err(DigitiserError::Calibration(
                "logarithmic y axis needs strictly positive reference values".to_string(),
            ));
        }
        if homography(&pixel_corners, false).is_none() {
            return Err(DigitiserError::Calibration(
                "parallelogram corners are degenerate (collinear or repeated) — no valid \
                 mapping exists"
                    .to_string(),
            ));
        }
        Ok(Self {
            pixel_corners,
            x_scale,
            x_value_at_left,
            x_value_at_right,
            y_scale,
            y_value_at_top,
            y_value_at_bottom,
        })
    }

    /// The pixel corners' `(u, v)` position in `[0,1]×[0,1]` for pixel
    /// `(x_px, y_px)` — `None` if the corners are degenerate (see
    /// [`Self::new`], which normally prevents this from ever happening for a
    /// validly-constructed value).
    fn uv_at(&self, x_px: f64, y_px: f64) -> Option<(f64, f64)> {
        let h = homography(&self.pixel_corners, false)?;
        apply_homography(&h, x_px, y_px)
    }

    /// The pixel `(column, row)` at unit-square position `(u, v)` — the
    /// inverse of [`Self::uv_at`], via the homography built in the opposite
    /// direction (unit square → pixel corners) rather than inverting the
    /// 3×3 matrix [`Self::uv_at`] uses.
    fn pixel_at_uv(&self, u: f64, v: f64) -> Option<(f64, f64)> {
        let h = homography(&self.pixel_corners, true)?;
        apply_homography(&h, u, v)
    }

    /// Map an image pixel `(column, row)` to data coordinates `(x, y)`.
    /// Returns `(f64::NAN, f64::NAN)` only for a degenerate quad — see
    /// [`PlotCalibration::point_at`]'s doc for why that can only happen from
    /// hand-edited/corrupted JSON, not from anything [`Self::new`] accepts.
    pub fn point_at(&self, x_px: f64, y_px: f64) -> (f64, f64) {
        let Some((u, v)) = self.uv_at(x_px, y_px) else {
            return (f64::NAN, f64::NAN);
        };
        let x = interpolate(self.x_scale, self.x_value_at_left, self.x_value_at_right, u);
        let y = interpolate(self.y_scale, self.y_value_at_top, self.y_value_at_bottom, v);
        (x, y)
    }

    /// The inverse of [`Self::point_at`]: the pixel `(column, row)` data
    /// value `(x, y)` would sit at. `None` when the value cannot be placed
    /// (non-finite input, non-positive value on a logarithmic axis, or a
    /// degenerate quad).
    pub fn pixel_at(&self, x: f64, y: f64) -> Option<(f64, f64)> {
        let u = interpolate_inverse(self.x_scale, self.x_value_at_left, self.x_value_at_right, x)?;
        let v = interpolate_inverse(self.y_scale, self.y_value_at_top, self.y_value_at_bottom, y)?;
        self.pixel_at_uv(u, v)
    }
}

/// Linear/log interpolation shared by [`AxisCalibration::value_at`] and
/// [`ParallelogramCalibration::point_at`] — `t=0` gives `v0`, `t=1` gives
/// `v1`, matching [`AxisCalibration`]'s own `r1`/`r2` convention with the
/// reference pixels fixed at `0`/`1`.
fn interpolate(scale: AxisScale, v0: f64, v1: f64, t: f64) -> f64 {
    match scale {
        AxisScale::Linear => v0 + t * (v1 - v0),
        AxisScale::Logarithmic => {
            let l0 = v0.log10();
            let l1 = v1.log10();
            10f64.powf(l0 + t * (l1 - l0))
        }
    }
}

/// Inverse of [`interpolate`] — `None` for a non-finite or (on a log scale)
/// non-positive `value`, matching [`AxisCalibration::pixel_at`]'s rules.
fn interpolate_inverse(scale: AxisScale, v0: f64, v1: f64, value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    match scale {
        AxisScale::Linear => Some((value - v0) / (v1 - v0)),
        AxisScale::Logarithmic => {
            if value <= 0.0 {
                return None;
            }
            let l0 = v0.log10();
            let l1 = v1.log10();
            Some((value.log10() - l0) / (l1 - l0))
        }
    }
}

/// Solve an 8×8 linear system `a·h = b` via Gauss-Jordan elimination with
/// partial pivoting. `None` if `a` is singular (within a small tolerance).
fn solve8(mut a: [[f64; 8]; 8], mut b: [f64; 8]) -> Option<[f64; 8]> {
    for col in 0..8 {
        let mut pivot_row = col;
        let mut pivot_val = a[col][col].abs();
        for row in (col + 1)..8 {
            if a[row][col].abs() > pivot_val {
                pivot_val = a[row][col].abs();
                pivot_row = row;
            }
        }
        if pivot_val < 1e-9 {
            return None;
        }
        a.swap(col, pivot_row);
        b.swap(col, pivot_row);
        let d = a[col][col];
        for j in col..8 {
            a[col][j] /= d;
        }
        b[col] /= d;
        for row in 0..8 {
            if row == col {
                continue;
            }
            let f = a[row][col];
            if f != 0.0 {
                for j in col..8 {
                    a[row][j] -= f * a[col][j];
                }
                b[row] -= f * b[col];
            }
        }
    }
    Some(b)
}

/// Build the 8 free coefficients (`h22` fixed at `1`) of the homography
/// mapping the unit-square corners `(0,0),(1,0),(1,1),(0,1)` to
/// `pixel_corners` (`reverse = false`, used by [`ParallelogramCalibration::uv_at`]
/// after solving for the OPPOSITE direction — source = pixel corners, target
/// = unit square, so applying it maps a pixel straight to `(u, v)`) or the
/// reverse direction (`reverse = true`, source = unit square, target = pixel
/// corners, used by [`ParallelogramCalibration::pixel_at_uv`]).
///
/// `None` if the four corners are degenerate (collinear or repeated — the
/// 8×8 system is singular).
fn homography(pixel_corners: &[PixelPoint; 4], reverse: bool) -> Option<[f64; 8]> {
    let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let mut a = [[0.0; 8]; 8];
    let mut b = [0.0; 8];
    for i in 0..4 {
        let (px, py) = (pixel_corners[i].x, pixel_corners[i].y);
        let (u, v) = uv[i];
        // `uv_at` solves pixel -> (u,v): source = pixel, target = (u,v).
        // `pixel_at_uv` solves (u,v) -> pixel: source = (u,v), target = pixel.
        let (sx, sy, tx, ty) = if reverse { (u, v, px, py) } else { (px, py, u, v) };
        let r0 = 2 * i;
        let r1 = 2 * i + 1;
        a[r0] = [sx, sy, 1.0, 0.0, 0.0, 0.0, -tx * sx, -tx * sy];
        b[r0] = tx;
        a[r1] = [0.0, 0.0, 0.0, sx, sy, 1.0, -ty * sx, -ty * sy];
        b[r1] = ty;
    }
    solve8(a, b)
}

/// Apply a homography built by [`homography`] to `(sx, sy)`, returning the
/// mapped `(tx, ty)` — `None` if the perspective denominator is (near) zero.
fn apply_homography(h: &[f64; 8], sx: f64, sy: f64) -> Option<(f64, f64)> {
    let denom = h[6] * sx + h[7] * sy + 1.0;
    if denom.abs() < 1e-9 {
        return None;
    }
    let tx = (h[0] * sx + h[1] * sy + h[2]) / denom;
    let ty = (h[3] * sx + h[4] * sy + h[5]) / denom;
    Some((tx, ty))
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

    // --- PlotCalibration enum / backward compatibility (op-vyb9) ---

    fn corner(x: f64, y: f64) -> PixelPoint {
        PixelPoint { x, y }
    }

    #[test]
    fn old_flat_json_still_deserialises_as_axis_aligned() {
        // Exactly the shape a pre-enum `PlotCalibration { x, y }` produced.
        let json = r#"{
            "x": {"scale":"Linear","r1":{"pixel":100.0,"value":0.0},"r2":{"pixel":600.0,"value":10.0}},
            "y": {"scale":"Linear","r1":{"pixel":500.0,"value":0.0},"r2":{"pixel":100.0,"value":40.0}}
        }"#;
        let cal: PlotCalibration = serde_json::from_str(json).unwrap();
        assert!(matches!(cal, PlotCalibration::AxisAligned { .. }));
        assert_eq!(cal.point_at(350.0, 300.0), (5.0, 20.0));
    }

    #[test]
    fn axis_aligned_serialises_to_the_old_flat_shape_byte_for_byte() {
        let cal = PlotCalibration::AxisAligned { x: lin(0.0, 0.0, 100.0, 10.0), y: lin(0.0, 0.0, 100.0, 10.0) };
        let json = serde_json::to_value(&cal).unwrap();
        // No "AxisAligned" tag key anywhere -- just the flat {x, y} object.
        let obj = json.as_object().unwrap();
        assert_eq!(obj.len(), 2);
        assert!(obj.contains_key("x"));
        assert!(obj.contains_key("y"));
        assert!(!obj.contains_key("AxisAligned"));
    }

    fn parallelogram(corners: [PixelPoint; 4]) -> ParallelogramCalibration {
        ParallelogramCalibration::new(
            corners,
            AxisScale::Linear,
            0.0,
            10.0,
            AxisScale::Linear,
            40.0,
            0.0,
        )
        .unwrap()
    }

    #[test]
    fn parallelogram_json_round_trips_and_is_distinguishable_from_axis_aligned() {
        let cal = PlotCalibration::Parallelogram(parallelogram([
            corner(0.0, 0.0),
            corner(100.0, 0.0),
            corner(100.0, 100.0),
            corner(0.0, 100.0),
        ]));
        let json = serde_json::to_string(&cal).unwrap();
        assert!(!json.contains("\"x\":") || json.contains("pixel_corners")); // sanity: not the flat shape
        let back: PlotCalibration = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cal);
        assert!(matches!(back, PlotCalibration::Parallelogram(_)));
    }

    #[test]
    fn parallelogram_over_an_actual_rectangle_matches_axis_aligned_linear() {
        // A pixel-space quad that IS a plain axis-aligned rectangle should
        // reproduce exactly what AxisCalibration::value_at already gives —
        // the sanity check that the homography degenerates correctly.
        let cal = parallelogram([corner(100.0, 100.0), corner(600.0, 100.0), corner(600.0, 500.0), corner(100.0, 500.0)]);
        let x_axis = lin(100.0, 0.0, 600.0, 10.0);
        let y_axis = lin(500.0, 0.0, 100.0, 40.0); // v=1 at bottom(row 500)=y_at_bottom=0
        for (px, py) in [(100.0, 100.0), (600.0, 500.0), (350.0, 300.0), (250.0, 450.0)] {
            let (x, y) = cal.point_at(px, py);
            assert!((x - x_axis.value_at(px)).abs() < 1e-9, "x at ({px},{py}): got {x}");
            assert!((y - y_axis.value_at(py)).abs() < 1e-9, "y at ({px},{py}): got {y}");
        }
    }

    #[test]
    fn parallelogram_corners_map_to_their_own_reference_values() {
        let corners =
            [corner(50.0, 20.0), corner(400.0, 60.0), corner(370.0, 480.0), corner(30.0, 430.0)];
        let cal = parallelogram(corners);
        // top-left -> (x_left, y_top); top-right -> (x_right, y_top);
        // bottom-right -> (x_right, y_bottom); bottom-left -> (x_left, y_bottom).
        let expected = [(0.0, 40.0), (10.0, 40.0), (10.0, 0.0), (0.0, 0.0)];
        for (c, exp) in corners.iter().zip(expected) {
            let (x, y) = cal.point_at(c.x, c.y);
            assert!((x - exp.0).abs() < 1e-6, "x: got {x} want {}", exp.0);
            assert!((y - exp.1).abs() < 1e-6, "y: got {y} want {}", exp.1);
        }
    }

    #[test]
    fn parallelogram_pixel_at_is_the_inverse_of_point_at() {
        let cal = parallelogram([
            corner(50.0, 20.0),
            corner(400.0, 60.0),
            corner(370.0, 480.0),
            corner(30.0, 430.0),
        ]);
        for (px, py) in [(200.0, 250.0), (100.0, 100.0), (300.0, 400.0)] {
            let (x, y) = cal.point_at(px, py);
            let (back_px, back_py) = cal.pixel_at(x, y).unwrap();
            assert!((back_px - px).abs() < 1e-6, "px {px} -> {x} -> {back_px}");
            assert!((back_py - py).abs() < 1e-6, "py {py} -> {y} -> {back_py}");
        }
    }

    #[test]
    fn parallelogram_with_logarithmic_axis_matches_the_geometric_mean() {
        let cal = ParallelogramCalibration::new(
            [corner(0.0, 0.0), corner(200.0, 0.0), corner(200.0, 100.0), corner(0.0, 100.0)],
            AxisScale::Logarithmic,
            1.0,
            100.0,
            AxisScale::Linear,
            0.0,
            1.0,
        )
        .unwrap();
        let (x, _) = cal.point_at(100.0, 0.0); // u = 0.5
        assert!((x - 10.0).abs() < 1e-9, "got {x}");
    }

    #[test]
    fn degenerate_parallelogram_corners_are_rejected() {
        // All four corners identical.
        assert!(ParallelogramCalibration::new(
            [corner(0.0, 0.0); 4],
            AxisScale::Linear,
            0.0,
            10.0,
            AxisScale::Linear,
            0.0,
            10.0,
        )
        .is_err());
        // Three collinear corners (degenerate quad).
        assert!(ParallelogramCalibration::new(
            [corner(0.0, 0.0), corner(50.0, 0.0), corner(100.0, 0.0), corner(0.0, 100.0)],
            AxisScale::Linear,
            0.0,
            10.0,
            AxisScale::Linear,
            0.0,
            10.0,
        )
        .is_err());
    }

    #[test]
    fn parallelogram_rejects_the_same_invalid_inputs_as_axis_calibration() {
        let c = |x, y| corner(x, y);
        let corners = [c(0.0, 0.0), c(100.0, 0.0), c(100.0, 100.0), c(0.0, 100.0)];
        assert!(ParallelogramCalibration::new(corners, AxisScale::Linear, 3.0, 3.0, AxisScale::Linear, 0.0, 1.0).is_err());
        assert!(ParallelogramCalibration::new(corners, AxisScale::Logarithmic, 0.0, 1.0, AxisScale::Linear, 0.0, 1.0).is_err());
        assert!(ParallelogramCalibration::new(corners, AxisScale::Linear, f64::NAN, 1.0, AxisScale::Linear, 0.0, 1.0).is_err());
    }

    #[test]
    fn plot_calibration_pixel_at_dispatches_to_the_right_variant() {
        let axis_aligned =
            PlotCalibration::AxisAligned { x: lin(0.0, 0.0, 100.0, 10.0), y: lin(0.0, 0.0, 100.0, 10.0) };
        assert_eq!(axis_aligned.pixel_at(5.0, 5.0), Some((50.0, 50.0)));

        let para = PlotCalibration::Parallelogram(parallelogram([
            corner(0.0, 0.0),
            corner(100.0, 0.0),
            corner(100.0, 100.0),
            corner(0.0, 100.0),
        ]));
        let (x, y) = para.point_at(50.0, 50.0);
        let (px, py) = para.pixel_at(x, y).unwrap();
        assert!((px - 50.0).abs() < 1e-6 && (py - 50.0).abs() < 1e-6);
    }
}
