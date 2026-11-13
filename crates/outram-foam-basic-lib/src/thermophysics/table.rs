// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2011-2016 OpenFOAM Foundation
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

//! `(T, value)` tables as OpenFOAM-v2506 (ESI) evaluates them
//! (gh:#1362 row 4).
//!
//! Ports `thermophysicalFunctions::nonUniformTable` and
//! `thermophysicalFunctions::integratedNonUniformTable` from
//! `src/thermophysicalModels/thermophysicalProperties/thermophysicalFunctions/`,
//! v2506, **including their quirks**, because the point is bit identity with
//! upstream:
//!
//! * a temperature outside the table is **extrapolated linearly** from the
//!   first or last interval (above the last knot upstream reads one element past
//!   the end of its list, undefined behaviour; the port stops at the last interval) (the port's older [`crate::interpolation::interpolate_xy`]
//!   clamps to the end value);
//! * the interval is found through a jump table of spacing `0.9 min(dT)`;
//! * for a temperature exactly equal to the last knot, upstream's `index()`
//!   takes the `nd = 0` branch and returns interval 1 (not the last), so the
//!   integrals stored at the last knot are extrapolated from interval 1. With a
//!   two-entry table that is the right interval; with more entries it is not.
//!   Reproduced as upstream has it and measured in the test (see the V&V
//!   README).

use petir::mathf::RealMath;

use crate::thermophysics::constants::{P_REF, T_STD};

/// `thermophysicalFunctions::nonUniformTable`: a piecewise-linear `f(T)`.
#[derive(Debug, Clone)]
pub struct NonUniformTable {
    values: Vec<(f64, f64)>,
    t_min: f64,
    t_max: f64,
    delta_t: f64,
    jump: Vec<usize>,
}

impl NonUniformTable {
    /// Build from `(T, f)` pairs in ascending `T`. Panics on fewer than two
    /// entries, as upstream raises a `FatalIOError`.
    pub fn new(values: Vec<(f64, f64)>) -> Self {
        assert!(values.len() >= 2, "table has fewer than 2 entries");
        let t_min = values[0].0;
        let t_max = values[values.len() - 1].0;
        let mut delta_t = 1.0e15; // upstream GREAT
        for i in 1..values.len() {
            delta_t = f64::min(delta_t, values[i].0 - values[i - 1].0);
        }
        delta_t *= 0.9;
        let n_jump = ((t_max - t_min) / delta_t + 1.0) as usize;
        let mut jump = vec![0usize; n_jump];
        let mut i = 0usize;
        for (j, slot) in jump.iter_mut().enumerate() {
            let t = t_min + (j as f64) * delta_t;
            if t > values[i + 1].0 {
                i += 1;
            }
            *slot = i;
        }
        Self {
            values,
            t_min,
            t_max,
            delta_t,
            jump,
        }
    }

    /// The `(T, f)` pairs.
    pub fn values(&self) -> &[(f64, f64)] {
        &self.values
    }

    /// Upstream `index(p, T)`: the interval `[i, i + 1]` used for `T`.
    pub fn index(&self, t: f64) -> usize {
        let mut nd = 0.0;
        if t > self.t_min && t < self.t_max {
            nd = (t - self.t_min) / self.delta_t;
        } else if t > self.t_max {
            nd = (self.t_max - self.t_min) / self.delta_t;
        }
        let j = nd as usize;
        let mut i = self.jump[j];
        // Upstream: `if (i < values_.size() - 1 && T > values_[i + 1].first()) ++i;`.
        // For T above the last knot that makes `i = size - 1` and the next line
        // of `f()` reads `values_[size]`, one past the end (undefined behaviour
        // upstream). The guard here stops at the last interval, so the table
        // extrapolates linearly from it; identical to upstream for every T that
        // upstream evaluates without reading out of bounds.
        if i + 2 < self.values.len() && t > self.values[i + 1].0 {
            i += 1;
        }
        i
    }

    /// `f(T)`, linearly extrapolated outside the table.
    pub fn f(&self, t: f64) -> f64 {
        let i = self.index(t);
        let ti = self.values[i].0;
        let lambda = (t - ti) / (self.values[i + 1].0 - ti);
        self.values[i].1 + lambda * (self.values[i + 1].1 - self.values[i].1)
    }

