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
//! crosses (`MOABMesh::bins_crossed`). **CORRECTED 2026-11-19 (gh:#678):**
//! regular and rectilinear meshes now split too, at each grid plane
//! (`planar_bins_crossed`), and so do **cylindrical and spherical** meshes
//! (`cylindrical_bins_crossed`, `spherical_bins_crossed`, quadratic cuts); the
//! gap below is closed for all four kinds. ~~For the four structured kinds the
//! following gap is unchanged:~~ the
//! track-length **`bins_crossed`** sub-segmentation, so a segment is scored
//! whole into its midpoint's cell rather than split across the cells it
//! actually crosses. That approximation is exact only while a mesh cell is
//! large relative to the mean free path, and it is **more** wrong on a
//! cylindrical mesh than a Cartesian one, because a radial cell's width varies
//! across it. Worth knowing before using a fine R-Z mesh.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;
use crate::geometry::position::{Direction, Position};
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
/// into the last cell rather than falling out of the mesh. ~~An interior edge
/// went to the upper bin~~ **CORRECTED 2026-11-19** (gh:#687): as OpenMC's
/// `lower_bound_index`, it goes to the LOWER bin.
#[inline]
fn bin_on_grid(grid: &[f64], x: f64) -> Option<usize> {
    if grid.len() < 2 || x < grid[0] || x > grid[grid.len() - 1] {
        return None;
    }
    // `lower_bound_index` (gh:#687): an interior edge belongs to the lower bin.
    Some(crate::tally::filter::lower_bound_index(grid, x))
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
    /// needs. ~~`surface_crossings_agree_with_an_incremental_walk` checks the
    /// two against each other on randomised tracks rather than asserting the
    /// equivalence.~~ **CORRECTED 2026-11-22 (gh:#1709):** no such test
    /// existed. The equivalence does **not** hold at ties (a track ending on a
    /// plane); off ties it is checked against the port of the incremental walk,
    /// [`Self::surface_bins_crossed_openmc`], by
    /// `tests/mesh_surface_raytrace_1709.rs`. The mesh-surface filter uses the
    /// port.
    fn surface_bins_crossed(&self, r0: Position, r1: Position) -> Vec<usize>;
    /// Every surface bin the track `r0 -> r1` (direction `u`) crosses, in order
    /// of travel: a **port** of `StructuredMesh::raytrace_mesh` with its
    /// `SurfaceAggregator` (`src/mesh.cpp:1312-1488`, OpenMC `8f79d9d0`), and
    /// `RegularMesh::distance_to_grid_boundary` (`:1667`) and
    /// `get_index_in_direction` (`:1640`) for a 3-D regular mesh (gh:#1709).
    ///
    /// Beside the re-derivation [`Self::surface_bins_crossed`], which counts
    /// crossings on the open interval and so differs from upstream exactly when
    /// a track ends on a mesh plane. Upstream instead locates the start element
    /// `TINY_BIT` along `u`, walks the planes incrementally from `r0`, and stops
    /// once the distance travelled reaches `|r1 - r0|` (a crossing exactly at
    /// `r1` is not counted). [`MeshSurfaceFilter`] uses this one.
    ///
    /// [`MeshSurfaceFilter`]: crate::tally::filter_extra::MeshSurfaceFilter
    fn surface_bins_crossed_openmc(&self, r0: Position, r1: Position, u: Direction) -> Vec<usize>;
}

/// OpenMC's `TINY_BIT` (`include/openmc/constants.h`).
const TINY_BIT: f64 = 1.0e-8;

/// `RegularMesh::get_index_in_direction` (`src/mesh.cpp:1640`): the 1-based
/// index along axis `i`, 0 below the mesh and `shape + 1` above it.
fn regular_index_in_direction(m: &RegularMesh, r: f64, i: usize, width: f64) -> i64 {
    let shape = m.dimension[i] as i64;
    if r <= m.lower_left[i] {
        return if r == m.lower_left[i] { 1 } else { 0 };
    }
    if r >= m.upper_right[i] {
        return if r == m.upper_right[i] { shape } else { shape + 1 };
    }
    ((r - m.lower_left[i]) / width).ceil() as i64
}

/// `StructuredMesh::get_indices` (`src/mesh.cpp:1180`) on a 3-D regular mesh.
fn regular_indices(m: &RegularMesh, r: [f64; 3], width: [f64; 3]) -> ([i64; 3], bool) {
    let mut ijk = [0i64; 3];
    let mut in_mesh = true;
    for i in 0..3 {
        ijk[i] = regular_index_in_direction(m, r[i], i, width[i]);
        if ijk[i] < 1 || ijk[i] > m.dimension[i] as i64 {
            in_mesh = false;
        }
    }
    (ijk, in_mesh)
}

/// `MeshDistance` (`include/openmc/mesh.h:314`); the default is upstream's
/// `{0, false, INFTY}`.
#[derive(Clone, Copy)]
struct MeshDistance {
    next_index: i64,
    max_surface: bool,
    distance: f64,
}

/// `RegularMesh::distance_to_grid_boundary` (`src/mesh.cpp:1667`). The `l`
/// argument is unused upstream for a regular mesh, so it is not taken here.
fn regular_distance_to_grid_boundary(
    m: &RegularMesh,
    ijk: [i64; 3],
    i: usize,
    r0: [f64; 3],
    u: [f64; 3],
    width: [f64; 3],
) -> MeshDistance {
    let mut d = MeshDistance { next_index: ijk[i], max_surface: false, distance: f64::INFINITY };
    if u[i].abs() < FP_PRECISION {
        return d;
    }
    d.max_surface = u[i] > 0.0;
    if d.max_surface && ijk[i] <= m.dimension[i] as i64 {
        d.next_index += 1;
        d.distance = ((m.lower_left[i] + ijk[i] as f64 * width[i]) - r0[i]) / u[i];
    } else if !d.max_surface && ijk[i] >= 1 {
        d.next_index -= 1;
        d.distance = ((m.lower_left[i] + (ijk[i] - 1) as f64 * width[i]) - r0[i]) / u[i];
    }
    d
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
        events.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap_or(core::cmp::Ordering::Equal));

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

    fn surface_bins_crossed_openmc(&self, r0: Position, r1: Position, u: Direction) -> Vec<usize> {
        // gh:#1712: the walker is now generic over the four structured kinds;
        // on a regular mesh it runs the same operations in the same order as
        // the gh:#1709 body it replaced (pinned by
        // `tests/mesh_surface_curvilinear_1712.rs` and the #1709 tests).
        raytrace_surface_bins(self, r0, r1, u)
    }
}

/// OpenMC's `INFTY` (`include/openmc/constants.h:103`): `DBL_MAX`, not `inf`.
const INFTY: f64 = f64::MAX;

/// OpenMC's `RADIAL_MESH_TOL` (`include/openmc/constants.h:59`).
const RADIAL_MESH_TOL: f64 = 1.0e-10;

