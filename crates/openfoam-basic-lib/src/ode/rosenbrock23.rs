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

use super::{OdeError, OdeSystem, OdeSolverConfig, integrate_interval, normalize_error};
use crate::matrix::SquareMatrix;

// Rosenbrock23 coefficients — Foam::Rosenbrock23 constants
const GAMMA: f64 = 0.435_866_521_508_458_999_41;
const C2:    f64 = GAMMA;   // same as gamma
const A21:   f64 = 1.0;
const C21:   f64 = -1.015_617_108_387_770_209_2;
const C31:   f64 =  4.075_995_645_253_769_982_5;
const C32:   f64 =  9.207_679_429_833_079_124_2;
const B1:    f64 = 1.0;
const B2:    f64 = 6.169_794_704_382_824_559_3;
const B3:    f64 = -0.427_722_565_432_185_733_3;
const E1:    f64 = 0.5;
const E2:    f64 = -2.907_955_871_680_546_982_2;
const E3:    f64 =  0.223_540_698_978_115_696_3;
const D1:    f64 = GAMMA;
const D2:    f64 = 0.242_919_964_548_168_043_7;
const D3:    f64 = 2.185_138_002_766_405_851_2;

/// W-method Rosenbrock23 stiff solver with adaptive step size.
///
/// Requires the user's `OdeSystem::jacobian` to be implemented.
/// Maps to `Foam::Rosenbrock23`.
pub struct Rosenbrock23 {
    pub config: OdeSolverConfig,
    n: usize,
    dydx0: Vec<f64>,
    y_temp: Vec<f64>,
    k1: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    err: Vec<f64>,
    dydx_inner: Vec<f64>,
    dfdx: Vec<f64>,
    dfdy: SquareMatrix,
    a: SquareMatrix,
    pivot: Vec<usize>,
}

