//! Structured spatial meshes for tally binning.
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
//! Still absent, and still a real gap: the **unstructured** mesh family, which
//! is planned via OpenFOAM `polyMesh` reuse and is explicitly out of scope for
//! #260. Also still a gap, unchanged by this work and pre-dating it: the
//! track-length **`bins_crossed`** sub-segmentation, so a segment is scored
//! whole into its midpoint's cell rather than split across the cells it
//! actually crosses. That approximation is exact only while a mesh cell is
//! large relative to the mean free path, and it is **more** wrong on a
//! cylindrical mesh than a Cartesian one, because a radial cell's width varies
//! across it. Worth knowing before using a fine R-Z mesh.

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
    pub fn get_bin(&self, p: Position) -> Option<usize> {
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
    pub fn count_sites(&self, sites: &[crate::particle::bank::BankSite]) -> (Vec<f64>, bool) {
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
    pub fn shannon_entropy(&self, sites: &[crate::particle::bank::BankSite]) -> Option<f64> {
        let (counts, _outside) = self.count_sites(sites);
        let total: f64 = counts.iter().sum();
        if !(total > 0.0) {
            return None;
        }
        let mut h = 0.0;
        for c in &counts {
            let p = c / total;
            if p > 0.0 {
                h -= p * p.log2();
            }
        }
        Some(h)
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

// ── Rectilinear / cylindrical / spherical meshes (GitHub #260) ───────────────
//
// ~~"rectilinear / cylindrical / spherical meshes are a documented gap"~~
// **CORRECTED 2026-09-22** — the module doc above is struck where it says so.
// All three are below, ported from `src/mesh.cpp` at OpenMC `afa7a14`. (The
// commit the issue cites, `608a1c33`, is unavailable in this container and not
// fetchable; see
// `verification_and_validation/white_boundary/white_boundary_vs_openmc.md`.)

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

/// **Rectilinear** mesh: explicit, non-uniform bin edges on each axis.
///
/// `openmc::RectilinearMesh`. This is the cheap one, and it is what a radial
/// power profile with finer edge binning actually needs.
#[derive(Debug, Clone, PartialEq)]
pub struct RectilinearMesh {
    /// Ascending bin edges along x, y, z. Each needs at least two entries.
    pub grid: [Vec<f64>; 3],
}

impl RectilinearMesh {
    /// Number of bins along each axis.
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.grid[0].len().saturating_sub(1),
            self.grid[1].len().saturating_sub(1),
            self.grid[2].len().saturating_sub(1),
        ]
    }

    /// Total bins.
    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// `(i, j, k)` of the bin containing `p`, or `None` if outside.
    pub fn indices(&self, p: Position) -> Option<[usize; 3]> {
        Some([
            bin_on_grid(&self.grid[0], p.x)?,
            bin_on_grid(&self.grid[1], p.y)?,
            bin_on_grid(&self.grid[2], p.z)?,
        ])
    }

    /// Flat bin index, x fastest — the same ordering [`RegularMesh`] uses.
    pub fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:1867`, the product of
    /// the three edge differences.
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        (0..3)
            .map(|a| self.grid[a][ijk[a] + 1] - self.grid[a][ijk[a]])
            .product()
    }
}

/// **Cylindrical** `(r, phi, z)` mesh about `origin`.
///
/// `openmc::CylindricalMesh`. This is the natural tally geometry for every core
/// model in this repository — the workspace's standing correction is that
/// reactor cores are R-Z, not slabs.
///
/// `phi` is measured from the +x axis and is mapped into `[0, 2 pi)`, matching
/// `src/mesh.cpp:1932`. `z` is absolute (relative to `origin.z`).
#[derive(Debug, Clone, PartialEq)]
pub struct CylindricalMesh {
    /// Ascending radial edges \[cm\].
    pub r_grid: Vec<f64>,
    /// Ascending azimuthal edges \[rad\], within `[0, 2 pi]`.
    pub phi_grid: Vec<f64>,
    /// Ascending axial edges \[cm\], relative to `origin`.
    pub z_grid: Vec<f64>,
    /// Mesh origin \[cm\].
    pub origin: Position,
}

impl CylindricalMesh {
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.r_grid.len().saturating_sub(1),
            self.phi_grid.len().saturating_sub(1),
            self.z_grid.len().saturating_sub(1),
        ]
    }

    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// `(i_r, i_phi, i_z)` of the bin containing `p`, or `None` if outside.
    ///
    /// Ported from `CylindricalMesh::get_indices` (`src/mesh.cpp:1920`),
    /// including the `r < FP_PRECISION` guard that pins `phi = 0` on the axis
    /// rather than letting `atan2(0, 0)` decide it.
    pub fn indices(&self, p: Position) -> Option<[usize; 3]> {
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

    /// Flat bin index, r fastest.
    pub fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:2159`,
    /// `0.5 (r_o^2 - r_i^2) (phi_o - phi_i) (z_o - z_i)`.
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        let (ri, ro) = (self.r_grid[ijk[0]], self.r_grid[ijk[0] + 1]);
        let (pi_, po) = (self.phi_grid[ijk[1]], self.phi_grid[ijk[1] + 1]);
        let (zi, zo) = (self.z_grid[ijk[2]], self.z_grid[ijk[2] + 1]);
        0.5 * (ro * ro - ri * ri) * (po - pi_) * (zo - zi)
    }
}

