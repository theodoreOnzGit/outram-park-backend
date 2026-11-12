// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/SP3/include/
//                    {solveNeutronicsSP3,createFluxMatricesSP3,fluxEqSP3,
//                     normFluxesSP3}.H and src/classes/neutronics/include/
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

//! GeN-Foam's eigenvalue time step for the SP3 model (gh:#1361, S4).
//!
//! Translation of `solveNeutronicsSP3.H` (eigenvalue branch), `fluxEqSP3.H`
//! and `normFluxesSP3.H`; the shared pieces are as in the diffusion model's
//! `pseudo_time` module. The two moment equations are assembled term by term
//! in upstream's order, and the second-moment coefficients use upstream's
//! expressions (`discFactor/(sigmaRemoval+sigmaFromTo)*3.0/7.0` and
//! `(sr+ss)*5.0/3.0+(sr*4.0/3.0)`, not the closed forms of
//! [`super::Sp3XsFields`]). Discontinuity factors are 1.

use outram_foam_basic_lib::prelude::{fvc, fvm};

use super::{Sp3Neutronics, Sp3XsFields};
use crate::genfoam::neutronics::pseudo_time::{
    calc_keff, field_like, solve_twice, PseudoTimeEigenvalue, PseudoTimeError, PseudoTimeStep,
};

impl Sp3Neutronics {
    /// Set the second moment of every group to the uniform `value` and rebuild
    /// the physical flux, as GeN-Foam does when it reads `defaultFlux2`
    /// (`createNeutronicsFieldsSP3.H`). The shipped slab tutorials start from
    /// `defaultFlux2 uniform 1`; this crate's constructors start from 0.
    pub fn set_uniform_second_moment(&mut self, value: f64) {
        for field in self.flux_star2.iter_mut() {
            for v in field.internal.as_mut_slice() {
                *v = value;
            }
        }
        self.reconstruct_flux();
    }

    /// `power()`: `sum_g domainIntegrate(flux_g sigmaPow_g)`, `None` without
    /// cross sections.
    fn upstream_power(&self) -> Option<f64> {
        let xs = self.xs.as_ref()?;
        Some(self.upstream_power_with(xs))
    }

    fn upstream_power_with(&self, xs: &Sp3XsFields) -> f64 {
        let vols = &self.state.mesh().cell_volumes;
        let mut p_tot = 0.0;
        for (g, flux) in self.state.flux().iter().enumerate() {
            let sp = xs.base.sigma_pow[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            let mut integral = 0.0;
            for c in 0..vols.len() {
                integral += vols[c] * (ph[c] * sp[c]);
            }
            p_tot += integral;
        }
        p_tot
    }

    /// `flux_ = (fluxStar_ - fluxStar2_*2.0)/discFactor` with `discFactor = 1`.
    fn rebuild_flux_upstream(&mut self) {
        let n = self.state.mesh().n_cells;
        for g in 0..self.energy_groups() {
            let star = self.flux_star[g].internal.as_slice().to_vec();
            let star2 = self.flux_star2[g].internal.as_slice().to_vec();
            let dst = self.state.flux_mut()[g].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = (star[c] - star2[c] * 2.0) / 1.0;
            }
        }
    }

    fn scale_moments_by(&mut self, ratio: f64) {
        for f in self.flux_star.iter_mut().chain(self.flux_star2.iter_mut()) {
            for v in f.internal.as_mut_slice() {
                *v *= ratio;
            }
        }
    }
}

impl PseudoTimeEigenvalue for Sp3Neutronics {
    fn pseudo_time_power(&self) -> f64 {
        self.upstream_power().unwrap_or(0.0)
    }

    fn eigenvalue_time_step(
        &mut self,
        p_target: f64,
        first_step: bool,
    ) -> Result<PseudoTimeStep, PseudoTimeError> {
        let xs = self.xs.take().ok_or(PseudoTimeError::NoCrossSections)?;
        let step = self.eigenvalue_time_step_inner(&xs, p_target, first_step);
        self.xs = Some(xs);
        Ok(step)
    }
}

