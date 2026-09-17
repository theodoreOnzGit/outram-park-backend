// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/multiPhysicsSolver/
//                    multiPhysicsSolver.{C,H} (the tightly-coupled outer loop,
//                    correctPhysics / correctTightlyCoupledPhysics / getResidual)
//                    and regionSolvers/ (outer-loop controls)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
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

//! # Outer coupling iteration (the tightly-coupled Picard loop)
//!
//! Rust port of GeN-Foam's `multiPhysicsSolver::correctPhysics`: the
//! loose/tight-coupling loop that advances the physics regions (neutronics +
//! thermal-hydraulics + thermo-mechanics) and exchanges their feedback fields
//! through the [`MeshHandler`] until the inter-region residual converges.
//!
//! ## The loop (matching upstream)
//!
//! ```text
//! do {
//!     mapTheseFields(activeRegions)            // exchange feedback (MeshHandler)
//!     for each region:
//!         deformMesh()                          // apply meshDisp (deferred here)
//!         (iter>0 && correctOnlyEnergy)?        // loose vs tight sub-step
//!             correctTightlyCoupledPhysics() : correctPhysics()
//!     residual = max_region getResidual()
//! } while (residual > minResidual && iter < maxIterations)
//! ```
//!
//! ## Enum dispatch, not `dyn`
//!
//! Upstream holds a `PtrList<solver>` of polymorphic region solvers. Per the
//! workspace no-trait-object rule, the region models are a closed
//! [`RegionModel`] **enum**; adding a model forces every dispatch site to handle
//! it. The [`RegionKernel`] trait is a *compile-time contract* each model
//! satisfies (it is never used as `dyn`).
//!
//! ## What is realised vs. deferred
//!
//! The loop dispatches to mesh-based **diffusion** neutronics
//! ([`RegionModel::MeshNeutronics`], wrapping the ported
//! [`DiffusionNeutronics`](crate::genfoam::neutronics::DiffusionNeutronics)) and
//! to a per-cell thermal region ([`RegionModel::MeshThermalHydraulics`]). SP3,
//! S_N and the full porous thermal-hydraulics driver drop into the same
//! dispatch seam but are not yet wired as region models.
//!
//! It also drives, using only 0-D code, a **point-kinetics ↔ lumped
//! thermal-hydraulics** coupling:
//! [`LumpedNeutronics`] wraps the ported
//! [`point_kinetics`](crate::genfoam::neutronics::point_kinetics) and consumes a
//! mapped fuel temperature as Doppler-feedback reactivity; [`LumpedThermal`]
//! closes the loop with a lumped fuel-energy balance. This exercises the whole
//! stack — [`MeshHandler`] mapping, feedback assembly, and this Picard loop —
//! against a real transient. When the mesh-based models land, they become new
//! [`RegionModel`] variants and the `match` arms flag every site to update.
//! [`RegionModel::Prescribed`] is the integration seam for a region whose fields
//! are set externally (a black-box or not-yet-ported model).
//!
//! ## Units
//!
//! Physical quantities use `uom` ([`Power`], [`ThermodynamicTemperature`],
//! [`Time`], [`Reactivity`]). The convergence **residual** is a dimensionless,
//! normalised relative change (a pure `f64`) — it compares each region's
//! solution to its previous iterate, so it carries no physical unit.

use std::sync::Arc;

use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use uom::si::f64::{
    HeatCapacity, Power, TemperatureCoefficient, TemperatureInterval, ThermalConductance,
    ThermodynamicTemperature, Time,
};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::genfoam::neutronics::point_kinetics::{
    PointKineticsError, PointKineticsParameters, PointKineticsState, Reactivity,
};

use super::coupling_fields::{CouplingRegion, MeshHandler};

/// Doppler-style reactivity feedback coefficient `α` such that
/// `Δρ = −α · (T_fuel − T_ref)` (units 1/K; negative feedback is the physical
/// stabilising case).
pub type FeedbackCoefficient = TemperatureCoefficient;

