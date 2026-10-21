//! Shared helpers for the HEM-pipe integration tests.
//!
//! - [`toro_exact`]: the exact perfect-gas Riemann solver, copied from
//!   `outram-foam-appbuilder-lib` (see its header for provenance).
//! - [`hem_rarefaction`]: the semi-analytical HEM water double rarefaction.
//! - [`edwards_case`]: the Edwards–O'Brien case data, copied from
//!   `tests/edwards_blowdown.rs` (see its header).

#![allow(dead_code)]

pub mod edwards_case;
pub mod hem_rarefaction;
pub mod toro_exact;
