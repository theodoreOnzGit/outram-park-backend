// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/include/
//                    {solveNeutronics,fluxEq,createFluxMatrices,createNeutronicsFields}.H,
//                    src/classes/neutronics/neutronics.{C,H} (externalSourceNeutronics_,
//                    externalSource_, externalSourceModulationTimeProfile_) and
//                    src/classes/neutronics/include/{precEq,initializeNeutroSource,
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

//! The external neutron source of `diffusionNeutronics`, and the `solveNeutronics.H` time step of the
//! STATIONARY-fuel diffusion model with `eigenvalueNeutronics false` (gh:#1863, round 32 of gh:#1361).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! # What upstream does
//!
//! `createFluxMatrices.H` builds, once per time step and before the neutron iteration, one matrix per group
//!
//! ```text
//! ddt((1-eig) IV/disc, fluxStar) - laplacian(D/disc, fluxStar) - Sp(coeff, fluxStar)
//!     - externalSourceFlux[g] * externalSourceModulation * externalSourceNeutronics
//! ```
//!
//! with `externalSourceFlux[g]` the neutrons per unit volume and time of group `g` (read from
//! `externalSourceFlux<g>`, else `defaultExternalSourceFlux`, else 0), `externalSourceModulation` the value of
//! `externalSourceModulationTimeProfile` of `constant/<region>/externalSource` at the new time (1 without
//! one), and `externalSourceNeutronics` the flag of `neutronicsProperties` (default `false`). The source is not
//! divided by the discontinuity factor and is not part of the eigenvalue branch. `fluxEq.H` then subtracts the
//! prompt, delayed and scattering sources and solves every group twice; `solveNeutronics.H` ends with
//! `reactorState_.set("pTarget", pTot)` when the source is on. The matrices are built BEFORE the loop, so the
//! explicit part of a `corrected` laplacian is frozen at the flux the step starts from: this port takes the
//! gradient once per step and rebuilds the (identical) matrix each iteration.
//!
//! SP3 and SN: `grep externalSource` over `SP3/` and `SN/` of 652b3da finds nothing, so they do not read the
//! flag; [`refuse_external_source_for_transport`] refuses the combination by name instead of ignoring it.
//!
//! # Deliberate differences from the text of upstream
//!
//! * Aitken acceleration, `integralPredictor`, `implicitPredictor`: not performed (all `false` in the case; the
//!   same decision as `upstream_transient.rs`, maintainer 2026-11-26 "follow upstream").
//! * Discontinuity factors are 1 (`adjustDiscFactors false`).
//! * `liquidFuel true` is refused here ([`DiffusionNeutronics::upstream_transient_step`] is that form, and has
//!   no external source: it is refused there too if a source is set).
//! * The linear solver is basic-lib's CG, not `PCG`/`DIC`.
//! * ~~A time profile is supplied by the caller ([`ExternalSource::with_modulation`]); the reader of the
//!   `externalSource` dictionary refuses a case that has one.~~ **CORRECTED 2026-11-27 (round 32b):** the
//!   reader of `constant/<region>/externalSource` reads `externalSourceModulationTimeProfile` of `type table`
//!   (`external_source_case::parse_external_source_time_profile`, compared with the real binary); `type fmi`
//!   and every other `Function1` are refused by name.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::fv_operators::fvc;
use outram_foam_basic_lib::prelude::{fvm, FvMatrix, NonOrthoScheme};
use uom::si::f64::Time;
use uom::si::time::second;

use super::upstream_transient::euler_ddt;
use super::DiffusionNeutronics;
use crate::genfoam::common::TimeProfile;
use outram_foam_basic_lib::prelude::{Field, VolScalarField};
use crate::genfoam::neutronics::pseudo_time::{
    adjust_delta_t, field_like, neutronics_max_delta_t, solve_twice_residuals, PseudoTimeClock, PseudoTimeControls,
};
use crate::genfoam::neutronics::NeutronicsError;

/// The `ddtSchemes` entry the neutronics region uses (`system/<region>/fvSchemes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronicsDdtScheme {
    /// `steadyState`: `fvm::ddt` is the zero matrix, every step is a steady solve (the tutorial's setting).
    SteadyState,
    /// `Euler`: `EulerDdtScheme::fvmDdt` (`diag = rDeltaT c V`, `source = rDeltaT c old V`).
    Euler,
}

