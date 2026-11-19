//! CSG cells — regions bounded by surface half-spaces.
//!
//! **Moved to [`outram_blender::csg::cell`] on 2026-10-02 (GitHub issue
//! #486)**; every item is re-exported here so `outram_mc_libs::geometry::cell::*`
//! paths keep working.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub use outram_blender::csg::cell::*;
