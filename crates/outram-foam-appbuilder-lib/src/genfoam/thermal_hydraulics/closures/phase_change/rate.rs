// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/phaseChangeModels/
//             {heatDriven,forcedConstant}/*PhaseChange.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Phase-change mass-transfer-rate models: `dmdt` as a function of
//! interfacial heat fluxes (or a prescribed constant).
//!
//! Port of GeN-Foam's `phaseChangeModel::correctInterfacialDmdt()` family
//! (`heatDrivenPhaseChange`, `forcedConstantPhaseChange`). Upstream computes
//! the two per-phase interfacial heat fluxes as
//! `q1 = i_A * htc1 * (T1 - T_interface)` and
//! `q2 = i_A * htc2 * (T2 - T_interface)`, then evaluates one of several mode
//! formulas on `q1/L` and `q2/L`. This port takes `q1`/`q2` pre-multiplied
//! (see [`super::InterfacialHeatFlux`]) so it stays independent of the
//! interfacial-heat-transfer-coefficient and interfacial-area-density
//! closures (`super::heat_transfer`) — the caller assembles `q1`/`q2` and
//! supplies the [`super::LatentHeat`] from [`super::LatentHeatModel`].
//!
//! **Deferred** (solver-level bookkeeping, out of scope for a pure closure —
//! see the `phase_change` module docs): `phaseChangeModel::correct()`'s
//! wall-boiling split (`dmdtW_`), the adaptive mass-transfer limiter
//! (`limitMassTransfer`), interfacial-area flooring (`limitInterfacialArea`),
//! interfacial-temperature relaxation (`correctInterfacialTemperature`), and
//! the energy-conservative `heSources_` bookkeeping that feeds the two
//! phases' enthalpy equations. `forcedConstantPhaseChange`'s upstream
//! `cellZones`-based region masking (`dmdt_[celli] = value` only inside named
//! mesh zones) is mesh/solver state and is likewise out of scope — this port
//! exposes the constant rate itself; zone masking is a call-site concern.

use super::{InterfacialHeatFlux, LatentHeat, PhaseChangeRate};
use uom::si::volumetric_power_density::watt_per_cubic_meter;

/// Phase-change (evaporation/condensation) volumetric mass-transfer-rate model.
///
/// Closed enum port of GeN-Foam's `heatDrivenPhaseChange::mode` (four
/// variants, `heatDriven/heatDrivenPhaseChange.H`) plus
/// `forcedConstantPhaseChange` (`forcedConstant/forcedConstantPhaseChange.H`).
/// Evaluate with [`PhaseChangeRateModel::mass_transfer_rate`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseChangeRateModel {
    /// `dmdt = (q1 + q2) / L`: mass transfer conserves total interfacial
    /// energy transfer even if the two phases' heat fluxes to the interface
    /// don't individually balance (the TRACE approach). Can be unstable
    /// when the phases' volumetric heat capacities differ by orders of
    /// magnitude (upstream notes sodium's ~2000x liquid/vapour density ratio
    /// as the failure case) — upstream `mode::conductionLimited`.
    HeatDrivenConductionLimited,
    /// `dmdt = max(q1, 0)/L + min(q2, 0)/L`: evaporation is driven only by
    /// phase-1 superheat (`q1 > 0`), condensation only by phase-2 undercooling
    /// (`q2 < 0`) — no need to know which phase is liquid vs. vapour, that
    /// information lives in the sign of `L` at the call site (see
    /// [`super::LatentHeatModel`] docs). Upstream `mode::twoPhaseDriven`.
    HeatDrivenTwoPhaseDriven,
    /// `dmdt = q1/L` if `driving_is_phase1`, else `q2/L`: both evaporation and
    /// condensation are driven uniquely by one phase's heat flux. Upstream
    /// `mode::onePhaseDriven` (`drivingPhase` dictionary key resolved here to
    /// a bool since this port has no phase-name registry).
    HeatDrivenOnePhaseDriven {
        /// `true` selects phase 1's heat flux (`q1`) as the sole driver;
        /// `false` selects phase 2's (`q2`).
        driving_is_phase1: bool,
    },
    /// Counterpart to [`PhaseChangeRateModel::HeatDrivenTwoPhaseDriven`]: one
    /// phase drives both evaporation and condensation, the other only
    /// contributes when it reinforces (i.e. only its condensing/undercooling
    /// branch, via `neg_part`, is added). `dmdt = q1/L + min(q2,0)/L` if
    /// `driving_is_phase1`, else `dmdt = max(q1,0)/L + q2/L`. Upstream
    /// `mode::mixedDriven`.
    HeatDrivenMixedDriven {
        /// Same convention as [`PhaseChangeRateModel::HeatDrivenOnePhaseDriven`].
        driving_is_phase1: bool,
    },
    /// A prescribed constant rate, independent of the interfacial heat
    /// fluxes and latent heat (both ignored). Upstream
    /// `forcedConstantPhaseChange` (dictionary key `value`); the upstream
    /// `regions` cellZone mask that restricts where this rate is applied is
    /// solver/mesh state, out of scope here (see module docs).
    ForcedConstant {
        /// The prescribed volumetric mass-transfer rate.
        rate: PhaseChangeRate,
    },
}

impl PhaseChangeRateModel {
    /// Evaluate the volumetric phase-change mass-transfer rate `dmdt`.
    ///
    /// `q1`, `q2` are the volumetric interfacial heat fluxes from phase 1 and
    /// phase 2's bulk to the fluid-fluid interface (ignored by
    /// [`PhaseChangeRateModel::ForcedConstant`]); `l` is the specific latent
    /// heat of vaporization (also ignored by `ForcedConstant`). Faithful
    /// translation of each upstream model's `correctInterfacialDmdt()`.
    #[must_use]
    pub fn mass_transfer_rate(
        &self,
        q1: InterfacialHeatFlux,
        q2: InterfacialHeatFlux,
        l: LatentHeat,
    ) -> PhaseChangeRate {
        match *self {
            Self::HeatDrivenConductionLimited => (q1 + q2) / l,
            Self::HeatDrivenTwoPhaseDriven => pos_part(q1) / l + neg_part(q2) / l,
            Self::HeatDrivenOnePhaseDriven { driving_is_phase1 } => {
                if driving_is_phase1 {
                    q1 / l
                } else {
                    q2 / l
                }
            }
            Self::HeatDrivenMixedDriven { driving_is_phase1 } => {
                if driving_is_phase1 {
                    q1 / l + neg_part(q2) / l
                } else {
                    pos_part(q1) / l + q2 / l
                }
            }
            Self::ForcedConstant { rate } => rate,
        }
    }
}

/// OpenFOAM `posPart`: the non-negative part, `max(q, 0)`.
fn pos_part(q: InterfacialHeatFlux) -> InterfacialHeatFlux {
    InterfacialHeatFlux::new::<watt_per_cubic_meter>(q.get::<watt_per_cubic_meter>().max(0.0))
}

/// OpenFOAM `negPart`: the non-positive part, `min(q, 0)`.
fn neg_part(q: InterfacialHeatFlux) -> InterfacialHeatFlux {
    InterfacialHeatFlux::new::<watt_per_cubic_meter>(q.get::<watt_per_cubic_meter>().min(0.0))
}