/// The `laplacianSchemes` entry for the flux (`Gauss linear <snGrad>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeutronicsLaplacianScheme {
    /// `Gauss linear uncorrected`: implicit part with `nonOrthDeltaCoeffs`, no explicit correction (the 2D_MSFR).
    Uncorrected,
    /// `Gauss linear corrected`: the explicit non-orthogonal correction of `correctedSnGrad`, its gradient the
    /// `Gauss linear` one (the external-source tutorial).
    Corrected,
}

/// What the stationary-fuel step reads from `fvSchemes` and `fvSolution`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StationaryStepControls {
    /// `ddtSchemes default`.
    pub ddt: NeutronicsDdtScheme,
    /// `laplacianSchemes default`.
    pub laplacian: NeutronicsLaplacianScheme,
    /// `neutronIterationResidual` (upstream default `1e-5`).
    pub neutron_iteration_residual: f64,
    /// `maxNeutronIterations` (upstream default 10).
    pub max_neutron_iterations: usize,
}

impl Default for StationaryStepControls {
    fn default() -> Self {
        Self {
            ddt: NeutronicsDdtScheme::Euler,
            laplacian: NeutronicsLaplacianScheme::Uncorrected,
            neutron_iteration_residual: 1.0e-5,
            max_neutron_iterations: 10,
        }
    }
}

/// The external neutron source of a diffusion region: `externalSourceNeutronics_`, the per-group
/// `externalSourceFlux[g]` (neutrons per unit volume and time, one value per cell) and the optional time
/// modulation.
#[derive(Debug, Clone)]
pub struct ExternalSource {
    /// `externalSourceNeutronics_`.
    pub enabled: bool,
    /// `externalSourceFlux[g]`, one list of cell values per energy group.
    pub flux: Vec<Vec<f64>>,
    /// `externalSourceModulationTimeProfile_` (`None` is an invalid profile: modulation 1).
    pub modulation: Option<TimeProfile>,
}

impl ExternalSource {
    /// A source of the given per-group cell values, switched on, unmodulated.
    #[must_use]
    pub fn new(flux: Vec<Vec<f64>>) -> Self {
        Self { enabled: true, flux, modulation: None }
    }

    /// A source that is present in the case files but switched off (`externalSourceNeutronics false`).
    #[must_use]
    pub fn switched_off(flux: Vec<Vec<f64>>) -> Self {
        Self { enabled: false, flux, modulation: None }
    }

    /// The same source with `externalSourceModulationTimeProfile` set.
    #[must_use]
    pub fn with_modulation(mut self, profile: TimeProfile) -> Self {
        self.modulation = Some(profile);
        self
    }

    /// `externalSourceModulation` of `createFluxMatrices.H` at `time` (`timeOutputValue`).
    #[must_use]
    pub fn modulation_at(&self, time: f64) -> f64 {
        match &self.modulation {
            Some(p) if p.valid() => p.value(Time::new::<second>(time)),
            _ => 1.0,
        }
    }

    /// The integral `sum_c V_c S_c` of group `g` (neutrons per second), what the tutorial prints as
    /// `volIntegrate(zone0) of externalSourceFlux<g>`.
    #[must_use]
    pub fn group_integral(&self, g: usize, volumes: &[f64]) -> f64 {
        let mut s = 0.0;
        for (v, e) in volumes.iter().zip(self.flux[g].iter()) {
            s += v * e;
        }
        s
    }
}

/// SP3 and SN do not read `externalSourceNeutronics` (`grep externalSource` over `SP3/` and `SN/` of
/// GeN-Foam 652b3da finds nothing): a case that sets it with one of them would silently run without the
/// source upstream. The port refuses the combination by name.
///
/// # Errors
///
/// [`NeutronicsError::UpstreamTransient`] naming the solver, when `external_source_neutronics` is `true` and
/// `solver` is `"SP3Neutronics"` or `"SNNeutronics"`.
pub fn refuse_external_source_for_transport(solver: &str, external_source_neutronics: bool) -> Result<(), NeutronicsError> {
    if !external_source_neutronics {
        return Ok(());
    }
    match solver {
        "SP3Neutronics" => Err(NeutronicsError::UpstreamTransient(
            "externalSourceNeutronics true with SP3Neutronics: upstream's SP3 solver does not read it (silently runs source-free); not ported",
        )),
        "SNNeutronics" => Err(NeutronicsError::UpstreamTransient(
            "externalSourceNeutronics true with SNNeutronics: upstream's SN solver does not read it (silently runs source-free); not ported",
        )),
        _ => Ok(()),
    }
}

