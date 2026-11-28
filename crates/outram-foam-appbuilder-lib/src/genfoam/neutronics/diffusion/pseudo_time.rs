// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {solveNeutronics,createFluxMatrices,fluxEq,normFluxes}.H
//                    and src/classes/neutronics/include/
//                    {calcKeff,precEq,initializeNeutroSource,
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

//! GeN-Foam's eigenvalue time step for the diffusion model (gh:#1361, S4).
//!
//! Translation of `solveNeutronics.H` (`eigenvalueNeutronics true` branch),
//! `fluxEq.H`, `createFluxMatrices.H`, `precEq.H`, `initializeNeutroSource.H`,
//! `initializeDelayedNeutroSource.H`, `calcKeff.H`, `normFluxes.H`. See
//! [`crate::genfoam::neutronics::pseudo_time`] for the loop.
//!
//! Operation order follows upstream where it is cheap to: the precursor
//! "solve" of the diagonal matrix is `(V S/k beta) / (V lambda)` (OpenFOAM's
//! `diagonalSolver`), the source terms are added one by one in upstream's
//! order, `power()` is `V (flux sigmaPow)` summed per group, and every
//! equation is solved twice. The linear solver is not upstream's (basic-lib's
//! PCG/DIC, stopping on `||r||/||b||`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::{fvc, fvm};

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::pseudo_time::{
    calc_keff, field_like, neutronics_max_delta_t, solve_twice_logged, PseudoTimeClock,
    PseudoTimeControls, PseudoTimeEigenvalue, PseudoTimeError, PseudoTimeStep,
};

impl DiffusionNeutronics {
    /// `power()`: `sum_g domainIntegrate(flux_g sigmaPow_g)`, each integral
    /// `sum_c V (flux sigmaPow)`.
    pub(crate) fn upstream_power(&self) -> f64 {
        let vols = &self.state.mesh().cell_volumes;
        let mut p_tot = 0.0;
        for (g, flux) in self.state.flux().iter().enumerate() {
            let sp = self.xs.sigma_pow[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            let mut integral = 0.0;
            for c in 0..vols.len() {
                integral += vols[c] * (ph[c] * sp[c]);
            }
            p_tot += integral;
        }
        p_tot
    }
}

impl PseudoTimeEigenvalue for DiffusionNeutronics {
    fn pseudo_time_power(&self) -> f64 {
        self.upstream_power()
    }

    fn pseudo_time_max_delta_t(
        &self,
        controls: &PseudoTimeControls,
        clock: &PseudoTimeClock,
        p_tot: f64,
        p_tot_old: f64,
    ) -> f64 {
        neutronics_max_delta_t(controls, clock, p_tot, p_tot_old)
    }

    fn set_time(&mut self, time: f64) {
        self.time = time;
    }

