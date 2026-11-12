// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/SN/include/
//                    {readQuadratureSet,calcLegendreMatrices,legendrePoly,
//                     solveNeutronicsSN,fluxEqSN,calcSourceSN,
//                     calcThisIntraSource,updateScalarFluxes,
//                     updateThisScalarFlux,normFluxesSN}.H/.C,
//                    SN/SNNeutronics.C (facePhis_, maxDeltaT), and
//                    src/classes/neutronics/include/{calcKeff,precEq}.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Nordine Kerkar, Konstantin Mikityuk (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.


//! GeN-Foam's S_N eigenvalue time step, with the case's own `quadratureSet` and
//! anisotropic (Legendre-moment) scattering (gh:#1474, gh:#1361).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! [`SnNeutronics`](super::SnNeutronics) keeps its own algorithm (a static
//! power iteration on the baked-in level-symmetric sets with isotropic
//! scattering, weights summing to `4 pi`); it is untouched. This module is the
//! translation of `SNNeutronics`'s `correctPhysics()`:
//! `solveNeutronicsSN.H`, `fluxEqSN.H`, `calcSourceSN.H`,
//! `calcThisIntraSource.H`, `updateScalarFluxes.H`, `normFluxesSN.H`, with
//! upstream's conventions (weights sum to 1, no `1/(4 pi)` in the source).
//!
//! ## What one step does (eigenvalue branch)
//!
//! 1. `flux_g = sum_d w_d psi_{g,d}`; `pTot = power()`; the precursors and the
//!    delayed source from the fission source at the old `keff_` (stationary
//!    fuel: the diagonal solve `(V S/k beta)/(V lambda)`).
//! 2. On the first step only: every flux scaled by `pTarget / pTot`.
//! 3. For each group `g`, `Gauss-Seidel` over directions, repeating (at most
//!    `maxInnerIter`, until the inner residual is no more than
//!    `innToOutTolerance` times the step's largest) :
//!    `div(Omega_d psi) + Sp(Sigma_r + Sigma_{s0,g->g}) psi = extra_d + intra_d +
//!    delayed chi_d`, where `intra_d` re-evaluates the within-group fission and
//!    P0 source at the current `flux_g` plus, for moments `L >= 1`,
//!    `sum_{d'} psi_{g,d'} w_{d'} sum_L P_L(Omega_d' . Omega_d)(2L+1) Sigma_{sL,g->g}`,
//!    and `extra_d` the same from the other groups (fixed over the inner loop).
//! 4. `power()`, `calcKeff.H`, and every flux scaled by `pTarget / pTot`.
//!
//! The linear solver is basic-lib's ILU(0) BiCGStab (the analogue of
//! `PBiCGStab`/`DILU`), stopping on `||r||/||b||`, not upstream's residual.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvm, BoundaryCondition, Field, FvMesh, KrylovOptions, SolverSettings, SurfaceScalarField,
    VolScalarField,
};

use super::quadrature_set::QuadratureSet;
use crate::genfoam::neutronics::diffusion::DiffusionXsFields;
use crate::genfoam::neutronics::pseudo_time::{
    calc_keff, sp3_max_delta_t, PseudoTimeClock, PseudoTimeControls, PseudoTimeEigenvalue,
    PseudoTimeError, PseudoTimeStep,
};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::NeutronicsError;

/// Controls of the S_N time step: the `neutronTransport` entries of
/// `fvSolution` that `solveNeutronicsSN.H` reads, plus the `legendreMoments`
/// of `nuclearData` and the linear solver.
#[derive(Debug, Clone, Copy)]
pub struct SnPseudoTimeSettings {
    /// The number of scattering matrices used, `xs_.legendreMoments()` =
    /// `1 + legendreMoments` of `nuclearData` (`XS.C:75`): the Godiva tutorial's
    /// `legendreMoments 3` is 4 here (P0, P1, P2, P3). Must not exceed the stored
    /// matrices.
    pub legendre_moments: usize,
    /// `innToOutTolerance` (default 0.01).
    pub inn_to_out_tolerance: f64,
    /// `maxInnerIter` (default 3).
    pub max_inner_iter: usize,
    /// Per-direction linear solve.
    pub linear: SolverSettings,
}

impl Default for SnPseudoTimeSettings {
    fn default() -> Self {
        Self {
            legendre_moments: 1,
            inn_to_out_tolerance: 0.01,
            max_inner_iter: 3,
            linear: SolverSettings {
                tolerance: 1e-13,
                max_iter: 100_000,
            },
        }
    }
}

