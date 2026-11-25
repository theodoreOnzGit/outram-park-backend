// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {solveNeutronics,fluxEq,createFluxMatrices}.H and
//                    src/classes/neutronics/include/
//                    {precEq,initializeNeutroSource,
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

//! GeN-Foam's transient time step for the diffusion model with circulating
//! fuel (gh:#1361, round 6).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! Translation of `solveNeutronics.H` with `eigenvalueNeutronics false`,
//! `liquidFuel true`, no external source, no `integralPredictor` /
//! `implicitPredictor`, as the `2D_MSFR` `transient` stage runs it:
//!
//! 1. `xs_.correct()` at the mapped `TFuel`, `rhoCool`;
//! 2. `phi = fvc::flux(U alpha)` (held fixed by the caller: the stage solves no
//!    fluid mechanics) and `diffCoeffPrec = (alphat + mu/ScNo)/rhoCool`;
//! 3. `initializeNeutroSource.H`, `precEq.H`, `initializeDelayedNeutroSource.H`;
//! 4. the neutron iteration `do { fluxEq.H; precEq.H; delayed source }
//!    while (neutroResidual > neutronIterationResidual && iter < max)`, every
//!    flux equation `ddt(IV, fluxStar) - laplacian(D, fluxStar) - Sp(...)`
//!    with the own-group prompt fission implicit and the other groups, the
//!    delayed neutrons and the in-scatter explicit (Gauss-Seidel over groups,
//!    `neutroSource_` updated by `-=` / `+=` exactly as upstream does), every
//!    equation solved twice;
//! 5. `powerDensity_`, `power()`.
//!
//! `keff_` is not updated (`calcKeff.H` is inside `if (eigenvalueNeutronics_)`).
//!
//! # Deliberate differences from the text of upstream
//!
//! * **Aitken acceleration is not performed.** `fluxEq.H` declares three
//!   `List<scalar>` of integral fluxes *inside* the iteration, uninitialised,
//!   and tests them at the third iteration. In the run recorded in
//!   `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`
//!   ("Round 6") the test failed ("Aitken not performed because of diverging
//!   series") at all 157 attempts and never passed, so the observable
//!   behaviour of the binary is "no acceleration"; translating the text
//!   literally would need the contents of uninitialised memory. Whether to
//!   port the evident intent (lists that persist across iterations) is
//!   deferred to the maintainer.
//! * Discontinuity factors are 1 (`adjustDiscFactors false` in the MSFR case):
//!   `fluxStar_ = flux_`.
//! * The linear solvers are basic-lib's (PCG for the flux, BiCGStab for the
//!   precursors) where upstream uses `PCG`/`DIC` and `PBiCG`/`DILU`; at the
//!   `1e-14` protocol they agree to round-off, not bit for bit.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::ldu_matrix::solvers::krylov_solve::KrylovOptions;
use outram_foam_basic_lib::prelude::{fvm, Field, FvMatrix, VolScalarField};

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::pseudo_time::{field_like, solve_twice};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::neutronics::NeutronicsError;

use super::fields::DiffusionXsFields;

/// The `neutronTransport` entries of `system/<neutro>/fvSolution` and the
/// Schmidt number of `nuclearData` that the step reads.
///
/// [`Default`] is upstream's own default for each (`neutronIterationResidual
/// 1e-5`, `maxNeutronIterations 10`, `ScNo 1`); the `2D_MSFR` case sets `1e-6`
/// and `50`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UpstreamTransientControls {
    /// `neutronIterationResidual`.
    pub neutron_iteration_residual: f64,
    /// `maxNeutronIterations`.
    pub max_neutron_iterations: usize,
    /// `ScNo` of `nuclearData` (Schmidt number of the precursor diffusion).
    pub sc_no: f64,
}

impl Default for UpstreamTransientControls {
    fn default() -> Self {
        Self {
            neutron_iteration_residual: 1.0e-5,
            max_neutron_iterations: 10,
            sc_no: 1.0,
        }
    }
}

