//! High-level geometry navigation: particle location and boundary crossing.
//!
//! **Moved to [`outram_blender::csg::geometry`] on 2026-10-02 (GitHub issue
//! #486)**; every item is re-exported here so `outram_mc_libs::geometry::geometry::*`
//! paths keep working.
//!
//! The transport-state half (surface crossing, region exit distance, the
//! cross-section lookup, the boundary-condition check) stayed in this crate,
//! on [`GeometryExt`] in [`super::crossing`], re-exported here with
//! [`SurfaceCrossing`].

pub use outram_blender::csg::geometry::*;
pub use super::crossing::{GeometryExt, SurfaceCrossing};
