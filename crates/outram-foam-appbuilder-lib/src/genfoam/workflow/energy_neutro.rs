// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: GeN-Foam/GeN-Foam.C (time loop: mapAllFields, correctPhysics per region),
//                    src/classes/multiRegion/meshHandler/meshHandler.C (mapAllFields),
//                    src/classes/multiRegion/multiPhysicsSolver (correctPhysics),
//                    GeN-Foam/include/setDeltaT.C,
//                    Tutorials/reactorCases/2D_MSFR/Allrun (steadyStateEN, transient)
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

//! The circulating-fuel diffusion + energy-only `onePhase` pair, as `2D_MSFR` runs it (gh:#1814).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! One object, [`EnergyNeutroDiffusion`], holds the two regions, the identical-mesh `cellVolumeWeight` map
//! and the mapped fields, and runs both stages of the chain on them:
//!
//! * [`EnergyNeutroDiffusion::run_eigenvalue`], the `steadyStateEN` stage: `regionSolvers { fluidRegion
//!   onePhase; neutroRegion diffusionNeutronics; }`, `eigenvalueNeutronics true`, fixed step. Each step of
//!   `GeN-Foam.C`: `runTime++`, `mapAllFields` ONCE, then each region's `correctPhysics` once, in
//!   `regionSolvers` order (fluid, then neutronics; no coupling iteration, no `multiPhysicsSolver`);
//! * [`EnergyNeutroTransient`], the `transient` stage: `energyNeutroLoop multiPhysicsSolver`,
//!   `eigenvalueNeutronics false` (`keff_` frozen), adjustable step (round 9 of the gh1361 record).
//!
//! `U` and `alpha` (the fluid fraction `1 - structure`) are frozen (`solveFluidMechanics false`), so the
//! precursor transport `phi = fvc::flux(U alpha)` is built once.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use core::cell::RefCell;
use alloc::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use super::{ReactorState, WorkflowError};
use crate::genfoam::multi_region::coupled_loop::{
    adjust_delta_t, correct_physics_loop, CouplingDict, IdenticalMeshMap,
};
use crate::genfoam::neutronics::albedo::AlbedoLinearisation;
use crate::genfoam::neutronics::diffusion::upstream_transient::{
    diffusion_max_delta_t, UpstreamTransientControls, UpstreamTransientFeedback,
};
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings, PrecursorTransport};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{OnePhaseCcReport, OnePhaseCcSolver};

/// How the workflow's neutronics solves its linear systems (gh:#1890, round 43).
///
/// Upstream reads the entries of the case's `system/neutroRegion/fvSolution`
/// ([`NeutronLinear::FvSolution`], the default of [`super::allrun::AllrunEdits`]); the earlier form handed the
/// flux solver an explicit tolerance and kept it as the named option [`NeutronLinear::Explicit`].
#[derive(Debug, Clone, Copy)]
pub enum NeutronLinear {
    /// The `solvers` entries of the neutronics `fvSolution` (`io::nuclear_data::read_neutron_fv_solution`).
    FvSolution(crate::genfoam::common::linear_solvers::NeutronSolverEntries),
    /// `PCG`/`DIC`, `PBiCG`/`DILU` and `PBiCGStab`/`DILU` at one explicit `tolerance` and `maxIter`, `relTol 0`.
    Explicit(SolverSettings),
}

impl NeutronLinear {
    /// The diffusion settings carrying this choice.
    #[must_use]
    pub fn diffusion_settings(self) -> DiffusionSettings {
        match self {
            Self::FvSolution(entries) => DiffusionSettings::default().with_fv_solution(entries),
            Self::Explicit(linear) => DiffusionSettings { linear, ..DiffusionSettings::default() },
        }
    }
}

/// `phi = fvc::flux(U alpha)` on the neutronics mesh as `precEq.H` sees it, wedge patches included
/// ([`crate::genfoam::neutronics::diffusion::precursor_drift::upstream_precursor_transport`], gh:#1891: a wedge face
/// carries the flux of `transform(faceT, U) alpha`, a few 1e-14, not 0).
/// `u` is one velocity per cell, `alpha` the fluid fraction, `diffusivity` is overwritten each step.
#[must_use]
pub fn precursor_transport_from_velocity(mesh: &Arc<FvMesh>, u: &[Vector3], alpha: &[f64]) -> PrecursorTransport {
    crate::genfoam::neutronics::diffusion::precursor_drift::upstream_precursor_transport(mesh, u, alpha)
}