impl Sp3Neutronics {
    fn eigenvalue_time_step_inner(
        &mut self,
        xs: &Sp3XsFields,
        p_target: f64,
        first_step: bool,
    ) -> PseudoTimeStep {
        let groups = self.energy_groups();
        let n = self.state.mesh().n_cells;
        let vols = self.state.mesh().cell_volumes.clone();
        let mut keff = self.state.k_eff_raw();

        // solveNeutronicsSP3.H: flux_ = (fluxStar - 2 fluxStar2)/disc, then on
        // the first step normFluxesSP3.H.
        self.rebuild_flux_upstream();
        if first_step {
            let p_tot = self.upstream_power_with(xs);
            let ratio = p_target / p_tot;
            self.scale_moments_by(ratio);
            self.rebuild_flux_upstream();
        }

        let mut neutro_source = vec![0.0; n];
        for g in 0..groups {
            let nf = xs.base.nu_sigma_f[g].internal.as_slice();
            let ph = self.state.flux()[g].internal.as_slice();
            for c in 0..n {
                neutro_source[c] += nf[c] * ph[c];
            }
        }
        for kk in 0..self.prec_groups() {
            let beta = xs.base.beta[kk].internal.as_slice();
            let lambda = xs.base.lambda[kk].internal.as_slice();
            let dst = self.state.precursors_mut()[kk].internal.as_mut_slice();
            for c in 0..n {
                let x = (neutro_source[c] / keff) * beta[c];
                dst[c] = (vols[c] * x) / (vols[c] * lambda[c]);
            }
        }
        let mut delayed = vec![0.0; n];
        for kk in 0..self.prec_groups() {
            let lambda = xs.base.lambda[kk].internal.as_slice();
            let prec = self.state.precursors()[kk].internal.as_slice();
            for c in 0..n {
                delayed[c] += lambda[c] * prec[c];
            }
        }
        let one_minus_beta: Vec<f64> = xs
            .base
            .beta_tot
            .internal
            .as_slice()
            .iter()
            .map(|b| 1.0 - b)
            .collect();

        let mut neutro_residual = 0.0_f64;
        for gg in 0..groups {
            let mut scatter = vec![0.0; n];
            for j in 0..groups {
                if j != gg {
                    let sig = xs.base.scattering[j][gg].internal.as_slice();
                    let ph = self.state.flux()[j].internal.as_slice();
                    for c in 0..n {
                        scatter[c] += sig[c] * ph[c];
                    }
                }
            }
            let sr = xs.base.sigma_removal[gg].internal.as_slice();
            let ss = xs.base.scattering[gg][gg].internal.as_slice();
            let chi_p = xs.base.chi_prompt[gg].internal.as_slice();
            let chi_d = xs.base.chi_delayed[gg].internal.as_slice();
            let template = self.flux_star[gg].clone();

            let prompt: Vec<f64> = (0..n)
                .map(|c| ((neutro_source[c] * one_minus_beta[c]) * chi_p[c]) / keff)
                .collect();
            let delayed_g: Vec<f64> = (0..n).map(|c| delayed[c] * chi_d[c]).collect();
            let star2_old = self.flux_star2[gg].internal.as_slice().to_vec();
            let sr_star2: Vec<f64> = (0..n)
                .map(|c| (sr[c] * (star2_old[c] / 1.0)) * 2.0)
                .collect();

            // First moment: neutroEqList[g] - prompt - delayed - scatter
            //   - sr*(fluxStar2/disc)*2.0 (- fvc::ddt(0)*2.0, identically zero).
            let phi0 = &self.flux_star[gg];
            let eqn1 =
                fvm::laplacian_with_delta(&xs.base.d_face[gg], phi0, self.settings.delta_coeff)
                    + fvm::sp(&xs.base.sigma_removal[gg], phi0)
                    - fvm::su(&field_like(&template, prompt.clone()), phi0)
                    - fvm::su(&field_like(&template, delayed_g.clone()), phi0)
                    - fvm::su(&field_like(&template, scatter.clone()), phi0)
                    - fvm::su(&field_like(&template, sr_star2), phi0);
            let (sol1, res1) =
                solve_twice(&eqn1, &format!("fluxStar{gg}"), phi0, self.settings.linear);
            neutro_residual = neutro_residual.max(res1);
            Self::write_moment(&mut self.flux_star[gg], &sol1);

            // Second moment (fluxEqSP3.H neutroEq2).
            let d2: Vec<f64> = (0..n)
                .map(|c| ((1.0 / (sr[c] + ss[c])) * 3.0) / 7.0)
                .collect();
            let a2: Vec<f64> = (0..n)
                .map(|c| ((sr[c] + ss[c]) * 5.0) / 3.0 + (sr[c] * 4.0) / 3.0)
                .collect();
            let d2_face = fvc::interpolate(&field_like(&template, d2));
            let star_new = self.flux_star[gg].internal.as_slice().to_vec();
            let x1: Vec<f64> = (0..n)
                .map(|c| ((sr[c] * (star_new[c] / 1.0)) * 2.0) / 3.0)
                .collect();
            let x2: Vec<f64> = (0..n).map(|c| (prompt[c] * 2.0) / 3.0).collect();
            let x3: Vec<f64> = (0..n).map(|c| (delayed_g[c] * 2.0) / 3.0).collect();
            let x4: Vec<f64> = (0..n).map(|c| (scatter[c] * 2.0) / 3.0).collect();
            let phi2 = &self.flux_star2[gg];
            let eqn2 = fvm::laplacian_with_delta(&d2_face, phi2, self.settings.delta_coeff)
                + fvm::sp(&field_like(&template, a2), phi2)
                - fvm::su(&field_like(&template, x1), phi2)
                + fvm::su(&field_like(&template, x2), phi2)
                + fvm::su(&field_like(&template, x3), phi2)
                + fvm::su(&field_like(&template, x4), phi2);
            let (sol2, res2) =
                solve_twice(&eqn2, &format!("fluxStar2{gg}"), phi2, self.settings.linear);
            neutro_residual = neutro_residual.max(res2);
            Self::write_moment(&mut self.flux_star2[gg], &sol2);

            // flux_[g] = (fluxStar - 2 fluxStar2)/disc
            let star = self.flux_star[gg].internal.as_slice().to_vec();
            let star2 = self.flux_star2[gg].internal.as_slice().to_vec();
            let dst = self.state.flux_mut()[gg].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = (star[c] - star2[c] * 2.0) / 1.0;
            }
        }

        let p_tot = self.upstream_power_with(xs);
        keff = calc_keff(keff, p_tot, p_target);
        let ratio = p_target / p_tot;
        self.scale_moments_by(ratio);
        self.rebuild_flux_upstream();
        self.state.set_k_eff_raw(keff);
        self.update_derived_fields(xs);

        PseudoTimeStep {
            power: p_tot,
            k_eff: keff,
            first_residual: neutro_residual,
        }
    }
}