/// OpenMC's `PI` (`include/openmc/constants.h:101`), equal to
/// `core::f64::consts::PI` in `f64`.
const PI: f64 = core::f64::consts::PI;

/// `lower_bound_index` (`include/openmc/search.h:14`), **signed**: `-1` below
/// the grid, as upstream (the crate's [`crate::tally::filter::lower_bound_index`]
/// saturates at 0, which would put a point below the grid in element 1).
fn lower_bound_index_signed(grid: &[f64], x: f64) -> i64 {
    if grid[0] == x {
        return 0;
    }
    grid.partition_point(|&e| e < x) as i64 - 1
}

/// `std::min(a, b)` on `MeshDistance` (`include/openmc/mesh.h:321`, compares
/// `distance` only): `b` only when strictly smaller, else `a`.
fn min_distance(a: MeshDistance, b: MeshDistance) -> MeshDistance {
    if b.distance < a.distance {
        b
    } else {
        a
    }
}

/// `sanitize_angular_index` (`include/openmc/mesh.h:649`): wraps a periodic
/// (full-circle) angular index, sends an out-of-range one to 0 otherwise.
fn sanitize_angular_index(idx: i64, full: bool, n: i64) -> i64 {
    if idx > 0 && idx <= n {
        idx
    } else if full {
        (idx + n - 1) % n + 1
    } else {
        0
    }
}

/// What `StructuredMesh::raytrace_mesh` (`src/mesh.cpp:1312`) asks of a mesh.
/// A compile-time contract only (the walker is generic, monomorphised per
/// mesh type; no trait objects).
trait StructuredRaytrace {
    /// `shape_`: elements per axis.
    fn rt_shape(&self) -> [i64; 3];
    /// `local_coords` (`include/openmc/mesh.h:165`, `:480`).
    fn rt_local_coords(&self, r: [f64; 3]) -> [f64; 3];
    /// `get_indices(r, in_mesh)` on a **global** position (1-based).
    fn rt_indices(&self, r: [f64; 3]) -> ([i64; 3], bool);
    /// `distance_to_grid_boundary(ijk, i, r0, u, l)` from the **local** track
    /// start `r0`.
    fn distance_to_grid_boundary(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64)
        -> MeshDistance;
}

/// `StructuredMesh::get_indices` (`src/mesh.cpp:1180`) over a per-axis
/// `get_index_in_direction`.
fn structured_indices(shape: [i64; 3], r: [f64; 3], index: impl Fn(f64, usize) -> i64) -> ([i64; 3], bool) {
    let mut ijk = [0i64; 3];
    let mut in_mesh = true;
    for i in 0..3 {
        ijk[i] = index(r[i], i);
        if ijk[i] < 1 || ijk[i] > shape[i] {
            in_mesh = false;
        }
    }
    (ijk, in_mesh)
}

impl StructuredRaytrace for RegularMesh {
    fn rt_shape(&self) -> [i64; 3] {
        [self.dimension[0] as i64, self.dimension[1] as i64, self.dimension[2] as i64]
    }
    fn rt_local_coords(&self, r: [f64; 3]) -> [f64; 3] {
        r
    }
    fn rt_indices(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        regular_indices(self, r, self.width())
    }
    fn distance_to_grid_boundary(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], _l: f64) -> MeshDistance {
        regular_distance_to_grid_boundary(self, ijk, i, r0, u, self.width())
    }
}

impl StructuredRaytrace for RectilinearMesh {
    fn rt_shape(&self) -> [i64; 3] {
        let d = self.dimension();
        [d[0] as i64, d[1] as i64, d[2] as i64]
    }
    fn rt_local_coords(&self, r: [f64; 3]) -> [f64; 3] {
        r
    }
    /// `RectilinearMesh::get_index_in_direction` (`src/mesh.cpp:1885`):
    /// `lower_bound_index + 1`.
    fn rt_indices(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        structured_indices(self.rt_shape(), r, |x, i| lower_bound_index_signed(&self.grid[i], x) + 1)
    }
    /// `RectilinearMesh::distance_to_grid_boundary` (`src/mesh.cpp:1839`),
    /// with `positive_grid_boundary` / `negative_grid_boundary` (`:1827-1837`).
    fn distance_to_grid_boundary(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], _l: f64) -> MeshDistance {
        let mut d = MeshDistance { next_index: ijk[i], max_surface: true, distance: INFTY };
        if u[i].abs() < FP_PRECISION {
            return d;
        }
        d.max_surface = u[i] > 0.0;
        if d.max_surface && ijk[i] <= self.rt_shape()[i] {
            d.next_index += 1;
            d.distance = (self.grid[i][ijk[i] as usize] - r0[i]) / u[i];
        } else if !d.max_surface && ijk[i] > 0 {
            d.next_index -= 1;
            d.distance = (self.grid[i][(ijk[i] - 1) as usize] - r0[i]) / u[i];
        }
        d
    }
}

/// `CylindricalMesh::full_phi_` (`src/mesh.cpp`, `set_grid`).
fn cyl_full_phi(m: &CylindricalMesh) -> bool {
    m.phi_grid[0] == 0.0 && m.phi_grid[m.phi_grid.len() - 1] == 2.0 * PI
}

/// `CylindricalMesh::find_r_crossing` (`src/mesh.cpp:2028`).
fn cylindrical_find_r_crossing(m: &CylindricalMesh, r: [f64; 3], u: [f64; 3], l: f64, shell: i64) -> f64 {
    if shell < 0 || shell > m.rt_shape()[0] {
        return INFTY;
    }
    let r0 = m.r_grid[shell as usize];
    if r0 == 0.0 {
        return INFTY;
    }
    let denominator = u[0] * u[0] + u[1] * u[1];
    // Direction of flight is in z: never intersects r.
    if denominator.abs() < FP_PRECISION {
        return INFTY;
    }
    let inv_denominator = 1.0 / denominator;
    let p = (u[0] * r[0] + u[1] * r[1]) * inv_denominator;
    let big_r = (r[0] * r[0] + r[1] * r[1]).sqrt();
    let mut d = p * p - (big_r - r0) * (big_r + r0) * inv_denominator;
    if d < 0.0 {
        return INFTY;
    }
    d = d.sqrt();
    // Already on the shell surface: avoid a spurious crossing.
    if (big_r - r0).abs() <= RADIAL_MESH_TOL * (1.0 + r0.abs()) {
        return INFTY;
    }
    if -p - d > l {
        return -p - d;
    }
    if -p + d > l {
        return -p + d;
    }
    INFTY
}

