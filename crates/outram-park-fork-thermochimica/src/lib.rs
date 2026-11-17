//! # outram-park-fork-thermochimica
//!
//! Independent pure-Rust fork/translation of ORNL Thermochimica (BSD-3) — molten-salt Gibbs-energy-minimisation thermochemistry (fission-product speciation, redox, solubility) for the MSRE digital twin. SCAFFOLD: no human V&V. Not affiliated with ORNL or the Thermochimica project.
//!
//! > **⚠️ Scaffold — unverified until validated.** Skeleton crate; the port is
//! > in progress (MSRE digital-twin epic `op-6w0`). No human V&V. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork.
//!
//! ## Modules
//!
//! - [`gem`] — the CALPHAD **Gibbs-energy-minimisation core**: an
//!   element-potential / Lagrange-multiplier minimiser over multiple phases
//!   with ideal and binary Redlich-Kister solution models. This is the first
//!   ported piece of Thermochimica (bead `op-6w0.1`); the ChemSage `.dat`
//!   parser and the sublattice / quasichemical solution models are not yet
//!   ported (see the module's scope notes).
#![forbid(unsafe_code)]

// `no_std` + `alloc` without the default `std` feature (gh:#1590, epic
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

pub mod gem;
