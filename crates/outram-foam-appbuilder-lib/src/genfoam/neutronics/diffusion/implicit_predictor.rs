// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/implicitPredictor.H (included by solveNeutronics.H when
//                    `eigenvalueNeutronics false` and `neutronTransport.implicitPredictor true`, AFTER `integralPredictor.H`),
//                    with src/classes/neutronics/include/initializeNeutroSource.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! `implicitPredictor.H` of the diffusion neutronics (gh:#1926, README round 66).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Before the neutron iteration of a transient step, upstream solves, CELL BY CELL, the point (no-leakage-coupling) multigroup balance
//! with the leakage of the current star flux and the delayed-neutron source as explicit sources:
//!
//! `A_ij psi_j = b_i`, with
//! `A_ii = IV_i/dt/df_i - nuSigmaEff_i/keff (1 - BetaTot) chiPrompt_i/df_i + sigmaRemoval_i/df_i` and
//! `A_ij = - sigmaFromTo_ji/df_j - nuSigmaEff_j/keff (1 - BetaTot) chiPrompt_i/df_j` (`i != j`),
//! `b_i = IV_i fluxStar_i.oldTime()/dt/df_i + leakages_i + delayedNeutroSource chiDelayed_i`,
//!
//! `leakages_g = fvc::laplacian(D_g/discFactor_g, fluxStar_g)` (`Gauss linear corrected`, the star flux AFTER `integralPredictor.H`), and replaces
//! `fluxStar_` by the solution (`Foam::solve(List, SquareMatrix, List)`: Gaussian elimination with partial pivoting, the routine
//! [`crate::genfoam::thermal_hydraulics::structure::radial_solve::foam_solve`] already translates), then `flux_ = fluxStar_/discFactor` and
//! `initializeNeutroSource.H`. Upstream notes "(not tested for SP3)" and "can be unstable" (the real binary's power turns negative after
//! about 130 steps of the shipped transient: README round 66).
//!
//! `Dalbedo_ = D[g]` and `fluxStarAlbedo_ *= 0` (the albedo boundary condition's state) are not reproduced: a case with an albedo patch is
//! refused by name by the caller.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_foam_basic_lib::fv_operators::les_filter::surface_integrate;
use outram_foam_basic_lib::prelude::{fvc, NonOrthoScheme, VolScalarField};

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::NeutronicsError;
use crate::genfoam::thermal_hydraulics::structure::radial_solve::foam_solve;

impl DiffusionNeutronics {
    /// Switch `neutronTransport.implicitPredictor` on (`true`) or off for [`Self::upstream_stationary_step`].
    #[must_use]
    pub fn with_implicit_predictor(mut self, on: bool) -> Self {
        self.implicit_predictor = on;
        self
    }

    /// Whether the transient step runs `implicitPredictor.H`.
    #[must_use]
    pub fn implicit_predictor_enabled(&self) -> bool {
        self.implicit_predictor
    }

    /// `fvc::laplacian(D_g/discFactor_g, fluxStar_g)` per cell (`surfaceIntegrate` of the corrected face flux over `V`).
    fn leakage_field(&self, g: usize, star_field: &VolScalarField) -> Vec<f64> {
        let mesh = self.state.mesh().clone();
        let gamma = self.d_over_df_face(g, star_field);
        let sn = fvc::sn_grad_corrected(star_field, NonOrthoScheme::Corrected);
        let internal: Vec<f64> = (0..mesh.n_internal_faces).map(|f| (gamma.internal[f] * sn.internal[f]) * mesh.face_areas[f]).collect();
        let boundary: Vec<Vec<f64>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| (0..p.size).map(|fi| (gamma.boundary[pi].values[fi] * sn.boundary[pi].values[fi]) * mesh.face_areas[p.start + fi]).collect())
            .collect();
        surface_integrate(&mesh, &internal, &boundary)
    }

    /// `implicitPredictor.H`: `star` is `fluxStar_` (replaced in place), `star_fields` the same with boundary conditions (for the leakage),
    /// `flux_old` is `fluxStar_.oldTime()`, `delayed` is `delayedNeutroSource_`, `neutro_source` is re-initialised on return (`initializeNeutroSource.H`).
    ///
    /// # Errors
    /// [`NeutronicsError::UpstreamTransient`] when a cell's matrix is singular (upstream's `FatalError`) or an albedo patch is present.
    pub(super) fn run_implicit_predictor(
        &mut self,
        star: &mut [Vec<f64>],
        star_fields: &[VolScalarField],
        flux_old: &[Vec<f64>],
        neutro_source: &mut Vec<f64>,
        delayed: &[f64],
        dt: f64,
        keff: f64,
    ) -> Result<(), NeutronicsError> {
        if !self.albedo_specs.is_empty() {
            return Err(NeutronicsError::UpstreamTransient("implicitPredictor with an albedo patch is not ported (Dalbedo_/fluxStarAlbedo_)"));
        }
        let n = self.state.mesh().n_cells;
        let groups = self.energy_groups();
        let leakages: Vec<Vec<f64>> = (0..groups).map(|g| self.leakage_field(g, &star_fields[g])).collect();
        let beta_tot = self.xs.beta_tot.internal.as_slice().to_vec();
        for i in 0..n {
            let mut matrix = vec![0.0; groups * groups];
            let mut source = vec![0.0; groups];
            for e_i in 0..groups {
                let iv_i = self.xs.inv_velocity[e_i].internal.as_slice()[i];
                let df_i = self.xs.disc_factor[e_i].internal.as_slice()[i];
                let chi_p_i = self.xs.chi_prompt[e_i].internal.as_slice()[i];
                for e_j in 0..groups {
                    let df_j = self.xs.disc_factor[e_j].internal.as_slice()[i];
                    let nu_j = self.xs.nu_sigma_f[e_j].internal.as_slice()[i];
                    matrix[e_i * groups + e_j] = if e_j == e_i {
                        let removal_i = self.xs.sigma_removal[e_i].internal.as_slice()[i];
                        (((iv_i / dt) / df_i) - ((((nu_j / keff) * (1.0 - beta_tot[i])) * chi_p_i) / df_i)) + (removal_i / df_i)
                    } else {
                        let from_to = self.xs.scattering[e_j][e_i].internal.as_slice()[i];
                        (-from_to / df_j) - ((((nu_j / keff) * (1.0 - beta_tot[i])) * chi_p_i) / df_j)
                    };
                }
                let chi_d_i = self.xs.chi_delayed[e_i].internal.as_slice()[i];
                source[e_i] = ((((iv_i * flux_old[e_i][i]) / dt) / df_i) + leakages[e_i][i]) + (delayed[i] * chi_d_i);
            }
            let predicted = foam_solve(&matrix, groups, &source).map_err(|_| NeutronicsError::UpstreamTransient("implicitPredictor: a cell's matrix is singular"))?;
            for e in 0..groups {
                star[e][i] = predicted[e];
            }
        }
        self.write_flux_from_star_values(star);
        // initializeNeutroSource.H
        neutro_source.iter_mut().for_each(|x| *x = 0.0);
        self.accumulate_neutro_source(neutro_source);
        Ok(())
    }
}