/// The form before gh:#1891 (round 43): the wedge faces carry exactly zero flux. Kept as the named option.
#[must_use]
pub fn precursor_transport_from_velocity_zero_wedge_flux(mesh: &Arc<FvMesh>, u: &[Vector3], alpha: &[f64]) -> PrecursorTransport {
    crate::genfoam::neutronics::diffusion::precursor_drift::precursor_transport_zero_wedge_flux(mesh, u, alpha)
}

/// The `controlDict` entries of the `steadyStateEN` stage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EigenvalueRunControls {
    /// `startTime` (15 in `2D_MSFR`).
    pub start_time: f64,
    /// `endTime` (`endTimeEN`, 165 in `2D_MSFR`; the Allrun's `adjustTimeStep false`).
    pub end_time: f64,
    /// `deltaT` (0.25 in `2D_MSFR`), fixed.
    pub delta_t: f64,
}

/// One step of the `steadyStateEN` stage, as the upstream log prints it.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyEigenvalueRecord {
    /// `runTime.value()` after the step.
    pub time: f64,
    /// `deltaT` of the step.
    pub delta_t: f64,
    /// `keff_ previous`.
    pub k_eff_previous: f64,
    /// `keff_` after `calcKeff.H`.
    pub k_eff: f64,
    /// `power:` (before `normFluxes.H`).
    pub power: f64,
    /// `neutroResidual`.
    pub neutro_residual: f64,
    /// Initial residual of each of the fluid's `h` solves.
    pub h_residuals: Vec<f64>,
    /// `T (avg min max)` after each outer iteration.
    pub t_stats: Vec<(f64, f64, f64)>,
}

/// The two regions and everything the coupling needs. See the module documentation.
#[derive(Debug, Clone)]
pub struct EnergyNeutroDiffusion {
    /// The `onePhase` region (energy only, frozen flow).
    pub fluid: OnePhaseCcSolver,
    /// The `diffusionNeutronics` region, circulating fuel.
    pub neutro: DiffusionNeutronics,
    xs: CrossSectionData,
    zone_map: Vec<usize>,
    map: IdenticalMeshMap,
    controls: UpstreamTransientControls,
    mu: Vec<f64>,
    alphat: Vec<f64>,
    /// `powerDensity` of the neutronics region: what the next `mapAllFields` hands the fluid.
    pub neutro_power_density: Vec<f64>,
    neutro_t: Vec<f64>,
    neutro_rho: Vec<f64>,
    /// `reactorState`: `keff` (mirrored from the neutronics) and `pTarget`.
    pub reactor: ReactorState,
}

