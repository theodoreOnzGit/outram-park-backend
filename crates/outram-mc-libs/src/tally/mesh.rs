//! Structured spatial meshes for tally binning.
//!
//! **The description moved to [`outram_blender::spatial_mesh`] on
//! 2026-10-02 (GitHub #486)** and is re-exported here, so
//! `outram_mc_libs::tally::mesh::{RegularMesh, ..., MeshKind}` keep working.
//! What stays here is bin **lookup and scoring**, on the extension traits
//! [`RegularMeshExt`], [`RectilinearMeshExt`], [`CylindricalMeshExt`],
//! [`SphericalMeshExt`] and [`MeshKindExt`] (also in the prelude).
//!
//! C++ source: `src/mesh.cpp`, `include/openmc/mesh.h`.
//!
//! A mesh overlays a regular grid on the geometry so a flux (or reaction-rate)
//! tally can be resolved *spatially* — one bin per grid cell — independent of the
//! CSG cell structure. This is the spatial counterpart to the energy grouping an
//! [`super::filter::EnergyFilter`] provides. Only the axis-aligned
//! [`RegularMesh`] is ported here (the workhorse for the `post-processing`
//! notebook); ~~rectilinear / cylindrical / spherical meshes are a documented
//! gap (bead op-6tz.13)~~ **CORRECTED 2026-09-22 (GitHub #260)** --
//! [`RectilinearMesh`], [`CylindricalMesh`] and [`SphericalMesh`] are now
//! present, verified bin-for-bin against OpenMC's own per-bin volumes to
//! 2.854e-16 (`tests/mesh_vs_openmc.rs`).
//!
//! Scope item 4 (wiring into [`super::filter::MeshFilter`]) is done too, via
//! [`MeshKind`]; per-bin volumes for flux normalisation (scope item 5) are on
//! [`MeshKind::bin_volume`].
//!
//! ~~Still absent, and still a real gap: the **unstructured** mesh family, which
//! is planned via OpenFOAM `polyMesh` reuse and is explicitly out of scope for
//! #260.~~ **CORRECTED 2026-10-03 (GitHub #492):** the unstructured family is
//! [`MeshKind::Unstructured`] (description in `outram_blender::unstructured`,
//! built from or converted to OpenFOAM `polyMesh`), scored by
//! [`super::mesh_unstructured::UnstructuredMeshExt`] — point location, and a
//! track-length estimator that **does** split a segment across the cells it
//! crosses (`MOABMesh::bins_crossed`). For the four **structured** kinds the
//! following gap is unchanged: the
//! track-length **`bins_crossed`** sub-segmentation, so a segment is scored
//! whole into its midpoint's cell rather than split across the cells it
//! actually crosses. That approximation is exact only while a mesh cell is
//! large relative to the mean free path, and it is **more** wrong on a
//! cylindrical mesh than a Cartesian one, because a radial cell's width varies
//! across it. Worth knowing before using a fine R-Z mesh.

use crate::geometry::position::Position;
use crate::tally::mesh_unstructured::UnstructuredMeshExt;

pub use outram_blender::spatial_mesh::*;

