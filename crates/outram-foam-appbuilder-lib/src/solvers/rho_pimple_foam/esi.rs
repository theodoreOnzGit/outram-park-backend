// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), ESI/OpenCFD release v2506
// Copyright (C) 2011-2017 OpenFOAM Foundation
// Copyright (C) 2015-2024 OpenCFD Ltd.
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

//! # `esi` — rhoPimpleFoam as OpenFOAM-v2506 (ESI) writes it (gh:#1363, row 3a)
//!
//! A translation of ESI v2506 `applications/solvers/compressible/rhoPimpleFoam`
//! (`rhoPimpleFoam.C`, `createFields.H`, `UEqn.H`, `EEqn.H`, `pEqn.H` -- the
//! non-transonic, non-consistent branch) for the same restricted case as
//! [`crate::solvers::sonic_foam::esi`]: a one-dimensional mesh along x, closed
//! `zeroGradient` ends, `mu = 0`, Euler ddt, `upwind` for `U`, `limitedLinear 1`
//! for `h` and `K`, `smoothSolver symGaussSeidel`, `sensibleEnthalpy`
//! (`h`-form, so the `-dpdt` term runs). It reuses that module's translated
//! operators and linear solver; what is new here is the energy variable `h`
//! (`Hs = Cp (T - Tref)`, Newton with `dFdT = Cp`), `dpdt = fvc::ddt(p)`, the
//! compressibility pressure equation
//! `ddt(rho) + psi*correction(ddt(p)) + div(phiHbyA) - laplacian(rhorAUf, p)`
//! (a symmetric matrix) and the end-of-`pEqn` `rho = thermo.rho()`.
//!
//! The existing [`super::RhoPimpleFoam::step`] is not changed and stays callable.
//! Refusals are by name ([`AppBuilderError::UnsupportedScheme`]). The V&V
//! record is `verification_and_validation/rho_pimple_foam_vs_openfoam.md`.

use crate::error::AppBuilderError;
use crate::solvers::rho_central_foam::PerfectGas;
use crate::solvers::sonic_foam::esi::{
    build_topo, dot3, fvc_div, fvm_ddt, fvm_ddt_rho, fvm_div, grad3, interpolate_linear,
    limited_linear_weights, lin, solve_matrix, surface_integrate, unsupported, upwind_weights,
    Matrix, SolveLog, StageDump, StepLog, Topo, SMALL,
};
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use std::sync::Arc;

/// `hConstThermo::Hs` for a perfect gas: `Cp (T - Tref) + Hsref + H_eos`, with
/// `Hsref = 0` and `perfectGas::H = 0`.
fn hs(gas: &PerfectGas, t: f64) -> f64 {
    gas.cp * (t - gas.t_ref) + 0.0 + 0.0
}

/// `species::thermo::T` with `F = Hs`, `dFdT = Cp` (the `sensibleEnthalpy` form).
fn the_h(gas: &PerfectGas, h: f64, t0: f64) -> Result<f64, AppBuilderError> {
    if t0 < 0.0 {
        return Err(AppBuilderError::Diverged { iter: 0, residual: t0 });
    }
    let t_tol = t0 * 1.0e-4;
    let mut t_new = t0;
    let mut iter = 0;
    loop {
        let t_est = t_new;
        t_new = t_est - (hs(gas, t_est) - h) / (gas.cp + 0.0);
        let over = iter > 100;
        iter += 1;
        if over {
            return Err(AppBuilderError::Diverged { iter, residual: (t_new - t_est).abs() });
        }
        if (t_new - t_est).abs() <= t_tol {
            return Ok(t_new);
        }
    }
}

