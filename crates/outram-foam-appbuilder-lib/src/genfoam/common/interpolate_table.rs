// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/common/InterpolateTable/
//                    InterpolateTableGF.H, InterpolateTablesGF.{C,H}
//   Upstream commit: 652b3da (GeN-Foam), built on OpenFOAM v2506
//   Upstream copyright: (C) 2021 OpenFOAM Foundation / (C) 2015-2022 EPFL
//     Principal author: I. Clifford (PSI); contributions A. Scolaro, E. Brunetto,
//     C. Fiorina (EPFL); out-of-bounds modes F. Xiang (EPFL & XJTU)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # 1-D interpolation table with selectable method and out-of-bounds policy
//!
//! [`ScalarInterpolateTable`] is a 1-D lookup table over strictly-ascending
//! abscissae `x` returning a scalar `y` — GeN-Foam's `scalarInterpolateTableGF`
//! (the `InterpolateTableGF<scalarField, scalar, scalar>` instantiation). It is
//! richer than [`interpolate_xy`](fn@outram_foam_basic_lib::interpolation::interpolate_xy),
//! which offers only linear interpolation with endpoint clamping, in two ways:
//!
//! 1. **Interpolation method** — [`InterpolationMethod::Linear`] *or*
//!    [`InterpolationMethod::Step`] (each tabulated value holds until the next
//!    knot; a staircase, used e.g. for banded/tabulated cross sections).
//! 2. **Out-of-bounds policy** — [`OutOfBounds::Error`] (reject),
//!    [`OutOfBounds::Fixed`] (clamp to the endpoint, the `interpolate_xy`
//!    behaviour), or [`OutOfBounds::Extrapolation`] (linear extrapolation from
//!    the two nearest points).
//!
//! It also provides an [`integral`](ScalarInterpolateTable::integral) with a
//! coordinate-power weight (cartesian/cylindrical/spherical) and the table-walk
//! helpers [`next_point`](ScalarInterpolateTable::next_point) /
//! [`last_point`](ScalarInterpolateTable::last_point).
//!
//! ## Dimension convention
//!
//! Both axes are **raw `f64`**, because this is a generic numeric utility whose
//! axes carry different physics per use site (cross section vs temperature,
//! power vs radius, …); pinning a single `uom` unit would be physically wrong.
//! The consumer attaches units at its own boundary. (This matches the upstream
//! `scalar`-typed table and mirrors the same decision made for
//! [`TimeProfile`](super::time_profile::TimeProfile)'s ordinate.)
//!
//! ## Error handling — no panics
//!
//! Faithful to the workspace rule against papering over non-convergence, every
//! fallible query returns a [`Result`]; nothing panics on bad input. Upstream
//! `FatalError`s (out-of-bounds under [`OutOfBounds::Error`], malformed tables)
//! become [`InterpolateTableError`] values the caller must handle.
//!
//! ## Example
//!
//! ```
//! use outram_foam_appbuilder_lib::genfoam::common::interpolate_table::{
//!     InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
//! };
//!
//! let table = ScalarInterpolateTable::new(
//!     vec![0.0, 1.0, 2.0],
//!     vec![0.0, 10.0, 30.0],
//!     InterpolationMethod::Linear,
//!     OutOfBounds::Fixed,
//! )
//! .unwrap();
//!
//! assert_eq!(table.interpolate(0.5).unwrap(), 5.0);   // linear between knots
//! assert_eq!(table.interpolate(-1.0).unwrap(), 0.0);  // clamped to first value
//! assert_eq!(table.interpolate(9.0).unwrap(), 30.0);  // clamped to last value
//! ```

/// How a [`ScalarInterpolateTable`] interpolates between tabulated knots.
///
/// Maps to GeN-Foam's `InterpolateTableBaseGF::interpolationMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterpolationMethod {
    /// Each tabulated value applies from its knot until the next knot — a
    /// staircase (upstream `STEP`). For `x` in `[x[i-1], x[i])` the value is
    /// `y[i-1]`.
    Step,
    /// Linear interpolation between adjacent knots (upstream `LINEAR`).
    Linear,
}

/// What a [`ScalarInterpolateTable`] does when queried outside its tabulated
/// range `[x[0], x[n-1]]`.
///
/// Maps to GeN-Foam's `InterpolateTableBaseGF::outofBoundsMehtod` (sic).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutOfBounds {
    /// Reject the query — [`ScalarInterpolateTable::interpolate`] returns
    /// [`InterpolateTableError::OutOfBounds`] (upstream `ERROR`, a `FatalError`).
    Error,
    /// Linearly extrapolate using the two nearest tabulated points
    /// (upstream `EXTRAPOLATION`).
    Extrapolation,
    /// Clamp to the nearest endpoint value (upstream `FIXED`; the same behaviour
    /// as [`interpolate_xy`](fn@outram_foam_basic_lib::interpolation::interpolate_xy)).
    Fixed,
}