/// Index of the bin on a 1-D ascending grid containing `x`, or `None` if `x`
/// lies outside `[grid[0], grid[last]]`.
///
/// Upstream uses `lower_bound_index` and adds 1 because its `MeshIndex` is
/// 1-based (`src/mesh.cpp:2138`). This crate is 0-based throughout, so the `+1`
/// is deliberately **not** carried — carrying it would put every bin index one
/// high and the error would only show at the boundaries.
///
/// The upper edge is inclusive so a point exactly on the outer surface bins
/// into the last cell rather than falling out of the mesh.
#[inline]
fn bin_on_grid(grid: &[f64], x: f64) -> Option<usize> {
    if grid.len() < 2 || x < grid[0] || x > grid[grid.len() - 1] {
        return None;
    }
    // Ascending grid: find the last edge not exceeding `x`.
    let mut lo = 0usize;
    let mut hi = grid.len() - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if grid[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// `FP_PRECISION` (`include/openmc/constants.h`) — the on-axis guard above.
const FP_PRECISION: f64 = 1.0e-14;

/// Surface bins per mesh element: `4 * n_dimension` — for each of the three
/// axes, the min and max face, each with an outward and an inward current.
///
/// `StructuredMesh::n_surface_bins` (`src/mesh.cpp:1189`) at OpenMC `afa7a14`.
pub const SURFACE_BINS_PER_ELEMENT: usize = 12;

/// Bin lookup on [`RegularMesh`] that stays in `outram-mc-libs` (GitHub #486).
pub trait RegularMeshExt {
    /// Flat bin index of the mesh cell containing `p`, or `None` if `p` lies
    /// outside the meshed box.
    ///
    /// Mirrors `RegularMesh::get_index_in_direction`
    /// (`src/mesh.cpp:1580` in OpenMC 0.16.1-dev) — a 1-based per-axis index
    /// valid on `1..=dimension` — combined with
    /// `StructuredMesh::get_bin_from_indices` (`src/mesh.cpp:1134`), which for a
    /// 3-D mesh is `bin = ((iz)·ny + iy)·nx + ix` using the 0-based indices.
    ///
    /// # Boundary handling — both faces are INSIDE, and this is exact upstream
    ///
    /// Upstream does not simply take `ceil`; it special-cases both faces:
    ///
    /// ```text
    /// if (r <= lower_left_[i])  return r == lower_left_[i] ? 1 : 0;
    /// if (r >= upper_right_[i]) return r == upper_right_[i] ? shape_[i] : shape_[i] + 1;
    /// return std::ceil((r - lower_left_[i]) / width_[i]);
    /// ```
    ///
    /// So a point lying **exactly** on the lower face is in the FIRST cell, and
    /// one exactly on the upper face is in the LAST cell. Only strictly
    /// outside points are unbinned.
    ///
    /// ~~"A point exactly on the lower face of the box is outside (as in
    /// OpenMC)"~~ — **CORRECTED 2026-09-17**. That claim was false in both
    /// halves: upstream puts such a point in cell 1, and this port returned
    /// `None` for it because `ceil(0.0) = 0` fails the `c < 1.0` test. The
    /// upper face was already right, since `ceil(dim) = dim` lands in the last
    /// cell, so the defect was one-sided. Found by reading `mesh.cpp` while
    /// porting the Shannon-entropy diagnostic, which bins a fission bank whose
    /// sites can sit exactly on a mesh plane when the mesh is aligned to
    /// lattice pitch — precisely the case this got wrong.
    ///
    /// # Parameters
    /// - `p` — a position \[cm\] (the segment midpoint, in the tally scoring path).
    fn get_bin(&self, p: Position) -> Option<usize>;
    /// Accumulate the **weight** of each bank site into its mesh bin.
    ///
    /// Port of `RegularMesh::count_sites` (`src/mesh.cpp:1677`), which is
    /// byte-identical to `StructuredMesh::count_sites` (`:1200`) upstream.
    ///
    /// Returns the per-bin weight totals and a flag that is `true` if **any**
    /// site fell outside the mesh. Upstream warns on that flag rather than
    /// erroring (`src/eigenvalue.cpp:595-599`), because a few escaped fission
    /// sites are normal near a boundary; this port returns the flag and lets
    /// the caller decide, which is the same information without a global
    /// logger.
    ///
    /// # What is accumulated
    ///
    /// **Statistical weight `wgt`, not a site count.** Upstream does
    /// `cnt(mesh_bin) += site.wgt`. With analog fission and unit weights the
    /// two coincide, which is exactly why substituting a count passes casual
    /// testing and then diverges silently under weight windows or implicit
    /// capture. Do not "simplify" this to a tally of hits.
    ///
    /// The MPI reduction upstream wraps is omitted: this crate has no MPI
    /// path, and the non-MPI branch is a plain copy.
    ///
    /// # Parameters
    /// - `sites` — the fission bank for the generation just completed.
    fn count_sites(&self, sites: &[crate::particle::bank::BankSite]) -> (Vec<f64>, bool);
    /// **Shannon entropy of the fission source on this mesh, in bits.**
    ///
    /// Port of `shannon_entropy()` (`src/eigenvalue.cpp:587-616`):
    ///
    /// ```text
    /// p = count_sites(bank);  p /= p.sum();
    /// H = -sum_i p_i log2(p_i)     for p_i > 0
    /// ```
    ///
    /// # Why it exists
    ///
    /// `H` measures how spread out the fission source is. It rises from a
    /// concentrated initial guess and **plateaus once the source has
    /// converged**, which is the standard diagnostic for choosing how many
    /// inactive generations to discard. A k-eigenvalue quoted without it is
    /// a number whose source convergence nobody checked — and on a large,
    /// loosely-coupled core such as a pebble bed that is where the bias hides.
    ///
    /// # Units and range
    ///
    /// **Bits — the logarithm is base 2**, matching upstream's `std::log2`.
    /// `H` runs from 0 (every site in one bin) to `log2(n_bins)` (a perfectly
    /// uniform source), so the ceiling is set by the mesh, not the physics.
    /// Getting the base wrong rescales every value by `ln 2` and still
    /// plateaus, so it will not be caught by eye.
    ///
    /// # Returns
    ///
    /// `None` if the bank is empty or carries no weight inside the mesh —
    /// there is no distribution to take an entropy of. Upstream has no such
    /// guard because it divides by the sum unconditionally; a zero-weight bank
    /// there yields NaN. Returning `None` is the same information without a
    /// NaN propagating into a convergence plot.
    ///
    /// # Parameters
    /// - `sites` — the fission bank for the generation just completed.
    fn shannon_entropy(&self, sites: &[crate::particle::bank::BankSite]) -> Option<f64>;
    /// Number of bins a [`crate::tally::filter_extra::MeshSurfaceFilter`] on
    /// this mesh produces.
    fn n_surface_bins(&self) -> usize;
    /// The surface bin for one face of one element —
    /// `SurfaceAggregator::surface` (`src/mesh.cpp:1296`):
    /// `4 * n_dim * element + 4 * k + (max ? 2 : 0) + (inward ? 1 : 0)`.
    fn surface_bin(&self, element: usize, axis: usize, max: bool, inward: bool) -> usize;
    /// Every surface bin the segment `r0 -> r1` crosses, in order of travel —
    /// `StructuredMesh::surface_bins_crossed` (`src/mesh.cpp`).
    ///
    /// # What a crossing produces
    ///
    /// Each plane crossing scores **two** bins when both neighbouring elements
    /// are inside the mesh: an **outward** current on the element being left
    /// (through its max face when travelling in `+k`, its min face otherwise)
    /// and an **inward** current on the element being entered, through the
    /// opposite face. A crossing at the mesh boundary scores only the half
    /// that is inside.
    ///
    /// That pairing is the whole point: a net current across an internal face
    /// is `outward(left) - inward(right)` and a tally that recorded only one
    /// side could not form it.
    ///
    /// # Implementation note
    ///
    /// Upstream walks the track incrementally, recomputing the distance to the
    /// next grid boundary on one axis at a time. This enumerates the plane
    /// crossings on all three axes and sorts them, which is equivalent for a
    /// **uniform** grid (where every plane position is known in closed form)
    /// and avoids reproducing the `TINY_BIT` nudging that the incremental form
    /// needs. `surface_crossings_agree_with_an_incremental_walk` checks the
    /// two against each other on randomised tracks rather than asserting the
    /// equivalence.
    fn surface_bins_crossed(&self, r0: Position, r1: Position) -> Vec<usize>;
}

impl RegularMeshExt for RegularMesh {
    fn get_bin(&self, p: Position) -> Option<usize> {
        let r = [p.x, p.y, p.z];
        let w = self.width();
        let mut idx = [0usize; 3];
        for i in 0..3 {
            if w[i] <= 0.0 || !w[i].is_finite() {
                return None;
            }
            let dim = self.dimension[i] as f64;
            // Transcribed branch-for-branch from RegularMesh::get_index_in_direction.
            let c = if r[i] <= self.lower_left[i] {
                if r[i] == self.lower_left[i] {
                    1.0
                } else {
                    0.0
                }
            } else if r[i] >= self.upper_right[i] {
                if r[i] == self.upper_right[i] {
                    dim
                } else {
                    dim + 1.0
                }
            } else {
                ((r[i] - self.lower_left[i]) / w[i]).ceil()
            };
            if c < 1.0 || c > dim {
                return None;
            }
            idx[i] = c as usize - 1;
        }
        // Row-major: x fastest, then y, then z (matches get_bin_from_indices).
        Some((idx[2] * self.dimension[1] + idx[1]) * self.dimension[0] + idx[0])
    }

    fn count_sites(&self, sites: &[crate::particle::bank::BankSite]) -> (Vec<f64>, bool) {
        let mut counts = vec![0.0f64; self.n_bins()];
        let mut outside = false;
        for site in sites {
            match self.get_bin(site.r) {
                Some(bin) => counts[bin] += site.wgt,
                None => outside = true,
            }
        }
        (counts, outside)
    }

    fn shannon_entropy(&self, sites: &[crate::particle::bank::BankSite]) -> Option<f64> {
        // Deciding what is counted (binning the bank on this mesh) stays
        // here; the formula is RAFFLES' since 2026-10-02 (GitHub #500).
        let (counts, _outside) = self.count_sites(sites);
        raffles::estimators::shannon_entropy_bits(&counts)
    }

    fn n_surface_bins(&self) -> usize {
        SURFACE_BINS_PER_ELEMENT * self.n_bins()
    }

    fn surface_bin(&self, element: usize, axis: usize, max: bool, inward: bool) -> usize {
        SURFACE_BINS_PER_ELEMENT * element
            + 4 * axis
            + if max { 2 } else { 0 }
            + if inward { 1 } else { 0 }
    }

    fn surface_bins_crossed(&self, r0: Position, r1: Position) -> Vec<usize> {
        let w = self.width();
        let a = [r0.x, r0.y, r0.z];
        let b = [r1.x, r1.y, r1.z];
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        if !(len > 0.0) {
            return Vec::new();
        }

        // Every grid-plane crossing on the open interval (0, 1) in track
        // parameter, as (t, axis, moving in +axis).
        let mut events: Vec<(f64, usize, bool)> = Vec::new();
        for k in 0..3 {
            if d[k] == 0.0 || w[k] <= 0.0 || !w[k].is_finite() {
                continue;
            }
            let forward = d[k] > 0.0;
            for i in 0..=self.dimension[k] {
                let plane = self.lower_left[k] + i as f64 * w[k];
                let t = (plane - a[k]) / d[k];
                if t > 0.0 && t < 1.0 {
                    events.push((t, k, forward));
                }
            }
        }
        events.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut bins = Vec::with_capacity(2 * events.len());
        for (t, k, forward) in events {
            // Sample just either side of the crossing so the two elements are
            // identified by position rather than by index arithmetic that
            // would have to special-case the mesh edge.
            let eps = 1.0e-9;
            let at = |s: f64| Position::new(a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s);
            let leaving = self.get_bin(at(t - eps));
            let entering = self.get_bin(at(t + eps));
            if let Some(e) = leaving {
                bins.push(self.surface_bin(e, k, forward, false));
            }
            if let Some(e) = entering {
                bins.push(self.surface_bin(e, k, !forward, true));
            }
        }
        bins
    }
}

/// Bin lookup on [`RectilinearMesh`] that stays in `outram-mc-libs` (GitHub #486).
pub trait RectilinearMeshExt {
    /// `(i, j, k)` of the bin containing `p`, or `None` if outside.
    fn indices(&self, p: Position) -> Option<[usize; 3]>;
    /// Flat bin index, x fastest — the same ordering [`RegularMesh`] uses.
    fn bin(&self, p: Position) -> Option<usize>;
}

impl RectilinearMeshExt for RectilinearMesh {
    fn indices(&self, p: Position) -> Option<[usize; 3]> {
        Some([
            bin_on_grid(&self.grid[0], p.x)?,
            bin_on_grid(&self.grid[1], p.y)?,
            bin_on_grid(&self.grid[2], p.z)?,
        ])
    }

    fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }
}

/// Bin lookup on [`CylindricalMesh`] that stays in `outram-mc-libs` (GitHub #486).
pub trait CylindricalMeshExt {
    /// `(i_r, i_phi, i_z)` of the bin containing `p`, or `None` if outside.
    ///
    /// Ported from `CylindricalMesh::get_indices` (`src/mesh.cpp:1920`),
    /// including the `r < FP_PRECISION` guard that pins `phi = 0` on the axis
    /// rather than letting `atan2(0, 0)` decide it.
    fn indices(&self, p: Position) -> Option<[usize; 3]>;
    /// Flat bin index, r fastest.
    fn bin(&self, p: Position) -> Option<usize>;
}

impl CylindricalMeshExt for CylindricalMesh {
    fn indices(&self, p: Position) -> Option<[usize; 3]> {
        let x = p.x - self.origin.x;
        let y = p.y - self.origin.y;
        let z = p.z - self.origin.z;

        let r = x.hypot(y);
        let phi = if r < FP_PRECISION {
            0.0
        } else {
            let a = y.atan2(x);
            if a < 0.0 {
                a + std::f64::consts::TAU
            } else {
                a
            }
        };
        Some([
            bin_on_grid(&self.r_grid, r)?,
            bin_on_grid(&self.phi_grid, phi)?,
            bin_on_grid(&self.z_grid, z)?,
        ])
    }

    fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }
}

