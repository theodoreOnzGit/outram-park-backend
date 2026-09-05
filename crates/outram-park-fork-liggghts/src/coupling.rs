// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Ports/reimplements algorithms from LIGGGHTS-PUBLIC (CFDEMproject), whose
// source headers declare "GNU Public License, version 2 or later" — GPL-3
// compatible. See the crate NOTICE. Confirm each ported file's "or later"
// header and record its provenance.
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

//! Phase 5 — **Future CFD-DEM coupling** (bead `op-t3l.5`).
//!
//! # ⚠️ RESERVED ARCHITECTURE ONLY — NO PHYSICS IS IMPLEMENTED HERE
//!
//! This module is a **deliberately minimal, documentation-first interface
//! layer**. It reserves the *seam* where the OUTRAM-FOAM CFD/multiphase side
//! (crate `outram-foam-multiphase`, bead `op-2kk`) will one day exchange data
//! with this granular-DEM library (beads `op-t3l.1`–`op-t3l.4`). It defines the
//! **shape** of that exchange — enums, traits, and unit-typed data records —
//! and **nothing else**. Every behavioural method returns
//! [`DemError::NotImplemented`]. There is **no drag law, no interpolation, no
//! volume averaging, and no fluid solve** in this file, and none is faked: an
//! honest `NotImplemented` is the *correct and intended* state of this phase,
//! not a shortfall. Do not read any returned number as physically meaningful —
//! there are none to return yet.
//!
//! # Why a seam, not a dependency (Phase II separation principle)
//!
//! The three OUTRAM PARK physics pillars — thermophysical properties
//! (`tampines`), CFD/multiphase (`outram-foam-multiphase`),
//! and granular DEM (**this crate**) — are kept as **independent** crates. This
//! module therefore **must not** add a dependency on the CFD crate: coupling is
//! expressed as an *interface* (traits the CFD side will implement, traits the
//! DEM side will implement) rather than a compile-time link. That keeps each
//! pillar independently buildable, testable, and publishable, and keeps this
//! crate Android-buildable (pure-Rust, no CFD/BLAS pull-in). The two sides meet
//! only at run time, through the trait objects-by-generics wiring sketched in
//! [`ReservedCoupling::couple_particle`] — no `dyn`, no `Box`, per the
//! workspace design rules.
//!
//! # Intended volume-averaging / interpolation seam (design note)
//!
//! Unresolved (point-particle) CFD-DEM coupling exchanges two kinds of data at
//! every DEM particle, and both cross a **spatial-averaging boundary** that is
//! the crux of the eventual implementation:
//!
//! - **CFD → DEM (interpolation / sampling).** The fluid solver holds cell-
//!   averaged fields (velocity, pressure, void fraction). To force a *particle*
//!   the DEM side needs those fields **interpolated to the particle centre**
//!   (or, more carefully, filtered over the particle's neighbourhood so the
//!   particle does not "feel" its own back-reaction). [`LocalFluidState`]
//!   is the reserved record for that sampled snapshot at one particle.
//! - **DEM → CFD (volume averaging / projection).** The momentum the particles
//!   remove from the fluid, and the space they occupy, must be **averaged back
//!   onto the CFD mesh** — a per-cell solid volume fraction and a per-cell
//!   momentum sink. [`CouplingExchange`] is the reserved record for one
//!   particle's contribution: the drag force it received, and the particle
//!   volume fraction it projects back.
//!
//! The averaging kernel (nearest-cell, divided/diffused, statistical-kernel,
//! or coarse-grained "parcel" filtering), the void-fraction definition, and the
//! two-way momentum-conservation bookkeeping are **the physics to be designed
//! later**, once both the CFD side (`op-2kk`) and the DEM side (`op-t3l.1`
//! through `op-t3l.4`: particles, contact, boundaries, thermal) have matured
//! enough to pin the data contract down. Until then this module only *names*
//! the quantities and their units so that day-one implementation has a typed,
//! documented target to fill in.
//!
//! # Selected literature (public; for the eventual implementation)
//!
//! - R. Sun and H. Xiao, "Diffusion-based coarse graining in hybrid
//!   continuum–discrete solvers," *Int. J. Multiphase Flow* **72**, 233–247
//!   (2015).
//! - Z. Peng et al., "Influence of void fraction calculation on the numerical
//!   simulation of gas–solid flows by CFD-DEM," *Powder Technol.* **265**,
//!   26–39 (2014).
//! - R. Garg, J. Galvin, T. Li, S. Pannala, "Open-source MFIX-DEM software for
//!   gas–solids flows," *Powder Technol.* **220**, 122–137 (2012).
//! - C. Goniva, C. Kloss, N. G. Deen, J. A. M. Kuipers, S. Pirker,
//!   "Influence of rolling friction on single spout fluidized bed simulation,"
//!   *Particuology* **10**(5), 582–591 (2012) — the CFDEM/LIGGGHTS coupling
//!   approach this seam is modelled after.

