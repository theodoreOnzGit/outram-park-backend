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

use super::{OdeError, OdeSystem, OdeSolverConfig, adaptive_step, integrate_interval, normalize_error};

/// Explicit first-order Euler solver with adaptive step size.
/// Maps to `Foam::Euler` (which inherits from `adaptiveSolver`).
pub struct Euler {
    pub config: OdeSolverConfig,
    // scratch buffers (sized to n_eqns)
    dydx0: Vec<f64>,
    y_temp: Vec<f64>,
    err: Vec<f64>,
}

impl Euler {
    pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        let mut cfg = OdeSolverConfig::default();
        cfg.abs_tol = abs_tol;
        cfg.rel_tol = rel_tol;
        Self {
            config: cfg,
            dydx0: vec![0.0; n],
            y_temp: vec![0.0; n],
            err: vec![0.0; n],
        }
    }

    /// Take one adaptive step. On return `x` and `y` are updated and
    /// `dx_try` holds a suggested step size for the next call.
    pub fn solve_step(
        &mut self,
        ode: &dyn OdeSystem,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        let cfg = self.config.clone();
        let err_buf = &mut self.err;
        let abs_tol = cfg.abs_tol;
        let rel_tol = cfg.rel_tol;

        adaptive_step(
            &cfg,
            |_x0, y0, dydx0, dx, y_out| {
                for i in 0..y0.len() {
                    err_buf[i] = dx * dydx0[i];
                    y_out[i] = y0[i] + err_buf[i];
                }
                normalize_error(y0, y_out, err_buf, abs_tol, rel_tol)
            },
            ode,
            x,
            y,
            &mut self.dydx0,
            &mut self.y_temp,
            dx_try,
        )
    }

    /// Integrate from `x_start` to `x_end`.
    pub fn integrate(
        &mut self,
        ode: &dyn OdeSystem,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        let cfg = self.config.clone();
        integrate_interval(
            &cfg,
            &mut |x, y, dx| self.solve_step(ode, x, y, dx),
            x_start,
            x_end,
            y,
            dx_est,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DecayOde;
    impl OdeSystem for DecayOde {
        fn n_eqns(&self) -> usize { 1 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
    }

    struct RotationOde;
    impl OdeSystem for RotationOde {
        fn n_eqns(&self) -> usize { 2 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[1];
            dydx[1] =  y[0];
        }
    }

    #[test]
    fn euler_exponential_decay() {
        // y' = -y, y(0) = 1  →  y(1) = e^{-1} ≈ 0.36788
        // Euler is 1st-order: use loose tolerance so adaptive step stays ~0.01 (≈100 steps)
        let ode = DecayOde;
        let mut solver = Euler::new(1, 1e-3, 1e-2);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 1.0, &mut y, &mut dx).unwrap();
        let expected = (-1.0_f64).exp();
        assert!(
            (y[0] - expected).abs() < 1e-2,
            "y={}, expected {}", y[0], expected
        );
    }

    #[test]
    fn euler_rotation() {
        // y1' = -y2, y2' = y1,  y(0) = (1, 0)  →  y(π/2) ≈ (0, 1)
        let ode = RotationOde;
        let mut solver = Euler::new(2, 1e-3, 1e-2);
        let mut y = vec![1.0_f64, 0.0];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, std::f64::consts::PI / 2.0, &mut y, &mut dx).unwrap();
        assert!(y[0].abs() < 5e-2, "y0={}", y[0]);
        assert!((y[1] - 1.0).abs() < 5e-2, "y1={}", y[1]);
    }
}