/// An S_N model that takes GeN-Foam's eigenvalue time step. See the module doc.
#[derive(Debug, Clone)]
pub struct SnPseudoTimeSolver {
    mesh: Arc<FvMesh>,
    quadrature: QuadratureSet,
    legendre: Vec<Vec<Vec<f64>>>,
    xs: DiffusionXsFields,
    /// `sigmaFromTo[L][from][to][cell]` for `L < legendre_moments`.
    sigma_from_to: Vec<Vec<Vec<Vec<f64>>>>,
    /// `sigmaFromToYesNo[from][to]` (upstream: P0 reference entry > 5e-6).
    exchange: Vec<Vec<bool>>,
    face_phi: Vec<SurfaceScalarField>,
    /// `angularFlux_[group][direction]`.
    psi: Vec<Vec<VolScalarField>>,
    /// `flux_[group]`, internal values.
    flux: Vec<Vec<f64>>,
    k_eff: f64,
    settings: SnPseudoTimeSettings,
}

impl SnPseudoTimeSolver {
    /// Build the model: cross sections at the reference point, the case's
    /// quadrature, the Legendre matrices, the face fluxes, and the angular flux
    /// seeded with `defaultFlux = 1` and carrying `flux_boundary` (one boundary
    /// condition per patch; `Godiva_SN` uses `inletOutlet` with inlet value 0).
    ///
    /// `k_eff` is the `keff` of `0/uniform/reactorState`.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError`] from the cross-section materialisation, or
    /// [`NeutronicsError::PatchCountMismatch`].
    pub fn new(
        mesh: Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        quadrature: QuadratureSet,
        flux_boundary: &[BoundaryCondition<f64>],
        k_eff: f64,
        settings: SnPseudoTimeSettings,
    ) -> Result<Self, NeutronicsError> {
        if flux_boundary.len() != mesh.patches.len() {
            return Err(NeutronicsError::PatchCountMismatch {
                given: flux_boundary.len(),
                patches: mesh.patches.len(),
            });
        }
        let xs_fields = DiffusionXsFields::materialize(&mesh, xs, zone_of_cell, &[])?;
        let g = xs_fields.energy_groups;
        let n = mesh.n_cells;
        let n_dir = quadrature.direction_count();
        let legendre = quadrature.legendre_matrices(settings.legendre_moments);

        let mut sigma_from_to = vec![vec![vec![vec![0.0; n]; g]; g]; settings.legendre_moments];
        for c in 0..n {
            for (l, per_moment) in sigma_from_to.iter_mut().enumerate() {
                let m = xs.scattering_matrix(zone_of_cell[c], l, &[])?;
                for j in 0..g {
                    for i in 0..g {
                        per_moment[j][i][c] = m.get(j, i);
                    }
                }
            }
        }
        let mut exchange = vec![vec![false; g]; g];
        for zone in 0..xs.zone_count() {
            let m = xs.scattering_matrix(zone, 0, &[])?;
            for j in 0..g {
                for i in 0..g {
                    if m.get(j, i) > 5.0e-6 {
                        exchange[j][i] = true;
                    }
                }
            }
        }

        let face_phi = (0..n_dir).map(|d| quadrature.face_phi(&mesh, d)).collect();
        let psi = (0..g)
            .map(|gg| {
                (0..n_dir)
                    .map(|d| {
                        let mut f = VolScalarField::uniform(format!("angularFlux_{gg}_{d}"), mesh.clone(), 1.0);
                        for (patch, bc) in f.boundary.iter_mut().zip(flux_boundary.iter()) {
                            patch.bc = bc.clone();
                            if let BoundaryCondition::FixedValue(v) = bc {
                                patch.values = Field::uniform(patch.values.len(), *v);
                            }
                        }
                        f
                    })
                    .collect()
            })
            .collect();

        Ok(Self {
            flux: vec![vec![1.0; n]; g],
            mesh,
            quadrature,
            legendre,
            xs: xs_fields,
            sigma_from_to,
            exchange,
            face_phi,
            psi,
            k_eff,
            settings,
        })
    }

    /// The current `keff_`.
    #[must_use]
    pub fn k_eff(&self) -> f64 {
        self.k_eff
    }

    /// The scalar flux of group `g`, per cell.
    #[must_use]
    pub fn flux(&self, group: usize) -> &[f64] {
        &self.flux[group]
    }

    /// The quadrature in use.
    #[must_use]
    pub fn quadrature(&self) -> &QuadratureSet {
        &self.quadrature
    }

    /// `power()`: `sum_g sum_c V (flux sigmaPow)`.
    fn power(&self) -> f64 {
        let vols = &self.mesh.cell_volumes;
        let mut p_tot = 0.0;
        for (g, flux) in self.flux.iter().enumerate() {
            let sp = self.xs.sigma_pow[g].internal.as_slice();
            let mut integral = 0.0;
            for c in 0..vols.len() {
                integral += vols[c] * (flux[c] * sp[c]);
            }
            p_tot += integral;
        }
        p_tot
    }

