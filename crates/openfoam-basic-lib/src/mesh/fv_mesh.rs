use crate::primitives::Vector3;

/// Boundary patch descriptor: topology + kind.
///
/// Face indices in [start, start + size) within the global face array.
/// All boundary faces appear after the internal faces in OpenFOAM ordering:
/// `start >= n_internal_faces` for every patch.
#[derive(Debug, Clone)]
pub struct BoundaryPatch {
    pub name: String,
    /// Index of the first face of this patch in the global face list.
    pub start: usize,
    /// Number of faces in this patch.
    pub size: usize,
    pub kind: PatchKind,
}

impl BoundaryPatch {
    pub fn new(name: impl Into<String>, start: usize, size: usize, kind: PatchKind) -> Self {
        Self { name: name.into(), start, size, kind }
    }

    /// Last+1 face index (exclusive upper bound).
    pub fn end(&self) -> usize {
        self.start + self.size
    }

    /// True if global face index `f` belongs to this patch.
    pub fn contains_face(&self, f: usize) -> bool {
        f >= self.start && f < self.end()
    }
}

/// Topological type of a boundary patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchKind {
    Patch,      // generic boundary
    Wall,       // no-slip wall
    Symmetry,   // symmetry plane
    Empty,      // 2-D reduced case (zero area)
    Wedge,      // axisymmetric wedge
    Cyclic,     // periodic / matching pair
    Processor,  // inter-processor decomposition seam
}

/// Finite-volume mesh — topology and geometry in a flat data structure.
///
/// Mirrors `Foam::fvMesh` (`src/finiteVolume/fvMesh/fvMesh.H`) but without
/// the C++ inheritance chain (`polyMesh → primitiveMesh → lduMesh`).
/// Only the data required by the FV operators is stored.
///
/// ## Face ordering (OpenFOAM convention)
/// ```text
/// [0 .. n_internal_faces)         ← internal faces (have both owner & neighbour)
/// [n_internal_faces .. n_faces)   ← boundary faces (owner only)
/// ```
/// The `neighbour` array has length `n_internal_faces`; boundary faces have no
/// entry in `neighbour`.
#[derive(Debug, Clone)]
pub struct FvMesh {
    // ── Topology ──────────────────────────────────────────────────────────────

    /// Number of cells.
    pub n_cells: usize,
    /// Number of internal faces (both owner and neighbour defined).
    pub n_internal_faces: usize,
    /// Total number of faces (internal + boundary).
    pub n_faces: usize,

    /// `owner[f]` — cell that owns face `f` (for all faces).
    pub owner: Vec<usize>,
    /// `neighbour[f]` — cell on the other side of internal face `f`.
    /// Length == `n_internal_faces`; boundary faces have no neighbour.
    pub neighbour: Vec<usize>,

    /// Boundary patch descriptors (one per patch, in face-index order).
    pub patches: Vec<BoundaryPatch>,

    // ── Geometry ──────────────────────────────────────────────────────────────

    /// Cell volumes `V[c]` [m³].
    pub cell_volumes: Vec<f64>,
    /// Cell centres `C[c]` [m].
    pub cell_centres: Vec<Vector3>,
    /// Face area vectors `Sf[f]` [m²], pointing from owner toward neighbour
    /// (or outward for boundary faces).
    pub face_area_vectors: Vec<Vector3>,
    /// Face area magnitudes `|Sf[f]|` [m²].
    pub face_areas: Vec<f64>,
    /// Face centres `Cf[f]` [m].
    pub face_centres: Vec<Vector3>,
}

impl FvMesh {
    /// Total number of boundary faces.
    pub fn n_boundary_faces(&self) -> usize {
        self.n_faces - self.n_internal_faces
    }

    /// Number of patches.
    pub fn n_patches(&self) -> usize {
        self.patches.len()
    }

    /// True if face `f` is an internal face (has a neighbour cell).
    pub fn is_internal_face(&self, f: usize) -> bool {
        f < self.n_internal_faces
    }

    /// Given a global face index `f` that is a boundary face, return
    /// `(patch_index, local_face_index_within_patch)`.
    /// Returns `None` if `f` is an internal face.
    pub fn patch_for_face(&self, f: usize) -> Option<(usize, usize)> {
        if self.is_internal_face(f) {
            return None;
        }
        for (pi, patch) in self.patches.iter().enumerate() {
            if patch.contains_face(f) {
                return Some((pi, f - patch.start));
            }
        }
        None
    }

    /// Validate basic mesh consistency.  Returns `Err` with a description on
    /// the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        if self.owner.len() != self.n_faces {
            return Err(format!(
                "owner len {} != n_faces {}", self.owner.len(), self.n_faces));
        }
        if self.neighbour.len() != self.n_internal_faces {
            return Err(format!(
                "neighbour len {} != n_internal_faces {}", self.neighbour.len(), self.n_internal_faces));
        }
        if self.cell_volumes.len() != self.n_cells {
            return Err(format!(
                "cell_volumes len {} != n_cells {}", self.cell_volumes.len(), self.n_cells));
        }
        if self.cell_centres.len() != self.n_cells {
            return Err("cell_centres length mismatch".into());
        }
        if self.face_area_vectors.len() != self.n_faces {
            return Err("face_area_vectors length mismatch".into());
        }
        if self.face_areas.len() != self.n_faces {
            return Err("face_areas length mismatch".into());
        }
        if self.face_centres.len() != self.n_faces {
            return Err("face_centres length mismatch".into());
        }
        // Check patch coverage: patches should cover [n_internal_faces, n_faces)
        let mut covered = self.n_internal_faces;
        for p in &self.patches {
            if p.start != covered {
                return Err(format!("patch '{}' starts at {} but expected {}", p.name, p.start, covered));
            }
            covered += p.size;
        }
        if covered != self.n_faces {
            return Err(format!("patches cover {} faces but n_faces = {}", covered, self.n_faces));
        }
        Ok(())
    }
}