/// `find_phi_crossing` (`src/mesh.cpp:2074` cylindrical, `:2427` spherical;
/// the two bodies are identical but for which axis holds phi).
fn find_phi_crossing(phi_grid: &[f64], full_phi: bool, n_phi: i64, r: [f64; 3], u: [f64; 3], l: f64, shell: i64) -> f64 {
    // A [0, 2 pi] grid with one bin has no real surface to cross.
    if full_phi && n_phi == 1 {
        return INFTY;
    }
    let shell = sanitize_angular_index(shell, full_phi, n_phi);
    let p0 = phi_grid[shell as usize];
    let c0 = p0.r_cos();
    let s0 = p0.r_sin();
    let denominator = u[0] * s0 - u[1] * c0;
    // Not parallel to the phi surface.
    if denominator.abs() > FP_PRECISION {
        let s = -(r[0] * s0 - r[1] * c0) / denominator;
        // Forward, and on the +phi half-plane (not -phi).
        if s > l && (c0 * (r[0] + s * u[0]) + s0 * (r[1] + s * u[1])) > 0.0 {
            return s;
        }
    }
    INFTY
}

impl StructuredRaytrace for CylindricalMesh {
    fn rt_shape(&self) -> [i64; 3] {
        let d = self.dimension();
        [d[0] as i64, d[1] as i64, d[2] as i64]
    }
    /// `PeriodicStructuredMesh::local_coords` (`include/openmc/mesh.h:480`).
    fn rt_local_coords(&self, r: [f64; 3]) -> [f64; 3] {
        [r[0] - self.origin.x, r[1] - self.origin.y, r[2] - self.origin.z]
    }
    /// `CylindricalMesh::get_indices` (`src/mesh.cpp:1980`) with
    /// `get_index_in_direction` (`:2196`): `in_mesh` is decided before the phi
    /// index is sanitised, as upstream.
    fn rt_indices(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        let r = self.rt_local_coords(r);
        let mut mapped = [r[0].r_hypot(r[1]), 0.0, r[2]];
        if mapped[0] < FP_PRECISION {
            mapped[1] = 0.0;
        } else {
            mapped[1] = r[1].r_atan2(r[0]);
            if mapped[1] < 0.0 {
                mapped[1] += 2.0 * PI;
            }
        }
        let grids = [&self.r_grid, &self.phi_grid, &self.z_grid];
        let (mut idx, in_mesh) =
            structured_indices(self.rt_shape(), mapped, |x, i| lower_bound_index_signed(grids[i], x) + 1);
        idx[1] = sanitize_angular_index(idx[1], cyl_full_phi(self), self.rt_shape()[1]);
        (idx, in_mesh)
    }
    /// `CylindricalMesh::distance_to_grid_boundary` (`src/mesh.cpp:2128`), with
    /// `find_z_crossing` (`:2107`) inline.
    fn distance_to_grid_boundary(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
        let shape = self.rt_shape();
        let full = cyl_full_phi(self);
        match i {
            0 => min_distance(
                MeshDistance { next_index: ijk[0] + 1, max_surface: true, distance: cylindrical_find_r_crossing(self, r0, u, l, ijk[0]) },
                MeshDistance { next_index: ijk[0] - 1, max_surface: false, distance: cylindrical_find_r_crossing(self, r0, u, l, ijk[0] - 1) },
            ),
            1 => min_distance(
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[1] + 1, full, shape[1]),
                    max_surface: true,
                    distance: find_phi_crossing(&self.phi_grid, full, shape[1], r0, u, l, ijk[1]),
                },
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[1] - 1, full, shape[1]),
                    max_surface: false,
                    distance: find_phi_crossing(&self.phi_grid, full, shape[1], r0, u, l, ijk[1] - 1),
                },
            ),
            _ => {
                // find_z_crossing: `MeshDistance d; d.next_index = shell;`.
                let shell = ijk[2];
                let mut d = MeshDistance { next_index: shell, max_surface: true, distance: INFTY };
                if u[2].abs() < FP_PRECISION {
                    return d;
                }
                d.max_surface = u[2] > 0.0;
                if d.max_surface && shell <= shape[2] {
                    d.next_index += 1;
                    d.distance = (self.z_grid[shell as usize] - r0[2]) / u[2];
                } else if !d.max_surface && shell > 0 {
                    d.next_index -= 1;
                    d.distance = (self.z_grid[(shell - 1) as usize] - r0[2]) / u[2];
                }
                d
            }
        }
    }
}

/// `SphericalMesh::full_theta_`, `full_phi_` (`src/mesh.cpp`, `set_grid`).
fn sph_full(m: &SphericalMesh) -> (bool, bool) {
    (
        m.theta_grid[0] == 0.0 && m.theta_grid[m.theta_grid.len() - 1] == PI,
        m.phi_grid[0] == 0.0 && m.phi_grid[m.phi_grid.len() - 1] == 2.0 * PI,
    )
}

/// `SphericalMesh::find_r_crossing` (`src/mesh.cpp:2332`); assumes `|u| = 1`.
fn spherical_find_r_crossing(m: &SphericalMesh, r: [f64; 3], u: [f64; 3], l: f64, shell: i64) -> f64 {
    if shell < 0 || shell > m.rt_shape()[0] {
        return INFTY;
    }
    let r0 = m.r_grid[shell as usize];
    if r0 == 0.0 {
        return INFTY;
    }
    let p = r[0] * u[0] + r[1] * u[1] + r[2] * u[2];
    let big_r = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let mut d = p * p - (big_r - r0) * (big_r + r0);
    if (big_r - r0).abs() <= RADIAL_MESH_TOL * (1.0 + r0.abs()) {
        return INFTY;
    }
    if d >= 0.0 {
        d = d.sqrt();
        if -p - d > l {
            return -p - d;
        }
        if -p + d > l {
            return -p + d;
        }
    }
    INFTY
}

/// `SphericalMesh::find_theta_crossing` (`src/mesh.cpp:2363`).
fn spherical_find_theta_crossing(m: &SphericalMesh, r: [f64; 3], u: [f64; 3], l: f64, shell: i64) -> f64 {
    let (full_theta, _) = sph_full(m);
    let n = m.rt_shape()[1];
    if full_theta && n == 1 {
        return INFTY;
    }
    let shell = sanitize_angular_index(shell, full_theta, n);
    let cos_t = m.theta_grid[shell as usize].r_cos();
    let sgn = cos_t.is_sign_negative();
    let cos_t_2 = cos_t * cos_t;
    let rdu = r[0] * u[0] + r[1] * u[1] + r[2] * u[2];
    let rdr = r[0] * r[0] + r[1] * r[1] + r[2] * r[2];
    let a = cos_t_2 - u[2] * u[2];
    let b = rdu * cos_t_2 - r[2] * u[2];
    let c = rdr * cos_t_2 - r[2] * r[2];
    // a == 0: flight parallel to the cone.
    if a.abs() < FP_PRECISION {
        if b.abs() < FP_PRECISION {
            return INFTY;
        }
        let s = -0.5 * c / b;
        if s > l && (r[2] + s * u[2]).is_sign_negative() == sgn {
            return s;
        }
        return INFTY;
    }
    let p = b / a;
    let mut d = p * p - c / a;
    if d < 0.0 {
        return INFTY;
    }
    d = d.sqrt();
    let s = -p - d;
    if s > l && (r[2] + s * u[2]).is_sign_negative() == sgn {
        return s;
    }
    let s = -p + d;
    if s > l && (r[2] + s * u[2]).is_sign_negative() == sgn {
        return s;
    }
    INFTY
}

