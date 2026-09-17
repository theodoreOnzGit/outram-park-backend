//! `(p,s)` region-dispatch / boundary equations across IAPWS-IF97 Regions
//! 1-4. Given a pressure-entropy pair, the code determines which region a
//! state lies in — Region 1 (subcooled liquid), Region 2 (vapour), Region 3
//! (near-critical/supercritical single phase), or Region 4 (vapour-liquid
//! equilibrium / saturation) — and calls that region's backward equations.
//! This module supplies the near-critical Region 3/4 saturation boundary
//! (`boundary_eqn_ps3`, the p_s3(s) curve).

#[cfg(test)]
mod tests;

/// Near-critical Region 3 / Region 4 saturation boundary as a function of
/// specific entropy — the p_s3(s) backward equation.
pub mod boundary_eqn_ps3;
