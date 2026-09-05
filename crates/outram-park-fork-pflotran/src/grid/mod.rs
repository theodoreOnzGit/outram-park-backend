//! Structured Cartesian finite-volume grid (v1) — bead op-v6s.5.
//!
//! A logically-rectangular `nx * ny * nz` Cartesian mesh with the geometry a
//! two-point flux approximation (TPFA) needs: cell volumes and centroids,
//! internal cell-to-cell [`Connection`]s (shared face area, centroid distance,
//! and the purely geometric transmissibility `area / distance`), and exterior
//! [`BoundaryFace`]s for applying Dirichlet/Neumann boundary conditions.
//!
//! # Cell ordering (x-fastest)
//!
//! Cells are numbered with the x index varying fastest, then y, then z:
//!
//! ```text
//! index = i + nx * (j + ny * k)
//! ```
//!
//! [`CartesianGrid::cell_index`] and [`CartesianGrid::cell_ijk`] convert between
//! the linear index and the `(i, j, k)` logical triple in both directions.
//!
//! # Connection ordering (matches the LDU addressing)
//!
//! [`CartesianGrid::connections`] and [`CartesianGrid::ldu_addressing`] share one
//! canonical face order: cells are visited in linear order, and for each cell its
//! `+x`, `+y`, then `+z` internal faces are emitted (when the neighbour exists).
//! Face `f` in the LDU addressing therefore corresponds exactly to
//! `connections()[f]`, so a caller can assemble an
//! `outram_foam_basic_lib` `LduMatrix` and index its off-diagonal coefficients by
//! the same `f`.
//!
//! Every internal connection has `owner < neighbour`: the `+x`/`+y`/`+z`
//! neighbour always has the larger linear index, so the lower-index cell is the
//! owner by construction.
//!
//! # Units
//!
//! All lengths are metres (m), all areas m^2, all volumes m^3. The geometric
//! transmissibility has units of metres (m) — it is `area / distance`, and must
//! be multiplied by a mobility (`k_r / mu`) and an intrinsic permeability (m^2)
//! by the physics layer to obtain a true transmissibility.
//!
//! # Scope
//!
//! Structured Cartesian only; unstructured grids are deferred to a later bead.
//! Grid spacings are stored per axis, so both uniform
//! ([`CartesianGrid::uniform`]) and rectilinear
//! ([`CartesianGrid::rectilinear`]) meshes are supported by the same type.

use crate::error::PflotranError;
use crate::units::GeoLength;
use uom::si::length::meter;

/// Cartesian axis. Selects which permeability-tensor component (and which
/// direction's spacing) a connection or boundary face is aligned with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// The x axis (index `i`, varies fastest in the cell ordering).
    X,
    /// The y axis (index `j`).
    Y,
    /// The z axis (index `k`, varies slowest).
    Z,
}

/// One of the six faces of the logical box, naming where a boundary condition is
/// applied. `Min`/`Max` refer to the low and high ends of each axis
/// (`XMin` is the `i == 0` face, `XMax` the `i == nx - 1` face, and so on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundaryLocation {
    /// The `i == 0` exterior face (low-x end).
    XMin,
    /// The `i == nx - 1` exterior face (high-x end).
    XMax,
    /// The `j == 0` exterior face (low-y end).
    YMin,
    /// The `j == ny - 1` exterior face (high-y end).
    YMax,
    /// The `k == 0` exterior face (low-z end).
    ZMin,
    /// The `k == nz - 1` exterior face (high-z end).
    ZMax,
}

/// An internal connection between two adjacent cells (`owner < neighbour`),
/// carrying the geometry a two-point flux approximation needs.
///
/// The face is shared by exactly the two cells; `area` is its area (m^2),
/// `distance` is the straight-line owner-centroid to neighbour-centroid distance
/// (m), and `geometric_transmissibility` is `area / distance` (m). Multiply the
/// latter by phase mobility (`k_r / mu`) and intrinsic permeability (m^2) in the
/// physics layer to obtain a flux coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Connection {
    /// Linear index of the owner cell (the one with the smaller linear index).
    pub owner: usize,
    /// Linear index of the neighbour cell (the larger linear index).
    pub neighbour: usize,
    /// Shared face area, m^2. Product of the two cell widths on the axes
    /// orthogonal to [`Connection::axis`].
    pub area: f64,
    /// Owner-centroid to neighbour-centroid distance, m. Equals half the owner's
    /// width plus half the neighbour's width along [`Connection::axis`].
    pub distance: f64,
    /// Purely geometric transmissibility, `area / distance`, in metres (m).
    /// Mobility- and permeability-free; scale it in the physics layer.
    pub geometric_transmissibility: f64,
    /// Axis the connection is normal to (the flux direction).
    pub axis: Axis,
}

