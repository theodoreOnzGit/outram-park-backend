//! Heat exchanger rating: LMTD, epsilon-NTU effectiveness, the
//! Bowman/Underwood multi-pass LMTD correction factor, and Tinker's
//! (simplified) shell-and-tube method.
//!
//! Ported from DWSIM `UnitOperations/HeatExchanger.vb`. See
//! [`tinker_shell_and_tube`]'s module doc for that method's full source
//! mapping and the outer-convergence-loop flash-dependency boundary.

pub mod f_correction;
pub mod lmtd;
pub mod ntu_effectiveness;
pub mod tinker_shell_and_tube;
