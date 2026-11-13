// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The discrete residual and the Newton solve of [`super`] (raw SI `f64`).
//!
//! Unknown layout: axial block `k` (`0..n_z`) holds, for every channel `i`,
//! `ṁ_i(k+1)`, `T_i(k+1)`, `p_i(k)`, then `w_g(k)` for every gap. The inlet
//! `ṁ(0)`, `T(0)` and the outlet `p(n_z)` are boundary values. The residual
//! rows of block `k` are the balances of cell `k` in the same order (mass,
//! energy, axial momentum per channel, then lateral momentum per gap). Rows of
//! block `k` read unknowns of blocks `k−2..=k+1` only, which the colouring of
//! [`jacobian_triplets`] relies on.
//!
//! Unknowns are scaled (`ṁ/ṁ_ref`, `T/T_ref`, `p/p_ref`, `w/w_ref`) and rows
//! are scaled to O(1), so one infinity norm judges convergence.

use petir::multiroots::GSL_SQRT_DBL_EPSILON;
use petir::sparse::{CscMatrix, SparseLu};
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Length, MassRate, Pressure, ThermodynamicTemperature};
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use super::{
    si_area, si_cp, si_g, si_len, si_mdot, si_mu, si_p, si_q, si_rho, si_t, AxialHeat,
    LinearMassRate, SubchannelProblem, SubchannelSolution,
};
use crate::closures::wall_friction::wall_friction_acceleration;
use crate::TampinesError;

/// Converged when the scaled residual's infinity norm is at or below this.
const RESIDUAL_TOL: f64 = 1.0e-14;
/// Accept a stalled iteration (no decrease) only below this residual.
const ACCEPT_TOL: f64 = 1.0e-10;
/// Newton iteration cap.
const MAX_ITER: usize = 60;

/// One gap in raw SI.
#[derive(Debug, Clone, Copy)]
pub(super) struct RawGap {
    i: usize,
    j: usize,
    s: f64,
    l: f64,
    k: f64,
}

/// The problem in raw SI with its scales.
#[derive(Debug, Clone)]
pub(super) struct Raw {
    nc: usize,
    ng: usize,
    nz: usize,
    dz: f64,
    rho: f64,
    cp: f64,
    mu: f64,
    g: f64,
    area: Vec<f64>,
    dh: Vec<f64>,
    eps_d: Vec<f64>,
    phi: Vec<f64>,
    m_in: Vec<f64>,
    t_in: Vec<f64>,
    /// `q[i][k]`, W/m.
    q: Vec<Vec<f64>>,
    gaps: Vec<RawGap>,
    beta: f64,
    f_t: f64,
    p_out: f64,
    m_ref: f64,
    t_ref: f64,
    p_ref: f64,
    w_ref: f64,
}

fn invalid(msg: &str) -> TampinesError {
    TampinesError::InvalidInput(format!("subchannel: {msg}"))
}