/// rhoPimpleFoam (ESI v2506) on a 1-D closed tube. See the module documentation.
#[derive(Debug, Clone)]
pub struct RhoPimpleFoamEsi {
    topo: Topo,
    gas: PerfectGas,
    dt: f64,
    n_outer: usize,
    tolerance: f64,
    /// Density [kg/m3].
    pub rho: Vec<f64>,
    /// x velocity [m/s].
    pub ux: Vec<f64>,
    /// Pressure [Pa].
    pub p: Vec<f64>,
    /// Temperature [K].
    pub t: Vec<f64>,
    /// Specific sensible enthalpy [J/kg].
    pub h: Vec<f64>,
    /// Compressibility psi = 1/(R T).
    pub psi: Vec<f64>,
    /// Mass flux through the internal faces [kg/s].
    pub phi: Vec<f64>,
    uy: Vec<f64>,
    uz: Vec<f64>,
    uy0: Vec<f64>,
    uz0: Vec<f64>,
    k: Vec<f64>,
    dpdt: Vec<f64>,
    phi_b: Vec<f64>,
    phi0_b: Vec<f64>,
    rho_b: Vec<f64>,
    rho0_b: Vec<f64>,
    rho0: Vec<f64>,
    ux0: Vec<f64>,
    k0: Vec<f64>,
    h0: Vec<f64>,
    p0: Vec<f64>,
    phi0: Vec<f64>,
    step_index: usize,
    /// Intermediate fields of the steps in `[dump_stage_from, dump_stage_steps]`.
    pub dump_stage_steps: usize,
    /// First step recorded in [`Self::stages`].
    pub dump_stage_from: usize,
    /// Recorded intermediate fields.
    pub stages: StageDump,
}

impl RhoPimpleFoamEsi {
    /// Upstream `createFields.H` and the `hePsiThermo` constructor from `p`,
    /// `T`, `U_x`: `h = Hs(T)`, `T = THE(h, p, T)`, `psi`, `rho = p psi`,
    /// `phi = linearInterpolate(rho U) & Sf`, `K`, and `dpdt = fvc::ddt(p)`
    /// (zero: `p.oldTime()` is created by that very call).
    ///
    /// # Errors
    /// Refusals of [`build_topo`]; [`AppBuilderError::UnsupportedScheme`] for a
    /// viscous gas; [`AppBuilderError::Diverged`] from the temperature iteration.
    pub fn new(
        mesh: &Arc<FvMesh>,
        gas: PerfectGas,
        mu: f64,
        dt: f64,
        p: &[f64],
        t: &[f64],
        ux: &[f64],
    ) -> Result<Self, AppBuilderError> {
        if mu != 0.0 {
            return Err(unsupported("mu", "a viscous gas needs divDevRhoReff and alphaEff, not translated"));
        }
        let topo = build_topo(mesh)?;
        let n = topo.n;
        let mut h = Vec::with_capacity(n);
        for c in 0..n {
            h.push(hs(&gas, t[c]));
        }
        let mut tt = t.to_vec();
        let mut psi = vec![0.0; n];
        for c in 0..n {
            tt[c] = the_h(&gas, h[c], tt[c])?;
            psi[c] = gas.psi(tt[c]);
        }
        let rho: Vec<f64> = (0..n).map(|c| p[c] * psi[c]).collect();
        let rho_u: Vec<f64> = (0..n).map(|c| rho[c] * ux[c]).collect();
        let phi: Vec<f64> = (0..topo.nf)
            .map(|f| lin(topo.w[f], &rho_u, topo.owner[f], topo.nei[f]) * topo.sfx[f])
            .collect();
        let rho_b: Vec<f64> = topo.bfaces.iter().map(|&(c, _)| p[c] * psi[c]).collect();
        let phi_b: Vec<f64> = topo
            .bfaces
            .iter()
            .enumerate()
            .map(|(b, &(c, sf))| {
                let (rux, ruy, ruz) = (rho_b[b] * ux[c], rho_b[b] * 0.0, rho_b[b] * 0.0);
                sf[0] * rux + sf[1] * ruy + sf[2] * ruz
            })
            .collect();
        let k: Vec<f64> = (0..n).map(|c| 0.5 * (ux[c] * ux[c] + 0.0 * 0.0 + 0.0 * 0.0)).collect();
        Ok(Self {
            gas,
            dt,
            n_outer: 2,
            tolerance: 1.0e-15,
            rho0: rho.clone(),
            ux0: ux.to_vec(),
            k0: k.clone(),
            h0: h.clone(),
            p0: p.to_vec(),
            phi0: phi.clone(),
            phi0_b: phi_b.clone(),
            rho0_b: rho_b.clone(),
            dpdt: vec![0.0; n],
            uy: vec![0.0; n],
            uz: vec![0.0; n],
            uy0: vec![0.0; n],
            uz0: vec![0.0; n],
            rho,
            ux: ux.to_vec(),
            p: p.to_vec(),
            t: tt,
            h,
            psi,
            phi,
            k,
            phi_b,
            rho_b,
            topo,
            step_index: 0,
            dump_stage_steps: 0,
            dump_stage_from: 1,
            stages: Vec::new(),
        })
    }

