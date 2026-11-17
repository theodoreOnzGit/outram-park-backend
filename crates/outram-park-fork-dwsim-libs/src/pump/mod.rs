//! Pump duty/pressure-rise calculation, and net positive suction head (NPSH).
//!
//! Ported from DWSIM `UnitOperations/Pump.vb` -- see `modes`' module doc for
//! the full source mapping and the flash-dependency boundary.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod modes;
