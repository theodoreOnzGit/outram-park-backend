//! Unstructured polyhedral finite-volume grid with explicit connectivity
//! (bead op-v6s.15.8).
//!
//! The [`crate::grid`] module provides a structured Cartesian mesh only. This
//! module is its unstructured counterpart: a grid of **arbitrary polyhedral
//! cells** whose topology is given explicitly as a list of faces, rather than
//! implied by a logical `(i, j, k)` indexing. It mirrors PFLOTRAN's real
//! implicit-unstructured (`UGRID`) discretization, where each cell carries a
//! centroid and volume and each face records the two cells (or one cell + the
//! exterior) it separates plus the geometry a flux stencil needs.
//!
//! # What this layer provides — and what it does not
//!
//! This is the **connectivity + geometry** layer only, plus the classic
//! **two-point flux approximation (TPFA)** geometric transmissibility. It does
//! **not** provide:
//!
//! - mesh-file readers (Exodus/HDF5/PFLOTRAN `.ugi`/`.h5`) — cells and faces
//!   must be supplied explicitly to [`UnstructuredGrid::from_faces`];
//! - multi-point flux (MPFA) or any consistent scheme for non-K-orthogonal
//!   meshes — TPFA only (see the caveat below);
//! - the physics (mobility, permeability, boundary conditions) — the stored
//!   transmissibility is a *purely geometric* factor.
//!
//! # Two-point flux approximation (TPFA) transmissibility
//!
//! For an internal face of area `A` shared by cells `a` and `b` with centroids
//! `c_a`, `c_b`, unit face normal `n`, this module stores the geometric
//! transmissibility
//!
//! ```text
//!   T_geom = A * |n · (c_b - c_a)| / |c_b - c_a|^2       [m]
//! ```
//!
//! (`transmissibility_geom` on [`UnstructuredConnection`]). This is the standard
//! TPFA form: it projects the cell-to-cell vector onto the face normal, so it
//! reduces exactly to the familiar `A / d` (with `d = |c_b - c_a|`) when the
//! connection is **K-orthogonal** — i.e. the line joining the two centroids is
//! parallel to the face normal, as on a Cartesian or a Voronoi (PEBI) mesh. The
//! absolute value makes the result independent of whether `n` was supplied
//! pointing from `a` to `b` or the reverse; a well-posed TPFA transmissibility
//! is always positive. Multiply `T_geom` by phase mobility (`k_r / mu`) and
//! intrinsic permeability (m^2) in the physics layer to obtain a true flux
//! coefficient.
//!
//! ## K-orthogonality caveat (a real limitation, not a bug)
//!
//! TPFA is **only consistent** (convergent to the true solution) when the mesh
//! is K-orthogonal with respect to the permeability tensor. On a distorted mesh,
//! or with a full/anisotropic permeability tensor whose principal axes are not
//! aligned with the cell-connection directions, TPFA introduces an `O(1)`
//! discretization error that does **not** vanish under refinement — the scheme
//! is inconsistent there. Handling those cases needs a multi-point flux (MPFA)
//! or mimetic/mixed scheme, which is out of scope for this module. See
//! Aziz & Settari, *Petroleum Reservoir Simulation* (1979); Eymard, Gallouët &
//! Herbin, *Finite Volume Methods* (Handbook of Numerical Analysis, 2000);
//! and LeVeque, *Finite Volume Methods for Hyperbolic Problems* (2002).
//!
//! # Units
//!
//! Lengths are metres (m), areas m^2, volumes m^3. `transmissibility_geom` has
//! units of metres (m), being `area / distance`.
//!
//! # Relationship to the structured grid
//!
//! Field names and accessors deliberately track [`crate::grid`]: an
//! [`UnstructuredConnection`] plays the role of [`crate::grid::Connection`]
//! (two cell ids + area + geometric transmissibility), an
//! [`UnstructuredBoundaryFace`] the role of [`crate::grid::BoundaryFace`], and
//! [`UnstructuredGrid`] exposes `n_cells` / `cell_volume` / `connections` /
//! `boundary_faces` / `ldu_addressing` with the same meanings. This keeps the
//! two grids unify-able later, while remaining a separate self-contained type.
//! [`UnstructuredGrid::uniform_column`] builds a 1-D column that reproduces the
//! structured grid's geometry exactly, so an unstructured result can be
//! cross-checked cell-for-cell against the Cartesian one.
//!
//! # Provenance
//!
//! PFLOTRAN implicit-unstructured (`UGRID`) discretization + the standard TPFA
//! two-point flux stencil. **Untrusted AI-generated draft** per the workspace
//! `RESPONSIBLE_USE.md`: geometry/connectivity + TPFA only, no MPFA and no mesh
//! readers yet. No human V&V has been performed; not for facility operation,
//! reactor control, or licensing.

