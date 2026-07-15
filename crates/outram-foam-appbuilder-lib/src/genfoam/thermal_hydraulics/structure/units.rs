// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/structureModels/**
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Named `uom` aliases for the solid-structure (power-model) quantities
//!
//! GeN-Foam's `structure` / `powerModel` classes carry their per-cell state as
//! bare `Foam::volScalarField`s whose physical meaning lives only in comments
//! (`powerDensity_`, `alphaRhoCp_`, `iA_`, …). This module gives those recurring
//! quantities named, dimension-checked [`uom`] types so a reader hovering in
//! their editor sees [`PowerDensity`], not a raw `Quantity<...>`.
//!
//! The convective-coupling quantities the fluid side hands to the structure —
//! the effective heat-transfer coefficient and the enthalpy-weighted `h*T`
//! product — reuse the parent
//! [`thermal_hydraulics::units`](super::super::units) aliases
//! ([`HeatTransferCoefficient`], [`HeatFlux`]); they are re-exported here so a
//! caller working in `structure` finds the whole vocabulary in one place.
//!
//! All quantities are SI.

// Re-export the fluid-coupling aliases so the structure vocabulary is complete
// in one module.
pub use super::super::units::{HeatFlux, HeatTransferCoefficient};

/// Volumetric power (heat-source) density `q'''` — **base SI: W / m^3**.
///
/// The fission (or generic internal) power deposited per unit total cell volume,
/// i.e. GeN-Foam's `powerDensity_`. In a porous cell only the structure volume
/// fraction produces power, so the *effective* source in the energy balance is
/// `alpha * q'''` (see [`super::power_model`]). Aliased to [`uom`]'s
/// [`VolumetricPowerDensity`](uom::si::f64::VolumetricPowerDensity).
pub type PowerDensity = uom::si::f64::VolumetricPowerDensity;

/// Volumetric heat capacity `rho * Cp` (optionally `alpha`-weighted) —
/// **base SI: J / (m^3 K)**.
///
/// The lumped thermal inertia of the solid structure per unit total cell
/// volume: GeN-Foam's `alphaRhoCp_` (`alpha * rho * Cp`). Governs how fast the
/// structure surface temperature responds in the transient lumped energy
/// balance. Aliased to [`uom`]'s
/// [`VolumetricHeatCapacity`](uom::si::f64::VolumetricHeatCapacity).
pub type VolumetricHeatCapacity = uom::si::f64::VolumetricHeatCapacity;

/// Interfacial-area density `a_v` (wetted surface per unit volume) —
/// **base SI: 1 / m** (`m^2 / m^3`).
///
/// GeN-Foam's `iA_` / `iAact_` / `iApas_`: the structure-to-fluid heat-transfer
/// surface area per unit total cell volume. Multiplying a surface heat flux
/// `q''` [W/m^2] by this density yields a volumetric heat source [W/m^3].
/// Aliased to [`uom`]'s [`ReciprocalLength`](uom::si::f64::ReciprocalLength).
pub type InterfacialAreaDensity = uom::si::f64::ReciprocalLength;

/// Heat-exchanger wall conductance `H_w = k_wall / t_wall` —
/// **base SI: W / (m^2 K)**.
///
/// The series conductance of the heat-exchanger tube wall (thermal conductivity
/// divided by wall thickness): GeN-Foam's `Hw_`. Same dimension as a
/// convective heat-transfer coefficient. Aliased to [`uom`]'s
/// [`HeatTransfer`](uom::si::f64::HeatTransfer).
pub type WallConductance = uom::si::f64::HeatTransfer;