/// A boundary face of a single cell, for Dirichlet or Neumann boundary
/// conditions on the domain exterior.
///
/// `distance` is the cell-centroid to boundary-face distance (m) — half the
/// cell's width along [`BoundaryFace::axis`] — which a Dirichlet condition uses
/// as the one-sided gradient length.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundaryFace {
    /// Linear index of the cell owning this exterior face.
    pub cell: usize,
    /// Which of the six box faces this is.
    pub location: BoundaryLocation,
    /// Face area, m^2. Product of the two cell widths on the axes orthogonal to
    /// [`BoundaryFace::axis`].
    pub area: f64,
    /// Cell-centroid to boundary-face distance, m — half the cell width along
    /// [`BoundaryFace::axis`].
    pub distance: f64,
    /// Axis the boundary face is normal to.
    pub axis: Axis,
}

/// A logically-rectangular `nx * ny * nz` Cartesian finite-volume grid.
///
/// Cell ordering is x-fastest: `index = i + nx * (j + ny * k)`. Per-axis
/// spacings (metres) are stored, so a single instance can be uniform or
/// rectilinear. Cell centroids, internal [`Connection`]s, and exterior
/// [`BoundaryFace`]s are precomputed at construction and returned by reference.
#[derive(Debug, Clone)]
pub struct CartesianGrid {
    nx: usize,
    ny: usize,
    nz: usize,
    /// Per-column widths along x (len `nx`), metres.
    dx: Vec<f64>,
    /// Per-row widths along y (len `ny`), metres.
    dy: Vec<f64>,
    /// Per-layer widths along z (len `nz`), metres.
    dz: Vec<f64>,
    /// Cell-centre x coordinates per column (len `nx`), metres.
    x_centers: Vec<f64>,
    /// Cell-centre y coordinates per row (len `ny`), metres.
    y_centers: Vec<f64>,
    /// Cell-centre z coordinates per layer (len `nz`), metres.
    z_centers: Vec<f64>,
    connections: Vec<Connection>,
    boundary_faces: Vec<BoundaryFace>,
}

impl CartesianGrid {
    /// Build a uniform grid from per-axis cell counts and uniform spacings
    /// (metres).
    ///
    /// - `nx`, `ny`, `nz`: number of cells along each axis; each must be `>= 1`.
    /// - `dx`, `dy`, `dz`: uniform cell width on each axis; each must be a
    ///   strictly positive length.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any cell count is zero or any
    /// spacing is non-positive (`<= 0`) or non-finite.
    pub fn uniform(
        nx: usize,
        ny: usize,
        nz: usize,
        dx: GeoLength,
        dy: GeoLength,
        dz: GeoLength,
    ) -> Result<Self, PflotranError> {
        let dxm = dx.get::<meter>();
        let dym = dy.get::<meter>();
        let dzm = dz.get::<meter>();
        Self::rectilinear(vec![dxm; nx], vec![dym; ny], vec![dzm; nz])
    }

    /// Build a rectilinear grid from per-axis spacing arrays (metres).
    ///
    /// The length of each vector is the number of cells on that axis, so
    /// `dx.len() == nx`, `dy.len() == ny`, `dz.len() == nz`. Entry `dx[i]` is the
    /// width (m) of column `i`, and likewise for `dy`/`dz`.
    ///
    /// # Errors
    ///
    /// Returns [`PflotranError::InvalidInput`] if any array is empty (a zero
    /// dimension) or any spacing entry is non-positive (`<= 0`) or non-finite.
    pub fn rectilinear(dx: Vec<f64>, dy: Vec<f64>, dz: Vec<f64>) -> Result<Self, PflotranError> {
        validate_axis(&dx, "x")?;
        validate_axis(&dy, "y")?;
        validate_axis(&dz, "z")?;

        let nx = dx.len();
        let ny = dy.len();
        let nz = dz.len();

        let x_centers = centers(&dx);
        let y_centers = centers(&dy);
        let z_centers = centers(&dz);

        let mut grid = CartesianGrid {
            nx,
            ny,
            nz,
            dx,
            dy,
            dz,
            x_centers,
            y_centers,
            z_centers,
            connections: Vec::new(),
            boundary_faces: Vec::new(),
        };
        grid.connections = grid.build_connections();
        grid.boundary_faces = grid.build_boundary_faces();
        Ok(grid)
    }

    /// Cell counts per axis as `(nx, ny, nz)`.
    pub fn dims(&self) -> (usize, usize, usize) {
        (self.nx, self.ny, self.nz)
    }