use crate::error::PflotranError;

/// An index into a grid's cell array.
///
/// Topology links are stored as plain `usize` cell indices (never Rust
/// references), so the grid needs no lifetime parameters. This newtype is the
/// typed spelling of such an index for public signatures that want to be
/// explicit that a `usize` names a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CellId(pub usize);

impl CellId {
    /// The underlying array index of this cell.
    pub fn index(self) -> usize {
        self.0
    }
}

/// A single internal connection between two unstructured cells, carrying the
/// geometry a two-point flux approximation (TPFA) needs.
///
/// The face is shared by exactly the two distinct cells `cell_a` and `cell_b`
/// (both array indices). `area` is the shared face area (m^2), and
/// `transmissibility_geom` is the purely geometric TPFA factor
/// `A * |n · (c_b - c_a)| / |c_b - c_a|^2` in metres (m) — mobility- and
/// permeability-free. See the module docs for the formula and its
/// K-orthogonality limitation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnstructuredConnection {
    /// Array index of the first cell of the connection.
    pub cell_a: usize,
    /// Array index of the second cell of the connection (distinct from
    /// `cell_a`).
    pub cell_b: usize,
    /// Shared face area, m^2.
    pub area: f64,
    /// Purely geometric TPFA transmissibility factor `A / d`, in metres (m):
    /// `area * |n · (c_b - c_a)| / |c_b - c_a|^2`. Multiply by mobility
    /// (`k_r / mu`) and intrinsic permeability (m^2) in the physics layer.
    pub transmissibility_geom: f64,
}

/// A boundary face of a single unstructured cell — a face touching the domain
/// exterior — for applying Dirichlet or Neumann boundary conditions.
///
/// It has exactly one owner cell (`cell`, an array index), an area (m^2), an
/// outward unit `normal`, and a face `centroid` (`[x, y, z]`, metres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnstructuredBoundaryFace {
    /// Array index of the single cell owning this exterior face.
    pub cell: usize,
    /// Face area, m^2.
    pub area: f64,
    /// Outward unit normal `[nx, ny, nz]` (normalised at construction).
    pub normal: [f64; 3],
    /// Face centroid `[x, y, z]`, metres.
    pub centroid: [f64; 3],
}

/// An unstructured polyhedral finite-volume grid with explicit connectivity.
///
/// Each cell has a centroid (`[x, y, z]`, metres) and a volume (m^3). Internal
/// [`UnstructuredConnection`]s and exterior [`UnstructuredBoundaryFace`]s are
/// precomputed at construction from an explicit face list and returned by
/// reference. Unlike [`crate::grid::CartesianGrid`], there is no logical
/// `(i, j, k)` structure — topology is whatever the supplied faces describe.
#[derive(Debug, Clone)]
pub struct UnstructuredGrid {
    /// Per-cell centroids `[x, y, z]` (len `n_cells`), metres.
    centroids: Vec<[f64; 3]>,
    /// Per-cell volumes (len `n_cells`), m^3.
    volumes: Vec<f64>,
    connections: Vec<UnstructuredConnection>,
    boundary_faces: Vec<UnstructuredBoundaryFace>,
}