fn positive(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

impl Raw {
    /// Validate and convert a problem.
    pub(super) fn from_problem(p: &SubchannelProblem) -> Result<Self, TampinesError> {
        let nc = p.channels.len();
        let ng = p.gaps.len();
        let nz = p.n_axial;
        if nc == 0 {
            return Err(invalid("no channels"));
        }
        if nz == 0 {
            return Err(invalid("n_axial must be at least 1"));
        }
        let length = si_len(p.length);
        let rho = si_rho(p.fluid.density);
        let cp = si_cp(p.fluid.specific_heat);
        let mu = si_mu(p.fluid.viscosity);
        if !(positive(length) && positive(rho) && positive(cp) && positive(mu)) {
            return Err(invalid("length, density, specific heat and viscosity must be positive"));
        }
        let mut area = Vec::with_capacity(nc);
        let mut dh = Vec::with_capacity(nc);
        let mut eps_d = Vec::with_capacity(nc);
        let mut phi = Vec::with_capacity(nc);
        let mut m_in = Vec::with_capacity(nc);
        let mut t_in = Vec::with_capacity(nc);
        let mut q = Vec::with_capacity(nc);
        for c in &p.channels {
            let a = si_area(c.flow_area);
            let d = si_len(c.hydraulic_diameter);
            let e = si_len(c.roughness);
            let m = si_mdot(c.inlet_mass_flow);
            let t = si_t(c.inlet_temperature);
            if !(positive(a) && positive(d) && positive(c.shape_factor) && positive(m) && positive(t))
            {
                return Err(invalid(
                    "area, hydraulic diameter, shape factor, inlet flow and inlet temperature must be positive",
                ));
            }
            if !(e.is_finite() && e >= 0.0) {
                return Err(invalid("roughness must be non-negative"));
            }
            let qi = match &c.heat {
                AxialHeat::Uniform(v) => vec![si_q(*v); nz],
                AxialHeat::PerCell(v) => {
                    if v.len() != nz {
                        return Err(invalid("per-cell heat list length differs from n_axial"));
                    }
                    v.iter().map(|x| si_q(*x)).collect()
                }
            };
            if qi.iter().any(|x| !x.is_finite()) {
                return Err(invalid("heat must be finite"));
            }
            area.push(a);
            dh.push(d);
            eps_d.push(e / d);
            phi.push(c.shape_factor);
            m_in.push(m);
            t_in.push(t);
            q.push(qi);
        }
        let mut gaps = Vec::with_capacity(ng);
        for g in &p.gaps {
            if g.from >= nc || g.to >= nc || g.from == g.to {
                return Err(invalid("a gap names a missing channel or joins a channel to itself"));
            }
            let s = si_len(g.width);
            let l = si_len(g.centroid_distance);
            if !(positive(s) && positive(l)) {
                return Err(invalid("gap width and centroid distance must be positive"));
            }
            if !(g.loss_coefficient.is_finite() && g.loss_coefficient >= 0.0) {
                return Err(invalid("gap loss coefficient must be non-negative"));
            }
            gaps.push(RawGap {
                i: g.from,
                j: g.to,
                s,
                l,
                k: g.loss_coefficient,
            });
        }
        let (beta, f_t) = match p.mixing {
            Some(m) => {
                if !(m.beta.is_finite() && m.beta >= 0.0) {
                    return Err(invalid("mixing coefficient beta must be non-negative"));
                }
                if !m.momentum_factor.is_finite() {
                    return Err(invalid("momentum mixing factor must be finite"));
                }
                (m.beta, m.momentum_factor)
            }
            None => (0.0, 0.0),
        };
        let g = si_g(p.gravity_along_flow);
        let p_out = si_p(p.outlet_pressure);
        if !(g.is_finite() && p_out.is_finite()) {
            return Err(invalid("gravity and outlet pressure must be finite"));
        }
        let m_ref = m_in.iter().sum::<f64>() / nc as f64;
        let t_ref = t_in.iter().cloned().fold(0.0, f64::max);
        let p_ref = p_out.abs().max(rho * (g.abs() + 1.0) * length).max(1.0);
        Ok(Self {
            nc,
            ng,
            nz,
            dz: length / nz as f64,
            rho,
            cp,
            mu,
            g,
            area,
            dh,
            eps_d,
            phi,
            m_in,
            t_in,
            q,
            gaps,
            beta,
            f_t,
            p_out,
            m_ref,
            t_ref,
            p_ref,
            w_ref: m_ref / length,
        })
    }

    fn block(&self) -> usize {
        3 * self.nc + self.ng
    }

    fn n(&self) -> usize {
        self.block() * self.nz
    }

    /// `ṁ_i` at node `k`.
    fn m(&self, x: &[f64], k: usize, i: usize) -> f64 {
        if k == 0 {
            self.m_in[i]
        } else {
            x[(k - 1) * self.block() + i] * self.m_ref
        }
    }

    /// `T_i` at node `k`.
    fn t(&self, x: &[f64], k: usize, i: usize) -> f64 {
        if k == 0 {
            self.t_in[i]
        } else {
            x[(k - 1) * self.block() + self.nc + i] * self.t_ref
        }
    }

    /// `p_i` at node `k`.
    fn p(&self, x: &[f64], k: usize, i: usize) -> f64 {
        if k == self.nz {
            self.p_out
        } else {
            x[k * self.block() + 2 * self.nc + i] * self.p_ref
        }
    }

    /// `w_g` in cell `k`.
    fn w(&self, x: &[f64], k: usize, g: usize) -> f64 {
        x[k * self.block() + 3 * self.nc + g] * self.w_ref
    }

    /// Mean axial velocity of cell `k` in channel `i`.
    fn ubar(&self, x: &[f64], k: usize, i: usize) -> f64 {
        0.5 * (self.m(x, k, i) + self.m(x, k + 1, i)) / (self.rho * self.area[i])
    }

    /// Turbulent mixing rate `w' = β s |Ḡ|` of gap `g` in cell `k`.
    fn wprime(&self, x: &[f64], k: usize, g: usize) -> f64 {
        let gp = self.gaps[g];
        let gi = 0.5 * (self.m(x, k, gp.i) + self.m(x, k + 1, gp.i)) / self.area[gp.i];
        let gj = 0.5 * (self.m(x, k, gp.j) + self.m(x, k + 1, gp.j)) / self.area[gp.j];
        self.beta * gp.s * (0.5 * (gi + gj)).abs()
    }

    /// The scaled residual rows of cell `k` into `out` (length `block`).
    fn cell_residual(&self, x: &[f64], k: usize, out: &mut [f64]) {
        let nc = self.nc;
        let (rho, cp, dz) = (self.rho, self.cp, self.dz);
        // Channel-local terms.
        for i in 0..nc {
            let (m0, m1) = (self.m(x, k, i), self.m(x, k + 1, i));
            let (t0, t1) = (self.t(x, k, i), self.t(x, k + 1, i));
            let (p0, p1) = (self.p(x, k, i), self.p(x, k + 1, i));
            let a = self.area[i];
            let ub = self.ubar(x, k, i);
            let af = wall_friction_acceleration(ub, rho, self.mu, self.dh[i], self.eps_d[i], self.phi[i]);
            out[i] = m1 - m0;
            out[nc + i] = cp * (m1 * t1 - m0 * t0) - self.q[i][k] * dz;
            out[2 * nc + i] =
                (m1 * m1 - m0 * m0) / (rho * a) + a * (p1 - p0) + a * dz * rho * (af + self.g);
        }
        // Gap exchange terms.
        for g in 0..self.ng {
            let gp = self.gaps[g];
            let (i, j) = (gp.i, gp.j);
            let w = self.w(x, k, g);
            let wp = self.wprime(x, k, g);
            let tbi = 0.5 * (self.t(x, k, i) + self.t(x, k + 1, i));
            let tbj = 0.5 * (self.t(x, k, j) + self.t(x, k + 1, j));
            let ubi = self.ubar(x, k, i);
            let ubj = self.ubar(x, k, j);
            let (td, ud) = if w >= 0.0 { (tbi, ubi) } else { (tbj, ubj) };
            let mass = dz * w;
            let energy = dz * (w * cp * td + wp * cp * (tbi - tbj));
            let mom = dz * (w * ud + self.f_t * wp * (ubi - ubj));
            out[i] += mass;
            out[j] -= mass;
            out[nc + i] += energy;
            out[nc + j] -= energy;
            out[2 * nc + i] += mom;
            out[2 * nc + j] -= mom;
            // Lateral momentum.
            let ug = 0.5 * (ubi + ubj);
            let flux_prev = if k == 0 {
                0.0
            } else {
                0.5 * (self.ubar(x, k - 1, i) + self.ubar(x, k - 1, j)) * self.w(x, k - 1, g)
            };
            let pbi = 0.5 * (self.p(x, k, i) + self.p(x, k + 1, i));
            let pbj = 0.5 * (self.p(x, k, j) + self.p(x, k + 1, j));
            let sl = gp.s / gp.l;
            let lat = (ug * w - flux_prev) / dz - sl * (pbi - pbj)
                + gp.k * w * w.abs() / (2.0 * rho * gp.s * gp.l);
            out[3 * nc + g] = lat / (sl * self.p_ref);
        }
        // Row scaling.
        for i in 0..nc {
            out[i] /= self.m_ref;
            out[nc + i] /= self.m_ref * cp * self.t_ref;
            out[2 * nc + i] /= self.area[i] * self.p_ref;
        }
    }

    /// The full scaled residual.
    fn residual(&self, x: &[f64], out: &mut [f64]) {
        let b = self.block();
        for k in 0..self.nz {
            self.cell_residual(x, k, &mut out[k * b..(k + 1) * b]);
        }
    }

    /// The initial guess: inlet flow and temperature everywhere, no
    /// crossflow, and a pressure falling linearly at the bundle-mean
    /// friction-plus-gravity gradient.
    fn initial_guess(&self) -> Vec<f64> {
        let b = self.block();
        let mut grad = 0.0;
        for i in 0..self.nc {
            let u = self.m_in[i] / (self.rho * self.area[i]);
            let af = wall_friction_acceleration(u, self.rho, self.mu, self.dh[i], self.eps_d[i], self.phi[i]);
            grad += self.rho * (af + self.g);
        }
        grad /= self.nc as f64;
        let mut x = vec![0.0; self.n()];
        for k in 0..self.nz {
            for i in 0..self.nc {
                x[k * b + i] = self.m_in[i] / self.m_ref;
                x[k * b + self.nc + i] = self.t_in[i] / self.t_ref;
                let zk = k as f64 * self.dz;
                let len = self.nz as f64 * self.dz;
                x[k * b + 2 * self.nc + i] = (self.p_out + grad * (len - zk)) / self.p_ref;
            }
        }
        x
    }

    /// Unpack a converged state into the public solution.
    pub(super) fn unpack(&self, x: &[f64], iterations: usize, residual: f64) -> SubchannelSolution {
        let nodes = self.nz + 1;
        let z = (0..nodes)
            .map(|k| Length::new::<meter>(k as f64 * self.dz))
            .collect();
        let mut mass_flow = Vec::with_capacity(self.nc);
        let mut temperature = Vec::with_capacity(self.nc);
        let mut pressure = Vec::with_capacity(self.nc);
        for i in 0..self.nc {
            mass_flow.push(
                (0..nodes)
                    .map(|k| MassRate::new::<kilogram_per_second>(self.m(x, k, i)))
                    .collect(),
            );
            temperature.push(
                (0..nodes)
                    .map(|k| ThermodynamicTemperature::new::<kelvin>(self.t(x, k, i)))
                    .collect(),
            );
            pressure.push(
                (0..nodes)
                    .map(|k| Pressure::new::<pascal>(self.p(x, k, i)))
                    .collect(),
            );
        }
        let crossflow = (0..self.ng)
            .map(|g| {
                (0..self.nz)
                    .map(|k| LinearMassRate::new::<pascal_second>(self.w(x, k, g)))
                    .collect()
            })
            .collect();
        let turbulent_mixing = (0..self.ng)
            .map(|g| {
                (0..self.nz)
                    .map(|k| LinearMassRate::new::<pascal_second>(self.wprime(x, k, g)))
                    .collect()
            })
            .collect();
        SubchannelSolution {
            z,
            mass_flow,
            temperature,
            pressure,
            crossflow,
            turbulent_mixing,
            newton_iterations: iterations,
            residual_inf_norm: residual,
        }
    }
}

fn inf_norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |a, x| a.max(x.abs()))
}