    /// Total number of cells, `nx * ny * nz`.
    pub fn n_cells(&self) -> usize {
        self.nx * self.ny * self.nz
    }

    /// Linear index of cell `(i, j, k)` using x-fastest ordering
    /// (`i + nx * (j + ny * k)`).
    ///
    /// # Panics
    ///
    /// Panics in debug builds (via `debug_assert!`) if any index is out of range
    /// for its axis.
    pub fn cell_index(&self, i: usize, j: usize, k: usize) -> usize {
        debug_assert!(
            i < self.nx && j < self.ny && k < self.nz,
            "cell index out of range"
        );
        i + self.nx * (j + self.ny * k)
    }

    /// Logical `(i, j, k)` triple of a linear cell index (inverse of
    /// [`CartesianGrid::cell_index`]).
    ///
    /// # Panics
    ///
    /// Panics in debug builds if `cell >= n_cells()`.
    pub fn cell_ijk(&self, cell: usize) -> (usize, usize, usize) {
        debug_assert!(cell < self.n_cells(), "cell index out of range");
        let i = cell % self.nx;
        let rem = cell / self.nx;
        let j = rem % self.ny;
        let k = rem / self.ny;
        (i, j, k)
    }

    /// Volume of a cell, m^3 — `dx[i] * dy[j] * dz[k]`.
    pub fn cell_volume(&self, cell: usize) -> f64 {
        let (i, j, k) = self.cell_ijk(cell);
        self.dx[i] * self.dy[j] * self.dz[k]
    }

    /// Centroid of a cell as `[x, y, z]` in metres.
    pub fn cell_center(&self, cell: usize) -> [f64; 3] {
        let (i, j, k) = self.cell_ijk(cell);
        [self.x_centers[i], self.y_centers[j], self.z_centers[k]]
    }

    /// All internal cell-to-cell connections, in the canonical face order (see
    /// the module docs). Face `f` here matches face `f` of
    /// [`CartesianGrid::ldu_addressing`].
    pub fn connections(&self) -> &[Connection] {
        &self.connections
    }

    /// All exterior boundary faces, one per cell face lying on the domain
    /// boundary.
    pub fn boundary_faces(&self) -> &[BoundaryFace] {
        &self.boundary_faces
    }

    /// The `(owner, neighbour)` index arrays for constructing an
    /// `outram_foam_basic_lib` `LduMatrix`, in the SAME order as
    /// [`CartesianGrid::connections`] — so LDU face `f` corresponds to
    /// `connections()[f]`. Both returned vectors have length
    /// `connections().len()`, and `owner[f] < neighbour[f]` for every `f`.
    pub fn ldu_addressing(&self) -> (Vec<usize>, Vec<usize>) {
        let owners = self.connections.iter().map(|c| c.owner).collect();
        let neighbours = self.connections.iter().map(|c| c.neighbour).collect();
        (owners, neighbours)
    }