/// What the thermal-hydraulic region hands the neutronics at the start of a
/// step (the neutronics-side fields of `multiRegionCouplingDict`'s mapping),
/// one value per cell.
#[derive(Debug, Clone, Copy)]
pub struct UpstreamTransientFeedback<'a> {
    /// The cross-section parameters of each cell, `n_cells x n_variables`
    /// row-major, in `xsVariables` order (`TFuel`, `rhoCool` for the MSFR).
    pub cell_parameters: &'a [f64],
    /// `rhoCool`, for `diffCoeffPrec`.
    pub rho_cool: &'a [f64],
    /// `alphat` of the neutronics mesh (the mapped `thermo:alpha`).
    pub alphat: &'a [f64],
    /// `mu` of the neutronics mesh.
    pub mu: &'a [f64],
}

/// What one transient step reports (the lines the upstream log prints).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UpstreamTransientStep {
    /// `power()` before the step (`pTotOld_`).
    pub power_old: f64,
    /// `power()` after the step (`power:`).
    pub power: f64,
    /// `keff_`, unchanged by a transient step.
    pub k_eff: f64,
    /// The neutron iterations performed (`Final neutroResidual at neutroIter no`).
    pub neutron_iterations: usize,
    /// The final `neutroResidual`.
    pub neutro_residual: f64,
}

/// `fvm::ddt(coeff, psi)` of `EulerDdtScheme::fvmDdt(vsf, vf)`, operation
/// order as upstream: `diag = rDeltaT*vsf*V`, `source = rDeltaT*vsf_0*vf_0*V`.
fn euler_ddt(coeff: &[f64], psi_old: &[f64], volumes: &[f64], dt: f64, like: &VolScalarField) -> FvMatrix {
    let r_delta_t = 1.0 / dt;
    let mut m = FvMatrix::new(like.mesh.clone());
    for c in 0..coeff.len() {
        m.ldu.diag[c] += r_delta_t * coeff[c] * volumes[c];
        m.source[c] += r_delta_t * coeff[c] * psi_old[c] * volumes[c];
    }
    m
}

