//! `(h,s)` region-dispatch / boundary equations across IAPWS-IF97 Regions
//! 1-4. Given an enthalpy-entropy pair, the code determines which region a
//! state lies in — Region 1 (subcooled liquid), Region 2 (vapour), Region 3
//! (near-critical/supercritical single phase), or Region 4 (vapour-liquid
//! equilibrium / saturation) — and calls that region's backward equations.
//! The submodules supply the boundary curves used to make that decision:
//! the saturated-liquid (bubble) and saturated-vapour (dew) lines, and the
//! B13 (Region 1/3) and B23 (Region 2/3) inter-region boundaries.

#[cfg(test)]
mod tests;

/// B13 boundary between Region 1 (subcooled liquid) and Region 3.
pub mod region_1_and_3;
/// B23 boundary between Region 2 (vapour) and Region 3.
pub mod region_2_and_3;
/// Saturated-liquid (bubble) line h'(s) boundaries for the `(h,s)` dispatch.
pub mod saturated_liquid_line;
/// Saturated-vapour (dew) line h''(s) boundaries for the `(h,s)` dispatch.
pub mod saturated_vapour_line;