    /// The mesh data the translation uses.
    #[must_use]
    pub fn topo(&self) -> &Topo {
        &self.topo
    }

    /// Set `nOuterCorrectors` (tutorial-style case: 2).
    pub fn set_n_outer_correctors(&mut self, n: usize) {
        self.n_outer = n.max(1);
    }

    fn record(&mut self, tag: &'static str, name: &'static str, v: &[f64]) {
        if self.step_index >= self.dump_stage_from && self.step_index <= self.dump_stage_steps {
            self.stages.push((tag, name, v.to_vec()));
        }
    }

    fn record_matrix(&mut self, tag: &'static str, m: &Matrix) {
        self.record(tag, "diag", &m.diag.clone());
        self.record(tag, "source", &m.source.clone());
        self.record(tag, "lower", &m.lower.clone().unwrap_or_default());
        self.record(tag, "upper", &m.upper.clone().unwrap_or_default());
    }

    /// `thermo.correct()`: `T = THE(h, p, T)`, `psi = 1/(R T)`.
    fn thermo_correct(&mut self) -> Result<(), AppBuilderError> {
        for c in 0..self.topo.n {
            self.t[c] = the_h(&self.gas, self.h[c], self.t[c])?;
            self.psi[c] = self.gas.psi(self.t[c]);
        }
        Ok(())
    }

    /// `rhoEqn.H` without `fvOptions`.
    fn rho_eqn(&mut self, r_dt: f64) -> SolveLog {
        let t = &self.topo;
        let mut m = fvm_ddt(t, r_dt, &self.rho0);
        let divphi = surface_integrate(t, &self.phi, &self.phi_b);
        m.add_field(t, &divphi);
        let mut rho = self.rho.clone();
        let log = solve_matrix(t, &m, &mut rho, self.tolerance);
        self.rho = rho;
        log
    }

    /// `compressibleContinuityErrs.H`; returns `(sumLocal, global)`.
    fn continuity_errors(&self) -> (f64, f64) {
        let t = &self.topo;
        let mut total_mass = 0.0;
        for c in 0..t.n {
            total_mass += t.v[c] * self.rho[c];
        }
        let mut local = 0.0;
        let mut global = 0.0;
        for c in 0..t.n {
            let d = self.rho[c] - self.p[c] * self.psi[c];
            local += t.v[c] * d.abs();
        }
        for c in 0..t.n {
            let d = self.rho[c] - self.p[c] * self.psi[c];
            global += t.v[c] * d;
        }
        (local / total_mass, global / total_mass)
    }

    /// `rho = thermo.rho()` on the cells and the boundary faces.
    fn assign_thermo_rho(&mut self) {
        for c in 0..self.topo.n {
            self.rho[c] = self.p[c] * self.psi[c];
        }
        for (b, &(c, _)) in self.topo.bfaces.iter().enumerate() {
            self.rho_b[b] = self.p[c] * self.psi[c];
        }
    }