/// Bin lookup on [`SphericalMesh`] that stays in `outram-mc-libs` (GitHub #486).
pub trait SphericalMeshExt {
    /// `(i_r, i_theta, i_phi)`, or `None` if outside. `src/mesh.cpp:2218`.
    fn indices(&self, p: Position) -> Option<[usize; 3]>;
    /// Flat bin index, r fastest.
    fn bin(&self, p: Position) -> Option<usize>;
}

impl SphericalMeshExt for SphericalMesh {
    fn indices(&self, p: Position) -> Option<[usize; 3]> {
        let x = p.x - self.origin.x;
        let y = p.y - self.origin.y;
        let z = p.z - self.origin.z;

        let r = (x * x + y * y + z * z).sqrt();
        let (theta, phi) = if r < FP_PRECISION {
            (0.0, 0.0)
        } else {
            let a = y.atan2(x);
            (
                (z / r).clamp(-1.0, 1.0).acos(),
                if a < 0.0 {
                    a + std::f64::consts::TAU
                } else {
                    a
                },
            )
        };
        Some([
            bin_on_grid(&self.r_grid, r)?,
            bin_on_grid(&self.theta_grid, theta)?,
            bin_on_grid(&self.phi_grid, phi)?,
        ])
    }

    fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }
}

/// Bin lookup on [`MeshKind`] that stays in `outram-mc-libs` (GitHub #486).
pub trait MeshKindExt {
    /// Flat bin index containing `p`, or `None` if `p` is outside the mesh.
    fn bin(&self, p: Position) -> Option<usize>;
    /// Whether the track-length estimator splits a segment across the bins
    /// it crosses on this mesh ([`Self::bins_crossed`]), rather than scoring
    /// it whole into its midpoint's bin.
    ///
    /// `true` only for [`MeshKind::Unstructured`] (GitHub #492, after
    /// `MOABMesh::bins_crossed`). The four structured kinds keep the
    /// documented midpoint approximation (module docs): changing them would
    /// move every recorded structured-mesh tally, which needs its own
    /// re-measurement, and is not part of #492.
    fn splits_track_length(&self) -> bool;
    /// `(bin, length fraction)` for every bin the segment `r0 -> r1` \[cm\]
    /// crosses, or `None` where [`Self::splits_track_length`] is `false`.
    fn bins_crossed(&self, r0: Position, r1: Position) -> Option<Vec<(usize, f64)>>;
}

impl MeshKindExt for MeshKind {
    fn bin(&self, p: Position) -> Option<usize> {
        match self {
            Self::Regular(m) => m.get_bin(p),
            Self::Rectilinear(m) => m.bin(p),
            Self::Cylindrical(m) => m.bin(p),
            Self::Spherical(m) => m.bin(p),
            Self::Unstructured(m) => m.locate(p),
        }
    }

    fn splits_track_length(&self) -> bool {
        match self {
            Self::Regular(_) | Self::Rectilinear(_) | Self::Cylindrical(_) | Self::Spherical(_) => false,
            Self::Unstructured(_) => true,
        }
    }

    fn bins_crossed(&self, r0: Position, r1: Position) -> Option<Vec<(usize, f64)>> {
        match self {
            Self::Regular(_) | Self::Rectilinear(_) | Self::Cylindrical(_) | Self::Spherical(_) => None,
            Self::Unstructured(m) => Some(m.bins_crossed(r0, r1)),
        }
    }
}

#[cfg(test)]
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
