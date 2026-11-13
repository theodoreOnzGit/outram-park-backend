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

//! **ESI OpenFOAM-v2506 `twoPhaseEulerFoam` time step** (the `pU` branch,
//! `faceMomentum no`), as an option beside the dev-form
//! [`TwoFluidPimple::solve_timestep`] (gh:#1363 row 5; maintainer: "ESI
//! preferred"). The dev form is unchanged and still callable.
//!
//! ## Upstream provenance
//!
//! | Rust step | ESI v2506 source |
//! |---|---|
//! | phase-fraction transport (`flux` of the mixture flux plus the relative-flux compression term, explicit update) | `src/phaseSystemModels/twoPhaseEuler/twoPhaseSystem/twoPhaseSystem/twoPhaseSystem.C`, `twoPhaseSystem::solve()` |
//! | explicit update formula | `src/finiteVolume/fvMatrices/solvers/MULES/MULESTemplates.C`, `explicitSolve(rDeltaT, ...)` |
//! | `contErr` | `applications/solvers/multiphase/twoPhaseEulerFoam/contErrs.H` |
//! | momentum matrices `U1Eqn`, `U2Eqn` (not solved) | `.../twoPhaseEulerFoam/pU/UEqns.H` |
//! | `rAU`, `HbyA`, `phig`, `phiHbyA`, pressure equation, partial elimination of the drag | `.../twoPhaseEulerFoam/pU/pEqn.H` |
//! | `p = p_rgh + rho*gh` | `.../twoPhaseEulerFoam/pU/pEqn.H`, `gh.H` |
//!
//! ## What differs from the dev form (and why the dev form cannot match ESI)
//!
//! 1. **Order.** ESI advances alpha FIRST, with the fluxes of the previous
//!    step; the dev form advances alpha last, on the new velocity.
//! 2. **No momentum solve.** ESI builds `U1Eqn`/`U2Eqn` and uses only `A()`
//!    and `H()`; the velocity comes from the partial-elimination corrector.
//!    `H()` is evaluated on the CURRENT velocity (off-diagonal terms
//!    included); the dev form's `HbyA` omits them.
//! 3. **Drag by partial elimination** (`rAUKd`, `phir`, `Ur`) instead of an
//!    explicit other-phase velocity.
//! 4. **Pressure variable** `p_rgh = p - rho*gh` with the buoyancy flux
//!    `phig` built from `snGrad(rho)` and `(rho1 - rho2) g`.
//! 5. **Mass-weighted equations**: `ddt(alpha*rho, U)`, `contErr` in the
//!    momentum diagonal.
//!
//! ## Honest scope
//!
//! Translated: constant-density phases (`rhoConst`), two phases, drag as the
//! only interfacial force (`Vm = 0`, `F = 0`, `D = 0`), laminar phases,
//! upwind convection, one outer and one pressure corrector, walls with a
//! zero-valued velocity as the only boundaries (plus `empty`), the
//! `ddtPhiCorr` filter, `residualAlpha`, the `dgdt` feedback. Refused by
//! name: any other boundary kind.
//!
//! Not translated (gaps, recorded in
//! `outram-foam-appbuilder-lib/verification_and_validation/reacting_two_phase_euler_foam_vs_openfoam.md`):
//! the explicit
//! `dev2(T(grad(U)))` stress term of `divDevRhoReff`, compressible phases,
//! the energy equations (they cannot feed back with `rhoConst`), virtual
//! mass, lift, wall lubrication, turbulent dispersion, blending, `faceMomentum`.

use crate::two_fluid_pimple::TwoFluidPimple;
use crate::MultiphaseError;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, PatchKind, SolverSettings, SurfaceScalarField, Tensor, Vector3,
    VolScalarField, VolVectorField,
};
use std::sync::Arc;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::mass_density::kilogram_per_cubic_meter;

/// ESI `pMin` of the tutorial `phaseProperties` `[Pa]`.
pub const ESI_P_MIN: f64 = 1.0e4;

/// The ESI-form two-fluid stepper. Owns a [`TwoFluidPimple`] (phase fields,
/// fluxes, gravity, drag closure) and the extra state ESI carries between
/// steps: the mixture flux, `p_rgh`, and the compressibility feedback `dgdt`.
pub struct TwoFluidPimpleEsi {
    /// The dev-form coupler whose fields this stepper advances.
    pub core: TwoFluidPimple,
    /// Mixture volumetric flux `phi` `[m3/s]` after the last pressure solve.
    pub phi: SurfaceScalarField,
    /// `p_rgh = p - rho*gh` `[Pa]` per cell.
    pub p_rgh: Vec<f64>,
    /// `fluid.dgdt()` `[1/s]` per cell.
    pub dgdt: Vec<f64>,
    /// `residualAlpha` of both phases (`phaseProperties`).
    pub residual_alpha: f64,
    /// Apply the MULES limiter (`MULES::explicitSolve` -> `limit` ->
    /// `limiter`, `nLimiterIter 3`, no extrema coefficients, `psiMax 1`,
    /// `psiMin 0`) to the relative-flux part of the alpha flux, as upstream
    /// always does. `false` is the explicit ablation: the unlimited flux.
    pub mules_limiter: bool,
    /// `pMin` of `phaseProperties` `[Pa]` (`p = max(p_rgh + rho*gh, pMin)`).
    pub p_min: f64,
    /// Tolerance of the (DIC-preconditioned) conjugate-gradient pressure solve.
    pub p_tolerance: f64,
    /// Iteration cap of the pressure solve.
    pub p_max_iter: usize,
    /// Include the explicit `dev2(T(grad(U)))` part of `divDevRhoReff` in the
    /// momentum source (upstream always does; `false` is the ablation).
    pub explicit_stress: bool,
    /// Record the intermediates of the last step in [`trace`](Self::trace)
    /// (names follow the dump of the ESI `twoPhaseEulerDump` driver).
    pub record_trace: bool,
    /// Intermediates of the last step: `(name, values)`, cell or internal-face
    /// arrays, vectors flattened `x y z`. Empty unless
    /// [`record_trace`](Self::record_trace) is set.
    pub trace: Vec<(String, Vec<f64>)>,
}

