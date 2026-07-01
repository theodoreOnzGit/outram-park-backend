// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

/// Catmull-Rom cubic spline interpolation over a sorted table `(xs, ys)`.
///
/// At the boundary knots the missing neighbours are mirrored (ghost-point
/// extension), matching OpenFOAM's `Foam::interpolateSplineXY`.
/// Clamps to endpoint values outside the table range.
/// Assumes `xs` is sorted in ascending order.
pub fn interpolate_spline_xy(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len();
    assert_eq!(n, ys.len(), "xs and ys must have the same length");

    if n == 0 { return 0.0; }
    if n == 1 || x <= xs[0] { return ys[0]; }
    if x >= xs[n - 1] { return ys[n - 1]; }

    // Linear fallback for two points
    if n == 2 {
        let t = (x - xs[0]) / (xs[1] - xs[0]);
        return ys[0] + t * (ys[1] - ys[0]);
    }

    // Binary search: hi = first index where xs[hi] >= x
    let hi = xs.partition_point(|&v| v < x);
    let lo = hi - 1;

    let y1 = ys[lo];
    let y2 = ys[hi];

    // Ghost points at the boundary
    let y0 = if lo == 0 { 2.0 * y1 - y2 } else { ys[lo - 1] };
    let y3 = if hi + 1 == n { 2.0 * y2 - y1 } else { ys[hi + 1] };

    // Normalised position in the [lo, hi] interval
    let mu = (x - xs[lo]) / (xs[hi] - xs[lo]);

    // Catmull-Rom evaluation (Horner form)
    0.5 * (2.0 * y1
        + mu * ((-y0 + y2)
            + mu * ((2.0 * y0 - 5.0 * y1 + 4.0 * y2 - y3)
                + mu * (-y0 + 3.0 * y1 - 3.0 * y2 + y3))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_at_knots() {
        let xs = [0.0, 1.0, 2.0, 3.0, 4.0];
        let ys = [1.0, 2.0, 0.0, -1.0, 1.0];
        for i in 0..5 {
            let v = interpolate_spline_xy(xs[i], &xs, &ys);
            assert!((v - ys[i]).abs() < 1e-12, "at x={}: got {} expected {}", xs[i], v, ys[i]);
        }
    }

    #[test]
    fn clamp_left() {
        let xs = [1.0, 2.0, 3.0];
        let ys = [5.0, 10.0, 15.0];
        assert_eq!(interpolate_spline_xy(0.0, &xs, &ys), 5.0);
    }

    #[test]
    fn clamp_right() {
        let xs = [1.0, 2.0, 3.0];
        let ys = [5.0, 10.0, 15.0];
        assert_eq!(interpolate_spline_xy(4.0, &xs, &ys), 15.0);
    }

    #[test]
    fn linear_data_matches_linear() {
        // For linear data, Catmull-Rom is exact
        let xs = [0.0, 1.0, 2.0, 3.0];
        let ys = [0.0, 2.0, 4.0, 6.0];
        for &tx in &[0.25, 0.5, 0.75, 1.25, 1.5, 1.75, 2.5] {
            let v = interpolate_spline_xy(tx, &xs, &ys);
            let expected = 2.0 * tx;
            assert!((v - expected).abs() < 1e-12, "at x={tx}: got {v}, expected {expected}");
        }
    }

    #[test]
    fn single_and_two_point_edge_cases() {
        // Single
        let xs1 = [3.0];
        let ys1 = [99.0];
        assert_eq!(interpolate_spline_xy(0.0, &xs1, &ys1), 99.0);

        // Two-point falls back to linear
        let xs2 = [0.0, 2.0];
        let ys2 = [0.0, 4.0];
        let v = interpolate_spline_xy(1.0, &xs2, &ys2);
        assert!((v - 2.0).abs() < 1e-14);
    }
}