impl UnstructuredGrid {
    /// Build an unstructured grid from explicit cell geometry and a face list.
    ///
    /// - `centroids[c]` / `volumes[c]`: the centroid (metres) and volume (m^3)
    ///   of cell `c`; the two vectors must be the same length (the cell count).
    /// - `faces`: one tuple per face,
    ///   `(cell_a, cell_b, area, centroid, normal)`, where `cell_b` is
    ///   `Some(index)` for an **internal** face separating two cells and `None`
    ///   for a **boundary** face touching the exterior. `area` is the face area
    ///   (m^2), `centroid` its centroid (`[x, y, z]`, metres) and `normal` its
    ///   face normal (`[x, y, z]`; normalised internally, and only its direction
    ///   is used).
    ///
    /// For each internal face the geometric TPFA transmissibility
    /// `area * |n · (c_b - c_a)| / |c_b - c_a|^2` is computed and stored (see the
    /// module docs). Boundary faces store the owner cell, area, unit normal and
    /// centroid.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] (never panics) if:
    /// - `centroids.len() != volumes.len()`;
    /// - any centroid coordinate is non-finite, or any volume is non-positive
    ///   (`<= 0`) or non-finite;
    /// - a face references a cell index `>= n_cells`;
    /// - an internal face has `cell_a == cell_b` (a connection must join two
    ///   distinct cells);
    /// - a face area is non-positive or non-finite, a face centroid coordinate
    ///   is non-finite, the face normal is non-finite or has (near) zero length,
    ///   or an internal face's two cell centroids coincide (zero centroid
    ///   distance).
    pub fn from_faces(
        centroids: Vec<[f64; 3]>,
        volumes: Vec<f64>,
        faces: Vec<(usize, Option<usize>, f64, [f64; 3], [f64; 3])>,
    ) -> Result<Self, PflotranError> {
        if centroids.len() != volumes.len() {
            return Err(PflotranError::InvalidInput(format!(
                "unstructured grid: centroids.len() = {} != volumes.len() = {}",
                centroids.len(),
                volumes.len()
            )));
        }
        let n_cells = centroids.len();
        if n_cells == 0 {
            return Err(PflotranError::InvalidInput(
                "unstructured grid: needs at least one cell (empty centroids/volumes)".to_string(),
            ));
        }

        for (c, ctr) in centroids.iter().enumerate() {
            if ctr.iter().any(|x| !x.is_finite()) {
                return Err(PflotranError::InvalidInput(format!(
                    "unstructured grid: cell {c} centroid {ctr:?} has a non-finite coordinate"
                )));
            }
        }
        for (c, &v) in volumes.iter().enumerate() {
            if !v.is_finite() || v <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "unstructured grid: cell {c} volume {v} is not a positive finite value"
                )));
            }
        }

        let mut connections = Vec::new();
        let mut boundary_faces = Vec::new();

        for (f, &(cell_a, cell_b_opt, area, centroid, normal)) in faces.iter().enumerate() {
            if cell_a >= n_cells {
                return Err(PflotranError::InvalidInput(format!(
                    "unstructured grid: face {f} references cell_a = {cell_a} >= n_cells = {n_cells}"
                )));
            }
            if !area.is_finite() || area <= 0.0 {
                return Err(PflotranError::InvalidInput(format!(
                    "unstructured grid: face {f} area {area} is not a positive finite value"
                )));
            }
            if centroid.iter().any(|x| !x.is_finite()) {
                return Err(PflotranError::InvalidInput(format!(
                    "unstructured grid: face {f} centroid {centroid:?} has a non-finite coordinate"
                )));
            }
            let nhat = normalise(normal).ok_or_else(|| {
                PflotranError::InvalidInput(format!(
                    "unstructured grid: face {f} normal {normal:?} is non-finite or zero-length"
                ))
            })?;

            match cell_b_opt {
                None => {
                    boundary_faces.push(UnstructuredBoundaryFace {
                        cell: cell_a,
                        area,
                        normal: nhat,
                        centroid,
                    });
                }
                Some(cell_b) => {
                    if cell_b >= n_cells {
                        return Err(PflotranError::InvalidInput(format!(
                            "unstructured grid: face {f} references cell_b = {cell_b} >= n_cells = {n_cells}"
                        )));
                    }
                    if cell_a == cell_b {
                        return Err(PflotranError::InvalidInput(format!(
                            "unstructured grid: internal face {f} joins cell {cell_a} to itself"
                        )));
                    }
                    let ca = centroids[cell_a];
                    let cb = centroids[cell_b];
                    let d = sub(cb, ca);
                    let dist2 = dot(d, d);
                    if !dist2.is_finite() || dist2 <= 0.0 {
                        return Err(PflotranError::InvalidInput(format!(
                            "unstructured grid: internal face {f} has coincident cell centroids \
                             (cells {cell_a}, {cell_b}) — zero centroid distance"
                        )));
                    }
                    // Standard TPFA geometric factor: A * |n · (c_b - c_a)| / |c_b - c_a|^2.
                    // Reduces to A / d for K-orthogonal connections. The abs() makes it
                    // independent of the supplied normal's orientation.
                    let transmissibility_geom = area * dot(nhat, d).abs() / dist2;
                    connections.push(UnstructuredConnection {
                        cell_a,
                        cell_b,
                        area,
                        transmissibility_geom,
                    });
                }
            }
        }

        Ok(UnstructuredGrid {
            centroids,
            volumes,
            connections,
            boundary_faces,
        })
    }

    /// Build an unstructured grid reproducing a uniform 1-D column of `n` cells
    /// spanning total length `length` (metres) with uniform cross-sectional
    /// `area` (m^2), oriented along the x axis.
    ///
    /// Cell `i` occupies `[i·dx, (i+1)·dx]` with `dx = length / n`, has volume
    /// `area · dx` and centroid `[(i + 0.5)·dx, 0, 0]`. There are `n - 1`
    /// internal faces (each of the given `area`, normal `[1, 0, 0]`, at
    /// `x = (i+1)·dx`) and exactly two boundary faces, at `x = 0` (normal
    /// `[-1, 0, 0]`) and `x = length` (normal `[1, 0, 0]`).
    ///
    /// This geometry matches a structured Cartesian column exactly — each
    /// internal `transmissibility_geom` equals `area / dx` — so an unstructured
    /// solve can be cross-checked cell-for-cell against
    /// [`crate::grid::CartesianGrid`].
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if `n == 0`, or if `length` or
    /// `area` is non-positive (`<= 0`) or non-finite.
    pub fn uniform_column(n: usize, length: f64, area: f64) -> Result<Self, PflotranError> {
        if n == 0 {
            return Err(PflotranError::InvalidInput(
                "uniform_column: n must be >= 1".to_string(),
            ));
        }
        if !length.is_finite() || length <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "uniform_column: length {length} is not a positive finite value"
            )));
        }
        if !area.is_finite() || area <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "uniform_column: area {area} is not a positive finite value"
            )));
        }

        let dx = length / n as f64;
        let volume = area * dx;

        let centroids: Vec<[f64; 3]> = (0..n).map(|i| [(i as f64 + 0.5) * dx, 0.0, 0.0]).collect();
        let volumes = vec![volume; n];

        let mut faces: Vec<(usize, Option<usize>, f64, [f64; 3], [f64; 3])> = Vec::new();
        // Internal faces between consecutive cells.
        for i in 0..n.saturating_sub(1) {
            let x = (i as f64 + 1.0) * dx;
            faces.push((i, Some(i + 1), area, [x, 0.0, 0.0], [1.0, 0.0, 0.0]));
        }
        // Two boundary faces: low-x (outward -x) and high-x (outward +x).
        faces.push((0, None, area, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]));
        faces.push((n - 1, None, area, [length, 0.0, 0.0], [1.0, 0.0, 0.0]));

        Self::from_faces(centroids, volumes, faces)
    }

    /// Total number of cells.
    pub fn n_cells(&self) -> usize {
        self.centroids.len()
    }

    /// Volume of a cell, m^3.
    ///
    /// # Panics
    ///
    /// Panics if `cell >= n_cells()`.
    pub fn cell_volume(&self, cell: usize) -> f64 {
        self.volumes[cell]
    }

    /// Centroid of a cell as `[x, y, z]` in metres.
    ///
    /// # Panics
    ///
    /// Panics if `cell >= n_cells()`.
    pub fn cell_centroid(&self, cell: usize) -> [f64; 3] {
        self.centroids[cell]
    }

    /// All internal cell-to-cell connections, in the order the internal faces
    /// were supplied to [`UnstructuredGrid::from_faces`]. Connection `f` here
    /// matches face `f` of [`UnstructuredGrid::ldu_addressing`].
    pub fn connections(&self) -> &[UnstructuredConnection] {
        &self.connections
    }

    /// All exterior boundary faces, in the order they were supplied to
    /// [`UnstructuredGrid::from_faces`].
    pub fn boundary_faces(&self) -> &[UnstructuredBoundaryFace] {
        &self.boundary_faces
    }

    /// The `(lower, upper)` index arrays for assembling an LDU / sparse matrix
    /// over the connections, in the SAME order as
    /// [`UnstructuredGrid::connections`] — so LDU face `f` corresponds to
    /// `connections()[f]`.
    ///
    /// Matching the [`crate::grid`] convention that `owner < neighbour`, each
    /// entry is normalised so that `lower[f] = min(cell_a, cell_b)` and
    /// `upper[f] = max(cell_a, cell_b)`; therefore `lower[f] < upper[f]` for
    /// every `f`. Both vectors have length `connections().len()`.
    pub fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
        let lower = self
            .connections
            .iter()
            .map(|c| c.cell_a.min(c.cell_b))
            .collect();
        let upper = self
            .connections
            .iter()
            .map(|c| c.cell_a.max(c.cell_b))
            .collect();
        (lower, upper)
    }
}

