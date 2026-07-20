// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/boundaryConditions/
//             NusseltThermalBaffle1D/NusseltThermalBaffle1DFvPatchScalarField.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (sradman@pm.me; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `nusselt_baffle` — 1-D through-wall conduction coupled baffle (SCAFFOLD ONLY)
//!
//! **Not implemented.** This module only declares the public data shape of
//! GeN-Foam's `NusseltThermalBaffle1DFvPatchScalarField` so the rest of the
//! `boundary_conditions` module tree can reference its types; every method
//! that would compute a physical result is `unimplemented!()`. See bead
//! op-p6p.7.13 (follow-up) for the full port.
//!
//! ## What upstream does (for the next porter's context)
//!
//! Two GeN-Foam patches (a "master" and a "slave", named via the `samplePatch`
//! entry) sit on opposite faces of a **thin** wall — thin meaning the wall's
//! own thermal inertia is neglected, so its two surface temperatures are set
//! by an **instantaneous heat-flux balance** rather than a transient
//! conduction solve:
//!
//! ```text
//! h_master * (T_fluid,master - T_wall,master)
//!     = kappa_wall / thickness * (T_wall,master - T_wall,slave)
//!     = h_slave  * (T_wall,slave  - T_fluid,slave)
//! ```
//!
//! `h_master`/`h_slave` are convective heat-transfer coefficients from a
//! Nusselt-number correlation of the form
//!
//! ```text
//! Nu = const + coeff * Re^expRe * Pr^expPr
//! ```
//!
//! with per-side `(const, coeff, expRe, expPr)` — the slave side may omit any
//! of the four and inherit the master's value (upstream: "If not provided,
//! defaults to master value").
//!
//! The update is **implicit**: upstream's `updateCoeffs()` on the master patch
//! also reaches into the slave patch's `valueFraction`/`refValue`/`refGradient`
//! (a `mixed` boundary condition) to solve the three-way balance above as one
//! coupled system, and — in the two-phase case — additionally couples across
//! *both* fluid phases' temperature fields simultaneously. That
//! cross-patch/cross-phase coupling is genuinely solver-shaped (it needs
//! mutable access to another patch's BC state inside one patch's coefficient
//! update), which is why this is scaffolded rather than fully ported here: it
//! does not fit this module's "plain struct + pure function over dimensioned
//! scalars" contract without first deciding how the porous-solver bead
//! (op-p6p.7.11) represents inter-patch coupling.
//!
//! ## Not fabricated here
//!
//! No formula below is evaluated — the structs only carry the dictionary
//! entries upstream reads (`thickness`, `kappa`, `const`/`coeff`/`expRe`/
//! `expPr` per side) and the method signatures document, but do not compute,
//! the flux balance above. Every method body is `unimplemented!()`.

use uom::si::f64::{Length, Ratio, ThermalConductivity, ThermodynamicTemperature};

use super::super::units::HeatTransferCoefficient;

/// Which side of the baffle (master or slave patch) a query refers to.
///
/// Mirrors upstream's `owner()` distinction between the patch that owns the
/// dictionary's `thickness`/`kappa` entries (the master, `samplePatch`
/// pointing at the slave) and the patch that reads them from its neighbour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaffleSide {
    /// The patch that declares `thickness` and `kappa` directly.
    Master,
    /// The patch that reads `thickness`/`kappa` from its `samplePatch` (the
    /// master).
    Slave,
}

/// Nusselt-number correlation coefficients, `Nu = const + coeff * Re^expRe *
/// Pr^expPr` — one instance per baffle side.
///
/// Mirrors the upstream `const`, `coeff`, `expRe`, `expPr` dictionary entries
/// verbatim. All four are dimensionless (the correlation itself is
/// dimensionless; `Re` and `Pr` are dimensionless inputs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NusseltCorrelationCoefficients {
    /// The additive constant term in the Nusselt correlation.
    pub const_term: f64,
    /// The multiplicative coefficient on `Re^expRe * Pr^expPr`.
    pub coeff: f64,
    /// The Reynolds-number exponent.
    pub exp_reynolds: f64,
    /// The Prandtl-number exponent.
    pub exp_prandtl: f64,
}

/// Placeholder for the (not yet ported) 1-D through-wall conduction baffle.
///
/// See the [module documentation](self) for why this is a data-only skeleton:
/// every method that would evaluate the coupled flux balance is
/// `unimplemented!()`. Do not call the methods below expecting a result —
/// they exist only to fix the public API shape ahead of the real port (bead
/// op-p6p.7.13 follow-up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NusseltThermalBaffle1DBc {
    /// Baffle (wall) thickness, upstream `thickness`.
    pub thickness: Length,
    /// Baffle (wall) thermal conductivity, upstream `kappa`.
    pub wall_conductivity: ThermalConductivity,
    /// Nusselt correlation coefficients for the master-side convective
    /// coupling.
    pub master_correlation: NusseltCorrelationCoefficients,
    /// Nusselt correlation coefficients for the slave-side convective
    /// coupling (defaults to `master_correlation` upstream when the
    /// dictionary omits them — that default-inheritance is case-I/O and is
    /// the caller's responsibility here, not this type's).
    pub slave_correlation: NusseltCorrelationCoefficients,
}

impl NusseltThermalBaffle1DBc {
    /// Build a baffle BC from its dictionary entries. Pure data assembly — no
    /// physics is evaluated by this constructor.
    #[must_use]
    pub fn new(
        thickness: Length,
        wall_conductivity: ThermalConductivity,
        master_correlation: NusseltCorrelationCoefficients,
        slave_correlation: NusseltCorrelationCoefficients,
    ) -> Self {
        Self {
            thickness,
            wall_conductivity,
            master_correlation,
            slave_correlation,
        }
    }

    /// The convective heat-transfer coefficient on one side of the baffle,
    /// `h = Nu(Re, Pr) * k_fluid / D_h`.
    ///
    /// Signature only — port of upstream `calcH`. **Not implemented.**
    // TODO(genfoam): full NusseltThermalBaffle1D port (bead op-p6p.7.13 follow-up).
    pub fn convective_htc(
        &self,
        _side: BaffleSide,
        _reynolds: Ratio,
        _prandtl: Ratio,
        _fluid_conductivity: ThermalConductivity,
        _hydraulic_diameter: Length,
    ) -> HeatTransferCoefficient {
        unimplemented!(
            "NusseltThermalBaffle1D::convective_htc — full port not done yet, bead op-p6p.7.13 follow-up"
        )
    }

    /// The coupled master/slave wall-face temperatures solving the
    /// three-way flux balance in the [module documentation](self):
    /// `h_master*(T_f,m - T_w,m) = (kappa/thickness)*(T_w,m - T_w,s) =
    /// h_slave*(T_w,s - T_f,s)`.
    ///
    /// Signature only — port of upstream `updateCoeffs`. **Not implemented.**
    // TODO(genfoam): full NusseltThermalBaffle1D port (bead op-p6p.7.13 follow-up).
    pub fn coupled_wall_temperatures(
        &self,
        _master_fluid_temperature: ThermodynamicTemperature,
        _slave_fluid_temperature: ThermodynamicTemperature,
        _master_htc: HeatTransferCoefficient,
        _slave_htc: HeatTransferCoefficient,
    ) -> (ThermodynamicTemperature, ThermodynamicTemperature) {
        unimplemented!(
            "NusseltThermalBaffle1D::coupled_wall_temperatures — full port not done yet, bead op-p6p.7.13 follow-up"
        )
    }
}