/// Errors from building or querying a [`ScalarInterpolateTable`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InterpolateTableError {
    /// The `x` and `y` vectors had different lengths.
    #[error("interpolation table has mismatched lengths: {xs} ordinates vs {ys} values")]
    LengthMismatch {
        /// Number of abscissae supplied.
        xs: usize,
        /// Number of ordinate values supplied.
        ys: usize,
    },

    /// A table needs at least two points to interpolate between.
    #[error("interpolation table needs at least 2 points, got {0}")]
    TooFewPoints(usize),

    /// The abscissa column was not strictly ascending. The reported index `i` is
    /// the first offender (`x[i] <= x[i-1]`).
    #[error(
        "interpolation table abscissae not strictly ascending at index {index}: {prev} then {next}"
    )]
    NonAscendingAbscissae {
        /// Index of the first non-ascending entry.
        index: usize,
        /// The preceding abscissa.
        prev: f64,
        /// The offending abscissa.
        next: f64,
    },

    /// A query fell outside `[x[0], x[n-1]]` under the [`OutOfBounds::Error`]
    /// policy.
    #[error("query x = {x} out of table bounds [{lo}, {hi}]")]
    OutOfBounds {
        /// The queried abscissa.
        x: f64,
        /// The table's lower bound `x[0]`.
        lo: f64,
        /// The table's upper bound `x[n-1]`.
        hi: f64,
    },
}

/// A 1-D interpolation table over strictly-ascending abscissae.
///
/// See the [module documentation](self) for the interpolation methods,
/// out-of-bounds policies, dimension convention (both axes are raw `f64`), and
/// the no-panic error policy. Build with [`new`](Self::new); query with
/// [`interpolate`](Self::interpolate).
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarInterpolateTable {
    /// Strictly-ascending abscissae, length ≥ 2.
    xs: Vec<f64>,
    /// Ordinate values, one per abscissa.
    ys: Vec<f64>,
    method: InterpolationMethod,
    out_of_bounds: OutOfBounds,
}

impl ScalarInterpolateTable {
    /// Build a table from tabulated `(xs[i], ys[i])` points.
    ///
    /// # Parameters
    /// - `xs` — strictly-ascending abscissae, length ≥ 2.
    /// - `ys` — ordinate values, one per abscissa (same length as `xs`).
    /// - `method` — [`InterpolationMethod::Linear`] or [`InterpolationMethod::Step`].
    /// - `out_of_bounds` — policy for queries outside `[xs[0], xs[n-1]]`.
    ///
    /// # Errors
    /// - [`InterpolateTableError::LengthMismatch`] if the vectors differ in length.
    /// - [`InterpolateTableError::TooFewPoints`] if fewer than two points.
    /// - [`InterpolateTableError::NonAscendingAbscissae`] if `xs` is not strictly
    ///   increasing.
    pub fn new(
        xs: Vec<f64>,
        ys: Vec<f64>,
        method: InterpolationMethod,
        out_of_bounds: OutOfBounds,
    ) -> Result<Self, InterpolateTableError> {
        if xs.len() != ys.len() {
            return Err(InterpolateTableError::LengthMismatch {
                xs: xs.len(),
                ys: ys.len(),
            });
        }
        if xs.len() < 2 {
            return Err(InterpolateTableError::TooFewPoints(xs.len()));
        }
        for i in 1..xs.len() {
            if xs[i] <= xs[i - 1] {
                return Err(InterpolateTableError::NonAscendingAbscissae {
                    index: i,
                    prev: xs[i - 1],
                    next: xs[i],
                });
            }
        }
        Ok(Self {
            xs,
            ys,
            method,
            out_of_bounds,
        })
    }

    /// The configured interpolation method.
    pub fn method(&self) -> InterpolationMethod {
        self.method
    }

    /// The configured out-of-bounds policy.
    pub fn out_of_bounds(&self) -> OutOfBounds {
        self.out_of_bounds
    }

