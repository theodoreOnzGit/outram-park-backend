//! Binary **interaction parameters** and departure-function assignments for the
//! multi-fluid mixture model — CoolProp's `mixture_binary_pairs.json` +
//! `mixture_departure_functions.json`, hardcoded.
//!
//! **One real pair is ported so far: Nitrogen–Oxygen** (transcribed by hand
//! from `reference/CoolProp/dev/mixtures/mixture_binary_pairs.json`, MIT). It
//! has `F = 0` (no departure function — air is close enough to ideal mixing
//! that GERG-2008 doesn't fit one), so it also exercises
//! [`super::reducing`] without needing [`super::departure`] ported yet. The
//! other 887 CoolProp binary pairs are a follow-up (bead op-kbc.16) —
//! ideally via a `dev/gen_mixture.py` codegen script mirroring
//! `dev/gen_fluid.py`, given the JSON's size (~10k lines).
//!
//! A pair **absent** from [`BINARY_PAIRS`] is treated as ideal
//! Lorentz–Berthelot combining (`β = γ = 1`, `F = 0`) by
//! [`super::reducing`]/[`super::Mixture::residual_derivs`] — matching
//! CoolProp's own default for binary pairs it has no fitted data for, not an
//! error condition. [`lookup`] returning `None` is the normal, common case.

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

/// The hardcoded binary-pair table.
pub static BINARY_PAIRS: &[BinaryPair] = &[
    // CoolProp dev/mixtures/mixture_binary_pairs.json, Gernert-Thesis-2013.
    // F = 0: no departure function fitted for N2-O2 (ideal-mixing-like air).
    BinaryPair {
        a: Fluid::Nitrogen,
        b: Fluid::Oxygen,
        beta_gamma_t: (0.997190589, 0.995157044),
        beta_gamma_v: (0.99952177, 0.997082328),
        f_departure: 0.0,
        departure_terms: &[],
    },
];

/// Look up the interaction data for the unordered pair `(a, b)`.
///
/// Returns `None` when the pair is absent from the (currently one-entry)
/// table — the common case, treated as ideal combining (`β=γ=1`, `F=0`) by
/// the callers, per the module doc.
pub fn lookup(a: Fluid, b: Fluid) -> Option<&'static BinaryPair> {
    BINARY_PAIRS
        .iter()
        .find(|p| (p.a == a && p.b == b) || (p.a == b && p.b == a))
}
