//! # PUNGGOL
//!
//! **P**article transport, **UN**certainty, **G**eometry, **G**amma transport,
//! **O**ptimization **L**ibrary.
//!
//! Radiation transport beyond neutrons: photons (gammas), electrons and
//! positrons, coupled to `outram-mc-libs`' neutrons. The question it answers
//! is **"where do the gammas and betas go, and where do they deposit their
//! energy?"** (`docs/ecosystem-naming.md`).
//!
//! # Status
//!
//! - [`decay_spectrum`]: **decay-photon source spectra** from the depletion
//!   chain, a port of OpenMC's `openmc.data.decay_photon_energy`,
//!   `Material.get_decay_photon_energy` and `openmc.stats.DecaySpectrum`
//!   (gh:#1391). Checked against OpenMC, see
//!   `verification_and_validation/decay_photon_spectra/`.
//! - [`egsnrc`]: the EGSnrc port (gh:#976), ~~**scaffolded, nothing
//!   implemented**~~ **CORRECTED 2026-11-21 (gh:#1682):** PEGS4, HATCH, the
//!   interaction routines, `PHOTON`, `ELECTR`, `SHOWER`, the EM-field macros
//!   and `egs_parallel` are ported and differentially tested against a
//!   compiled EGSnrc (`docs/egsnrc_phase2_differential.md`; bit-identical
//!   with the `std` feature, to round-off without it); `rad_compton1` and
//!   `xsec_out` are not ported, on purpose.
//! - [`openmc_photon`]: the OpenMC photon interaction samplers
//!   (Klein-Nishina, Compton with Doppler broadening, Rayleigh, pair
//!   production, atomic relaxation), thick-target bremsstrahlung and the
//!   photon/electron/positron reaction sampling (gh:#975, gh:#1683).
//!   ~~**First port, not differentially tested.**~~ **CORRECTED 2026-11-21:**
//!   differentially tested against a built OpenMC for U and H
//!   (`docs/openmc_photon_phase2.md`); other elements are data-blocked
//!   (gh:#1695). Data is passed in; no HDF5 reader yet.
//! - ~~**No transport loop.**~~ **CORRECTED 2026-11-21:** EGSnrc's `PHOTON`,
//!   `ELECTR` and `SHOWER` are transport loops, behind the compile-time
//!   `Geometry` / `ElectronGeometry` seams; this crate supplies **no
//!   geometry** (the tests use a slab), so nothing here tracks a particle
//!   through a real geometry.
//!
//! # Boundaries
//!
//! Neutrons stay in `outram-mc-libs`; **this crate depends on it, never the
//! reverse**. General numerics (generators, alias tables, interpolation) go
//! to `petir`. Ions stay in `boon-lay`.
//!
//! # `no_std`
//!
//! `no_std` + `alloc` without the default `std` feature, like the crates it
//! builds on; checked on `x86_64-unknown-none`.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports))]
#![forbid(unsafe_code)]

extern crate alloc;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import. Glob-imported by each module in `no_std` builds.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}

pub mod decay_spectrum;
pub mod egsnrc;
pub mod openmc_photon;

/// The scope this crate states, as a machine-readable string.
pub const SCOPE: &str =
    "Radiation transport beyond neutrons: decay-photon source spectra, the EGSnrc port and the OpenMC photon samplers (transport loops behind a geometry seam, no geometry supplied)";

#[cfg(test)]
mod tests {
    use super::*;

    /// The scope string names what exists and what does not.
    #[test]
    fn the_scope_is_recorded() {
        assert!(SCOPE.contains("decay-photon"));
        assert!(SCOPE.contains("no geometry supplied"));
    }
}
