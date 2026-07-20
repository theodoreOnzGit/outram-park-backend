//! Structured spatial meshes for tally binning.
//!
//! C++ source: `src/mesh.cpp`, `include/openmc/mesh.h`.
//!
//! A mesh overlays a regular grid on the geometry so a flux (or reaction-rate)
//! tally can be resolved *spatially* — one bin per grid cell — independent of the
//! CSG cell structure. This is the spatial counterpart to the energy grouping an
//! [`super::filter::EnergyFilter`] provides. Only the axis-aligned
//! [`RegularMesh`] is ported here (the workhorse for the `post-processing`
//! notebook); rectilinear / cylindrical / spherical meshes are a documented gap
//! (bead op-6tz.13).

use crate::geometry::position::Position;

/// Axis-aligned regular (equal-spacing) Cartesian mesh.
///
/// Maps to `openmc::RegularMesh` (`src/mesh.cpp`). The mesh spans the box
/// `[lower_left, upper_right]` \[cm\] and is divided into `dimension[i]` equal
/// cells along each axis `i ∈ {x, y, z}`. A point is binned into the grid cell
/// containing it; points outside the box are unbinned (`None`).
///
/// # Fields (all lengths in cm)
/// - `lower_left` — the low corner `[x0, y0, z0]` of the meshed box.
/// - `upper_right` — the high corner `[x1, y1, z1]`; each `upper_right[i]` must
///   exceed `lower_left[i]`.
/// - `dimension` — number of cells `[nx, ny, nz]` along each axis (each ≥ 1). A
///   flat 2-D mesh sets one dimension to 1 (e.g. `[4, 4, 1]`).
#[derive(Debug, Clone, PartialEq)]
pub struct RegularMesh {
    /// Low corner `[x0, y0, z0]` of the meshed box \[cm\].
    pub lower_left: [f64; 3],
    /// High corner `[x1, y1, z1]` of the meshed box \[cm\].
    pub upper_right: [f64; 3],
    /// Number of equal cells `[nx, ny, nz]` along each axis.
    pub dimension: [usize; 3],
}

impl RegularMesh {
    /// Cell width `[wx, wy, wz]` \[cm\] along each axis
    /// (`(upper_right - lower_left) / dimension`).
    #[inline]
    pub fn width(&self) -> [f64; 3] {
        [
            (self.upper_right[0] - self.lower_left[0]) / self.dimension[0].max(1) as f64,
            (self.upper_right[1] - self.lower_left[1]) / self.dimension[1].max(1) as f64,
            (self.upper_right[2] - self.lower_left[2]) / self.dimension[2].max(1) as f64,
        ]
    }

    /// Total number of mesh cells = `nx · ny · nz`.
    ///
    /// Maps to `StructuredMesh::n_bins` (`src/mesh.cpp:1081`).
    #[inline]
    pub fn n_bins(&self) -> usize {
        self.dimension[0] * self.dimension[1] * self.dimension[2]
    }

    /// Flat bin index of the mesh cell containing `p`, or `None` if `p` lies
    /// outside the meshed box.
    ///
    /// Mirrors `RegularMesh::get_index_in_direction`
    /// (`src/mesh.cpp:1473`) — a 1-based per-axis index
    /// `ceil((r - lower_left) / width)` valid on `1..=dimension` — combined with
    /// `StructuredMesh::get_bin_from_indices` (`src/mesh.cpp:1039`), which for a
    /// 3-D mesh is `bin = ((iz)·ny + iy)·nx + ix` using the 0-based indices
    /// `ix = ceil(...) - 1`. A point exactly on the lower face of the box is
    /// outside (as in OpenMC); a point on the upper face falls in the last cell.
    ///
    /// # Parameters
    /// - `p` — a position \[cm\] (the segment midpoint, in the tally scoring path).
    pub fn get_bin(&self, p: Position) -> Option<usize> {
        let r = [p.x, p.y, p.z];
        let w = self.width();
        let mut idx = [0usize; 3];
        for i in 0..3 {
            if w[i] <= 0.0 || !w[i].is_finite() {
                return None;
            }
            // OpenMC's 1-based index: ceil((r - ll)/w) ∈ 1..=dim ⇒ 0-based ix.
            let c = ((r[i] - self.lower_left[i]) / w[i]).ceil();
            if c < 1.0 || c > self.dimension[i] as f64 {
                return None;
            }
            idx[i] = c as usize - 1;
        }
        // Row-major: x fastest, then y, then z (matches get_bin_from_indices).
        Some((idx[2] * self.dimension[1] + idx[1]) * self.dimension[0] + idx[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×4×1 unit mesh bins interior points into the expected row-major cell,
    /// and rejects points outside the box.
    #[test]
    fn regular_mesh_binning() {
        let m = RegularMesh {
            lower_left: [-2.0, -2.0, -1.0],
            upper_right: [2.0, 2.0, 1.0],
            dimension: [4, 4, 1],
        };
        assert_eq!(m.n_bins(), 16);
        // Lower-left interior cell (ix=0, iy=0) → bin 0.
        assert_eq!(m.get_bin(Position::new(-1.9, -1.9, 0.0)), Some(0));
        // ix=3, iy=3 → bin (0*4 + 3)*4 + 3 = 15.
        assert_eq!(m.get_bin(Position::new(1.9, 1.9, 0.0)), Some(15));
        // ix=1, iy=2 → bin (0*4 + 2)*4 + 1 = 9.
        assert_eq!(m.get_bin(Position::new(-0.5, 0.5, 0.0)), Some(9));
        // Outside the box (x, z) → None.
        assert_eq!(m.get_bin(Position::new(3.0, 0.0, 0.0)), None);
        assert_eq!(m.get_bin(Position::new(0.0, 0.0, 5.0)), None);
    }
}
