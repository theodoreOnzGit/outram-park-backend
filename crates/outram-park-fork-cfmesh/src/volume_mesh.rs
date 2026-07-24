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

/// Invert the mesh to a per-cell list of **outward-wound** face rings: each
/// cell gets its owner faces as stored (owner→neighbour = outward from the
/// owner) and its neighbour faces reversed (outward from that cell). The
/// inverse of [`from_cell_faces`].
pub fn cells_faces(mesh: &VolumeMesh) -> Vec<Vec<Vec<usize>>> {
    let mut cf: Vec<Vec<Vec<usize>>> = vec![Vec::new(); mesh.n_cells];
    for f in 0..mesh.face_count() {
        cf[mesh.owner[f]].push(mesh.faces[f].clone());
        if let Some(nb) = mesh.neighbour[f] {
            let mut rev = mesh.faces[f].clone();
            rev.reverse();
            cf[nb].push(rev);
        }
    }
    cf
}

/// Assemble a [`VolumeMesh`] from `points` and a per-cell list of
/// **outward-wound** face rings. Faces shared by two cells (matched by their
/// vertex *set*) become internal faces (owner = the cell that listed it first);
/// unmatched faces are boundary faces in a single `walls` patch. Faces are
/// ordered internal-first.
///
/// This is the general "cells → connectivity" assembler used by mesh surgery
/// such as boundary-layer insertion, where owner/neighbour are easiest to
/// recover by matching shared faces rather than tracked directly.
pub fn from_cell_faces(points: Vec<Vec3>, cells: &[Vec<Vec<usize>>]) -> VolumeMesh {
    use std::collections::HashMap;
    // Cell centroids (vertex average over the cell's faces) — used to orient
    // every emitted face outward from its owner, so callers need not wind the
    // input rings correctly.
    let centroid: Vec<Vec3> = cells
        .iter()
        .map(|faces| {
            let mut c = Vec3::ZERO;
            let mut n = 0usize;
            for ring in faces {
                for &v in ring {
                    c = c.add(points[v]);
                    n += 1;
                }
            }
            if n > 0 {
                c.scale(1.0 / n as f64)
            } else {
                Vec3::ZERO
            }
        })
        .collect();

    // First occurrence of each face (by sorted vertex key) -> (cell, ring).
    let mut first: HashMap<Vec<usize>, (usize, Vec<usize>)> = HashMap::new();
    let mut int_faces: Vec<Vec<usize>> = Vec::new();
    let mut int_owner: Vec<usize> = Vec::new();
    let mut int_nb: Vec<usize> = Vec::new();
    for (cid, faces) in cells.iter().enumerate() {
        for ring in faces {
            let mut key = ring.clone();
            key.sort_unstable();
            if let Some((owner_cell, owner_ring)) = first.remove(&key) {
                // Second time we see this face -> internal (owner = first cell).
                int_faces.push(orient_ring(owner_ring, centroid[owner_cell], &points));
                int_owner.push(owner_cell);
                int_nb.push(cid);
            } else {
                first.insert(key, (cid, ring.clone()));
            }
        }
    }
    // Whatever is left unmatched is a boundary face.
    let n_internal = int_faces.len();
    let mut faces = int_faces;
    let mut owner = int_owner;
    let mut neighbour: Vec<Option<usize>> = int_nb.into_iter().map(Some).collect();
    let mut bnd = 0usize;
    for (_key, (cid, ring)) in first {
        faces.push(orient_ring(ring, centroid[cid], &points));
        owner.push(cid);
        neighbour.push(None);
        bnd += 1;
    }
    let patches = vec![BoundaryPatch { name: "walls".into(), start_face: n_internal, n_faces: bnd }];
    VolumeMesh { points, faces, owner, neighbour, n_cells: cells.len(), patches }
}

/// Order a face `ring` so its area vector points **away from** `owner_center`
/// (owner→neighbour for an internal face, outward for a boundary face). Shared
/// by the meshers so face construction never has to hand-derive winding.
pub(crate) fn orient_ring(ring: Vec<usize>, owner_center: Vec3, points: &[Vec3]) -> Vec<usize> {
    let k = ring.len();
    let mut area = Vec3::ZERO;
    for i in 0..k {
        area = area.add(points[ring[i]].cross(points[ring[(i + 1) % k]]));
    }
    let mut c = Vec3::ZERO;
    for &v in &ring {
        c = c.add(points[v]);
    }
    let c = c.scale(1.0 / k as f64);
    if area.dot(c.sub(owner_center)) < 0.0 {
        let mut r = ring;
        r.reverse();
        r
    } else {
        ring
    }
}
