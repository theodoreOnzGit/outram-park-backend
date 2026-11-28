// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/
//     porousKEpsilon2PhaseCorrected/porousKEpsilon2PhaseCorrected.{C,H}
//     and the call site src/classes/thermalHydraulics/solvers/twoPhase/include/UEqns_2p.H (lines 11-32)
//   and the OpenFOAM-ESI v2506 classes it builds on: TurbulenceModels/compressible/EddyDiffusivity,
//     TurbulenceModels/turbulenceModels/{eddyViscosity,RAS/RASModel}, finiteVolume/cfdTools/general/bound/bound.C,
//     finiteVolume/fvMatrices/fvMatrix/fvMatrix.C (`relax`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506 (C) OpenCFD Ltd
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `porousKEpsilon2PhaseCorrected`: the two-phase turbulence of `solver::two_phase` (gh:#1802)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! GeN-Foam's `twoPhase` solver calls `fluid.turbulence().correct()` for both fluids at the top of
//! `UEqns_2p.H`, with BOTH phase fractions temporarily `max(alpha, residualAlpha)`. The model solves, for each
//! fluid, a phase-weighted `epsilon` equation and then a `k` equation (the standard high-Reynolds kEpsilon
//! equations, with their production and dissipation terms switched OFF where the cell holds a structure, and a
//! relaxation toward an equilibrium state set by the structure instead), then `nut = Cmu k^2/epsilon` and
//! `alphat = rho nut/Prt`. What it feeds back is only `nuEff = nut + mu/rho` (momentum, `alpha rho nuEff`) and
//! `alphaEff = alpha_ + alphat` (energy).
//!
//! It is NOT a derivation of the one-phase `porousKEpsilon` (gh:#1775/#1812): it derives from
//! `eddyViscosity<RASModel<..>>` itself, so that class is not needed and not ported here.
//!
//! ## Ported
//!
//! `correct()`, `correctNut()`, `bound` (ESI `bound.C`), `fvMatrix::relax(alpha)`, the reading of
//! `turbulenceProperties.<fluid>` (`RASModel porousKEpsilon2PhaseCorrected`) and of `0/<region>/{k,epsilon,nut,alphat}.<fluid>`.
//!
//! ## Refused by name ([`read_porous_k_epsilon_2phase`])
//!
//! `simulationType` other than `laminar`/`RAS`; `RASModel` other than `porousKEpsilon2PhaseCorrected` (`kEpsilon`,
//! `porousKEpsilon`, `LaheyKEpsilon`, `mixtureKEpsilon`, ...); a `DhStruct` key (`nutStabilization`); wall-function
//! patches of `nut`, `k`, `epsilon`; a linear solver for `k`/`epsilon` other than `smoothSolver` + `symGaussSeidel`
//! with `relTol 0`; non-uniform initial fields.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[cfg(feature = "std")]
use crate::error::AppBuilderError;
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry};
use outram_foam_basic_lib::prelude::*;
use petir::mathf::RealMath;

#[cfg(feature = "std")]
use super::case::{bool_entry, read_field, scalar, scalar_bcs, word, Ctx};
use super::fv::{self, Bc, Eqn, Geometry, Scalar, Vector};
use super::TwoPhaseSolver;

/// `SMALL` of OpenFOAM's double precision (`doubleScalarSMALL`), the default of `kMin` and `epsilonMin`.
pub const SMALL: f64 = 1.0e-15;

/// The linear solver of `k` / `epsilon`: `smoothSolver` with `symGaussSeidel` (one sweep per iteration).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmoothSolve {
    /// Absolute tolerance of the normalised residual.
    pub tolerance: f64,
    /// `minIter`: sweeps done even when the system is already converged.
    pub min_iter: usize,
    /// `maxIter` (OpenFOAM default 1000).
    pub max_iter: usize,
}

