//! Binary **departure functions** `Δα^r(δ, τ, x)` — the excess reduced
//! Helmholtz energy beyond the mole-fraction-weighted pure-component sum.
//!
//! **Scaffold only (bead op-kbc.16).** Bodies are `todo!()`.
//!
//! `Δα^r = Σ_i Σ_{j>i} x_i x_j F_{ij} · α^r_{ij}(δ, τ)`, where `F_{ij}` is the
//! binary scaling factor and `α^r_{ij}` is a departure term list. The term forms
//! are Power / Gaussian-like — the same shapes as [`crate::eos::ResidualTerm`]
//! but with the GERG generalised-exponential (`η`, `ε`, `β`, `γ`) parameters.
//! Both `F_{ij}` and the term coefficients come from [`super::binary_pairs`].
#![allow(dead_code, unused_variables)] // TODO(op-kbc.16): drop once implemented

use crate::eos::HelmholtzDerivs;

/// One term of a binary departure function.
///
/// CoolProp's departure functions use two forms: a plain **power** term and the
/// GERG **generalised-exponential** term. The scaffold pins both variants.
#[derive(Debug, Clone, Copy)]
pub enum DepartureTerm {
    /// `n · δ^d · τ^t`. CoolProp departure `Exponential`/power term (`l = 0`).
    Power { n: f64, d: f64, t: f64 },
    /// `n · δ^d · τ^t · exp(-η(δ-ε)² - β(δ-γ))` — GERG generalised exponential.
    GergExponential {
        n: f64,
        d: f64,
        t: f64,
        eta: f64,
        epsilon: f64,
        beta: f64,
        gamma: f64,
    },
}

impl DepartureTerm {
    /// Accumulate this departure term's contribution to `acc` at reduced state
    /// `(δ, τ)` (before the `x_i x_j F_{ij}` binary weighting).
    pub fn accumulate(&self, delta: f64, tau: f64, acc: &mut HelmholtzDerivs) {
        match self {
            DepartureTerm::Power { .. } => {
                // TODO(op-kbc.16): reuse the Power accumulation from eos.rs.
                todo!("op-kbc.16: departure Power term derivatives")
            }
            DepartureTerm::GergExponential { .. } => {
                todo!("op-kbc.16: GERG generalised-exponential term derivatives")
            }
        }
    }
}
