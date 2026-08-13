//! Stage 1 — the faithful translation of Than Yan Ren's MATLAB.
//!
//! # Translation rules
//!
//! These are not style preferences; they are what makes the translation
//! checkable. See `docs/bedok-port-scoping.md` §1.
//!
//! - **Do not re-architect.** Keep the solver structure, iteration order and
//!   convergence logic as they are in the MATLAB.
//! - **Do not optimise.** A faster formulation that reorders floating-point
//!   accumulation defeats the purpose of a reference.
//! - **Do not substitute** OUTRAM PARK libraries here. The one decided
//!   exception is IAPWS-IF97, which comes from `tampines-steam-tables` rather
//!   than being ported from the third-party MATLAB file.
//! - **Do not fix what looks wrong.** Record it in the doc comment where it
//!   occurs and leave the behaviour alone.
//!
//! # Naming
//!
//! Every ported item carries a descriptive Rust name, and names its MATLAB
//! origin in the doc comment: the `.m` file and the original function. The
//! MATLAB names (`calc_a1234_expansionxyz`, `makegradDxyz`) are provenance,
//! not API.

pub mod cases;
pub mod coupling;
pub mod fixtures;
pub mod grid;
pub mod nodal;
pub mod th;

pub use grid::{Geometry, Grid};
