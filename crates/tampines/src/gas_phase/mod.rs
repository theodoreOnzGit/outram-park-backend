//! # Gas-phase thermal-hydraulics for the HTR-10 helium primary circuit
//!
//! Single-phase **compressible gas** component models, at the low Mach
//! numbers of a gas-cooled reactor primary loop.
//!
//! ## Why this module exists (scope ruling, 2026-08-11)
//!
//! [`tuas_boussinesq_solver`], which backs [`crate::single_phase`], models
//! nearly-**incompressible liquids**: its Boussinesq treatment assumes
//! density varies only weakly, and only in the buoyancy term. Helium at
//! 3.0 MPa spans a factor of **1.85 in density** across the HTR-10 core's
//! 250-700 C rise — `2.739989 kg/m^3` at 523.15 K down to
//! `1.478781 kg/m^3` at 973.15 K, a 46.0 % fall (measured 2026-08-11, see
//! [`properties`]) — so that assumption fails outright. Gas is therefore
//! **permanently out of TUAS scope**, and the gas layer belongs in TAMPINES
//! — the superset that composes TUAS, `outram-park-fork-coolprop` and the
//! steam tables.
//!
//! ## Low-Mach design assumption
//!
//! The HTR-10 primary circuit runs at Mach numbers between **1.4e-3**
//! (the pebble-bed core, measured in `outram-park-fork-coolprop`) and
//! **2.2e-2** (a 0.30 m hot duct at the design flow, measured 2026-08-11 —
//! see [`pipe`]). The models here are therefore built for
//! **low-Mach compressible flow**: density is a full function of `(T, p)`
//! via the Helmholtz equation of state, but the kinetic-energy and
//! compressibility-work terms in the energy equation are dropped as
//! negligible at `Ma^2 ~ 1e-6`. This is stated rather than silently
//! assumed, and each model repeats it where it bites.
//!
//! Consequently the `HybridAllMach` machinery in
//! `outram-park-fork-coolprop` is **not** needed for the primary circuit,
//! and neither is coolprop's 50 kg/m^3 density-taper window — HTR-10
//! helium runs at 1.23-2.74 kg/m^3, far below that floor, so the taper
//! would zero the all-Mach dissipation everywhere anyway.
//!
//! ## What belongs here
//!
//! - [`properties`] — helium thermophysical-property adapter over
//!   `outram-park-fork-coolprop` (density, viscosity, conductivity, cp,
//!   Prandtl, speed of sound) at a `(T, p)` state point.
//! - [`pipe`] — steady-state gas duct: Darcy friction pressure drop
//!   (Churchill) and convective heat transfer (Dittus-Boelter /
//!   Gnielinski).
//! - [`kta_bed`] — KTA 3102.3 packed-bed (pebble-bed) pressure drop.
//! - [`circulator`] — idealised helium circulator (fixed pressure rise or
//!   fixed mass flow, isentropic-efficiency temperature rise).
//!
//! ## What does NOT belong here
//!
//! - **Property correlations** — those live in
//!   `outram-park-fork-coolprop`; [`properties`] only adapts them.
//! - **Liquid thermal-hydraulics** — [`crate::single_phase`] (TUAS) and
//!   [`crate::hem`] (steam/water) own those.
//! - **Two-phase flow** — helium in the primary circuit is single-phase
//!   supercritical gas by a wide margin; [`crate::multiphase_1d`] owns
//!   two-phase.
//! - **Bed *conduction*** — the Zehner-Bauer-Schlunder effective
//!   conductivity lives in [`crate::pebble_bed::zbs`]. This module owns the
//!   bed's *friction* side only.
//! - **Reactor physics** — neutronics belongs to `teh-o-prke`,
//!   `outram-mc-libs` and `njoy-outram-park-fork`.
//!
//! ## Module placement note (2026-08-11)
//!
//! [`crate::pebble_bed`]'s module docs list `kta` as a *planned* module
//! **there**. This session delivers it as [`kta_bed`] **here** instead,
//! because the KTA correlation is a gas-side friction closure that needs
//! [`properties`] and shares the low-Mach framing with [`pipe`], whereas
//! `pebble_bed` is the *conduction* stack. The stale note in
//! `pebble_bed/mod.rs` is left for the maintainer to reconcile.
//!
//! ## Status
//!
//! **NOT VALIDATED against HTR-10 measurements.** The word "validated" is
//! not used for any HTR-10-specific claim in this module. [`kta_bed`] is
//! *verified against the VTB gold values*; everything else carries
//! code-to-code and analytic-limit checks only. AI-assisted draft pending
//! human review per `RESPONSIBLE_USE.md`.

pub mod circulator;
pub mod kta_bed;
pub mod pipe;
pub mod properties;

pub use circulator::{Circulator, CirculatorDuty};
pub use kta_bed::{KtaBed, KtaBedResult};
pub use pipe::{GasDuct, GasDuctResult, HeatTransferCorrelation};
pub use properties::{
    helium_cp, helium_density, helium_prandtl, helium_state, helium_thermal_conductivity,
    helium_viscosity, HeliumState, SpecificEnthalpy,
};

use uom::si::{Quantity, ISQ, SI};
use uom::typenum::{N1, N2, P1, Z0};

/// Mass flux `G = mdot / A`, kg/(m^2 s) — mass flow per unit cross-section.
///
/// `uom` ships no named alias for this dimension, so it is spelled out
/// here. The same alias is defined in the read-only
/// `outram-park-digital-twin-engine` KTA module; this is an independent
/// definition of the identical dimension, not a shared type.
pub type MassFlux = Quantity<ISQ<N2, P1, N1, Z0, Z0, Z0, Z0>, SI<f64>, f64>;

/// Pressure gradient `dp/dx`, Pa/m.
///
/// `uom` ships no named alias for this dimension, so it is spelled out
/// here.
pub type PressureGradient = Quantity<ISQ<N2, P1, N2, Z0, Z0, Z0, Z0>, SI<f64>, f64>;
