//! Turbine/expander thermodynamics: isentropic expansion with adiabatic or
//! Schultz-corrected polytropic efficiency.
//!
//! Ported from DWSIM `UnitOperations/Expander.vb` -- see `isentropic`'s
//! module doc for the full source mapping, the flash-dependency boundary,
//! and the sign convention this port uses. DWSIM's `Curves` calculation mode
//! (Floater-Hormann rational interpolation of head/efficiency/power vs.
//! flow) is not ported -- see the workspace's `op-qo2.9` bead.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod isentropic;
