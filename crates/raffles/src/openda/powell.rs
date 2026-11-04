// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream file:    algorithms/java/src/org/openda/algorithms/PowellCoreOptimizer.java
//                       (initialize, next, LineSearch, bracketLineMinimum,
//                        brentLineSearch)
//   Upstream commit:  e51565c2c8def7d35f8b378a9ac118edbd98a94d
//   Accessed:         2026-11-04
//
//   Copyright (c) 2012 OpenDA Association. All rights reserved.
//   OpenDA is free software: you can redistribute it and/or modify it under
//   the terms of the GNU Lesser General Public License as published by the
//   Free Software Foundation, either version 3 of the License, or (at your
//   option) any later version. OpenDA is distributed in the hope that it will
//   be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser
//   General Public License for more details (LICENSE-LGPL-3.0-OPENDA).
//
// This Rust translation is part of RAFFLES / Outram Park, conveyed under
// AGPL-3.0-only as a combined work (GPLv3 section 13 / AGPLv3 section 13).
//
// Translation notes: OpenDA's own adaptation of Numerical Recipes' powell,
// mnbrak and brent, ported in its order (Java's `Math.pow(x, 2.0)` written
// `x*x`, identical on every sample measured). Ported faithfully, NOT fixed:
// the line search evaluates its base point again, and rescales the search
// direction IN PLACE by max(|alpha|, 0.1), so the stored directions change
// every call; the Brent stopping test is on the change in f; the extrapolation
// test compares with f_p (NR: f_prev). Extra stop criteria are not ported.
// ---------------------------------------------------------------------------

//! **OpenDA's Powell minimiser** with its bracketing and Brent line search
//! (gh:#1192).

use super::cost::CostFunction;
use super::linalg::axpy;
use crate::{RafflesError, Result};

/// OpenDA's `PowellCoreOptimizer`.
#[derive(Debug, Clone, PartialEq)]
pub struct Powell {
    /// `maxitPowell` (200).
    pub max_iterations: usize,
    /// `maxitBrent` (200).
    pub max_brent: usize,
    /// `maxitBracket` (20).
    pub max_bracket: usize,
    /// `relTolPowell` (0.01).
    pub rel_tol: f64,
    /// `absTolPowell` (0.01).
    pub abs_tol: f64,
    /// `relTolBrent` (0.01).
    pub rel_tol_brent: f64,
    /// `absTolBrent` (0.001).
    pub abs_tol_brent: f64,
    /// `bracketFirstTry` (1.0).
    pub bracket_first_try: f64,
    /// `limit` (100).
    pub limit: f64,
    /// `tiny` (1e-15).
    pub tiny: f64,
    p_min: Vec<f64>,
    f_min: f64,
    p_current: Vec<f64>,
    f_current: f64,
    directions: Vec<Vec<f64>>,
    more_to_do: bool,
    iterations: usize,
}

impl Default for Powell {
    fn default() -> Self {
        Self {
            max_iterations: 200,
            max_brent: 200,
            max_bracket: 20,
            rel_tol: 0.01,
            abs_tol: 0.01,
            rel_tol_brent: 0.01,
            abs_tol_brent: 0.001,
            bracket_first_try: 1.0,
            limit: 100.0,
            tiny: 1.0e-15,
            p_min: Vec::new(),
            f_min: f64::MAX,
            p_current: Vec::new(),
            f_current: 0.0,
            directions: Vec::new(),
            more_to_do: true,
            iterations: 0,
        }
    }
}

fn along(base: &[f64], alpha: f64, xi: &[f64]) -> Vec<f64> {
    let mut p = base.to_vec();
    axpy(&mut p, alpha, xi);
    p
}

impl Powell {
    /// `initialize(pInit, width)`: directions `width·e_{n−1}`, …,
    /// `width·e_0` (OpenDA's order: the last coordinate first).
    ///
    /// # Errors
    /// The cost's.
    pub fn initialize<C: CostFunction>(&mut self, f: &mut C, p_init: &[f64], width: f64) -> Result<()> {
        let n = p_init.len();
        let dirs = (0..n)
            .map(|i| {
                let mut x: Vec<f64> = p_init.iter().map(|v| v * 0.0).collect();
                x[n - i - 1] += width;
                x
            })
            .collect();
        self.initialize_with_directions(f, p_init, dirs)
    }

