// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/**
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::structure` — mesh-unresolved solid structure
//!
//! The **solid side of the porous two-field formulation**: the fuel pins,
//! cladding, and grid hardware that occupy the complementary volume of each
//! porous cell but are not resolved by the mesh. This module owns the
//! structure's *thermal* state (surface temperature, stored heat) and its power
//! source, and it exchanges heat with the fluid phase through the per-cell
//! convective closure. It does **not** own the fluid-phase field state (that is
//! the [`phase`](super::phase) module) nor the correlation leaves (those are
//! [`closures`](super::closures)).
//!
//! Ported upstream:
//! `src/classes/thermalHydraulics/src/phaseModels/structureModels/**`.
//!
//! ## Module map
//!
//! | Submodule | Ports | What it provides |
//! |---|---|---|
//! | [`units`] | — | Named `uom` aliases: [`PowerDensity`], [`VolumetricHeatCapacity`], [`InterfacialAreaDensity`], [`WallConductance`] (+ re-exported [`HeatTransferCoefficient`], [`HeatFlux`]) |
//! | [`power_model`] | `powerModel/{fixedPower,fixedTemperature}` | [`PowerModel`] enum — the structure's internal power source and its lumped surface-temperature update |
//! | [`heat_source`] | `structure.C` heat terms | Explicit fluid ⟷ structure source, wall heat flux, the inert [`PassiveStructure`] |
//! | [`pump`] | `pump` | [`Pump`] momentum source with time scaling |
//! | [`heat_exchanger`] | `heatExchanger` | [`HeatExchanger`] two-sided wall-temperature solve |
//! | [`power_off`] | `powerOffCriterionModels/{timer,fieldValue}` | [`PowerOffCriterion`] SCRAM criteria |
//!
//! [`StructureCell`] (below) is the small aggregate that wires a power model,
//! an optional passive sub-structure, an optional pump, and an optional
//! power-off criterion into a single per-cell object and advances them together
//! — the direct, self-contained analogue of one cell's worth of
//! `structure::correct` + `structure::explicitHeatSource`.
//!
//! ## Design boundary — what belongs to the porous-solver bead
//!
//! Everything here is **per-cell / mesh-free algebra plus owned lumped state**.
//! The genuinely mesh-coupled machinery of GeN-Foam's `structure` — the
//! anisotropic tortuosity / local-frame rotation tensors, the mesh-to-mesh
//! heat-exchanger mapping, the semi-implicit `fvm::Sp` enthalpy-source matrix,
//! and the assembly of these per-cell quantities into the porous `UEqn`/`EEqn`
//! — is deferred to the porous one-phase solver bead (op-p6p.7.11), mirroring
//! how [`closures::fs_drag`](super::closures::fs_drag) ports the friction
//! correlations but leaves the drag-tensor assembly to the solver. The
//! higher-fidelity radial-conduction pin/pebble models and the FMU couplings are
//! separate follow-ups (see [`power_model`]).

pub mod heat_exchanger;
pub mod heat_source;
pub mod power_model;
pub mod power_off;
pub mod pump;
pub mod units;

pub use heat_exchanger::{hx_wall_temperature, HeatExchanger};
pub use heat_source::{explicit_heat_source, wall_heat_flux, PassiveStructure};
pub use power_model::{FixedPower, FixedTemperature, PowerModel};
pub use power_off::{FieldReduction, PowerOffCriterion, ThresholdDirection};
pub use pump::Pump;
pub use units::{
    HeatFlux, HeatTransferCoefficient, InterfacialAreaDensity, PowerDensity,
    VolumetricHeatCapacity, WallConductance,
};

use outram_foam_basic_lib::prelude::Vector3;
use uom::si::f64::{ThermodynamicTemperature, Time};

/// The thermal state produced by advancing a [`StructureCell`] one timestep.
///
/// All quantities are for the single porous cell that the [`StructureCell`]
/// represents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StructureThermalState {
    /// Active (powered) structure surface temperature `T_act` [K].
    pub active_surface_temperature: ThermodynamicTemperature,
    /// Passive sub-structure surface temperature `T_pas` [K], if a passive
    /// sub-structure is present.
    pub passive_surface_temperature: Option<ThermodynamicTemperature>,
    /// Wall temperature `T_wall` [K] = the active surface temperature (matching
    /// GeN-Foam's `Twall = Tact`).
    pub wall_temperature: ThermodynamicTemperature,
    /// Total explicit heat source exchanged with the fluid [W/m^3] — the sum of
    /// the active and passive convective terms, in GeN-Foam's
    /// `a_v*(ht_sum - h_sum*T_struct)` convention. Its magnitude is the power
    /// per unit volume crossing the wall.
    pub heat_source_to_fluid: PowerDensity,
    /// Diagnostic wall heat flux `q''` [W/m^2] (positive = structure to fluid).
    pub wall_heat_flux: HeatFlux,
    /// Whether the power-off (SCRAM) criterion has fired.
    pub powered_off: bool,
}

/// One porous cell's worth of mesh-unresolved solid structure.
///
/// Aggregates the structure's active [`PowerModel`], an optional inert
/// [`PassiveStructure`], an optional [`Pump`] momentum source, and an optional
/// [`PowerOffCriterion`] SCRAM trigger, and advances them together with
/// [`StructureCell::correct`] — the per-cell analogue of `structure::correct`
/// followed by `structure::explicitHeatSource`.
///
/// This is deliberately lumped and mesh-free: it is the unit that is directly
/// hand-verifiable (energy balance) and that a lumped or operator-split
/// integrator can drive. The full field-wide assembly over a real mesh is the
/// porous-solver bead's responsibility (see the [module documentation](self)).
#[derive(Debug, Clone, PartialEq)]
pub struct StructureCell {
    /// The active power model (fission / internal source + surface temperature).
    pub power_model: PowerModel,
    /// Optional inert sub-structure sharing the cell.
    pub passive: Option<PassiveStructure>,
    /// Optional pump momentum source over this cell.
    pub pump: Option<Pump>,
    /// Optional power-off (SCRAM) criterion.
    pub power_off: Option<PowerOffCriterion>,
    powered_off: bool,
}

impl StructureCell {
    /// Build a structure cell from just an active power model. Attach the
    /// optional parts with the `with_*` builders.
    #[must_use]
    pub fn new(power_model: PowerModel) -> Self {
        Self {
            power_model,
            passive: None,
            pump: None,
            power_off: None,
            powered_off: false,
        }
    }

    /// Attach an inert passive sub-structure.
    #[must_use]
    pub fn with_passive(mut self, passive: PassiveStructure) -> Self {
        self.passive = Some(passive);
        self
    }

    /// Attach a pump momentum source.
    #[must_use]
    pub fn with_pump(mut self, pump: Pump) -> Self {
        self.pump = Some(pump);
        self
    }

    /// Attach a power-off (SCRAM) criterion.
    #[must_use]
    pub fn with_power_off(mut self, criterion: PowerOffCriterion) -> Self {
        self.power_off = Some(criterion);
        self
    }

    /// This cell's pump momentum source [N/m^3] at time `t` (zero if no pump).
    #[must_use]
    pub fn momentum_source(&self, t: Time) -> Vector3 {
        self.pump
            .as_ref()
            .map_or(Vector3::ZERO, |p| p.momentum_source(t))
    }

    /// Advance the structure one timestep and return its thermal state.
    ///
    /// Order of operations (matching `structure::correct`):
    /// 1. Evaluate the power-off criterion; if it fires, zero the power model's
    ///    internal source (once).
    /// 2. Update the active surface temperature (lumped energy balance).
    /// 3. Update the passive surface temperature, if present.
    /// 4. Form the explicit heat source to the fluid = active + passive
    ///    convective terms, the wall temperature (= active surface), and the
    ///    diagnostic wall heat flux.
    ///
    /// # Parameters
    /// - `h_sum`/`ht_sum` — this cell's fluid-coupling sums (see
    ///   [`heat_source`]).
    /// - `dt` — the timestep; `t` — the new (end-of-step) time.
    /// - `scram_field_extremum` — the reduced (max/min) value of the field the
    ///   [`PowerOffCriterion::FieldValue`] criterion monitors, in that field's
    ///   own units. Ignored by a timer criterion; pass any value (e.g. the
    ///   current peak structure temperature) when unused.
    pub fn correct(
        &mut self,
        h_sum: HeatTransferCoefficient,
        ht_sum: HeatFlux,
        dt: Time,
        t: Time,
        scram_field_extremum: f64,
    ) -> StructureThermalState {
        // 1. Power-off / SCRAM check.
        if !self.powered_off {
            if let Some(criterion) = self.power_off.as_mut() {
                if criterion.check(t, scram_field_extremum) {
                    self.power_model.power_off();
                    self.powered_off = true;
                }
            }
        }

        // 2. Active structure surface temperature.
        let t_act = self.power_model.correct(h_sum, ht_sum, dt, t);
        let active_source =
            explicit_heat_source(self.power_model.interfacial_area(), h_sum, ht_sum, t_act);

        // 3. Passive sub-structure.
        let (t_pas, passive_source) = match self.passive.as_mut() {
            Some(p) => {
                let tp = p.correct(h_sum, ht_sum, dt);
                let src = explicit_heat_source(p.interfacial_area(), h_sum, ht_sum, tp);
                (Some(tp), src)
            }
            None => (None, PowerDensity::default()),
        };

        // 4. Aggregate.
        StructureThermalState {
            active_surface_temperature: t_act,
            passive_surface_temperature: t_pas,
            wall_temperature: t_act,
            heat_source_to_fluid: active_source + passive_source,
            wall_heat_flux: wall_heat_flux(h_sum, ht_sum, t_act),
            powered_off: self.powered_off,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Ratio;
    use uom::si::heat_flux_density::watt_per_square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::ratio::ratio;
    use uom::si::reciprocal_length::reciprocal_meter;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::time::second;
    use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
    use uom::si::volumetric_power_density::watt_per_cubic_meter;

    /// V&V — integrated steady-state fuel-pin energy balance and heat closure.
    ///
    /// Methodology: a single porous cell with a `fixedPower` active structure is
    /// marched to steady state against a fixed single-phase fluid coupling
    /// (`h_sum`, `ht_sum = h_sum*T_fluid`). Two conservation checks:
    ///
    /// 1. **Structure balance** — the active surface temperature must satisfy
    ///    `T_act = T_fluid + alpha*q''' / (a_v*h_sum)`.
    /// 2. **Global energy balance** — at steady state all deposited power leaves
    ///    through the wall, so the returned `heat_source_to_fluid` magnitude must
    ///    equal `alpha*q'''`.
    ///
    /// Inputs: alpha = 0.5, q''' = 4.0e7 W/m^3, a_v = 100 1/m,
    /// h_sum = 1.0e4 W/(m^2 K), T_fluid = 600 K.
    /// Hand calc: dT = 0.5*4.0e7/(100*1.0e4) = 20 K ⇒ T_act = 620 K; deposited
    /// power alpha*q''' = 2.0e7 W/m^3 = wall exchange a_v*h_sum*(T_act - T_fluid)
    /// = 100*1e4*20 = 2.0e7 W/m^3.
    ///
    /// Result (2026-07-15): T_act = 620.000 K (|err| < 1e-6 K); wall exchange
    /// balances the source to relative < 1e-7. Pass.
    #[test]
    fn integrated_steady_state_energy_balance() {
        let alpha = 0.5;
        let q = 4.0e7;
        let a_v = 100.0;
        let h = 1.0e4;
        let t_fluid = 600.0;

        let pm = PowerModel::FixedPower(FixedPower::new(
            Ratio::new::<ratio>(alpha),
            InterfacialAreaDensity::new::<reciprocal_meter>(a_v),
            PowerDensity::new::<watt_per_cubic_meter>(q),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(3.85e6),
            ThermodynamicTemperature::new::<kelvin>(t_fluid),
        ));
        let mut cell = StructureCell::new(pm);

        let h_sum = HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(h);
        let ht_sum = HeatFlux::new::<watt_per_square_meter>(h * t_fluid);
        let dt = Time::new::<second>(1.0e9);

        let mut state = cell.correct(h_sum, ht_sum, dt, Time::new::<second>(1.0), 0.0);
        for i in 2..=6 {
            state = cell.correct(h_sum, ht_sum, dt, Time::new::<second>(i as f64), 0.0);
        }

        assert!(
            (state.active_surface_temperature.get::<kelvin>() - 620.0).abs() < 1e-6,
            "T_act = {} K",
            state.active_surface_temperature.get::<kelvin>()
        );

        let source_mag = state
            .heat_source_to_fluid
            .get::<watt_per_cubic_meter>()
            .abs();
        assert!(
            (source_mag - alpha * q).abs() / (alpha * q) < 1e-7,
            "steady wall exchange = {} W/m^3, expected {} W/m^3",
            source_mag,
            alpha * q
        );
    }

    /// V&V — SCRAM zeroes the power and the structure cools to the fluid.
    ///
    /// Methodology: after reaching steady state at 620 K, a timer SCRAM at
    /// t = 100 s zeroes the internal power; continuing to march (large dt) must
    /// relax the active surface temperature back to the fluid temperature
    /// (600 K), since with no source the steady balance is `T = T_fluid`.
    ///
    /// Result (2026-07-15): after SCRAM the surface relaxes to 600.000 K
    /// (|err| < 1e-6 K) and `powered_off` is set. Pass.
    #[test]
    fn scram_relaxes_structure_to_fluid() {
        let pm = PowerModel::FixedPower(FixedPower::new(
            Ratio::new::<ratio>(0.5),
            InterfacialAreaDensity::new::<reciprocal_meter>(100.0),
            PowerDensity::new::<watt_per_cubic_meter>(4.0e7),
            VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(3.85e6),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        ));
        let mut cell = StructureCell::new(pm)
            .with_power_off(PowerOffCriterion::timer(Time::new::<second>(100.0)));

        let h_sum = HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(1.0e4);
        let ht_sum = HeatFlux::new::<watt_per_square_meter>(1.0e4 * 600.0);
        let big = Time::new::<second>(1.0e9);

        for i in 1..=5 {
            cell.correct(h_sum, ht_sum, big, Time::new::<second>(i as f64), 0.0);
        }
        let mut state = cell.correct(h_sum, ht_sum, big, Time::new::<second>(150.0), 0.0);
        for _ in 0..4 {
            state = cell.correct(h_sum, ht_sum, big, Time::new::<second>(200.0), 0.0);
        }
        assert!(state.powered_off);
        assert!(
            (state.active_surface_temperature.get::<kelvin>() - 600.0).abs() < 1e-6,
            "post-SCRAM T_act = {} K",
            state.active_surface_temperature.get::<kelvin>()
        );
    }

    #[test]
    fn pump_momentum_source_passthrough() {
        let pm = PowerModel::FixedTemperature(FixedTemperature::new(
            InterfacialAreaDensity::new::<reciprocal_meter>(1.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        ));
        let cell = StructureCell::new(pm).with_pump(Pump::new(Vector3::new(0.0, 0.0, 500.0)));
        assert_eq!(cell.momentum_source(Time::new::<second>(0.0)).z, 500.0);

        let no_pump = StructureCell::new(PowerModel::FixedTemperature(FixedTemperature::new(
            InterfacialAreaDensity::new::<reciprocal_meter>(1.0),
            ThermodynamicTemperature::new::<kelvin>(600.0),
        )));
        assert_eq!(
            no_pump.momentum_source(Time::new::<second>(0.0)),
            Vector3::ZERO
        );
    }
}