/// `a - b` component-wise.
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Dot product `a · b`.
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Return the unit vector along `v`, or `None` if `v` is non-finite or has
/// (near) zero length.
fn normalise(v: [f64; 3]) -> Option<[f64; 3]> {
    if v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    let len = dot(v, v).sqrt();
    if !len.is_finite() || len <= f64::EPSILON {
        return None;
    }
    Some([v[0] / len, v[1] / len, v[2] / len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_column_geometry_and_connectivity() {
        let g = UnstructuredGrid::uniform_column(10, 10.0, 1.0).unwrap();
        assert_eq!(g.n_cells(), 10);
        // 10 cells, dx = 1.0, each volume = area*dx = 1.0.
        for c in 0..10 {
            assert!((g.cell_volume(c) - 1.0).abs() < 1e-12);
        }
        // 9 internal connections, 2 boundary faces.
        assert_eq!(g.connections().len(), 9);
        assert_eq!(g.boundary_faces().len(), 2);
        // Each internal transmissibility_geom = area/dx = 1.0/1.0 = 1.0.
        for conn in g.connections() {
            assert!((conn.transmissibility_geom - 1.0).abs() < 1e-12);
            assert!((conn.area - 1.0).abs() < 1e-12);
        }
        // Centroids equally spaced at 0.5, 1.5, ... 9.5.
        for c in 0..10 {
            let expected = c as f64 + 0.5;
            assert!((g.cell_centroid(c)[0] - expected).abs() < 1e-12);
            assert_eq!(g.cell_centroid(c)[1], 0.0);
            assert_eq!(g.cell_centroid(c)[2], 0.0);
        }
    }

    #[test]
    fn uniform_column_boundary_faces_have_one_owner_each() {
        let g = UnstructuredGrid::uniform_column(4, 4.0, 2.0).unwrap();
        let bf = g.boundary_faces();
        assert_eq!(bf.len(), 2);
        // Low-x boundary owned by cell 0, outward normal -x.
        assert_eq!(bf[0].cell, 0);
        assert!((bf[0].normal[0] - (-1.0)).abs() < 1e-12);
        assert_eq!(bf[0].centroid, [0.0, 0.0, 0.0]);
        // High-x boundary owned by last cell, outward normal +x.
        assert_eq!(bf[1].cell, 3);
        assert!((bf[1].normal[0] - 1.0).abs() < 1e-12);
        assert!((bf[1].centroid[0] - 4.0).abs() < 1e-12);
        // Each boundary face names exactly one owner cell (< n_cells).
        for f in bf {
            assert!(f.cell < g.n_cells());
        }
    }

    #[test]
    fn from_faces_two_cell_mesh() {
        // Two unit cubes stacked along x: cell 0 at x=0.5, cell 1 at x=1.5.
        let centroids = vec![[0.5, 0.0, 0.0], [1.5, 0.0, 0.0]];
        let volumes = vec![1.0, 1.0];
        let faces = vec![
            // internal face between 0 and 1, area 1, at x=1, normal +x
            (0usize, Some(1usize), 1.0, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
            // two boundary faces
            (0usize, None, 1.0, [0.0, 0.0, 0.0], [-1.0, 0.0, 0.0]),
            (1usize, None, 1.0, [2.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        let g = UnstructuredGrid::from_faces(centroids, volumes, faces).unwrap();
        assert_eq!(g.n_cells(), 2);
        assert_eq!(g.connections().len(), 1);
        assert_eq!(g.boundary_faces().len(), 2);
        let c = g.connections()[0];
        // Internal connection joins two distinct cells.
        assert_ne!(c.cell_a, c.cell_b);
        assert_eq!(c.cell_a, 0);
        assert_eq!(c.cell_b, 1);
        assert!((c.area - 1.0).abs() < 1e-12);
        // distance = 1.0, A/d = 1.0.
        assert!((c.transmissibility_geom - 1.0).abs() < 1e-12);
    }

    #[test]
    fn ldu_addressing_lengths_and_ordering() {
        let g = UnstructuredGrid::uniform_column(5, 5.0, 1.0).unwrap();
        let (lower, upper) = g.ldu_addressing();
        assert_eq!(lower.len(), g.connections().len());
        assert_eq!(upper.len(), g.connections().len());
        for f in 0..lower.len() {
            assert!(lower[f] < upper[f], "lower must be < upper");
        }
    }

    #[test]
    fn ldu_addressing_normalises_reversed_connection() {
        // Supply an internal face with cell_a > cell_b; ldu must still order
        // lower < upper.
        let centroids = vec![[0.5, 0.0, 0.0], [1.5, 0.0, 0.0]];
        let volumes = vec![1.0, 1.0];
        let faces = vec![
            // note cell_a = 1, cell_b = 0 (reversed)
            (1usize, Some(0usize), 1.0, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ];
        let g = UnstructuredGrid::from_faces(centroids, volumes, faces).unwrap();
        let (lower, upper) = g.ldu_addressing();
        assert_eq!(lower, vec![0]);
        assert_eq!(upper, vec![1]);
    }

    #[test]
    fn transmissibility_scales_as_area_over_distance() {
        let base = single_internal_conn(1.0, 1.0);
        let double_area = single_internal_conn(2.0, 1.0);
        let double_spacing = single_internal_conn(1.0, 2.0);
        // Double area -> double factor.
        assert!((double_area - 2.0 * base).abs() < 1e-12);
        // Double spacing -> half factor.
        assert!((double_spacing - 0.5 * base).abs() < 1e-12);
    }

    /// Build a 2-cell mesh spaced `dist` apart along x with face `area`, return
    /// the single internal connection's geometric transmissibility.
    fn single_internal_conn(area: f64, dist: f64) -> f64 {
        let centroids = vec![[0.0, 0.0, 0.0], [dist, 0.0, 0.0]];
        let volumes = vec![area * dist, area * dist];
        let faces = vec![(
            0usize,
            Some(1usize),
            area,
            [dist / 2.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
        )];
        let g = UnstructuredGrid::from_faces(centroids, volumes, faces).unwrap();
        g.connections()[0].transmissibility_geom
    }

    #[test]
    fn tpfa_projection_reduces_to_area_over_distance_for_unnormalised_normal() {
        // Normal supplied non-unit and slightly skew but still along x -> the
        // projection normalises and recovers A/d for the K-orthogonal case.
        let centroids = vec![[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let volumes = vec![2.0, 2.0];
        let faces = vec![(
            0usize,
            Some(1usize),
            1.0,
            [1.0, 0.0, 0.0],
            [5.0, 0.0, 0.0], // non-unit +x normal
        )];
        let g = UnstructuredGrid::from_faces(centroids, volumes, faces).unwrap();
        // A/d = 1.0/2.0 = 0.5.
        assert!((g.connections()[0].transmissibility_geom - 0.5).abs() < 1e-12);
    }

    #[test]
    fn reversed_normal_gives_same_positive_transmissibility() {
        // The abs() in the TPFA factor makes orientation irrelevant.
        let fwd = {
            let c = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
            let v = vec![1.0, 1.0];
            let f = vec![(0usize, Some(1usize), 1.0, [0.5, 0.0, 0.0], [1.0, 0.0, 0.0])];
            UnstructuredGrid::from_faces(c, v, f).unwrap().connections()[0].transmissibility_geom
        };
        let rev = {
            let c = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
            let v = vec![1.0, 1.0];
            let f = vec![(0usize, Some(1usize), 1.0, [0.5, 0.0, 0.0], [-1.0, 0.0, 0.0])];
            UnstructuredGrid::from_faces(c, v, f).unwrap().connections()[0].transmissibility_geom
        };
        assert!(fwd > 0.0);
        assert!((fwd - rev).abs() < 1e-12);
    }

    #[test]
    fn volume_and_centroid_accessors_return_constructed_values() {
        let centroids = vec![[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
        let volumes = vec![7.0, 8.0];
        let faces = vec![(0usize, None, 1.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])];
        let g = UnstructuredGrid::from_faces(centroids, volumes, faces).unwrap();
        assert_eq!(g.cell_centroid(0), [1.0, 2.0, 3.0]);
        assert_eq!(g.cell_centroid(1), [4.0, 5.0, 6.0]);
        assert!((g.cell_volume(0) - 7.0).abs() < 1e-12);
        assert!((g.cell_volume(1) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn cell_id_index() {
        assert_eq!(CellId(7).index(), 7);
        assert_eq!(CellId(0), CellId(0));
    }

    #[test]
    fn invalid_mismatched_lengths() {
        let err = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            vec![1.0], // only one volume
            vec![],
        );
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn invalid_face_references_nonexistent_cell() {
        let err = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0]],
            vec![1.0],
            // cell_a = 5 does not exist
            vec![(5usize, None, 1.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])],
        );
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));

        let err2 = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0]],
            vec![1.0],
            // cell_b = 9 does not exist
            vec![(0usize, Some(9usize), 1.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])],
        );
        assert!(matches!(err2, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn invalid_negative_area_or_volume() {
        let neg_vol = UnstructuredGrid::from_faces(vec![[0.0, 0.0, 0.0]], vec![-1.0], vec![]);
        assert!(matches!(neg_vol, Err(PflotranError::InvalidInput(_))));

        let neg_area = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0]],
            vec![1.0],
            vec![(0usize, None, -3.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])],
        );
        assert!(matches!(neg_area, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn invalid_internal_face_self_connection() {
        let err = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
            vec![1.0, 1.0],
            // cell_a == cell_b -> not two distinct cells
            vec![(0usize, Some(0usize), 1.0, [0.5, 0.0, 0.0], [1.0, 0.0, 0.0])],
        );
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn invalid_zero_length_normal_and_coincident_centroids() {
        let zero_normal = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0]],
            vec![1.0],
            vec![(0usize, None, 1.0, [0.0, 0.0, 0.0], [0.0, 0.0, 0.0])],
        );
        assert!(matches!(zero_normal, Err(PflotranError::InvalidInput(_))));

        let coincident = UnstructuredGrid::from_faces(
            vec![[0.0, 0.0, 0.0], [0.0, 0.0, 0.0]],
            vec![1.0, 1.0],
            vec![(0usize, Some(1usize), 1.0, [0.0, 0.0, 0.0], [1.0, 0.0, 0.0])],
        );
        assert!(matches!(coincident, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn empty_grid_rejected() {
        let err = UnstructuredGrid::from_faces(vec![], vec![], vec![]);
        assert!(matches!(err, Err(PflotranError::InvalidInput(_))));
    }

    #[test]
    fn uniform_column_invalid_inputs() {
        assert!(UnstructuredGrid::uniform_column(0, 1.0, 1.0).is_err());
        assert!(UnstructuredGrid::uniform_column(3, 0.0, 1.0).is_err());
        assert!(UnstructuredGrid::uniform_column(3, -1.0, 1.0).is_err());
        assert!(UnstructuredGrid::uniform_column(3, 1.0, 0.0).is_err());
        assert!(UnstructuredGrid::uniform_column(3, 1.0, -2.0).is_err());
    }

    #[test]
    fn single_cell_column_has_no_internal_faces() {
        let g = UnstructuredGrid::uniform_column(1, 2.0, 3.0).unwrap();
        assert_eq!(g.n_cells(), 1);
        assert_eq!(g.connections().len(), 0);
        // Still two boundary faces, both owned by cell 0.
        assert_eq!(g.boundary_faces().len(), 2);
        for f in g.boundary_faces() {
            assert_eq!(f.cell, 0);
        }
        assert!((g.cell_volume(0) - 6.0).abs() < 1e-12); // area*length = 3*2
    }
}
