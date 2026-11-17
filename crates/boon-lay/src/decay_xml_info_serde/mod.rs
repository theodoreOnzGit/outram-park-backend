// i imported code from openmc-endf-8-depletion-lib-b

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub use openmc_endf_8_depletion_lib_b::prelude::*;
pub use openmc_endf_8_depletion_lib_b::SerdeNuclideData;
pub use openmc_endf_8_depletion_lib_b::SerdeNuclideVec;