/// **Spherical** `(r, theta, phi)` mesh about `origin`.
///
/// `openmc::SphericalMesh`. `theta` is the **polar** angle from +z in
/// `[0, pi]`; `phi` the azimuth from +x in `[0, 2 pi)`. That ordering is
/// upstream's (`src/mesh.cpp:2230`) and is the opposite of the physics
/// convention some texts use, which is exactly the kind of thing that produces
/// a mesh that looks right and bins wrong.
#[derive(Debug, Clone, PartialEq)]
pub struct SphericalMesh {
    /// Ascending radial edges \[cm\].
    pub r_grid: Vec<f64>,
    /// Ascending polar edges \[rad\], within `[0, pi]`.
    pub theta_grid: Vec<f64>,
    /// Ascending azimuthal edges \[rad\], within `[0, 2 pi]`.
    pub phi_grid: Vec<f64>,
    /// Mesh origin \[cm\].
    pub origin: Position,
}

impl SphericalMesh {
    pub fn dimension(&self) -> [usize; 3] {
        [
            self.r_grid.len().saturating_sub(1),
            self.theta_grid.len().saturating_sub(1),
            self.phi_grid.len().saturating_sub(1),
        ]
    }

    pub fn n_bins(&self) -> usize {
        let d = self.dimension();
        d[0] * d[1] * d[2]
    }

    /// `(i_r, i_theta, i_phi)`, or `None` if outside. `src/mesh.cpp:2218`.
    pub fn indices(&self, p: Position) -> Option<[usize; 3]> {
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
                if a < 0.0 { a + std::f64::consts::TAU } else { a },
            )
        };
        Some([
            bin_on_grid(&self.r_grid, r)?,
            bin_on_grid(&self.theta_grid, theta)?,
            bin_on_grid(&self.phi_grid, phi)?,
        ])
    }

    /// Flat bin index, r fastest.
    pub fn bin(&self, p: Position) -> Option<usize> {
        let [i, j, k] = self.indices(p)?;
        let d = self.dimension();
        Some(i + d[0] * (j + d[1] * k))
    }

    /// Volume of bin `(i, j, k)` \[cm^3\]: `src/mesh.cpp:2493`,
    /// `(1/3) (r_o^3 - r_i^3) (cos theta_i - cos theta_o) (phi_o - phi_i)`.
    ///
    /// Note the cosine difference is `inner - outer`: `cos` decreases on
    /// `[0, pi]`, so that ordering is what keeps the volume positive.
    pub fn volume(&self, ijk: [usize; 3]) -> f64 {
        let (ri, ro) = (self.r_grid[ijk[0]], self.r_grid[ijk[0] + 1]);
        let (ti, to) = (self.theta_grid[ijk[1]], self.theta_grid[ijk[1] + 1]);
        let (pi_, po) = (self.phi_grid[ijk[2]], self.phi_grid[ijk[2] + 1]);
        (1.0 / 3.0) * (ro * ro * ro - ri * ri * ri) * (ti.cos() - to.cos()) * (po - pi_)
    }
}

/// `FP_PRECISION` (`include/openmc/constants.h`) — the on-axis guard above.
const FP_PRECISION: f64 = 1.0e-14;

