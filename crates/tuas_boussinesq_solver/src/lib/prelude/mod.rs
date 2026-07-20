//! Convenience re-export surface for `tuas_boussinesq_solver`.
//!
//! Glob-import one of the submodules to pull in the crate's common public
//! types (control volumes, boundary conditions, materials, heat-transfer
//! entities and property helpers) without naming their full module paths.
//! `beta_testing` is the current, near-stable prelude; `alpha_nightly` is
//! reserved for unstable, in-development re-exports.

/// for code currently unstable and under development
pub mod alpha_nightly;
/// for code currently moving towards a stable API and read for public testing
pub mod beta_testing;