impl StructuredRaytrace for SphericalMesh {
    fn rt_shape(&self) -> [i64; 3] {
        let d = self.dimension();
        [d[0] as i64, d[1] as i64, d[2] as i64]
    }
    /// `PeriodicStructuredMesh::local_coords` (`include/openmc/mesh.h:480`).
    fn rt_local_coords(&self, r: [f64; 3]) -> [f64; 3] {
        [r[0] - self.origin.x, r[1] - self.origin.y, r[2] - self.origin.z]
    }
    /// `SphericalMesh::get_indices` (`src/mesh.cpp:2278`) with
    /// `get_index_in_direction` (`:2530`). No clamp on `acos` (upstream has
    /// none; `sqrt(z^2) == |z|` exactly in IEEE, so `|z / r| <= 1`).
    fn rt_indices(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        let r = self.rt_local_coords(r);
        let mut mapped = [(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt(), 0.0, 0.0];
        if mapped[0] >= FP_PRECISION {
            mapped[1] = (r[2] / mapped[0]).r_acos();
            mapped[2] = r[1].r_atan2(r[0]);
            if mapped[2] < 0.0 {
                mapped[2] += 2.0 * PI;
            }
        }
        let grids = [&self.r_grid, &self.theta_grid, &self.phi_grid];
        let shape = self.rt_shape();
        let (mut idx, in_mesh) = structured_indices(shape, mapped, |x, i| lower_bound_index_signed(grids[i], x) + 1);
        let (full_theta, full_phi) = sph_full(self);
        idx[1] = sanitize_angular_index(idx[1], full_theta, shape[1]);
        idx[2] = sanitize_angular_index(idx[2], full_phi, shape[2]);
        (idx, in_mesh)
    }
    /// `SphericalMesh::distance_to_grid_boundary` (`src/mesh.cpp:2460`).
    fn distance_to_grid_boundary(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
        let shape = self.rt_shape();
        let (full_theta, full_phi) = sph_full(self);
        match i {
            0 => min_distance(
                MeshDistance { next_index: ijk[0] + 1, max_surface: true, distance: spherical_find_r_crossing(self, r0, u, l, ijk[0]) },
                MeshDistance { next_index: ijk[0] - 1, max_surface: false, distance: spherical_find_r_crossing(self, r0, u, l, ijk[0] - 1) },
            ),
            1 => min_distance(
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[1] + 1, full_theta, shape[1]),
                    max_surface: true,
                    distance: spherical_find_theta_crossing(self, r0, u, l, ijk[1]),
                },
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[1] - 1, full_theta, shape[1]),
                    max_surface: false,
                    distance: spherical_find_theta_crossing(self, r0, u, l, ijk[1] - 1),
                },
            ),
            _ => min_distance(
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[2] + 1, full_phi, shape[2]),
                    max_surface: true,
                    distance: find_phi_crossing(&self.phi_grid, full_phi, shape[2], r0, u, l, ijk[2]),
                },
                MeshDistance {
                    next_index: sanitize_angular_index(ijk[2] - 1, full_phi, shape[2]),
                    max_surface: false,
                    distance: find_phi_crossing(&self.phi_grid, full_phi, shape[2], r0, u, l, ijk[2] - 1),
                },
            ),
        }
    }
}

/// `StructuredMesh::get_bin_from_indices` (`src/mesh.cpp:1194`), 3-D case.
fn bin_from_indices(shape: [i64; 3], ijk: [i64; 3]) -> usize {
    (((ijk[2] - 1) * shape[1] + (ijk[1] - 1)) * shape[0] + ijk[0] - 1) as usize
}

/// `SurfaceAggregator::surface` (`src/mesh.cpp:1474`) for `n_dimension_ = 3`.
fn surface_bin_3d(element: usize, axis: usize, max: bool, inward: bool) -> usize {
    SURFACE_BINS_PER_ELEMENT * element + 4 * axis + if max { 2 } else { 0 } + if inward { 1 } else { 0 }
}

