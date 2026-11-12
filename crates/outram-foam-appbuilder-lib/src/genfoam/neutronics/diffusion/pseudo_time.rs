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

use outram_foam_basic_lib::prelude::fvm;

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::pseudo_time::{
    calc_keff, field_like, neutronics_max_delta_t, solve_twice, PseudoTimeClock,
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

    fn eigenvalue_time_step(
        &mut self,
        p_target: f64,
        first_step: bool,
    ) -> Result<PseudoTimeStep, PseudoTimeError> {
        if self.drift.is_some() {
            return Err(PseudoTimeError::LiquidFuel);
        }
        let groups = self.energy_groups();
        let n = self.state.mesh().n_cells;
        let vols = self.state.mesh().cell_volumes.clone();
        let mut keff = self.state.k_eff_raw();

        // `normFluxes.H` on the first step: fluxStar *= pTarget / pTot.
        if first_step {
            let p_tot = self.upstream_power();
            let ratio = p_target / p_tot;
            for field in self.state.flux_mut() {
                for v in field.internal.as_mut_slice() {
                    *v *= ratio;
                }
            }
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

            let flux_g = &self.state.flux()[gg];
            let template = flux_g.clone();
            let eqn = fvm::laplacian_with_delta(&self.xs.d_face[gg], flux_g, self.settings.delta_coeff)
                + fvm::sp(&self.xs.sigma_removal[gg], flux_g)
                - fvm::su(&field_like(&template, prompt), flux_g)
                - fvm::su(&field_like(&template, delayed_g), flux_g)
                - fvm::su(&field_like(&template, scatter), flux_g);
            let (sol, init_res) = solve_twice(&eqn, &format!("flux{gg}"), flux_g, self.settings.linear);
            neutro_residual = neutro_residual.max(init_res);
            self.write_group_flux(gg, &sol);
        }

        let p_tot = self.upstream_power();
        keff = calc_keff(keff, p_tot, p_target);

        // normFluxes.H
        let ratio = p_target / p_tot;
        for field in self.state.flux_mut() {
            for v in field.internal.as_mut_slice() {
                *v *= ratio;
            }
        }
        self.state.set_k_eff_raw(keff);
        self.update_derived_fields();

        Ok(PseudoTimeStep {
            power: p_tot,
            k_eff: keff,
            first_residual: neutro_residual,
        })
    }
}