use uom::si::f64::{Ratio, Volume};

use crate::particle::{Particle, Vec3};
use crate::DemError;

/// The direction and fidelity of momentum exchange across the CFD-DEM seam.
///
/// This closed set of coupling regimes is dispatched by `match` (enum dispatch
/// per the workspace design rules — no trait objects). It selects *which* of
/// the reserved data paths a future driver would actually walk; it carries no
/// physics itself.
///
/// # Variants
///
/// - [`OneWay`](CouplingScheme::OneWay) — the fluid drives the particles but the
///   particles do **not** feed momentum or volume back to the fluid. The CFD
///   solve is independent of the DEM state; only the CFD → DEM interpolation
///   path (sampling [`LocalFluidState`]) is exercised. Valid physical regime:
///   dilute suspensions where the particle volume fraction is small enough
///   (typically a solid fraction `≲ 1e-3`) that back-reaction is negligible.
/// - [`TwoWay`](CouplingScheme::TwoWay) — momentum is exchanged in **both**
///   directions: the fluid drags the particles, and each particle's reaction
///   force is projected back onto the fluid as a per-cell momentum sink. Both
///   the interpolation and the volume-averaging/projection paths are exercised.
///   Valid regime: moderate loadings where drag back-reaction matters but the
///   local void fraction is still near unity.
/// - [`VolumeFiltered`](CouplingScheme::VolumeFiltered) — full "four-way"-style
///   volume-filtered / coarse-grained coupling: in addition to two-way momentum
///   exchange, the fluid equations carry the **particle volume fraction**
///   explicitly (the void fraction departs meaningfully from unity), so the
///   solid phase displaces fluid. This is the dense-bed regime (e.g. a
///   pebble/packed bed), and the one where the volume-averaging kernel choice
///   (see the module design note) dominates accuracy.
///
/// # Status
///
/// Reserved only. No variant drives any computation in this phase — the enum
/// exists so the eventual driver, and the trait method docs, can name the
/// regime they apply to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CouplingScheme {
    /// Fluid → particles only; no back-reaction (dilute regime). See the type
    /// docs for the valid physical range.
    OneWay,
    /// Bidirectional momentum exchange; void fraction still ≈ 1 (moderate
    /// loading). See the type docs.
    TwoWay,
    /// Volume-filtered / coarse-grained; particle volume fraction enters the
    /// fluid equations (dense regime). See the type docs.
    VolumeFiltered,
}

