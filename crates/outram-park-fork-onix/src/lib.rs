//! # outram-park-fork-onix
//!
//! Independent pure-Rust fork/translation of ONIX (open-source depletion/burnup;
//! MIT upstream) — Bateman/CRAM depletion + fission-product inventory for the
//! MSRE digital twin. Not affiliated with the ONIX project.
//!
//! > **⚠️ Untrusted AI-assisted draft — pending human V&V.** This first-pass
//! > port was produced with AI assistance and is untrusted draft material until
//! > a human reviews it (see the workspace `RESPONSIBLE_USE.md`). No human V&V.
//! > MSRE digital-twin epic `op-6w0`, bead `op-6w0.2`. Not for nuclear facility
//! > operation, reactor control, safety-critical, or licensing decisions.
//!
//! ## What this crate does (stand-alone / precomputed-input mode)
//!
//! Given a set of nuclides, their decay data (decay constants + branching),
//! one-group (or few-group collapsed) neutron-reaction rates, and fission
//! yields, this crate assembles the depletion (Bateman) matrix `A` (units
//! `1/s`) and computes the depleted inventory `n(Δt) = exp(A·Δt)·n0` using the
//! **order-16 CRAM** (Chebyshev Rational Approximation Method) solver — the same
//! algorithm and coefficients ONIX uses in `onix/salameche/cram.py`.
//!
//! It is a faithful port of ONIX's *depletion-math core* only. See "Scope" below
//! for what is deliberately **not** ported.
//!
//! ## Quick start
//!
//! ```
//! use outram_park_fork_onix::{
//!     DepletionSystem, DecayData, ReactionRates, FissionYields, Nuclide, DecayMode,
//! };
//!
//! // Two-step decay chain A -> B -> C (C stable).
//! let a = Nuclide::new(50, 100, 0);
//! let b = Nuclide::new(51, 100, 0);
//! let c = Nuclide::new(52, 100, 0);
//!
//! let mut sys = DepletionSystem::new();
//! sys.add_nuclide(a, DecayData::single_mode(1e-2, DecayMode::BetaMinus),
//!                 ReactionRates::none(), FissionYields::empty()).unwrap();
//! sys.add_nuclide(b, DecayData::single_mode(1e-3, DecayMode::BetaMinus),
//!                 ReactionRates::none(), FissionYields::empty()).unwrap();
//! sys.add_nuclide(c, DecayData::stable(),
//!                 ReactionRates::none(), FissionYields::empty()).unwrap();
//!
//! let n0 = sys.inventory_vector(&[(a, 1.0)]).unwrap();
//! let n = sys.deplete(&n0, 100.0).unwrap(); // deplete 100 s
//! // n[2] is the C inventory after 100 s.
//! # assert!(n[2] >= 0.0);
//! ```
//!
//! ## Scope — what is and is NOT ported
//!
//! **Ported:** nuclide identity ([`Nuclide`]), decay/transmutation channel
//! identity + daughter lookup ([`DecayMode`], [`ReactionChannel`]), burnup-matrix
//! assembly ([`DepletionSystem::build_matrix`]), the order-16 CRAM solver
//! ([`cram::cram16`]), and a stand-alone single/multi-step driver
//! ([`DepletionSystem`]).
//!
//! **NOT ported (out of scope for this first pass):** the OpenMC coupling
//! (`onix/couple/`); ONIX's nuclide-data libraries (decay, cross section,
//! fission-yield files under `onix/data/`) — the caller supplies precomputed
//! data instead; the predictor-corrector / higher-order flux approximations
//! (`burn_substep_pc`, `burn_substep_pcME4`); the order-48 CRAM (ONIX itself
//! only ships order-16); the full input/sequence/reporting machinery
//! (`onix/input.py`, `onix/sequence.py`, `onix/system.py`, `onix/utils/`).
#![forbid(unsafe_code)]

// `no_std` + `alloc` without the default `std` feature (gh:#1589, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]

extern crate alloc;
// `thiserror` without its `std` default (the root's `thiserror-nostd` key),
// under the name the code uses.
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use nostd_prelude::*;

pub mod chain;
pub mod cram;
pub mod driver;
pub mod matrix;
pub mod nuclide;
pub mod reactions;

// --- Flat public API re-exports (so users need one `use`, per the workspace
// "human interface layer" rule: an example must read top-to-bottom without
// hunting through modules). ---
pub use chain::{DecayData, FissionYields, ReactionRates};
pub use cram::{clamp_nonnegative, cram16, CramError};
pub use driver::{DepletionError, DepletionSystem, NuclideIndex};
pub use matrix::BurnupMatrix;
pub use nuclide::{Nuclide, ZamId};
pub use reactions::{DecayMode, ReactionChannel};
