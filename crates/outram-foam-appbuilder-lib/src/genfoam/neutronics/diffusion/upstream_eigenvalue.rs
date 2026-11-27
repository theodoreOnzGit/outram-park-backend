// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {solveNeutronics,fluxEq,createFluxMatrices,normFluxes}.H and
//                    src/classes/neutronics/include/
//                    {precEq,calcKeff,initializeNeutroSource,
//                     initializeDelayedNeutroSource,calcScatteringSource}.H
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

//! GeN-Foam's `eigenvalueNeutronics true` time step for the diffusion model with circulating fuel
//! (gh:#1814; the `steadyStateEN` stage of `2D_MSFR`).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Translation of `solveNeutronics.H` with `eigenvalueNeutronics true`, `liquidFuel true`, no external
//! source, discontinuity factors 1. The stationary-fuel form of the same step is
//! [`crate::genfoam::neutronics::pseudo_time`] (`eigenvalue_time_step`); the precursors of a circulating
//! fuel are transported (`precEq.H`'s `liquidFuel` branch), so they are a linear system.
//!
//! Order, as upstream:
//!
//! 1. `xs_.correct()` at the mapped `TFuel`, `rhoCool`; `diffCoeffPrec = (alphat + mu/ScNo)/rhoCool`;
//! 2. on the first step of the run (`timeIndex - startTimeIndex == 1`): `normFluxes.H`,
//!    `fluxStar_ *= pTarget_/power()`;
//! 3. `initializeNeutroSource.H`, `precEq.H` (its `ddt` coefficient is `alpha (1 - 1) = 0`, so the
//!    equation is the steady transport one), `initializeDelayedNeutroSource.H`;
//! 4. ONE pass of `fluxEq.H` (`do {...} while (... && !eigenvalueNeutronics_)`): every group solved
//!    twice, `neutroSource_` left as step 3 made it (its `-=`/`+=` are multiplied by `1 - 1`), then
//!    `precEq.H` again (same source, new guess);
//! 5. `powerDensity_ = sum_g flux_g sigmaPow_g` and `pTot = power()` of the UN-normalised flux;
//! 6. `calcKeff.H`: `keff_ *= pTot/pTarget_` clamped to `[0.1, 5]`; `normFluxes.H`:
//!    `fluxStar_ *= pTarget_/pTot`. The precursors are not rescaled.
//!
//! Step 5 matters for coupling: the `powerDensity` the fluid receives next step is the one BEFORE the
//! renormalisation, so away from the eigenvalue the heat source is `pTot`, not `pTarget_`.
//!
//! Deliberate differences, as in [`super::upstream_transient`]: discontinuity factors 1, and basic-lib's
//! linear solvers where upstream uses `PCG`/`DIC` and `PBiCG`/`DILU`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::{fvm, Field};

use super::upstream_transient::{UpstreamTransientControls, UpstreamTransientFeedback};
use super::DiffusionNeutronics;
use crate::genfoam::neutronics::pseudo_time::{calc_keff, field_like, solve_twice};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::NeutronicsError;

/// What one eigenvalue time step reports (the lines the upstream log prints).
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamEigenvalueStep {
    /// `keff_ previous`.
    pub k_eff_previous: f64,
    /// `keff_` after `calcKeff.H`.
    pub k_eff: f64,
    /// `power:`, the integral of the flux BEFORE `normFluxes.H`.
    pub power: f64,
    /// `powerDensity_` of the step (before `normFluxes.H`): what the next step maps to the fluid.
    pub power_density: Vec<f64>,
    /// `neutroResidual`: the largest initial residual of the (first) flux solves.
    pub neutro_residual: f64,
    /// The first-solve initial residual of each group.
    pub group_residuals: Vec<f64>,
}