    /// Interpolate the table at `x`.
    ///
    /// Inside `[xs[0], xs[n-1]]` the value follows the configured
    /// [`InterpolationMethod`]. Outside it, the configured [`OutOfBounds`] policy
    /// applies: [`Fixed`](OutOfBounds::Fixed) clamps, [`Extrapolation`](OutOfBounds::Extrapolation)
    /// extends linearly from the two nearest points, and [`Error`](OutOfBounds::Error)
    /// yields [`InterpolateTableError::OutOfBounds`].
    ///
    /// # Errors
    /// [`InterpolateTableError::OutOfBounds`] only, and only under
    /// [`OutOfBounds::Error`]. The other policies never fail.
    pub fn interpolate(&self, x: f64) -> Result<f64, InterpolateTableError> {
        let n = self.xs.len();

        // In-range: locate the bracketing interval [xs[i-1], xs[i]] with xs[0] <= x <= xs[n-1].
        if x >= self.xs[0] && x <= self.xs[n - 1] {
            // First i with x <= xs[i]; guaranteed to exist and be >= 1 here.
            let i = self.xs.partition_point(|&xi| xi < x).max(1);
            return Ok(match self.method {
                InterpolationMethod::Step => self.ys[i - 1],
                InterpolationMethod::Linear => {
                    let x0 = self.xs[i - 1];
                    let x1 = self.xs[i];
                    let alpha = (x - x0) / (x1 - x0);
                    (1.0 - alpha) * self.ys[i - 1] + alpha * self.ys[i]
                }
            });
        }

        // Out of range.
        let below = x < self.xs[0];
        match self.out_of_bounds {
            OutOfBounds::Error => Err(InterpolateTableError::OutOfBounds {
                x,
                lo: self.xs[0],
                hi: self.xs[n - 1],
            }),
            OutOfBounds::Fixed => Ok(if below { self.ys[0] } else { self.ys[n - 1] }),
            OutOfBounds::Extrapolation => {
                let (i0, i1) = if below { (0, 1) } else { (n - 2, n - 1) };
                let slope = (self.ys[i1] - self.ys[i0]) / (self.xs[i1] - self.xs[i0]);
                Ok(slope * (x - self.xs[i0]) + self.ys[i0])
            }
        }
    }

    /// The integral of the tabulated data, `∫ y · d(x^k)`, with coordinate power
    /// `k` selecting the geometry: `k = 1` cartesian (`∫ y dx`), `k = 2`
    /// cylindrical (weight `r`, i.e. `∫ y d(r²)`), `k = 3` spherical
    /// (`∫ y d(r³)`).
    ///
    /// Uses rectangle summation for [`InterpolationMethod::Step`] and the
    /// trapezoidal rule for [`InterpolationMethod::Linear`], each over the
    /// `x^k`-transformed abscissae. This is the ported
    /// `scalarInterpolateTableGF::integral(scalar k)`; upstream `NotImplemented`
    /// for the non-scalar (field) instantiations.
    pub fn integral(&self, k: f64) -> f64 {
        let mut res = 0.0;
        for i in 1..self.xs.len() {
            let dx = self.xs[i].powf(k) - self.xs[i - 1].powf(k);
            match self.method {
                InterpolationMethod::Step => res += self.ys[i - 1] * dx,
                InterpolationMethod::Linear => res += 0.5 * (self.ys[i - 1] + self.ys[i]) * dx,
            }
        }
        res
    }

    /// The next tabulated abscissa strictly greater than `x` (the upstream
    /// `getNextPoint`). Useful for stepping a solver exactly onto the next table
    /// break.
    ///
    /// # Errors
    /// [`InterpolateTableError::OutOfBounds`] if `x` is below `xs[0]` or at/above
    /// the last abscissa (no strictly-greater point exists).
    pub fn next_point(&self, x: f64) -> Result<f64, InterpolateTableError> {
        let n = self.xs.len();
        if x >= self.xs[0] {
            for i in 1..n {
                if x < self.xs[i] {
                    return Ok(self.xs[i]);
                }
            }
        }
        Err(InterpolateTableError::OutOfBounds {
            x,
            lo: self.xs[0],
            hi: self.xs[n - 1],
        })
    }

    /// The last (largest) tabulated abscissa `xs[n-1]` (upstream `getLastPoint`).
    pub fn last_point(&self) -> f64 {
        self.xs[self.xs.len() - 1]
    }