/// **Enum dispatch over every structured mesh type** — the form a
/// [`super::filter::MeshFilter`] holds so one filter serves all four.
///
/// Enum rather than a trait object, per the workspace Rust design rule
/// (`docs/claude-md/rust-design-rules.md`: dispatch with enums, no `Box<dyn>`).
/// Upstream uses virtual dispatch off a `Mesh` base class; the enum is the
/// faithful equivalent here and costs no indirection.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshKind {
    Regular(RegularMesh),
    Rectilinear(RectilinearMesh),
    Cylindrical(CylindricalMesh),
    Spherical(SphericalMesh),
}

impl MeshKind {
    /// Total bins.
    pub fn n_bins(&self) -> usize {
        match self {
            Self::Regular(m) => m.n_bins(),
            Self::Rectilinear(m) => m.n_bins(),
            Self::Cylindrical(m) => m.n_bins(),
            Self::Spherical(m) => m.n_bins(),
        }
    }

    /// Flat bin index containing `p`, or `None` if `p` is outside the mesh.
    pub fn bin(&self, p: Position) -> Option<usize> {
        match self {
            Self::Regular(m) => m.get_bin(p),
            Self::Rectilinear(m) => m.bin(p),
            Self::Cylindrical(m) => m.bin(p),
            Self::Spherical(m) => m.bin(p),
        }
    }

    /// Volume of flat bin `bin` \[cm^3\], or `None` if out of range.
    ///
    /// This is what a **flux normalisation** needs: a track-length tally scores
    /// `cm` and dividing by the bin volume is what turns it into a flux. It is
    /// non-trivial for the curvilinear cases, which is why it is carried here
    /// rather than left to the caller to work out per mesh type.
    pub fn bin_volume(&self, bin: usize) -> Option<f64> {
        if bin >= self.n_bins() {
            return None;
        }
        Some(match self {
            Self::Regular(m) => {
                let w = m.width();
                w[0] * w[1] * w[2]
            }
            Self::Rectilinear(m) => m.volume(unflatten(bin, m.dimension())),
            Self::Cylindrical(m) => m.volume(unflatten(bin, m.dimension())),
            Self::Spherical(m) => m.volume(unflatten(bin, m.dimension())),
        })
    }
}

/// Flat bin index back to `(i, j, k)`, first axis fastest — the inverse of
/// `i + d0 (j + d1 k)`, which every mesh here uses.
#[inline]
fn unflatten(bin: usize, d: [usize; 3]) -> [usize; 3] {
    let i = bin % d[0];
    let j = (bin / d[0]) % d[1];
    let k = bin / (d[0] * d[1]);
    [i, j, k]
}

// ═══════════════════════════════════════════════════════════════════════════
// Mesh surface crossings (GitHub #261, `MESH_SURFACE`)
// ═══════════════════════════════════════════════════════════════════════════

/// Surface bins per mesh element: `4 * n_dimension` — for each of the three
/// axes, the min and max face, each with an outward and an inward current.
///
/// `StructuredMesh::n_surface_bins` (`src/mesh.cpp:1189`) at OpenMC `afa7a14`.
pub const SURFACE_BINS_PER_ELEMENT: usize = 12;

impl RegularMesh {
    /// Number of bins a [`crate::tally::filter_extra::MeshSurfaceFilter`] on
    /// this mesh produces.
    pub fn n_surface_bins(&self) -> usize {
        SURFACE_BINS_PER_ELEMENT * self.n_bins()
    }

    /// The surface bin for one face of one element —
    /// `SurfaceAggregator::surface` (`src/mesh.cpp:1296`):
    /// `4 * n_dim * element + 4 * k + (max ? 2 : 0) + (inward ? 1 : 0)`.
    pub fn surface_bin(&self, element: usize, axis: usize, max: bool, inward: bool) -> usize {
        SURFACE_BINS_PER_ELEMENT * element
            + 4 * axis
            + if max { 2 } else { 0 }
            + if inward { 1 } else { 0 }
    }

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
    pub fn surface_bins_crossed(&self, r0: Position, r1: Position) -> Vec<usize> {
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
            let at = |s: f64| {
                Position::new(a[0] + d[0] * s, a[1] + d[1] * s, a[2] + d[2] * s)
            };
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
