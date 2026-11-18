//! Universe hierarchy.
//!
//! **Moved to [`outram_blender::csg::universe`] on 2026-10-02 (GitHub issue
//! #486)**; every item is re-exported here so `outram_mc_libs::geometry::universe::*`
//! paths keep working.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub use outram_blender::csg::universe::*;
