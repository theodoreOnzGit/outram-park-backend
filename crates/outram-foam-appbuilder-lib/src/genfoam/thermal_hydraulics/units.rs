// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Named `uom` aliases for thermal-hydraulics quantities
//!
//! GeN-Foam's thermal-hydraulics closures return bare `Foam::scalar`s whose
//! physical meaning is only documented in comments. This module gives the
//! recurring TH quantities named, dimension-checked [`uom`] types so a reader
//! hovering in their editor sees `DarcyFrictionFactor`, not a raw
//! `Quantity<...>`. Each alias below is the type a closure method takes or
//! returns.
//!
//! All quantities are SI. Two of them ([`ReynoldsNumber`], [`DarcyFrictionFactor`])
//! are dimensionless and alias [`uom`]'s [`Ratio`](uom::si::f64::Ratio); the
//! others carry genuine dimensions.
//!
//! ## Why the correlation cores still take a plain `Ratio`
//!
//! A friction-factor correlation is a pure map `Re -> f`, both dimensionless.
//! Wrapping them in `Ratio` keeps the *call site* self-documenting (you cannot
//! pass a temperature where a Reynolds number is wanted) without inventing a
//! bespoke newtype the compiler could not check.

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Reynolds number `Re = rho * |U| * D_h / mu` — **dimensionless**.
///
/// The single independent variable of the fluid-structure wall-friction
/// correlations in [`super::closures::fs_drag`]. GeN-Foam forms it per cell from
/// the local fluid density, superficial velocity magnitude, hydraulic diameter,
/// and dynamic viscosity; here it is passed in already assembled. Aliased to
/// [`uom`]'s [`Ratio`].
pub type ReynoldsNumber = Ratio;

/// Darcy(-Weisbach) friction factor `f` — **dimensionless**.
///
/// The quantity returned by the fluid-structure wall-friction correlations. In
/// the laminar limit it reduces to `f = C / Re` (circular pipe `C = 64`,
/// wire-wrapped rod bundles `C ~ 99..110`). The pressure gradient along a
/// channel is `dp/dx = -f * (rho * U^2) / (2 * D_h)`. Aliased to [`uom`]'s
/// [`Ratio`].
///
/// Note: this is the **Darcy** factor (the `64/Re` convention), four times the
/// Fanning factor. GeN-Foam's `FSDragCoefficientModel::value()` returns this
/// Darcy form (its laminar branch is `64/Re`, `99/Re`, etc.).
pub type DarcyFrictionFactor = Ratio;

/// Convective heat-transfer coefficient `h` — **base SI: W / (m^2 K)**.
///
/// Used by the fluid-structure and fluid-fluid heat-transfer closures
/// (`Nu = h * D_h / k`). Aliased to [`uom`]'s
/// [`HeatTransfer`](uom::si::f64::HeatTransfer).
pub type HeatTransferCoefficient = uom::si::f64::HeatTransfer;

/// Surface heat flux `q''` — **base SI: W / m^2**.
///
/// The wall heat flux exchanged between the fluid and the structure; the target
/// of the critical-heat-flux (CHF) closures. Aliased to [`uom`]'s
/// [`HeatFluxDensity`](uom::si::f64::HeatFluxDensity).
pub type HeatFlux = uom::si::f64::HeatFluxDensity;

/// Build a [`ReynoldsNumber`] from a plain (dimensionless) magnitude.
///
/// Convenience for call sites that have already computed `Re` as an `f64`.
#[inline]
#[must_use]
pub fn reynolds_number(value: f64) -> ReynoldsNumber {
    ReynoldsNumber::new::<ratio>(value)
}

/// Read a [`DarcyFrictionFactor`] back as a plain (dimensionless) `f64`.
#[inline]
#[must_use]
pub fn friction_factor_value(f: DarcyFrictionFactor) -> f64 {
    f.get::<ratio>()
}