    /// The first (smallest) tabulated abscissa `xs[0]`.
    pub fn first_point(&self) -> f64 {
        self.xs[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linear_fixed(xs: Vec<f64>, ys: Vec<f64>) -> ScalarInterpolateTable {
        ScalarInterpolateTable::new(xs, ys, InterpolationMethod::Linear, OutOfBounds::Fixed)
            .unwrap()
    }

    #[test]
    fn linear_between_knots() {
        let t = linear_fixed(vec![0.0, 1.0, 2.0], vec![0.0, 10.0, 30.0]);
        assert_eq!(t.interpolate(0.0).unwrap(), 0.0);
        assert_eq!(t.interpolate(0.5).unwrap(), 5.0);
        assert_eq!(t.interpolate(1.0).unwrap(), 10.0);
        assert_eq!(t.interpolate(1.5).unwrap(), 20.0);
        assert_eq!(t.interpolate(2.0).unwrap(), 30.0);
    }

    #[test]
    fn step_holds_previous_value() {
        let t = ScalarInterpolateTable::new(
            vec![0.0, 1.0, 2.0],
            vec![5.0, 15.0, 25.0],
            InterpolationMethod::Step,
            OutOfBounds::Fixed,
        )
        .unwrap();
        // x in [x[i-1], x[i]] returns y[i-1]; at the exact upper knot the bracket
        // is [1,2] so it returns y at index 1.
        assert_eq!(t.interpolate(0.0).unwrap(), 5.0);
        assert_eq!(t.interpolate(0.99).unwrap(), 5.0);
        assert_eq!(t.interpolate(1.0).unwrap(), 5.0);
        assert_eq!(t.interpolate(1.5).unwrap(), 15.0);
        assert_eq!(t.interpolate(2.0).unwrap(), 15.0);
    }

    #[test]
    fn fixed_clamps() {
        let t = linear_fixed(vec![1.0, 3.0], vec![10.0, 30.0]);
        assert_eq!(t.interpolate(-5.0).unwrap(), 10.0);
        assert_eq!(t.interpolate(100.0).unwrap(), 30.0);
    }

    #[test]
    fn error_policy_rejects_out_of_range() {
        let t = ScalarInterpolateTable::new(
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            InterpolationMethod::Linear,
            OutOfBounds::Error,
        )
        .unwrap();
        assert_eq!(t.interpolate(0.5).unwrap(), 0.5);
        assert_eq!(
            t.interpolate(2.0).unwrap_err(),
            InterpolateTableError::OutOfBounds {
                x: 2.0,
                lo: 0.0,
                hi: 1.0
            }
        );
        assert!(t.interpolate(-0.1).is_err());
    }

    #[test]
    fn extrapolation_extends_linearly() {
        let t = ScalarInterpolateTable::new(
            vec![0.0, 1.0, 2.0],
            vec![0.0, 10.0, 20.0],
            InterpolationMethod::Linear,
            OutOfBounds::Extrapolation,
        )
        .unwrap();
        // slope 10 per unit both ends
        assert_eq!(t.interpolate(-1.0).unwrap(), -10.0);
        assert_eq!(t.interpolate(3.0).unwrap(), 30.0);
    }

    #[test]
    fn integral_cartesian_trapezoid() {
        // y = 2x over [0,2]: exact area under line = 4.
        let t = linear_fixed(vec![0.0, 1.0, 2.0], vec![0.0, 2.0, 4.0]);
        assert!((t.integral(1.0) - 4.0).abs() < 1e-12);
    }

    #[test]
    fn integral_step_rectangles() {
        // Step values 5 on [0,1], 15 on [1,2] -> 5*1 + 15*1 = 20.
        let t = ScalarInterpolateTable::new(
            vec![0.0, 1.0, 2.0],
            vec![5.0, 15.0, 25.0],
            InterpolationMethod::Step,
            OutOfBounds::Fixed,
        )
        .unwrap();
        assert!((t.integral(1.0) - 20.0).abs() < 1e-12);
    }

    #[test]
    fn next_and_last_point() {
        let t = linear_fixed(vec![0.0, 1.0, 2.0], vec![0.0, 1.0, 2.0]);
        assert_eq!(t.next_point(0.5).unwrap(), 1.0);
        assert_eq!(t.next_point(1.0).unwrap(), 2.0);
        assert!(t.next_point(2.0).is_err());
        assert_eq!(t.last_point(), 2.0);
        assert_eq!(t.first_point(), 0.0);
    }

    #[test]
    fn build_validation() {
        assert_eq!(
            ScalarInterpolateTable::new(
                vec![0.0],
                vec![0.0],
                InterpolationMethod::Linear,
                OutOfBounds::Fixed
            )
            .unwrap_err(),
            InterpolateTableError::TooFewPoints(1)
        );
        assert_eq!(
            ScalarInterpolateTable::new(
                vec![0.0, 1.0],
                vec![0.0],
                InterpolationMethod::Linear,
                OutOfBounds::Fixed
            )
            .unwrap_err(),
            InterpolateTableError::LengthMismatch { xs: 2, ys: 1 }
        );
        assert_eq!(
            ScalarInterpolateTable::new(
                vec![0.0, 0.0],
                vec![0.0, 1.0],
                InterpolationMethod::Linear,
                OutOfBounds::Fixed
            )
            .unwrap_err(),
            InterpolateTableError::NonAscendingAbscissae {
                index: 1,
                prev: 0.0,
                next: 0.0
            }
        );
    }
}