/// What one stationary-fuel step reports (the lines the upstream log prints).
#[derive(Debug, Clone, PartialEq)]
pub struct StationaryStep {
    /// `power()` before the step (`pTotOld_`).
    pub power_old: f64,
    /// `power()` after the step (`power:`).
    pub power: f64,
    /// Neutron iterations performed (`Neutron iteration no:` lines of the step).
    pub neutron_iterations: usize,
    /// `neutroResidual` after each iteration (`neutroResidual (neutroIter: k)`).
    pub residual_history: Vec<f64>,
    /// `getResidual()` of the step (`residual_` of `diffusionNeutronics`, gh:#1950): `max` over every pass and group of the SECOND solve's initial
    /// residual (`fluxEq.H:29`), then `max` with the last pass's `neutroResidual` (`solveNeutronics.H:138`).
    pub solver_residual: f64,
    /// `pTarget` written to `reactorState` at the end of the step: `Some(pTot)` with the source on.
    pub p_target: Option<f64>,
    /// `integralPredictor.H`'s report (gh:#1919) when `neutronTransport.integralPredictor` is on, else `None`.
    pub predictor: Option<super::integral_predictor::IntegralPredictorReport>,
}

/// One row of the history of [`DiffusionNeutronics::run_stationary_transient`].
#[derive(Debug, Clone, PartialEq)]
pub struct StationaryRecord {
    /// `Time = ` of the step.
    pub time: f64,
    /// The `deltaT` the step was taken with.
    pub delta_t: f64,
    /// The `deltaT` printed after the step (`adjustDeltaT`).
    pub next_delta_t: f64,
    /// The step's report.
    pub step: StationaryStep,
}

