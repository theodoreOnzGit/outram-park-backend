//! Moved to [`dhoby_ghaut::mc_web::raster`] (gh:#1653, 2026-11-18) so every
//! k_eff simulator draws its geometry from the assembled CSG with the same
//! slicer; re-exported here so the rungs' call sites are unchanged.
#![allow(unused_imports)]
pub use dhoby_ghaut::mc_web::raster::*;