/// **Port of `StructuredMesh::raytrace_mesh` with its `SurfaceAggregator`**
/// (`src/mesh.cpp:1312-1488`, OpenMC `8f79d9d0`), generic over the four
/// structured meshes (gh:#1712; regular-only in gh:#1709).
///
/// Distances are measured from `local_coords(r0)` (origin subtracted once);
/// elements are located with `indices(r0 + s u)` on the **global** position,
/// each mesh subtracting its own origin (upstream keeps `global_r` for
/// exactly this). The track-length `tally.track` calls are no-ops in a
/// `SurfaceAggregator` and are not carried.
fn raytrace_surface_bins<M: StructuredRaytrace>(m: &M, r0: Position, r1: Position, u: Direction) -> Vec<usize> {
    let mut bins = Vec::new();
    let shape = m.rt_shape();
    // Compute the length of the entire track.
    let dr = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    let total_distance = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
    if total_distance == 0.0 {
        return bins;
    }
    let global_r = [r0.x, r0.y, r0.z];
    let local_r = m.rt_local_coords(global_r);
    let uu = [u.u, u.v, u.w];
    let at = |l: f64| [global_r[0] + l * uu[0], global_r[1] + l * uu[1], global_r[2] + l * uu[2]];
    let mut traveled_distance = 0.0f64;
    // Index of the current cell, offset a tiny bit in the direction of flight.
    let (mut ijk, mut in_mesh) = m.rt_indices(at(TINY_BIT));
    // A very short track is inside one cell and crosses no surface.
    if total_distance < 2.0 * TINY_BIT {
        return bins;
    }
    let mut distances = [MeshDistance { next_index: 0, max_surface: false, distance: f64::INFINITY }; 3];
    for (k, dk) in distances.iter_mut().enumerate() {
        *dk = m.distance_to_grid_boundary(ijk, k, local_r, uu, 0.0);
    }
    loop {
        if in_mesh {
            // The surface at minimal distance (the first one on a tie, as
            // `std::min_element`).
            let mut k = 0;
            for j in 1..3 {
                if distances[j].distance < distances[k].distance {
                    k = j;
                }
            }
            traveled_distance = distances[k].distance;
            if traveled_distance >= total_distance {
                return bins;
            }
            // Outward current from the cell being left.
            bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, distances[k].max_surface, false));
            ijk[k] = distances[k].next_index;
            distances[k] = m.distance_to_grid_boundary(ijk, k, local_r, uu, traveled_distance);
            in_mesh = ijk[k] >= 1 && ijk[k] <= shape[k];
            // Inward current into the next cell.
            if in_mesh {
                bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, !distances[k].max_surface, true));
            }
        } else {
            // Outside: travel the largest distance needed to cross every
            // outer surface.
            let mut k_max: Option<usize> = None;
            for k in 0..3 {
                if (ijk[k] < 1 || ijk[k] > shape[k]) && distances[k].distance > traveled_distance {
                    traveled_distance = distances[k].distance;
                    k_max = Some(k);
                }
            }
            if k_max.is_none() {
                traveled_distance += TINY_BIT;
            }
            if traveled_distance >= total_distance {
                return bins;
            }
            let (n, inside) = m.rt_indices(at(traveled_distance + TINY_BIT));
            ijk = n;
            in_mesh = inside;
            for (k, dk) in distances.iter_mut().enumerate() {
                *dk = m.distance_to_grid_boundary(ijk, k, local_r, uu, traveled_distance);
            }
            if let (true, Some(k)) = (in_mesh, k_max) {
                bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, !distances[k].max_surface, true));
            }
        }
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Corrected mesh-surface raytrace, BESIDE the port (gh:#1715).
//
// Maintainer, 2026-11-23: "Fix them beside the port". The port above
// (`raytrace_surface_bins` and every routine it calls) stays exactly as
// upstream and is reached by name (`MeshSurfaceRaytrace::OpenMc`). This route
// fixes three defects that the code-to-code runs showed OpenMC 0.15.3
// (`27e38e89`) itself has (gh:#1712 findings A, B, C):
//
// - A. The inward current is labelled by the face the particle ENTERED, not
//   by the face it will leave through next (`mesh.cpp:1393,1427` take
//   `!distances[k].max_surface` after the distance is recomputed). On a
//   Cartesian axis the two are always the same face, so regular and
//   rectilinear meshes give identical bins on both routes.
// - B. On a partial angular axis an out-of-range index is not sent to 0
//   (`sanitize_angular_index`, `mesh.h:649`). The gap outside a partial phi
//   wedge is one region (phi wraps): its boundaries are phi_min going up and
//   phi_max going down. Outside a partial theta range the boundary is
//   theta_min from below and theta_max from above (theta does not wrap).
// - C. A theta edge with |cos theta| < FP_PRECISION (theta = pi/2) is the plane
//   z = 0 and is crossed as a plane; the cone quadratic is degenerate there and
//   upstream loses the crossing to the sign of round-off.
//
// New work, not a port: say so where it is used.
// ────────────────────────────────────────────────────────────────────────────

/// Which mesh-surface raytrace a [`crate::tally::filter_extra::MeshSurfaceFilter`]
/// uses (gh:#1715).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeshSurfaceRaytrace {
    /// The corrected route (the default, by the correct-physics-default
    /// rule): OpenMC's walk with its three defects fixed (gh:#1712 findings
    /// A, B, C; see the comment above this enum in `tally/mesh.rs`). Identical
    /// to [`Self::OpenMc`] on regular and rectilinear meshes.
    #[default]
    Corrected,
    /// The port of upstream's `raytrace_mesh`, defects included: for
    /// code-to-code comparison with OpenMC.
    OpenMc,
}

/// The per-mesh hooks the corrected walker needs; the defaults are the port's
/// own routines, which are correct on Cartesian axes (gh:#1715).
trait CorrectedRaytrace: StructuredRaytrace {
    /// Element indices with out-of-range angular indices kept out of range
    /// (finding B).
    fn rt_indices_corrected(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        self.rt_indices(r)
    }
    /// Distance to the next grid boundary on axis `i`, with findings B and C
    /// fixed.
    fn distance_corrected(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
        self.distance_to_grid_boundary(ijk, i, r0, u, l)
    }
}

impl CorrectedRaytrace for RegularMesh {}
impl CorrectedRaytrace for RectilinearMesh {}

/// Index along a partial phi axis with the gap kept as one out-of-range
/// region `n + 1` (finding B); a full axis wraps as upstream.
fn corrected_phi_index(idx: i64, full: bool, n: i64) -> i64 {
    if full {
        sanitize_angular_index(idx, full, n)
    } else if idx < 1 || idx > n {
        n + 1
    } else {
        idx
    }
}

/// Phi-axis distance with the gap handled (finding B): from the gap, the
/// boundary going up is `phi_grid[0]` (entered as element 1, through its min
/// face) and going down is `phi_grid[n]` (entered as element n, max face).
fn corrected_phi_distance(phi_grid: &[f64], full: bool, n: i64, ijk1: i64, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
    if !full && (ijk1 < 1 || ijk1 > n) {
        return min_distance(
            MeshDistance { next_index: 1, max_surface: true, distance: find_phi_crossing(phi_grid, full, n, r0, u, l, 0) },
            MeshDistance { next_index: n, max_surface: false, distance: find_phi_crossing(phi_grid, full, n, r0, u, l, n) },
        );
    }
    min_distance(
        MeshDistance {
            next_index: corrected_phi_index(ijk1 + 1, full, n),
            max_surface: true,
            distance: find_phi_crossing(phi_grid, full, n, r0, u, l, ijk1),
        },
        MeshDistance {
            next_index: corrected_phi_index(ijk1 - 1, full, n),
            max_surface: false,
            distance: find_phi_crossing(phi_grid, full, n, r0, u, l, ijk1 - 1),
        },
    )
}

impl CorrectedRaytrace for CylindricalMesh {
    fn rt_indices_corrected(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        let r = self.rt_local_coords(r);
        let mut mapped = [r[0].r_hypot(r[1]), 0.0, r[2]];
        if mapped[0] >= FP_PRECISION {
            mapped[1] = r[1].r_atan2(r[0]);
            if mapped[1] < 0.0 {
                mapped[1] += 2.0 * PI;
            }
        }
        let grids = [&self.r_grid, &self.phi_grid, &self.z_grid];
        let shape = self.rt_shape();
        let (mut idx, in_mesh) = structured_indices(shape, mapped, |x, i| lower_bound_index_signed(grids[i], x) + 1);
        idx[1] = corrected_phi_index(idx[1], cyl_full_phi(self), shape[1]);
        (idx, in_mesh)
    }
    fn distance_corrected(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
        if i == 1 {
            corrected_phi_distance(&self.phi_grid, cyl_full_phi(self), self.rt_shape()[1], ijk[1], r0, u, l)
        } else {
            self.distance_to_grid_boundary(ijk, i, r0, u, l)
        }
    }
}

/// `find_theta_crossing` with finding C fixed: a theta edge with
/// `|cos theta| < FP_PRECISION` is the plane z = 0, crossed where
/// `r_z + s u_z = 0`. Other edges are the port's routine unchanged.
fn corrected_theta_crossing(m: &SphericalMesh, r: [f64; 3], u: [f64; 3], l: f64, shell: i64) -> f64 {
    let n = m.rt_shape()[1];
    if shell < 0 || shell > n {
        return INFTY;
    }
    let cos_t = m.theta_grid[shell as usize].r_cos();
    if cos_t.abs() < FP_PRECISION {
        if u[2].abs() < FP_PRECISION {
            return INFTY;
        }
        let s = -r[2] / u[2];
        return if s > l { s } else { INFTY };
    }
    spherical_find_theta_crossing(m, r, u, l, shell)
}

