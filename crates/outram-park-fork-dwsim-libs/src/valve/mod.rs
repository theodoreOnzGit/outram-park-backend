//! Control-valve sizing (IEC 60534 / ISA-75.01.01).
//!
//! Ported from DWSIM `UnitOperations/Valve.vb` -- see `iec_60534`'s module
//! doc for the full source mapping and unit conventions.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod iec_60534;
