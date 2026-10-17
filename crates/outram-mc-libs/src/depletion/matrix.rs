//! Dense depletion (burnup) matrix `A` with units of inverse seconds.
//!
//! **MOVED 2026-10-17 to `petir::depletion::matrix` (gh:#950, maintainer
//! direction "All math moved to petir").** This module re-exports it so every
//! `outram_mc_libs::depletion::matrix::DepletionMatrix` path keeps working.
//! The storage and arithmetic are unchanged
//! (`tests/depletion_move_fingerprints.rs`); what changed is that `set`,
//! `add` and `mat_vec` return a `Result` instead of panicking out of range,
//! and `get` returns `NaN` out of range (PETIR's no-panic rule).
//!
//! The sign and index convention (`dN[row]/dt = sum_col A[row][col] N[col]`,
//! production off the diagonal, total removal on it) is documented at the
//! PETIR type. Assembling the entries — decay constants, branching, reaction
//! rates, fission yields — stays in this crate, in
//! [`DepletionChain::build_matrix`](super::chain::DepletionChain::build_matrix).

pub use petir::depletion::matrix::{lincomb, DepletionMatrix, Sign};
