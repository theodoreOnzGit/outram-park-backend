//! Experimental explicit (non-iterative) backward correlations, fitted as
//! Chebyshev polynomials.
//!
//! # What lives here, and why it is separate
//!
//! The rest of this crate's backward equations are line-for-line
//! transcriptions of published IAPWS-IF97 tables. **Nothing in this module is.**
//! Every correlation here is an in-house fit, produced to cover a gap where
//! IF97 either publishes no backward equation at all (Region 5) or where this
//! crate previously had to fall back on an iterative solve (the near-critical
//! Region 4 `(h,s)` band).
//!
//! They are kept in their own module tree, rather than folded into the
//! `region_*` modules, precisely so that the IAPWS-traceable equations stay
//! unambiguously IAPWS-traceable. Treat everything here as a numerical
//! accelerator whose authority is this crate's own forward equations — never
//! as an IAPWS reference value, and never as a published correlation.
//!
//! # Provenance
//!
//! Fitted with AI assistance and contributed as prototypes; see GitHub issue
//! [#34](https://github.com/theodoreOnzGit/outram-park-backend/issues/34) for
//! the original prototype sources, the fitting approach, and the discussion.
//! Per the workspace's AI-usage policy this is untrusted draft material until
//! a human has reviewed it: the round-trip tests in [`tests`] characterise the
//! measured accuracy, but no human V&V sign-off has been recorded.
//!
//! # Contents
//!
//! - [`region_5_t_ph_ps`] — Region 5 `T(p,h)` and `T(p,s)`. IF97 publishes no
//!   backward equations for Region 5 at all.
//! - [`region_4_near_critical_hs`] — near-critical Region 4 `p(h,s)` and
//!   vapour quality `x(h,s)`, for `623.15 K <= T_sat <= 647.04 K`.
//! - [`p_rho_h`] — pressure from density and enthalpy across Regions 1 to 5,
//!   for solvers that carry `(rho, h)` as their state. Includes a statistical
//!   region classifier for callers that have nothing else to go on; read its
//!   accuracy caveats before relying on it.
//!
//! # Shared caveat — fit domains are hard edges
//!
//! A Chebyshev polynomial diverges rapidly outside the interval it was fitted
//! on. Every function here documents its fit domain, and **none of them clamp
//! or validate their inputs**. A caller that cannot guarantee an in-domain
//! state must bound-check before calling; an out-of-domain call returns a
//! plausible-looking number that is meaningless.

// The fitted coefficient tables are transcribed verbatim from the fitting
// procedure, at full output precision. Clippy flags digits beyond what `f64`
// can distinguish, but rounding them here would edit the correlations by hand
// and silently change their results, so the literals are kept exactly as
// fitted — the same convention the IAPWS coefficient tables elsewhere in this
// crate follow.
#![allow(clippy::excessive_precision)]

/// Chebyshev basis evaluation shared by the correlations in this module.
mod chebyshev;

/// Near-critical Region 4 `(h,s)` flash: saturation pressure and vapour
/// quality, for `623.15 K <= T_sat <= 647.04 K`.
pub mod region_4_near_critical_hs;
pub use region_4_near_critical_hs::*;

/// Pressure from density and enthalpy, `p(rho,h)`, across Regions 1 to 5,
/// plus the statistical region classifier that a `(rho,h)`-only caller needs.
pub mod p_rho_h;
pub use p_rho_h::*;

/// Region 5 backward correlations `T(p,h)` and `T(p,s)`, covering a gap where
/// IAPWS-IF97 publishes no backward equations.
pub mod region_5_t_ph_ps;
pub use region_5_t_ph_ps::*;

#[cfg(test)]
mod tests;