    /// Advance one time step (`while (runTime.loop())` body of `rhoPimpleFoam.C`).
    ///
    /// # Errors
    /// [`AppBuilderError::Diverged`] from the temperature iteration.
    pub fn step(&mut self) -> Result<StepLog, AppBuilderError> {
        self.step_index += 1;
        let mut log = StepLog { solves: Vec::new(), continuity: Vec::new() };
        let r_dt = 1.0 / self.dt;
        let n = self.topo.n;
        if self.step_index > 1 {
            self.rho0 = self.rho.clone();
            self.ux0 = self.ux.clone();
            self.uy0 = self.uy.clone();
            self.uz0 = self.uz.clone();
            self.k0 = self.k.clone();
            self.h0 = self.h.clone();
            self.p0 = self.p.clone();
            self.phi0 = self.phi.clone();
            self.phi0_b = self.phi_b.clone();
            self.rho0_b = self.rho_b.clone();
        }

        for outer in 0..self.n_outer {
            let t = self.topo.clone();
            if outer == 0 {
                // pimple.firstIter(): rhoEqn.H
                log.solves.push(("rho", self.rho_eqn(r_dt)));
                let rho_dump = self.rho.clone();
                self.record("rhoEqn", "rho", &rho_dump);
            }

            // ---- UEqn.H
            let w_u = upwind_weights(&self.phi);
            let mut u_eqn = fvm_ddt_rho(&t, r_dt, &self.rho, &self.rho0, &self.ux0);
            u_eqn.add(&fvm_div(&t, &self.phi, &w_u, &self.phi_b));
            self.record_matrix("UEqn:m", &u_eqn);
            let gradp = grad3(&t, &self.p);
            let mut pred = u_eqn.clone();
            for c in 0..n {
                pred.source[c] += t.v[c] * (-gradp[c][0]);
            }
            let mut ux = self.ux.clone();
            log.solves.push(("Ux", solve_matrix(&t, &pred, &mut ux, self.tolerance)));
            self.ux = ux;
            for c in 0..n {
                self.k[c] = 0.5 * (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c] + self.uz[c] * self.uz[c]);
            }
            if self.step_index == 1 && outer == 0 {
                // K.oldTime() is first requested by fvc::ddt(rho, K) (createK.H
                // creates it only `if (U.nOldTimes())`), after the predictor
                self.k0 = self.k.clone();
            }
            let ux_dump = self.ux.clone();
            self.record("UEqn", "Ux", &ux_dump);

            // ---- EEqn.H (he is h)
            {
                let w_h = limited_linear_weights(&t, &self.phi, &self.h);
                let mut e_eqn = fvm_ddt_rho(&t, r_dt, &self.rho, &self.rho0, &self.h0);
                e_eqn.add(&fvm_div(&t, &self.phi, &w_h, &self.phi_b));
                let ddt_k: Vec<f64> = (0..n)
                    .map(|c| r_dt * (self.rho[c] * self.k[c] - self.rho0[c] * self.k0[c]))
                    .collect();
                e_eqn.add_field(&t, &ddt_k);
                let w_k = limited_linear_weights(&t, &self.phi, &self.k);
                let div_k = fvc_div(&t, &self.phi, &w_k, &self.k, &self.phi_b);
                e_eqn.add_field(&t, &div_k);
                // - dpdt: fvMatrix - volField adds V*field to the source
                for c in 0..n {
                    e_eqn.source[c] += t.v[c] * self.dpdt[c];
                }
                self.record_matrix("EEqn:m", &e_eqn);
                let mut h = self.h.clone();
                log.solves.push(("h", solve_matrix(&t, &e_eqn, &mut h, self.tolerance)));
                self.h = h;
                self.thermo_correct()?;
            }
            let (h_dump, t_dump) = (self.h.clone(), self.t.clone());
            self.record("EEqn", "h", &h_dump);
            self.record("EEqn", "T", &t_dump);

            // ---- pEqn.H (nCorrectors 1)
            {
                self.assign_thermo_rho();
                let mut d_diag = u_eqn.diag.clone();
                let mut b_term = vec![0.0_f64; n];
                for (b, &(c, _)) in t.bfaces.iter().enumerate() {
                    let ic = u_eqn.bcoef[b];
                    let cmpt_av = (ic + ic + ic) / 3.0;
                    d_diag[c] += cmpt_av;
                    b_term[c] = -(0.0 + ic) + cmpt_av;
                }
                let a: Vec<f64> = (0..n).map(|c| d_diag[c] / t.v[c]).collect();
                let r_au: Vec<f64> = a.iter().map(|x| 1.0 / *x).collect();
                let rho_rau: Vec<f64> = (0..n).map(|c| self.rho[c] * r_au[c]).collect();
                let rhor_auf = interpolate_linear(&t, &rho_rau);
                let (lo, up) = (
                    u_eqn.lower.as_ref().expect("asymmetric"),
                    u_eqn.upper.as_ref().expect("asymmetric"),
                );
                let mut hh = vec![0.0; n];
                for f in 0..t.nf {
                    hh[t.nei[f]] -= lo[f] * self.ux[t.owner[f]];
                    hh[t.owner[f]] -= up[f] * self.ux[t.nei[f]];
                }
                let hbya: Vec<f64> = (0..n)
                    .map(|c| {
                        let hc = (b_term[c] * self.ux[c] + (hh[c] + u_eqn.source[c])) / t.v[c];
                        r_au[c] * hc
                    })
                    .collect();
                // ddtCorr(rho, U, phi)
                let rho_u0: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.ux0[c]).collect();
                let rho_u0y: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.uy0[c]).collect();
                let rho_u0z: Vec<f64> = (0..n).map(|c| self.rho0[c] * self.uz0[c]).collect();
                let rho_f0 = interpolate_linear(&t, &rho_u0);
                let rho_f0y = interpolate_linear(&t, &rho_u0y);
                let rho_f0z = interpolate_linear(&t, &rho_u0z);
                let rho_f = interpolate_linear(&t, &self.rho);
                let hbya_f = interpolate_linear(&t, &hbya);
                // phiHbyA = interpolate(rho)*flux(HbyA) + rhorAUf*ddtCorr
                let mut phi_hbya = Vec::with_capacity(t.nf);
                for f in 0..t.nf {
                    let phi_corr = self.phi0[f]
                        - (t.sfx[f] * rho_f0[f] + t.sfy[f] * rho_f0y[f] + t.sfz[f] * rho_f0z[f]);
                    let m = (phi_corr.abs() / (self.phi0[f].abs() + SMALL)).min(1.0);
                    let coeff = 1.0 - m;
                    let ddt_corr = coeff * r_dt * phi_corr;
                    let flux_hbya = t.sfx[f] * hbya_f[f] + t.sfy[f] * 0.0 + t.sfz[f] * 0.0;
                    phi_hbya.push(rho_f[f] * flux_hbya + rhor_auf[f] * ddt_corr);
                }
                let mut phi_hbya_b = Vec::with_capacity(t.bfaces.len());
                for (b, &(c, sf)) in t.bfaces.iter().enumerate() {
                    let flux_h = sf[0] * hbya[c] + sf[1] * 0.0 + sf[2] * 0.0;
                    let rhor_auf_b = self.rho_b[b] * r_au[c];
                    let rho_u0b = [
                        self.rho0_b[b] * self.ux0[c],
                        self.rho0_b[b] * self.uy0[c],
                        self.rho0_b[b] * self.uz0[c],
                    ];
                    let phi_corr = self.phi0_b[b] - dot3(sf, rho_u0b);
                    // boundary coefficient 0 (rhoU0 has calculated patches, which fixesValue)
                    let ddt_corr = 0.0 * r_dt * phi_corr;
                    phi_hbya_b.push(self.rho_b[b] * flux_h + rhor_auf_b * ddt_corr);
                }
                self.record("pEqn:rAU", "rAU", &r_au);
                self.record("pEqn:HbyA", "Ux", &hbya);
                self.record("pEqn:phiHbyA", "phiHbyA", &phi_hbya);
                self.record("pEqn:rhorAUf", "rhorAUf", &rhor_auf);

