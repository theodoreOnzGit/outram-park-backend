// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/**
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `closures::heat_transfer` — fluid-structure & fluid-fluid heat transfer
//!
//! Rust port of GeN-Foam's `physicsModels/heatTransferModels/**` — the largest
//! closure family in the workspace by upstream line count. It covers wall
//! (fluid-structure, "FS") and interfacial (fluid-fluid, "FF") heat-transfer
//! coefficients: single-phase forced convection, pool boiling, and a
//! multi-regime boiling dispatcher that blends them, plus the sub-models that
//! feed it (critical heat flux, Leidenfrost temperature, onset-of-nucleate-
//! boiling temperature, flow-enhancement / suppression factors, sub-cooled
//! boiling fraction).
//!
//! Belongs here: wall/interfacial **heat-transfer coefficients and heat
//! fluxes** as pure functions of already-known local state (temperatures,
//! pressure, Reynolds/Prandtl numbers, fluid properties). Does **not** belong
//! here: phase-change mass-transfer rates ([`super::phase_change`]) — see
//! "Scope boundary" below for the one place this matters.
//!
//! ## Sub-modules
//!
//! - [`fs_htc`] — fluid-structure forced-convection Nusselt correlations
//!   (Dittus-Boelter-form `Nu = A + B Re^C Pr^D`, with an optional wall-
//!   temperature-ratio term or a wall-resistance combination) and pool-boiling
//!   correlations (Shah, Gorenflo).
//! - [`boiling`] — the multi-regime boiling dispatcher (condensation /
//!   single-phase / nucleate-boiling superposition, TRACE-style) and the
//!   `superpositionNucleateBoiling` htc combiner.
//! - [`chf`] — critical-heat-flux, Leidenfrost temperature, onset-of-
//!   nucleate-boiling temperature, and the flow-enhancement / suppression /
//!   sub-cooled-boiling-fraction sub-models that feed [`boiling`].
//! - [`ff_htc`] — fluid-fluid (interfacial) forced-convection Nusselt
//!   correlation.
//!
//! ## Scope boundary: sub-cooled boiling mass transfer
//!
//! Upstream's `multiRegimeBoiling::value()` also writes a wall mass-transfer
//! source term (`dmdtW`, the sub-cooled-boiling vapour-generation rate) into a
//! *mutable field owned by the phase-change model* as a side effect of
//! computing the htc — a real `fUcK eNcApSuLaTi1o0N` moment even by upstream's
//! own code-comment admission (see `subCooledBoilingFractionModel.H`).
//! [`boiling::multi_regime_boiling_htc`] instead **returns** the two
//! ingredients that feed that term (`nucleate_boiling_heat_flux`,
//! `forced_convection_heat_flux`) as plain values; the caller combines them
//! with a [`chf::SubcooledBoilingFraction`] and writes the resulting mass rate
//! wherever the phase-change coupling (a different bead) puts it. No mutable
//! shared state crosses this module boundary.
//!
//! ## Local `uom` aliases
//!
//! [`super::units`] (this crate's wired `thermal_hydraulics::units` module)
//! already provides [`HeatTransferCoefficient`](crate::genfoam::thermal_hydraulics::units::HeatTransferCoefficient),
//! [`HeatFlux`](crate::genfoam::thermal_hydraulics::units::HeatFlux), and
//! [`ReynoldsNumber`](crate::genfoam::thermal_hydraulics::units::ReynoldsNumber);
//! all sub-modules here use those. This module additionally needs a few named
//! quantities that module does not yet define — [`PrandtlNumber`] and
//! [`LatentHeat`] below. There is also a *second*, sibling
//! `thermal_hydraulics::thermophysical::units` module in this crate with
//! overlapping candidates ([`MassDensity`](uom::si::f64::MassDensity),
//! [`ThermalConductivity`](uom::si::f64::ThermalConductivity), …). That
//! sibling module is now wired in (`thermophysical::units` exists and exports
//! its own `PrandtlNumber`), so the two aliases below are a genuine
//! duplication rather than a workaround; folding them — and the sibling's —
//! into `thermal_hydraulics::units` is an open follow-up.
//!
//! All other quantities used across this family (temperatures, pressures,
//! lengths, densities, …) are `uom`'s own already-named `f64` quantity types
//! (`ThermodynamicTemperature`, `TemperatureInterval`, `Pressure`, `Length`,
//! `MassDensity`, `ThermalConductivity`, `DynamicViscosity`, `Velocity`,
//! `SurfaceTension`), imported directly per file — matching the convention
//! already used elsewhere in this crate (e.g. `genfoam::thermo_mechanics::mesh_solve`).
//!
//! ## What is deferred (not ported; omitted rather than half-stubbed)
//!
//! - **`multiRegimeBoilingTRACE`, `multiRegimeBoilingTRACECHF`,
//!   `multiRegimeBoilingVapourTRACE`** — TRACE-specific variants layered on
//!   top of the base `multiRegimeBoiling` dispatcher with additional CHF/
//!   post-CHF wiring this port does not include (see next point).
//! - **`NusseltWallAndHfromFMU`** — couples to an external FMU (Functional
//!   Mock-up Unit) co-simulation; out of scope for a pure-algebra port.
//! - **CHF `lookUpTableCHF`** — a 3-D (pressure, mass-flux, quality) table
//!   interpolation over externally-supplied tabulated data
//!   (`InterpolateTablesGF`/`interpolation2DTable` infrastructure); porting it
//!   faithfully needs that table-interpolation machinery (out of scope here)
//!   and real published table data to verify against. [`chf::CriticalHeatFlux`]
//!   only ports `constantCHF`.
//! - **Post-CHF `CachardLiquid`/`CachardVapour`** (inverted-annular-flow film
//!   models) — both compute a vapour-film thickness from a term
//!   `pow(pi/max(DRi,1e-6),2)` where, reading the constructor,
//!   `pi` is bound to `p_[celli]`, the cell **pressure** field (`"p"` in the
//!   OpenFOAM registry) — not the rod pitch, despite the physically sensible
//!   reading (a pitch-to-diameter ratio) being what the surrounding
//!   dimensionless bracket `1 + alpha*(...)` requires, and despite a sibling
//!   model in the same family (`lookUpTableCHF`) taking an explicit
//!   `PitchToDiameter` dictionary parameter for exactly this purpose. Divided
//!   by a diameter, `pressure / DR` is not dimensionless — `uom` correctly
//!   refuses to compile the literal expression (`1.0 + Ratio::something`
//!   requires the something to be dimensionless), which is exactly the class
//!   of bug this port's type system exists to catch. Resolving which field
//!   was actually intended needs the upstream `structure`/`FSPair` classes,
//!   which are out of this bead's scope, so both models are omitted rather
//!   than guessed at.
//! - **`multiRegimeBoiling`'s post-CHF branch** — upstream's own `value()`
//!   hardcodes `TCHFi = 1e69` with the comment "needs dedicated model for its
//!   setting", i.e. the branch is unreachable and undocumented even upstream.
//!   [`boiling::multi_regime_boiling_htc`] matches this (no post-CHF branch)
//!   rather than inventing one.
//! - **FF `NoKazimi`** — see [`ff_htc`]'s module doc for why (an undocumented
//!   `fluid::operator[]` use whose semantics cannot be confirmed from the
//!   available headers).
//!
//! Everything else named in bead op-p6p.7.6 is ported and V&V'd in
//! [`tests`].

pub mod boiling;
pub mod chf;
pub mod ff_htc;
pub mod fs_htc;

#[cfg(test)]
mod tests;

use uom::si::f64::{AvailableEnergy, Ratio};

/// Prandtl number `Pr = c_p * mu / k` — **dimensionless**.
///
/// Local alias (see the module doc's "Local `uom` aliases" section) of
/// [`uom`]'s [`Ratio`]. Used throughout [`fs_htc`], [`ff_htc`], and [`chf`]
/// alongside [`crate::genfoam::thermal_hydraulics::units::ReynoldsNumber`] as
/// the second independent variable of the Nusselt-form forced-convection
/// correlations.
pub type PrandtlNumber = Ratio;

/// Latent heat of vaporization `L = h_g - h_f` — **base SI: J / kg**.
///
/// Local alias (see the module doc) of [`uom`]'s
/// [`AvailableEnergy`](uom::si::f64::AvailableEnergy), the quantity `uom`
/// uses for mass-specific energy. Used by [`chf::OnsetOfNucleateBoilingTemperature`].
pub type LatentHeat = AvailableEnergy;