    /// `df/dT` of the interval containing `T`.
    pub fn dfdt(&self, t: f64) -> f64 {
        let i = self.index(t);
        (self.values[i + 1].1 - self.values[i].1) / (self.values[i + 1].0 - self.values[i].0)
    }
}

/// `thermophysicalFunctions::integratedNonUniformTable`: a [`NonUniformTable`]
/// with the running integrals of `f` and `f/T`, both zero at `Tstd`.
#[derive(Debug, Clone)]
pub struct IntegratedNonUniformTable {
    table: NonUniformTable,
    intf: Vec<f64>,
    intf_by_t: Vec<f64>,
}

impl IntegratedNonUniformTable {
    /// Upstream constructor: integrals at each knot, then offset so that both
    /// are zero at `(Pstd, Tstd)`.
    pub fn new(values: Vec<(f64, f64)>) -> Self {
        let table = NonUniformTable::new(values);
        let n = table.values.len();
        let mut me = Self {
            table,
            intf: vec![0.0; n],
            intf_by_t: vec![0.0; n],
        };
        for i in 1..n {
            let ti = me.table.values[i].0;
            me.intf[i] = me.intfdt(ti);
            me.intf_by_t[i] = me.intf_by_tdt(ti);
        }
        let _ = P_REF; // upstream passes Pstd as the (unused) pressure argument
        let intf_std = me.intfdt(T_STD);
        let intf_by_t_std = me.intf_by_tdt(T_STD);
        for i in 0..n {
            me.intf[i] -= intf_std;
            me.intf_by_t[i] -= intf_by_t_std;
        }
        me
    }

    /// The underlying table.
    pub fn table(&self) -> &NonUniformTable {
        &self.table
    }

    /// `f(T)`.
    pub fn f(&self, t: f64) -> f64 {
        self.table.f(t)
    }

    /// `int f dT`, zero at `Tstd`.
    pub fn intfdt(&self, t: f64) -> f64 {
        let v = &self.table.values;
        let i = self.table.index(t);
        let ti = v[i].0;
        let fi = v[i].1;
        let dt = t - ti;
        let lambda = dt / (v[i + 1].0 - ti);
        self.intf[i] + (fi + 0.5 * lambda * (v[i + 1].1 - fi)) * dt
    }

    /// `int f/T dT`, zero at `Tstd`.
    pub fn intf_by_tdt(&self, t: f64) -> f64 {
        let v = &self.table.values;
        let i = self.table.index(t);
        let ti = v[i].0;
        let fi = v[i].1;
        let gradf = (v[i + 1].1 - fi) / (v[i + 1].0 - ti);
        self.intf_by_t[i] + ((fi - gradf * ti) * (t / ti).r_ln() + gradf * (t - ti))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cp_table() -> Vec<(f64, f64)> {
        vec![(200.0, 1000.0), (400.0, 1020.0), (800.0, 1100.0), (1200.0, 1180.0), (2000.0, 1250.0)]
    }

    #[test]
    fn f_interpolates_and_extrapolates_linearly() {
        let t = NonUniformTable::new(cp_table());
        assert_eq!(t.f(300.0), 1010.0);
        assert_eq!(t.f(600.0), 1060.0);
        // below the first knot: slope of interval 0 (0.1 per K)
        assert!((t.f(100.0) - 990.0).abs() < 1e-12);
        // above the last knot: slope of the last interval (0.0875 per K)
        assert!((t.f(2400.0) - 1285.0).abs() < 1e-12);
    }

    #[test]
    fn integral_is_zero_at_tstd_and_matches_trapezoid_inside() {
        let t = IntegratedNonUniformTable::new(cp_table());
        assert!(t.intfdt(T_STD).abs() < 1e-9);
        // exact integral of the piecewise-linear f from 400 to 800 is the trapezoid
        let d = t.intfdt(800.0) - t.intfdt(400.0);
        assert!((d - 400.0 * (1020.0 + 1100.0) / 2.0).abs() < 1e-6);
        assert!(t.intf_by_tdt(T_STD).abs() < 1e-12);
        // the f/T integral is monotone increasing for positive f
        assert!(t.intf_by_tdt(900.0) > t.intf_by_tdt(500.0));
        assert_eq!(t.table().values().len(), 5);
        assert!((t.table().dfdt(500.0) - 0.2).abs() < 1e-12);
    }
}