/// `porousKEpsilon2PhaseCorrected` of one fluid, as GeN-Foam's `turbulenceProperties.<fluid>` and the
/// `0/<region>/{k,epsilon,nut,alphat}.<fluid>` files define it. Built by [`read_porous_k_epsilon_2phase`].
#[derive(Debug, Clone)]
pub struct PorousKEpsilon2PhaseCorrected {
    /// `RAS { turbulence on|off; }`: with `off`, `correct()` returns at once (`nut` and `alphat` stay as read).
    pub turbulence: bool,
    /// `Cmu` (default 0.09).
    pub cmu: f64,
    /// `C1` (default 1.44).
    pub c1: f64,
    /// `C2` (default 1.92).
    pub c2: f64,
    /// `C3` (default 0).
    pub c3: f64,
    /// `sigmak` (default 1).
    pub sigmak: f64,
    /// `sigmaEps` (default 1.3).
    pub sigma_eps: f64,
    /// `Prt` (default 1).
    pub prt: f64,
    /// `kMin` (default `SMALL`).
    pub k_min: f64,
    /// `epsilonMin` (default `SMALL`).
    pub epsilon_min: f64,
    /// Turbulent kinetic energy `k` [m^2/s^2].
    pub k: Scalar,
    /// Dissipation rate `epsilon` [m^2/s^3].
    pub epsilon: Scalar,
    /// Turbulent viscosity `nut` [m^2/s].
    pub nut: Scalar,
    /// Turbulent thermal diffusivity `alphat` [kg/(m s)].
    pub alphat: Scalar,
    /// `convergenceLength` per cell [m] (`1e69` where no region sets it).
    pub convergence_length: Vec<f64>,
    /// `turbulenceIntensityCoeff` (A of `I_t = A Re^B + D (1 - alpha)`) per cell.
    pub turbulence_intensity_coeff: Vec<f64>,
    /// `turbulenceIntensityAlphaCoeff` (D) per cell.
    pub turbulence_intensity_alpha_coeff: Vec<f64>,
    /// `turbulenceIntensityExp` (B) per cell.
    pub turbulence_intensity_exp: Vec<f64>,
    /// `turbulenceLengthScaleCoeff` (C of `L_t = C Dh`) per cell.
    pub turbulence_length_scale_coeff: Vec<f64>,
    /// `equilibriumK` of the last `correct()`.
    pub equilibrium_k: Vec<f64>,
    /// `equilibriumEpsilon` of the last `correct()`.
    pub equilibrium_epsilon: Vec<f64>,
    /// Linear solver of `k`.
    pub k_solver: SmoothSolve,
    /// Linear solver of `epsilon`.
    pub epsilon_solver: SmoothSolve,
    /// `relaxationFactors.equations` entry matching `k.<fluid>` (upstream calls `relax()` on every equation).
    pub k_equation_relaxation: Option<f64>,
    /// `relaxationFactors.equations` entry matching `epsilon.<fluid>`.
    pub epsilon_equation_relaxation: Option<f64>,
    /// The production term `G` of the last `correct()` (diagnostic).
    pub production: Vec<f64>,
    /// How many times `bound` had to act on `k` and `epsilon` (diagnostic; X5 of README round 16).
    pub bound_count: usize,
    pub(super) k_old: Vec<f64>,
    pub(super) epsilon_old: Vec<f64>,
}

impl PorousKEpsilon2PhaseCorrected {
    /// `Cmu3by4_ = pow(Cmu_, 0.75)`.
    #[must_use]
    pub fn cmu_three_quarters(&self) -> f64 {
        self.cmu.r_powf(0.75)
    }

    /// `runTime++`: the old-time values `fvm::ddt` reads (OpenFOAM stores them at the first access of the step,
    /// before `k` or `epsilon` is modified, so they are the values of the end of the previous step).
    pub(super) fn begin_time_step(&mut self) {
        self.k_old = self.k.c.clone();
        self.epsilon_old = self.epsilon.c.clone();
    }

    /// DEPRECATED ablation (gh:#1802, README round 16 X7): skip `fvMatrix::relax()` of both equations.
    #[must_use]
    pub fn without_equation_relaxation(mut self) -> Self {
        self.k_equation_relaxation = None;
        self.epsilon_equation_relaxation = None;
        self
    }
}

/// ESI `bound(vsf, lowerBound)` (`finiteVolume/cfdTools/general/bound/bound.C`): if the minimum over the cells
/// AND the boundary faces is below `lower`, replace every cell value by `max(max(v, average(max(vsf, lower)) pos0(-v)), lower)`
/// (`average` = `fvc::average`: `surfaceSum(magSf linearInterpolate(.))/surfaceSum(magSf)`) and every boundary value by `max(b, lower)`.
/// Returns whether it acted.
pub fn bound(g: &Geometry, s: &mut Scalar, lower: f64) -> bool {
    let ni = g.n_internal();
    let mut min_v = f64::MAX;
    for &v in &s.c {
        min_v = min_v.min(v);
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            min_v = min_v.min(s.b[f - ni]);
        }
    }
    if min_v >= lower {
        return false;
    }
    // fvc::average(max(vsf, lowerBound)): linear interpolation of the bounded field, area weighted over the faces of a cell
    let mut maxed = s.clone();
    maxed.c.iter_mut().for_each(|v| *v = v.max(lower));
    maxed.b.iter_mut().for_each(|v| *v = v.max(lower));
    let face = fv::interp_linear_openfoam(g, &maxed);
    let m = &g.mesh;
    let n = g.n_cells();
    let mut num = vec![0.0; n];
    let mut den = vec![0.0; n];
    for f in 0..ni {
        let a = m.face_areas[f];
        num[m.owner[f]] += a * face[f];
        den[m.owner[f]] += a;
        num[m.neighbour[f]] += a * face[f];
        den[m.neighbour[f]] += a;
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            let a = m.face_areas[f];
            num[m.owner[f]] += a * face[f];
            den[m.owner[f]] += a;
        }
    }
    for c in 0..n {
        let avg = num[c] / den[c];
        let pos0 = if -s.c[c] >= 0.0 { 1.0 } else { 0.0 };
        s.c[c] = s.c[c].max(avg * pos0).max(lower);
    }
    s.b.iter_mut().for_each(|v| *v = v.max(lower));
    true
}

/// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`) on a scalar equation whose boundary coefficients are folded into
/// `e.ldu.diag`: `diag' = max(|diag + inflow2|, sumOff)/alpha` with `inflow2 = 2 sum max(-iCoeffs, 0)` of the cell's
/// non-coupled boundary faces (a convective INFLOW through a non-fixed patch is not neutral), and
/// `source += (diag' - diag) psi`. `alpha <= 0` leaves the matrix alone.
pub fn relax_equation(g: &Geometry, e: &mut Eqn, alpha: f64, inflow2: &[f64], psi: &[f64]) {
    if alpha <= 0.0 {
        return;
    }
    let n = g.n_cells();
    let mut sum_off = vec![0.0; n];
    for f in 0..g.n_internal() {
        sum_off[e.ldu.owner[f]] += e.ldu.upper[f].abs();
        sum_off[e.ldu.neighbour[f]] += e.ldu.lower[f].abs();
    }
    if g.has_cyclic() {
        // a coupled patch (`fvMatrix.C:1130-1140, 1205-1215`, gh:#1905): its `internalCoeffs` (kept apart in `Eqn::cyc_int`) go into `D`
        // before the dominance step and come out after it; `mag(boundaryCoeffs)` goes into `sumOff`
        let ni = g.n_internal();
        let mut d = e.ldu.diag.clone();
        for c in 0..n {
            d[c] += inflow2[c];
        }
        for f in ni..g.n_faces() {
            if g.cyclic_nbr[f - ni] != usize::MAX {
                d[g.mesh.owner[f]] += e.cyc_int[f - ni];
                sum_off[g.mesh.owner[f]] += e.cyc_bou[f - ni].abs();
            }
        }
        for c in 0..n {
            d[c] = d[c].abs().max(sum_off[c]) / alpha;
        }
        for f in ni..g.n_faces() {
            if g.cyclic_nbr[f - ni] != usize::MAX {
                d[g.mesh.owner[f]] -= e.cyc_int[f - ni];
            }
        }
        for c in 0..n {
            let d0 = e.ldu.diag[c];
            e.ldu.diag[c] = d[c];
            e.source[c] += (d[c] - d0) * psi[c];
        }
        return;
    }
    for c in 0..n {
        let d0 = e.ldu.diag[c];
        let d = (d0 + inflow2[c]).abs().max(sum_off[c]) / alpha;
        e.ldu.diag[c] = d;
        e.source[c] += (d - d0) * psi[c];
    }
}

/// `2 sum max(-phi_b, 0)` per owner cell over the boundary faces of `phi` whose patch of `psi` is not `fixedValue`
/// (a `zeroGradient` or `symmetry` patch has `valueInternalCoeffs = 1`, so its `iCoeffs` is `phi_b`; a fixed one has 0).
pub fn convective_inflow2(g: &Geometry, phi: &[f64], psi: &Scalar) -> Vec<f64> {
    let mut out = vec![0.0; g.n_cells()];
    for f in g.n_internal()..g.n_faces() {
        // a coupled face is not folded into the diagonal (`Eqn::cyc_int`, gh:#1905): `relax_equation` treats it
        if g.active[f] && !psi.is_fixed(g, f) && g.cyclic_nbr[f - g.n_internal()] == usize::MAX {
            out[g.mesh.owner[f]] += 2.0 * (-phi[f]).max(0.0);
        }
    }
    out
}