    /// `initialize(pInit, searchInit)`.
    ///
    /// # Errors
    /// The cost's, or no directions.
    pub fn initialize_with_directions<C: CostFunction>(&mut self, f: &mut C, p_init: &[f64], directions: Vec<Vec<f64>>) -> Result<()> {
        if directions.is_empty() {
            return Err(RafflesError::InvalidParameter { parameter: "directions".into(), value: 0.0, reason: "need at least one".into() });
        }
        self.f_min = f.evaluate(p_init)?;
        self.p_min = p_init.to_vec();
        self.p_current = p_init.to_vec();
        self.f_current = self.f_min;
        self.directions = directions;
        Ok(())
    }

    /// The best point and cost the optimiser has accepted.
    #[must_use]
    pub fn optimum(&self) -> (&[f64], f64) {
        (&self.p_min, self.f_min)
    }

    /// The current search directions.
    #[must_use]
    pub fn directions(&self) -> &[Vec<f64>] {
        &self.directions
    }

    /// Outer iterations done.
    #[must_use]
    pub fn iterations(&self) -> usize {
        self.iterations
    }

    /// `optimize()`.
    ///
    /// # Errors
    /// The cost's; bracketing that runs away (OpenDA throws); a call before
    /// initialisation.
    pub fn optimize<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        if self.directions.is_empty() {
            return Err(RafflesError::InvalidParameter { parameter: "powell".into(), value: 0.0, reason: "initialize first".into() });
        }
        while self.more_to_do {
            self.next(f)?;
        }
        Ok(())
    }

    fn next<C: CostFunction>(&mut self, f: &mut C) -> Result<()> {
        let n = self.directions.len();
        if self.iterations >= self.max_iterations {
            self.more_to_do = false;
        }
        if !self.more_to_do {
            return Ok(());
        }
        self.iterations += 1;
        let mut ibig = 0;
        let mut del = 0.0;
        let mut p = self.p_current.clone();
        let mut f_p = self.f_current;
        let f_prev = f_p;
        for i in 0..n {
            let mut xi = std::mem::take(&mut self.directions[i]);
            let (p_opt, f_opt) = self.line_search(f, &p, &mut xi)?;
            self.directions[i] = xi;
            if (f_p - f_opt).abs() > del {
                del = (f_p - f_opt).abs();
                ibig = i;
            }
            p = p_opt;
            f_p = f_opt;
        }
        if f_p < self.f_current {
            self.f_min = f_p;
            self.p_min.clone_from(&p);
        }
        let rel = (self.f_current - f_p).abs() / (0.5 * (self.f_current + f_p).abs());
        let abs = (self.f_current - f_p).abs();
        if rel < self.rel_tol || abs < self.abs_tol {
            self.more_to_do = false;
        }
        if self.more_to_do {
            let mut xi = p.clone();
            axpy(&mut xi, -1.0, &self.p_current);
            let mut p_ext = p.clone();
            axpy(&mut p_ext, 1.0, &xi);
            let f_ext = f.evaluate(&p_ext)?;
            let a = f_prev - f_p - del;
            let b = f_prev - f_ext;
            let temp = (2.0 * (f_prev - 2.0 * f_p + f_ext) * (a * a)) - del * (b * b);
            if f_ext > f_p || temp >= 0.0 {
                self.p_current = p;
                self.f_current = f_p;
            } else {
                let mut xi_dir = xi.clone();
                let (p_opt, f_opt) = self.line_search(f, &p, &mut xi_dir)?;
                self.p_current.clone_from(&p_opt);
                self.f_current = f_opt;
                self.directions[ibig] = self.directions[n - 1].clone();
                // OpenDA stores xi.clone() taken AFTER the line search has
                // rescaled xi in place.
                self.directions[n - 1] = xi_dir;
                if f_opt < self.f_min {
                    self.f_min = f_opt;
                    self.p_min = p_opt;
                }
            }
        }
        Ok(())
    }

    fn line_search<C: CostFunction>(&self, f: &mut C, base: &[f64], xi: &mut Vec<f64>) -> Result<(Vec<f64>, f64)> {
        let mut init = [0.0, self.bracket_first_try, 0.0];
        let mut fb = [0.0, 0.0, 0.0];
        for i in 0..2 {
            fb[i] = f.evaluate(&along(base, init[i], xi))?;
        }
        let f_start = fb[1];
        self.bracket(f, base, xi, &mut init, &mut fb)?;
        let opt = self.brent(f, base, xi, &mut init, &mut fb, f_start)?;
        let p_opt = along(base, init[1], xi);
        let alpha = init[1].abs().max(0.1);
        for v in xi.iter_mut() {
            *v *= alpha;
        }
        Ok((p_opt, opt))
    }

    fn bracket<C: CostFunction>(&self, f: &mut C, base: &[f64], xi: &[f64], ab: &mut [f64; 3], fb: &mut [f64; 3]) -> Result<()> {
        let gold = 1.618034;
        let (mut ax, mut bx) = (ab[0], ab[1]);
        let (mut fa, mut fbv) = (fb[0], fb[1]);
        if fbv > fa {
            std::mem::swap(&mut ax, &mut bx);
            std::mem::swap(&mut fbv, &mut fa);
        }
        let mut cx = bx + (bx - ax) * gold;
        let mut fc = f.evaluate(&along(base, cx, xi))?;
        let mut iter = 0;
        while fbv >= fc {
            if iter > self.max_bracket {
                return Err(RafflesError::InvalidParameter { parameter: "Powell bracketing".into(), value: iter as f64, reason: "the parameter is running away (OpenDA throws)".into() });
            }
            let r = (bx - ax) * (fbv - fc);
            let q = (bx - cx) * (fbv - fa);
            let mut q_min_r = q - r;
            if q_min_r.abs() < self.tiny {
                q_min_r = if q_min_r >= 0.0 { self.tiny } else { -self.tiny };
            }
            let mut u = bx - ((bx - cx) * q - (bx - ax) * r) / (2.0 * q_min_r);
            let ulim = bx + (cx - bx) * self.limit;
            let fu;
            if (bx - u) * (u - cx) > 0.0 {
                let f1 = f.evaluate(&along(base, u, xi))?;
                if f1 < fc {
                    ax = bx;
                    bx = u;
                    fa = fbv;
                    fbv = f1;
                    continue;
                } else if f1 > fbv {
                    cx = u;
                    fc = f1;
                    continue;
                }
                u = cx + (cx - bx) * gold;
                fu = f.evaluate(&along(base, u, xi))?;
            } else if (cx - u) * (u - ulim) > 0.0 {
                let f1 = f.evaluate(&along(base, u, xi))?;
                if f1 < fc {
                    bx = cx;
                    cx = u;
                    u = cx + (cx - bx) * gold;
                    fbv = fc;
                    fc = f1;
                    fu = f.evaluate(&along(base, u, xi))?;
                } else {
                    fu = f1;
                }
            } else if (u - ulim) * (ulim - cx) >= 0.0 {
                u = ulim;
                fu = f.evaluate(&along(base, u, xi))?;
            } else {
                u = cx + (cx - bx) * gold;
                fu = f.evaluate(&along(base, u, xi))?;
            }
            ax = bx;
            bx = cx;
            cx = u;
            fa = fbv;
            fbv = fc;
            fc = fu;
            iter += 1;
        }
        *ab = [ax, bx, cx];
        *fb = [fa, fbv, fc];
        Ok(())
    }

    #[allow(clippy::many_single_char_names)]
    fn brent<C: CostFunction>(&self, f: &mut C, base: &[f64], xi: &[f64], ab: &mut [f64; 3], fb: &mut [f64; 3], f_start: f64) -> Result<f64> {
        let (ax, bx, cx) = (ab[0], ab[1], ab[2]);
        let mut a = ax.min(cx);
        let mut b = ax.max(cx);
        let mut v = bx;
        let mut w = v;
        let mut x = v;
        let mut e: f64 = 0.0;
        let mut d: f64 = 0.0;
        let mut fx = f.evaluate(&along(base, x, xi))?;
        let mut fv = fx;
        let mut fw = fx;
        let mut f_last = fx + 1.0;
        let mut f_previous = fx;
        for _ in 1..=self.max_brent {
            let xm = (a + b) * 0.5;
            let tol1 = self.rel_tol_brent * xm.abs() + self.tiny;
            let tol2 = tol1 * 2.0;
            let abs_change = (f_last - f_previous).abs();
            let rel_change = abs_change / (f_start - f_last).abs();
            if f_last < f_start && (abs_change < self.abs_tol_brent || rel_change < self.rel_tol_brent) {
                break;
            }
            let r = (x - w) * (fx - fv);
            let mut q = (x - v) * (fx - fw);
            let mut p = (x - v) * q - (x - w) * r;
            q = (q - r) * 2.0;
            if q > 0.0 {
                p = -p;
            }
            q = q.abs();
            let etemp = e;
            e = d;
            if etemp.abs() > tol1 && (p.abs() < (q * 0.5 * etemp).abs() && p > q * (a - x) && p < q * (b - x)) {
                d = p / q;
                let u = x + d;
                if u - a < tol2 || b - u < tol2 {
                    d = if xm - x > 0.0 { tol1 } else { -tol1 };
                }
            } else {
                e = if x >= xm { a - x } else { b - x };
                d = e * 0.381966;
            }
            let u = if d.abs() >= tol1 { x + d } else { x + if d > 0.0 { tol1 } else { -tol1 } };
            let fu = f.evaluate(&along(base, u, xi))?;
            f_previous = f_last;
            f_last = fu;
            if fu <= fx {
                if u >= x {
                    a = x;
                } else {
                    b = x;
                }
                v = w;
                fv = fw;
                w = x;
                fw = fx;
                x = u;
                fx = fu;
            } else {
                if u < x {
                    a = u;
                } else {
                    b = u;
                }
                if fu <= fw || w == x {
                    v = w;
                    fv = fw;
                    w = u;
                    fw = fu;
                } else if fu <= fv || v == x || v == w {
                    v = u;
                    fv = fu;
                }
            }
        }
        *ab = [v, x, w];
        *fb = [fv, fx, fw];
        Ok(fx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openda::cost::{RosenbrockCost, SimpleCost};

    /// It reaches the quadratic's minimum; Rosenbrock descends; the
    /// directions are rescaled in place; given directions are used; errors.
    #[test]
    fn powell_minimises() {
        let mut f = SimpleCost::default();
        let mut p = Powell { abs_tol: 1e-12, rel_tol: 1e-12, ..Powell::default() };
        p.initialize(&mut f, &[0.1, 0.1], 0.5).unwrap();
        assert_eq!(p.directions()[0], vec![0.0, 0.5]);
        p.optimize(&mut f).unwrap();
        let (x, c) = p.optimum();
        assert!(c < 1e-12 && (x[0] - 1.0).abs() < 1e-6 && p.iterations() >= 1, "{x:?} {c}");
        let mut r = RosenbrockCost::default();
        let mut p = Powell::default();
        p.initialize(&mut r, &[0.0, 0.0], 0.5).unwrap();
        p.optimize(&mut r).unwrap();
        assert!(p.optimum().1 < 0.1);
        let mut g = SimpleCost::default();
        let mut p = Powell::default();
        p.initialize_with_directions(&mut g, &[0.0, 0.0], vec![vec![2.0, 0.0], vec![0.0, 1.0]]).unwrap();
        p.optimize(&mut g).unwrap();
        assert!(p.optimum().1 < 1e-6);
        assert!(Powell::default().optimize(&mut SimpleCost::default()).is_err());
        assert!(Powell::default().initialize_with_directions(&mut SimpleCost::default(), &[0.0], vec![]).is_err());
        let p = Powell { max_bracket: 0, ..Powell::default() };
        // A linear cost runs away in the bracketing.
        struct Lin(crate::openda::cost::CostHistory);
        impl CostFunction for Lin {
            fn evaluate(&mut self, x: &[f64]) -> Result<f64> {
                Ok(-x[0])
            }
            fn history(&self) -> &crate::openda::cost::CostHistory {
                &self.0
            }
        }
        let mut lin = Lin(Default::default());
        let mut q = p.clone();
        q.initialize(&mut lin, &[0.0], 1.0).unwrap();
        assert!(q.optimize(&mut lin).is_err());
    }
}