impl DiffusionNeutronics {
    /// Set the external source (`externalSourceNeutronics_` and `externalSourceFlux[g]`).
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::UpstreamTransient`] if the lists are not `energy_groups x n_cells`.
    pub fn set_external_source(&mut self, source: ExternalSource) -> Result<(), NeutronicsError> {
        let n = self.state.mesh().n_cells;
        if source.flux.len() != self.energy_groups() || source.flux.iter().any(|s| s.len() != n) {
            return Err(NeutronicsError::UpstreamTransient("externalSourceFlux is not sized groups x cells"));
        }
        self.external_source = Some(source);
        Ok(())
    }

    /// The external source, if one was set.
    #[must_use]
    pub fn external_source(&self) -> Option<&ExternalSource> {
        self.external_source.as_ref()
    }

    /// `precEq.H`, `liquidFuel false` branch, every group:
    /// `ddt(1, prec) + Sp(lambda, prec) - neutroSource/keff*Beta`, a diagonal matrix, solved as
    /// `source/diag` (OpenFOAM's `diagonalSolver`). `ddt` is zero for `steadyState`.
    fn stationary_prec_eq(
        &mut self,
        neutro_source: &[f64],
        keff: f64,
        ddt: NeutronicsDdtScheme,
        prec_old: &[Vec<f64>],
        volumes: &[f64],
        dt: f64,
    ) {
        let n = neutro_source.len();
        let r_delta_t = 1.0 / dt;
        // `fvm::ddt(dimensionedScalar, vf)` on a moving mesh: `Vsc()` in the diagonal, `Vsc0()` in the source (gh:#1856)
        let volumes_old: Vec<f64> = self.old_volumes_or(volumes).to_vec();
        for k in 0..self.prec_groups() {
            let beta = self.xs.beta[k].internal.as_slice();
            let lambda = self.xs.lambda[k].internal.as_slice();
            let dst = self.state.precursors_mut()[k].internal.as_mut_slice();
            for c in 0..n {
                let (mut diag, mut source) = match ddt {
                    NeutronicsDdtScheme::SteadyState => (0.0, 0.0),
                    NeutronicsDdtScheme::Euler => (
                        (r_delta_t * 1.0) * volumes[c],
                        ((r_delta_t * 1.0) * prec_old[k][c]) * volumes_old[c],
                    ),
                };
                diag += volumes[c] * lambda[c];
                source += volumes[c] * ((neutro_source[c] / keff) * beta[c]);
                dst[c] = source / diag;
            }
        }
    }

    /// The group matrix of `createFluxMatrices.H` (`gradient` is the step-start `Gauss linear` gradient of
    /// the group's flux, used by a `corrected` laplacian only).
    #[allow(clippy::too_many_arguments)]
    fn stationary_group_matrix(
        &self,
        g: usize,
        controls: &StationaryStepControls,
        gradient: Option<&outram_foam_basic_lib::prelude::VolVectorField>,
        star_field: &VolScalarField,
        flux_old: &[f64],
        volumes: &[f64],
        dt: f64,
        external: Option<&[f64]>,
    ) -> FvMatrix {
        let n = volumes.len();
        let one_minus_beta: Vec<f64> = self.xs.beta_tot.internal.as_slice().iter().map(|b| 1.0 - b).collect();
        let nf = self.xs.nu_sigma_f[g].internal.as_slice();
        let chi_p = self.xs.chi_prompt[g].internal.as_slice();
        let removal = self.xs.sigma_removal[g].internal.as_slice();
        let iv = self.xs.inv_velocity[g].internal.as_slice();
        let keff = self.state.k_eff_raw();
        // coefficient of `- fvm::Sp(...)`: (nuSigmaEff/keff (1-BetaTot) chiPrompt (1-eig) - sigmaRemoval)/disc
        let df = self.xs.disc_factor[g].internal.as_slice();
        let sp_coeff: Vec<f64> = (0..n)
            .map(|c| ((((nf[c] / keff) * one_minus_beta[c]) * chi_p[c]) * 1.0 - removal[c]) / df[c])
            .collect();
        let iv_over_disc: Vec<f64> = (0..n).map(|c| (1.0 * iv[c]) / df[c]).collect();
        // `fvm::laplacian(D/discFactor, fluxStar_)` (createFluxMatrices.H:27): the unknown is `fluxStar_`
        let d_over_df: Vec<f64> = (0..n).map(|c| self.xs.d[g].internal.as_slice()[c] / df[c]).collect();
        let d_face = fvc::interpolate(&field_like(star_field, d_over_df));

        let flux_g = star_field;
        let ddt = match controls.ddt {
            NeutronicsDdtScheme::SteadyState => FvMatrix::new(flux_g.mesh.clone()),
            NeutronicsDdtScheme::Euler => euler_ddt(&iv_over_disc, flux_old, volumes, self.old_volumes_or(volumes), dt, flux_g),
        };
        let lap = match (controls.laplacian, gradient) {
            (NeutronicsLaplacianScheme::Corrected, Some(grad)) => {
                fvm::laplacian_corrected(&d_face, flux_g, grad, NonOrthoScheme::Corrected)
            }
            _ => fvm::laplacian_with_delta(&d_face, flux_g, self.settings.delta_coeff),
        };
        let mut eqn = ddt + lap - fvm::sp(&field_like(flux_g, sp_coeff), flux_g);
        if let Some(ext) = external {
            eqn = eqn - fvm::su(&field_like(flux_g, ext.to_vec()), flux_g);
        }
        eqn
    }

    /// One `solveNeutronics.H` time step, `eigenvalueNeutronics false`, `liquidFuel false`, with the external
    /// source set by [`Self::set_external_source`] (none: no source). `time` is the time AFTER the increment
    /// (`timeOutputValue`, for the modulation), `dt` the step, `keff_` is `state().k_eff_raw()`, unchanged.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::NonPositiveTimeStep`]; [`NeutronicsError::UpstreamTransient`] when circulating fuel is
    /// set (that is [`Self::upstream_transient_step`]).
    pub fn upstream_stationary_step(
        &mut self,
        time: f64,
        dt: f64,
        controls: &StationaryStepControls,
    ) -> Result<StationaryStep, NeutronicsError> {
        // The factors of the dictionary (`XS::correct()` at `timeIndex > 0`) are applied (gh:#1887). A time-varying
        // `XsEvolution` (driveline, adjustment) is the eigenvalue driver's: `adjustDiscFactors` is in the eigenvalue
        // branch only, and the driveline is not compared for this step: refused by name.
        if dt <= 0.0 || dt.is_nan() {
            return Err(NeutronicsError::NonPositiveTimeStep(dt));
        }
        if self.drift.is_some() {
            return Err(NeutronicsError::UpstreamTransient(
                "upstream_stationary_step is the liquidFuel false form; circulating fuel uses upstream_transient_step",
            ));
        }
        let n = self.state.mesh().n_cells;
        let groups = self.energy_groups();
        let volumes = self.state.mesh().cell_volumes.clone();
        let keff = self.state.k_eff_raw();
        let power_old = self.upstream_power();
        // gh:#1893 (round 43): AFTER `pTotOld_ = power()` (`correctPhysics`), the time-dependent cross sections of an `XsEvolution` (control-rod driveline motion, the
        // discontinuity-factor lists) are materialised at the time of the step, as `xs_.correct()` does at the top of
        // `solveNeutronics.H`; `adjustDiscFactors` stays in the eigenvalue branch only, so it is never called here.
        if let Some(evolution) = &self.evolution {
            let mesh = self.state.mesh().clone();
            self.xs = evolution
                .materialize(&mesh, time)
                .map_err(|_| NeutronicsError::UpstreamTransient("the cross sections of the XsEvolution could not be materialised"))?;
            for (patch, gamma, lin) in self.albedo_specs.clone() {
                self.set_albedo_boundary(patch, gamma, lin);
            }
        }
        // `fluxStar_` (read from the case, else `defaultFlux`: the state's flux at the first step), then
        // `flux_ = fluxStar_/discFactor` (solveNeutronics.H:29-32); the `ddt` takes `fluxStar_.oldTime()`
        let mut star: Vec<Vec<f64>> = match self.flux_star.take() {
            Some(v) => v,
            None => self.state.flux().iter().map(|f| f.internal.as_slice().to_vec()).collect(),
        };
        for g in 0..groups {
            let df = self.xs.disc_factor[g].internal.as_slice();
            let dst = self.state.flux_mut()[g].internal.as_mut_slice();
            for c in 0..n {
                dst[c] = star[g][c] / df[c];
            }
        }
        let flux_old: Vec<Vec<f64>> = match &self.stationary_old {
            Some((f, _)) => f.clone(),
            None => star.clone(),
        };
        let mut star_fields: Vec<VolScalarField> = (0..groups)
            .map(|g| field_with_boundary(&self.state.flux()[g], star[g].clone()))
            .collect();
        let prec_old: Vec<Vec<f64>> = match &self.stationary_old {
            Some((_, p)) => p.clone(),
            None => self.state.precursors().iter().map(|p| p.internal.as_slice().to_vec()).collect(),
        };
        let one_minus_beta: Vec<f64> = self.xs.beta_tot.internal.as_slice().iter().map(|b| 1.0 - b).collect();

        // createFluxMatrices.H: externalSourceFlux[g] * externalSourceModulation * externalSourceNeutronics
        let external: Option<Vec<Vec<f64>>> = match &self.external_source {
            Some(s) if s.enabled => {
                let m = s.modulation_at(time);
                Some(s.flux.iter().map(|e| e.iter().map(|x| (x * m) * 1.0).collect()).collect())
            }
            _ => None,
        };

        // initializeNeutroSource.H, precEq.H, initializeDelayedNeutroSource.H
        let mut neutro_source = vec![0.0; n];
        self.accumulate_neutro_source(&mut neutro_source);
        self.stationary_prec_eq(&neutro_source, keff, controls.ddt, &prec_old, &volumes, dt);
        let mut delayed = vec![0.0; n];
        self.accumulate_delayed_source(&mut delayed);

        // `integralPredictor.H` (`solveNeutronics.H`, `eigenvalueNeutronics false`): scales `fluxStar_` BEFORE `createFluxMatrices.H`, so the
        // matrices and the explicit correction below see the scaled fluxes; the `ddt` old-time values (`flux_old`) are untouched
        let predictor = if self.integral_predictor {
            let r = self.run_integral_predictor(&mut star, &star_fields, &flux_old, &mut neutro_source, &delayed, dt, keff);
            star_fields = (0..groups).map(|g| field_with_boundary(&self.state.flux()[g], star[g].clone())).collect();
            Some(r)
        } else {
            None
        };
        // `implicitPredictor.H` (`solveNeutronics.H`): AFTER `integralPredictor.H`, on the star flux that left it (gh:#1926, README round 66)
        if self.implicit_predictor {
            self.run_implicit_predictor(&mut star, &star_fields, &flux_old, &mut neutro_source, &delayed, dt, keff)?;
            star_fields = (0..groups).map(|g| field_with_boundary(&self.state.flux()[g], star[g].clone())).collect();
        }

        // createFluxMatrices.H: the explicit correction is taken from the flux the step starts from
        let gradients: Vec<Option<outram_foam_basic_lib::prelude::VolVectorField>> = (0..groups)
            .map(|g| match controls.laplacian {
                NeutronicsLaplacianScheme::Corrected => Some(fvc::grad(&star_fields[g])),
                NeutronicsLaplacianScheme::Uncorrected => None,
            })
            .collect();

        let mut neutro_iter = 0usize;
        let mut residual_history = Vec::new();
        let mut solver_residual = 0.0_f64;
        loop {
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
                // neutroSource_ -= nuSigmaEff[g] * flux_[g] * (1 - eigenvalueNeutronics_)
                {
                    let nf = self.xs.nu_sigma_f[gg].internal.as_slice();
                    let ph = self.state.flux()[gg].internal.as_slice();
                    for c in 0..n {
                        neutro_source[c] -= nf[c] * ph[c] * 1.0;
                    }
                }
                let chi_p = self.xs.chi_prompt[gg].internal.as_slice();
                let chi_d = self.xs.chi_delayed[gg].internal.as_slice();
                let prompt: Vec<f64> =
                    (0..n).map(|c| ((neutro_source[c] * one_minus_beta[c]) * chi_p[c]) / keff).collect();
                let delayed_g: Vec<f64> = (0..n).map(|c| delayed[c] * chi_d[c]).collect();

                let base = self.stationary_group_matrix(
                    gg,
                    controls,
                    gradients[gg].as_ref(),
                    &star_fields[gg],
                    &flux_old[gg],
                    &volumes,
                    dt,
                    external.as_ref().map(|e| e[gg].as_slice()),
                );
                let flux_g = &self.state.flux()[gg];
                let eqn = base
                    - fvm::su(&field_like(flux_g, prompt), flux_g)
                    - fvm::su(&field_like(flux_g, delayed_g), flux_g)
                    - fvm::su(&field_like(flux_g, scatter), flux_g);
                let (sol, init_res, second_res) =
                    solve_twice_residuals(&eqn, &format!("fluxStar{gg}"), &star_fields[gg], self.settings.linear_solve());
                neutro_residual = neutro_residual.max(init_res);
                solver_residual = solver_residual.max(second_res);
                // fluxStar_ = solution; flux_ = fluxStar_/discFactor (fluxEq.H:31)
                star[gg] = sol.internal.as_slice().to_vec();
                star_fields[gg] = field_with_boundary(&star_fields[gg], star[gg].clone());
                {
                    let df = self.xs.disc_factor[gg].internal.as_slice();
                    let dst = self.state.flux_mut()[gg].internal.as_mut_slice();
                    for c in 0..n {
                        dst[c] = star[gg][c] / df[c];
                    }
                }

                // neutroSource_ += nuSigmaEff[g] * flux_[g] * (1 - eigenvalueNeutronics_)
                let nf = self.xs.nu_sigma_f[gg].internal.as_slice();
                let ph = self.state.flux()[gg].internal.as_slice();
                for c in 0..n {
                    neutro_source[c] += nf[c] * ph[c] * 1.0;
                }
            }

            // precEq.H, initializeDelayedNeutroSource.H
            self.stationary_prec_eq(&neutro_source, keff, controls.ddt, &prec_old, &volumes, dt);
            delayed.iter_mut().for_each(|d| *d = 0.0);
            self.accumulate_delayed_source(&mut delayed);

            neutro_iter += 1;
            residual_history.push(neutro_residual);
            if !(neutro_residual > controls.neutron_iteration_residual
                && neutro_iter < controls.max_neutron_iterations)
            {
                break;
            }
        }

        self.flux_star = Some(star);
        self.update_derived_fields();
        let power = self.upstream_power();
        let p_target = match &self.external_source {
            Some(s) if s.enabled => Some(power),
            _ => None,
        };
        let solver_residual = solver_residual.max(residual_history.last().copied().unwrap_or(0.0));
        Ok(StationaryStep { power_old, power, neutron_iterations: neutro_iter, residual_history, solver_residual, p_target, predictor })
    }

    /// GeN-Foam's main loop for a single stationary-fuel diffusion region
    /// (`setDeltaT; while (run) { runTime++; correctPhysics; adjustDeltaT }`), the same clock as
    /// [`crate::genfoam::neutronics::pseudo_time::run_pseudo_time_eigenvalue`], with
    /// [`Self::upstream_stationary_step`] as `correctPhysics`.
    ///
    /// Returns the history and the `pTarget` of `reactorState` after the last step (`None` without a source).
    ///
    /// # Errors
    ///
    /// The first error of a step.
    pub fn run_stationary_transient(
        &mut self,
        clock_controls: &PseudoTimeControls,
        controls: &StationaryStepControls,
    ) -> Result<(Vec<StationaryRecord>, Option<f64>), NeutronicsError> {
        let mut clock = PseudoTimeClock::new(clock_controls);
        let mut history = Vec::new();
        let mut p_target = None;
        while clock.run() {
            clock.advance();
            let delta_t = clock.delta_t();
            let step = self.upstream_stationary_step(clock.value(), delta_t, controls)?;
            if step.p_target.is_some() {
                p_target = step.p_target;
            }
            let solver_max = neutronics_max_delta_t(clock_controls, &clock, step.power, step.power_old);
            adjust_delta_t(clock_controls, &mut clock, solver_max);
            history.push(StationaryRecord { time: clock.value(), delta_t, next_delta_t: clock.delta_t(), step });
        }
        Ok((history, p_target))
    }
}