/// The cell-centre Gauss-linear gradient of a velocity field, `T_ij = d_i U_j` (OpenFOAM's `fvc::grad(U)`, internal values).
pub fn velocity_gradient(g: &Geometry, u: &Vector) -> Vec<[[f64; 3]; 3]> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let mut gr = vec![[[0.0; 3]; 3]; g.n_cells()];
    for f in 0..g.n_faces() {
        if !g.active[f] {
            continue;
        }
        let s = m.face_area_vectors[f];
        let s = [s.x, s.y, s.z];
        let uf = if f < ni {
            let w = g.weights[f];
            u.c[m.owner[f]] * w + u.c[m.neighbour[f]] * (1.0 - w)
        } else if g.cyclic_nbr[f - ni] != usize::MAX {
            // a coupled patch: `lerp(U_N, U_P, w)` from the cells (gh:#1905)
            g.cyclic_lerp_vector(f, &u.c, g.cyclic_w[f - ni])
        } else {
            u.b[f - ni]
        };
        let uf = [uf.x, uf.y, uf.z];
        let o = m.owner[f];
        for i in 0..3 {
            for j in 0..3 {
                gr[o][i][j] += s[i] * uf[j];
                if f < ni {
                    gr[m.neighbour[f]][i][j] -= s[i] * uf[j];
                }
            }
        }
    }
    for (c, t) in gr.iter_mut().enumerate() {
        for row in t.iter_mut() {
            for v in row.iter_mut() {
                *v /= m.cell_volumes[c];
            }
        }
    }
    gr
}

/// `dev(twoSymm(T)) && T` of the production term `G = nut (dev(twoSymm(grad U)) && grad U)` for one cell.
#[must_use]
pub fn production_invariant(t: &[[f64; 3]; 3]) -> f64 {
    let mut s = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            s[i][j] = t[i][j] + t[j][i];
        }
    }
    let tr = s[0][0] + s[1][1] + s[2][2];
    let third = (1.0 / 3.0) * tr;
    for (i, row) in s.iter_mut().enumerate() {
        row[i] -= third;
    }
    let mut sum = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            sum += s[i][j] * t[i][j];
        }
    }
    sum
}

/// `smoothSolver` + `symGaussSeidel` of `k` / `epsilon`: `minIter` sweeps are done even when the initial residual is below the tolerance.
fn smooth_solve(e: &Eqn, x: &mut [f64], s: SmoothSolve) {
    let (sweeps, _, _) = super::super::one_phase::smooth_solve_sym_gauss_seidel(
        &e.ldu,
        &e.source,
        x,
        SolverSettings { tolerance: s.tolerance, max_iter: s.max_iter },
    );
    if sweeps == 0 && s.min_iter > 0 {
        let _ = super::super::one_phase::smooth_solve_sym_gauss_seidel(
            &e.ldu,
            &e.source,
            x,
            SolverSettings { tolerance: 0.0, max_iter: s.min_iter },
        );
    }
}

impl TwoPhaseSolver {
    /// `fluid1_.turbulence().correct(); fluid2_.turbulence().correct();` (`UEqns_2p.H`). Each model sees its own
    /// phase fraction as `max(alpha, residualAlpha)` (cells and boundary), the momentum and energy equations the unlimited one.
    pub(super) fn correct_turbulence(&mut self) {
        for k in 0..2 {
            if self.fluids[k].turbulence.as_ref().is_some_and(|t| t.turbulence) {
                self.correct_turbulence_of(k);
            }
        }
    }

