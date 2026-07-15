// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/thermophysicalProperties/H2/
//     (`H2.H`, `H2.C`, `H2I.H`)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Thomas Guilbaud, Eymeric Simonnot (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Dissociating hydrogen (H <-> H2) thermophysical properties
//!
//! Port of GeN-Foam's `Foam::H2` class: a bespoke property package for
//! hydrogen coolant/working fluid that accounts for thermal dissociation of
//! molecular H2 into atomic H at high temperature (order 2000-6000 K, e.g. a
//! nuclear-thermal-propulsion or gas-core reactor). Every bulk property is a
//! mass- (or mole-) fraction-weighted combination of a pure-atomic-H
//! correlation and a pure-molecular-H2 correlation, where the blend weight
//! comes from the `H2 <-> 2H` equilibrium in [`dissociation`].
//!
//! ## Sub-module map
//!
//! - [`constants`] — fixed molar masses, critical-point data, and derived
//!   cubic-EOS coefficients.
//! - [`dissociation`] — the `H2 <-> 2H` equilibrium (`Kp`, mole/mass fraction
//!   of atomic H).
//! - [`collision_integrals`] — Chapman-Enskog collision integrals and
//!   Hirschfelder-Curtiss-Bird mixture-rule ratios.
//! - [`equation_of_state`] — density (ideal-gas atomic H, cubic-EOS molecular
//!   H2) and `Cp - Cv`.
//! - [`heat_capacity`] — isobaric/isochoric heat capacity and the second
//!   virial coefficient.
//! - [`thermodynamics`] — specific enthalpy and entropy.
//! - [`diffusion`] — binary H-H2 diffusivity.
//! - [`viscosity`] — dynamic viscosity (Muzny et al. 2013 for H2, kinetic
//!   theory for H, Hirschfelder-Curtiss-Bird mixture rule for the blend).
//! - [`thermal_conductivity`] — thermal conductivity (Assael et al. 2011 for
//!   H2), thermal diffusivity, Prandtl number.
//!
//! Each sub-module's functions are `pub(super)` — internal to this package —
//! because the crate's public surface is the closed
//! [`BespokeFluidPropertyModel`](super::BespokeFluidPropertyModel) enum in the
//! parent [`thermophysical`](super) module, which dispatches to the `pub`
//! wrapper functions below. That mirrors
//! [`closures::fs_drag`](crate::genfoam::thermal_hydraulics::closures::fs_drag)'s
//! enum-only public surface.
//!
//! ## What was intentionally not ported
//!
//! GeN-Foam's `H2` class inherits OpenFOAM's `liquidProperties` base class and
//! therefore carries a set of NSRDS correlation-coefficient members (`rho_`,
//! `pv_`, `hl_`, `Cp_`, `h_`, `Cpg_`, `B_`, `mu_`, `mug_`, `kappa_`, `kappag_`,
//! `sigma_`, `D_`) that the upstream header itself labels **"useless but
//! needed because liquidProperties is over constrained"** — no method in
//! `H2I.H` ever reads them, since every physical property is computed by a
//! bespoke closed-form correlation instead. This port does not translate
//! `liquidProperties` or these unused members (they would be dead code with no
//! caller). A few individual leaf functions with the same property — no
//! caller upstream and no reference value in upstream's own regression test —
//! are likewise not ported; see the "no orphaned dead code" notes in
//! [`collision_integrals`] (`C_12`), [`diffusion`] (`diffusionH2`, the pure-H2
//! self-diffusivity), and [`equation_of_state`] (`psi`).

mod collision_integrals;
mod constants;
mod diffusion;
mod dissociation;
mod equation_of_state;
mod heat_capacity;
mod thermal_conductivity;
mod thermodynamics;
mod viscosity;