impl Rosenbrock23 {
    pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        let mut cfg = OdeSolverConfig::default();
        cfg.abs_tol = abs_tol;
        cfg.rel_tol = rel_tol;
        Self {
            config: cfg,
            n,
            dydx0: vec![0.0; n],
            y_temp: vec![0.0; n],
            k1: vec![0.0; n],
            k2: vec![0.0; n],
            k3: vec![0.0; n],
            err: vec![0.0; n],
            dydx_inner: vec![0.0; n],
            dfdx: vec![0.0; n],
            dfdy: SquareMatrix::new(n),
            a: SquareMatrix::new(n),
            pivot: vec![0; n],
        }
    }

    /// Single inner step (no retry). Returns the normalised error estimate.
    fn inner_step(
        &mut self,
        ode: &dyn OdeSystem,
        x0: f64,
        y0: &[f64],
        dydx0: &[f64],
        dx: f64,
        y_out: &mut Vec<f64>,
    ) -> f64 {
        let n = self.n;
        let abs_tol = self.config.abs_tol;
        let rel_tol = self.config.rel_tol;

        // Build Jacobian
        ode.jacobian(x0, y0, &mut self.dfdx, &mut self.dfdy);

        // a = I/(gamma*dx) - J
        for i in 0..n {
            for j in 0..n {
                self.a.set(i, j, -self.dfdy.get(i, j));
            }
            self.a.add(i, i, 1.0 / (GAMMA * dx));
        }

        // LU-decompose a (in place)
        self.pivot = self.a.lu_decompose();

        // k1 = (I/(γΔx) − J)^{-1} · (f + Δx·d1·∂f/∂x)
        for i in 0..n {
            self.k1[i] = dydx0[i] + dx * D1 * self.dfdx[i];
        }
        self.a.lu_back_substitute(&self.pivot, &mut self.k1);

        // k2
        for i in 0..n {
            y_out[i] = y0[i] + A21 * self.k1[i];
        }
        ode.derivatives(x0 + C2 * dx, y_out, &mut self.dydx_inner);
        for i in 0..n {
            self.k2[i] = self.dydx_inner[i] + dx * D2 * self.dfdx[i]
                + C21 * self.k1[i] / dx;
        }
        self.a.lu_back_substitute(&self.pivot, &mut self.k2);

        // k3
        for i in 0..n {
            self.k3[i] = self.dydx_inner[i] + dx * D3 * self.dfdx[i]
                + (C31 * self.k1[i] + C32 * self.k2[i]) / dx;
        }
        self.a.lu_back_substitute(&self.pivot, &mut self.k3);

        // Solution and error
        for i in 0..n {
            y_out[i] = y0[i] + B1 * self.k1[i] + B2 * self.k2[i] + B3 * self.k3[i];
            self.err[i] = E1 * self.k1[i] + E2 * self.k2[i] + E3 * self.k3[i];
        }

        normalize_error(y0, y_out, &self.err, abs_tol, rel_tol)
    }

    /// One adaptive step (retries with smaller dx if error > 1).
    pub fn solve_step(
        &mut self,
        ode: &dyn OdeSystem,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        let cfg = self.config.clone();
        let mut dx = *dx_try;

        // Get initial derivatives
        ode.derivatives(*x, y, &mut self.dydx0);
        let dydx0_snapshot = self.dydx0.clone();

        // Temporarily take y_temp out to avoid a double-&mut-self borrow
        let mut y_temp = std::mem::take(&mut self.y_temp);

        let err = loop {
            let err = self.inner_step(ode, *x, y, &dydx0_snapshot, dx, &mut y_temp);
            if err <= 1.0 {
                break err;
            }
            let scale = (cfg.safe_scale * err.powf(-cfg.alpha_dec)).max(cfg.min_scale);
            dx *= scale;
            if dx.abs() < f64::EPSILON {
                self.y_temp = y_temp;
                return Err(OdeError::StepSizeUnderflow);
            }
        };

        *x += dx;
        std::mem::swap(y, &mut y_temp);
        self.y_temp = y_temp;

        let threshold = (cfg.max_scale / cfg.safe_scale).powf(-1.0 / cfg.alpha_inc);
        *dx_try = if err > threshold {
            let scale = (cfg.safe_scale * err.powf(-cfg.alpha_inc))
                .clamp(cfg.min_scale, cfg.max_scale);
            dx * scale
        } else {
            dx * cfg.safe_scale * cfg.max_scale
        };

        Ok(())
    }

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

    // Stiff scalar: y' = -1000·y,  y(0)=1  →  y(t)=e^{-1000t}
    struct StiffDecay;
    impl OdeSystem for StiffDecay {
        fn n_eqns(&self) -> usize { 1 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -1000.0 * y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdy.set(0, 0, -1000.0);
        }
    }

    // Non-stiff: y' = -y
    struct DecayOde;
    impl OdeSystem for DecayOde {
        fn n_eqns(&self) -> usize { 1 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdy.set(0, 0, -1.0);
        }
    }

    // 2D rotation: y1' = -y2, y2' = y1
    struct RotationOde;
    impl OdeSystem for RotationOde {
        fn n_eqns(&self) -> usize { 2 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[1];
            dydx[1] =  y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdx[1] = 0.0;
            dfdy.set(0, 0,  0.0); dfdy.set(0, 1, -1.0);
            dfdy.set(1, 0,  1.0); dfdy.set(1, 1,  0.0);
        }
    }

    #[test]
    fn rosenbrock23_stiff_decay() {
        // Stiff y' = -1000y: explicit methods need tiny steps, Rosenbrock handles it
        let ode = StiffDecay;
        let mut solver = Rosenbrock23::new(1, 1e-8, 1e-6);
        let mut y = vec![1.0_f64];
        let mut dx = 0.01;
        solver.integrate(&ode, 0.0, 0.01, &mut y, &mut dx).unwrap();
        let expected = (-10.0_f64).exp(); // e^{-1000 * 0.01}
        assert!(
            (y[0] - expected).abs() < 1e-5,
            "y={:.10}, expected={:.10}", y[0], expected
        );
    }

    #[test]
    fn rosenbrock23_nonstiff_decay() {
        // y' = -y, y(0) = 1 → y(1) = e^{-1}
        let ode = DecayOde;
        let mut solver = Rosenbrock23::new(1, 1e-8, 1e-6);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 1.0, &mut y, &mut dx).unwrap();
        let expected = (-1.0_f64).exp();
        assert!(
            (y[0] - expected).abs() < 1e-6,
            "y={:.10}, expected={:.10}", y[0], expected
        );
    }

    #[test]
    fn rosenbrock23_rotation() {
        // y(π/2) ≈ (0, 1)
        let ode = RotationOde;
        let mut solver = Rosenbrock23::new(2, 1e-8, 1e-6);
        let mut y = vec![1.0_f64, 0.0];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, std::f64::consts::PI / 2.0, &mut y, &mut dx).unwrap();
        assert!(y[0].abs() < 1e-5, "y0={}", y[0]);
        assert!((y[1] - 1.0).abs() < 1e-5, "y1={}", y[1]);
    }

    // Stiff Van der Pol (mu=1000) with Jacobian — for Rosenbrock23
    struct VanDerPolStiff { mu: f64 }
    impl OdeSystem for VanDerPolStiff {
        fn n_eqns(&self) -> usize { 2 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = y[1];
            dydx[1] = self.mu * (1.0 - y[0] * y[0]) * y[1] - y[0];
        }
        fn jacobian(&self, _x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdx[1] = 0.0;
            dfdy.set(0, 0, 0.0);
            dfdy.set(0, 1, 1.0);
            dfdy.set(1, 0, -2.0 * self.mu * y[0] * y[1] - 1.0);
            dfdy.set(1, 1, self.mu * (1.0 - y[0] * y[0]));
        }
    }

    #[test]
    fn rosenbrock23_stiff_vdp_mu1000() {
        // Stiff Van der Pol (mu=1000): Rosenbrock23 handles it efficiently.
        // RKF45 would need h < 2/mu^2 ≈ 2e-6, requiring ~50k steps over t=0.1.
        let ode = VanDerPolStiff { mu: 1000.0 };
        let mut solver = Rosenbrock23::new(2, 1e-6, 1e-4);
        let mut y = vec![2.0_f64, 0.0];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 0.1, &mut y, &mut dx).unwrap();
        // Limit cycle amplitude ∈ [-2, 2] for VdP
        assert!(y[0].abs() < 3.0, "y0={}", y[0]);
    }
}