    /// `porousKEpsilon2PhaseCorrected::correct()` of fluid `k`.
    #[allow(clippy::too_many_lines)]
    fn correct_turbulence_of(&mut self, k: usize) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let nb = g.n_faces() - ni;
        let Some(mut t) = self.fluids[k].turbulence.take() else {
            return;
        };
        let fl = &self.fluids[k];
        let rdt = 1.0 / self.dt;
        let vol = &g.mesh.cell_volumes;
        // the limited phase fraction `max(alpha, residualAlpha)`, cells and boundary
        let ra = fl.residual_alpha;
        let alpha_c: Vec<f64> = fl.alpha.c.iter().map(|v| v.max(ra)).collect();
        let alpha_b: Vec<f64> = fl.alpha.b.iter().map(|v| v.max(ra)).collect();
        let rho = &fl.rho;
        // nu = thermo.mu()/rho
        let nu_c: Vec<f64> = (0..n).map(|c| fl.mu.c[c] / rho.c[c]).collect();
        let nu_b: Vec<f64> = (0..nb).map(|i| fl.mu.b[i] / rho.b[i]).collect();
        let s_alpha = &self.structure.alpha.c;
        let no_structure: Vec<f64> = s_alpha.iter().map(|&s| if s <= 0.0 { 1.0 } else { 0.0 }).collect();
        let yes_structure: Vec<f64> = s_alpha.iter().map(|&s| if s >= 0.0 { 1.0 } else { 0.0 }).collect();
        let div_u = fv::div(&g, &fl.phi);
        // G = nut (dev(twoSymm(grad U)) && grad U)
        let grad_u = velocity_gradient(&g, &fl.u);
        let production: Vec<f64> = (0..n).map(|c| t.nut.c[c] * production_invariant(&grad_u[c])).collect();
        // equilibrium k and epsilon
        let cmu34 = t.cmu_three_quarters();
        let mut eq_k = vec![0.0; n];
        let mut eq_eps = vec![0.0; n];
        for c in 0..n {
            let mag_u = fl.u.c[c].mag();
            let ti = t.turbulence_intensity_coeff[c] * self.re_fs[k][c].r_powf(t.turbulence_intensity_exp[c])
                + t.turbulence_intensity_alpha_coeff[c] * (1.0 - alpha_c[c] / (1.0 - s_alpha[c]));
            let mu_ti = mag_u * ti;
            eq_k[c] = 1.5 * (mu_ti * mu_ti);
            let len = (t.turbulence_length_scale_coeff[c] * self.structure.dh[c]).max(SMALL);
            eq_eps[c] = cmu34 * eq_k[c].r_powf(1.5) / len;
        }
        // alpha rho |U|/convergenceLength (structure cells)
        let conv: Vec<f64> = (0..n)
            .map(|c| alpha_c[c] * rho.c[c] * (fl.u.c[c].mag() / t.convergence_length[c]) * yes_structure[c])
            .collect();
        // gamma = alpha rho D*Eff, D*Eff = nut/sigma + nu, interpolated linearly to the faces
        let gamma_of = |sigma: f64| -> Vec<f64> {
            let mut gm = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                gm.c[c] = alpha_c[c] * rho.c[c] * (t.nut.c[c] / sigma + nu_c[c]);
            }
            for i in 0..nb {
                gm.b[i] = alpha_b[i] * rho.b[i] * (t.nut.b[i] / sigma + nu_b[i]);
            }
            fv::interp_linear_openfoam(&g, &gm)
        };
        let alpha_old = &fl.alpha_old;
        let rho_old = &fl.rho_old;
        let ddt = |e: &mut Eqn, psi_old: &[f64]| {
            for c in 0..n {
                e.ldu.diag[c] += rdt * alpha_c[c] * rho.c[c] * vol[c];
                e.source[c] += rdt * alpha_old[c] * rho_old[c] * psi_old[c] * vol[c];
            }
        };

        // ---- dissipation equation ----
        let mut e = Eqn::new(&g);
        ddt(&mut e, &t.epsilon_old);
        e.div_upwind(&g, &fl.alpha_rho_phi, &t.epsilon);
        e.neg_laplacian(&g, &gamma_of(t.sigma_eps), &t.epsilon);
        let c1 = t.c1;
        let susp_scale = (2.0 / 3.0) * c1 - t.c3;
        let mut su = vec![0.0; n];
        let mut susp = vec![0.0; n];
        let mut sp = vec![0.0; n];
        let mut su_conv = vec![0.0; n];
        for c in 0..n {
            su[c] = c1 * alpha_c[c] * rho.c[c] * production[c] * t.epsilon.c[c] / t.k.c[c] * no_structure[c];
            susp[c] = susp_scale * alpha_c[c] * rho.c[c] * div_u[c] * no_structure[c];
            sp[c] = t.c2 * alpha_c[c] * rho.c[c] * t.epsilon.c[c] / t.k.c[c] * no_structure[c];
            su_conv[c] = conv[c] * eq_eps[c];
        }
        e.su(&g, &su);
        e.su_sp(&g, &susp, &t.epsilon.c);
        e.sp(&g, &sp);
        e.sp(&g, &conv);
        e.su(&g, &su_conv);
        if let Some(a) = t.epsilon_equation_relaxation {
            let inflow2 = convective_inflow2(&g, &fl.alpha_rho_phi, &t.epsilon);
            relax_equation(&g, &mut e, a, &inflow2, &t.epsilon.c);
        }
        let mut x = t.epsilon.c.clone();
        smooth_solve(&e, &mut x, t.epsilon_solver);
        t.epsilon.c = x;
        t.epsilon.correct_bc(&g);
        if bound(&g, &mut t.epsilon, t.epsilon_min) {
            t.bound_count += 1;
        }

        // ---- turbulent kinetic energy equation ----
        let mut e = Eqn::new(&g);
        ddt(&mut e, &t.k_old);
        e.div_upwind(&g, &fl.alpha_rho_phi, &t.k);
        e.neg_laplacian(&g, &gamma_of(t.sigmak), &t.k);
        let mut su = vec![0.0; n];
        let mut susp = vec![0.0; n];
        let mut sp = vec![0.0; n];
        let mut su_conv = vec![0.0; n];
        for c in 0..n {
            su[c] = alpha_c[c] * rho.c[c] * production[c] * no_structure[c];
            susp[c] = (2.0 / 3.0) * alpha_c[c] * rho.c[c] * div_u[c] * no_structure[c];
            sp[c] = alpha_c[c] * rho.c[c] * t.epsilon.c[c] / t.k.c[c] * no_structure[c];
            su_conv[c] = conv[c] * eq_k[c];
        }
        e.su(&g, &su);
        e.su_sp(&g, &susp, &t.k.c);
        e.sp(&g, &sp);
        e.sp(&g, &conv);
        e.su(&g, &su_conv);
        if let Some(a) = t.k_equation_relaxation {
            let inflow2 = convective_inflow2(&g, &fl.alpha_rho_phi, &t.k);
            relax_equation(&g, &mut e, a, &inflow2, &t.k.c);
        }
        let mut x = t.k.c.clone();
        smooth_solve(&e, &mut x, t.k_solver);
        t.k.c = x;
        t.k.correct_bc(&g);
        if bound(&g, &mut t.k, t.k_min) {
            t.bound_count += 1;
        }

        // ---- correctNut ----
        for c in 0..n {
            t.nut.c[c] = t.cmu * (t.k.c[c] * t.k.c[c]) / t.epsilon.c[c];
        }
        for i in 0..nb {
            t.nut.b[i] = t.cmu * (t.k.b[i] * t.k.b[i]) / t.epsilon.b[i];
        }
        t.nut.correct_bc(&g);
        for c in 0..n {
            t.alphat.c[c] = rho.c[c] * t.nut.c[c] / t.prt;
        }
        for i in 0..nb {
            t.alphat.b[i] = rho.b[i] * t.nut.b[i] / t.prt;
        }
        t.alphat.correct_bc(&g);
        t.equilibrium_k = eq_k;
        t.equilibrium_epsilon = eq_eps;
        t.production = production;
        self.fluids[k].turbulence = Some(t);
    }
}