    /// `updateScalarFluxes.H`: `flux_g = sum_d psi_{g,d} w_d`.
    fn update_scalar_fluxes(&mut self) {
        let n = self.mesh.n_cells;
        for g in 0..self.flux.len() {
            let mut f = vec![0.0; n];
            for (d, w) in self.quadrature.weights().iter().enumerate() {
                let psi = self.psi[g][d].internal.as_slice();
                for c in 0..n {
                    f[c] += psi[c] * w;
                }
            }
            self.flux[g] = f;
        }
    }

    /// `flux_ *= ratio` and `psi *= ratio` (`normFluxesSN.H`).
    fn scale_all(&mut self, ratio: f64) {
        for f in self.flux.iter_mut() {
            for v in f.iter_mut() {
                *v *= ratio;
            }
        }
        for group in self.psi.iter_mut() {
            for field in group.iter_mut() {
                for v in field.internal.as_mut_slice() {
                    *v *= ratio;
                }
            }
        }
    }

    /// `sum_{L=1}^{M-1} P_L(Omega_dj . Omega_d)(2L+1) Sigma_{sL, from->to}` per cell.
    fn moment_coefficient(&self, dj: usize, d: usize, from: usize, to: usize) -> Vec<f64> {
        let n = self.mesh.n_cells;
        let mut coef = vec![0.0; n];
        for l in 1..self.settings.legendre_moments {
            let scattering_coeff = self.legendre[l][dj][d] * (2.0 * (l as f64) + 1.0);
            let sft = &self.sigma_from_to[l][from][to];
            for c in 0..n {
                coef[c] += scattering_coeff * sft[c];
            }
        }
        coef
    }

