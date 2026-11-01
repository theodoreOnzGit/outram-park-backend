//! Shared helpers for `tests/fem1d_vs_fv.rs` (gh:#1069), copied from
//! `tampines-steam-tables/tests/common/` (see each file's header):
//!
//! - [`toro_exact`]: the exact perfect-gas Riemann solver (Toro 2009 ch. 4).
//! - [`hem_rarefaction`]: the semi-analytical HEM water double rarefaction.
//! - [`edwards_case`]: the Edwards–O'Brien case data and digitised GS-1.

#![allow(dead_code)]

pub mod edwards_case;
pub mod hem_rarefaction;
pub mod toro_exact;