impl EnergyNeutroDiffusion {
    /// Build both regions at their start state.
    ///
    /// The neutronics starts as upstream's constructor leaves it when `0/neutroRegion` holds only
    /// `defaultFlux`/`defaultPrec` (`initial_flux`, `initial_prec` uniform), `powerDensity` zero, `keff_` and
    /// `pTarget_` from `reactor`. `albedo` lists the `albedoSP3` patches of `defaultFlux`
    /// (patch name, `gamma`). The fluid's `U` and fluid fraction give the precursor transport.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] if the two meshes differ (the general `cellVolumeWeight` is gh:#651) or an
    /// albedo patch is unknown; the neutronics construction errors.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        fluid: OnePhaseCcSolver,
        mesh: Arc<FvMesh>,
        xs: CrossSectionData,
        zone_map: Vec<usize>,
        albedo: &[(String, f64)],
        controls: UpstreamTransientControls,
        linear: NeutronLinear,
        reactor: ReactorState,
        initial_flux: f64,
        initial_prec: f64,
    ) -> Result<Self, WorkflowError> {
        let n = mesh.n_cells;
        let groups = xs.energy_groups();
        let precs = xs.prec_groups();
        let map = IdenticalMeshMap::new(&fluid.case().mesh, &mesh).map_err(WorkflowError::Case)?;
        let t0: Vec<f64> = fluid.temperature().to_vec();
        let rho0 = fluid.thermo_density();
        let params0: Vec<f64> = t0.iter().zip(&rho0).flat_map(|(&t, &r)| [t, r]).collect();
        let flux_boundary: Vec<BoundaryCondition<f64>> = mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
        let settings = linear.diffusion_settings();
        let mut neutro =
            DiffusionNeutronics::new_with_cell_parameters(mesh.clone(), &xs, &zone_map, &params0, &flux_boundary, settings)?;
        for (name, gamma) in albedo {
            let idx = mesh
                .patches
                .iter()
                .position(|p| &p.name == name)
                .ok_or_else(|| WorkflowError::Case(format!("albedo patch `{name}` is not on the neutronics mesh")))?;
            neutro.set_albedo_boundary(idx, *gamma, AlbedoLinearisation::CellValue);
        }
        let alpha: Vec<f64> = fluid.case().alpha_structure.iter().map(|s| 1.0 - s).collect();
        neutro.set_precursor_drift(precursor_transport_from_velocity(&mesh, &fluid.case().u, &alpha));
        neutro.set_initial_guess(&vec![vec![initial_flux; n]; groups], reactor.k_eff);
        neutro.set_prec_star(&vec![vec![initial_prec; n]; precs])?;
        // `mu` and `alphat` (the thermo's `alphah`) of the stage are constants of the Boussinesq package; another package varies with T
        let (mu, alphat) = match &fluid.case().thermo {
            crate::genfoam::thermal_hydraulics::solver::two_phase::PhaseThermo::BoussinesqHConst(th) => (vec![th.mu; n], vec![th.alpha_h(); n]),
            _ => return Err(WorkflowError::Case("the EN stage maps constant `mu` and `alphat`: only the Boussinesq/hConst/const thermo is ported here".to_string())),
        };
        Ok(Self {
            fluid,
            neutro,
            xs,
            zone_map,
            map,
            controls,
            mu,
            alphat,
            neutro_power_density: vec![0.0; n],
            neutro_t: t0,
            neutro_rho: rho0,
            reactor,
        })
    }

    /// `meshHandler::mapAllFields`, in `mappings` order: `fluidRegion <- neutroRegion`
    /// (`powerDensity -> powerDensityNeutronicsToLiquid`), then `neutroRegion <- fluidRegion`
    /// (`T -> TFuel`, `thermo:rho -> rhoCool`). `U`, `alpha`, `mu` and `alphat` are constants of the stage.
    pub fn map_all_fields(&mut self) {
        let mut pd = self.fluid.case().power_density.clone();
        self.map.map(&self.neutro_power_density, &mut pd);
        self.fluid.set_power_density(&pd);
        let t = self.fluid.temperature().to_vec();
        let rho = self.fluid.thermo_density();
        self.map.map(&t, &mut self.neutro_t);
        self.map.map(&rho, &mut self.neutro_rho);
    }

    fn cell_parameters(&self) -> Vec<f64> {
        self.neutro_t.iter().zip(&self.neutro_rho).flat_map(|(&t, &r)| [t, r]).collect()
    }

    /// One step of the `steadyStateEN` stage of length `dt`; `first_step` is `timeIndex - startTimeIndex == 1`.
    ///
    /// # Errors
    ///
    /// The neutronics step's error.
    pub fn eigenvalue_step(&mut self, dt: f64, first_step: bool) -> Result<EnergyEigenvalueRecord, WorkflowError> {
        self.fluid.begin_time_step(dt);
        self.map_all_fields();
        let (_residual, report) = self.fluid.correct_physics();
        let params = self.cell_parameters();
        let step = self.neutro.upstream_eigenvalue_step(
            &self.xs,
            &self.zone_map,
            UpstreamTransientFeedback {
                cell_parameters: &params,
                rho_cool: &self.neutro_rho,
                alphat: &self.alphat,
                mu: &self.mu,
            },
            self.reactor.p_target,
            first_step,
            &self.controls,
        )?;
        self.neutro_power_density = step.power_density.clone();
        self.reactor.k_eff = step.k_eff;
        Ok(EnergyEigenvalueRecord {
            time: self.fluid.time(),
            delta_t: dt,
            k_eff_previous: step.k_eff_previous,
            k_eff: step.k_eff,
            power: step.power,
            neutro_residual: step.neutro_residual,
            h_residuals: report.h_residuals,
            t_stats: report.t_stats,
        })
    }

    /// The whole `steadyStateEN` stage: `run()`/`operator++` as `Time` does them
    /// (`value < endTime - 0.5 deltaT`, `value += deltaT`). `on_step` sees each record.
    ///
    /// # Errors
    ///
    /// The first step error.
    pub fn run_eigenvalue<F: FnMut(&EnergyEigenvalueRecord, &Self)>(
        &mut self,
        controls: &EigenvalueRunControls,
        mut on_step: F,
    ) -> Result<Vec<EnergyEigenvalueRecord>, WorkflowError> {
        let mut records = Vec::new();
        let mut value = controls.start_time;
        let mut index = 0usize;
        while value < controls.end_time - 0.5 * controls.delta_t {
            value += controls.delta_t;
            index += 1;
            let rec = self.eigenvalue_step(controls.delta_t, index == 1)?;
            on_step(&rec, self);
            records.push(rec);
        }
        Ok(records)
    }
}

