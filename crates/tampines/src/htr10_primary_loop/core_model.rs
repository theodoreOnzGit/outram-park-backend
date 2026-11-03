// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.
//! The core plug-in interface of the HTR-10 primary loop (gh:#1151).
//!
//! The loop sees the reactor pressure vessel as a black box between two
//! nozzles: the **cold nozzle** (RPV inlet, where the coaxial duct annulus
//! delivers circulator helium) and the **hot nozzle** (RPV outlet, the
//! hot-gas-duct inner tube). Everything inside the RPV — downcomer, risers,
//! bed, bypass paths, plena, reflectors, RCCS — belongs to the core model.
//!
//! What crosses the interface, per step:
//!
//! - **in:** the loop mass flow (signed: positive in the forced direction,
//!   cold nozzle to hot nozzle), the temperature of the gas *arriving* at
//!   each nozzle, the system pressure, and the total thermal power the
//!   caller deposits ([`CoreBoundary`], `power`): a fixed or user-set heat
//!   source; the kinetics that may set it lives in `nee_soon`;
//! - **out:** the temperature of the gas *leaving* at the downstream
//!   nozzle, the heat given to the coolant and to the RCCS, and the
//!   power-weighted fuel / moderator / reflector temperatures a caller's
//!   kinetics may feed back on ([`CoreResponse`], [`FeedbackTemperatures`]);
//! - **for the loop momentum balance:** the core's flow resistance at a
//!   trial flow ([`CoreModel::pressure_drop`]) and its gravity head
//!   ([`CoreModel::gravity_head`]).
//!
//! [`CoreModel`] is an enum (no trait objects, workspace rule). Part A
//! carried one variant, the lumped stand-in; part B (gh:#1151) adds the
//! R-Z porous core ([`RzPorousCore`]) behind the same five methods.
//!
//! > Research, education and V&V only. AI draft, unreviewed.

use uom::si::f64::{MassRate, Power, Pressure, ThermodynamicTemperature, Time};

use super::lumped_core::LumpedPebbleBedCore;
use super::rz_core::RzPorousCore;
use crate::TampinesError;

/// Power-weighted fuel and moderator temperatures and the reflector
/// temperature of a core: what a point-kinetics feedback reads. (Moved
/// here from the kinetics module, 2026-11-03, when the kinetics moved to
/// `nee_soon`: the temperatures are thermal-hydraulic outputs.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeedbackTemperatures {
    /// Power-weighted fuel temperature.
    pub fuel: ThermodynamicTemperature,
    /// Power-weighted moderator (pebble graphite) temperature.
    pub moderator: ThermodynamicTemperature,
    /// Reflector temperature.
    pub reflector: ThermodynamicTemperature,
}

/// The loop-side state the core is advanced against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreBoundary {
    /// Loop mass flow; positive in the forced direction (cold nozzle in,
    /// hot nozzle out), negative for reversed natural circulation.
    pub mass_flow: MassRate,
    /// Gas arriving at the cold nozzle (used when `mass_flow > 0`).
    pub cold_nozzle_temperature: ThermodynamicTemperature,
    /// Gas arriving at the hot nozzle (used when `mass_flow < 0`).
    pub hot_nozzle_temperature: ThermodynamicTemperature,
    /// System pressure.
    pub pressure: Pressure,
}

/// What the core returns after a step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoreResponse {
    /// Gas leaving at the downstream nozzle (the hot nozzle for forward
    /// flow; the cold nozzle for reversed flow). Equal to the old value when
    /// there is no flow.
    pub outlet_temperature: ThermodynamicTemperature,
    /// Net heat taken up by the coolant between the nozzles.
    pub power_to_coolant: Power,
    /// Heat leaving the RPV to the RCCS.
    pub heat_to_rccs: Power,
    /// Temperatures for the reactivity feedback.
    pub feedback: FeedbackTemperatures,
}

/// A core model the loop can carry.
///
/// Not `PartialEq` since gh:#1151 part B: the R-Z variant holds a solver
/// state with no meaningful equality.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // one long-lived core per run; boxing is barred (workspace rule)
pub enum CoreModel {
    /// The lumped stand-in of part A ([`LumpedPebbleBedCore`]).
    LumpedStandIn(LumpedPebbleBedCore),
    /// The R-Z porous core of part B ([`RzPorousCore`]).
    RzPorous(RzPorousCore),
}

impl CoreModel {
    /// Advance by `dt` with `power` deposited.
    ///
    /// # Errors
    /// Propagates the model's property and solver errors.
    pub fn advance(
        &mut self,
        dt: Time,
        power: Power,
        boundary: &CoreBoundary,
    ) -> Result<CoreResponse, TampinesError> {
        match self {
            Self::LumpedStandIn(c) => c.advance(Some(dt), power, boundary),
            Self::RzPorous(c) => c.advance(dt, power, boundary),
        }
    }

    /// Put the core in steady state at `power` and `boundary`.
    ///
    /// # Errors
    /// As [`Self::advance`], and if the steady iteration does not converge.
    pub fn steady_state(
        &mut self,
        power: Power,
        boundary: &CoreBoundary,
    ) -> Result<CoreResponse, TampinesError> {
        match self {
            Self::LumpedStandIn(c) => c.steady_state(power, boundary),
            Self::RzPorous(c) => c.steady_state(power, boundary),
        }
    }

    /// Current feedback temperatures.
    pub fn feedback_temperatures(&self) -> FeedbackTemperatures {
        match self {
            Self::LumpedStandIn(c) => c.feedback_temperatures(),
            Self::RzPorous(c) => c.feedback_temperatures(),
        }
    }

    /// Pressure loss between the nozzles at the trial flow `mass_flow`
    /// (signed like the flow: it opposes it), at the current temperatures.
    ///
    /// # Errors
    /// Property or correlation errors.
    pub fn pressure_drop(&self, mass_flow: MassRate, pressure: Pressure) -> Result<Pressure, TampinesError> {
        match self {
            Self::LumpedStandIn(c) => c.pressure_drop(mass_flow, pressure),
            Self::RzPorous(c) => Ok(c.pressure_drop(mass_flow)),
        }
    }

    /// Gravity head between the nozzles, `-g sum rho dz` along the forced
    /// direction (positive drives forward flow).
    ///
    /// # Errors
    /// Property errors.
    pub fn gravity_head(&self, pressure: Pressure) -> Result<Pressure, TampinesError> {
        match self {
            Self::LumpedStandIn(c) => c.gravity_head(pressure),
            Self::RzPorous(c) => Ok(c.gravity_head()),
        }
    }

    /// `sum L / A` of the flow path between the nozzles, 1/m.
    pub fn inertance_per_m(&self) -> f64 {
        match self {
            Self::LumpedStandIn(c) => c.inertance_per_m(),
            Self::RzPorous(c) => c.inertance_per_m(),
        }
    }
}
