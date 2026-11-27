// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/pointKinetics/include/solvePointKineticsLiquidFuel.H,
//                    pointKineticNeutronics.{C,H} (liquidFuel branches)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506; license GPL-3.0
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `pointKineticNeutronics` with `liquidFuel true` (gh:#1882, round 48)
//!
//! A line-by-line translation of `solvePointKineticsLiquidFuel.H`: the precursors `precPKStar_` are
//! transported on the neutronics mesh by the equation of [`super::super::cyclic_transport`] with the
//! fluid fields `U`, `alpha`, `alphat`, `mu` handed in (upstream maps them from the fluid region), an
//! equilibrium solve without `ddt` at the first step (`initPrecursorsLiquidFuel`), `liquidFuelBeta_`
//! and `precEquilibriumReactivity_` from it, and the neutron iteration `power_ = (powerOld/dt +
//! sum weightedPrec lambda)/(1/dt - (rho - beta)/pgt)` until `neutroResidual` is below
//! `neutronIterationResidual` or `maxNeutronIterations` is reached.
//!
//! Upstream behaviours reproduced: the references `T*Ref_` are set only when `initialize` (so with
//! `initPrecursorsLiquidFuel false` they stay 0); `alpha.oldTime()` and `precPKStar.oldTime()` follow
//! `GeometricField::storeOldTimes()` ([`AlphaUpdate`]); the decay power, the external source and the
//! 0-D matrix of the solid-fuel step play no role.
//!
//! Refused by name: the coupled fluid region (the fluid fields are inputs here), `reactorState`
//! references, `GEMReactivityMap` (as in the solid-fuel class).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::prelude::Vector3;

use super::{
    FeedbackReferences, FeedbackValues, PkConfig, PkFeedbackFields, PkMeshData, PkModelError, PointKineticNeutronics,
    ReactivityTerms,
};
use crate::genfoam::neutronics::point_kinetics::cyclic_transport::{
    assemble_precursor_matrix, FaceFlux, InterfaceMatrix, PrecursorEquationInputs, TransportError, TransportMesh,
};
use crate::genfoam::neutronics::point_kinetics::LinearSolveForm;

/// How the class' `alpha` field is written each step, which decides `alpha.oldTime()` of `fvm::ddt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaUpdate {
    /// `alpha = mapped` (`GeometricField::operator=`): `storeOldTimes()` first, so `oldTime()` is the
    /// value of the previous time index.
    Assign,
    /// Element writes (`alpha[i] = ...`): nothing is stored; `oldTime()` copies the CURRENT value at the
    /// first `ddt` of the time index.
    Direct,
}

/// The keys of the liquid-fuel branch.
#[derive(Debug, Clone, Copy)]
pub struct LiquidFuelConfig {
    /// `nuclearData` `ScNo` (default 1).
    pub sc_no: f64,
    /// `nuclearData` `initPrecursorsLiquidFuel` (default false).
    pub init_precursors: bool,
    /// `fvSolution` `neutronTransport` `neutronIterationResidual` (default 1e-5).
    pub neutron_iteration_residual: f64,
    /// `fvSolution` `neutronTransport` `maxNeutronIterations` (default 10).
    pub max_neutron_iterations: usize,
    /// `fvSolution` entry of `precPKStar*`: `tolerance`.
    pub solver_tolerance: f64,
    /// `relTol`.
    pub solver_rel_tol: f64,
    /// `maxIter` (default 1000).
    pub solver_max_iter: usize,
    /// `minIter` (default 0).
    pub solver_min_iter: usize,
    /// How `alpha` is written.
    pub alpha_update: AlphaUpdate,
}

/// The fluid fields mapped onto the neutronics mesh (cell values).
#[derive(Debug, Clone)]
pub struct FluidFields {
    /// `U`.
    pub u: Vec<Vector3>,
    /// `alpha`.
    pub alpha: Vec<f64>,
    /// `alphat`.
    pub alphat: Vec<f64>,
    /// `mu`.
    pub mu: Vec<f64>,
}