/// The `controlDict` / `multiRegionCouplingDict` entries of the `transient` stage.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientSettings {
    /// `energyNeutroLoop` of `multiRegionCouplingDict`.
    pub coupling: CouplingDict,
    /// `deltaT` of `controlDict` (the first step, and the `maxDeltaT` guard of the neutronics).
    pub control_delta_t: f64,
    /// `maxDeltaT` of `controlDict`.
    pub max_delta_t: f64,
    /// `maxPowerVariation` of `controlDict` (default 0.025).
    pub max_power_variation: f64,
}

/// What one transient step reports.
#[derive(Debug, Clone, PartialEq)]
pub struct TransientRecord {
    /// `runTime.value()` after the step.
    pub time: f64,
    /// `deltaT` of this step.
    pub delta_t: f64,
    /// `deltaT` after `adjustDeltaT` (the next step's).
    pub next_delta_t: f64,
    /// `power:`.
    pub power: f64,
    /// `pTotOld_`.
    pub power_old: f64,
    /// Iterations of the multiphysics loop and whether it converged.
    pub coupling_iterations: usize,
    /// See above.
    pub coupling_converged: bool,
    /// Neutron iterations of the step.
    pub neutron_iterations: usize,
    /// The fluid's `h` residuals and `T (avg min max)` of the last `correctPhysics`.
    pub fluid_report: OnePhaseCcReport,
}

/// The `transient` stage on a [`EnergyNeutroDiffusion`] whose `keff_` is frozen.
#[derive(Debug, Clone)]
pub struct EnergyNeutroTransient {
    /// The regions.
    pub inner: EnergyNeutroDiffusion,
    settings: TransientSettings,
    dt: f64,
    p_old_reference: f64,
    first_step: bool,
    per_call_old_time: bool,
    lower_bound_residual: bool,
}

impl EnergyNeutroTransient {
    /// Start the transient from a normalised state. `pTotOld_` of the first step is the power with the
    /// flux of the state and the `sigmaPow` of the cross-section REFERENCE state (a restarted run builds a
    /// fresh `xs_` object; gh:#1773-1775).
    ///
    /// # Errors
    ///
    /// The cross-section error of the reference-state power.
    pub fn new(inner: EnergyNeutroDiffusion, settings: TransientSettings) -> Result<Self, WorkflowError> {
        let p_old_reference = inner.neutro.upstream_power_at_reference_state(&inner.xs, &inner.zone_map)?;
        let dt = settings.control_delta_t;
        Ok(Self { inner, settings, dt, p_old_reference, first_step: true, per_call_old_time: false, lower_bound_residual: false })
    }

    /// ABLATION, the form before gh:#1832: every coupling iteration snapshots the neutronics' old-time flux and
    /// `precStar` from the last iterate instead of the previous time step. Wrong whenever the coupling loop
    /// runs more than once in a step; kept as a named option to measure what the fix carries.
    #[must_use]
    pub fn with_per_call_old_time_snapshot(mut self) -> Self {
        self.per_call_old_time = true;
        self
    }

    /// DEPRECATED (gh:#1813): use the round 9 stand-in for the neutronics `residual_` (`max` of the first-iteration group
    /// residuals and the whole `neutroResidual` history, larger than upstream's value when the neutron loop iterates more
    /// than once) instead of upstream's form (`fluxEq.H:29` second-solve residuals and the final `neutroResidual`).
    #[must_use]
    pub fn with_lower_bound_neutro_residual(mut self) -> Self {
        self.lower_bound_residual = true;
        self
    }

    /// The step the next [`Self::step`] takes.
    #[must_use]
    pub fn delta_t(&self) -> f64 {
        self.dt
    }