/// One wall face: patch index, index in the patch, global face, owner cell.
struct WallFace {
    patch: usize,
    local: usize,
    face: usize,
    cell: usize,
}

fn vol_scalar(mesh: &Arc<FvMesh>, name: &str, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh.patches.iter().map(|p| PatchField::zero_gradient(p.size)).collect();
    VolScalarField::new(name, mesh.clone(), Field::new(vals), boundary)
}

/// `fvc::interpolate` of a cell array to the internal faces, in upstream's
/// `w*(P - N) + N` form.
fn interp(mesh: &FvMesh, x: &[f64]) -> Vec<f64> {
    (0..mesh.n_internal_faces)
        .map(|f| {
            let w = fvc::linear_weight(mesh, f);
            let p = x[mesh.owner[f]];
            let n = x[mesh.neighbour[f]];
            (p - n) * w + n
        })
        .collect()
}

/// Upwind face value of a cell array for the face flux `s`
/// (`upwind::weights() = pos0(phi)`, applied as `w*(P - N) + N`).
fn upwind(mesh: &FvMesh, x: &[f64], f: usize, s: f64) -> f64 {
    let w = if s >= 0.0 { 1.0 } else { 0.0 };
    let p = x[mesh.owner[f]];
    let n = x[mesh.neighbour[f]];
    w * (p - n) + n
}

/// `fvc::surfaceIntegrate` of an internal-face flux (boundary fluxes are zero
/// on the walls this stepper accepts).
fn surface_integrate(mesh: &FvMesh, phi: &[f64]) -> Vec<f64> {
    let mut d = vec![0.0_f64; mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        d[mesh.owner[f]] += phi[f];
        d[mesh.neighbour[f]] -= phi[f];
    }
    (0..mesh.n_cells).map(|c| d[c] / mesh.cell_volumes[c]).collect()
}

/// `MULES::limiter` (ESI v2506 `MULESTemplates.C`) for a mesh whose boundary
/// faces carry no `phiCorr` (walls, `empty`), a boundary `psi` that does not fix
/// its value, `psiMax = 1`, `psiMin = 0`, `rho = 1`, `nLimiterIter = 3`,
/// `extremaCoeff = boundaryExtremaCoeff = smoothLimiter = 0`. Returns the face
/// limiter `lambda` of the internal faces.
fn mules_lambda(
    mesh: &FvMesh,
    psi: &[f64],
    phi_bd: &[f64],
    phi_corr: &[f64],
    sp: &[f64],
    su: &[f64],
    r_delta_t: f64,
) -> Vec<f64> {
    let n = mesh.n_cells;
    let nf = mesh.n_internal_faces;
    let (psi_max, psi_min) = (1.0_f64, 0.0_f64);
    let root_vsmall = 1.0e-150_f64;
    let mut psi_maxn = vec![psi_min; n];
    let mut psi_minn = vec![psi_max; n];
    let mut sum_phi_bd = vec![0.0_f64; n];
    let mut sum_phip = vec![0.0_f64; n];
    let mut m_sum_phim = vec![0.0_f64; n];
    for f in 0..nf {
        let (own, nei) = (mesh.owner[f], mesh.neighbour[f]);
        psi_maxn[own] = psi_maxn[own].max(psi[nei]);
        psi_minn[own] = psi_minn[own].min(psi[nei]);
        psi_maxn[nei] = psi_maxn[nei].max(psi[own]);
        psi_minn[nei] = psi_minn[nei].min(psi[own]);
        sum_phi_bd[own] += phi_bd[f];
        sum_phi_bd[nei] -= phi_bd[f];
        let pc = phi_corr[f];
        if pc > 0.0 {
            sum_phip[own] += pc;
            m_sum_phim[nei] += pc;
        } else {
            m_sum_phim[own] -= pc;
            sum_phip[nei] -= pc;
        }
    }
    for c in 0..n {
        let v = mesh.cell_volumes[c];
        psi_maxn[c] = (psi_maxn[c] + 0.0 * (psi_max - psi_min)).min(psi_max);
        psi_minn[c] = (psi_minn[c] - 0.0 * (psi_max - psi_min)).max(psi_min);
        let (mx, mn) = (psi_maxn[c], psi_minn[c]);
        psi_maxn[c] = v * ((r_delta_t - sp[c]) * mx - su[c] - r_delta_t * psi[c]) + sum_phi_bd[c];
        psi_minn[c] = v * (su[c] - (r_delta_t - sp[c]) * mn + r_delta_t * psi[c]) - sum_phi_bd[c];
    }
    let mut lambda = vec![1.0_f64; nf];
    for _iter in 0..3 {
        let mut sum_lphip = vec![0.0_f64; n];
        let mut m_sum_lphim = vec![0.0_f64; n];
        for f in 0..nf {
            let (own, nei) = (mesh.owner[f], mesh.neighbour[f]);
            let lpc = lambda[f] * phi_corr[f];
            if lpc > 0.0 {
                sum_lphip[own] += lpc;
                m_sum_lphim[nei] += lpc;
            } else {
                m_sum_lphim[own] -= lpc;
                sum_lphip[nei] -= lpc;
            }
        }
        let mut lambdam = vec![0.0_f64; n];
        let mut lambdap = vec![0.0_f64; n];
        for c in 0..n {
            lambdam[c] = ((sum_lphip[c] + psi_maxn[c]) / (m_sum_phim[c] + root_vsmall)).min(1.0).max(0.0);
            lambdap[c] = ((m_sum_lphim[c] + psi_minn[c]) / (sum_phip[c] + root_vsmall)).min(1.0).max(0.0);
        }
        for f in 0..nf {
            let (own, nei) = (mesh.owner[f], mesh.neighbour[f]);
            lambda[f] = if phi_corr[f] > 0.0 {
                lambda[f].min(lambdap[own].min(lambdam[nei]))
            } else {
                lambda[f].min(lambdam[own].min(lambdap[nei]))
            };
        }
    }
    lambda
}

