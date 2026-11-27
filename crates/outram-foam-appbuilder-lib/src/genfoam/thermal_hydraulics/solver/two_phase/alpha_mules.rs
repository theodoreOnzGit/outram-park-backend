// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Files: src/classes/thermalHydraulics/solvers/twoPhase/include/{alphaEqns_2p.H,
//          alphaEqnsRoutines/solveAlphaMULES.H}, solvers/twoPhase/twoPhase.C (correctCourant)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! The `MULES` branch of `alphaEqns_2p.H` (`solveAlphaMULES.H`, `solvePhase both`),
//! gh:#1361 round 7. Statement order follows upstream; see [`super::mules`] for the numerics.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::mules::{self, AlphaConvection};
use super::{fv, MulesControls, TwoPhaseSolver};

/// OpenFOAM `SMALL`.
const SMALL: f64 = 1.0e-15;

fn fmin(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

fn fmax(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

impl TwoPhaseSolver {
    /// `twoPhase::correctCourant` as `CoNum_` is left (`max(CoNum, UrCoNum)`), in upstream's operation order.
    pub(super) fn co_num_upstream(&self) -> f64 {
        let g = &self.geom;
        let nf = g.n_faces();
        let abs_phi: Vec<f64> = self.phi.iter().map(|v| v.abs()).collect();
        let sum = fv::surface_sum_mag(g, &abs_phi);
        let rel: Vec<f64> = (0..nf).map(|f| (self.fluids[0].phi[f] - self.fluids[1].phi[f]).abs()).collect();
        let sum_r = fv::surface_sum_mag(g, &rel);
        let mut co = f64::MIN;
        let mut ur = f64::MIN;
        for c in 0..g.n_cells() {
            let mv = self.moving(c);
            let v = g.mesh.cell_volumes[c];
            co = fmax(co, (sum[c] / mv) / v);
            ur = fmax(ur, (sum_r[c] / mv) / v);
        }
        let co = 0.5 * co * self.dt;
        let ur = 0.5 * ur * self.dt;
        fmax(co, ur)
    }

    /// `phase1 and phase2` of `twoPhase::maxDeltaT`: both fluids have `max(alpha) >= 1e-6` somewhere.
    pub(super) fn phases_present(&self) -> bool {
        let p = |k: usize| self.fluids[k].alpha.c.iter().cloned().fold(f64::MIN, f64::max) >= 1e-6;
        p(0) && p(1)
    }

    /// `correctInflowOutflow(alpha, phi, alphaPhi)`: non-coupled boundary faces take `phi_b alpha_b`.
    fn mules_correct_inflow_outflow(&self, k: usize, ap: &mut [f64]) {
        let ni = self.geom.n_internal();
        for f in ni..self.geom.n_faces() {
            if self.geom.active[f] {
                ap[f] = self.fluids[k].phi[f] * self.fluids[k].alpha.b[f - ni];
            }
        }
    }

    /// `alphaPhic<k>` of `solveAlphas`: three convective fluxes, summed left to right.
    fn mules_alpha_phic(&self, mc: &MulesControls, k: usize) -> Vec<f64> {
        self.mules_alpha_phic_with(mc, k, &self.fluids[1 - k].alpha)
    }

    /// [`Self::mules_alpha_phic`] with the field the relative flux carries given explicitly: `alpha_o` of
    /// `solveAlphas`, `movingAlpha_ - solveFluid` of the `solveAlpha` lambda (solveAlphaMULES.H:31-37).
    fn mules_alpha_phic_with(&self, mc: &MulesControls, k: usize, ao: &fv::Scalar) -> Vec<f64> {
        let g = &self.geom;
        let nf = g.n_faces();
        let o = 1 - k;
        let ak = &self.fluids[k].alpha;
        let phik = &self.fluids[k].phi;
        let phio = &self.fluids[o].phi;
        let x = mules::flux(g, mc.phi_scheme, &self.phi, ak);
        // fvc::flux(-fvc::flux(-(phik - phio), alpha_o, phir), alpha_k, phir)
        let neg_rel: Vec<f64> = (0..nf).map(|f| -(phik[f] - phio[f])).collect();
        let a = mules::flux(g, mc.phir_scheme, &neg_rel, ao);
        let neg_a: Vec<f64> = a.iter().map(|v| -v).collect();
        let b = mules::flux(g, mc.phir_scheme, &neg_a, ak);
        // fvc::flux(-fvc::flux(-phik, alphas, phir), alpha_k, phir)
        let neg_phik: Vec<f64> = phik.iter().map(|v| -v).collect();
        let a2 = mules::flux(g, mc.phir_scheme, &neg_phik, &self.structure.alpha);
        let neg_a2: Vec<f64> = a2.iter().map(|v| -v).collect();
        let c = mules::flux(g, mc.phir_scheme, &neg_a2, ak);
        (0..nf).map(|f| (x[f] + b[f]) + c[f]).collect()
    }

    /// `Sp`, `Su` of fluid `k` from `dgdt` (`minAlphaSp` branch of `solveAlphaMULES.H`).
    fn mules_sp_su(&self, mc: &MulesControls, k: usize) -> (Vec<f64>, Vec<f64>) {
        let n = self.geom.n_cells();
        let mut sp = vec![0.0; n];
        let mut su = vec![0.0; n];
        for c in 0..n {
            let d = self.fluids[k].dgdt[c];
            if mc.min_alpha_sp != 0.0 {
                if d < 0.0 {
                    sp[c] += d / fmax(self.fluids[k].alpha.c[c], mc.min_alpha_sp);
                } else {
                    su[c] += d;
                }
            } else {
                su[c] += d;
            }
        }
        (sp, su)
    }

    /// The `explicitSolve` of fluid `k`, `psi.relax()`, `correctBoundaryConditions`, then
    /// `alpha = min(max(alpha, 0), 1)` on cells and (non-fixed) boundary values.
    fn mules_explicit_solve(&mut self, mc: &MulesControls, k: usize, r_delta_t: f64, ap: &[f64], sp: &[f64], su: &[f64]) {
        let g = self.geom.clone();
        let prev = self.fluids[k].alpha.c.clone();
        let psi0 = self.fluids[k].alpha_old.clone();
        let mut x = prev.clone();
        mules::explicit_update(&g, r_delta_t, &mut x, &psi0, ap, sp, su);
        if let Some(fa) = mc.relax[k] {
            for c in 0..x.len() {
                x[c] = prev[c] + fa * (x[c] - prev[c]);
            }
        }
        let a = &mut self.fluids[k].alpha;
        a.c = x;
        a.correct_bc(&g);
        for v in a.c.iter_mut() {
            *v = fmin(fmax(*v, 0.0), 1.0);
        }
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if g.active[f] && !matches!(a.bc[g.patch_of[f - ni]], fv::Bc::Fixed(_)) {
                a.b[f - ni] = fmin(fmax(a.b[f - ni], 0.0), 1.0);
            }
        }
    }

    /// One `solveAlphas(rDeltaT)` of `solveAlphaMULES.H` (`solvePhase both`).
    fn mules_solve_both(&mut self, mc: &MulesControls, r_delta_t: f64) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let psi_max: Vec<f64> = (0..n).map(|c| fmin(self.moving(c), 1.0)).collect();

        let mut ap1 = self.mules_alpha_phic(mc, 0);
        self.mules_correct_inflow_outflow(0, &mut ap1);
        let a1_old = self.fluids[0].alpha_old.clone();
        let phi = self.phi.clone();
        mules::limit_return_corr(&g, &mc.limiter, r_delta_t, &self.fluids[0].alpha, &a1_old, &phi, &mut ap1, &psi_max);
        let (sp1, su1) = self.mules_sp_su(mc, 0);

        let mut ap2 = self.mules_alpha_phic(mc, 1);
        self.mules_correct_inflow_outflow(1, &mut ap2);
        let a2_old = self.fluids[1].alpha_old.clone();
        mules::limit_return_corr(&g, &mc.limiter, r_delta_t, &self.fluids[1].alpha, &a2_old, &phi, &mut ap2, &psi_max);
        let (sp2, su2) = self.mules_sp_su(mc, 1);

        // Preserve phi: the corrected alphaPhis must sum to phi_.
        let a1fu = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &self.fluids[0].alpha);
        let a2fu = mules::interpolate(&g, AlphaConvection::Upwind, &phi, &self.fluids[1].alpha);
        for f in 0..nf {
            if f < ni || g.active[f] {
                let delta_phi = ((ap1[f] + ap2[f]) - phi[f]) / (a1fu[f] + a2fu[f]);
                ap1[f] -= delta_phi * a1fu[f];
                ap2[f] -= delta_phi * a2fu[f];
            }
        }

        self.mules_correct_inflow_outflow(0, &mut ap1);
        self.mules_explicit_solve(mc, 0, r_delta_t, &ap1, &sp1, &su1);
        self.fluids[0].alpha_phi = ap1;

        self.mules_correct_inflow_outflow(1, &mut ap2);
        self.mules_explicit_solve(mc, 1, r_delta_t, &ap2, &sp2, &su2);
        self.fluids[1].alpha_phi = ap2;

        // zeroSum: stabilise a vanishing sum, then renormalise to movingAlpha_.
        let nb = nf - ni;
        let zero_sum_c: Vec<f64> = (0..n)
            .map(|c| if self.fluids[0].alpha.c[c] + self.fluids[1].alpha.c[c] <= 0.0 { 1.0 } else { 0.0 })
            .collect();
        let zero_sum_b: Vec<f64> = (0..nb)
            .map(|i| if self.fluids[0].alpha.b[i] + self.fluids[1].alpha.b[i] <= 0.0 { 1.0 } else { 0.0 })
            .collect();
        for k in 0..2 {
            let old_c = self.fluids[k].alpha_old.clone();
            let old_b = self.fluids[k].alpha_old_b.clone();
            let a = &mut self.fluids[k].alpha;
            for c in 0..n {
                a.c[c] += zero_sum_c[c] * old_c[c];
            }
            for f in ni..nf {
                if g.active[f] && !matches!(a.bc[g.patch_of[f - ni]], fv::Bc::Fixed(_)) {
                    a.b[f - ni] += zero_sum_b[f - ni] * old_b[f - ni];
                }
            }
        }
        let corr_c: Vec<f64> =
            (0..n).map(|c| self.moving(c) / (self.fluids[0].alpha.c[c] + self.fluids[1].alpha.c[c])).collect();
        let corr_b: Vec<f64> = (0..nb)
            .map(|i| (1.0 - self.structure.alpha.b[i]) / (self.fluids[0].alpha.b[i] + self.fluids[1].alpha.b[i]))
            .collect();
        for k in 0..2 {
            let a = &mut self.fluids[k].alpha;
            for c in 0..n {
                a.c[c] *= corr_c[c];
            }
            for f in ni..nf {
                if g.active[f] && !matches!(a.bc[g.patch_of[f - ni]], fv::Bc::Fixed(_)) {
                    a.b[f - ni] *= corr_b[f - ni];
                }
            }
        }
        for k in 0..2 {
            self.fluids[k].alpha.correct_bc(&g);
        }
    }


    /// `movingAlpha_ - solveFluid` as a field with boundary values (`movingAlpha_ = 1 - structure_`, twoPhase.C:194).
    fn mules_moving_minus(&self, k: usize) -> fv::Scalar {
        let g = &self.geom;
        let a = &self.fluids[k].alpha;
        let s = &self.structure.alpha;
        let c: Vec<f64> = (0..g.n_cells()).map(|i| self.moving(i) - a.c[i]).collect();
        let b: Vec<f64> = (0..a.b.len()).map(|i| (1.0 - s.b[i]) - a.b[i]).collect();
        fv::Scalar { c, b, bc: g.default_bcs() }
    }

    /// The `solveAlpha` lambda of `solveAlphaMULES.H` (lines 24-122) for `solvePhase` = fluid `k`; the other fluid
    /// follows by continuity (`alphaPhi = phi_ - alphaPhi_k`, `alpha = movingAlpha_ - alpha_k`).
    fn mules_solve_one(&mut self, mc: &MulesControls, k: usize, r_delta_t: f64) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let o = 1 - k;
        let m_minus = self.mules_moving_minus(k);
        let mut ap = self.mules_alpha_phic_with(mc, k, &m_minus);
        self.mules_correct_inflow_outflow(k, &mut ap);
        let (sp, su) = self.mules_sp_su(mc, k);
        // myMULES::explicitSolve(rDeltaT, one, solveFluid, phi_, alphaPhic, Sp, Su, movingAlpha_, zeroField()):
        // psi.correctBoundaryConditions(); limit(..., returnCorr = false); explicitSolve (6 arguments)
        self.fluids[k].alpha.correct_bc(&g);
        let psi_max: Vec<f64> = (0..n).map(|c| self.moving(c)).collect();
        let psi_min = vec![0.0; n];
        let psi0 = self.fluids[k].alpha_old.clone();
        let phi = self.phi.clone();
        mules::limit_full_flux(&g, &mc.limiter, r_delta_t, &self.fluids[k].alpha, &psi0, &phi, &mut ap, &sp, &su, &psi_max, &psi_min);
        {
            let prev = self.fluids[k].alpha.c.clone();
            let mut x = prev.clone();
            mules::explicit_update(&g, r_delta_t, &mut x, &psi0, &ap, &sp, &su);
            if let Some(fa) = mc.relax[k] {
                for c in 0..n {
                    x[c] = prev[c] + fa * (x[c] - prev[c]);
                }
            }
            let a = &mut self.fluids[k].alpha;
            a.c = x;
            a.correct_bc(&g);
        }
        // solveFluid.alphaPhi() = alphaPhic; otherFluid.alphaPhi() = phi_ - solveFluid.alphaPhi();
        let mut ap_o: Vec<f64> = (0..nf).map(|f| phi[f] - ap[f]).collect();
        self.fluids[k].alpha_phi = ap;
        self.mules_correct_inflow_outflow(o, &mut ap_o);
        self.fluids[o].alpha_phi = ap_o;
        // solveFluid = min(max(solveFluid, 0), movingAlpha_); correctBoundaryConditions; other = min(max(movingAlpha_ - solveFluid, 0), movingAlpha_)
        {
            let ms: Vec<f64> = (0..n).map(|c| self.moving(c)).collect();
            let mb: Vec<f64> = (0..nf - ni).map(|i| 1.0 - self.structure.alpha.b[i]).collect();
            let a = &mut self.fluids[k].alpha;
            for c in 0..n {
                a.c[c] = fmin(fmax(a.c[c], 0.0), ms[c]);
            }
            for f in ni..nf {
                if g.active[f] && !matches!(a.bc[g.patch_of[f - ni]], fv::Bc::Fixed(_)) {
                    a.b[f - ni] = fmin(fmax(a.b[f - ni], 0.0), mb[f - ni]);
                }
            }
            a.correct_bc(&g);
            let sc = self.fluids[k].alpha.c.clone();
            let sb = self.fluids[k].alpha.b.clone();
            let b = &mut self.fluids[o].alpha;
            for c in 0..n {
                b.c[c] = fmin(fmax(ms[c] - sc[c], 0.0), ms[c]);
            }
            for f in ni..nf {
                if g.active[f] && !matches!(b.bc[g.patch_of[f - ni]], fv::Bc::Fixed(_)) {
                    b.b[f - ni] = fmin(fmax(mb[f - ni] - sb[f - ni], 0.0), mb[f - ni]);
                }
            }
        }
    }

    /// `solve(solvePhase, rDeltaT)` of solveAlphaMULES.H:365-398.
    fn mules_solve(&mut self, mc: &MulesControls, r_delta_t: f64) {
        match mc.solve_phase {
            None => self.mules_solve_both(mc, r_delta_t),
            Some(k) => self.mules_solve_one(mc, k, r_delta_t),
        }
    }

    /// `solveAlphaMULES.H`: the sub-cycle count, `solve` once or `nSubCycles` times with
    /// upstream's old-time bookkeeping, then the mass fluxes `alphaRhoPhi = interpolate(rho) alphaPhi`.
    pub(super) fn correct_alpha_mules(&mut self, mc: &MulesControls) {
        let g = self.geom.clone();
        let nf = g.n_faces();
        let n_sub = if mc.adjust_sub_cycles && self.both_phases_present {
            let want = ((self.co_num + SMALL) / mc.alpha_max_co).ceil();
            fmin(want, mc.max_sub_cycles as f64) as usize
        } else {
            mc.n_sub_cycles
        };
        self.alpha_sub_cycles = n_sub;
        let r_delta_t = n_sub as f64 / self.dt;
        if n_sub == 1 {
            self.mules_solve(mc, r_delta_t);
            if let (true, Some(k)) = (self.other_alpha_old_stale, mc.solve_phase) {
                // the other fluid's alpha.oldTime() is created at its first request, after the lambda assigned it
                let o = 1 - k;
                self.fluids[o].alpha_old = self.fluids[o].alpha.c.clone();
                self.fluids[o].alpha_old_b = self.fluids[o].alpha.b.clone();
            }
            self.other_alpha_old_stale = false;
        } else {
            // `alpha1.oldTime()` / `alpha2.oldTime()` are requested before the sub-cycle loop (solveAlphaMULES.H:473): both exist
            self.other_alpha_old_stale = false;
            let keep: Vec<(Vec<f64>, Vec<f64>)> =
                (0..2).map(|k| (self.fluids[k].alpha_old.clone(), self.fluids[k].alpha_old_b.clone())).collect();
            let mut sums = [vec![0.0; nf], vec![0.0; nf]];
            for _ in 0..n_sub {
                self.mules_solve(mc, r_delta_t);
                for k in 0..2 {
                    self.fluids[k].alpha_old = self.fluids[k].alpha.c.clone();
                    self.fluids[k].alpha_old_b = self.fluids[k].alpha.b.clone();
                    for f in 0..nf {
                        sums[k][f] += self.fluids[k].alpha_phi[f];
                    }
                }
            }
            for k in 0..2 {
                self.fluids[k].alpha_phi = sums[k].iter().map(|v| v / n_sub as f64).collect();
                self.fluids[k].alpha_old = keep[k].0.clone();
                self.fluids[k].alpha_old_b = keep[k].1.clone();
            }
        }
        for k in 0..2 {
            let rf = fv::interp_linear_openfoam(&g, &self.fluids[k].rho);
            self.fluids[k].alpha_rho_phi = (0..nf).map(|f| rf[f] * self.fluids[k].alpha_phi[f]).collect();
        }
    }
}