impl DiffusionNeutronics {
    /// One `solveNeutronics.H` time step with `eigenvalueNeutronics true` and circulating fuel. See the
    /// module documentation. `first_step` is `timeIndex - startTimeIndex == 1`.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::UpstreamTransient`] if the circulating-fuel transport or `precStar` is not set
    /// or a feedback array is not sized to the mesh; the cross-section error.
    pub fn upstream_eigenvalue_step(
        &mut self,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        feedback: UpstreamTransientFeedback<'_>,
        p_target: f64,
        first_step: bool,
        controls: &UpstreamTransientControls,
    ) -> Result<UpstreamEigenvalueStep, NeutronicsError> {
        if !self.xs.disc_factors_are_unity() {
            return Err(NeutronicsError::UpstreamTransient(
                "discontinuity factors other than 1: only the pseudo-time eigenvalue driver divides by them (gh:#1785)",
            ));
        }
        let n = self.state.mesh().n_cells;
        if self.drift.is_none() || self.prec_star.len() != self.prec_groups() {
            return Err(NeutronicsError::UpstreamTransient(
                "circulating-fuel transport and precStar must be set first",
            ));
        }
        if feedback.rho_cool.len() != n || feedback.alphat.len() != n || feedback.mu.len() != n {
            return Err(NeutronicsError::UpstreamTransient("a feedback array is not sized to the mesh"));
        }
        let groups = self.energy_groups();
        let volumes = self.state.mesh().cell_volumes.clone();
        let keff_previous = self.state.k_eff_raw();
        let mut keff = keff_previous;

        // xs_.correct()
        self.refresh_cross_sections(xs, zone_of_cell, feedback.cell_parameters)?;

        // liquidFuel: diffCoeffPrec = (alphat + mu/ScNo)/rhoCool
        {
            let diff: Vec<f64> = (0..n)
                .map(|c| (feedback.alphat[c] + feedback.mu[c] / controls.sc_no) / feedback.rho_cool[c])
                .collect();
            if let Some(d) = self.drift.as_mut() {
                d.diffusivity.internal = Field::new(diff);
            }
        }

        // normFluxes.H on the first step: fluxStar *= pTarget / pTot.
        if first_step {
            let p_tot = self.upstream_power();
            let ratio = p_target / p_tot;
            for field in self.state.flux_mut() {
                for v in field.internal.as_mut_slice() {
                    *v *= ratio;
                }
            }
        }

        let one_minus_beta: Vec<f64> =
            self.xs.beta_tot.internal.as_slice().iter().map(|b| 1.0 - b).collect();

        // initializeNeutroSource.H, precEq.H, initializeDelayedNeutroSource.H
        let mut neutro_source = vec![0.0; n];
        self.accumulate_neutro_source(&mut neutro_source);
        self.upstream_prec_eq(&neutro_source, keff, &[], &volumes, 1.0, false)?;
        let mut delayed = vec![0.0; n];
        self.accumulate_delayed_source(&mut delayed);

        // fluxEq.H, once
        let mut neutro_residual = 0.0_f64;
        let mut group_residuals = Vec::with_capacity(groups);
        for gg in 0..groups {
            // calcScatteringSource.H
            let mut scatter = vec![0.0; n];
            for j in 0..groups {
                if j != gg {
                    let sig = self.xs.scattering[j][gg].internal.as_slice();
                    let ph = self.state.flux()[j].internal.as_slice();
                    for c in 0..n {
                        scatter[c] += sig[c] * ph[c];
                    }
                }
            }
            let chi_p = self.xs.chi_prompt[gg].internal.as_slice();
            let chi_d = self.xs.chi_delayed[gg].internal.as_slice();
            let prompt: Vec<f64> =
                (0..n).map(|c| ((neutro_source[c] * one_minus_beta[c]) * chi_p[c]) / keff).collect();
            let delayed_g: Vec<f64> = (0..n).map(|c| delayed[c] * chi_d[c]).collect();

            let flux_g = &self.state.flux()[gg];
            let eqn = fvm::laplacian_with_delta(&self.xs.d_face[gg], flux_g, self.settings.delta_coeff)
                + fvm::sp(&self.xs.sigma_removal[gg], flux_g)
                - fvm::su(&field_like(flux_g, prompt), flux_g)
                - fvm::su(&field_like(flux_g, delayed_g), flux_g)
                - fvm::su(&field_like(flux_g, scatter), flux_g);
            let (sol, init_res) = solve_twice(&eqn, &format!("fluxStar{gg}"), flux_g, self.settings.linear);
            neutro_residual = neutro_residual.max(init_res);
            group_residuals.push(init_res);
            self.write_group_flux(gg, &sol);
        }

        // precEq.H again (same `neutroSource_`, a new guess)
        self.upstream_prec_eq(&neutro_source, keff, &[], &volumes, 1.0, false)?;

        // powerDensity_ and pTot = power(), before normFluxes.H
        let power_density = self.upstream_power_density();
        let p_tot = self.upstream_power();

        // calcKeff.H, normFluxes.H
        keff = calc_keff(keff, p_tot, p_target);
        let ratio = p_target / p_tot;
        for field in self.state.flux_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= ratio;
            }
        }
        self.state.set_k_eff_raw(keff);
        self.update_derived_fields();

        Ok(UpstreamEigenvalueStep {
            k_eff_previous: keff_previous,
            k_eff: keff,
            power: p_tot,
            power_density,
            neutro_residual,
            group_residuals,
        })
    }
}
