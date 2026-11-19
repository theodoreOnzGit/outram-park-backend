//! General-purpose numerical interpolation methods used by the equipment
//! models in this crate -- not DWSIM-specific code, just the supporting
//! numerics one of DWSIM's calculation modes needs (see [`floater_hormann`]'s
//! module doc for exactly which one, and its literature citations).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod floater_hormann;
