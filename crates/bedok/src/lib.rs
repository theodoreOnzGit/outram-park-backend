//! # BEDOK — systems-level multiphysics coupling
//!
//! Three-dimensional nodal-diffusion neutronics coupled to thermal hydraulics,
//! at the fidelity band **above one-dimensional neutronics and below CFD**.
//! For CFD-fidelity multiphysics coupling see GeN-Foam in
//! `outram-foam-appbuilder-lib`; for neutronics-and-nuclear-data integration
//! see `nee_soon`.
//!
//! ## Provenance
//!
//! This crate is a Rust translation of a MATLAB implementation by **Than Yan
//! Ren** (Singapore Nuclear Research and Safety Institute). Permission to
//! translate and to publish the result as open source under OUTRAM PARK was
//! given by the author and approved at project-lead level; see
//! `docs/bedok-port-scoping.md` §6.
//!
//! The original code was unfinished when it was handed over. Gaps are
//! translated **as they are** and marked in place; completing them is a
//! separate, separately documented step. See `docs/bedok-port-scoping.md` §1.0
//! for why that separation matters.
//!
//! ## The two paths
//!
//! - [`reference`] — the faithful translation. Structure, iteration order and
//!   convergence logic follow the MATLAB line for line. Not idiomatic, not
//!   optimised, deliberately.
//! - [`substituted`] — the same physics rebuilt on OUTRAM PARK libraries. Every
//!   component here must reproduce [`reference`] on the benchmark suite before
//!   it is accepted, and nothing may be improved before it has passed parity.
//!
//! Both paths coexist so parity tests can call them in the same process.
//!
//! ## Verification status
//!
//! **Unverified.** No part of this crate has been validated. Reference
//! fixtures under `tests/fixtures/` record what Yan Ren's implementation
//! produces; agreement with them shows the translation is faithful, which is
//! not the same as being correct. Benchmark comparison against the published
//! IAEA-3D and NEACRP results is a separate check.
//!
//! Not for nuclear facility operation, reactor control, licensing, or
//! safety-critical decisions.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod reference;
pub mod substituted;

pub use error::{BedokError, Result};