/// A snapshot of the **CFD fluid field sampled at one particle's location** —
/// the data the CFD side (`outram-foam-multiphase`, bead `op-2kk`) would
/// provide to the DEM side each coupling step (the CFD → DEM interpolation
/// path).
///
/// This is a plain unit-carrying record: it holds *what the fluid looks like*
/// at a single point, already interpolated/filtered from the CFD mesh by the
/// provider. It performs no interpolation itself (that is the provider's job)
/// and no physics.
///
/// # Fields, quantities, and units
///
/// | Field | Physical quantity | Unit | Valid range |
/// |---|---|---|---|
/// | `velocity` | local fluid (continuous-phase) velocity `u_f` | `[m/s]` | any finite vector |
/// | `pressure_gradient` | local fluid pressure gradient `∇p` | `[Pa/m]` | any finite vector |
/// | `void_fraction` | fluid volume fraction `ε_f` (fraction of the local cell occupied by fluid) | dimensionless | `(0, 1]` |
///
/// The two vector quantities use [`Vec3`] (unitless container; the SI unit is
/// fixed by this documentation, following the crate convention in
/// [`crate::particle`]). `void_fraction` uses the `uom` dimensionless
/// [`Ratio`] so it cannot be confused with a dimensional scalar at a call site;
/// physically it satisfies `ε_f = 1 - ε_s` with the particle (solid) volume
/// fraction `ε_s`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalFluidState {
    /// Local fluid velocity `u_f` `[m/s]` at the particle centre (interpolated
    /// from the CFD field by the provider).
    pub velocity: Vec3,
    /// Local fluid pressure gradient `∇p` `[Pa/m]` at the particle centre.
    pub pressure_gradient: Vec3,
    /// Local fluid volume fraction `ε_f` (dimensionless, `(0, 1]`): the fraction
    /// of the surrounding averaging volume occupied by fluid rather than solid.
    pub void_fraction: Ratio,
}

/// One particle's **contribution back to the CFD solve** — the data the DEM
/// side returns each coupling step (the DEM → CFD volume-averaging/projection
/// path).
///
/// # Fields, quantities, and units
///
/// | Field | Physical quantity | Unit | Sign / range |
/// |---|---|---|---|
/// | `drag_force` | fluid → particle drag force applied to the DEM particle | `[N]` | any finite vector; its negative is the momentum sink the fluid feels |
/// | `particle_volume_fraction` | solid volume fraction `ε_s` this particle projects onto its CFD cell(s) | dimensionless | `[0, 1)` |
///
/// The `drag_force` is what the DEM integrator would add to a particle's force
/// balance; by Newton's third law the equal-and-opposite reaction is the
/// per-cell momentum sink handed to the fluid under
/// [`CouplingScheme::TwoWay`]/[`CouplingScheme::VolumeFiltered`].
/// `particle_volume_fraction` is `ε_s = 1 - ε_f`, the quantity the fluid
/// continuity/momentum equations need under [`CouplingScheme::VolumeFiltered`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CouplingExchange {
    /// Fluid → particle drag force `[N]` applied to the DEM particle this step.
    pub drag_force: Vec3,
    /// Particle (solid) volume fraction `ε_s` (dimensionless, `[0, 1)`) this
    /// particle projects back onto the CFD mesh.
    pub particle_volume_fraction: Ratio,
}

/// The **CFD side** of the seam: a source of interpolated fluid state at a
/// point. The future `outram-foam-multiphase` crate would implement this trait;
/// this crate only *declares* it, so the two pillars stay decoupled at compile
/// time (Phase II separation principle).
///
/// It is a compiler-enforced contract, **not** a dynamic-dispatch boundary —
/// consumers take `impl FluidCouplingSource` / a generic `F:
/// FluidCouplingSource` (see [`ReservedCoupling::couple_particle`]), never
/// `dyn`/`Box`, per the workspace design rules.
///
/// # Status
///
/// Reserved only. No implementor in this crate does real interpolation; the
/// bundled [`ReservedFluidSource`] returns [`DemError::NotImplemented`].
pub trait FluidCouplingSource {
    /// Sample the CFD fluid field at `position` `[m]` (a particle centre),
    /// returning the interpolated [`LocalFluidState`] (fluid velocity `[m/s]`,
    /// pressure gradient `[Pa/m]`, void fraction, dimensionless).
    ///
    /// The `position` is a world-space point in metres, expected to lie inside
    /// the CFD domain; a point outside the meshed region is the provider's to
    /// reject.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::NotImplemented`] in this reserved phase. A real
    /// implementor would additionally error (e.g. [`DemError::InvalidInput`])
    /// when `position` falls outside the CFD domain or the field is unavailable.
    fn sample_fluid_state(&self, position: Vec3) -> Result<LocalFluidState, DemError>;
}

