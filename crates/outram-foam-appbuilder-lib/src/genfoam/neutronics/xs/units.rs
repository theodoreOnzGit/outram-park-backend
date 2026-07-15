// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/ (XS.{C,H})
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
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

//! # Named `uom` aliases for multigroup cross-section quantities
//!
//! GeN-Foam's `nuclearData` header states that all nuclear data are provided in
//! **MKSA units, not in cm** — i.e. macroscopic cross sections are per **metre**
//! (`m^-1`), lengths in metres. This module gives each group constant a named,
//! dimension-checked [`uom`] type so a reader hovering in their editor sees
//! `MacroscopicCrossSection`, not a raw `Quantity<ISQ<...>>`.
//!
//! ## Why the interpolation core is unit-free
//!
//! The radial-basis-function interpolator ([`crate::genfoam::common::rbf`]) and the
//! per-quantity store ([`super::nuclear_data_one_energy`]) operate on bare
//! `f64`. They are dimension-agnostic numerical primitives: a single
//! interpolator maps a **heterogeneous** parameter vector — fuel temperature
//! (K), coolant density (kg/m^3), axial/radial expansion (dimensionless) — onto
//! one scalar of whatever physical dimension the quantity happens to have. No
//! single `uom` type can describe that mixed input space, exactly as
//! `outram_foam_basic_lib`'s `SquareMatrix` and `interpolate_xy` are unit-free.
//! Physical units are re-attached at the typed accessor boundary
//! ([`super::group_constants`]), which is where a solver meets the data.
//!
//! ## Convention for the helper constructors
//!
//! Each `fn` below takes a value already expressed in **base SI** and wraps it
//! in the corresponding `uom` type. The base SI units are spelled out in each
//! doc comment. Read a value back with the quantity's `.value` field, which is
//! likewise the base-SI magnitude.

use core::marker::PhantomData;
use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N1, N2, P1, Z0};

/// Diffusion coefficient `D_g` of energy group `g` — **base SI: metre (m)**.
///
/// Multigroup diffusion theory writes the current as `J_g = -D_g grad(phi_g)`;
/// with `phi_g` a group flux (`m^-2 s^-1`) the coefficient carries a length so
/// that `D_g grad(phi_g)` has flux-per-area units. Aliased to [`uom`]'s
/// `Length`.
pub type DiffusionCoefficient = uom::si::f64::Length;

/// A macroscopic cross section `Sigma` — **base SI: per metre (m^-1)**.
///
/// Used for the removal cross section `Sigma_{r,g}`, the fission-neutron
/// production cross section `nu Sigma_{f,g}`, and each scattering-matrix entry
/// `Sigma_{s, g->g'}`. Macroscopic (already multiplied by number density), so
/// the natural dimension is inverse length. Aliased to [`uom`]'s
/// `LinearNumberDensity` (`L^-1`).
pub type MacroscopicCrossSection = uom::si::f64::LinearNumberDensity;

/// Fission energy-release cross section `kappa Sigma_{f,g}` —
/// **base SI: joule per metre (J/m = kg m s^-2)**.
///
/// Its product with a scalar group flux `phi_g` (`m^-2 s^-1`, neutrons treated
/// as a dimensionless count) yields a volumetric power density
/// (`W/m^3 = kg m^-1 s^-3`), hence dimension `L M T^-2`.
pub type EnergyReleaseCrossSection = Quantity<ISQ<P1, P1, N2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Inverse neutron speed `1/v_g` of energy group `g` —
/// **base SI: second per metre (s/m = L^-1 T)**.
///
/// The time-dependent multigroup balance carries `(1/v_g) d(phi_g)/dt`; the
/// inverse speed sets the group's kinetic time scale. There is no standard
/// named `uom` quantity for `s/m`, so it is defined here from the ISQ base.
pub type InverseVelocity = Quantity<ISQ<N1, Z0, P1, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Delayed-neutron precursor decay constant `lambda_k` —
/// **base SI: per second (s^-1)**.
///
/// Group `k`'s precursors decay as `exp(-lambda_k t)`. Dimensionally a
/// frequency, so aliased to [`uom`]'s `Frequency`.
pub type PrecursorDecayConstant = uom::si::f64::Frequency;

/// A dimensionless nuclear-data ratio — the fission spectra `chi_{p,g}` /
/// `chi_{d,g}`, delayed fractions `beta_k` / `beta_tot`, and discontinuity
/// factors `gamma_g`. Aliased to [`uom`]'s `Ratio`.
pub type Dimensionless = uom::si::f64::Ratio;

/// Wrap a base-SI value (`m`) as a [`DiffusionCoefficient`].
#[must_use]
pub const fn diffusion_coefficient(metres: f64) -> DiffusionCoefficient {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: metres,
    }
}

/// Wrap a base-SI value (`m^-1`) as a [`MacroscopicCrossSection`].
#[must_use]
pub const fn macroscopic_cross_section(per_metre: f64) -> MacroscopicCrossSection {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: per_metre,
    }
}

/// Wrap a base-SI value (`J/m`) as an [`EnergyReleaseCrossSection`].
#[must_use]
pub const fn energy_release_cross_section(joule_per_metre: f64) -> EnergyReleaseCrossSection {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: joule_per_metre,
    }
}

/// Wrap a base-SI value (`s/m`) as an [`InverseVelocity`].
#[must_use]
pub const fn inverse_velocity(second_per_metre: f64) -> InverseVelocity {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: second_per_metre,
    }
}

/// Wrap a base-SI value (`s^-1`) as a [`PrecursorDecayConstant`].
#[must_use]
pub const fn precursor_decay_constant(per_second: f64) -> PrecursorDecayConstant {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: per_second,
    }
}

/// Wrap a bare ratio as a [`Dimensionless`] quantity.
#[must_use]
pub const fn dimensionless(ratio: f64) -> Dimensionless {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: ratio,
    }
}
