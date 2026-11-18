//! Five-shell TRISO particle builder.
//!
//! **Moved to [`outram_blender::csg::triso_particle`] on 2026-10-02 (GitHub issue
//! #486)**; every item is re-exported here so `outram_mc_libs::geometry::triso_particle::*`
//! paths keep working.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub use outram_blender::csg::triso_particle::*;
