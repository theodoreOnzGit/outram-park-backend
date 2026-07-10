//! Binary **interaction parameters** and departure-function assignments for the
//! multi-fluid mixture model — CoolProp's `mixture_binary_pairs.json` +
//! `mixture_departure_functions.json`, hardcoded.
//!
//! **Scaffold only (bead op-kbc.16).** No pairs are ported yet; this fixes the
//! lookup shape. Populate from `reference/CoolProp/dev/mixtures/` via a codegen
//! follow-up (the mixture analogue of `dev/regen_all.py`).
#![allow(dead_code, unused_variables)] // TODO(op-kbc.16): drop once data is generated

use super::departure::DepartureTerm;
use crate::fluid::Fluid;

/// The interaction data for one unordered pair of components `(i, j)`.
///
/// Holds the Lorentz–Berthelot reducing-function parameters ([`reducing`])
/// and the departure-function scaling `F_{ij}` + term list ([`departure`]).
///
/// [`reducing`]: super::reducing
/// [`departure`]: super::departure
#[derive(Debug, Clone, Copy)]
pub struct BinaryPair {
    /// First component.
    pub a: Fluid,
    /// Second component.
    pub b: Fluid,
    /// Reducing-temperature parameters `(β^T, γ^T)`.
    pub beta_gamma_t: (f64, f64),
    /// Reducing-volume parameters `(β^v, γ^v)`.
    pub beta_gamma_v: (f64, f64),
    /// Departure-function scaling `F_{ij}` (0 ⇒ no departure term, ideal mixing).
    pub f_departure: f64,
    /// Departure-function term list (empty when `f_departure == 0`).
    pub departure_terms: &'static [DepartureTerm],
}

/// The hardcoded binary-pair table. Empty in the scaffold.
///
/// TODO(op-kbc.16): emit one entry per CoolProp binary pair here.
pub static BINARY_PAIRS: &[BinaryPair] = &[];

/// Look up the interaction data for the unordered pair `(a, b)`.
///
/// Returns `None` when the pair is absent — the caller
/// ([`super::Mixture::residual_derivs`]) then reports
/// [`super::MixtureError::MissingBinaryPair`] rather than silently assuming
/// ideal mixing.
pub fn lookup(a: Fluid, b: Fluid) -> Option<&'static BinaryPair> {
    BINARY_PAIRS
        .iter()
        .find(|p| (p.a == a && p.b == b) || (p.a == b && p.b == a))
}