    /// Build the internal connections in canonical face order: cells in linear
    /// order, each contributing its `+x`, `+y`, then `+z` face when it exists.
    fn build_connections(&self) -> Vec<Connection> {
        let mut conns = Vec::new();
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let owner = self.cell_index(i, j, k);
                    // +x face
                    if i + 1 < self.nx {
                        let neighbour = self.cell_index(i + 1, j, k);
                        let area = self.dy[j] * self.dz[k];
                        let distance = 0.5 * self.dx[i] + 0.5 * self.dx[i + 1];
                        conns.push(Connection {
                            owner,
                            neighbour,
                            area,
                            distance,
                            geometric_transmissibility: area / distance,
                            axis: Axis::X,
                        });
                    }
                    // +y face
                    if j + 1 < self.ny {
                        let neighbour = self.cell_index(i, j + 1, k);
                        let area = self.dx[i] * self.dz[k];
                        let distance = 0.5 * self.dy[j] + 0.5 * self.dy[j + 1];
                        conns.push(Connection {
                            owner,
                            neighbour,
                            area,
                            distance,
                            geometric_transmissibility: area / distance,
                            axis: Axis::Y,
                        });
                    }
                    // +z face
                    if k + 1 < self.nz {
                        let neighbour = self.cell_index(i, j, k + 1);
                        let area = self.dx[i] * self.dy[j];
                        let distance = 0.5 * self.dz[k] + 0.5 * self.dz[k + 1];
                        conns.push(Connection {
                            owner,
                            neighbour,
                            area,
                            distance,
                            geometric_transmissibility: area / distance,
                            axis: Axis::Z,
                        });
                    }
                }
            }
        }
        conns
    }

    /// Build the exterior boundary faces: cells in linear order, each
    /// contributing faces `XMin, XMax, YMin, YMax, ZMin, ZMax` when it lies on
    /// the corresponding boundary. A single-cell-thick axis yields both the min
    /// and max face for that cell.
    fn build_boundary_faces(&self) -> Vec<BoundaryFace> {
        let mut faces = Vec::new();
        for k in 0..self.nz {
            for j in 0..self.ny {
                for i in 0..self.nx {
                    let cell = self.cell_index(i, j, k);
                    let yz = self.dy[j] * self.dz[k];
                    let xz = self.dx[i] * self.dz[k];
                    let xy = self.dx[i] * self.dy[j];
                    if i == 0 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::XMin,
                            area: yz,
                            distance: 0.5 * self.dx[i],
                            axis: Axis::X,
                        });
                    }
                    if i == self.nx - 1 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::XMax,
                            area: yz,
                            distance: 0.5 * self.dx[i],
                            axis: Axis::X,
                        });
                    }
                    if j == 0 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::YMin,
                            area: xz,
                            distance: 0.5 * self.dy[j],
                            axis: Axis::Y,
                        });
                    }
                    if j == self.ny - 1 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::YMax,
                            area: xz,
                            distance: 0.5 * self.dy[j],
                            axis: Axis::Y,
                        });
                    }
                    if k == 0 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::ZMin,
                            area: xy,
                            distance: 0.5 * self.dz[k],
                            axis: Axis::Z,
                        });
                    }
                    if k == self.nz - 1 {
                        faces.push(BoundaryFace {
                            cell,
                            location: BoundaryLocation::ZMax,
                            area: xy,
                            distance: 0.5 * self.dz[k],
                            axis: Axis::Z,
                        });
                    }
                }
            }
        }
        faces
    }
}

/// Validate a per-axis spacing array: non-empty, every entry strictly positive
/// and finite.
fn validate_axis(widths: &[f64], name: &str) -> Result<(), PflotranError> {
    if widths.is_empty() {
        return Err(PflotranError::InvalidInput(format!(
            "grid {name} axis has zero cells (spacing array is empty)"
        )));
    }
    for (idx, &w) in widths.iter().enumerate() {
        if !w.is_finite() || w <= 0.0 {
            return Err(PflotranError::InvalidInput(format!(
                "grid {name} axis spacing[{idx}] = {w} is not a positive finite length"
            )));
        }
    }
    Ok(())
}