use super::units::{
    DynamicViscosity, MassDensity, MassFraction, MoleFraction, PrandtlNumber, SpecificEnthalpy,
    SpecificEntropy, SpecificHeatCapacity, SpecificVolume, ThermalConductivity,
};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Bulk mixture density (upstream `H2::rho`). `p` in Pa, `T` in K.
#[inline]
pub(super) fn density(p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(equation_of_state::density(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Bulk mixture isobaric heat capacity (upstream `H2::Cp`).
#[inline]
pub(super) fn specific_heat_capacity(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(heat_capacity::specific_heat_capacity(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Bulk mixture isochoric heat capacity `Cv = Cp - (Cp - Cv)` (see
/// [`heat_capacity::specific_heat_capacity_cv`]).
#[inline]
pub(super) fn specific_heat_capacity_cv(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
        heat_capacity::specific_heat_capacity_cv(p.get::<pascal>(), t.get::<kelvin>()),
    )
}

/// Bulk mixture dynamic viscosity (upstream `H2::mu`).
#[inline]
pub(super) fn dynamic_viscosity(p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pascal_second>(viscosity::dynamic_viscosity(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Bulk mixture thermal conductivity (upstream `H2::kappa`).
#[inline]
pub(super) fn thermal_conductivity(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(thermal_conductivity::thermal_conductivity(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Bulk mixture specific enthalpy (upstream `H2::Ha`/`H2::Hs`/`H2::h`).
#[inline]
pub(super) fn specific_enthalpy(p: Pressure, t: ThermodynamicTemperature) -> SpecificEnthalpy {
    SpecificEnthalpy::new::<joule_per_kilogram>(thermodynamics::specific_enthalpy(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Bulk mixture specific entropy (upstream `H2::S`).
#[inline]
pub(super) fn specific_entropy(p: Pressure, t: ThermodynamicTemperature) -> SpecificEntropy {
    SpecificEntropy::new::<joule_per_kilogram_kelvin>(thermodynamics::specific_entropy(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Mole fraction of atomic hydrogen at equilibrium (upstream `H2::x_H`).
#[inline]
pub(super) fn mole_fraction_atomic_hydrogen(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> MoleFraction {
    MoleFraction::new::<ratio>(dissociation::mole_fraction_atomic_h(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Mass fraction of atomic hydrogen at equilibrium (upstream `H2::w_H`).
#[inline]
pub(super) fn mass_fraction_atomic_hydrogen(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> MassFraction {
    MassFraction::new::<ratio>(dissociation::mass_fraction_atomic_h(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Second virial coefficient of the H/H2 mixture (upstream `H2::B`). See
/// [`heat_capacity`]'s module doc for the faithfully-ported upstream
/// argument-order quirk this reproduces.
#[inline]
pub(super) fn second_virial_coefficient(
    p: Pressure,
    t: ThermodynamicTemperature,
) -> SpecificVolume {
    SpecificVolume::new::<cubic_meter_per_kilogram>(heat_capacity::second_virial_coefficient(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// Prandtl number `Pr = Cp mu / kappa` (upstream `H2::Pr`). See
/// [`thermal_conductivity`](mod@thermal_conductivity)'s module doc for the
/// "not covered by upstream's own regression fixture" caveat that also
/// applies here (it is built from `kappa`, which has no upstream reference
/// value).
#[inline]
pub(super) fn prandtl_number(p: Pressure, t: ThermodynamicTemperature) -> PrandtlNumber {
    PrandtlNumber::new::<ratio>(thermal_conductivity::prandtl_number(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}

/// `kappa / Cp` (upstream `H2::alphah`, upstream-named "thermal diffusivity").
///
/// **Not** the usual thermal diffusivity `kappa / (rho Cp)` (m^2/s) — upstream
/// literally divides by `Cp` alone, which is dimensionally `kg / (m s)`, the
/// same base dimension as dynamic viscosity ([`DynamicViscosity`]), which is
/// why that is the return type here. Ported exactly as upstream defines it;
/// not "fixed" to insert a `/ rho`, per this crate's guardrail against
/// papering over upstream behaviour.
#[inline]
pub(super) fn kappa_over_cp(p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pascal_second>(thermal_conductivity::thermal_diffusivity(
        p.get::<pascal>(),
        t.get::<kelvin>(),
    ))
}