/// Forward-difference Jacobian as `(row, col, value)` triplets, by column
/// colouring on the block band: a column in block `m` touches only the rows
/// of blocks `m−1..=m+2`, so columns four blocks apart are perturbed in one
/// residual evaluation (`4 × block` evaluations in all, not `n`). The step is
/// GSL's (`multiroots/fdjac.c`, as ported in
/// [`petir::multiroots::fdjacobian`]): `h = ε |x_j|`, or `ε` when `x_j = 0`,
/// with `ε = GSL_SQRT_DBL_EPSILON`.
fn jacobian_triplets(raw: &Raw, x: &[f64], r: &[f64]) -> Vec<(usize, usize, f64)> {
    let b = raw.block();
    let nz = raw.nz;
    let n = raw.n();
    let mut trip = Vec::new();
    let mut xp = x.to_vec();
    let mut rp = vec![0.0; n];
    let mut h = vec![0.0; nz];
    for phase in 0..4 {
        for loc in 0..b {
            let mut m = phase;
            while m < nz {
                let j = m * b + loc;
                let hj = if x[j] == 0.0 {
                    GSL_SQRT_DBL_EPSILON
                } else {
                    GSL_SQRT_DBL_EPSILON * x[j].abs()
                };
                // The representable step, so the quotient divides by the
                // perturbation actually applied.
                xp[j] = x[j] + hj;
                h[m] = xp[j] - x[j];
                m += 4;
            }
            raw.residual(&xp, &mut rp);
            let mut m = phase;
            while m < nz {
                let j = m * b + loc;
                let lo = m.saturating_sub(1);
                let hi = (m + 2).min(nz - 1);
                for row in lo * b..(hi + 1) * b {
                    let d = (rp[row] - r[row]) / h[m];
                    if d != 0.0 {
                        trip.push((row, j, d));
                    }
                }
                xp[j] = x[j];
                m += 4;
            }
        }
    }
    trip
}

