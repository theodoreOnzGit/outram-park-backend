//! Convenience re-export surface for `tuas_boussinesq_solver`.
//!
//! Glob-import one of the submodules to pull in the crate's common public
//! types (control volumes, boundary conditions, materials, heat-transfer
//! entities and property helpers) without naming their full module paths.
//! `beta_testing` is the current, near-stable prelude; `alpha_nightly` is
//! reserved for unstable, in-development re-exports.
//!
//! The plain path works and is the one to reach for:
//!
//! ```
//! use tuas_boussinesq_solver::prelude::*;
//!
//! // Types come from the prelude alone -- no deep module paths needed.
//! let steel: SolidMaterial = SolidMaterial::SteelSS304L;
//! let therminol: LiquidMaterial = LiquidMaterial::TherminolVP1;
//! let _m: Material = steel.into();
//! let _l: Material = therminol.into();
//! ```

/// for code currently unstable and under development
pub mod alpha_nightly;
/// for code currently moving towards a stable API and read for public testing
pub mod beta_testing;

// The plain path has to work. `use tuas_boussinesq_solver::prelude::*;` is
// what a caller writes first, and before this re-export it imported *nothing*
// — not an error, just an empty scope, after which every type in the file
// failed to resolve with a message pointing at the use site rather than here.
// A prelude that silently imports nothing is worse than no prelude at all.
//
// `beta_testing` is the tier this forwards to, per the module docs above. The
// tiers remain individually importable for callers who want to pin one.
pub use beta_testing::*;
