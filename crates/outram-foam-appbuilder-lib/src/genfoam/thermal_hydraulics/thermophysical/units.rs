// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/*
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Named `uom` aliases for bespoke thermophysical-property quantities
//!
//! [`super`]'s bespoke fluid-property models (currently only hydrogen) take a
//! `(p, T)` state and return several distinct physical quantities. `uom`
//! already gives most of these clear names ([`MassDensity`], [`DynamicViscosity`],
//! [`ThermalConductivity`], [`SpecificHeatCapacity`]); this module re-exports
//! them under one roof and adds the two names `uom` does not distinguish on its
//! own: **specific enthalpy** (`uom`'s [`AvailableEnergy`](uom::si::f64::AvailableEnergy))
//! and **specific entropy**, which is dimensionally identical to specific heat
//! capacity (`J / (kg K)`) but a different physical quantity, so a bare
//! `SpecificHeatCapacity` return type at a `entropy()` call site would mislead a
//! reader. Two dimensionless fractions ([`MoleFraction`], [`MassFraction`]) get
//! the same treatment as the sibling [`units`](super::super::units) module's
//! `ReynoldsNumber`/`DarcyFrictionFactor` aliases of [`Ratio`](uom::si::f64::Ratio).
//!
//! All quantities are SI (`p` in Pa, `T` in K).

/// Mass density `rho` — **base SI: kg / m^3**.
pub type MassDensity = uom::si::f64::MassDensity;

/// Isobaric specific heat capacity `Cp` — **base SI: J / (kg K)**.
pub type SpecificHeatCapacity = uom::si::f64::SpecificHeatCapacity;

/// Specific enthalpy `h` — **base SI: J / kg**. Aliases `uom`'s
/// [`AvailableEnergy`](uom::si::f64::AvailableEnergy), the quantity `uom` uses
/// for mass-specific energy.
pub type SpecificEnthalpy = uom::si::f64::AvailableEnergy;

/// Specific entropy `s` — **base SI: J / (kg K)**. Dimensionally identical to
/// [`SpecificHeatCapacity`] (both are energy / mass / temperature), so `uom`
/// has no distinct quantity for it; this alias exists purely so a function
/// signature returning entropy does not read as if it returns a heat capacity.
pub type SpecificEntropy = uom::si::f64::SpecificHeatCapacity;

/// Dynamic viscosity `mu` — **base SI: Pa s**.
pub type DynamicViscosity = uom::si::f64::DynamicViscosity;

/// Thermal conductivity `kappa` — **base SI: W / (m K)**.
pub type ThermalConductivity = uom::si::f64::ThermalConductivity;

/// A mole fraction (e.g. atomic-hydrogen mole fraction `x_H`) —
/// **dimensionless**. Aliases [`uom`]'s [`Ratio`](uom::si::f64::Ratio).
pub type MoleFraction = uom::si::f64::Ratio;

/// A mass fraction (e.g. atomic-hydrogen mass fraction `w_H`) —
/// **dimensionless**. Aliases [`uom`]'s [`Ratio`](uom::si::f64::Ratio).
pub type MassFraction = uom::si::f64::Ratio;

/// Specific volume (e.g. a second virial coefficient `B`) — **base SI:
/// m^3 / kg**. Aliases [`uom`]'s [`SpecificVolume`](uom::si::f64::SpecificVolume).
pub type SpecificVolume = uom::si::f64::SpecificVolume;

/// A Prandtl number `Pr = Cp mu / kappa` — **dimensionless**. Aliases
/// [`uom`]'s [`Ratio`](uom::si::f64::Ratio).
pub type PrandtlNumber = uom::si::f64::Ratio;
