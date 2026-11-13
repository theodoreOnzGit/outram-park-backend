// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149), not a port of GeN-Foam. This file is part of OUTRAM
// PARK, distributed under GPL-3.0 like the rest of this crate's GeN-Foam
// module.

//! # `solid_conduction` — 2-D R-Z solid conduction with radiation (THERMIX-like)
//!
//! **New work, not port (gh:#1149, epic #1146, maintainer decision D2
//! 2026-11-03).** Upstream GeN-Foam has no multi-region solid conduction
//! field: its structures are lumped per cell ([`super::structure::PowerModel`])
//! with constant properties. This module adds, beside the port, the solid
//! energy equation a THERMIX-style pebble-bed analysis solves over the bed,
//! reflectors, barrel and vessel, as homogenised regions:
//!
//! ```text
//!   d H(T)/dt = div( lambda(T) grad T ) + q'''          H(T) = int rho c_p dT
//! ```
//!
//! on an axisymmetric wedge (or planar slab) [`RzMesh`], with
//!
//! - **temperature-dependent properties** per region, `lambda(T)` and
//!   `rho c_p (T)`, by enum dispatch over [`PropertyModel`] (constant, linear,
//!   polynomial, clamped table, or a `fn` pointer; gh:#1148's closures plug
//!   in as new variants);
//! - **Picard iteration** of the nonlinearity in every steady solve and every
//!   time step (properties, radiation and gap coefficients re-evaluated at
//!   the previous iterate until no cell moves more than
//!   [`SolidConductionSettings::picard_tolerance`], default 1e-8 K);
//! - the **enthalpy form** of storage in backward Euler,
//!   `C(T^k)(T^{k+1} - T^k) + H(T^k) - H(T^n)`, which conserves energy exactly
//!   at convergence for any `rho c_p (T)` (and reduces to `C (T^{k+1} - T^n)`,
//!   bit for bit, for a constant one);
//! - boundaries ([`SolidBoundary`]): adiabatic, fixed temperature, convective
//!   `(h, T_inf)`, radiative `(eps, T_inf)`, and radiative plus enclosure
//!   natural convection of air ([`EnclosureNaturalConvection`], gh:#1173);
//! - **gas gaps** between regions ([`GapConductance`]): gas conduction plus
//!   grey two-surface radiation (Kugeler et al. 2017 §9.4), as a series
//!   resistance on the faces between the two regions.
//!
//! ## How the nonlinear terms are linearised
//!
//! - A **radiative boundary** uses the tangent (Newton) form at the previous
//!   surface temperature, `q ~ q(T*) + 4 eps sigma T*^3 (T - T*)`, imposed as a
//!   Robin condition. The secant form `eps sigma (T*^2+Ta^2)(T*+Ta)(T - Ta)`
//!   was tried first and does not converge once `T_s > ~1.5 T_a` (its
//!   fixed-point slope is about `-3 (T_s - T_a)/T_s`); see test (a).
//! - A **gap** uses the secant (exact-factorisation) coefficient at the
//!   previous surface temperatures, because the tangent form is not a single
//!   symmetric face conductance. Its fixed-point slope is about
//!   `-3 dT_gap / (4 T_mean)`: it converged in 44 passes for a vacuum gap with
//!   456 K across it at 1400 K mean (test `vacuum_gap_with_large_temperature_drop_converges`),
//!   but a gap whose drop approaches its mean absolute temperature is outside
//!   what has been tested and may need under-relaxation.
//! - `lambda(T)` and `rho c_p (T)` are evaluated at cell temperatures of the
//!   previous pass; face conductivities are the harmonic (series) mean of the
//!   two cell values.
//!
//! ## What is reused
//!
//! | piece | from |
//! |---|---|
//! | implicit Laplacian, Robin (`MixedField`) and fixed-value boundary arms | `outram_foam_basic_lib::fvm::laplacian` (OpenFOAM `laplacianFoam`) |
//! | harmonic (series-resistance) face conductivity, `A/(d_P/k_P + d_N/k_N)` | the same construction as `outram_foam_basic_lib::steady_conduction` (OpenFOAM `harmonic`) |
//! | convective weight `h d/(h d + k)` | `externalWallHeatFluxTemperature`, as in basic-lib's steady conduction |
//! | DIC-preconditioned CG | `FvMatrix::solve_cg_with_guess` |
//! | `sigma` | the ported `blackbody_radiation` BC |
//! | mesh container | basic-lib's `ConductionMesh` (so the same mesh also runs basic-lib's steady solver, used here as a code-to-code check) |
//!
//! Checked and not reused: basic-lib's `steady_conduction` (steady only, no
//! radiation, gaps or storage; its 1-D radial and voxel meshes have no R-Z
//! form — it is extended around, not duplicated, and is the cross-check);
//! the GeN-Foam `thermo_mechanics::mesh_solve` `TEqn` (constant properties,
//! single region); `dhoby-ghaut`'s `porous_core.rs` (rings with no conduction
//! between them, inside a GUI binary); `dhoby-ghaut`'s `RzDomain` and
//! `nee_soon::htr10_rmc` (HTR-10 geometry for other codes). ~~The HTR-10
//! geometry comes in a later part of gh:#1149 and lives downstream of this
//! crate.~~ **CORRECTED 2026-11-03 (gh:#1149 part B):** it is in this crate,
//! [`crate::htr10_rz`], built from the cited input sheet on this field.
//!
//! **Held cells** ([`SolidConductionField::set_fixed_temperatures`], part B):
//! a Dirichlet condition on cells, for a gas side or a body whose
//! temperature is imposed; verified by
//! `held_cells_are_eliminated_and_their_duty_is_reported`.
//!
//! ## Verification
//!
//! Analytic cases (a)–(e) of gh:#1149 are in this module's `tests.rs`, each
//! with its methodology, the prediction written before the run, and the
//! measured result. The R-Z region map and mesh of the test geometry are
//! drawn from the assembled mesh by `tests/solid_conduction_rz_images.rs`
//! into `verification_and_validation/solid_conduction_rz/`.
//!
//! Status: untrusted AI-generated draft (2026-11-03) per `RESPONSIBLE_USE.md`.
//! For research, education and V&V only; not validated against any reactor.