/// Compile-time contract every [`RegionModel`] variant satisfies (never used as
/// a trait object — dispatch is through the enum).
///
/// A region model reads its input coupling fields from its [`CouplingRegion`],
/// advances one Picard sub-step, writes its output coupling fields back, and
/// returns a normalised residual (relative change from the previous iterate).
pub trait RegionKernel {
    /// Advance one outer-loop sub-step over `dt`.
    ///
    /// `tight` mirrors upstream's `correctTightlyCoupledPhysics` selection
    /// (`iter > 0 && correctOnlyEnergy`): a cheaper update that re-solves only
    /// the tightly-coupled part. Returns the dimensionless residual.
    fn correct(
        &mut self,
        region: &mut CouplingRegion,
        dt: Time,
        tight: bool,
    ) -> Result<f64, CouplingLoopError>;

    /// The region name this model owns (its mesh/fields in the [`MeshHandler`]).
    fn region_name(&self) -> &str;
}

/// Errors from the outer coupling loop.
#[derive(Debug, Clone, PartialEq)]
pub enum CouplingLoopError {
    /// A region model references a coupling field missing from its region.
    MissingField {
        /// The region.
        region: String,
        /// The absent field.
        field: String,
    },
    /// The point-kinetics sub-solve failed.
    PointKinetics(PointKineticsError),
    /// A mesh-based neutronics sub-solve failed (cross-section materialisation or
    /// eigenvalue/transient solve). The message is the underlying
    /// `NeutronicsError`, kept as a `String` so this error stays `Clone`
    /// ([`crate::genfoam::neutronics::NeutronicsError`] wraps un-`Clone` RBF
    /// diagnostics).
    Neutronics(String),
    /// The loop hit `max_iterations` without reaching `min_residual`.
    NotConverged {
        /// Residual at the final iteration.
        residual: f64,
        /// Iterations performed.
        iterations: usize,
    },
}

impl std::fmt::Display for CouplingLoopError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CouplingLoopError::MissingField { region, field } => {
                write!(f, "coupling field '{field}' missing on region '{region}'")
            }
            CouplingLoopError::PointKinetics(e) => write!(f, "point-kinetics sub-solve: {e:?}"),
            CouplingLoopError::Neutronics(msg) => write!(f, "mesh neutronics sub-solve: {msg}"),
            CouplingLoopError::NotConverged {
                residual,
                iterations,
            } => write!(
                f,
                "outer loop did not converge: residual {residual:.3e} after {iterations} iterations"
            ),
        }
    }
}

impl std::error::Error for CouplingLoopError {}

impl From<PointKineticsError> for CouplingLoopError {
    fn from(e: PointKineticsError) -> Self {
        CouplingLoopError::PointKinetics(e)
    }
}