/// The **DEM side** of the seam: given a particle and the fluid state at it,
/// produce the momentum/volume feedback for the CFD solve. This crate would
/// implement this trait once a drag closure and a volume-averaging kernel
/// exist (Phase 5 physics, deferred).
///
/// Like [`FluidCouplingSource`] it is a compiler-enforced contract used through
/// generics, never `dyn`/`Box`.
///
/// # Status
///
/// Reserved only. The bundled [`ReservedDragModel`] returns
/// [`DemError::NotImplemented`] from every method — there is no drag physics in
/// this phase.
pub trait DemCouplingResponse {
    /// Compute the **fluid → particle drag force** `[N]` on `particle` given the
    /// locally sampled `fluid` state.
    ///
    /// A real implementor would evaluate a void-fraction-corrected drag
    /// correlation (e.g. Gidaspow / Beetstra-van der Hoef-Kuipers), using the
    /// slip velocity `u_f - v_p` `[m/s]` (from `fluid.velocity` and
    /// `particle.velocity`), the particle radius/volume (from `particle`), and
    /// `fluid.void_fraction`. The pressure-gradient (buoyancy/`∇p`) contribution
    /// would also enter here or in a companion term. **None of that is done in
    /// this phase.**
    ///
    /// # Errors
    ///
    /// Returns [`DemError::NotImplemented`] in this reserved phase.
    fn drag_force(&self, particle: &Particle, fluid: &LocalFluidState) -> Result<Vec3, DemError>;

    /// Compute the **particle (solid) volume fraction** `ε_s` (dimensionless,
    /// `[0, 1)`) that `particle` projects back onto the CFD mesh, given the
    /// local `averaging_volume` `[m³]` over which the projection is taken.
    ///
    /// This is the DEM → CFD volume-averaging path: the particle's own volume
    /// (`(4/3)πr³`, from `particle`) divided/filtered over `averaging_volume`
    /// by the eventual averaging kernel. The naive point estimate is
    /// `ε_s = V_particle / averaging_volume`, but the real definition depends on
    /// the kernel chosen (see the module design note), so this stays reserved.
    ///
    /// # Errors
    ///
    /// Returns [`DemError::NotImplemented`] in this reserved phase. A real
    /// implementor would also reject a non-positive `averaging_volume` with
    /// [`DemError::InvalidInput`].
    fn particle_volume_fraction(
        &self,
        particle: &Particle,
        averaging_volume: Volume,
    ) -> Result<Ratio, DemError>;
}

/// A reserved stand-in for the future CFD provider, so this crate's tests and
/// docs can name the CFD side of the seam **without** depending on
/// `outram-foam-multiphase`.
///
/// It implements [`FluidCouplingSource`] with a body that returns
/// [`DemError::NotImplemented`] — it holds no field data and computes nothing.
/// The real provider lives in the CFD crate; this exists only to make the seam
/// constructible and testable here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReservedFluidSource;

impl FluidCouplingSource for ReservedFluidSource {
    fn sample_fluid_state(&self, _position: Vec3) -> Result<LocalFluidState, DemError> {
        Err(DemError::NotImplemented(
            "CFD→DEM fluid-state sampling is reserved architecture (Phase 5, \
             bead op-t3l.5); no interpolation is implemented — deferred until \
             the CFD side (op-2kk) matures"
                .to_string(),
        ))
    }
}

/// A reserved stand-in for the future DEM drag/volume model, implementing
/// [`DemCouplingResponse`] with [`DemError::NotImplemented`] bodies.
///
/// No drag correlation and no volume-averaging kernel are implemented — this is
/// the reserved DEM half of the seam, present so the interface is constructible
/// and testable in this phase. Real physics is deferred (Phase 5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReservedDragModel;

impl DemCouplingResponse for ReservedDragModel {
    fn drag_force(&self, _particle: &Particle, _fluid: &LocalFluidState) -> Result<Vec3, DemError> {
        Err(DemError::NotImplemented(
            "fluid→particle drag force is reserved architecture (Phase 5, bead \
             op-t3l.5); no drag correlation is implemented — deferred, and \
             deliberately not faked"
                .to_string(),
        ))
    }

