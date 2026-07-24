//! The core **volume mesh** data structure — the Rust analogue of cfMesh's
//! `polyMeshGen` and OpenFOAM's `polyMesh`.
//!
//! A [`VolumeMesh`] is a *finite-volume* mesh: a set of points, a set of
//! polygonal **faces** (each an ordered list of point indices), and, for every
//! face, the **owner** cell and an optional **neighbour** cell. A face with a
//! neighbour is *internal* (it separates two cells); a face without one is a
//! *boundary* face belonging to a [`BoundaryPatch`]. Cells are implicit — the
//! set of faces that reference a given cell index.
//!
//! This is exactly the connectivity OpenFOAM's `constant/polyMesh` stores
//! (`points` / `faces` / `owner` / `neighbour` / `boundary`), and exactly what
//! `outram-foam-basic-lib`'s `io::poly_mesh::PolyMesh` consumes — so this type
//! is the generator's output substrate, deliberately shaped to bridge to the
//! solver with no restructuring.
//!
//! # Conventions
//!
//! - A face's geometric normal (its area vector) points **from its owner toward
//!   its neighbour**; on a boundary face it points **out of the domain** (the
//!   owner is inside). Meshers in this crate build faces to satisfy this.
//! - Faces are ordered **internal first**, then boundary faces grouped by
//!   patch, so [`VolumeMesh::n_internal_faces`] is a prefix count — the OpenFOAM
//!   ordering rule.
//!
//! Index-based throughout (newtype-free `usize` indices into `Vec`s), no
//! lifetimes, no trait objects — per the workspace rules.

use crate::math::Vec3;

/// A named boundary patch: a contiguous run of boundary faces.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundaryPatch {
    /// Patch name (e.g. `"xMin"`, `"walls"`).
    pub name: String,
    /// Index of the first face of this patch in [`VolumeMesh::faces`].
    pub start_face: usize,
    /// Number of faces in this patch.
    pub n_faces: usize,
}

/// A finite-volume mesh: points, faces, per-face owner/neighbour, and boundary
/// patches. See the module docs for the conventions this type guarantees.
#[derive(Debug, Clone)]
pub struct VolumeMesh {
    /// Vertex positions, indexed by the entries of [`VolumeMesh::faces`].
    pub points: Vec<Vec3>,
    /// Faces, each an ordered ring of point indices (outward/owner→neighbour
    /// wound, see module docs).
    pub faces: Vec<Vec<usize>>,
    /// `owner[f]` — the cell that owns face `f`.
    pub owner: Vec<usize>,
    /// `neighbour[f]` — the cell across face `f`, or `None` for a boundary face.
    pub neighbour: Vec<Option<usize>>,
    /// Number of cells.
    pub n_cells: usize,
    /// Boundary patches (each a contiguous run of boundary faces).
    pub patches: Vec<BoundaryPatch>,
}

impl VolumeMesh {
    /// Number of points.
    pub fn point_count(&self) -> usize {
        self.points.len()
    }

    /// Number of faces (internal + boundary).
    pub fn face_count(&self) -> usize {
        self.faces.len()
    }

    /// Number of cells.
    pub fn cell_count(&self) -> usize {
        self.n_cells
    }

    /// Number of internal faces (those with a neighbour).
    pub fn n_internal_faces(&self) -> usize {
        self.neighbour.iter().filter(|n| n.is_some()).count()
    }

    /// Number of boundary faces (those without a neighbour).
    pub fn n_boundary_faces(&self) -> usize {
        self.face_count() - self.n_internal_faces()
    }

    /// The area vector of face `f` (Newell's method): magnitude = face area,
    /// direction = face normal (owner→neighbour, outward on a boundary).
    pub fn face_area_vector(&self, f: usize) -> Vec3 {
        let ring = &self.faces[f];
        let mut a = Vec3::ZERO;
        let k = ring.len();
        for i in 0..k {
            let p = self.points[ring[i]];
            let q = self.points[ring[(i + 1) % k]];
            a = a.add(p.cross(q));
        }
        a.scale(0.5)
    }

    /// The centroid (vertex average) of face `f`.
    pub fn face_centroid(&self, f: usize) -> Vec3 {
        let ring = &self.faces[f];
        let mut c = Vec3::ZERO;
        for &v in ring {
            c = c.add(self.points[v]);
        }
        c.scale(1.0 / ring.len() as f64)
    }

    /// Total enclosed volume of the domain, via the divergence theorem over the
    /// boundary faces (`V = (1/3) Σ_boundary c_f · A_f`, `A_f` outward).
    ///
    /// Internal faces cancel, so only boundary faces contribute; the result is
    /// positive for a well-formed closed domain with outward boundary normals.
    pub fn total_volume(&self) -> f64 {
        let mut v6 = 0.0;
        for f in 0..self.face_count() {
            if self.neighbour[f].is_none() {
                v6 += self.face_centroid(f).dot(self.face_area_vector(f));
            }
        }
        v6 / 3.0
    }

    /// Check structural validity: owner/neighbour cell indices in range, and
    /// every **cell closed** — the sum of its oriented face-area vectors is
    /// (near) zero, the discrete Gauss condition a valid FV cell must satisfy.
    ///
    /// Returns `Ok(())` or a message describing the first failure.
    pub fn validate(&self) -> Result<(), String> {
        if self.owner.len() != self.face_count() || self.neighbour.len() != self.face_count() {
            return Err("owner/neighbour length does not match face count".into());
        }
        // Accumulate each face's area vector onto its owner (+) and neighbour (−).
        let mut cell_area = vec![Vec3::ZERO; self.n_cells];
        for f in 0..self.face_count() {
            let a = self.face_area_vector(f);
            let o = self.owner[f];
            if o >= self.n_cells {
                return Err(format!("face {f} owner {o} out of range"));
            }
            cell_area[o] = cell_area[o].add(a);
            if let Some(n) = self.neighbour[f] {
                if n >= self.n_cells {
                    return Err(format!("face {f} neighbour {n} out of range"));
                }
                cell_area[n] = cell_area[n].sub(a);
            }
        }
        let scale = self.total_volume().abs().cbrt().max(1.0);
        for (c, a) in cell_area.iter().enumerate() {
            if a.length() > 1e-9 * scale * scale {
                return Err(format!("cell {c} is not closed (Σ face areas = {})", a.length()));
            }
        }
        Ok(())
    }
}