pub mod gap;
pub mod properties;
pub mod rz_mesh;
mod solver;

#[cfg(test)]
mod tests;

pub use gap::{GapConductance, RegionGap};
pub use properties::{ConductivityModel, HeatCapacityModel, PropertyModel, PropertyTable};
pub use rz_mesh::{AxisEdges, RegionMap, RzGeometry, RzMesh, RzPatch, RzRect};
pub use solver::{
    air_properties, EnclosureNaturalConvection, PicardReport, RzBoundaries, SolidBoundary, SolidConductionField,
    SolidConductionSettings,
    SolidEnergyBalance, SolidRegion,
};

use thiserror::Error;

/// Volumetric heat-transfer coefficient, W/(m^3 K): a film coefficient times
/// the interfacial area per unit volume (`h a_v`), as a porous region's
/// solid-to-fluid exchange uses it. `uom` has no named alias for it.
pub type VolumetricHeatTransferCoefficient = uom::si::Quantity<
    uom::si::ISQ<
        uom::typenum::N1,
        uom::typenum::P1,
        uom::typenum::N3,
        uom::typenum::Z0,
        uom::typenum::N1,
        uom::typenum::Z0,
        uom::typenum::Z0,
    >,
    uom::si::SI<f64>,
    f64,
>;

/// What can go wrong setting up or solving the solid conduction field.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SolidConductionError {
    /// An input was out of range or inconsistent (the message says which).
    #[error("invalid solid-conduction input: {0}")]
    InvalidInput(String),
    /// The mesh failed validation.
    #[error("mesh assembly failed: {0}")]
    Mesh(String),
    /// A property model returned a non-positive or non-finite value.
    #[error("{property} of region {region} at {temperature_kelvin} K is {value}; it must be positive and finite")]
    BadProperty {
        /// Region index.
        region: usize,
        /// Which property.
        property: &'static str,
        /// Temperature it was asked at, K.
        temperature_kelvin: f64,
        /// What it returned (SI).
        value: f64,
    },
    /// The linear solve missed its tolerance.
    #[error("linear solve did not converge: residual {residual:e} after {iterations} iterations")]
    LinearSolveFailed {
        /// Normalised residual at exit.
        residual: f64,
        /// Iterations performed.
        iterations: usize,
    },
    /// The Picard iteration did not settle.
    #[error("Picard iteration did not converge: last change {last_change_kelvin:e} K after {iterations} passes")]
    PicardNotConverged {
        /// Largest cell change of the last pass, K.
        last_change_kelvin: f64,
        /// Passes performed.
        iterations: usize,
    },
}