    fn particle_volume_fraction(
        &self,
        _particle: &Particle,
        _averaging_volume: Volume,
    ) -> Result<Ratio, DemError> {
        Err(DemError::NotImplemented(
            "particle→CFD volume-fraction projection is reserved architecture \
             (Phase 5, bead op-t3l.5); no volume-averaging kernel is \
             implemented — deferred"
                .to_string(),
        ))
    }
}

/// The reserved **coupling driver**: it names a [`CouplingScheme`] and sketches
/// how the two sides of the seam would compose into one particle's exchange,
/// **without** implementing the exchange.
///
/// This type exists to fix the *wiring shape* — how a [`FluidCouplingSource`]
/// and a [`DemCouplingResponse`] combine per particle — using generics (no
/// `dyn`, no `Box`) so both sides stay statically dispatched and this crate
/// stays independent of the CFD crate.
///
/// # Status
///
/// Reserved only. [`couple_particle`](ReservedCoupling::couple_particle)
/// returns [`DemError::NotImplemented`]; no coupling loop runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReservedCoupling {
    /// The coupling regime this driver would apply (see [`CouplingScheme`]).
    pub scheme: CouplingScheme,
}

impl ReservedCoupling {
    /// Construct a reserved driver for the given [`CouplingScheme`].
    #[must_use]
    pub const fn new(scheme: CouplingScheme) -> Self {
        Self { scheme }
    }

    /// Sketch of **one particle's coupling step**: sample the fluid at the
    /// particle from `fluid_source`, hand it to `drag_model` to obtain the
    /// drag force and the projected particle volume fraction, and bundle both
    /// into a [`CouplingExchange`].
    ///
    /// The generic bounds (`F: FluidCouplingSource`, `D: DemCouplingResponse`)
    /// are how the CFD and DEM sides meet *without* a compile-time dependency
    /// between the crates and *without* trait objects. `averaging_volume` `[m³]`
    /// is the local control/averaging volume used for the volume-fraction
    /// projection (see the module design note).
    ///
    /// # Errors
    ///
    /// Returns [`DemError::NotImplemented`] in this reserved phase — the method
    /// signature shows how the pieces compose, but no data is exchanged. (The
    /// error message names `self.scheme` so the reserved regime is visible.) A
    /// future implementation would, under [`CouplingScheme::OneWay`], skip the
    /// volume-fraction projection, and under the other schemes perform both
    /// paths.
    pub fn couple_particle<F, D>(
        &self,
        particle: &Particle,
        fluid_source: &F,
        drag_model: &D,
        averaging_volume: Volume,
    ) -> Result<CouplingExchange, DemError>
    where
        F: FluidCouplingSource,
        D: DemCouplingResponse,
    {
        // Reserved: discard the inputs unused (no exchange is performed) and
        // report the honest not-implemented state, naming the regime.
        let _ = (particle, fluid_source, drag_model, averaging_volume);
        Err(DemError::NotImplemented(format!(
            "CFD-DEM coupling step ({:?}) is reserved architecture (Phase 5, \
             bead op-t3l.5); the seam is defined but no exchange is implemented \
             — deferred until both the CFD (op-2kk) and DEM (op-t3l.1–.4) \
             sides mature",
            self.scheme
        )))
    }
}

#[cfg(test)]
mod tests {
    //! Interface-surface tests for the **reserved** CFD-DEM coupling seam.
    //!
    //! # Scope — no physics V&V (there is none this phase)
    //!
    //! These tests verify only that the reserved *interface* exists, is
    //! constructible, composes as intended, and errors cleanly with
    //! [`DemError::NotImplemented`]. There is **no** verification or validation
    //! of coupling physics here because **no coupling physics is implemented** —
    //! any physics V&V is deferred to the eventual Phase 5 implementation. A
    //! passing suite here means "the seam is shaped and honest", not "coupling
    //! works".

    use super::*;
    use crate::particle::Vec3;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::kelvin;
    use uom::si::volume::cubic_meter;