/// Builder for `FvMesh` — lets tests and I/O code assemble a mesh incrementally.
#[derive(Default)]
pub struct FvMeshBuilder {
    n_cells: usize,
    n_internal_faces: usize,
    owner: Vec<usize>,
    neighbour: Vec<usize>,
    patches: Vec<BoundaryPatch>,
    cell_volumes: Vec<f64>,
    cell_centres: Vec<Vector3>,
    face_area_vectors: Vec<Vector3>,
    face_areas: Vec<f64>,
    face_centres: Vec<Vector3>,
}

impl FvMeshBuilder {
    pub fn new() -> Self { Self::default() }

    pub fn n_cells(mut self, n: usize) -> Self { self.n_cells = n; self }
    pub fn n_internal_faces(mut self, n: usize) -> Self { self.n_internal_faces = n; self }
    pub fn owner(mut self, v: Vec<usize>) -> Self { self.owner = v; self }
    pub fn neighbour(mut self, v: Vec<usize>) -> Self { self.neighbour = v; self }
    pub fn patches(mut self, v: Vec<BoundaryPatch>) -> Self { self.patches = v; self }
    pub fn cell_volumes(mut self, v: Vec<f64>) -> Self { self.cell_volumes = v; self }
    pub fn cell_centres(mut self, v: Vec<Vector3>) -> Self { self.cell_centres = v; self }
    pub fn face_area_vectors(mut self, v: Vec<Vector3>) -> Self { self.face_area_vectors = v; self }
    pub fn face_areas(mut self, v: Vec<f64>) -> Self { self.face_areas = v; self }
    pub fn face_centres(mut self, v: Vec<Vector3>) -> Self { self.face_centres = v; self }

    /// Derive `face_areas` from `face_area_vectors` if not set.
    fn ensure_face_areas(&mut self) {
        if self.face_areas.is_empty() && !self.face_area_vectors.is_empty() {
            self.face_areas = self.face_area_vectors.iter().map(|sf| sf.mag()).collect();
        }
    }

    pub fn build(mut self) -> Result<FvMesh, String> {
        self.ensure_face_areas();
        let n_faces = self.owner.len();
        let mesh = FvMesh {
            n_cells: self.n_cells,
            n_internal_faces: self.n_internal_faces,
            n_faces,
            owner: self.owner,
            neighbour: self.neighbour,
            patches: self.patches,
            cell_volumes: self.cell_volumes,
            cell_centres: self.cell_centres,
            face_area_vectors: self.face_area_vectors,
            face_areas: self.face_areas,
            face_centres: self.face_centres,
        };
        mesh.validate()?;
        Ok(mesh)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple 1-D mesh: two cells sharing one internal face and each having
    /// one boundary face.
    ///
    /// ```
    ///  |   cell 0   |   cell 1   |
    ///  ^            ^            ^
    /// face 2    face 0(int)    face 1
    /// (patch 0)               (patch 1)
    /// ```
    fn two_cell_mesh() -> FvMesh {
        FvMeshBuilder::new()
            .n_cells(2)
            .n_internal_faces(1)
            // faces: [0=internal, 1=right boundary, 2=left boundary]
            .owner(vec![0, 1, 0])
            .neighbour(vec![1])          // only internal face 0
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left",  2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![Vector3::new(0.25, 0.0, 0.0), Vector3::new(0.75, 0.0, 0.0)])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),   // internal face: pointing right (owner→neighbour)
                Vector3::new(1.0, 0.0, 0.0),   // right boundary: outward
                Vector3::new(-1.0, 0.0, 0.0),  // left boundary: outward
            ])
            .face_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
            ])
            .build()
            .expect("two_cell_mesh should be valid")
    }

    #[test]
    fn validate_two_cell_mesh() {
        let m = two_cell_mesh();
        assert_eq!(m.n_cells, 2);
        assert_eq!(m.n_internal_faces, 1);
        assert_eq!(m.n_faces, 3);
        assert_eq!(m.n_boundary_faces(), 2);
    }

    #[test]
    fn is_internal_face() {
        let m = two_cell_mesh();
        assert!(m.is_internal_face(0));
        assert!(!m.is_internal_face(1));
        assert!(!m.is_internal_face(2));
    }

    #[test]
    fn patch_for_face() {
        let m = two_cell_mesh();
        assert_eq!(m.patch_for_face(0), None);         // internal
        assert_eq!(m.patch_for_face(1), Some((0, 0))); // right patch (index 0), local face 0
        assert_eq!(m.patch_for_face(2), Some((1, 0))); // left patch (index 1), local face 0
    }

    #[test]
    fn face_areas_derived() {
        let m = two_cell_mesh();
        assert!((m.face_areas[0] - 1.0).abs() < 1e-15);
    }
}