    fn eigenvalue_time_step(
        &mut self,
        p_target: f64,
        first_step: bool,
    ) -> Result<PseudoTimeStep, PseudoTimeError> {
        if self.drift.is_some() {
            return Err(PseudoTimeError::LiquidFuel);
        }
        // `xs_.correct()` (`solveNeutronics.H:4`): driveline zones and the discontinuity-factor lists at
        // the time of this step; after `pTotOld_` was taken by the loop, as upstream.
        if let Some(evolution) = &self.evolution {
            let mesh = self.state.mesh().clone();
            self.xs = evolution
                .materialize(&mesh, self.time)
                .map_err(|_| PseudoTimeError::CrossSections)?;
            for (patch, gamma, lin) in self.albedo_specs.clone() {
                self.set_albedo_boundary(patch, gamma, lin);
            }
        }
        let groups = self.energy_groups();
        let n = self.state.mesh().n_cells;
        let vols = self.state.mesh().cell_volumes.clone();
        let mut keff = self.state.k_eff_raw();

        let disc: Vec<Vec<f64>> =
            (0..groups).map(|g| self.xs.disc_factor[g].internal.as_slice().to_vec()).collect();
        let mut flux_star: Vec<Vec<f64>> = match self.flux_star.take() {
            Some(v) => v,
            None => self.state.flux().iter().map(|f| f.internal.as_slice().to_vec()).collect(),
        };
        // `flux_ = fluxStar_/discFactor` (`solveNeutronics.H:28-31`).
        self.write_flux_from_star(&flux_star, &disc);

        // `normFluxes.H` on the first step: fluxStar *= pTarget / pTot; flux = fluxStar/discFactor.
        if first_step {
            let p_tot = self.upstream_power();
            let ratio = p_target / p_tot;
            for star in flux_star.iter_mut() {
                for v in star.iter_mut() {
                    *v *= ratio;
                }
            }
            self.write_flux_from_star(&flux_star, &disc);
        }

        // initializeNeutroSource.H
        let mut neutro_source = vec![0.0; n];
        for g in 0..groups {
            let nf = self.xs.nu_sigma_f[g].internal.as_slice();
            let ph = self.state.flux()[g].internal.as_slice();
            for c in 0..n {
                neutro_source[c] += nf[c] * ph[c];
            }
        }

        // precEq.H, stationary fuel: diagonal matrix, psi = source / diag with
        // source = V (S/k beta) and diag = V lambda.
        for kk in 0..self.prec_groups() {
            let beta = self.xs.beta[kk].internal.as_slice();
            let lambda = self.xs.lambda[kk].internal.as_slice();
            let dst = self.state.precursors_mut()[kk].internal.as_mut_slice();
            for c in 0..n {
                let x = (neutro_source[c] / keff) * beta[c];
                dst[c] = (vols[c] * x) / (vols[c] * lambda[c]);
            }
        }
        // initializeDelayedNeutroSource.H
        let mut delayed = vec![0.0; n];
        for kk in 0..self.prec_groups() {
            let lambda = self.xs.lambda[kk].internal.as_slice();
            let prec = self.state.precursors()[kk].internal.as_slice();
            for c in 0..n {
                delayed[c] += lambda[c] * prec[c];
            }
        }

        let one_minus_beta: Vec<f64> = self
            .xs
            .beta_tot
            .internal
            .as_slice()
            .iter()
            .map(|b| 1.0 - b)
            .collect();

        let mut neutro_residual = 0.0_f64;
        // gh:#1966, README round 86: the flux solves of this step, as upstream's log prints them
        self.flux_solve_log.clear();
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
            let prompt: Vec<f64> = (0..n)
                .map(|c| ((neutro_source[c] * one_minus_beta[c]) * chi_p[c]) / keff)
                .collect();
            let delayed_g: Vec<f64> = (0..n).map(|c| delayed[c] * chi_d[c]).collect();

            // `fvm::laplacian(D/discFactor, fluxStar)` and `fvm::Sp(sigmaRemoval/discFactor, fluxStar)`
            // (`createFluxMatrices.H`); the unknown is fluxStar, the sources are in `flux`.
            let mut star_g = self.state.flux()[gg].clone();
            star_g.internal = outram_foam_basic_lib::prelude::Field::new(flux_star[gg].clone());
            let template = star_g.clone();
            let d_over_df: Vec<f64> = (0..n).map(|c| self.xs.d[gg].internal.as_slice()[c] / disc[gg][c]).collect();
            let sr_over_df: Vec<f64> =
                (0..n).map(|c| self.xs.sigma_removal[gg].internal.as_slice()[c] / disc[gg][c]).collect();
            let d_face = fvc::interpolate(&field_like(&template, d_over_df));
            // `laplacianSchemes Gauss linear corrected` (gh:#1856): the explicit non-orthogonal correction is taken from the
            // `Gauss linear` gradient of the flux the step starts from (`createFluxMatrices.H` is evaluated before `fluxEq.H`)
            let lap = match self.eigenvalue_laplacian {
                super::external_source::NeutronicsLaplacianScheme::Corrected => {
                    let grad = fvc::grad(&star_g);
                    fvm::laplacian_corrected(&d_face, &star_g, &grad, outram_foam_basic_lib::prelude::NonOrthoScheme::Corrected)
                }
                super::external_source::NeutronicsLaplacianScheme::Uncorrected => {
                    fvm::laplacian_with_delta(&d_face, &star_g, self.settings.delta_coeff)
                }
            };
            let eqn = lap
                + fvm::sp(&field_like(&template, sr_over_df), &star_g)
                - fvm::su(&field_like(&template, prompt), &star_g)
                - fvm::su(&field_like(&template, delayed_g), &star_g)
                - fvm::su(&field_like(&template, scatter), &star_g);
            let (sol, init_res) = solve_twice_logged(&eqn, &format!("flux{gg}"), &star_g, self.settings.linear_solve(), &mut self.flux_solve_log);
            neutro_residual = neutro_residual.max(init_res);
            flux_star[gg] = sol.internal.as_slice().to_vec();
            // flux_ = fluxStar_/discFactor (`fluxEq.H:31`), before the next group's scattering source.
            self.write_flux_from_star_group(gg, &flux_star[gg], &disc[gg]);
        }

        let p_tot = self.upstream_power();
        keff = calc_keff(keff, p_tot, p_target);

        // normFluxes.H
        let ratio = p_target / p_tot;
        for star in flux_star.iter_mut() {
            for v in star.iter_mut() {
                *v *= ratio;
            }
        }
        self.write_flux_from_star(&flux_star, &disc);
        self.state.set_k_eff_raw(keff);
        self.update_derived_fields();

        // `xs_.adjustDiscFactors(fluxStar_)` (`solveNeutronics.H:128`): takes effect at the next step.
        if let Some(evolution) = self.evolution.as_mut() {
            evolution.adjust_disc_factors(&flux_star, &vols);
        }
        self.flux_star = Some(flux_star);

        Ok(PseudoTimeStep {
            power: p_tot,
            k_eff: keff,
            first_residual: neutro_residual,
        })
    }
}

impl DiffusionNeutronics {
    /// `flux_[g] = fluxStar_[g]/discFactor[g]` for one group (internal values).
    fn write_flux_from_star_group(&mut self, g: usize, star: &[f64], disc: &[f64]) {
        let dst = self.state.flux_mut()[g].internal.as_mut_slice();
        for c in 0..dst.len() {
            dst[c] = star[c] / disc[c];
        }
    }

    /// `flux_ = fluxStar_/discFactor` for every group.
    fn write_flux_from_star(&mut self, star: &[Vec<f64>], disc: &[Vec<f64>]) {
        for g in 0..star.len() {
            self.write_flux_from_star_group(g, &star[g], &disc[g]);
        }
    }
}