fn numerical(msg: String) -> TampinesError {
    TampinesError::Numerical(format!("subchannel: {msg}"))
}

/// Newton's method with backtracking on the scaled residual. Returns the
/// scaled state, the iterations taken and the final residual norm.
pub(super) fn newton(raw: &Raw) -> Result<(Vec<f64>, usize, f64), TampinesError> {
    let n = raw.n();
    let mut x = raw.initial_guess();
    let mut r = vec![0.0; n];
    raw.residual(&x, &mut r);
    let mut rn = inf_norm(&r);
    let mut xt = vec![0.0; n];
    let mut rt = vec![0.0; n];
    let mut iterations = 0;
    while rn > RESIDUAL_TOL && iterations < MAX_ITER {
        iterations += 1;
        let trip = jacobian_triplets(raw, &x, &r);
        let jac = CscMatrix::try_from_triplets(n, n, &trip)
            .map_err(|e| numerical(format!("Jacobian assembly: {e}")))?;
        let lu = SparseLu::factorize(&jac).map_err(|e| numerical(format!("LU: {e}")))?;
        let rhs: Vec<f64> = r.iter().map(|v| -v).collect();
        let dx = lu.solve(&rhs).map_err(|e| numerical(format!("LU solve: {e}")))?;
        let mut lambda = 1.0;
        let mut accepted = false;
        while lambda >= 1.0 / 1024.0 {
            for q in 0..n {
                xt[q] = x[q] + lambda * dx[q];
            }
            raw.residual(&xt, &mut rt);
            let rtn = inf_norm(&rt);
            if rtn.is_finite() && rtn < rn {
                core::mem::swap(&mut x, &mut xt);
                core::mem::swap(&mut r, &mut rt);
                rn = rtn;
                accepted = true;
                break;
            }
            lambda *= 0.5;
        }
        if !accepted {
            // No decrease along the Newton direction: at round-off, or stuck.
            break;
        }
    }
    if rn <= ACCEPT_TOL {
        Ok((x, iterations, rn))
    } else {
        Err(numerical(format!(
            "Newton did not converge: residual {rn:e} after {iterations} iterations"
        )))
    }
}
