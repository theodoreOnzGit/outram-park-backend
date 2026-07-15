// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/{interfacialAreaModels,
//             fluidDiameterModels,virtualMassModels,dispersionModels,
//             contactPartitionModels,regimeMapModels}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Named `uom` aliases local to `closures::interfacial`
//!
//! [`crate::genfoam::thermal_hydraulics::units`] (the shared
//! `thermal_hydraulics::units` module) only defines the aliases the
//! already-ported [`crate::genfoam::thermal_hydraulics::closures::fs_drag`]
//! family needs (`ReynoldsNumber`, `DarcyFrictionFactor`,
//! `HeatTransferCoefficient`, `HeatFlux`). This module adds the two names the
//! two-phase geometry/regime closures need that are not there yet:
//!
//! - [`InterfacialAreaConcentration`] — interfacial area per unit mixture volume
//!   `a_i` (1/m). The one genuinely new *dimension* this closure family
//!   introduces (`uom`'s [`ReciprocalLength`](uom::si::f64::ReciprocalLength)).
//! - [`FluidDiameter`] — the bubble/droplet/film characteristic diameter (m). An
//!   alias of `uom`'s plain [`Length`](uom::si::f64::Length); named separately so
//!   a call site cannot confuse it with, say, a hydraulic or pin diameter that
//!   happens to also be a `Length`.
//!
//! `virtualMassModels`/`dispersionModels`/`contactPartitionModels` all reduce to
//! plain dimensionless fractions or coefficients, so they reuse `uom`'s
//! [`Ratio`](uom::si::f64::Ratio) directly (same convention as `ReynoldsNumber`
//! in the shared module) — no new alias needed for those.
//!
//! **Candidate for promotion to the shared `units.rs`:** [`InterfacialAreaConcentration`]
//! is likely to recur once the fluid-fluid drag/heat-transfer closures (bead
//! op-p6p.7.9/.7.10) are ported, since `a_i` is their shared multiplier. Left
//! local here per the op-p6p.7.8 task scope (touching the shared file is out of
//! this bead's lane); flagged in the bead's hand-off report for whoever ports
//! `ff_drag`/`heat_transfer` next to hoist it if it turns out to be shared.

use uom::si::f64::{Length, ReciprocalLength};
use uom::si::length::meter;
use uom::si::reciprocal_length::reciprocal_meter;

/// Interfacial area concentration `a_i` — **base SI: 1/m** (area of the
/// fluid-fluid interface per unit volume of the two-fluid mixture).
///
/// Returned by every [`super::area::InterfacialArea`] variant. Aliases `uom`'s
/// [`ReciprocalLength`](uom::si::f64::ReciprocalLength) (dimension `L^-1`, which
/// is exactly `a_i`'s dimension: interface area `[L^2]` per mixture volume `[L^3]`).
pub type InterfacialAreaConcentration = ReciprocalLength;

/// A characteristic fluid diameter — bubble, droplet, or film thickness —
/// **base SI: m**.
///
/// Returned by every [`super::diameter::BubbleDiameter`] and
/// [`super::diameter::FilmDiameter`] variant, and consumed as the
/// `dispersed_diameter` argument of [`super::area::InterfacialArea::spherical`]/
/// [`super::area::InterfacialArea::annular`]. Aliases `uom`'s
/// [`Length`](uom::si::f64::Length).
pub type FluidDiameter = Length;

/// Build an [`InterfacialAreaConcentration`] from a bare (1/m) magnitude.
#[inline]
#[must_use]
pub fn interfacial_area_concentration(value_per_metre: f64) -> InterfacialAreaConcentration {
    InterfacialAreaConcentration::new::<reciprocal_meter>(value_per_metre)
}

/// Build a [`FluidDiameter`] from a bare metre magnitude.
#[inline]
#[must_use]
pub fn fluid_diameter(value_m: f64) -> FluidDiameter {
    FluidDiameter::new::<meter>(value_m)
}