/// The explicit part of the laminar `divDevRhoReff`:
/// `fvc::div((alpha*mu)*dev2(T(grad(U))))` with `Gauss linear` for both the
/// gradient and the interpolation (ESI `laminarModels/Stokes`,
/// `phaseCompressibleMomentumTransportModel`). Returns the value per unit
/// volume. The boundary value of `grad(U)` on a wall is upstream's corrected
/// patch gradient `grad_c + n (snGrad(U) - n . grad_c)`.
fn explicit_stress_div(mesh: &Arc<FvMesh>, u: &VolVectorField, alpha_mu: &[f64]) -> Vec<Vector3> {
    let n = mesh.n_cells;
    let grad = fvc::grad_vec(u);
    let sigma = |g: Tensor, am: f64| -> Tensor { g.transpose().dev2() * am };
    let cell_sigma: Vec<Tensor> = (0..n).map(|c| sigma(grad.internal[c], alpha_mu[c])).collect();
    let mut d = vec![Vector3::ZERO; n];
    for f in 0..mesh.n_internal_faces {
        let w = fvc::linear_weight(mesh, f);
        let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
        let face = (cell_sigma[o] - cell_sigma[nb]) * w + cell_sigma[nb];
        let flux = Tensor::vec_mat(mesh.face_area_vectors[f], face);
        d[o] += flux;
        d[nb] -= flux;
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if patch.kind != PatchKind::Wall {
            continue;
        }
        for local in 0..patch.size {
            let gf = patch.start + local;
            let c = mesh.owner[gf];
            let sf = mesh.face_area_vectors[gf];
            let nhat = sf * (1.0 / sf.mag());
            let delta = nhat.dot(mesh.face_centres[gf] - mesh.cell_centres[c]).abs();
            let ub = u.boundary[pi].values[local];
            let sn_grad = (ub - u.internal[c]) * (1.0 / delta);
            let n_dot_g = Tensor::vec_mat(nhat, grad.internal[c]);
            let gb = grad.internal[c] + nhat * (sn_grad - n_dot_g);
            let flux = Tensor::vec_mat(sf, sigma(gb, alpha_mu[c]));
            d[c] += flux;
        }
    }
    (0..n).map(|c| d[c] * (1.0 / mesh.cell_volumes[c])).collect()
}

impl TwoFluidPimpleEsi {
    /// Wrap a configured [`TwoFluidPimple`] (phase fields, drag, gravity,
    /// no-slip walls). `p0` is the initial static pressure `[Pa]` (the
    /// tutorial: `1e5`). The mixture flux starts at zero, as upstream's does.
    ///
    /// # Errors
    /// [`MultiphaseError::NotImplemented`] if a patch is neither `Wall` nor
    /// `Empty`, or a wall velocity is not zero.
    pub fn new(core: TwoFluidPimple, p0: f64) -> Result<Self, MultiphaseError> {
        for (pi, patch) in core.mesh.patches.iter().enumerate() {
            match patch.kind {
                PatchKind::Empty => {}
                PatchKind::Wall => {
                    for ph in [&core.system.dispersed, &core.system.continuous] {
                        let nonzero = ph.u().boundary[pi]
                            .values
                            .iter()
                            .any(|v| v.x != 0.0 || v.y != 0.0 || v.z != 0.0);
                        if nonzero {
                            return Err(MultiphaseError::NotImplemented(format!(
                                "ESI form: wall patch {} has a non-zero velocity",
                                patch.name
                            )));
                        }
                    }
                }
                _ => {
                    return Err(MultiphaseError::NotImplemented(format!(
                        "ESI form: boundary kind of patch {} (only Wall and Empty are translated)",
                        patch.name
                    )))
                }
            }
        }
        let mesh = core.mesh.clone();
        let n = mesh.n_cells;
        let mut core = core;
        core.p = vol_scalar(&mesh, "p", vec![p0; n]);
        Ok(Self {
            phi: SurfaceScalarField::uniform("phi", mesh.clone(), 0.0),
            p_rgh: vec![p0; n],
            dgdt: vec![0.0; n],
            residual_alpha: 1.0e-6,
            mules_limiter: true,
            p_min: ESI_P_MIN,
            p_tolerance: 1.0e-14,
            p_max_iter: 100_000,
            explicit_stress: true,
            record_trace: false,
            trace: Vec::new(),
            core,
        })
    }

