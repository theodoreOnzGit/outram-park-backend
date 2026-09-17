// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/turbulenceModels/
//             {LaheyKEpsilon,mixtureKEpsilon,porousKEpsilon,
//             porousKEpsilon2PhaseCorrected}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Named `uom` aliases for the two-phase / porous turbulence closures
//!
//! [`super`]'s bubble-induced and porous-relaxation closures combine void
//! fractions, slip velocities, and the turbulence pair `(k, epsilon)` into a
//! handful of quantities `uom` does not all have off-the-shelf names for. This
//! module gives each one a named, dimension-checked type so a reader hovering
//! in their editor sees `EddyViscosity` or `TurbulentKineticEnergy`, not a raw
//! `Quantity<...>`.
//!
//! **Local to this sub-module.** [`ReynoldsNumber`](super::super::super::units::ReynoldsNumber)
//! is reused from the already-wired-in
//! [`thermal_hydraulics::units`](super::super::super::units) module. The
//! quantities below were defined fresh here during the initial port, when the
//! sibling `phase` / `thermophysical` sub-modules were not yet wired into the
//! tree. Those modules are **now** live, so [`VoidFraction`] here is a
//! near-exact duplicate of
//! [`phase::phase_base::VolumeFraction`](super::super::super::phase::phase_base::VolumeFraction)
//! (both `uom` `Ratio`). Unifying them — re-export one from the other, or hoist
//! both to the shared `thermal_hydraulics::units` — is a small follow-up tracked
//! in beads (see the appbuilder epic `op-p6p`), deliberately left out of the
//! integration pass to avoid churning verified turbulence closures.
//!
//! ## Standard quantities (aliases of existing `uom` types)
//!
//! | Alias | `uom` type | Base SI | Physical meaning |
//! |---|---|---|---|
//! | [`VoidFraction`] | `Ratio` | – | phase volume fraction `alpha`, `0..=1` |
//! | [`RelativeVelocity`] | `Velocity` | m/s | interfacial slip speed `\|U_c - U_d\|` |
//! | [`HydraulicDiameter`] | `Length` | m | dispersed-phase or porous-structure `Dh` |
//! | [`EddyViscosity`] | `KinematicViscosity` | m^2/s | turbulent (eddy) viscosity `nu_t` |
//! | [`TurbulentKineticEnergy`] | `AvailableEnergy` | m^2/s^2 | `k` |
//! | [`TurbulentDissipationRate`] | `SpecificPower` | m^2/s^3 | `epsilon`, and `dk/dt`-shaped rates |
//! | [`DragCoefficient`] | `Ratio` | – | fluid-fluid drag coefficient `Cd` |
//!
//! ## Composite quantities (no built-in `uom` name)
//!
//! | Alias | Base SI | Physical meaning |
//! |---|---|---|
//! | [`VolumetricRelaxationCoefficient`] | kg/(m^3 s) | `Kd` (interfacial friction) and `alpha*rho/tau` (porous relaxation) |
//! | [`MixtureBubbleProduction`] | kg/(m s^3) | mixture-model bubble-induced production `bubbleG` (dimensionally distinct from the Lahey model's — see [`super::mixture_k_epsilon`]) |
//! | [`TurbulentDissipationRateOfChange`] | m^2/s^4 | `d(epsilon)/dt`-shaped production terms |

use core::marker::PhantomData;
use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N1, N3, N4, P1, P2, Z0};

/// Phase **void (volume) fraction** `alpha` — **dimensionless** (`0..=1`).
///
/// Aliased to [`uom`]'s [`Ratio`](uom::si::f64::Ratio). See the module doc for
/// why this duplicates (for now) `phase::phase_base::VolumeFraction`.
pub type VoidFraction = uom::si::f64::Ratio;

/// Interfacial **relative (slip) velocity magnitude** `\|U_c - U_d\|` —
/// **base SI: m/s**. Aliased to [`uom`]'s [`Velocity`](uom::si::f64::Velocity).
pub type RelativeVelocity = uom::si::f64::Velocity;

/// A **hydraulic diameter** — **base SI: m**. Used both for the dispersed
/// (bubble/droplet) phase's `Dh` and the porous structure's `Dh`. Aliased to
/// [`uom`]'s [`Length`](uom::si::f64::Length).
pub type HydraulicDiameter = uom::si::f64::Length;

/// **Eddy (turbulent) viscosity** `nu_t` — **base SI: m^2/s**. Aliased to
/// [`uom`]'s [`KinematicViscosity`](uom::si::f64::KinematicViscosity).
pub type EddyViscosity = uom::si::f64::KinematicViscosity;

