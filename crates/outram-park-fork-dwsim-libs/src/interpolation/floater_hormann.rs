//! Floater-Hormann barycentric rational interpolation.
//!
//! Ported (as a general numerical method, not DWSIM-specific code) to
//! replace DWSIM `UnitOperations/Pump.vb`/`Expander.vb`'s `Curves`
//! calculation mode, which interpolates manufacturer performance data
//! (head/efficiency/NPSHr/power vs. flow) using this same method via
//! `ratinterpolation.buildfloaterhormannrationalinterpolant`/
//! `polinterpolation.barycentricinterpolation` (DWSIM vendors ALGLIB for
//! this; ALGLIB's own source was not consulted -- see the citations below
//! instead).
//!
//! # Primary reference
//!
//! Floater, M. S., & Hormann, K. (2007). Barycentric rational interpolation
//! with no poles and high rates of approximation. *Numerische Mathematik*,
//! 107(2), 315-331. <https://doi.org/10.1007/s00211-007-0093-y>
//!
//! # Formula cross-checked against
//!
//! The exact indexing convention implemented here (weight sign, summation
//! range) was cross-checked against SciPy's documentation for
//! `scipy.interpolate.FloaterHormannInterpolator` (SciPy is BSD-3-Clause
//! licensed; NumPy, which SciPy depends on, is also BSD-3-Clause -- no
//! SciPy/NumPy source code is copied here, only the openly-documented
//! mathematical formula, reproduced independently in Rust):
//! <https://docs.scipy.org/doc/scipy/reference/generated/scipy.interpolate.FloaterHormannInterpolator.html>
//!
//! ```text
//! w_k = (-1)^(k-d) * sum_{i in J_k} prod_{j=i, j != k}^{i+d} 1/|x_k - x_j|
//! J_k = { i in I : k-d <= i <= k },  I = {0, 1, ..., n-d}
//!
//! r(x) = ( sum_k w_k y_k / (x - x_k) ) / ( sum_k w_k / (x - x_k) )
//! ```
//! where `n` is the number of data points minus one (points indexed
//! `0..=n`) and `d` (`0 <= d < n`) is the blended-polynomial degree
//! (higher `d` gives smoother, higher-order interpolation; `d=3`-`4` is a
//! common default, matching ALGLIB's/DWSIM's typical usage).

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Errors constructing or evaluating a [`FloaterHormannInterpolant`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FloaterHormannError {
    /// Fewer than 2 data points were supplied.
    #[error("Floater-Hormann interpolation needs at least 2 points, got {count}")]
    TooFewPoints {
        /// Number of points actually supplied.
        count: usize,
    },
    /// `degree` did not satisfy `0 <= degree < n` (`n = points.len() - 1`).
    #[error("Floater-Hormann degree {degree} must satisfy 0 <= degree < {n} (n = points - 1)")]
    DegreeOutOfRange {
        /// The invalid degree that was supplied.
        degree: usize,
        /// The number of points minus one.
        n: usize,
    },
}

/// A Floater-Hormann rational interpolant through a fixed set of `(x, y)`
/// data points -- no real poles, reproduces polynomials of degree `d`
/// exactly, well-conditioned for tabulated manufacturer performance data
/// (pump/turbine head, efficiency, NPSHr, power vs. flow).
#[derive(Debug, Clone, PartialEq)]
pub struct FloaterHormannInterpolant {
    x: Vec<f64>,
    y: Vec<f64>,
    weights: Vec<f64>,
}

impl FloaterHormannInterpolant {
    /// Build the interpolant from data points `(x_i, y_i)`, blending
    /// polynomials of degree `degree` (`0 <= degree <= points.len() - 1`;
    /// `degree = points.len() - 1` degenerates to plain polynomial
    /// interpolation through all points; `degree = 3` is a reasonable
    /// default for smooth curves, matching common practice). `points` need
    /// not be pre-sorted by `x`, but must not contain duplicate `x` values.
    pub fn new(points: &[(f64, f64)], degree: usize) -> Result<Self, FloaterHormannError> {
        let n_points = points.len();
        if n_points < 2 {
            return Err(FloaterHormannError::TooFewPoints { count: n_points });
        }
        let n = n_points - 1;
        if degree > n {
            return Err(FloaterHormannError::DegreeOutOfRange { degree, n });
        }

        let mut sorted: Vec<(f64, f64)> = points.to_vec();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let x: Vec<f64> = sorted.iter().map(|p| p.0).collect();
        let y: Vec<f64> = sorted.iter().map(|p| p.1).collect();

        let d = degree;
        let mut weights = vec![0.0; n_points];
        for k in 0..n_points {
            let i_lo = k.saturating_sub(d);
            let i_hi = k.min(n - d);
            let mut sum = 0.0;
            for i in i_lo..=i_hi {
                let mut prod = 1.0;
                for j in i..=(i + d) {
                    if j != k {
                        prod *= 1.0 / (x[k] - x[j]).abs();
                    }
                }
                sum += prod;
            }
            let sign = if (k as isize - d as isize).rem_euclid(2) == 0 { 1.0 } else { -1.0 };
            weights[k] = sign * sum;
        }

        Ok(Self { x, y, weights })
    }

    /// Evaluate the interpolant at `x`. Returns the exact data value if `x`
    /// coincides with a data point (avoiding the `0/0` removable
    /// singularity in the barycentric formula).
    pub fn evaluate(&self, x: f64) -> f64 {
        if let Some(idx) = self.x.iter().position(|&xi| (xi - x).abs() < 1e-12) {
            return self.y[idx];
        }
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for k in 0..self.x.len() {
            let t = self.weights[k] / (x - self.x[k]);
            numerator += t * self.y[k];
            denominator += t;
        }
        numerator / denominator
    }

    /// [`Self::evaluate`], with `uom`-typed dimensionless input/output --
    /// convenience for callers whose `x`/`y` are already in consistent SI
    /// units expressed as [`Ratio`] (e.g. a normalised flow fraction).
    pub fn evaluate_ratio(&self, x: Ratio) -> Ratio {
        Ratio::new::<ratio>(self.evaluate(x.get::<ratio>()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduces_linear_data_exactly() {
        let points: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, 2.0 * i as f64 + 1.0)).collect();
        let interpolant = FloaterHormannInterpolant::new(&points, 1).unwrap();
        for x in [0.5, 2.3, 5.7, 8.9] {
            let expected = 2.0 * x + 1.0;
            assert!((interpolant.evaluate(x) - expected).abs() < 1e-9, "x={x}");
        }
    }

    #[test]
    fn passes_through_data_points_exactly() {
        let points = vec![(0.0, 1.0), (1.0, 4.0), (2.0, 9.0), (3.0, 16.0), (4.0, 25.0)];
        let interpolant = FloaterHormannInterpolant::new(&points, 2).unwrap();
        for &(x, y) in &points {
            assert!((interpolant.evaluate(x) - y).abs() < 1e-9);
        }
    }

    #[test]
    fn interpolates_reasonably_between_points() {
        // Pump-like head-vs-flow curve: monotonically decreasing.
        let points = vec![
            (0.0, 50.0),
            (10.0, 48.0),
            (20.0, 43.0),
            (30.0, 35.0),
            (40.0, 24.0),
            (50.0, 10.0),
        ];
        let interpolant = FloaterHormannInterpolant::new(&points, 3).unwrap();
        let mid = interpolant.evaluate(25.0);
        assert!(mid < 43.0 && mid > 35.0);
    }

    #[test]
    fn rejects_too_few_points() {
        let points = vec![(0.0, 1.0)];
        assert_eq!(
            FloaterHormannInterpolant::new(&points, 0),
            Err(FloaterHormannError::TooFewPoints { count: 1 })
        );
    }

    #[test]
    fn rejects_degree_out_of_range() {
        // 3 points -> n = 2; degree must be <= n, so 3 is out of range
        // (degree = 2 is valid -- it's the degenerate "single polynomial
        // through all points" case).
        let points = vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0)];
        assert!(FloaterHormannInterpolant::new(&points, 2).is_ok());
        assert!(FloaterHormannInterpolant::new(&points, 3).is_err());
    }

    #[test]
    fn evaluate_ratio_matches_evaluate() {
        let points = vec![(0.0, 1.0), (1.0, 2.0), (2.0, 3.0), (3.0, 4.0)];
        let interpolant = FloaterHormannInterpolant::new(&points, 1).unwrap();
        let x = Ratio::new::<ratio>(1.5);
        assert!((interpolant.evaluate_ratio(x).get::<ratio>() - interpolant.evaluate(1.5)).abs() < 1e-12);
    }
}
