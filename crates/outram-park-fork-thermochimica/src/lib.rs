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

pub mod gem;