    /// Helper: a valid particle to feed the (reserved) DEM side.
    fn test_particle() -> Particle {
        Particle::new(
            Vec3::zero(),
            Vec3::new(0.1, 0.0, 0.0),
            Vec3::zero(),
            Mass::new::<kilogram>(1.0e-3),
            Length::new::<meter>(1.0e-3),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid particle")
    }

    /// Helper: a sampled fluid state (plain data record — always constructible).
    fn test_fluid_state() -> LocalFluidState {
        LocalFluidState {
            velocity: Vec3::new(1.0, 0.0, 0.0),
            pressure_gradient: Vec3::new(-100.0, 0.0, 0.0),
            void_fraction: Ratio::new::<ratio>(0.6),
        }
    }

    /// The [`CouplingScheme`] enum surface exists and every reserved variant is
    /// constructible and distinct.
    #[test]
    fn coupling_scheme_variants_exist_and_differ() {
        let one = CouplingScheme::OneWay;
        let two = CouplingScheme::TwoWay;
        let vf = CouplingScheme::VolumeFiltered;
        assert_ne!(one, two);
        assert_ne!(two, vf);
        assert_ne!(one, vf);
        // Copy + Debug are available (used by the driver's error message).
        assert_eq!(one, one);
        assert_eq!(format!("{two:?}"), "TwoWay");
    }

    /// The plain data records compose and carry their documented units.
    #[test]
    fn data_records_are_constructible() {
        let fluid = test_fluid_state();
        assert_eq!(fluid.void_fraction.get::<ratio>(), 0.6);
        let exchange = CouplingExchange {
            drag_force: Vec3::zero(),
            particle_volume_fraction: Ratio::new::<ratio>(0.4),
        };
        assert_eq!(exchange.particle_volume_fraction.get::<ratio>(), 0.4);
    }

    /// The CFD-side trait is implemented by the reserved stand-in, and its
    /// method errors cleanly with [`DemError::NotImplemented`] (not a panic, not
    /// a fabricated value).
    #[test]
    fn reserved_fluid_source_errors_not_implemented() {
        let src = ReservedFluidSource;
        let out = src.sample_fluid_state(Vec3::zero());
        assert!(matches!(out, Err(DemError::NotImplemented(_))));
    }

    /// The DEM-side trait is implemented by the reserved stand-in, and both of
    /// its methods error cleanly with [`DemError::NotImplemented`].
    #[test]
    fn reserved_drag_model_errors_not_implemented() {
        let model = ReservedDragModel;
        let p = test_particle();
        let fluid = test_fluid_state();

        let drag = model.drag_force(&p, &fluid);
        assert!(matches!(drag, Err(DemError::NotImplemented(_))));

        let frac = model.particle_volume_fraction(&p, Volume::new::<cubic_meter>(1.0e-6));
        assert!(matches!(frac, Err(DemError::NotImplemented(_))));
    }

    /// **Composition (doc-level) test**: the whole seam wires together as
    /// intended — a [`CouplingScheme`], a [`FluidCouplingSource`], and a
    /// [`DemCouplingResponse`] compose through [`ReservedCoupling`] into one
    /// call that would yield a [`CouplingExchange`]. In this reserved phase the
    /// call errors cleanly with [`DemError::NotImplemented`]; the point of the
    /// test is that the *types compose* (the generic bounds are satisfiable and
    /// the signatures line up), not that any exchange happens.
    #[test]
    fn seam_composes_and_errors_not_implemented() {
        let driver = ReservedCoupling::new(CouplingScheme::TwoWay);
        let fluid_source = ReservedFluidSource;
        let drag_model = ReservedDragModel;
        let particle = test_particle();

        let out = driver.couple_particle(
            &particle,
            &fluid_source,
            &drag_model,
            Volume::new::<cubic_meter>(1.0e-6),
        );
        assert!(matches!(out, Err(DemError::NotImplemented(_))));

        // The reserved driver reports the regime it would have applied, so the
        // honest "not implemented" state stays legible to a caller.
        if let Err(DemError::NotImplemented(msg)) = out {
            assert!(msg.contains("TwoWay"));
        } else {
            panic!("reserved coupling must return NotImplemented");
        }
    }
}