    /// One `correctPhysics()` of `SNNeutronics` for `eigenvalueNeutronics true`.
    fn eigenvalue_step(&mut self, p_target: f64, first_step: bool) -> PseudoTimeStep {
        let groups = self.xs.energy_groups;
        let n = self.mesh.n_cells;
        let n_dir = self.quadrature.direction_count();
        let vols = self.mesh.cell_volumes.clone();
        let keff = self.k_eff;
        let moments = self.settings.legendre_moments;

        self.update_scalar_fluxes();
        let p_tot0 = self.power();

        // initializeNeutroSource.H, precEq.H (diagonal solve), delayed source.
        let mut neutro_source = vec![0.0; n];
        for g in 0..groups {
            let nf = self.xs.nu_sigma_f[g].internal.as_slice();
            for c in 0..n {
                neutro_source[c] += nf[c] * self.flux[g][c];
            }
        }
        let mut delayed = vec![0.0; n];
        for kk in 0..self.xs.prec_groups {
            let beta = self.xs.beta[kk].internal.as_slice();
            let lambda = self.xs.lambda[kk].internal.as_slice();
            for c in 0..n {
                let x = (neutro_source[c] / keff) * beta[c];
                let prec = (vols[c] * x) / (vols[c] * lambda[c]);
                delayed[c] += lambda[c] * prec;
            }
        }

        if first_step {
            self.scale_all(p_target / p_tot0);
        }
        self.update_scalar_fluxes();

        let one_minus_beta: Vec<f64> = self.xs.beta_tot.internal.as_slice().iter().map(|b| 1.0 - b).collect();
        let weights = self.quadrature.weights().to_vec();
        let mut neutro_residual = 0.0_f64;

        for g in 0..groups {
            let nu_sigma = self.xs.nu_sigma_f[g].internal.as_slice().to_vec();
            let chi_p = self.xs.chi_prompt[g].internal.as_slice().to_vec();
            let chi_d = self.xs.chi_delayed[g].internal.as_slice().to_vec();
            let sft0_gg = self.sigma_from_to[0][g][g].clone();

            // calcSourceSN.H: extra-group source.
            let mut iso_extra = vec![0.0; n];
            for j in 0..groups {
                if j != g {
                    let nu_j = self.xs.nu_sigma_f[j].internal.as_slice();
                    let sft0_jg = &self.sigma_from_to[0][j][g];
                    for c in 0..n {
                        iso_extra[c] += self.flux[j][c]
                            * ((nu_j[c] * chi_p[c]) * (one_minus_beta[c] / keff) + sft0_jg[c]);
                    }
                }
            }
            let mut extra: Vec<Vec<f64>> = vec![iso_extra; n_dir];
            if moments > 1 {
                for j in 0..groups {
                    if j != g && self.exchange[j][g] {
                        for d in 0..n_dir {
                            for dj in 0..n_dir {
                                let coef = self.moment_coefficient(dj, d, j, g);
                                let psi = self.psi[j][dj].internal.as_slice();
                                for c in 0..n {
                                    extra[d][c] += psi[c] * coef[c] * weights[dj];
                                }
                            }
                        }
                    }
                }
            }

            let sigma_t: Vec<f64> = {
                let sr = self.xs.sigma_removal[g].internal.as_slice();
                (0..n).map(|c| sr[c] + sft0_gg[c]).collect()
            };
            let template = self.psi[g][0].clone();
            let sigma_t_field = field_like_zero_gradient(&template, sigma_t);

            let mut inner_iter = 0usize;
            loop {
                let mut inner_residual = 0.0_f64;
                for d in 0..n_dir {
                    // calcThisIntraSource.H
                    let mut intra = vec![0.0; n];
                    for c in 0..n {
                        intra[c] = self.flux[g][c]
                            * (((nu_sigma[c] * chi_p[c]) * one_minus_beta[c]) / keff + sft0_gg[c]);
                    }
                    if moments > 1 {
                        for dj in 0..n_dir {
                            let coef = self.moment_coefficient(dj, d, g, g);
                            let psi = self.psi[g][dj].internal.as_slice();
                            for c in 0..n {
                                intra[c] += psi[c] * weights[dj] * coef[c];
                            }
                        }
                    }
                    let rhs: Vec<f64> = (0..n)
                        .map(|c| (extra[d][c] + intra[c]) + delayed[c] * chi_d[c])
                        .collect();
                    let rhs_field = field_like_zero_gradient(&template, rhs);
                    let psi_field = &self.psi[g][d];
                    let eqn = fvm::div(&self.face_phi[d], psi_field)
                        + fvm::sp(&sigma_t_field, psi_field)
                        - fvm::su(&rhs_field, psi_field);
                    let b: Vec<f64> = eqn.source.iter().copied().collect();
                    let local_residual = eqn
                        .ldu
                        .openfoam_initial_residual(psi_field.internal.as_slice(), &b);
                    let (sol, _perf) = eqn.solve_bicgstab_with_guess(
                        format!("angularFlux_{g}_{d}"),
                        psi_field,
                        KrylovOptions::default(),
                        self.settings.linear,
                    );
                    self.psi[g][d].internal.as_mut_slice().copy_from_slice(sol.internal.as_slice());
                    inner_residual = inner_residual.max(local_residual);
                    neutro_residual = neutro_residual.max(local_residual);
                }
                // updateThisScalarFlux.H
                let mut f = vec![0.0; n];
                for d in 0..n_dir {
                    let psi = self.psi[g][d].internal.as_slice();
                    for c in 0..n {
                        f[c] += psi[c] * weights[d];
                    }
                }
                self.flux[g] = f;
                inner_iter += 1;
                if !((inner_residual > neutro_residual * self.settings.inn_to_out_tolerance)
                    && (inner_iter < self.settings.max_inner_iter))
                {
                    break;
                }
            }
        }

        self.update_scalar_fluxes();
        let p_tot = self.power();
        self.k_eff = calc_keff(keff, p_tot, p_target);
        self.scale_all(p_target / p_tot);

        PseudoTimeStep {
            power: p_tot,
            k_eff: self.k_eff,
            first_residual: neutro_residual,
        }
    }
}

/// A coefficient or source field with zero-gradient patches on the mesh of
/// `template` (see `field_like` in the pseudo-time module for why the flux's
/// own boundary must not be inherited).
fn field_like_zero_gradient(template: &VolScalarField, values: Vec<f64>) -> VolScalarField {
    let mut f = template.clone();
    f.internal = Field::new(values);
    for patch in f.boundary.iter_mut() {
        patch.bc = BoundaryCondition::ZeroGradient;
        patch.values = Field::uniform(patch.values.len(), 0.0);
    }
    f
}

impl PseudoTimeEigenvalue for SnPseudoTimeSolver {
    fn pseudo_time_power(&self) -> f64 {
        self.power()
    }

    fn pseudo_time_max_delta_t(
        &self,
        controls: &PseudoTimeControls,
        clock: &PseudoTimeClock,
        p_tot: f64,
        p_tot_old: f64,
    ) -> f64 {
        sp3_max_delta_t(controls, clock, p_tot, p_tot_old)
    }

    fn eigenvalue_time_step(
        &mut self,
        p_target: f64,
        first_step: bool,
    ) -> Result<PseudoTimeStep, PseudoTimeError> {
        Ok(self.eigenvalue_step(p_target, first_step))
    }
}
