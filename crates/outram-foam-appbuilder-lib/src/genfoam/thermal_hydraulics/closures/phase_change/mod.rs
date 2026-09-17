// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/phaseChangeModels/
//             (saturationModels/, latentHeatModels/, heatDriven/, forcedConstant/)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream authors: Stefan Radman (EPFL) — constantTemperature/water saturation,
//             water/FinkLeibowitz/fromThermophysicalProperties latent heat,
//             heatDriven/forcedConstant phase change; Gauthier Lazare, Carlo
//             Fiorina (waterTRACE saturation)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `closures::phase_change` — evaporation/condensation source models
//!
//! Rust port of GeN-Foam's `physicsModels/phaseChangeModels/**`: the saturation
//! models (constant temperature, water, waterTRACE, Browning-Potter),
//! latent-heat models (Fink-Leibowitz, water, from-thermophysical-properties),
//! and the phase-change mass-transfer-rate models (heat-driven, forced
//! constant). Together these supply the interfacial mass source term `dmdt`
//! (kg / (m^3 s)) that the two-phase solver's continuity and energy equations
//! consume. Belongs here: the phase-change rate and its saturation/latent-heat
//! inputs, as pure algebra in temperature/pressure/enthalpy. Does **not**
//! belong here: the interfacial heat-transfer coefficients and area density
//! that produce the heat fluxes consumed by [`rate::PhaseChangeRateModel`]
//! (that is `super::heat_transfer`, out of scope for this module — see
//! "Deferred" below), and the enthalpy-consistency (`adjust`) correction that
//! reads the two phases' live thermodynamic state (out of scope: no sibling
//! thermo dependency per this port's mandate).
//!
//! ## Model set (three closed enums, no `dyn` dispatch)
//!
//! | Family | Type | Variants | Upstream |
//! |---|---|---|---|
//! | Saturation | [`SaturationModel`] | `ConstantTemperature`, `Water`, `WaterTrace`, `BrowningPotter` | `saturationModels/{constantTemperature,water,waterTRACE,BrowningPotter}` |
//! | Latent heat | [`LatentHeatModel`] | `FinkLeibowitz`, `Water`, `FromThermophysicalProperties` | `latentHeatModels/{FinkLeibowitz,water,fromThermophysicalProperties}` |
//! | Phase-change rate | [`PhaseChangeRateModel`] | `HeatDrivenConductionLimited`, `HeatDrivenTwoPhaseDriven`, `HeatDrivenOnePhaseDriven`, `HeatDrivenMixedDriven`, `ForcedConstant` | `heatDriven/`, `forcedConstant/` |
//!
//! Each family lives in its own file: [`saturation`], [`latent_heat`], [`rate`].
//! [`tests`] holds the cross-family verification & validation suite.
//!
//! ## Local `uom` aliases
//!
//! GeN-Foam's phase-change classes pass bare `Foam::scalar`s between the
//! saturation model, the latent-heat model, and the mass-transfer-rate model.
//! This module gives the recurring quantities named, dimension-checked `uom`
//! types. `uom` 0.38 has no built-in named quantity for the phase-change rate
//! or the saturation-pressure/temperature slope's dimensions, so both are
//! *composed* from existing `uom` quantities via `core::ops::Div`'s associated
//! `Output` type — `uom`'s `si` system implements cross-dimension arithmetic
//! generically (`Quantity<Dl,..> / Quantity<Dr,..> -> Quantity<Dl-Dr,..>`), so
//! this is fully dimension-checked, just without a named unit table:
//!
//! - [`LatentHeat`] — alias for [`uom::si::f64::AvailableEnergy`] (J/kg).
//! - [`InterfacialHeatFlux`] — alias for
//!   [`uom::si::f64::VolumetricPowerDensity`] (W/m^3), the volumetric
//!   interfacial heat flux `iA * htc * (T - T_interface)` that drives
//!   [`rate::PhaseChangeRateModel::HeatDrivenConductionLimited`] and friends.
//! - [`PhaseChangeRate`] — composed as `InterfacialHeatFlux / LatentHeat`;
//!   dimension L^-3 M T^-1, i.e. **kg / (m^3 s)**, GeN-Foam's `dmdt`.
//! - [`SaturationPressureSlope`] — composed as `Pressure / ThermodynamicTemperature`;
//!   **Pa / K**, GeN-Foam's `dPsat/dT` (`valuePSatPrime`).
//!
//! These four aliases arguably belong in the shared `units.rs` module once a
//! second closure family needs them (see the port report for op-p6p.7.7).