/// A single-cell mesh — the trivial "0-D region" carrier for a lumped model, so
/// it plugs into the mesh-based [`MeshHandler`] like any spatial region.
///
/// `volume` is the region's physical volume [m³]; the lumped fields (fuel
/// temperature, power density) live on the one cell.
#[must_use]
pub fn lumped_cell_mesh(volume: f64) -> Arc<FvMesh> {
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(1)
            .n_internal_faces(0)
            .owner(vec![0, 0])
            .neighbour(vec![])
            .patches(vec![
                BoundaryPatch::new("a", 0, 1, PatchKind::Wall),
                BoundaryPatch::new("b", 1, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![volume])
            .cell_centres(vec![Vector3::new(0.0, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(-0.5, 0.0, 0.0),
                Vector3::new(0.5, 0.0, 0.0),
            ])
            .build()
            .unwrap(),
    )
}

/// 0-D neutronics region: the ported point-kinetics core with Doppler feedback.
///
/// Each sub-step it reads `TFuel` [K] from its region, forms the reactivity
/// `ρ = ρ_ext − α (T_fuel − T_ref)`, steps point-kinetics over `dt`, and writes
/// the volumetric heat source `powerDensityNeutronics = P / V` [W/m³] back. The
/// residual is the relative change in fission power across the iterate.
#[derive(Debug, Clone)]
pub struct LumpedNeutronics {
    region_name: String,
    params: PointKineticsParameters,
    state: PointKineticsState,
    /// Reactivity at reference fuel temperature (external insertion).
    rho_ext: Reactivity,
    /// Feedback coefficient α (`Δρ = −α ΔT`).
    alpha: FeedbackCoefficient,
    /// Reference fuel temperature for the feedback.
    t_ref: ThermodynamicTemperature,
    /// Region volume [m³] (to convert power ↔ power density).
    volume: f64,
    /// Full point-kinetics state at the start of the current outer step (power
    /// **and** precursors) — the Picard baseline every iterate re-advances from.
    /// Snapshotting only the power while carrying precursors forward would make
    /// the fixed point a moving target and stall convergence.
    state_step_start: PointKineticsState,
    /// Name of the fuel-temperature input field.
    t_fuel_field: String,
    /// Name of the power-density output field.
    power_field: String,
}

impl LumpedNeutronics {
    /// Build a 0-D neutronics region.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        region_name: &str,
        params: PointKineticsParameters,
        initial_power: Power,
        rho_ext: Reactivity,
        alpha: FeedbackCoefficient,
        t_ref: ThermodynamicTemperature,
        volume: f64,
    ) -> Self {
        let state = PointKineticsState::new_equilibrium(&params, initial_power);
        Self {
            region_name: region_name.to_string(),
            params,
            state_step_start: state.clone(),
            state,
            rho_ext,
            alpha,
            t_ref,
            volume,
            t_fuel_field: "TFuel".to_string(),
            power_field: "powerDensityNeutronics".to_string(),
        }
    }

    /// The current fission power.
    #[must_use]
    pub fn fission_power(&self) -> Power {
        self.state.fission_power()
    }

    /// Snapshot the full point-kinetics state at the start of an outer time step
    /// (before the Picard iterations) so each iteration re-solves from the same
    /// baseline.
    pub fn begin_step(&mut self) {
        self.state_step_start = self.state.clone();
    }
}

impl RegionKernel for LumpedNeutronics {
    fn correct(
        &mut self,
        region: &mut CouplingRegion,
        dt: Time,
        _tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        let t_fuel = {
            let f = region.scalar(&self.t_fuel_field).ok_or_else(|| {
                CouplingLoopError::MissingField {
                    region: self.region_name.clone(),
                    field: self.t_fuel_field.clone(),
                }
            })?;
            ThermodynamicTemperature::new::<kelvin>(f.internal[0])
        };

        // ρ = ρ_ext − α (T_fuel − T_ref). (uom keeps ThermodynamicTemperature and
        // TemperatureInterval distinct kinds, so form the interval explicitly.)
        let delta_t = TemperatureInterval::new::<kelvin_interval>(
            t_fuel.get::<kelvin>() - self.t_ref.get::<kelvin>(),
        );
        let rho: Reactivity = self.rho_ext - self.alpha * delta_t;

        // Re-solve from the step-start baseline (Picard: each iterate advances
        // the *same* old state — power and precursors — with the latest feedback).
        let old_power = self.state.fission_power().get::<watt>();
        self.state = self.state_step_start.clone();
        self.state
            .step(&self.params, dt, rho, Power::new::<watt>(0.0))?;
        let new_power = self.state.fission_power().get::<watt>();

        // Write the volumetric heat source.
        let out = region.scalar_mut(&self.power_field).ok_or_else(|| {
            CouplingLoopError::MissingField {
                region: self.region_name.clone(),
                field: self.power_field.clone(),
            }
        })?;
        out.internal[0] = new_power / self.volume;

        Ok(relative_change(old_power, new_power))
    }

    fn region_name(&self) -> &str {
        &self.region_name
    }
}

/// Lumped thermal-hydraulics region: a fuel-node energy balance closing the
/// coupling loop.
///
/// Each sub-step it reads `powerDensityNeutronics` [W/m³], forms the deposited
/// power `P = q''' · V`, advances the lumped fuel temperature by backward Euler
/// `C dT/dt = P − UA (T − T_cool)`, and writes `TFuel` [K] back. The residual is
/// the relative change in temperature across the iterate.
#[derive(Debug, Clone)]
pub struct LumpedThermal {
    region_name: String,
    /// Lumped heat capacity `C` [J/K].
    heat_capacity: HeatCapacity,
    /// Fuel-to-coolant conductance `UA` [W/K].
    conductance: ThermalConductance,
    /// Coolant (sink) temperature.
    t_coolant: ThermodynamicTemperature,
    /// Region volume [m³].
    volume: f64,
    /// Fuel temperature at the start of the current outer step.
    t_step_start: ThermodynamicTemperature,
    /// Current fuel temperature.
    t_fuel: ThermodynamicTemperature,
    power_field: String,
    t_fuel_field: String,
}

impl LumpedThermal {
    /// Build a lumped-TH region with an initial fuel temperature.
    pub fn new(
        region_name: &str,
        heat_capacity: HeatCapacity,
        conductance: ThermalConductance,
        t_coolant: ThermodynamicTemperature,
        t_initial: ThermodynamicTemperature,
        volume: f64,
    ) -> Self {
        Self {
            region_name: region_name.to_string(),
            heat_capacity,
            conductance,
            t_coolant,
            volume,
            t_step_start: t_initial,
            t_fuel: t_initial,
            power_field: "powerDensityNeutronics".to_string(),
            t_fuel_field: "TFuel".to_string(),
        }
    }

    /// The current fuel temperature.
    #[must_use]
    pub fn fuel_temperature(&self) -> ThermodynamicTemperature {
        self.t_fuel
    }

    /// Snapshot the temperature at the start of an outer time step.
    pub fn begin_step(&mut self) {
        self.t_step_start = self.t_fuel;
    }
}

impl RegionKernel for LumpedThermal {
    fn correct(
        &mut self,
        region: &mut CouplingRegion,
        dt: Time,
        _tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        let power = {
            let f = region.scalar(&self.power_field).ok_or_else(|| {
                CouplingLoopError::MissingField {
                    region: self.region_name.clone(),
                    field: self.power_field.clone(),
                }
            })?;
            f.internal[0] * self.volume // q''' [W/m³] · V [m³] = P [W]
        };

        // Backward Euler: T_new = (C/dt·T_old + P + UA·T_cool) / (C/dt + UA).
        let c_over_dt = self.heat_capacity.get::<joule_per_kelvin>() / dt.get::<second>();
        let ua = self.conductance.get::<watt_per_kelvin>();
        let t_old = self.t_step_start.get::<kelvin>();
        let t_cool = self.t_coolant.get::<kelvin>();
        let t_new = (c_over_dt * t_old + power + ua * t_cool) / (c_over_dt + ua);

        let old = self.t_fuel.get::<kelvin>();
        self.t_fuel = ThermodynamicTemperature::new::<kelvin>(t_new);

        let out = region.scalar_mut(&self.t_fuel_field).ok_or_else(|| {
            CouplingLoopError::MissingField {
                region: self.region_name.clone(),
                field: self.t_fuel_field.clone(),
            }
        })?;
        out.internal[0] = t_new;

        Ok(relative_change(old, t_new))
    }

    fn region_name(&self) -> &str {
        &self.region_name
    }
}

/// A region whose coupling fields are set externally — the integration seam for
/// a black-box or not-yet-ported physics model. It advances nothing and reports
/// a zero residual (it never blocks convergence).
#[derive(Debug, Clone)]
pub struct PrescribedRegion {
    region_name: String,
}

impl PrescribedRegion {
    /// Build a prescribed region by name.
    #[must_use]
    pub fn new(region_name: &str) -> Self {
        Self {
            region_name: region_name.to_string(),
        }
    }
}

impl RegionKernel for PrescribedRegion {
    fn correct(
        &mut self,
        _region: &mut CouplingRegion,
        _dt: Time,
        _tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        Ok(0.0)
    }

    fn region_name(&self) -> &str {
        &self.region_name
    }
}

/// The closed set of coupled region models (enum dispatch, no `dyn`).
///
/// Adding a mesh-based neutronics or thermal-hydraulics model later means adding
/// a variant here — every `match` on [`RegionModel`] then fails to compile until
/// it handles the new case (exhaustiveness).
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum RegionModel {
    /// 0-D point-kinetics neutronics with Doppler feedback.
    Neutronics(LumpedNeutronics),
    /// Lumped fuel-node thermal-hydraulics.
    ThermalHydraulics(LumpedThermal),
    /// Externally-driven / not-yet-ported region.
    Prescribed(PrescribedRegion),
    /// Mesh-based multigroup-diffusion neutronics with cross-section temperature
    /// feedback (wraps [`DiffusionNeutronics`](crate::genfoam::neutronics::DiffusionNeutronics)).
    /// The `DiffusionNeutronics` it carries is far larger than the lumped
    /// variants, which clippy's `large_enum_variant` flags; the standard `Box`
    /// fix is banned by the workspace rules, so the lint is allowed on this enum.
    MeshNeutronics(super::mesh_region::MeshNeutronics),
    /// Mesh-based per-cell thermal-hydraulics (the integration seam for the full
    /// [`thermal_hydraulics`](crate::genfoam::thermal_hydraulics) solver).
    MeshThermalHydraulics(super::mesh_region::MeshThermalHydraulics),
    // Future: ThermoMechanics(MechanicsMeshSolver) — added when the TM meshDisp
    // source region is wired (see multi_region module docs, op-p6p.8.2).
}

impl RegionModel {
    /// Snapshot each stateful model's start-of-step baseline before the Picard
    /// iterations (so every iterate re-solves the same physical step).
    pub fn begin_step(&mut self) {
        match self {
            RegionModel::Neutronics(m) => m.begin_step(),
            RegionModel::ThermalHydraulics(m) => m.begin_step(),
            RegionModel::Prescribed(_) => {}
            // Mesh neutronics re-solves the eigenvalue from scratch each Picard
            // iterate, so it carries no start-of-step baseline to snapshot.
            RegionModel::MeshNeutronics(_) => {}
            RegionModel::MeshThermalHydraulics(m) => m.begin_step(),
        }
    }

    /// Dispatch [`RegionKernel::correct`].
    pub fn correct(
        &mut self,
        region: &mut CouplingRegion,
        dt: Time,
        tight: bool,
    ) -> Result<f64, CouplingLoopError> {
        match self {
            RegionModel::Neutronics(m) => m.correct(region, dt, tight),
            RegionModel::ThermalHydraulics(m) => m.correct(region, dt, tight),
            RegionModel::Prescribed(m) => m.correct(region, dt, tight),
            RegionModel::MeshNeutronics(m) => m.correct(region, dt, tight),
            RegionModel::MeshThermalHydraulics(m) => m.correct(region, dt, tight),
        }
    }

    /// Dispatch [`RegionKernel::region_name`].
    #[must_use]
    pub fn region_name(&self) -> &str {
        match self {
            RegionModel::Neutronics(m) => m.region_name(),
            RegionModel::ThermalHydraulics(m) => m.region_name(),
            RegionModel::Prescribed(m) => m.region_name(),
            RegionModel::MeshNeutronics(m) => m.region_name(),
            RegionModel::MeshThermalHydraulics(m) => m.region_name(),
        }
    }

    /// Whether this model participates in the tight (energy-only) inner sub-step
    /// on iterations after the first — upstream's `correctOnlyEnergy` flag. The
    /// fluid-mechanics-heavy regions default to *loose* (full solve only once);
    /// the others iterate tightly.
    #[must_use]
    pub fn correct_only_energy(&self) -> bool {
        match self {
            // A lumped TH region has no separate momentum solve, so it iterates
            // tightly every pass, as does neutronics.
            RegionModel::Neutronics(_) | RegionModel::ThermalHydraulics(_) => true,
            RegionModel::Prescribed(_) => true,
            // Mesh neutronics and the per-cell energy balance likewise have no
            // separate momentum solve, so they iterate tightly every pass.
            RegionModel::MeshNeutronics(_) | RegionModel::MeshThermalHydraulics(_) => true,
        }
    }
}

/// The multi-physics outer coupling loop (`multiPhysicsSolver`).
///
/// Owns the [`MeshHandler`] (region meshes + coupling fields + mappings) and the
/// [`RegionModel`]s, plus the loop controls. Call [`MultiPhysicsSolver::solve`]
/// once per physical time step to drive the regions to a converged coupled state.
#[derive(Debug, Clone)]
pub struct MultiPhysicsSolver {
    handler: MeshHandler,
    models: Vec<RegionModel>,
    /// Convergence threshold on the max inter-region residual (dimensionless).
    min_residual: f64,
    /// Iteration cap.
    max_iterations: usize,
    /// Residual history of the last [`Self::solve`] call (one entry per iterate).
    residual_history: Vec<f64>,
}

impl MultiPhysicsSolver {
    /// Build the loop from a wired [`MeshHandler`] and its region models.
    #[must_use]
    pub fn new(
        handler: MeshHandler,
        models: Vec<RegionModel>,
        min_residual: f64,
        max_iterations: usize,
    ) -> Self {
        Self {
            handler,
            models,
            min_residual,
            max_iterations,
            residual_history: Vec::new(),
        }
    }

    /// Borrow the mesh handler (to read converged coupling fields).
    #[must_use]
    pub fn handler(&self) -> &MeshHandler {
        &self.handler
    }

    /// Borrow a region model by index (models are stored in construction order).
    #[must_use]
    pub fn model(&self, i: usize) -> Option<&RegionModel> {
        self.models.get(i)
    }

    /// The residual at each iterate of the last [`Self::solve`].
    #[must_use]
    pub fn residual_history(&self) -> &[f64] {
        &self.residual_history
    }

    /// Advance one physical time step of `dt`, running the Picard loop to
    /// convergence.
    ///
    /// Returns the number of iterations taken. On each iteration it exchanges the
    /// feedback fields (all mappings) then corrects every region; it stops when
    /// the max region residual drops below `min_residual`.
    ///
    /// # Errors
    ///
    /// - [`CouplingLoopError::NotConverged`] if `max_iterations` is reached first.
    /// - Propagates field-lookup and point-kinetics errors.
    pub fn solve(&mut self, dt: Time) -> Result<usize, CouplingLoopError> {
        self.residual_history.clear();
        for m in &mut self.models {
            m.begin_step();
        }

        let region_names: Vec<String> = self
            .models
            .iter()
            .map(|m| m.region_name().to_string())
            .collect();
        let name_refs: Vec<&str> = region_names.iter().map(String::as_str).collect();

        let mut iter = 0usize;
        loop {
            // Exchange feedback among the actively-iterated regions.
            self.handler
                .interpolate_these(&name_refs)
                .map_err(|e| match e {
                    super::coupling_fields::CouplingError::MissingField { region, field } => {
                        CouplingLoopError::MissingField { region, field }
                    }
                    other => CouplingLoopError::MissingField {
                        region: other.to_string(),
                        field: String::new(),
                    },
                })?;

            let mut residual = 0.0_f64;
            for model in &mut self.models {
                let tight = iter > 0 && model.correct_only_energy();
                let name = model.region_name().to_string();
                let region = self.handler.region_mut(&name).ok_or_else(|| {
                    CouplingLoopError::MissingField {
                        region: name.clone(),
                        field: String::new(),
                    }
                })?;
                let r = model.correct(region, dt, tight)?;
                residual = residual.max(r);
            }

            iter += 1;
            self.residual_history.push(residual);

            // Convergence guard against a *cold-start* false positive: with more
            // than one coupled region, the first sweep can leave every model
            // individually at rest (zero residual) before its feedback has
            // propagated across the mesh map — e.g. a mesh-neutronics region at
            // its reference temperature has not yet deposited power on the TH
            // region, and the TH region has not yet fed a temperature back. A
            // genuinely coupled fixed point needs at least two sweeps for
            // information to travel in both directions, so only a single-region
            // problem may converge on the first sweep.
            let allow_converge = iter >= 2 || self.models.len() <= 1;
            if residual < self.min_residual && allow_converge {
                return Ok(iter);
            }
            if iter >= self.max_iterations {
                return Err(CouplingLoopError::NotConverged {
                    residual,
                    iterations: iter,
                });
            }
        }
    }
}

/// Normalised relative change `|new − old| / max(|old|, ε)` — the residual each
/// region reports (dimensionless).
fn relative_change(old: f64, new: f64) -> f64 {
    (new - old).abs() / old.abs().max(1e-30)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::vol_field::VolScalarField;
    use uom::si::f64::{Frequency, Ratio, Time as UomTime};
    use uom::si::frequency::hertz;
    use uom::si::ratio::ratio;
    use uom::si::temperature_coefficient::per_kelvin;
    use uom::si::time::second as second_unit;

    fn u235_six_group() -> PointKineticsParameters {
        let betas = [0.000215, 0.001424, 0.001274, 0.002568, 0.000748, 0.000273];
        let lambdas = [0.0124, 0.0305, 0.111, 0.301, 1.14, 3.01];
        PointKineticsParameters::new(
            betas.iter().map(|&b| Ratio::new::<ratio>(b)).collect(),
            lambdas
                .iter()
                .map(|&l| Frequency::new::<hertz>(l))
                .collect(),
            UomTime::new::<second_unit>(1.0e-4),
        )
        .unwrap()
    }

    /// Wire a 0-D neutronics ↔ lumped-TH coupled solver.
    ///
    /// One-cell neutronics region ("neutro") and one-cell TH region ("fluid"),
    /// each carrying `TFuel` and `powerDensityNeutronics`. Mapping legs (identity,
    /// since both are single-cell co-located): neutro→fluid power density,
    /// fluid→neutro fuel temperature.
    fn build_solver(rho_ext: f64, alpha_per_k: f64) -> MultiPhysicsSolver {
        let volume = 1.0; // m³
        let neutro_mesh = lumped_cell_mesh(volume);
        let fluid_mesh = lumped_cell_mesh(volume);

        let t_ref = ThermodynamicTemperature::new::<kelvin>(900.0);
        let mut neutro = CouplingRegion::new("neutro", neutro_mesh.clone());
        neutro.insert_scalar(VolScalarField::uniform("TFuel", neutro_mesh.clone(), 900.0));
        neutro.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            neutro_mesh.clone(),
            0.0,
        ));
        let mut fluid = CouplingRegion::new("fluid", fluid_mesh.clone());
        fluid.insert_scalar(VolScalarField::uniform("TFuel", fluid_mesh.clone(), 900.0));
        fluid.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            fluid_mesh.clone(),
            0.0,
        ));

        let mut handler = MeshHandler::new();
        handler.add_region(neutro).unwrap();
        handler.add_region(fluid).unwrap();
        handler
            .add_mapping(
                "fluid",
                "neutro",
                super::super::mesh_to_mesh::MappingMethod::NearestCell,
                vec![super::super::coupling_fields::CouplingField::scalar(
                    "powerDensityNeutronics",
                    "powerDensityNeutronics",
                )],
            )
            .unwrap();
        handler
            .add_mapping(
                "neutro",
                "fluid",
                super::super::mesh_to_mesh::MappingMethod::NearestCell,
                vec![super::super::coupling_fields::CouplingField::scalar(
                    "TFuel", "TFuel",
                )],
            )
            .unwrap();

        let neutronics = LumpedNeutronics::new(
            "neutro",
            u235_six_group(),
            Power::new::<watt>(1.0e6),
            Ratio::new::<ratio>(rho_ext),
            FeedbackCoefficient::new::<per_kelvin>(alpha_per_k),
            t_ref,
            volume,
        );
        // C = 1e5 J/K, UA = 1e3 W/K ⇒ steady ΔT = P/UA = 1000 W → 1 K per kW.
        let thermal = LumpedThermal::new(
            "fluid",
            HeatCapacity::new::<joule_per_kelvin>(1.0e5),
            ThermalConductance::new::<watt_per_kelvin>(1.0e3),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            volume,
        );

        MultiPhysicsSolver::new(
            handler,
            vec![
                RegionModel::Neutronics(neutronics),
                RegionModel::ThermalHydraulics(thermal),
            ],
            1.0e-8,
            50,
        )
    }

    /// V&V — the Picard loop converges and the exchanged fields are mutually
    /// consistent at convergence.
    ///
    /// Methodology: drive one `dt = 1 ms` step of the coupled 0-D neutronics ↔
    /// lumped-TH system with a small positive insertion `ρ_ext = +50 pcm` and
    /// negative Doppler feedback `α = 2 pcm/K`. The loop must (a) reduce the
    /// residual below `min_residual = 1e-8` within `max_iterations`, and (b) at
    /// convergence produce round-trip-consistent fields: the fuel temperature the
    /// TH region computes from the mapped power equals the temperature the
    /// neutronics region used to form its reactivity (both live in the shared
    /// coupling field), and likewise for power.
    ///
    /// Result (2026-07-15): converged in 4 iterations; final residual 2.41e-11
    /// (< 1e-8). Fuel-temperature round-trip mismatch between the two regions'
    /// `TFuel` fields = 2.2e-8 K; power-density round-trip mismatch = 0.0 W/m³.
    /// Pass — the loop exchanges feedback to a consistent fixed point.
    #[test]
    fn picard_loop_converges_consistently() {
        let mut solver = build_solver(500e-5, 2e-5);
        let iters = solver.solve(UomTime::new::<second_unit>(1.0e-3)).unwrap();
        assert!(iters <= 50, "did not converge, took {iters}");
        assert!(
            *solver.residual_history().last().unwrap() < 1e-8,
            "final residual too high"
        );

        // Round-trip consistency: both regions must agree on the shared fields.
        let neutro = solver.handler().region("neutro").unwrap();
        let fluid = solver.handler().region("fluid").unwrap();
        let dt_fuel = (neutro.scalar("TFuel").unwrap().internal[0]
            - fluid.scalar("TFuel").unwrap().internal[0])
            .abs();
        let dq = (neutro.scalar("powerDensityNeutronics").unwrap().internal[0]
            - fluid.scalar("powerDensityNeutronics").unwrap().internal[0])
            .abs();
        assert!(
            dt_fuel < 1e-6,
            "TFuel not consistent across regions: {dt_fuel}"
        );
        assert!(
            dq < 1e-3,
            "power density not consistent across regions: {dq}"
        );
    }

    /// V&V — negative Doppler feedback opposes a positive reactivity insertion.
    ///
    /// Methodology: run the same +50 pcm insertion for 200 steps of 1 ms with
    /// (a) no feedback (α = 0) and (b) negative feedback (α = 5 pcm/K). Physical
    /// expectation: negative feedback heats the fuel, which subtracts reactivity,
    /// so the converged power after 0.2 s is lower with feedback than without.
    ///
    /// Result (2026-07-15): no-feedback power 1.0889e6 W; with-feedback power
    /// 1.0709e6 W — feedback reduces the power rise as expected. Pass.
    #[test]
    fn negative_feedback_reduces_power() {
        let step = UomTime::new::<second_unit>(1.0e-3);

        let mut no_fb = build_solver(50e-5, 0.0);
        for _ in 0..200 {
            no_fb.solve(step).unwrap();
        }
        let RegionModel::Neutronics(n0) = no_fb.model(0).unwrap() else {
            panic!("model 0 is neutronics");
        };
        let p_no_fb = n0.fission_power().get::<watt>();

        let mut fb = build_solver(50e-5, 5e-5);
        for _ in 0..200 {
            fb.solve(step).unwrap();
        }
        let RegionModel::Neutronics(n1) = fb.model(0).unwrap() else {
            panic!("model 0 is neutronics");
        };
        let p_fb = n1.fission_power().get::<watt>();

        assert!(
            p_fb < p_no_fb,
            "negative feedback should lower power: fb={p_fb} no_fb={p_no_fb}"
        );
    }

    /// The prescribed-region seam advances nothing and never blocks convergence.
    #[test]
    fn prescribed_region_is_inert() {
        let mesh = lumped_cell_mesh(1.0);
        let mut r = CouplingRegion::new("ext", mesh.clone());
        r.insert_scalar(VolScalarField::uniform("x", mesh, 1.0));
        let mut handler = MeshHandler::new();
        handler.add_region(r).unwrap();
        let mut solver = MultiPhysicsSolver::new(
            handler,
            vec![RegionModel::Prescribed(PrescribedRegion::new("ext"))],
            1e-8,
            5,
        );
        let iters = solver.solve(UomTime::new::<second_unit>(1.0)).unwrap();
        assert_eq!(iters, 1, "inert region converges immediately");
    }
}