/// OpenFOAM keyword regex, the subset `solvers` and `relaxationFactors` use: literals, `\x` escapes, `.`, `*` (after an atom
/// or a group), `(a|b|c)` groups and top-level `|`. The whole name must match.
#[must_use]
pub fn regex_full_match(pattern: &str, name: &str) -> bool {
    fn alternatives(p: &[char]) -> Vec<&[char]> {
        let mut out = Vec::new();
        let (mut depth, mut start) = (0usize, 0usize);
        let mut i = 0;
        while i < p.len() {
            match p[i] {
                '\\' => i += 1,
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                '|' if depth == 0 => {
                    out.push(&p[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        out.push(&p[start..]);
        out
    }
    // the end index of the atom starting at p[0], and whether it is a group (then its inner pattern)
    fn atom_len(p: &[char]) -> usize {
        match p[0] {
            '\\' => 2.min(p.len()),
            '(' => {
                let mut depth = 0usize;
                for (i, &ch) in p.iter().enumerate() {
                    match ch {
                        '\\' => {}
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                return i + 1;
                            }
                        }
                        _ => {}
                    }
                }
                p.len()
            }
            _ => 1,
        }
    }
    fn m(p: &[char], s: &[char]) -> bool {
        let alts = alternatives(p);
        if alts.len() > 1 {
            return alts.iter().any(|a| m(a, s));
        }
        if p.is_empty() {
            return s.is_empty();
        }
        let al = atom_len(p);
        let atom = &p[..al];
        let star = p.get(al) == Some(&'*');
        let rest = if star { &p[al + 1..] } else { &p[al..] };
        // one match of the atom against a prefix of `s`: the possible prefix lengths
        let one = |s: &[char]| -> Vec<usize> {
            match atom[0] {
                '(' => {
                    let inner = &atom[1..atom.len().saturating_sub(1)];
                    (0..=s.len()).filter(|&l| m(inner, &s[..l])).collect()
                }
                '.' => {
                    if s.is_empty() {
                        Vec::new()
                    } else {
                        vec![1]
                    }
                }
                '\\' => {
                    if s.first() == atom.get(1) && s.first().is_some() {
                        vec![1]
                    } else {
                        Vec::new()
                    }
                }
                ch => {
                    if s.first() == Some(&ch) {
                        vec![1]
                    } else {
                        Vec::new()
                    }
                }
            }
        };
        if !star {
            return one(s).into_iter().any(|l| m(rest, &s[l..]));
        }
        // star: zero or more repetitions
        if m(rest, s) {
            return true;
        }
        one(s).into_iter().filter(|&l| l > 0).any(|l| m(p, &s[l..]))
    }
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = name.chars().collect();
    m(&p, &s)
}

/// OpenFOAM's dictionary lookup of `name`: the exact keyword, else the LAST declared pattern that matches ([`regex_full_match`]).
#[cfg(feature = "std")]
fn lookup_last_regex<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().filter(|(k, _)| regex_full_match(k, name)).last().map(|(_, e)| e)
}

/// Read `turbulenceProperties.<fluid>` and the four fields of one fluid, or `None` for `simulationType laminar`.
/// `regions` are the structure regions `(name, cells, volumeFraction)` of `structureProperties`; `fs` is `system/<region>/fvSolution`.
///
/// # Errors
/// [`AppBuilderError::Case`] naming the first selection, key, region or boundary condition the port does not implement
/// (the module header lists them).
#[cfg(feature = "std")]
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn read_porous_k_epsilon_2phase(
    ctx: &Ctx,
    g: &Geometry,
    region: &str,
    fluid: &str,
    tp: &FoamDict,
    fs: &FoamDict,
    regions: &[(String, Vec<usize>, f64)],
) -> Result<Option<PorousKEpsilon2PhaseCorrected>, AppBuilderError> {
    let sim = word(tp, "simulationType").unwrap_or_default();
    match sim.as_str() {
        "laminar" => return Ok(None),
        "RAS" => {}
        other => {
            return Err(ctx.e(format!(
                "{fluid}: simulationType `{other}` is not ported (laminar, and RAS with porousKEpsilon2PhaseCorrected)"
            )))
        }
    }
    let ras = ctx.sub(tp, "RAS")?;
    let model = ctx.word(ras, "RASModel")?;
    if model != "porousKEpsilon2PhaseCorrected" {
        return Err(ctx.e(format!(
            "{fluid}: RASModel `{model}` is not ported (only porousKEpsilon2PhaseCorrected; kEpsilon, porousKEpsilon, \
             LaheyKEpsilon and mixtureKEpsilon are not)"
        )));
    }
    let coeff_name = "porousKEpsilon2PhaseCorrectedCoeffs";
    for (key, _) in ras.iter() {
        let known = matches!(
            key,
            "RASModel"
                | "turbulence"
                | "printCoeffs"
                | "liquidName"
                | "gasName"
                | "kMin"
                | "epsilonMin"
                | "Cmu"
                | "C1"
                | "C2"
                | "C3"
                | "sigmak"
                | "sigmaEps"
                | "Prt"
        ) || key == coeff_name;
        if !known {
            return Err(ctx.e(format!("{fluid}: RAS entry `{key}` is not ported")));
        }
    }
    // `coeffDict_ = RASDict_.optionalSubDict(type + "Coeffs")`: the sub-dictionary if present, else the RAS dictionary itself
    let coeffs = ras.get_dict(coeff_name).unwrap_or(ras);
    let coeff = |key: &str, default: f64| scalar(coeffs, key).unwrap_or(default);
    let k_min = scalar(ras, "kMin").unwrap_or(SMALL);
    let epsilon_min = scalar(ras, "epsilonMin").unwrap_or(SMALL);

    let n = g.n_cells();
    let props = ctx.sub(tp, "porousKEpsilon2PhaseCorrectedProperties")?;
    let mut convergence_length = vec![1e69; n];
    let mut ti = vec![0.0; n];
    let mut ti_alpha = vec![0.0; n];
    let mut ti_exp = vec![0.0; n];
    let mut ls = vec![0.0; n];
    let mut in_dict: Vec<String> = Vec::new();
    for (key, e) in props.iter() {
        let FoamEntry::SubDict(rd) = e else {
            return Err(ctx.e(format!("{fluid}: porousKEpsilon2PhaseCorrectedProperties.{key} must be a dictionary")));
        };
        for name in key.split(':') {
            let (_, cells, vf) = regions
                .iter()
                .find(|(rn, _, _)| rn == name)
                .ok_or_else(|| ctx.e(format!("{fluid}: porousKEpsilon2PhaseCorrectedProperties names an unknown structure region `{name}`")))?;
            // a region without structure has regular kEpsilon: nothing to set
            if *vf == 0.0 {
                continue;
            }
            in_dict.push(name.to_string());
            for (k2, _) in rd.iter() {
                if !matches!(
                    k2,
                    "convergenceLength"
                        | "turbulenceIntensityCoeff"
                        | "turbulenceIntensityAlphaCoeff"
                        | "turbulenceIntensityExp"
                        | "turbulenceLengthScaleCoeff"
                ) {
                    if k2 == "DhStruct" || k2 == "laminarReStruct" {
                        return Err(ctx.e(format!(
                            "{fluid}: `{k2}` (nutStabilization of porousKEpsilon2PhaseCorrected) is not ported"
                        )));
                    }
                    return Err(ctx.e(format!("{fluid}: porousKEpsilon2PhaseCorrectedProperties.{key}.{k2} is not ported")));
                }
            }
            let cl = ctx.f64(rd, "convergenceLength")?;
            let a = ctx.f64(rd, "turbulenceIntensityCoeff")?;
            let d = ctx.f64(rd, "turbulenceIntensityAlphaCoeff")?;
            let b = ctx.f64(rd, "turbulenceIntensityExp")?;
            let c = ctx.f64(rd, "turbulenceLengthScaleCoeff")?;
            for &cell in cells {
                convergence_length[cell] = cl;
                ti[cell] = a;
                ti_alpha[cell] = d;
                ti_exp[cell] = b;
                ls[cell] = c;
            }
        }
    }
    for (name, _, vf) in regions {
        if *vf != 0.0 && !in_dict.iter().any(|r| r == name) {
            return Err(ctx.e(format!(
                "{fluid}: porousKEpsilon2PhaseCorrected region `{name}`: properties were not set / subDictionary not found"
            )));
        }
    }
    // `convergenceLength_ = max(convergenceLength_, SMALL)`
    for v in &mut convergence_length {
        *v = v.max(SMALL);
    }

    // the four fields
    let zero = "0/".to_string() + region;
    let read = |name: &str| -> Result<Scalar, AppBuilderError> {
        let rel = format!("{zero}/{name}.{fluid}");
        let ff = read_field(ctx, &rel)?;
        let bc = scalar_bcs(ctx, g, &ff)?;
        if (name == "k" || name == "epsilon") && bc.iter().any(|b| matches!(b, Bc::Calculated)) {
            return Err(ctx.e(format!("{rel}: a `calculated` patch of {name} is not ported")));
        }
        let v = ff.internal.as_scalar().ok_or_else(|| ctx.e(format!("{rel}: only a uniform scalar internalField is ported")))?;
        Ok(Scalar::uniform(g, v, bc))
    };
    let k = read("k")?;
    let epsilon = read("epsilon")?;
    let nut = read("nut")?;
    let alphat = read("alphat")?;

    // linear solvers and equation relaxation of k.<fluid> and epsilon.<fluid>
    let solvers = ctx.sub(fs, "solvers")?;
    let solver_of = |field: &str| -> Result<SmoothSolve, AppBuilderError> {
        let name = format!("{field}.{fluid}");
        let Some(FoamEntry::SubDict(d)) = lookup_last_regex(solvers, &name) else {
            return Err(ctx.e(format!("fvSolution: no solver for {name}")));
        };
        if word(d, "solver").as_deref() != Some("smoothSolver") || word(d, "smoother").as_deref() != Some("symGaussSeidel") {
            return Err(ctx.e(format!("fvSolution: {name} must be smoothSolver with symGaussSeidel (the only linear solver ported for k and epsilon)")));
        }
        if scalar(d, "relTol").unwrap_or(0.0) != 0.0 {
            return Err(ctx.e(format!("fvSolution: {name}: relTol != 0 is not ported for k and epsilon")));
        }
        if scalar(d, "nSweeps").unwrap_or(1.0) != 1.0 {
            return Err(ctx.e(format!("fvSolution: {name}: nSweeps != 1 is not ported")));
        }
        Ok(SmoothSolve {
            tolerance: ctx.f64(d, "tolerance")?,
            min_iter: scalar(d, "minIter").unwrap_or(0.0) as usize,
            max_iter: scalar(d, "maxIter").unwrap_or(1000.0) as usize,
        })
    };
    let relax_of = |field: &str| -> Option<f64> {
        let eqs = fs.get_dict("relaxationFactors")?.get_dict("equations")?;
        let name = format!("{field}.{fluid}");
        match lookup_last_regex(eqs, &name).or_else(|| eqs.get("default")) {
            Some(FoamEntry::Scalar(x)) => Some(*x),
            _ => None,
        }
    };
    let _ = bool_entry;
    Ok(Some(PorousKEpsilon2PhaseCorrected {
        turbulence: bool_entry(ras, "turbulence").unwrap_or(true),
        cmu: coeff("Cmu", 0.09),
        c1: coeff("C1", 1.44),
        c2: coeff("C2", 1.92),
        c3: coeff("C3", 0.0),
        sigmak: coeff("sigmak", 1.0),
        sigma_eps: coeff("sigmaEps", 1.3),
        prt: coeff("Prt", 1.0),
        k_min,
        epsilon_min,
        k_old: k.c.clone(),
        epsilon_old: epsilon.c.clone(),
        k,
        epsilon,
        nut,
        alphat,
        convergence_length,
        turbulence_intensity_coeff: ti,
        turbulence_intensity_alpha_coeff: ti_alpha,
        turbulence_intensity_exp: ti_exp,
        turbulence_length_scale_coeff: ls,
        equilibrium_k: vec![0.0; n],
        equilibrium_epsilon: vec![0.0; n],
        k_solver: solver_of("k")?,
        epsilon_solver: solver_of("epsilon")?,
        k_equation_relaxation: relax_of("k"),
        epsilon_equation_relaxation: relax_of("epsilon"),
        production: vec![0.0; n],
        bound_count: 0,
    }))
}