    fn wall_faces(&self) -> Vec<WallFace> {
        let mesh = &self.core.mesh;
        let mut out = Vec::new();
        for (pi, patch) in mesh.patches.iter().enumerate() {
            if patch.kind != PatchKind::Wall {
                continue;
            }
            for local in 0..patch.size {
                let face = patch.start + local;
                out.push(WallFace { patch: pi, local, face, cell: mesh.owner[face] });
            }
        }
        out
    }

    /// Advance one time step `dt` `[s]` with ESI v2506 `twoPhaseEulerFoam`'s
    /// algorithm (see the module documentation for the scope).
    ///
    /// # Errors
    /// - [`MultiphaseError::InvalidInput`] if `dt <= 0`.
    /// - [`MultiphaseError::Solver`] on a non-finite pressure solution.
    /// - the drag closure's errors.
    pub fn solve_timestep_esi(&mut self, dt: f64) -> Result<(), MultiphaseError> {
        if dt <= 0.0 {
            return Err(MultiphaseError::InvalidInput(format!("dt must be > 0 (got {dt})")));
        }
        let mesh = self.core.mesh.clone();
        let n = mesh.n_cells;
        let nf = mesh.n_internal_faces;
        let rho1 = self.core.system.dispersed.rho().get::<kilogram_per_cubic_meter>();
        let rho2 = self.core.system.continuous.rho().get::<kilogram_per_cubic_meter>();
        let mu1 = self.core.system.dispersed.mu().get::<pascal_second>();
        let mu2 = self.core.system.continuous.mu().get::<pascal_second>();
        let g = self.core.gravity;
        let res = self.residual_alpha;
        let r_delta_t = 1.0 / dt;
        let walls = self.wall_faces();

        // ---- state entering the step --------------------------------------------
        let a1_old: Vec<f64> = (0..n).map(|c| self.core.system.dispersed.alpha().internal[c]).collect();
        let a2_old: Vec<f64> = a1_old.iter().map(|a| 1.0 - a).collect();
        let u1_old = self.core.system.dispersed.u().clone();
        let u2_old = self.core.system.continuous.u().clone();
        let phi1_old: Vec<f64> = self.core.phi_d.internal.as_slice().to_vec();
        let phi2_old: Vec<f64> = self.core.phi_c.internal.as_slice().to_vec();
        let phic: Vec<f64> = self.phi.internal.as_slice().to_vec();

        // ---- twoPhaseSystem::solve(): explicit alpha transport --------------------
        let one_minus_a1: Vec<f64> = a1_old.iter().map(|a| 1.0 - a).collect();
        let phi_bd: Vec<f64> = (0..nf).map(|f| phic[f] * upwind(&mesh, &a1_old, f, phic[f])).collect();
        let alpha_phic1_full: Vec<f64> = (0..nf)
            .map(|f| {
                let phir = phi1_old[f] - phi2_old[f];
                let first = phic[f] * upwind(&mesh, &a1_old, f, phic[f]);
                let neg_phir = -phir;
                let inner = -(neg_phir * upwind(&mesh, &one_minus_a1, f, neg_phir));
                first + inner * upwind(&mesh, &a1_old, f, inner)
            })
            .collect();
        let div_phic = surface_integrate(&mesh, &phic);
        let mut sp_cell = vec![0.0_f64; n];
        let mut su_cell = vec![0.0_f64; n];
        for c in 0..n {
            su_cell[c] = div_phic[c] * a1_old[c].min(1.0);
            let dg = self.dgdt[c];
            if dg > 0.0 {
                sp_cell[c] -= dg / (1.0 - a1_old[c]).max(1e-4);
                su_cell[c] += dg / (1.0 - a1_old[c]).max(1e-4);
            } else if dg < 0.0 {
                sp_cell[c] += dg / a1_old[c].max(1e-4);
            }
        }
        let alpha_phic1: Vec<f64> = if self.mules_limiter {
            let phi_corr: Vec<f64> = (0..nf).map(|f| alpha_phic1_full[f] - phi_bd[f]).collect();
            let lambda = mules_lambda(&mesh, &a1_old, &phi_bd, &phi_corr, &sp_cell, &su_cell, r_delta_t);
            (0..nf).map(|f| phi_bd[f] + lambda[f] * phi_corr[f]).collect()
        } else {
            alpha_phic1_full
        };
        let div_alpha_phic1 = surface_integrate(&mesh, &alpha_phic1);
        let mut a1 = vec![0.0_f64; n];
        for c in 0..n {
            a1[c] = (a1_old[c] * r_delta_t + su_cell[c] - div_alpha_phic1[c]) / (r_delta_t - sp_cell[c]);
        }
        let alpha_phi1 = alpha_phic1;
        let alpha_phi2: Vec<f64> = (0..nf).map(|f| phic[f] - alpha_phi1[f]).collect();
        for v in a1.iter_mut() {
            *v = v.clamp(0.0, 1.0);
        }
        let a2: Vec<f64> = a1.iter().map(|a| 1.0 - a).collect();
        for c in 0..n {
            self.core.system.dispersed.alpha_mut().internal[c] = a1[c];
        }
        self.core.system.enforce_alpha_constraint();

        // ---- contErrs.H ------------------------------------------------------------
        let alpha_rho_phi1: Vec<f64> = alpha_phi1.iter().map(|v| rho1 * v).collect();
        let alpha_rho_phi2: Vec<f64> = alpha_phi2.iter().map(|v| rho2 * v).collect();
        let div_arp1 = surface_integrate(&mesh, &alpha_rho_phi1);
        let div_arp2 = surface_integrate(&mesh, &alpha_rho_phi2);
        let cont_err1: Vec<f64> =
            (0..n).map(|c| r_delta_t * (a1[c] * rho1 - a1_old[c] * rho1) + div_arp1[c]).collect();
        let cont_err2: Vec<f64> =
            (0..n).map(|c| r_delta_t * (a2[c] * rho2 - a2_old[c] * rho2) + div_arp2[c]).collect();

        // ---- pU/UEqns.H: momentum matrices (assembled, not solved) -----------------
        let kd = self.core.drag.k_d(&self.core.system)?;
        let arp1 = SurfaceScalarField::uniform("alphaRhoPhi1", mesh.clone(), 0.0);
        let arp2 = SurfaceScalarField::uniform("alphaRhoPhi2", mesh.clone(), 0.0);
        let mut arp1 = arp1;
        let mut arp2 = arp2;
        for f in 0..nf {
            arp1.internal[f] = alpha_rho_phi1[f];
            arp2.internal[f] = alpha_rho_phi2[f];
        }
        let build = |a_new: &[f64],
                     a_old: &[f64],
                     rho: f64,
                     mu: f64,
                     arp: &SurfaceScalarField,
                     cont_err: &[f64],
                     u: &VolVectorField,
                     u_old: &VolVectorField| {
            let stress = if self.explicit_stress {
                let am: Vec<f64> = (0..n).map(|c| a_new[c] * mu).collect();
                explicit_stress_div(&mesh, u, &am)
            } else {
                vec![Vector3::ZERO; n]
            };
            let mut m = fvm::div_vec(arp, u, mesh.clone());
            let gamma = vol_scalar(&mesh, "alphaMu", (0..n).map(|c| a_new[c] * mu).collect());
            let lap = fvm::laplacian_vec(&gamma, u, mesh.clone());
            m = m + lap;
            for c in 0..n {
                let v = mesh.cell_volumes[c];
                // fvm::ddt(alpha, rho, U): new coefficient on the diagonal,
                // old coefficient times the old velocity in the source.
                m.ldu.diag[c] += a_new[c] * rho * r_delta_t * v;
                m.source[c] = m.source[c] + u_old.internal[c] * (a_old[c] * rho * r_delta_t * v);
                // - fvm::Sp(contErr, U) and + fvm::Sp(Kd, U)
                m.ldu.diag[c] -= cont_err[c] * v;
                m.ldu.diag[c] += kd.internal[c] * v;
                // + divDevRhoReff explicit part: - fvc::div(...) on the left
                m.source[c] = m.source[c] + stress[c] * v;
            }
            m
        };
        let m1 = build(&a1, &a1_old, rho1, mu1, &arp1, &cont_err1, self.core.system.dispersed.u(), &u1_old);
        let m2 = build(&a2, &a2_old, rho2, mu2, &arp2, &cont_err2, self.core.system.continuous.u(), &u2_old);

        // ---- pU/pEqn.H ------------------------------------------------------------
        let vol = |c: usize| mesh.cell_volumes[c];
        let rau1: Vec<f64> = (0..n)
            .map(|c| 1.0 / (m1.ldu.diag[c] / vol(c) + (res - a1[c]).max(0.0) * rho1 / dt))
            .collect();
        let rau2: Vec<f64> = (0..n)
            .map(|c| 1.0 / (m2.ldu.diag[c] / vol(c) + (res - a2[c]).max(0.0) * rho2 / dt))
            .collect();

        let h1 = m1.h_field(self.core.system.dispersed.u());
        let h2 = m2.h_field(self.core.system.continuous.u());
        let hbya_vec = |h: &VolVectorField, rau: &[f64], a: &[f64], rho: f64, u_old: &VolVectorField| -> Vec<Vector3> {
            (0..n)
                .map(|c| {
                    let hv = h.internal[c] * (1.0 / vol(c));
                    (hv + u_old.internal[c] * ((res - a[c]).max(0.0) * rho / dt)) * rau[c]
                })
                .collect()
        };
        let hbya1_cells = hbya_vec(&h1, &rau1, &a1, rho1, &u1_old);
        let hbya2_cells = hbya_vec(&h2, &rau2, &a2, rho2, &u2_old);
        let hbya1 = VolVectorField::new(
            "HbyA.air",
            mesh.clone(),
            Field::new(hbya1_cells.clone()),
            self.core.system.dispersed.u().boundary.clone(),
        );
        let hbya2 = VolVectorField::new(
            "HbyA.water",
            mesh.clone(),
            Field::new(hbya2_cells.clone()),
            self.core.system.continuous.u().boundary.clone(),
        );

        // rho, gh
        let rho_mix: Vec<f64> = (0..n).map(|c| a1[c] * rho1 + a2[c] * rho2).collect();
        let gh_cell: Vec<f64> = (0..n).map(|c| g.dot(mesh.cell_centres[c])).collect();
        let p_start: Vec<f64> = (0..n).map(|c| self.core.p.internal[c]).collect();
        let p_rgh_start: Vec<f64> = (0..n).map(|c| p_start[c] - rho_mix[c] * gh_cell[c]).collect();

        let alphaf1 = interp(&mesh, &a1);
        let alphaf2: Vec<f64> = alphaf1.iter().map(|a| 1.0 - a).collect();
        let alpha_rau1_cell: Vec<f64> = (0..n).map(|c| a1[c].max(res) * rau1[c]).collect();
        let alpha_rau2_cell: Vec<f64> = (0..n).map(|c| a2[c].max(res) * rau2[c]).collect();
        let alpharauf1 = interp(&mesh, &alpha_rau1_cell);
        let alpharauf2 = interp(&mesh, &alpha_rau2_cell);

        // ghSnGradRho = ghf*snGrad(rho)*magSf, phig1, phig2
        let rho_vol = vol_scalar(&mesh, "rho", rho_mix.clone());
        let sn_rho = fvc::sn_grad(&rho_vol);
        let g_dot_sf: Vec<f64> = (0..nf).map(|f| g.dot(mesh.face_area_vectors[f])).collect();
        let gh_sn_grad_rho: Vec<f64> = (0..nf)
            .map(|f| g.dot(mesh.face_centres[f]) * sn_rho.internal[f] * mesh.face_areas[f])
            .collect();
        let phig1: Vec<f64> = (0..nf)
            .map(|f| alpharauf1[f] * (gh_sn_grad_rho[f] - alphaf2[f] * (rho1 - rho2) * g_dot_sf[f]))
            .collect();
        let phig2: Vec<f64> = (0..nf)
            .map(|f| alpharauf2[f] * (gh_sn_grad_rho[f] - alphaf1[f] * (rho2 - rho1) * g_dot_sf[f]))
            .collect();

        // ddtPhiCorr filter: only in pure(ish) phases
        let mut sum_mag = vec![0.0_f64; n];
        let mut sum_mag_alpha = vec![0.0_f64; n];
        for f in 0..nf {
            let (o, nb) = (mesh.owner[f], mesh.neighbour[f]);
            let ms = mesh.face_areas[f];
            sum_mag[o] += ms;
            sum_mag[nb] += ms;
            sum_mag_alpha[o] += ms * alphaf1[f];
            sum_mag_alpha[nb] += ms * alphaf1[f];
        }
        for w in &walls {
            let ms = mesh.face_areas[w.face];
            sum_mag[w.cell] += ms;
            sum_mag_alpha[w.cell] += ms * a1[w.cell];
        }
        let avg: Vec<f64> = (0..n).map(|c| sum_mag_alpha[c] / sum_mag[c]).collect();
        let alphaf1_bar = interp(&mesh, &avg);
        let corr_coeff1: Vec<f64> = alphaf1_bar.iter().map(|b| if b - 0.99 >= 0.0 { 1.0 } else { 0.0 }).collect();
        let corr_coeff2: Vec<f64> = alphaf1_bar.iter().map(|b| if 0.01 - b >= 0.0 { 1.0 } else { 0.0 }).collect();
        let old_rau1: Vec<f64> = (0..n).map(|c| a1_old[c] * rho1 * rau1[c]).collect();
        let old_rau2: Vec<f64> = (0..n).map(|c| a2_old[c] * rho2 * rau2[c]).collect();
        let f_old_rau1 = interp(&mesh, &old_rau1);
        let f_old_rau2 = interp(&mesh, &old_rau2);
        let flux_u1_old = fvc::flux(&u1_old);
        let flux_u2_old = fvc::flux(&u2_old);

        let flux_h1 = fvc::flux(&hbya1);
        let flux_h2 = fvc::flux(&hbya2);
        let phihbya1: Vec<f64> = (0..nf)
            .map(|f| {
                flux_h1.internal[f]
                    + corr_coeff1[f] * f_old_rau1[f] * (phi1_old[f] - flux_u1_old.internal[f]) / dt
                    - 0.0
                    - phig1[f]
            })
            .collect();
        let phihbya2: Vec<f64> = (0..nf)
            .map(|f| {
                flux_h2.internal[f]
                    + corr_coeff2[f] * f_old_rau2[f] * (phi2_old[f] - flux_u2_old.internal[f]) / dt
                    - 0.0
                    - phig2[f]
            })
            .collect();

        let rau_kd1_cell: Vec<f64> = (0..n).map(|c| rau1[c] * kd.internal[c]).collect();
        let rau_kd2_cell: Vec<f64> = (0..n).map(|c| rau2[c] * kd.internal[c]).collect();
        let raukd1 = interp(&mesh, &rau_kd1_cell);
        let raukd2 = interp(&mesh, &rau_kd2_cell);

        let phihbya: Vec<f64> = (0..nf)
            .map(|f| {
                alphaf1[f] * (phihbya1[f] + raukd1[f] * phi2_old[f])
                    + alphaf2[f] * (phihbya2[f] + raukd2[f] * phi1_old[f])
            })
            .collect();
        let rauf: Vec<f64> = (0..nf)
            .map(|f| (alphaf1[f] * alpharauf1[f] + alphaf2[f] * alpharauf2[f]).abs())
            .collect();

        // Wall-face boundary values (fixedFluxPressure makes the net wall flux
        // zero; phase wall fluxes are fixed to zero).
        let mut wall_alpharau1 = Vec::with_capacity(walls.len());
        let mut wall_alpharau2 = Vec::with_capacity(walls.len());
        let mut wall_phig1 = Vec::with_capacity(walls.len());
        let mut wall_phig2 = Vec::with_capacity(walls.len());
        let mut wall_m_sf_gradp = Vec::with_capacity(walls.len());
        for w in &walls {
            let c = w.cell;
            let ar1 = a1[c].max(res) * rau1[c];
            let ar2 = a2[c].max(res) * rau2[c];
            let gdotsf = g.dot(mesh.face_area_vectors[w.face]);
            let pg1 = ar1 * (0.0 - a2[c] * (rho1 - rho2) * gdotsf);
            let pg2 = ar2 * (0.0 - a1[c] * (rho2 - rho1) * gdotsf);
            let phb1 = -pg1;
            let phb2 = -pg2;
            let phb = a1[c] * phb1 + a2[c] * phb2;
            let rauf_b = (a1[c] * ar1 + a2[c] * ar2).abs();
            wall_alpharau1.push(ar1);
            wall_alpharau2.push(ar2);
            wall_phig1.push(pg1);
            wall_phig2.push(pg2);
            wall_m_sf_gradp.push(-phb / rauf_b);
        }

        // compressibility parts of the pressure equation (psi = 0): the field
        // (contErr - (ddt(alpha) + div(alphaPhi))*rho)/rho of each phase
        let ddt_plus_div1: Vec<f64> =
            (0..n).map(|c| r_delta_t * (a1[c] - a1_old[c]) + surface_integrate(&mesh, &alpha_phi1)[c]).collect();
        let ddt_plus_div2: Vec<f64> =
            (0..n).map(|c| r_delta_t * (a2[c] - a2_old[c]) + surface_integrate(&mesh, &alpha_phi2)[c]).collect();
        let comp1: Vec<f64> = (0..n).map(|c| (cont_err1[c] - ddt_plus_div1[c] * rho1) / rho1).collect();
        let comp2: Vec<f64> = (0..n).map(|c| (cont_err2[c] - ddt_plus_div2[c] * rho2) / rho2).collect();

        // pressure equation: div(phiHbyA) + comp - laplacian(rAUf, p_rgh) = 0
        let div_phihbya = surface_integrate(&mesh, &phihbya);
        let rauf_surf = {
            let mut s = SurfaceScalarField::uniform("rAUf", mesh.clone(), 0.0);
            for f in 0..nf {
                s.internal[f] = rauf[f];
            }
            s
        };
        let p_rgh_field = vol_scalar(&mesh, "p_rgh", p_rgh_start.clone());
        let mut p_eqn = fvm::laplacian(&rauf_surf, &p_rgh_field);
        for c in 0..n {
            p_eqn.source[c] = -vol(c) * (comp1[c] + comp2[c] + div_phihbya[c]);
        }
        p_eqn.set_reference_openfoam(0, p_rgh_start[0]);
        let settings = SolverSettings { tolerance: self.p_tolerance, max_iter: self.p_max_iter };
        let (p_rgh_new, _perf) = p_eqn.solve_cg_with_guess("p_rgh", &p_rgh_field, settings);
        for c in 0..n {
            if !p_rgh_new.internal[c].is_finite() {
                return Err(MultiphaseError::Solver(format!("p_rgh non-finite in cell {c}")));
            }
        }
        let sn_p = fvc::sn_grad(&p_rgh_new);

        // fluxes
        let flux_inc: Vec<f64> = (0..nf).map(|f| -(rauf[f] * mesh.face_areas[f]) * sn_p.internal[f]).collect();
        let phi_new: Vec<f64> = (0..nf).map(|f| phihbya[f] + flux_inc[f]).collect();
        let m_sf_gradp: Vec<f64> = (0..nf).map(|f| flux_inc[f] / rauf[f]).collect();
        let mut phi1_new = vec![0.0_f64; nf];
        let mut phi2_new = vec![0.0_f64; nf];
        for f in 0..nf {
            let phi1s = phihbya1[f] + alpharauf1[f] * m_sf_gradp[f];
            let phi2s = phihbya2[f] + alpharauf2[f] * m_sf_gradp[f];
            let phir = ((phi1s + raukd1[f] * phi2s) - (phi2s + raukd2[f] * phi1s))
                / (1.0 - raukd1[f] * raukd2[f]);
            phi1_new[f] = phi_new[f] + alphaf2[f] * phir;
            phi2_new[f] = phi_new[f] - alphaf1[f] * phir;
        }

        if self.record_trace {
            let flat3 = |v: &[Vector3]| -> Vec<f64> { v.iter().flat_map(|a| [a.x, a.y, a.z]).collect() };
            let mut t: Vec<(String, Vec<f64>)> = Vec::new();
            t.push(("Kd".into(), kd.internal.as_slice().to_vec()));
            t.push(("rAU1".into(), rau1.clone()));
            t.push(("rAU2".into(), rau2.clone()));
            t.push(("HbyA1".into(), flat3(&hbya1_cells)));
            t.push(("HbyA2".into(), flat3(&hbya2_cells)));
            t.push(("phiHbyA1".into(), phihbya1.clone()));
            t.push(("phiHbyA2".into(), phihbya2.clone()));
            t.push(("phiHbyA".into(), phihbya.clone()));
            t.push(("rAUf".into(), rauf.clone()));
            t.push(("alphaf1".into(), alphaf1.clone()));
            t.push(("phig1".into(), phig1.clone()));
            t.push(("phig2".into(), phig2.clone()));
            t.push(("alpharAUf1".into(), alpharauf1.clone()));
            t.push(("alpharAUf2".into(), alpharauf2.clone()));
            t.push(("ghSnGradRho".into(), gh_sn_grad_rho.clone()));
            t.push(("rho".into(), rho_mix.clone()));
            t.push(("gh".into(), gh_cell.clone()));
            t.push(("p_rgh".into(), p_rgh_new.internal.as_slice().to_vec()));
            t.push(("phi1".into(), phi1_new.clone()));
            t.push(("phi2".into(), phi2_new.clone()));
            t.push(("alpha1".into(), a1.clone()));
            self.trace = t;
        }

        // dgdt for the next step
        for c in 0..n {
            self.dgdt[c] = a1[c] * comp2[c] - a2[c] * comp1[c];
        }

        // velocity: partial-elimination corrector
        let mut srf1 = SurfaceScalarField::uniform("srf1", mesh.clone(), 0.0);
        let mut srf2 = SurfaceScalarField::uniform("srf2", mesh.clone(), 0.0);
        for f in 0..nf {
            srf1.internal[f] = alpharauf1[f] * m_sf_gradp[f] - 0.0 - phig1[f];
            srf2.internal[f] = alpharauf2[f] * m_sf_gradp[f] - 0.0 - phig2[f];
        }
        for (i, w) in walls.iter().enumerate() {
            srf1.boundary[w.patch].values[w.local] = wall_alpharau1[i] * wall_m_sf_gradp[i] - 0.0 - wall_phig1[i];
            srf2.boundary[w.patch].values[w.local] = wall_alpharau2[i] * wall_m_sf_gradp[i] - 0.0 - wall_phig2[i];
        }
        let rec1 = fvc::reconstruct(&srf1);
        let rec2 = fvc::reconstruct(&srf2);
        let mut u1_new = vec![Vector3::ZERO; n];
        let mut u2_new = vec![Vector3::ZERO; n];
        for c in 0..n {
            let us1 = hbya1_cells[c] + rec1.internal[c];
            let us2 = hbya2_cells[c] + rec2.internal[c];
            let d1 = rau1[c] * kd.internal[c];
            let d2 = rau2[c] * kd.internal[c];
            let u = (us1 + u2_old.internal[c] * d1) * a1[c] + (us2 + u1_old.internal[c] * d2) * a2[c];
            let ur = (us1 * (1.0 - d2) - us2 * (1.0 - d1)) * (1.0 / (1.0 - d1 * d2));
            u1_new[c] = u + ur * a2[c];
            u2_new[c] = u - ur * a1[c];
        }

        // pressure update
        let mut p_new = vec![0.0_f64; n];
        let mut p_rgh_final = vec![0.0_f64; n];
        for c in 0..n {
            p_new[c] = (p_rgh_new.internal[c] + rho_mix[c] * gh_cell[c]).max(self.p_min);
            p_rgh_final[c] = p_new[c] - rho_mix[c] * gh_cell[c];
        }

        // ---- commit ------------------------------------------------------------------
        for c in 0..n {
            self.core.system.dispersed.u_mut().internal[c] = u1_new[c];
            self.core.system.continuous.u_mut().internal[c] = u2_new[c];
            self.core.p.internal[c] = p_new[c];
        }
        self.p_rgh = p_rgh_final;
        for f in 0..nf {
            self.phi.internal[f] = phi_new[f];
            self.core.phi_d.internal[f] = phi1_new[f];
            self.core.phi_c.internal[f] = phi2_new[f];
        }
        Ok(())
    }
}
