//! Composition-dependent **reducing functions** `T_r(x)`, `ρ_r(x)` for the
//! multi-fluid mixture model (GERG-2008 Lorentz–Berthelot form).
//!
//! **Scaffold only (bead op-kbc.16).** Bodies are `todo!()`.
//!
//! The reduced state that every component's Helmholtz surface is evaluated at is
//! `δ = ρ / ρ_r(x)`, `τ = T_r(x) / T`, with
//!
//! `T_r(x) = Σ_i x_i² T_{c,i} + Σ_i Σ_{j>i} 2 x_i x_j · β^T_{ij} γ^T_{ij} ·
//!            (x_i + x_j)/(β^T²_{ij} x_i + x_j) · sqrt(T_{c,i} T_{c,j})`
//!
//! and the analogous `1/ρ_r(x)` mixing rule on the critical volumes. The binary
//! `β`, `γ` parameters come from [`super::binary_pairs`].
#![allow(dead_code, unused_variables)] // TODO(op-kbc.16): drop once implemented

use super::Mixture;

/// Composition-dependent reducing temperature `T_r(x)` \[K\].
pub fn reducing_temperature(mix: &Mixture) -> f64 {
    // TODO(op-kbc.16): double sum over components using β^T, γ^T from binary_pairs.
    todo!("op-kbc.16: T_r(x) reducing function")
}

/// Composition-dependent reducing molar density `ρ_r(x)` \[mol/m³\].
pub fn reducing_density(mix: &Mixture) -> f64 {
    // TODO(op-kbc.16): mix on 1/ρ_c (critical volume) using β^v, γ^v; invert.
    todo!("op-kbc.16: ρ_r(x) reducing function")
}