impl CorrectedRaytrace for SphericalMesh {
    fn rt_indices_corrected(&self, r: [f64; 3]) -> ([i64; 3], bool) {
        let r = self.rt_local_coords(r);
        let mut mapped = [(r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt(), 0.0, 0.0];
        if mapped[0] >= FP_PRECISION {
            mapped[1] = (r[2] / mapped[0]).r_acos();
            mapped[2] = r[1].r_atan2(r[0]);
            if mapped[2] < 0.0 {
                mapped[2] += 2.0 * PI;
            }
        }
        let grids = [&self.r_grid, &self.theta_grid, &self.phi_grid];
        let shape = self.rt_shape();
        let (mut idx, in_mesh) = structured_indices(shape, mapped, |x, i| lower_bound_index_signed(grids[i], x) + 1);
        let (full_theta, full_phi) = sph_full(self);
        // Theta does not wrap: keep 0 (below theta_min) and n + 1 (above
        // theta_max) as they are; a full theta range has no outside.
        if full_theta {
            idx[1] = sanitize_angular_index(idx[1], full_theta, shape[1]);
        }
        idx[2] = corrected_phi_index(idx[2], full_phi, shape[2]);
        (idx, in_mesh)
    }
    fn distance_corrected(&self, ijk: [i64; 3], i: usize, r0: [f64; 3], u: [f64; 3], l: f64) -> MeshDistance {
        let shape = self.rt_shape();
        let (full_theta, full_phi) = sph_full(self);
        match i {
            1 => {
                let n = shape[1];
                if full_theta && n == 1 {
                    return MeshDistance { next_index: ijk[1], max_surface: true, distance: INFTY };
                }
                // Below theta_min only theta_min can be crossed (going up),
                // above theta_max only theta_max (going down).
                let up = if ijk[1] <= n { corrected_theta_crossing(self, r0, u, l, ijk[1].max(0)) } else { INFTY };
                let down = if ijk[1] >= 1 { corrected_theta_crossing(self, r0, u, l, (ijk[1] - 1).min(n)) } else { INFTY };
                min_distance(
                    MeshDistance { next_index: ijk[1].max(0) + 1, max_surface: true, distance: up },
                    MeshDistance { next_index: ijk[1].min(n + 1) - 1, max_surface: false, distance: down },
                )
            }
            2 => corrected_phi_distance(&self.phi_grid, full_phi, shape[2], ijk[2], r0, u, l),
            _ => self.distance_to_grid_boundary(ijk, i, r0, u, l),
        }
    }
}

/// The corrected mesh-surface walk (gh:#1715): [`raytrace_surface_bins`] with
/// the inward current labelled by the face entered (finding A) and the
/// corrected per-mesh hooks (findings B, C). New work beside the port.
fn raytrace_surface_bins_corrected<M: CorrectedRaytrace>(m: &M, r0: Position, r1: Position, u: Direction) -> Vec<usize> {
    let mut bins = Vec::new();
    let shape = m.rt_shape();
    let dr = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    let total_distance = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();
    if total_distance == 0.0 {
        return bins;
    }
    let global_r = [r0.x, r0.y, r0.z];
    let local_r = m.rt_local_coords(global_r);
    let uu = [u.u, u.v, u.w];
    let at = |l: f64| [global_r[0] + l * uu[0], global_r[1] + l * uu[1], global_r[2] + l * uu[2]];
    let mut traveled_distance = 0.0f64;
    let (mut ijk, mut in_mesh) = m.rt_indices_corrected(at(TINY_BIT));
    if total_distance < 2.0 * TINY_BIT {
        return bins;
    }
    let mut distances = [MeshDistance { next_index: 0, max_surface: false, distance: f64::INFINITY }; 3];
    for (k, dk) in distances.iter_mut().enumerate() {
        *dk = m.distance_corrected(ijk, k, local_r, uu, 0.0);
    }
    loop {
        if in_mesh {
            let mut k = 0;
            for j in 1..3 {
                if distances[j].distance < distances[k].distance {
                    k = j;
                }
            }
            traveled_distance = distances[k].distance;
            if traveled_distance >= total_distance {
                return bins;
            }
            let left_through_max = distances[k].max_surface;
            bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, left_through_max, false));
            ijk[k] = distances[k].next_index;
            distances[k] = m.distance_corrected(ijk, k, local_r, uu, traveled_distance);
            in_mesh = ijk[k] >= 1 && ijk[k] <= shape[k];
            // Finding A: the face entered is opposite the one left.
            if in_mesh {
                bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, !left_through_max, true));
            }
        } else {
            let mut k_max: Option<usize> = None;
            let mut entered_max = false;
            for k in 0..3 {
                if (ijk[k] < 1 || ijk[k] > shape[k]) && distances[k].distance > traveled_distance {
                    traveled_distance = distances[k].distance;
                    k_max = Some(k);
                    // Finding A: arriving from above the axis enters through
                    // the max face, from below through the min face; on a
                    // wrapped phi gap the boundary crossed says which.
                    entered_max = !distances[k].max_surface;
                }
            }
            if k_max.is_none() {
                traveled_distance += TINY_BIT;
            }
            if traveled_distance >= total_distance {
                return bins;
            }
            let (n, inside) = m.rt_indices_corrected(at(traveled_distance + TINY_BIT));
            ijk = n;
            in_mesh = inside;
            for (k, dk) in distances.iter_mut().enumerate() {
                *dk = m.distance_corrected(ijk, k, local_r, uu, traveled_distance);
            }
            if let (true, Some(k)) = (in_mesh, k_max) {
                bins.push(surface_bin_3d(bin_from_indices(shape, ijk), k, entered_max, true));
            }
        }
    }
}

