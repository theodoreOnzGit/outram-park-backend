//! The NJOY processing modules and the driver that sequences them.
//!
//! Each upstream module (`RECONR`, `BROADR`, `ACER`, …) is an essentially
//! independent program that reads one or more ENDF tapes and writes another.
//! In Rust each becomes its own submodule with a `run` entry point operating on
//! [`crate::endf`] data.
//!
//! Per the workspace design rules, module selection is dispatched through an
//! **enum** ([`NjoyModule`]), not a trait object — the set of modules is closed
//! and known at compile time, so a missing `match` arm is a compile error.
//!
//! **Status:** scaffold. Only the Phase-1/2 modules have files; the rest are
//! listed in `docs/porting-plan.md` and added as they are ported.

pub mod reconr;
pub mod broadr;
pub mod acer;

/// The NJOY processing modules, in the order a typical ACE-generation run uses
/// them. Variants are added as modules are ported (see `docs/porting-plan.md`).
///
/// Dispatch is by `match`, not `dyn` — adding a variant forces every call site
/// to handle it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NjoyModule {
    /// Reconstruct pointwise cross sections from resonance parameters.
    Reconr,
    /// Doppler-broaden and thin pointwise cross sections.
    Broadr,
    /// Prepare an ACE library for continuous-energy Monte Carlo (e.g. OpenMC).
    Acer,
}