/// `template` (keeping its boundary conditions and patch values) with the internal values replaced.
fn field_with_boundary(template: &VolScalarField, values: Vec<f64>) -> VolScalarField {
    let mut f = template.clone();
    f.internal = Field::new(values);
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_with_boundary_replaces_only_the_internal_values() {
        use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
        use uom::si::area::square_meter;
        use uom::si::f64::{Area, Length};
        use uom::si::length::meter;
        let mesh = alloc::sync::Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), 3).expect("mesh"),
        );
        let t = VolScalarField::uniform("t", mesh, 1.0);
        let f = field_with_boundary(&t, vec![4.0, 5.0, 6.0]);
        assert_eq!(f.internal.as_slice(), &[4.0, 5.0, 6.0]);
        assert_eq!(f.boundary.len(), t.boundary.len());
    }

    #[test]
    fn sp3_and_sn_with_a_source_are_refused_by_name() {
        assert!(refuse_external_source_for_transport("SP3Neutronics", true).is_err());
        assert!(refuse_external_source_for_transport("SNNeutronics", true).is_err());
        assert!(refuse_external_source_for_transport("SP3Neutronics", false).is_ok());
        assert!(refuse_external_source_for_transport("diffusionNeutronics", true).is_ok());
    }

    #[test]
    fn modulation_is_one_without_a_profile_and_the_profile_value_with_one() {
        let s = ExternalSource::new(vec![vec![2.0]]);
        assert_eq!(s.modulation_at(3.0), 1.0);
        let p = TimeProfile::constant(0.25);
        assert_eq!(s.with_modulation(p).modulation_at(3.0), 0.25);
    }

    #[test]
    fn group_integral_is_the_volume_weighted_sum() {
        let s = ExternalSource::new(vec![vec![1.0, 3.0]]);
        assert_eq!(s.group_integral(0, &[2.0, 0.5]), 3.5);
        assert!(!ExternalSource::switched_off(vec![vec![1.0]]).enabled);
    }
}