    /// One time step: `runTime++`, `mapAllFields`, `correctPhysics` of the `multiPhysicsSolver`,
    /// `adjustDeltaT`.
    ///
    /// # Errors
    ///
    /// The neutronics step's error.
    pub fn step(&mut self) -> Result<TransientRecord, WorkflowError> {
        let dt = self.dt;
        self.inner.fluid.begin_time_step(dt);
        // `runTime++`: OpenFOAM stores the old-time flux and `precStar` once per time step, so the later
        // iterations of an unconverged coupling loop integrate from them too (gh:#1832)
        if self.per_call_old_time {
            self.inner.neutro.clear_old_times();
        } else {
            self.inner.neutro.store_old_times();
        }
        let only_energy = self.settings.coupling.correct_only_energy();
        let (min_residual, max_iterations) = (self.settings.coupling.min_residual, self.settings.coupling.max_iterations);
        struct Work {
            fluid_residual: f64,
            neutro_residual: f64,
            p_tot: f64,
            p_tot_old: f64,
            neutron_iterations: usize,
            report: Option<OnePhaseCcReport>,
            error: Option<WorkflowError>,
            first_step: bool,
        }
        let work = RefCell::new(Work {
            fluid_residual: 0.0,
            neutro_residual: 0.0,
            p_tot: 0.0,
            p_tot_old: 0.0,
            neutron_iterations: 0,
            report: None,
            error: None,
            first_step: self.first_step,
        });
        let p_old_reference = self.p_old_reference;
        let lower_bound_residual = self.lower_bound_residual;
        let this = RefCell::new(&mut self.inner);
        let outcome = correct_physics_loop(
            min_residual,
            max_iterations,
            &only_energy,
            || this.borrow_mut().map_all_fields(),
            |i, tight| {
                let mut guard = this.borrow_mut();
                let c = &mut **guard;
                let mut w = work.borrow_mut();
                if i == 0 {
                    if tight {
                        w.fluid_residual = c.fluid.correct_tightly_coupled_physics();
                    } else {
                        let (r, rep) = c.fluid.correct_physics();
                        w.fluid_residual = r;
                        w.report = Some(rep);
                    }
                } else {
                    let params = c.cell_parameters();
                    let res = c.neutro.upstream_transient_step(
                        &c.xs,
                        &c.zone_map,
                        UpstreamTransientFeedback {
                            cell_parameters: &params,
                            rho_cool: &c.neutro_rho,
                            alphat: &c.alphat,
                            mu: &c.mu,
                        },
                        dt,
                        &c.controls,
                    );
                    match res {
                        Ok(rep) => {
                            w.p_tot = rep.power;
                            w.p_tot_old = if w.first_step { p_old_reference } else { rep.power_old };
                            w.first_step = false;
                            w.neutron_iterations = rep.neutron_iterations;
                            w.neutro_residual =
                                if lower_bound_residual { rep.residual_lower_bound_form } else { rep.residual };
                            c.neutro_power_density = c.neutro.upstream_power_density();
                        }
                        Err(e) => {
                            w.error = Some(e.into());
                            w.neutro_residual = 0.0;
                        }
                    }
                }
            },
            |i| {
                let w = work.borrow();
                if i == 0 { w.fluid_residual } else { w.neutro_residual }
            },
        );
        let w = work.into_inner();
        if let Some(e) = w.error {
            return Err(e);
        }
        self.first_step = w.first_step;
        let fluid_max = self.inner.fluid.max_delta_t();
        let time = self.inner.fluid.time();
        let neutro_max = diffusion_max_delta_t(
            time,
            self.settings.control_delta_t,
            self.settings.max_delta_t,
            self.settings.max_power_variation,
            dt,
            w.p_tot,
            w.p_tot_old,
        );
        if let Some(new_dt) = adjust_delta_t(true, &[fluid_max, neutro_max], f64::MIN_POSITIVE, dt) {
            self.dt = new_dt;
        }
        Ok(TransientRecord {
            time,
            delta_t: dt,
            next_delta_t: self.dt,
            power: w.p_tot,
            power_old: w.p_tot_old,
            coupling_iterations: outcome.iterations,
            coupling_converged: outcome.converged,
            neutron_iterations: w.neutron_iterations,
            fluid_report: w.report.ok_or_else(|| WorkflowError::Case("the fluid region did not run".into()))?,
        })
    }
}

/// The `albedoSP3` patches of a flux file (`0/neutroRegion/defaultFlux`): `(patch name, gamma)`, in file order.
///
/// # Errors
///
/// [`WorkflowError::Case`] if the file cannot be read or has no `boundaryField`.
#[cfg(feature = "std")]
pub fn read_albedo_patches(flux_file: &std::path::Path) -> Result<Vec<(String, f64)>, WorkflowError> {
    use outram_foam_basic_lib::io::dict::FoamFile;
    use outram_foam_basic_lib::io::FoamEntry;
    let f = FoamFile::read(flux_file).map_err(|e| WorkflowError::Case(format!("{}: {e}", flux_file.display())))?;
    let bf = f.dict.get_dict("boundaryField").ok_or_else(|| WorkflowError::Case("no boundaryField".into()))?;
    let mut out = Vec::new();
    for (name, entry) in bf.iter() {
        let FoamEntry::SubDict(d) = entry else { continue };
        let is_albedo = matches!(d.get("type"), Some(FoamEntry::Word(w)) if w == "albedoSP3");
        if let (true, Some(FoamEntry::Scalar(g))) = (is_albedo, d.get("gamma")) {
            out.push((name.to_string(), *g));
        }
    }
    Ok(out)
}