                // pDDtEqn = fvc::ddt(rho) + psi*correction(fvm::ddt(p)) + fvc::div(phiHbyA)
                let m_ddt = fvm_ddt(&t, r_dt, &self.p0);
                let mut corr_source = m_ddt.source.clone();
                for c in 0..n {
                    // (A & p) = (-diag*p + (0 + source))/(-V); A - field: source += V*field
                    let mphi = (-m_ddt.diag[c]) * self.p[c] + (0.0 + m_ddt.source[c]);
                    let f_c = mphi / (-t.v[c]);
                    corr_source[c] += t.v[c] * f_c;
                }
                let mut diag: Vec<f64> = (0..n).map(|c| m_ddt.diag[c] * self.psi[c]).collect();
                let mut source: Vec<f64> = (0..n).map(|c| corr_source[c] * self.psi[c]).collect();
                let ddt_rho: Vec<f64> = (0..n).map(|c| r_dt * (self.rho[c] - self.rho0[c])).collect();
                for c in 0..n {
                    source[c] -= t.v[c] * ddt_rho[c];
                }
                let div_phi_hbya = surface_integrate(&t, &phi_hbya, &phi_hbya_b);
                for c in 0..n {
                    source[c] -= t.v[c] * div_phi_hbya[c];
                }
                // pEqn = pDDtEqn - fvm::laplacian(rhorAUf, p): symmetric, upper = -B.upper
                let lap_upper: Vec<f64> =
                    (0..t.nf).map(|f| t.dc[f] * (rhor_auf[f] * t.mag_sf[f])).collect();
                let mut lap_diag = vec![0.0; n];
                for f in 0..t.nf {
                    lap_diag[t.owner[f]] -= lap_upper[f];
                    lap_diag[t.nei[f]] -= lap_upper[f];
                }
                for c in 0..n {
                    diag[c] -= lap_diag[c];
                }
                let neg_upper: Vec<f64> = lap_upper.iter().map(|x| -*x).collect();
                let p_eqn = Matrix {
                    diag,
                    lower: Some(neg_upper.clone()),
                    upper: Some(neg_upper),
                    source,
                    bcoef: vec![0.0; t.bfaces.len()],
                };
                self.record_matrix("pEqn:m", &p_eqn);
                let mut p = self.p.clone();
                log.solves.push(("p", solve_matrix(&t, &p_eqn, &mut p, self.tolerance)));
                self.p = p;
                // phi = phiHbyA + pEqn.flux(); boundary flux of pEqn is zero
                let up = p_eqn.upper.as_ref().expect("symmetric");
                self.phi = (0..t.nf)
                    .map(|f| phi_hbya[f] + (up[f] * self.p[t.nei[f]] - up[f] * self.p[t.owner[f]]))
                    .collect();
                for b in 0..t.bfaces.len() {
                    self.phi_b[b] = phi_hbya_b[b] + 0.0;
                }

                // U = HbyA - rAU*grad(p); K
                let gradp = grad3(&t, &self.p);
                for c in 0..n {
                    self.ux[c] = hbya[c] - r_au[c] * gradp[c][0];
                    self.uy[c] = 0.0 - r_au[c] * gradp[c][1];
                    self.uz[c] = 0.0 - r_au[c] * gradp[c][2];
                    self.k[c] = 0.5 * (self.ux[c] * self.ux[c] + self.uy[c] * self.uy[c] + self.uz[c] * self.uz[c]);
                }

                // rhoEqn.H, continuity errors, rho = thermo.rho(), dpdt
                log.solves.push(("rho", self.rho_eqn(r_dt)));
                log.continuity.push(self.continuity_errors());
                self.assign_thermo_rho();
                for c in 0..n {
                    self.dpdt[c] = r_dt * (self.p[c] - self.p0[c]);
                }
                let (p_dump, phi_dump, u_dump) = (self.p.clone(), self.phi.clone(), self.ux.clone());
                self.record("pEqn", "p", &p_dump);
                self.record("pEqn", "phi", &phi_dump);
                self.record("pEqn", "Ux", &u_dump);
            }
        }
        self.assign_thermo_rho();
        Ok(log)
    }
}
