//! Heat exchanger rating: LMTD, epsilon-NTU effectiveness, and the
//! Bowman/Underwood multi-pass LMTD correction factor.
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`. The full Tinker
//! shell-and-tube rating (tube-layout-dependent Bell-Delaware-like shell-side
//! correlations, ~700 lines) is not ported here -- see the workspace's
//! `op-qo2.7` bead.

pub mod f_correction;
pub mod lmtd;
pub mod ntu_effectiveness;