impl DiffusionNeutronics {
    /// Set `precStar` (precursors per unit fluid volume), one list per group,
    /// one value per cell; the state's `prec` is `precStar * alpha`.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::UpstreamTransient`] if the lists are not sized
    /// `prec_groups x n_cells`, or [`Self::set_precursor_drift`] was not called.
    pub fn set_prec_star(&mut self, prec_star: &[Vec<f64>]) -> Result<(), NeutronicsError> {
        let n = self.state.mesh().n_cells;
        if prec_star.len() != self.prec_groups() || prec_star.iter().any(|p| p.len() != n) {
            return Err(NeutronicsError::UpstreamTransient("precStar is not sized groups x cells"));
        }
        let Some(drift) = self.drift.as_ref() else {
            return Err(NeutronicsError::UpstreamTransient("set_precursor_drift was not called"));
        };
        let alpha = drift.alpha.internal.as_slice().to_vec();
        self.prec_star = prec_star
            .iter()
            .map(|p| field_like(&drift.alpha, p.clone()))
            .collect();
        for (k, p) in prec_star.iter().enumerate() {
            let dst = self.state.precursors_mut()[k].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = p[c] * alpha[c];
            }
        }
        Ok(())
    }

    /// `precStar` per group, per cell (empty before [`Self::set_prec_star`]).
    #[must_use]
    pub fn prec_star_values(&self) -> Vec<Vec<f64>> {
        self.prec_star.iter().map(|f| f.internal.as_slice().to_vec()).collect()
    }

    /// Replace the cross sections by those at new per-cell feedback parameters
    /// (`xs_.correct()`), and rebuild every albedo patch at the new `D`
    /// (upstream sets `Dalbedo_ = D[energyI]` before each group's solve).
    ///
    /// # Errors
    ///
    /// The cross-section evaluation's error.
    pub fn refresh_cross_sections(
        &mut self,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        cell_parameters: &[f64],
    ) -> Result<(), NeutronicsError> {
        let mesh = self.state.mesh().clone();
        self.xs = DiffusionXsFields::materialize_per_cell(&mesh, xs, zone_of_cell, cell_parameters)?;
        for (patch, gamma, lin) in self.albedo_specs.clone() {
            self.set_albedo_boundary(patch, gamma, lin);
        }
        Ok(())
    }

    /// One `solveNeutronics.H` time step of the `liquidFuel true`, transient
    /// diffusion model, `dt` seconds long. See the module documentation.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::NonPositiveTimeStep`]; [`NeutronicsError::UpstreamTransient`]
    /// if the circulating-fuel transport or `precStar` is not set or a feedback
    /// array is not sized to the mesh; the cross-section error.
    pub fn upstream_transient_step(
        &mut self,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        feedback: UpstreamTransientFeedback<'_>,
        dt: f64,
        controls: &UpstreamTransientControls,
    ) -> Result<UpstreamTransientStep, NeutronicsError> {
        if dt <= 0.0 || dt.is_nan() {
            return Err(NeutronicsError::NonPositiveTimeStep(dt));
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
        let keff = self.state.k_eff_raw();
        let power_old = self.upstream_power();

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

        // Old-time values of this step (`oldTime()` is stored at the first
        // modification in the time step).
        let flux_old: Vec<Vec<f64>> =
            self.state.flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
        let prec_star_old: Vec<Vec<f64>> = self.prec_star_values();

        let one_minus_beta: Vec<f64> =
            self.xs.beta_tot.internal.as_slice().iter().map(|b| 1.0 - b).collect();

        // initializeNeutroSource.H
        let mut neutro_source = vec![0.0; n];
        self.accumulate_neutro_source(&mut neutro_source);
        // precEq.H, then initializeDelayedNeutroSource.H
        self.upstream_prec_eq(&neutro_source, keff, &prec_star_old, &volumes, dt)?;
        let mut delayed = vec![0.0; n];
        self.accumulate_delayed_source(&mut delayed);

        let mut neutro_iter = 0usize;
        let mut neutro_residual;
        loop {
            // fluxEq.H
            neutro_residual = 0.0_f64;
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
                // neutroSource_ -= nuSigmaEff[g] * flux_[g] * (1 - eigenvalueNeutronics_)
                {
                    let nf = self.xs.nu_sigma_f[gg].internal.as_slice();
                    let ph = self.state.flux()[gg].internal.as_slice();
                    for c in 0..n {
                        neutro_source[c] -= nf[c] * ph[c] * 1.0;
                    }
                }
                let nf = self.xs.nu_sigma_f[gg].internal.as_slice();
                let chi_p = self.xs.chi_prompt[gg].internal.as_slice();
                let chi_d = self.xs.chi_delayed[gg].internal.as_slice();
                let removal = self.xs.sigma_removal[gg].internal.as_slice();
                let iv = self.xs.inv_velocity[gg].internal.as_slice();

                // coefficient of `- fvm::Sp(...)`:
                // (nuSigmaEff/keff*(1-BetaTot)*chiPrompt*(1-eig) - sigmaRemoval)/discFactor
                let sp_coeff: Vec<f64> = (0..n)
                    .map(|c| ((((nf[c] / keff) * one_minus_beta[c]) * chi_p[c]) * 1.0 - removal[c]) / 1.0)
                    .collect();
                let prompt: Vec<f64> =
                    (0..n).map(|c| ((neutro_source[c] * one_minus_beta[c]) * chi_p[c]) / keff).collect();
                let delayed_g: Vec<f64> = (0..n).map(|c| delayed[c] * chi_d[c]).collect();
                let iv_over_disc: Vec<f64> = (0..n).map(|c| (1.0 * iv[c]) / 1.0).collect();

                let flux_g = &self.state.flux()[gg];
                let ddt = euler_ddt(&iv_over_disc, &flux_old[gg], &volumes, dt, flux_g);
                let lap = fvm::laplacian_with_delta(&self.xs.d_face[gg], flux_g, self.settings.delta_coeff);
                let sp_field = field_like(flux_g, sp_coeff);
                let eqn = ddt + lap - fvm::sp(&sp_field, flux_g)
                    - fvm::su(&field_like(flux_g, prompt), flux_g)
                    - fvm::su(&field_like(flux_g, delayed_g), flux_g)
                    - fvm::su(&field_like(flux_g, scatter), flux_g);
                let (sol, init_res) =
                    solve_twice(&eqn, &format!("fluxStar{gg}"), flux_g, self.settings.linear);
                neutro_residual = neutro_residual.max(init_res);
                self.write_group_flux(gg, &sol);

                // neutroSource_ += nuSigmaEff[g] * flux_[g] * (1 - eigenvalueNeutronics_)
                let nf = self.xs.nu_sigma_f[gg].internal.as_slice();
                let ph = self.state.flux()[gg].internal.as_slice();
                for c in 0..n {
                    neutro_source[c] += nf[c] * ph[c] * 1.0;
                }
            }

            // precEq.H, initializeDelayedNeutroSource.H
            self.upstream_prec_eq(&neutro_source, keff, &prec_star_old, &volumes, dt)?;
            delayed.iter_mut().for_each(|d| *d = 0.0);
            self.accumulate_delayed_source(&mut delayed);

            neutro_iter += 1;
            if !(neutro_residual > controls.neutron_iteration_residual
                && neutro_iter < controls.max_neutron_iterations)
            {
                break;
            }
        }

        self.update_derived_fields();
        Ok(UpstreamTransientStep {
            power_old,
            power: self.upstream_power(),
            k_eff: keff,
            neutron_iterations: neutro_iter,
            neutro_residual,
        })
    }

    /// `neutroSource_ *= 0; neutroSource_ += nuSigmaEff[g]*flux_[g]`.
    fn accumulate_neutro_source(&self, out: &mut [f64]) {
        for (g, flux) in self.state.flux().iter().enumerate() {
            let nf = self.xs.nu_sigma_f[g].internal.as_slice();
            let ph = flux.internal.as_slice();
            for c in 0..out.len() {
                out[c] += nf[c] * ph[c];
            }
        }
    }

    /// `delayedNeutroSource_ += lambda[k]*prec_[k]`.
    fn accumulate_delayed_source(&self, out: &mut [f64]) {
        for (k, prec) in self.state.precursors().iter().enumerate() {
            let lam = self.xs.lambda[k].internal.as_slice();
            let p = prec.internal.as_slice();
            for c in 0..out.len() {
                out[c] += lam[c] * p[c];
            }
        }
    }

    /// `precEq.H`, `liquidFuel true` branch, every group: solve
    /// `ddt(alpha, precStar) + Sp(lambda alpha, precStar) - neutroSource/keff*Beta
    ///  + div(phi, precStar) - laplacian(diffCoeffPrec, precStar)` once, then
    /// `prec = precStar*alpha`.
    fn upstream_prec_eq(
        &mut self,
        neutro_source: &[f64],
        keff: f64,
        prec_star_old: &[Vec<f64>],
        volumes: &[f64],
        dt: f64,
    ) -> Result<(), NeutronicsError> {
        let n = neutro_source.len();
        let Some(drift) = self.drift.as_ref() else {
            return Err(NeutronicsError::UpstreamTransient("set_precursor_drift was not called"));
        };
        let alpha = drift.alpha.internal.as_slice().to_vec();
        let diffusivity_face = drift.diffusivity_face();
        let phi = drift.phi.clone();
        let alpha_field = drift.alpha.clone();
        for k in 0..self.prec_groups() {
            let beta = self.xs.beta[k].internal.as_slice();
            let lambda = self.xs.lambda[k].internal.as_slice();
            let lambda_alpha: Vec<f64> = (0..n).map(|c| lambda[c] * alpha[c]).collect();
            let source: Vec<f64> = (0..n).map(|c| (neutro_source[c] / keff) * beta[c]).collect();
            let c_star = &self.prec_star[k];
            let ddt = euler_ddt(&alpha, &prec_star_old[k], volumes, dt, c_star);
            // Sign (gh:#952): `fvm::laplacian*` here is the POSITIVE -div(D grad),
            // so it is ADDED; upstream's `- fvm::laplacian` is OpenFOAM's negative one.
            let eqn = ddt
                + fvm::sp(&field_like(&alpha_field, lambda_alpha), c_star)
                - fvm::su(&field_like(&alpha_field, source), c_star)
                + fvm::div(&phi, c_star)
                + fvm::laplacian_with_delta(&diffusivity_face, c_star, self.settings.delta_coeff);
            let (sol, _perf) = eqn.solve_bicgstab_with_guess(
                format!("precStar{k}"),
                c_star,
                KrylovOptions::default(),
                self.settings.linear,
            );
            let sol_values = sol.internal.as_slice().to_vec();
            self.prec_star[k].internal = Field::new(sol_values.clone());
            let dst = self.state.precursors_mut()[k].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = sol_values[c] * alpha[c];
            }
        }
        Ok(())
    }
}