/// Every mesh-surface bin the track `r0 -> r1` (direction `u`) crosses on
/// `mesh`, by the chosen raytrace (gh:#1715); `None` for an unstructured mesh.
pub fn mesh_surface_bins_crossed(
    mesh: &MeshKind,
    r0: Position,
    r1: Position,
    u: Direction,
    raytrace: MeshSurfaceRaytrace,
) -> Option<Vec<usize>> {
    match raytrace {
        MeshSurfaceRaytrace::OpenMc => mesh.surface_bins_crossed_openmc(r0, r1, u),
        MeshSurfaceRaytrace::Corrected => match mesh {
            MeshKind::Regular(m) => Some(raytrace_surface_bins_corrected(m, r0, r1, u)),
            MeshKind::Rectilinear(m) => Some(raytrace_surface_bins_corrected(m, r0, r1, u)),
            MeshKind::Cylindrical(m) => Some(raytrace_surface_bins_corrected(m, r0, r1, u)),
            MeshKind::Spherical(m) => Some(raytrace_surface_bins_corrected(m, r0, r1, u)),
            MeshKind::Unstructured(_) => None,
        },
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

        let r = x.r_hypot(y);
        let phi = if r < FP_PRECISION {
            0.0
        } else {
            let a = y.r_atan2(x);
            if a < 0.0 {
                a + core::f64::consts::TAU
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
            let a = y.r_atan2(x);
            (
                (z / r).clamp(-1.0, 1.0).r_acos(),
                if a < 0.0 {
                    a + core::f64::consts::TAU
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
    /// Every mesh-surface bin the track `r0 -> r1` (direction `u`) crosses, in
    /// order of travel: the port of `StructuredMesh::surface_bins_crossed`
    /// (`src/mesh.cpp:1460-1488`) on any of the four structured kinds
    /// (gh:#1712), bin layout [`RegularMeshExt::surface_bin`]'s. `None` for an
    /// unstructured mesh, where upstream `fatal_error`s
    /// (`UnstructuredMesh::surface_bins_crossed`, `src/mesh.cpp:1087`).
    fn surface_bins_crossed_openmc(&self, r0: Position, r1: Position, u: Direction) -> Option<Vec<usize>>;
    /// `StructuredMesh::n_surface_bins` (`src/mesh.cpp:1249`):
    /// [`SURFACE_BINS_PER_ELEMENT`] per element; `None` for an unstructured mesh.
    fn n_surface_bins(&self) -> Option<usize>;
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
            // gh:#678: planar grids split a segment at every grid plane, as
            // `StructuredMesh::bins_crossed` does. ~~All four structured kinds
            // scored the whole segment into its midpoint's bin~~ **CORRECTED
            // 2026-11-19** for regular and rectilinear meshes, and for
            // cylindrical and spherical ones (quadratic crossings).
            Self::Regular(_)
            | Self::Rectilinear(_)
            | Self::Cylindrical(_)
            | Self::Spherical(_)
            | Self::Unstructured(_) => true,
        }
    }

    fn bins_crossed(&self, r0: Position, r1: Position) -> Option<Vec<(usize, f64)>> {
        match self {
            Self::Regular(m) => {
                let edges: [Vec<f64>; 3] = core::array::from_fn(|a| {
                    let n = m.dimension[a];
                    let w = (m.upper_right[a] - m.lower_left[a]) / n as f64;
                    (0..=n).map(|k| m.lower_left[a] + k as f64 * w).collect()
                });
                Some(planar_bins_crossed(&edges, r0, r1, |p| m.get_bin(p)))
            }
            Self::Rectilinear(m) => Some(planar_bins_crossed(&m.grid, r0, r1, |p| m.bin(p))),
            Self::Cylindrical(m) => Some(cylindrical_bins_crossed(m, r0, r1)),
            Self::Spherical(m) => Some(spherical_bins_crossed(m, r0, r1)),
            Self::Unstructured(m) => Some(m.bins_crossed(r0, r1)),
        }
    }

    fn surface_bins_crossed_openmc(&self, r0: Position, r1: Position, u: Direction) -> Option<Vec<usize>> {
        match self {
            Self::Regular(m) => Some(raytrace_surface_bins(m, r0, r1, u)),
            Self::Rectilinear(m) => Some(raytrace_surface_bins(m, r0, r1, u)),
            Self::Cylindrical(m) => Some(raytrace_surface_bins(m, r0, r1, u)),
            Self::Spherical(m) => Some(raytrace_surface_bins(m, r0, r1, u)),
            Self::Unstructured(_) => None,
        }
    }

    fn n_surface_bins(&self) -> Option<usize> {
        match self {
            Self::Unstructured(_) => None,
            other => Some(SURFACE_BINS_PER_ELEMENT * other.n_bins()),
        }
    }
}

/// `(bin, length fraction)` for each cell of a planar grid that the segment
/// `r0 -> r1` crosses (gh:#678, after `StructuredMesh::bins_crossed`,
/// `src/mesh.cpp`): the segment is cut at every grid plane it crosses and each
/// piece is binned at its own midpoint, so a piece outside the mesh scores
/// nothing and the fractions sum to the in-mesh share of the segment.
fn planar_bins_crossed(
    edges: &[Vec<f64>; 3],
    r0: Position,
    r1: Position,
    bin: impl Fn(Position) -> Option<usize>,
) -> Vec<(usize, f64)> {
    let o = [r0.x, r0.y, r0.z];
    let d = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    let mut ts = vec![0.0_f64, 1.0];
    for a in 0..3 {
        if d[a] == 0.0 {
            continue;
        }
        for &e in &edges[a] {
            let t = (e - o[a]) / d[a];
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    }
    bins_from_cuts(ts, r0, r1, bin)
}

/// Sort the cut parameters `ts` (including 0 and 1), bin each piece at its
/// midpoint, and merge neighbouring pieces in the same bin. Surplus cuts that do
/// not change the bin are harmless, so callers may add candidates liberally.
fn bins_from_cuts(
    mut ts: Vec<f64>,
    r0: Position,
    r1: Position,
    bin: impl Fn(Position) -> Option<usize>,
) -> Vec<(usize, f64)> {
    let o = [r0.x, r0.y, r0.z];
    let d = [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z];
    ts.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut out: Vec<(usize, f64)> = Vec::new();
    for w in ts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b <= a {
            continue;
        }
        let m = 0.5 * (a + b);
        let mid = Position::new(o[0] + m * d[0], o[1] + m * d[1], o[2] + m * d[2]);
        if let Some(i) = bin(mid) {
            match out.last_mut() {
                Some(last) if last.0 == i => last.1 += b - a,
                _ => out.push((i, b - a)),
            }
        }
    }
    out
}

/// Push the roots in `(0, 1)` of `a t^2 + b t + c = 0` onto `ts`.
fn push_quadratic_roots(ts: &mut Vec<f64>, a: f64, b: f64, c: f64) {
    if a == 0.0 {
        if b != 0.0 {
            let t = -c / b;
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
        return;
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return;
    }
    let q = disc.sqrt();
    for t in [(-b - q) / (2.0 * a), (-b + q) / (2.0 * a)] {
        if t > 0.0 && t < 1.0 {
            ts.push(t);
        }
    }
}

/// Cut parameters of the segment `p(t) = r0 + t (r1 - r0)` against the azimuthal
/// half-planes of angle `phi` through the axis `(ox, oy)`: the full plane is
/// used, its mirror half only adds a harmless cut.
fn push_azimuthal_cuts(ts: &mut Vec<f64>, phis: &[f64], o: [f64; 2], p0: [f64; 2], d: [f64; 2]) {
    for &phi in phis {
        let (nx, ny) = (-phi.r_sin(), phi.r_cos());
        let den = nx * d[0] + ny * d[1];
        if den != 0.0 {
            let t = -(nx * (p0[0] - o[0]) + ny * (p0[1] - o[1])) / den;
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    }
}

/// Cylindrical-mesh `bins_crossed` (gh:#678): cuts at the radial cylinders, the
/// axial planes and the azimuthal planes.
fn cylindrical_bins_crossed(
    m: &CylindricalMesh,
    r0: Position,
    r1: Position,
) -> Vec<(usize, f64)> {
    let o = m.origin;
    let (p0, d) = ([r0.x - o.x, r0.y - o.y, r0.z - o.z], [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z]);
    let mut ts = vec![0.0_f64, 1.0];
    for &r in &m.r_grid {
        push_quadratic_roots(
            &mut ts,
            d[0] * d[0] + d[1] * d[1],
            2.0 * (p0[0] * d[0] + p0[1] * d[1]),
            p0[0] * p0[0] + p0[1] * p0[1] - r * r,
        );
    }
    if d[2] != 0.0 {
        for &z in &m.z_grid {
            let t = (z - p0[2]) / d[2];
            if t > 0.0 && t < 1.0 {
                ts.push(t);
            }
        }
    }
    push_azimuthal_cuts(&mut ts, &m.phi_grid, [0.0, 0.0], [p0[0], p0[1]], [d[0], d[1]]);
    bins_from_cuts(ts, r0, r1, |p| m.bin(p))
}

/// Spherical-mesh `bins_crossed` (gh:#678): cuts at the radial spheres, the polar
/// cones (`p_z^2 = cos^2(theta) |p|^2`, both nappes) and the azimuthal planes.
fn spherical_bins_crossed(m: &SphericalMesh, r0: Position, r1: Position) -> Vec<(usize, f64)> {
    let o = m.origin;
    let (p0, d) = ([r0.x - o.x, r0.y - o.y, r0.z - o.z], [r1.x - r0.x, r1.y - r0.y, r1.z - r0.z]);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut ts = vec![0.0_f64, 1.0];
    for &r in &m.r_grid {
        push_quadratic_roots(&mut ts, dot(d, d), 2.0 * dot(p0, d), dot(p0, p0) - r * r);
    }
    for &th in &m.theta_grid {
        let c2 = th.r_cos() * th.r_cos();
        push_quadratic_roots(
            &mut ts,
            d[2] * d[2] - c2 * dot(d, d),
            2.0 * (p0[2] * d[2] - c2 * dot(p0, d)),
            p0[2] * p0[2] - c2 * dot(p0, p0),
        );
    }
    push_azimuthal_cuts(&mut ts, &m.phi_grid, [0.0, 0.0], [p0[0], p0[1]], [d[0], d[1]]);
    bins_from_cuts(ts, r0, r1, |p| m.bin(p))
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    /// gh:#678: a chord through the axis/centre of a cylindrical and a spherical
    /// mesh with radial edges `[0, 1, 2]`: x from -3 to 3, so 1 cm of each outer
    /// shell, 2 cm of the core, and 1 cm outside on each side that scores nothing.
    /// Fractions (of the 6 cm segment): 1/6, 2/6, 1/6, summing to the in-mesh 4/6.
    #[test]
    fn curved_mesh_track_length_is_split_at_the_radial_surfaces() {
        let (a, b) = (Position::new(-3.0, 0.0, 0.1), Position::new(3.0, 0.0, 0.1));
        let two_pi = 2.0 * core::f64::consts::PI;
        let cyl = MeshKind::Cylindrical(CylindricalMesh {
            r_grid: vec![0.0, 1.0, 2.0],
            phi_grid: vec![0.0, two_pi],
            z_grid: vec![-1.0, 1.0],
            origin: Position::ZERO,
        });
        let sph = MeshKind::Spherical(SphericalMesh {
            r_grid: vec![0.0, 1.0, 2.0],
            theta_grid: vec![0.0, core::f64::consts::PI],
            phi_grid: vec![0.0, two_pi],
            origin: Position::ZERO,
        });
        for (name, m) in [("cylindrical", cyl), ("spherical", sph)] {
            assert!(m.splits_track_length(), "{name}");
            let r = m.bins_crossed(a, b).unwrap();
            assert_eq!(r.len(), 3, "{name}: {r:?}");
            let f = |i: usize| r[i].1;
            assert!((f(0) - 1.0 / 6.0).abs() < 1e-3 && (f(1) - 2.0 / 6.0).abs() < 2e-3 && (f(2) - 1.0 / 6.0).abs() < 1e-3, "{name}: {r:?}");
            assert_eq!(r[0].0, r[2].0, "{name}: both outer shells are the same bin");
            assert_ne!(r[0].0, r[1].0, "{name}: the core is another");
        }
    }

    /// gh:#678: the issue's case. Segment (-1,0,0)->(0.2,0,0) over a one-cell-in-x
    /// mesh on [0,1]^3: only 0.2 of the 1.2 cm is inside, so the fraction is
    /// 0.2/1.2 (upstream scores 0.2 cm; the midpoint rule scored 0). A segment
    /// across two cells splits at the plane, and the fractions sum to 1.
    #[test]
    fn planar_mesh_track_length_is_split_at_the_grid_planes() {
        let one = MeshKind::Regular(RegularMesh {
            lower_left: [0.0; 3],
            upper_right: [1.0; 3],
            dimension: [1, 1, 1],
        });
        assert!(one.splits_track_length());
        let r = one
            .bins_crossed(Position::new(-1.0, 0.5, 0.5), Position::new(0.2, 0.5, 0.5))
            .unwrap();
        assert_eq!(r.len(), 1);
        assert!((r[0].1 - 0.2 / 1.2).abs() < 1e-12, "{r:?}");

        let two = MeshKind::Regular(RegularMesh {
            lower_left: [0.0; 3],
            upper_right: [2.0, 1.0, 1.0],
            dimension: [2, 1, 1],
        });
        let r = two
            .bins_crossed(Position::new(0.5, 0.5, 0.5), Position::new(1.5, 0.5, 0.5))
            .unwrap();
        assert_eq!(r.iter().map(|x| x.0).collect::<Vec<_>>(), vec![0, 1]);
        assert!((r[0].1 - 0.5).abs() < 1e-12 && (r[1].1 - 0.5).abs() < 1e-12);

        let rect = MeshKind::Rectilinear(RectilinearMesh {
            grid: [vec![0.0, 0.25, 1.0], vec![0.0, 1.0], vec![0.0, 1.0]],
        });
        let r = rect
            .bins_crossed(Position::new(0.0, 0.5, 0.5), Position::new(1.0, 0.5, 0.5))
            .unwrap();
        assert!((r[0].1 - 0.25).abs() < 1e-12 && (r[1].1 - 0.75).abs() < 1e-12, "{r:?}");
    }

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
