//! Quadric and toroidal CSG surfaces.
//!
//! **Moved to [`outram_blender::csg::surface`] on 2026-10-02 (GitHub issue
//! #486)**; every item is re-exported here so `outram_mc_libs::geometry::surface::*`
//! paths keep working.
//!
//! Three methods stayed in this crate, on [`SurfaceKindExt`]: the RNG-driven
//! `diffuse_reflect` (white boundary) and the virtual-lattice helpers
//! `sphere_centre_radius` / `overlaps_voxel`.

pub use outram_blender::csg::surface::*;

pub use super::crossing::SurfaceKindExt;