/// Cumulative cell-centre coordinates from a per-cell width array: the centre of
/// cell `n` is the sum of all preceding widths plus half of its own width.
fn centers(widths: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(widths.len());
    let mut edge = 0.0_f64;
    for &w in widths {
        out.push(edge + 0.5 * w);
        edge += w;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Length;

    fn m(x: f64) -> GeoLength {
        Length::new::<meter>(x)
    }

    #[test]
    fn two_by_one_by_one_has_one_connection_with_correct_transmissibility() {
        // dx=2, dy=3, dz=4. One x-normal connection.
        let g = CartesianGrid::uniform(2, 1, 1, m(2.0), m(3.0), m(4.0)).unwrap();
        assert_eq!(g.n_cells(), 2);
        let conns = g.connections();
        assert_eq!(conns.len(), 1);
        let c = conns[0];
        assert_eq!(c.owner, 0);
        assert_eq!(c.neighbour, 1);
        assert_eq!(c.axis, Axis::X);
        assert!((c.area - 12.0).abs() < 1e-12); // dy*dz = 3*4
        assert!((c.distance - 2.0).abs() < 1e-12); // dx
        assert!((c.geometric_transmissibility - 6.0).abs() < 1e-12); // 12/2
    }

    #[test]
    fn three_cubed_uniform_counts() {
        let g = CartesianGrid::uniform(3, 3, 3, m(1.0), m(1.0), m(1.0)).unwrap();
        assert_eq!(g.dims(), (3, 3, 3));
        assert_eq!(g.n_cells(), 27);
        // 3 * (2*3*3) = 54 internal connections (18 per axis direction).
        assert_eq!(g.connections().len(), 54);
        let per_axis = |ax: Axis| g.connections().iter().filter(|c| c.axis == ax).count();
        assert_eq!(per_axis(Axis::X), 18);
        assert_eq!(per_axis(Axis::Y), 18);
        assert_eq!(per_axis(Axis::Z), 18);
    }

    #[test]
    fn index_roundtrip_is_x_fastest() {
        let g = CartesianGrid::uniform(3, 4, 5, m(1.0), m(1.0), m(1.0)).unwrap();
        // x-fastest: index = i + nx*(j + ny*k)
        assert_eq!(g.cell_index(1, 0, 0), 1);
        assert_eq!(g.cell_index(0, 1, 0), 3);
        assert_eq!(g.cell_index(0, 0, 1), 12);
        for cell in 0..g.n_cells() {
            let (i, j, k) = g.cell_ijk(cell);
            assert_eq!(g.cell_index(i, j, k), cell);
        }
    }

    #[test]
    fn volume_and_center_correctness() {
        // Rectilinear so widths differ, exercising cumulative centres.
        let g = CartesianGrid::rectilinear(vec![2.0, 4.0], vec![1.0, 3.0], vec![5.0]).unwrap();
        // cell (0,0,0): vol = 2*1*5 = 10, centre = (1, 0.5, 2.5)
        let c0 = g.cell_index(0, 0, 0);
        assert!((g.cell_volume(c0) - 10.0).abs() < 1e-12);
        assert_eq!(g.cell_center(c0), [1.0, 0.5, 2.5]);
        // cell (1,1,0): vol = 4*3*5 = 60, centre x = 2 + 2 = 4, y = 1 + 1.5 = 2.5, z = 2.5
        let c3 = g.cell_index(1, 1, 0);
        assert!((g.cell_volume(c3) - 60.0).abs() < 1e-12);
        assert_eq!(g.cell_center(c3), [4.0, 2.5, 2.5]);
    }

    #[test]
    fn rectilinear_connection_distance_is_half_plus_half() {
        // Two cells along x: widths 2 and 4 → centroid distance = 1 + 2 = 3.
        let g = CartesianGrid::rectilinear(vec![2.0, 4.0], vec![1.0], vec![1.0]).unwrap();
        let c = g.connections()[0];
        assert!((c.distance - 3.0).abs() < 1e-12);
        assert!((c.area - 1.0).abs() < 1e-12); // dy*dz = 1*1
        assert!((c.geometric_transmissibility - (1.0 / 3.0)).abs() < 1e-12);
    }

    #[test]
    fn two_cubed_has_24_boundary_faces() {
        let g = CartesianGrid::uniform(2, 2, 2, m(1.0), m(1.0), m(1.0)).unwrap();
        // Exterior faces of a 2x2x2 box: 2*(nx*ny + ny*nz + nx*nz) = 2*(4+4+4)=24.
        assert_eq!(g.boundary_faces().len(), 24);
        // Every cell in a 2x2x2 sits on 3 boundaries → 8*3 = 24.
        for f in g.boundary_faces() {
            assert!((f.distance - 0.5).abs() < 1e-12);
            assert!((f.area - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn single_cell_gets_both_min_and_max_on_each_axis() {
        let g = CartesianGrid::uniform(1, 1, 1, m(1.0), m(1.0), m(1.0)).unwrap();
        assert_eq!(g.connections().len(), 0);
        // One cell, six faces.
        assert_eq!(g.boundary_faces().len(), 6);
        let locs: Vec<_> = g.boundary_faces().iter().map(|f| f.location).collect();
        for want in [
            BoundaryLocation::XMin,
            BoundaryLocation::XMax,
            BoundaryLocation::YMin,
            BoundaryLocation::YMax,
            BoundaryLocation::ZMin,
            BoundaryLocation::ZMax,
        ] {
            assert!(locs.contains(&want));
        }
    }

    #[test]
    fn ldu_addressing_matches_connections_and_owner_lt_neighbour() {
        let g = CartesianGrid::uniform(3, 3, 3, m(1.0), m(1.0), m(1.0)).unwrap();
        let (owners, neighbours) = g.ldu_addressing();
        assert_eq!(owners.len(), g.connections().len());
        assert_eq!(neighbours.len(), g.connections().len());
        for (f, c) in g.connections().iter().enumerate() {
            assert_eq!(owners[f], c.owner);
            assert_eq!(neighbours[f], c.neighbour);
            assert!(owners[f] < neighbours[f], "owner must be < neighbour");
        }
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert!(CartesianGrid::uniform(0, 1, 1, m(1.0), m(1.0), m(1.0)).is_err());
        assert!(CartesianGrid::uniform(1, 1, 1, m(0.0), m(1.0), m(1.0)).is_err());
        assert!(CartesianGrid::uniform(1, 1, 1, m(-1.0), m(1.0), m(1.0)).is_err());
        assert!(CartesianGrid::rectilinear(vec![], vec![1.0], vec![1.0]).is_err());
        assert!(CartesianGrid::rectilinear(vec![1.0, -2.0], vec![1.0], vec![1.0]).is_err());
    }
}