/// **Turbulent kinetic energy** `k` — **base SI: m^2/s^2 (J/kg)**. `uom` has
/// no quantity named "turbulent kinetic energy"; dimensionally it is a
/// mass-specific energy, so this aliases [`uom`]'s
/// [`AvailableEnergy`](uom::si::f64::AvailableEnergy) (the quantity `uom` uses
/// for `J/kg`).
pub type TurbulentKineticEnergy = uom::si::f64::AvailableEnergy;

/// **Turbulent dissipation rate** `epsilon` — **base SI: m^2/s^3 (W/kg)**.
/// Also used for any `dk/dt`-shaped rate (dimensionally identical: energy per
/// unit mass per unit time). Aliased to [`uom`]'s
/// [`SpecificPower`](uom::si::f64::SpecificPower).
pub type TurbulentDissipationRate = uom::si::f64::SpecificPower;

/// Fluid-fluid **drag coefficient** `Cd` — **dimensionless**. Aliased to
/// [`uom`]'s [`Ratio`](uom::si::f64::Ratio).
pub type DragCoefficient = uom::si::f64::Ratio;

/// A **volumetric relaxation coefficient** — **base SI: kg/(m^3 s)**
/// (`M L^-3 T^-1`).
///
/// GeN-Foam's fluid-fluid interfacial friction coefficient `Kd` (from
/// `F_drag/V = Kd * (U_c - U_d)`, so `Kd` has dimension
/// `(force/volume)/velocity = kg/(m^3 s)`) and the porous k-epsilon models'
/// `alpha*rho*(\|U\|/convergenceLength)` relaxation coefficient share this
/// exact dimension — both are "a density divided by a time scale". There is
/// no standard named `uom` quantity for it, so it is defined here from the
/// ISQ base, following the precedent in
/// `outram_foam_basic_lib::thermophysics::quantities::Compressibility` and
/// `genfoam::neutronics::xs::units`.
pub type VolumetricRelaxationCoefficient = Quantity<ISQ<N3, P1, N1, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// The **mixture-model bubble-induced production term** `bubbleG` —
/// **base SI: kg/(m s^3)** (`M L^-1 T^-3`).
///
/// GeN-Foam's `mixtureKEpsilon::bubbleG()` carries an extra `liquid()*rho_l`
/// factor the Lahey model's `bubbleG()` does not (the upstream source comments
/// this explicitly: "Differs from the Lahey model as it has this extra term
/// (which also makes them dimensionally different)"). The Lahey model's
/// `bubbleG` is a genuine specific (per-unit-mass) production rate — aliased
/// to [`TurbulentDissipationRate`] — while this one is not, hence the separate
/// composite type.
pub type MixtureBubbleProduction = Quantity<ISQ<N1, P1, N3, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// The **rate of change of the turbulent dissipation rate**, `d(epsilon)/dt`
/// — **base SI: m^2/s^4** (`L^2 T^-4`).
///
/// `epsilon` itself has dimension m^2/s^3 ([`TurbulentDissipationRate`]); its
/// own production/relaxation terms (as they appear on the RHS of the
/// `epsilon` transport equation) therefore carry one more inverse-time power.
/// No standard named `uom` quantity exists for this; defined here from the
/// ISQ base.
pub type TurbulentDissipationRateOfChange = Quantity<ISQ<P2, Z0, N4, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Wrap a base-SI value (`kg/(m^3 s)`) as a [`VolumetricRelaxationCoefficient`].
#[must_use]
pub const fn volumetric_relaxation_coefficient(
    kilogram_per_cubic_metre_second: f64,
) -> VolumetricRelaxationCoefficient {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: kilogram_per_cubic_metre_second,
    }
}

/// Wrap a base-SI value (`kg/(m s^3)`) as a [`MixtureBubbleProduction`].
#[must_use]
pub const fn mixture_bubble_production(
    kilogram_per_metre_second_cubed: f64,
) -> MixtureBubbleProduction {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: kilogram_per_metre_second_cubed,
    }
}

/// Wrap a base-SI value (`m^2/s^4`) as a [`TurbulentDissipationRateOfChange`].
#[must_use]
pub const fn dissipation_rate_of_change(
    square_metre_per_second_quartic: f64,
) -> TurbulentDissipationRateOfChange {
    Quantity {
        dimension: PhantomData,
        units: PhantomData,
        value: square_metre_per_second_quartic,
    }
}
