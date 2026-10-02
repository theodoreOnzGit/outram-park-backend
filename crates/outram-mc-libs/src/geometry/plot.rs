//! Geometry plotting — a port of OpenMC's slice and ray-trace plotters.
//!
//! **Moved to [`outram_blender::csg::plot`] on 2026-10-02 (GitHub issue
//! #486)**; every item, and every submodule (`slice`, `raytrace`,
//! `model_plot`, ...), is re-exported here so `outram_mc_libs::geometry::plot`
//! paths keep working. `ModelPlot` is generic over
//! [`MaterialIdentity`], which this crate implements for its
//! [`Material`](crate::material::material::Material) below.

pub use outram_blender::csg::plot::*;

impl MaterialIdentity for crate::material::material::Material {
    #[inline]
    fn material_id(&self) -> i32 {
        self.id
    }
    #[inline]
    fn material_name(&self) -> &str {
        &self.name
    }
}