use core::ops::Div;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{AvailableEnergy, Pressure, ThermodynamicTemperature, VolumetricPowerDensity};
use uom::si::volumetric_power_density::watt_per_cubic_meter;

pub mod latent_heat;
pub mod rate;
pub mod saturation;

#[cfg(test)]
mod tests;

pub use latent_heat::LatentHeatModel;
pub use rate::PhaseChangeRateModel;
pub use saturation::SaturationModel;

/// Specific latent heat of vaporization `h_fg` — **base SI: J / kg**.
///
/// The energy absorbed per unit mass converting liquid to vapour at
/// saturation. Always physically positive; the sign convention for which
/// phase is gaining/losing mass (GeN-Foam's `LSign_`) is a property of the
/// fluid-fluid pair, not of the correlation, and is applied by the caller
/// (see the [`latent_heat`] module doc). Aliased to
/// [`uom`]'s [`AvailableEnergy`](uom::si::f64::AvailableEnergy).
pub type LatentHeat = AvailableEnergy;

/// Volumetric interfacial heat flux `q = i_A * htc * (T - T_interface)` —
/// **base SI: W / m^3**.
///
/// The rate of heat conducted from a phase's bulk to the fluid-fluid
/// interface per unit mixture volume, i.e. interfacial area density
/// (`1/m`) times heat-transfer coefficient (`W/(m^2 K)`) times the
/// bulk-to-interface temperature difference. [`rate::PhaseChangeRateModel`]'s
/// heat-driven variants take this pre-multiplied quantity as input rather
/// than the interfacial HTC and area separately, so this module stays
/// independent of `super::heat_transfer`. Aliased to [`uom`]'s
/// [`VolumetricPowerDensity`](uom::si::f64::VolumetricPowerDensity).
pub type InterfacialHeatFlux = VolumetricPowerDensity;

/// Volumetric phase-change (evaporation/condensation) mass-transfer rate —
/// **base SI: kg / (m^3 s)**.
///
/// GeN-Foam's `dmdt`: positive means mass transferring from fluid1 to fluid2
/// per the pair's `LSign_` convention. Composed as
/// [`InterfacialHeatFlux`] / [`LatentHeat`] (dimension L^-3 M T^-1); build one
/// directly from a bare `kg/(m^3 s)` magnitude with [`phase_change_rate`], and
/// read one back with [`phase_change_rate_value`].
pub type PhaseChangeRate = <InterfacialHeatFlux as Div<LatentHeat>>::Output;

/// Saturation-curve slope `dP_sat/dT` — **base SI: Pa / K**.
///
/// GeN-Foam's `valuePSatPrime`. Composed as `Pressure / ThermodynamicTemperature`
/// (dimension M L^-1 T^-2 Theta^-1); see [`saturation::SaturationModel::p_sat_prime`].
pub type SaturationPressureSlope = <Pressure as Div<ThermodynamicTemperature>>::Output;

/// Build a [`PhaseChangeRate`] from a bare magnitude in kg / (m^3 s).
///
/// `uom` has no named unit table for this composed dimension, so the value is
/// carried through as `InterfacialHeatFlux(x W/m^3) / LatentHeat(1 J/kg)`:
/// dividing by the base-unit magnitude `1.0` leaves `x` unchanged (both
/// `watt_per_cubic_meter` and `joule_per_kilogram` are `uom`'s coefficient-1
/// base units for their quantities), so the resulting `.value` is exactly `x`
/// in kg / (m^3 s).
#[inline]
#[must_use]
pub fn phase_change_rate(value_kg_per_m3_per_s: f64) -> PhaseChangeRate {
    InterfacialHeatFlux::new::<watt_per_cubic_meter>(value_kg_per_m3_per_s)
        / LatentHeat::new::<joule_per_kilogram>(1.0)
}

/// Read a [`PhaseChangeRate`] back as a bare `f64` in kg / (m^3 s).
///
/// Valid because `PhaseChangeRate`'s composed dimension has no `uom` unit
/// table, so its public `value` field already holds the SI base-unit
/// magnitude (kg / (m^3 s) has coefficient 1 against `uom`'s SI base).
#[inline]
#[must_use]
pub fn phase_change_rate_value(rate: PhaseChangeRate) -> f64 {
    rate.value
}
