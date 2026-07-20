// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream: src/classes/thermalHydraulics/src/thermophysicalProperties/**
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::thermophysical` — bespoke GeN-Foam fluid property packages
//!
//! GeN-Foam's `thermophysicalProperties/` sub-tree defines fluid-property
//! packages that are not already covered by
//! [`tampines_steam_tables`](https://docs.rs/tampines-steam-tables) (steam/water)
//! or `outram-foam-basic-lib`'s thermo layer. As of upstream commit `652b3da`
//! there is exactly one such package: **dissociating hydrogen** (`H2`,
//! GeN-Foam's bespoke H/H2 property model for nuclear-thermal-propulsion-style
//! high-temperature hydrogen coolant). See [`hydrogen`] for the full
//! implementation and provenance notes.
//!
//! ## Public surface — [`BespokeFluidPropertyModel`]
//!
//! Following this crate's "no trait objects — use enums for dispatch" rule
//! (mirroring
//! [`closures::fs_drag::FsWallFriction`](crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction)),
//! the entire public surface of this module is the closed
//! [`BespokeFluidPropertyModel`] enum. It currently has one variant,
//! [`BespokeFluidPropertyModel::Hydrogen`]; adding a second bespoke fluid
//! package later means adding a variant here and a sibling module next to
//! [`hydrogen`], not introducing a trait object.
//!
//! Every evaluator takes the fluid state as `(p, T)` — pressure and
//! thermodynamic temperature — matching upstream's `H2::method(p, T)`
//! signatures, and returns a named `uom` quantity from [`units`].
//!
//! ## What this module does *not* cover
//!
//! `src/thermophysicalProperties/` is only 3 upstream files (1,415 LOC) out of
//! the ~65k-LOC `thermalHydraulics` module (see `docs/genfoam-port-plan.md`,
//! "thermalHydraulics breakdown"). Reading/writing GeN-Foam's `constant/`
//! dictionary format to select a fluid model at run time is an I/O concern
//! that belongs with [`io`](crate::io), not here — this module's job is the
//! physics evaluators, addressed by picking an enum variant in Rust code.

mod hydrogen;
pub mod units;

use units::{
    DynamicViscosity, MassDensity, MassFraction, MoleFraction, PrandtlNumber, SpecificEnthalpy,
    SpecificEntropy, SpecificHeatCapacity, SpecificVolume, ThermalConductivity,
};
use uom::si::f64::{Pressure, ThermodynamicTemperature};

/// A bespoke (non-steam-table) fluid thermophysical-property model.
///
/// Closed enum port of GeN-Foam's `thermophysicalProperties`
/// runtime-selectable family — see the [module documentation](self) for why
/// this is the crate's entire public surface for bespoke fluid properties.
/// Evaluate with the methods below, each taking pressure `p` and
/// thermodynamic temperature `T`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BespokeFluidPropertyModel {
    /// Dissociating hydrogen (H <-> H2 thermal equilibrium), GeN-Foam's `H2`
    /// class. See [`hydrogen`] for the correlations and their provenance.
    #[default]
    Hydrogen,
}

impl BespokeFluidPropertyModel {
    /// Mass density `rho`. `p` in Pa, `T` in K; returns kg/m^3.
    #[must_use]
    pub fn density(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
        match self {
            Self::Hydrogen => hydrogen::density(p, t),
        }
    }

    /// Isobaric specific heat capacity `Cp`. `p` in Pa, `T` in K; returns
    /// J/(kg K).
    #[must_use]
    pub fn specific_heat_capacity(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> SpecificHeatCapacity {
        match self {
            Self::Hydrogen => hydrogen::specific_heat_capacity(p, t),
        }
    }

    /// Isochoric specific heat capacity `Cv`. `p` in Pa, `T` in K; returns
    /// J/(kg K).
    #[must_use]
    pub fn specific_heat_capacity_cv(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> SpecificHeatCapacity {
        match self {
            Self::Hydrogen => hydrogen::specific_heat_capacity_cv(p, t),
        }
    }

    /// Dynamic viscosity `mu`. `p` in Pa, `T` in K; returns Pa s.
    #[must_use]
    pub fn dynamic_viscosity(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        match self {
            Self::Hydrogen => hydrogen::dynamic_viscosity(p, t),
        }
    }

    /// Thermal conductivity `kappa`. `p` in Pa, `T` in K; returns W/(m K).
    #[must_use]
    pub fn thermal_conductivity(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> ThermalConductivity {
        match self {
            Self::Hydrogen => hydrogen::thermal_conductivity(p, t),
        }
    }

    /// Specific enthalpy `h`. `p` in Pa, `T` in K; returns J/kg.
    #[must_use]
    pub fn specific_enthalpy(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnthalpy {
        match self {
            Self::Hydrogen => hydrogen::specific_enthalpy(p, t),
        }
    }

    /// Specific entropy `s`. `p` in Pa, `T` in K; returns J/(kg K).
    #[must_use]
    pub fn specific_entropy(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEntropy {
        match self {
            Self::Hydrogen => hydrogen::specific_entropy(p, t),
        }
    }

    /// Prandtl number `Pr = Cp mu / kappa`. `p` in Pa, `T` in K; dimensionless.
    #[must_use]
    pub fn prandtl_number(&self, p: Pressure, t: ThermodynamicTemperature) -> PrandtlNumber {
        match self {
            Self::Hydrogen => hydrogen::prandtl_number(p, t),
        }
    }

    /// Mole fraction of a dissociated/minority species (for [`Hydrogen`](Self::Hydrogen),
    /// the atomic-hydrogen mole fraction `x_H`). `p` in Pa, `T` in K;
    /// dimensionless, in `[0, 1)`.
    #[must_use]
    pub fn dissociated_mole_fraction(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> MoleFraction {
        match self {
            Self::Hydrogen => hydrogen::mole_fraction_atomic_hydrogen(p, t),
        }
    }

    /// Mass fraction of a dissociated/minority species (for [`Hydrogen`](Self::Hydrogen),
    /// the atomic-hydrogen mass fraction `w_H`). `p` in Pa, `T` in K;
    /// dimensionless, in `[0, 1)`.
    #[must_use]
    pub fn dissociated_mass_fraction(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> MassFraction {
        match self {
            Self::Hydrogen => hydrogen::mass_fraction_atomic_hydrogen(p, t),
        }
    }

    /// Second virial coefficient `B` of the real-gas equation of state. `p`
    /// in Pa, `T` in K; returns m^3/kg.
    #[must_use]
    pub fn second_virial_coefficient(
        &self,
        p: Pressure,
        t: ThermodynamicTemperature,
    ) -> SpecificVolume {
        match self {
            Self::Hydrogen => hydrogen::second_virial_coefficient(p, t),
        }
    }

    /// `kappa / Cp` (upstream `H2::alphah`). See
    /// [`hydrogen::kappa_over_cp`] for why this is *not* the usual
    /// `kappa / (rho Cp)` thermal diffusivity.
    #[must_use]
    pub fn kappa_over_cp(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
        match self {
            Self::Hydrogen => hydrogen::kappa_over_cp(p, t),
        }
    }
}