/// What one liquid-fuel step reports.
#[derive(Debug, Clone, PartialEq)]
pub struct LiquidFuelStepReport {
    /// `initialize` was true.
    pub initialized: bool,
    /// The weighted averages.
    pub values: FeedbackValues,
    /// Reactivity terms of the last `correctReactivity.H`.
    pub terms: ReactivityTerms,
    /// Number of neutron iterations.
    pub neutron_iterations: usize,
    /// `neutroResidual` after each iteration.
    pub neutro_residuals: Vec<f64>,
    /// The intermediate power after each iteration.
    pub intermediate_powers: Vec<f64>,
    /// `(iterations, initial residual, final residual)` of every `PBiCG` solve, equilibrium solves first.
    pub solves: Vec<(usize, f64, f64)>,
    /// `liquidFuelBeta_`.
    pub liquid_fuel_beta: f64,
    /// `precEquilibriumReactivity_`.
    pub prec_equilibrium_reactivity: f64,
}

/// Errors of the liquid-fuel class.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LiquidFuelError {
    /// The solid-fuel base class refused.
    #[error(transparent)]
    Base(#[from] PkModelError),
    /// The transport mesh or an array refused.
    #[error(transparent)]
    Transport(#[from] TransportError),
}

/// `pointKineticNeutronics` with `liquidFuel true`.
#[derive(Debug, Clone)]
pub struct LiquidFuelPk {
    base: PointKineticNeutronics,
    mesh: TransportMesh,
    cfg: LiquidFuelConfig,
    init_n: Vec<f64>,
    dom_int_n: f64,
    star: Vec<Vec<f64>>,
    star_old: Vec<Vec<f64>>,
    star_old_index: usize,
    prec: Vec<Vec<f64>>,
    alpha: Vec<f64>,
    alpha_old: Option<Vec<f64>>,
    alpha_field_index: usize,
    time_index: usize,
    prec_equilibrium_reactivity: f64,
    liquid_fuel_beta: f64,
    non_orth_correction: bool,
    frozen: bool,
}

impl LiquidFuelPk {
    /// The constructor: the solid-fuel constructor plus `initOneGroupFluxN_`,
    /// `domainIntegratedInitOneGroupFluxN_` and the zero precursor fields (`defaultPrec_`).
    ///
    /// # Errors
    ///
    /// The errors of [`PointKineticNeutronics::new`].
    pub fn new(
        cfg: PkConfig,
        mesh_data: PkMeshData,
        initial_flux: Vec<f64>,
        power_density: Vec<f64>,
        secondary_power_density: Vec<f64>,
        transport: TransportMesh,
        lf: LiquidFuelConfig,
    ) -> Result<Self, LiquidFuelError> {
        let n = mesh_data.volume.len();
        let volume = mesh_data.volume.clone();
        let groups = cfg.betas.len();
        let base = PointKineticNeutronics::new(
            cfg,
            mesh_data,
            initial_flux,
            power_density,
            secondary_power_density,
            LinearSolveForm::OpenFoamGaussian,
        )?;
        // setInitOneGroupFlux: initN = initFlux / domainIntegrate(initFlux); sum V*sqr(initN)
        let flux = &base.one_group_flux;
        let mut integral = 0.0;
        for c in 0..n {
            integral += volume[c] * flux[c];
        }
        let init_n: Vec<f64> = (0..n).map(|c| flux[c] / integral).collect();
        let mut dom_int_n = 0.0;
        for c in 0..n {
            dom_int_n += volume[c] * (init_n[c] * init_n[c]);
        }
        Ok(Self {
            base,
            mesh: transport,
            cfg: lf,
            init_n,
            dom_int_n,
            star: vec![vec![0.0; n]; groups],
            star_old: vec![vec![0.0; n]; groups],
            star_old_index: usize::MAX,
            prec: vec![vec![0.0; n]; groups],
            alpha: vec![1.0; n],
            alpha_old: None,
            alpha_field_index: 0,
            time_index: 0,
            prec_equilibrium_reactivity: 0.0,
            liquid_fuel_beta: 0.0,
            non_orth_correction: true,
            frozen: false,
        })
    }

    /// The ablation: leave out the explicit non-orthogonal correction of the precursor laplacian
    /// (a named deviation; upstream applies it).
    #[must_use]
    pub fn without_non_orth_correction(mut self) -> Self {
        self.non_orth_correction = false;
        self
    }

    /// The ablation: the precursors are NOT transported after the equilibrium initialisation (the neutron
    /// iteration still runs on the frozen distribution). A control, not a physical model.
    #[must_use]
    pub fn with_frozen_precursors(mut self) -> Self {
        self.frozen = true;
        self
    }

    /// `alpha = mapped` or `alpha[i] = ...` (see [`AlphaUpdate`]).
    fn write_alpha(&mut self, new: &[f64], time_index: usize) {
        if self.cfg.alpha_update == AlphaUpdate::Assign && self.alpha_old.is_some() && self.alpha_field_index != time_index
        {
            self.alpha_old = Some(self.alpha.clone());
        }
        if self.cfg.alpha_update == AlphaUpdate::Assign {
            self.alpha_field_index = time_index;
        }
        self.alpha = new.to_vec();
    }

    /// `alpha.oldTime()` as the first `ddt` of a step sees it (`GeometricField::oldTime()`).
    fn old_time_alpha(&mut self, time_index: usize) -> Vec<f64> {
        let store = match self.alpha_old {
            None => true,
            Some(_) => self.alpha_field_index != time_index,
        };
        if store {
            self.alpha_old = Some(self.alpha.clone());
        }
        self.alpha_field_index = time_index;
        self.alpha_old.clone().unwrap_or_default()
    }

    /// `weightedPrec_i = domainIntegrate(initN precPK_i) / domainIntegratedInitN`.
    fn weighted_prec(&self, group: usize) -> f64 {
        let mut s = 0.0;
        for c in 0..self.mesh.n_cells {
            s += self.mesh.volume[c] * (self.init_n[c] * self.prec[group][c]);
        }
        s / self.dom_int_n
    }

    /// `precPK_i = precPKStar_i * alpha`.
    fn update_prec(&mut self, group: usize) {
        for c in 0..self.mesh.n_cells {
            self.prec[group][c] = self.star[group][c] * self.alpha[c];
        }
    }

    /// The source field `initN power beta / pgt` of group `group`:
    /// `((initN*power)*beta)/pgt`.
    fn su_field(&self, group: usize, power: f64) -> Vec<f64> {
        let beta = self.base.cfg.betas[group];
        let pgt = self.base.cfg.prompt_generation_time;
        (0..self.mesh.n_cells).map(|c| ((self.init_n[c] * power) * beta) / pgt).collect()
    }

    /// One call of `correctPhysics()` (`solvePointKineticsLiquidFuel.H`). `time_index` is
    /// `runTime.timeIndex()` after `runTime++` (1 for the first step); `t` and `dt` the time and the step.
    ///
    /// # Errors
    ///
    /// Field-length mismatches.
    pub fn step(
        &mut self,
        time_index: usize,
        t: f64,
        dt: f64,
        fields: &PkFeedbackFields,
        fluid: &FluidFields,
    ) -> Result<LiquidFuelStepReport, LiquidFuelError> {
        let n = self.mesh.n_cells;
        let groups = self.base.cfg.betas.len();
        let lambdas = self.base.cfg.lambdas.clone();
        let pgt = self.base.cfg.prompt_generation_time;
        // the mapped fields are written before correctPhysics()
        self.write_alpha(&fluid.alpha, time_index);
        // pTotOld_ = power_
        self.base.p_tot_old = self.base.power;
        let values = self.base.feedback_values(fields)?;
        if self.time_index != time_index {
            self.time_index = time_index;
            self.base.power_old = self.base.power;
        }
        let mut solves: Vec<(usize, f64, f64)> = vec![];
        let initialize = self.cfg.init_precursors && time_index == 1;
        if initialize {
            self.base.refs = Some(FeedbackReferences {
                t_fuel: values.t_fuel,
                t_clad: values.t_clad,
                t_cool: values.t_cool,
                rho_cool: values.rho_cool,
                t_struct: values.t_struct,
                t_struct_mech: values.t_struct_mech,
                t_driveline: values.t_driveline,
            });
        }
        // references that were never set are 0 (constructor)
        let refs = self.base.refs.unwrap_or(FeedbackReferences {
            t_fuel: 0.0,
            t_clad: 0.0,
            t_cool: 0.0,
            rho_cool: 0.0,
            t_struct: 0.0,
            t_struct_mech: 0.0,
            t_driveline: 0.0,
        });
        // #include "correctReactivity.H"
        let mut terms = self.base.reactivity_terms_with_prec(&values, &refs, t, self.prec_equilibrium_reactivity);
        self.base.total_reactivity = terms.total;

        // diffCoeffPrec = (alphat + mu/ScNo)/rhoCool ; phi = fvc::flux(U*alpha)
        let diff: Vec<f64> = (0..n).map(|c| (fluid.alphat[c] + fluid.mu[c] / self.cfg.sc_no) / fields.rho_cool[c]).collect();
        let phi = self.mesh.flux_of_u_alpha(&fluid.u, &self.alpha)?;

        let power = self.base.power;
        if initialize {
            for g in 0..groups {
                let inp = PrecursorEquationInputs {
                    alpha: self.alpha.clone(),
                    lambda: lambdas[g],
                    su: self.su_field(g, power),
                    gamma: diff.clone(),
                    phi: phi.clone(),
                    ddt: None,
                    psi: self.star[g].clone(),
                    non_orth_correction: self.non_orth_correction,
                };
                let eqn = assemble_precursor_matrix(&self.mesh, &inp)?;
                let perf = eqn.solve(
                    &self.mesh,
                    &mut self.star[g],
                    self.cfg.solver_tolerance,
                    self.cfg.solver_rel_tol,
                    self.cfg.solver_max_iter,
                    self.cfg.solver_min_iter,
                );
                solves.push((perf.iterations, perf.initial_residual, perf.final_residual));
            }
        }
        // liquidFuelBeta_ *= 0.0 ; precPK = star*alpha ; weightedPrec ; liquidFuelBeta_ +=
        self.liquid_fuel_beta = 0.0;
        for g in 0..groups {
            self.update_prec(g);
            let w = self.weighted_prec(g);
            self.liquid_fuel_beta += w * lambdas[g] * pgt / self.base.power_old;
        }
        if initialize {
            self.prec_equilibrium_reactivity = 0.0;
            self.prec_equilibrium_reactivity += self.base.beta;
            self.prec_equilibrium_reactivity -= self.liquid_fuel_beta;
            terms = self.base.reactivity_terms_with_prec(&values, &refs, t, self.prec_equilibrium_reactivity);
            self.base.total_reactivity = terms.total;
        }

        // the neutron iteration
        let mut neutro_iter = 0usize;
        let mut residuals = vec![];
        let mut intermediate = vec![];
        loop {
            let mut neutro_residual = 0.0_f64;
            let mut weighted = vec![0.0; groups];
            for g in 0..groups {
                // ddt(alpha, precStar): the old-time fields as the first ddt of the step sees them
                let alpha_old = self.old_time_alpha(time_index);
                if self.star_old_index != time_index {
                    self.star_old = self.star.clone();
                    self.star_old_index = time_index;
                }
                let inp = PrecursorEquationInputs {
                    alpha: self.alpha.clone(),
                    lambda: lambdas[g],
                    su: self.su_field(g, self.base.power),
                    gamma: diff.clone(),
                    phi: phi.clone(),
                    ddt: Some((dt, alpha_old, self.star_old[g].clone())),
                    psi: self.star[g].clone(),
                    non_orth_correction: self.non_orth_correction,
                };
                let eqn = assemble_precursor_matrix(&self.mesh, &inp)?;
                if !self.frozen {
                    let perf = eqn.solve(
                        &self.mesh,
                        &mut self.star[g],
                        self.cfg.solver_tolerance,
                        self.cfg.solver_rel_tol,
                        self.cfg.solver_max_iter,
                        self.cfg.solver_min_iter,
                    );
                    solves.push((perf.iterations, perf.initial_residual, perf.final_residual));
                    if perf.initial_residual > neutro_residual {
                        neutro_residual = perf.initial_residual;
                    }
                }
                self.update_prec(g);
                weighted[g] = self.weighted_prec(g);
            }
            let mut sum_prec_lambda = 0.0;
            for g in 0..groups {
                sum_prec_lambda += weighted[g] * lambdas[g];
            }
            self.base.power = (self.base.power_old / dt + sum_prec_lambda)
                / (1.0 / dt - (self.base.total_reactivity - self.base.beta) / pgt);
            intermediate.push(self.base.power);
            neutro_iter += 1;
            residuals.push(neutro_residual);
            if !(neutro_residual > self.cfg.neutron_iteration_residual && neutro_iter < self.cfg.max_neutron_iterations) {
                break;
            }
        }

        // re-scale fluxes and power densities
        let scale = self.base.power / self.base.power_old;
        for g in self.base.fluxes.iter_mut() {
            for x in g.iter_mut() {
                *x *= scale;
            }
        }
        for x in self.base.one_group_flux.iter_mut() {
            *x *= scale;
        }
        for x in self.base.power_density.iter_mut() {
            *x *= scale;
        }
        for x in self.base.secondary_power_density.iter_mut() {
            *x *= scale;
        }
        self.base.steps += 1;
        Ok(LiquidFuelStepReport {
            initialized: initialize,
            values,
            terms,
            neutron_iterations: neutro_iter,
            neutro_residuals: residuals,
            intermediate_powers: intermediate,
            solves,
            liquid_fuel_beta: self.liquid_fuel_beta,
            prec_equilibrium_reactivity: self.prec_equilibrium_reactivity,
        })
    }

    /// The matrix of the FIRST neutron iteration of the coming step (or, with `equilibrium`, the
    /// equilibrium matrix of the initialisation), exactly as `solvePointKineticsLiquidFuel.H` writes it
    /// from the current state, with the face flux and `diffCoeffPrec` it used. It performs the writes of
    /// the mapped `alpha` and the old-time snapshots the step itself would make first (both are
    /// idempotent within a time index), so it can be followed by [`Self::step`]. Used by the V&V tests to
    /// locate a difference in the matrix rather than in the solve.
    ///
    /// # Errors
    ///
    /// Field-length mismatches.
    pub fn first_iteration_matrix(
        &mut self,
        time_index: usize,
        dt: f64,
        fields: &PkFeedbackFields,
        fluid: &FluidFields,
        group: usize,
        equilibrium: bool,
    ) -> Result<(InterfaceMatrix, FaceFlux, Vec<f64>), LiquidFuelError> {
        let n = self.mesh.n_cells;
        self.write_alpha(&fluid.alpha, time_index);
        let diff: Vec<f64> = (0..n).map(|c| (fluid.alphat[c] + fluid.mu[c] / self.cfg.sc_no) / fields.rho_cool[c]).collect();
        let phi = self.mesh.flux_of_u_alpha(&fluid.u, &self.alpha)?;
        let ddt = if equilibrium {
            None
        } else {
            let alpha_old = self.old_time_alpha(time_index);
            if self.star_old_index != time_index {
                self.star_old = self.star.clone();
                self.star_old_index = time_index;
            }
            Some((dt, alpha_old, self.star_old[group].clone()))
        };
        let inp = PrecursorEquationInputs {
            alpha: self.alpha.clone(),
            lambda: self.base.cfg.lambdas[group],
            su: self.su_field(group, self.base.power),
            gamma: diff.clone(),
            phi: phi.clone(),
            ddt,
            psi: self.star[group].clone(),
            non_orth_correction: self.non_orth_correction,
        };
        let eqn = assemble_precursor_matrix(&self.mesh, &inp)?;
        Ok((eqn, phi, diff))
    }

    /// The solid-fuel base (power, reactivity, references, rescaled fields).
    pub fn base(&self) -> &PointKineticNeutronics {
        &self.base
    }

    /// `precPKStar_` of group `group`.
    pub fn prec_pk_star(&self, group: usize) -> &[f64] {
        &self.star[group]
    }

    /// `precPK_` of group `group`.
    pub fn prec_pk(&self, group: usize) -> &[f64] {
        &self.prec[group]
    }

    /// `maxDeltaT()` with the `controlDict` keys passed in (as the solid-fuel class).
    pub fn max_delta_t(&self, delta_t: f64, control_max_delta_t: Option<f64>, max_power_variation: Option<f64>) -> f64 {
        self.base.max_delta_t(delta_t, control_max_delta_t, max_power_variation)
    }

    /// `liquidFuelBeta_`.
    pub fn liquid_fuel_beta(&self) -> f64 {
        self.liquid_fuel_beta
    }

    /// `precEquilibriumReactivity_`.
    pub fn prec_equilibrium_reactivity(&self) -> f64 {
        self.prec_equilibrium_reactivity
    }

    /// `initOneGroupFluxN_`.
    pub fn init_one_group_flux_n(&self) -> &[f64] {
        &self.init_n
    }

    /// `domainIntegratedInitOneGroupFluxN_`.
    pub fn domain_integrated_init_n(&self) -> f64 {
        self.dom_int_n
    }

    /// The transport mesh.
    pub fn transport_mesh(&self) -> &TransportMesh {
        &self.mesh
    }
}
