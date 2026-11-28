// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/onePhase/onePhase.C (correctFluidMechanics: the
//     `momentumMode_ == faceCentered` branch), include/equations/UEqn_1p.H (`if (momentumMode_ != faceCentered) UEqn += fvm::ddt(alpha, rho, U)`),
//     include/equations/pEqnf_1p.H and pEqnRoutines/{updateAlphaPhiAndInitControls_1p, correctBoundaryFlux_1p, calcPhiSbyA_1p,
//     calcrAUfSetSnGradCalcComp_1p, solveP_rgh_1p, reconstructU_1p, updatePP_rghRho_1p}.H,
//   and OpenFOAM-ESI v2506 `fvMatrix::{operator+=, relax, A, H, flux, setReference}`.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `one_phase_cc::face_centered` — the `momentumMode faceCentered` branch of `onePhase::correctFluidMechanics`
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Upstream has ONE `onePhase` class whose `momentumMode_` selects branches (gh:#1939, README round 73): this module is the
//! `faceCentered` branch of `OnePhaseCcSolver`, so every thermo, structure, closure, boundary condition and linear solver of the
//! cell-centred port (including `GamgPolicy::Native`) serves it. It is called from [`OnePhaseCcSolver::correct_fluid_mechanics`] after the
//! shared part of `UEqn_1p.H` (turbulence correction, `alpha rho nuEff`) and replaces what follows:
//!
//! 1. `UEqn` WITHOUT `fvm::ddt(alpha, rho, U)`, assembled per term in upstream's expression order (each fvm term its own matrix, added whole by
//!    `fvMatrix::operator+=`, [`UEqnCc::add`]; the order matters in the last bits, README round 46).
//! 2. `pEqnf_1p.H`: `rAUf = 1/(interpolate(A) + interpolate(alpha rho)/dt)`, `phiHbyA = alpharAUf (alphaRhof0ByDt phi.oldTime() + flux(UEqn.H()))
//!    + alphaf phiSbyA`, the compressible `p_rgh` equation, `phi = phi_/alphaf`, `U = reconstruct(...)` through `reconstructU_1p.H`.
//!
//! The pressure matrix is assembled as `fvMatrix` holds it (`pEqnComp + pEqnIncomp`, boundary coefficients apart until the solve), as
//! [`super::super::one_phase_fc`] does (rounds 38, 45, 46). `psi = 0` is assumed (`Boussinesq`, `icoPolynomial`): a compressible closed volume
//! is not reachable from the readers that feed this solver.
//!
//! ~~Not ported here (refused by name when the case is read): `localTortuosity` with `faceCentered`.~~ **CHANGED 2026-11-27 (gh:#1943,
//! README round 79):** the tortuous branch of `UEqn_1p.H` (tensor diffusivity `alpha rho nuEff tortuosity`) is in the `faceCentered`
//! `UEqn` too; nothing else of `pEqnf_1p.H` reads the tortuosity.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::primitives::Vector3;

use super::super::fv::{self, Bc, Eqn, Scalar, Vector};
use super::super::pcg::pcg_dic;
use super::super::tensor_laplacian;
use super::super::thermo::PhaseThermo;
use super::flow::{reconstruct_u_1p, FlowState, OnePhaseFlowCase, PressureSolve, UEqnCc};
use super::OnePhaseCcSolver;

impl OnePhaseCcSolver {
    /// `onePhase::correctFluidMechanics()` for `momentumMode faceCentered`: the rest of `UEqn_1p.H` (no `ddt`), `pEqnf_1p.H`, then
    /// `correctContErr()`. `arn` is `alpha rho nuEff` (cells and boundary) and `gamma` its face interpolate, as built by the shared
    /// part of [`OnePhaseCcSolver::correct_fluid_mechanics`].
    #[allow(clippy::too_many_lines)]
    pub(super) fn correct_fluid_mechanics_face_centered(
        &mut self,
        outer: usize,
        mut fs: FlowState,
        fc: &OnePhaseFlowCase,
        arn: &Scalar,
        gamma: &[f64],
    ) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let nf = g.n_faces();
        let nb = nf - ni;
        let m = g.mesh.clone();
        let boussinesq = self.case.thermo.boussinesq_rho0();
        let rho = self.rho.clone();
        let rho_b = self.rho_b.clone();
        let th = self.case.thermo.clone();
        let rdt = 1.0 / self.dt;
        let npatch = m.patches.len();

        // ---- UEqn_1p.H, momentumMode faceCentered: no fvm::ddt -------------------------------------------------------------------
        // UEqn = div + SuSp(-contErr) + ((-laplacian) - fvc::div(dev2)) + Sp(tr(Kd)/3) + (dev(Kd) & U)   (UEqn_1p.H:20-46)
        let zg = Scalar::uniform(&g, 0.0, g.default_bcs());
        let mut e = UEqnCc::new(&g);
        e.eqn.div_upwind_internal(&g, &self.alpha_rho_phi);
        e.boundary_convection(&g, &self.alpha_rho_phi, &self.u);
        let ce: Vec<f64> = self.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &self.u.c);
        let mut lap = UEqnCc::new(&g);
        let dev2 = if let Some(td) = self.case.tortuosity.as_ref().filter(|_| !self.ctl_no_tort_momentum) {
            // the tortuous branch of `UEqn_1p.H:27-40` (gh:#1943, README round 79): `- fvm::laplacian(alpha rho nuEff tortuosity, U)
            // - fvc::div(alpha rho nuEff tortuosity & dev2(T(grad U)))`, the same tensor-diffusivity terms as the cell-centred branch
            // (round 64, `tensor_laplacian`), assembled here as their own matrix (`lap`) and added whole, as upstream's parenthesis does
            let (tc, tb) = tensor_laplacian::tortuosity_fields(&g, td).expect("validated when the case was read");
            let gc: Vec<_> = (0..n).map(|c| tensor_laplacian::scale(arn.c[c], &tc[c])).collect();
            let gb: Vec<_> = (0..nb).map(|i| tensor_laplacian::scale(arn.b[i], &tb[i])).collect();
            let gf = tensor_laplacian::interpolate(&g, &gc, &gb);
            let tg = tensor_laplacian::decompose(&g, &gf);
            tensor_laplacian::add_neg_laplacian_vector(&mut lap, &g, &tg, &self.u);
            super::super::dev2_div_tortuous(&g, &self.u, arn, (&tc, &tb))
        } else {
            lap.eqn.neg_laplacian(&g, gamma, &zg);
            lap.boundary_laplacian(&g, gamma, &self.u);
            super::super::dev2_div(&g, &self.u, arn)
        };
        lap.sub_field(&g, &dev2);
        e.add(&lap);
        let mut sp_m = UEqnCc::new(&g);
        let mut sp_c = vec![0.0; n];
        let mut dev_u = vec![Vector3::ZERO; n];
        for c in 0..n {
            let [kx, ky, kz] = fs.kd[c];
            let tr = kx + ky + kz;
            sp_c[c] = (1.0 / 3.0) * tr;
            let third = (1.0 / 3.0) * tr;
            let u = self.u.c[c];
            dev_u[c] = Vector3::new((kx - third) * u.x, (ky - third) * u.y, (kz - third) * u.z);
        }
        sp_m.eqn.sp(&g, &sp_c);
        e.add(&sp_m);
        e.add_field(&g, &dev_u);
        if let Some(fct) = self.case.controls.relax.equation("U", self.pimple.final_iteration).filter(|f| *f > 0.0) {
            e.relax(&g, fct, &self.u.c);
        }

        // ---- pEqnf_1p.H ----------------------------------------------------------------------------------------------------------
        // updateAlphaPhiAndInitControls_1p.H
        let alpha_s = {
            let mut s = Scalar { c: self.alpha.clone(), b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
            for i in 0..nb {
                s.b[i] = self.alpha[m.owner[ni + i]];
            }
            s
        };
        let alphaf = fv::interp_linear_openfoam(&g, &alpha_s);
        let rho_s = Scalar { c: rho.clone(), b: rho_b.clone(), bc: vec![Bc::Calculated; npatch] };
        let rho_f = fv::interp_linear_openfoam(&g, &rho_s);
        for f in 0..nf {
            if g.active[f] {
                self.alpha_phi[f] = alphaf[f] * fs.phi[f];
                self.alpha_rho_phi[f] = match boussinesq {
                    Some(rho0) => rho0 * self.alpha_phi[f],
                    None => rho_f[f] * self.alpha_phi[f],
                };
            }
        }
        self.correct_cont_err();
        // alphaRhofByDt = interpolate(alpha rho)/dt, alphaRhof0ByDt = interpolate(alpha.old rho.old)/dt
        let ar_by_dt = |rc: &[f64], rb: &[f64]| -> Vec<f64> {
            let mut s = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
            for c in 0..n {
                s.c[c] = self.alpha[c] * rc[c];
            }
            for i in 0..nb {
                s.b[i] = self.alpha[m.owner[ni + i]] * rb[i];
            }
            fv::interp_linear_openfoam(&g, &s).into_iter().map(|x| x / self.dt).collect()
        };
        let arf = ar_by_dt(&rho, &rho_b);
        let arf0 = ar_by_dt(&self.rho_old, &fs.rho_b_old);
        // rAUf = 1/(interpolate(UEqn.A()) + alphaRhofByDt), alpharAUf = alphaf rAUf, rAUf(p) = |alphaf alpharAUf|
        let mut a = Scalar::uniform(&g, 0.0, g.default_bcs());
        a.c = e.a(&g);
        a.correct_bc(&g);
        let ai = fv::interp_linear_openfoam(&g, &a);
        let rauf0: Vec<f64> = (0..nf).map(|f| 1.0 / (ai[f] + arf[f])).collect();
        let alpharauf: Vec<f64> = (0..nf).map(|f| alphaf[f] * rauf0[f]).collect();
        let rauf: Vec<f64> = (0..nf).map(|f| (alphaf[f] * alpharauf[f]).abs()).collect();

        let mut converged_piso = false;
        let mut corr_piso = 0usize;
        loop {
            // customPimpleControl::correct(bool interrupt)
            corr_piso += 1;
            let interrupt = fc.correct_until_convergence && converged_piso;
            if !(corr_piso <= fc.n_correctors && !interrupt) {
                break;
            }
            for c in 0..n {
                fs.p_rgh.c[c] = self.p[c] - rho[c] * fs.gh[c];
            }
            // correctBoundaryFlux_1p.H
            for f in ni..nf {
                if g.active[f] && self.u.flux_is_fixed(&g, f) {
                    fs.phi[f] = m.face_area_vectors[f].dot(self.u.b[f - ni]);
                }
            }
            // calcPhiSbyA_1p.H: -alpharAUf ghf snGrad(rho(true)) magSf + alpharAUf flux(momentumSource)
            let mut rho_t = Scalar { c: vec![0.0; n], b: vec![0.0; nb], bc: vec![Bc::Calculated; npatch] };
            for c in 0..n {
                rho_t.c[c] = match &th {
                    PhaseThermo::BoussinesqHConst(b) => b.rho(self.t[c]),
                    _ => rho[c],
                };
            }
            for i in 0..nb {
                rho_t.b[i] = match &th {
                    PhaseThermo::BoussinesqHConst(b) => b.rho(self.t[m.owner[ni + i]]),
                    _ => rho_b[i],
                };
            }
            let sng = fv::sn_grad(&g, &rho_t);
            let mut ms = Vector::uniform_with_constraint_patches(&g, Vector3::ZERO);
            ms.c = fc.momentum_source.clone();
            for (cells, src, profile) in &fc.momentum_profiles {
                let fct = profile.value(uom::si::f64::Time::new::<uom::si::time::second>(self.time));
                let v = Vector3::new(src.x * fct, src.y * fct, src.z * fct);
                for &c in cells {
                    ms.c[c] = v;
                }
            }
            ms.correct_bc(&g);
            let ms_flux = fv::flux_openfoam(&g, &ms);
            let mut phi_s = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_s[f] = -alpharauf[f] * fs.ghf[f] * sng[f] * m.face_areas[f];
                    phi_s[f] += alpharauf[f] * ms_flux[f];
                }
            }
            // phiHbyA = alpharAUf (alphaRhof0ByDt phi.oldTime() + flux(UEqn.H())) + alphaf phiSbyA; the temporary H field carries the
            // constraint patches
            let h = e.h(&g, &self.u.c);
            let mut hv = Vector::uniform_with_constraint_patches(&g, Vector3::ZERO);
            hv.c = h;
            hv.correct_bc(&g);
            let fh = fv::flux_openfoam(&g, &hv);
            let mut phi_hbya = vec![0.0; nf];
            for f in 0..nf {
                if g.active[f] {
                    phi_hbya[f] = alpharauf[f] * (arf0[f] * fs.phi_old[f] + fh[f]) + alphaf[f] * phi_s[f];
                }
            }
            // calcrAUfSetSnGradCalcComp_1p.H: the fixedFluxPressure gradient from the boundary flux mismatch
            for (pi, patch) in m.patches.iter().enumerate() {
                if let Bc::FixedGradient(gr) = &mut fs.p_rgh.bc[pi] {
                    for k in 0..patch.size {
                        let f = patch.start + k;
                        gr[k] = (phi_hbya[f] - alphaf[f] * fs.phi[f]) / (m.face_areas[f] * rauf[f]);
                    }
                }
            }
            // pEqn = pEqnComp + pEqnIncomp as fvMatrix holds it (`one_phase_fc::correct_pressure`, round 46): the Laplacian's boundary coefficients
            // stay apart and join the diagonal and the source in `solveSegregated`; psi = 0, so the compressibility part is a pure source
            let dap = fv::div(&g, &self.alpha_phi);
            let mut peqn = Eqn::new(&g);
            peqn.neg_laplacian_internal(&g, &rauf);
            let (pic, pbc) = Eqn::neg_laplacian_boundary_coeffs(&g, &rauf, &fs.p_rgh);
            let dphi = fv::div(&g, &phi_hbya);
            let mut comp_all = vec![0.0; n];
            for c in 0..n {
                let v = m.cell_volumes[c];
                let comp = (self.cont_err[c] - (0.0 + dap[c]) * rho[c]) / rho[c];
                comp_all[c] = comp;
                let mut s_comp = 0.0;
                s_comp -= v * comp;
                let s_inc = -(v * dphi[c]);
                peqn.source[c] = s_comp + s_inc;
            }
            // setReference(pRefCell, pRefValue): the value branch (psi = 0)
            if fc.p_rgh_need_reference {
                let rc = fc.p_ref_cell;
                peqn.source[rc] += peqn.ldu.diag[rc] * fc.p_ref_value;
                peqn.ldu.diag[rc] += peqn.ldu.diag[rc];
            }
            let mut folded = peqn.clone();
            for f in ni..nf {
                if g.active[f] {
                    folded.ldu.diag[m.owner[f]] += pic[f - ni];
                    folded.source[m.owner[f]] += pbc[f - ni];
                }
            }
            let n_pass = fc.n_non_orth_correctors + 1;
            for pass in 1..=n_pass {
                let last = self.pimple.final_inner_iter(corr_piso, pass, &self.case.controls.pimple);
                let solver = if last { fc.p_rgh_final } else { fc.p_rgh };
                let gamg_ctl = if last { fc.p_rgh_final_gamg } else { fc.p_rgh_gamg };
                let mut x = fs.p_rgh.c.clone();
                let perf = match gamg_ctl {
                    Some(ctl) => {
                        let p = fs.gamg.solve(&g, &folded.ldu, &folded.source, &mut x, &ctl);
                        crate::genfoam::common::pcg::PcgPerformance { iterations: p.iterations, initial_residual: p.initial_residual, final_residual: p.final_residual }
                    }
                    None => pcg_dic(&folded.ldu, &folded.source, &mut x, solver.tolerance, solver.rel_tol, solver.max_iter, solver.min_iter),
                };
                fs.solves.push(PressureSolve { outer, corrector: corr_piso, non_orth: pass, initial_residual: perf.initial_residual, iterations: perf.iterations });
                if pass == 1 {
                    converged_piso = perf.iterations == 0;
                }
                self.pimple.record("p_rgh", perf.initial_residual);
                if pass == 1 || !self.first_piso_first_pass_only {
                    self.pimple.update_first_piso_prev_pimple(corr_piso);
                }
                fs.p_rgh.c = x;
                fs.p_rgh.correct_bc(&g);
                if pass == n_pass {
                    // fluid_.dgdt() = -pEqnComp & p_rgh_ (a pure source `-V comp` when psi = 0)
                    for c in 0..n {
                        let v = m.cell_volumes[c];
                        self.dgdt[c] = (v * comp_all[c]) / (-v);
                    }
                }
            }
            // phi_ = phiHbyA + pEqnIncomp.flux(); phi = phi_/alphaf (a fixedValue flux patch ignores the assignment)
            let p_flux = peqn.flux_of_coeffs(&g, &fs.p_rgh.c, &pic, &pbc);
            for f in 0..nf {
                if !g.active[f] {
                    continue;
                }
                fs.phi_solver[f] = phi_hbya[f] + p_flux[f];
                if f >= ni && self.u.flux_is_fixed(&g, f) {
                    continue;
                }
                fs.phi[f] = fs.phi_solver[f] / alphaf[f];
            }
            // p_rgh_.relax(): the `fields` entry `p_rgh` (`p_rghFinal` when final)
            if let Some(a) = self.case.controls.relax.field("p_rgh", self.pimple.final_iteration) {
                for c in 0..n {
                    fs.p_rgh.c[c] = self.p_rgh_prev[c] + a * (fs.p_rgh.c[c] - self.p_rgh_prev[c]);
                }
                fs.p_rgh.correct_bc(&g);
            }
            // reconstructU_1p.H (pEqnf_1p.H always includes it)
            let u_new = reconstruct_u_1p(&g, &fs.phi_solver, &fs.phi, &self.alpha, fc.porous_interface_sharpness);
            for c in 0..n {
                self.u.c[c] = u_new[c];
            }
            self.u.correct_bc(&g);
            for c in 0..n {
                fs.mag_u[c] = self.u.c[c].mag();
            }
            // updatePP_rghRho_1p.H (psi = 0: `rho += psi (p_rgh - p_rgh_0)` adds exactly 0, no closed-volume mass correction)
            for c in 0..n {
                self.p[c] = (fs.p_rgh.c[c] + rho[c] * fs.gh[c]).max(fc.p_min);
            }
            for c in 0..n {
                self.p[c] = self.p[c].max(fc.p_min);
                fs.p_rgh.c[c] = self.p[c] - rho[c] * fs.gh[c];
            }
            fs.p_rgh.correct_bc(&g);
        }
        let _ = rdt;
        self.flow = Some(fs);
        self.correct_cont_err();
    }
}
