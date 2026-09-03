// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

use crate::mesh::ami::AmiCoupling;
use crate::mesh::error::MeshError;
use crate::primitives::Vector3;

/// Boundary patch descriptor: topology + kind.
///
/// Face indices in [start, start + size) within the global face array.
/// All boundary faces appear after the internal faces in OpenFOAM ordering:
/// `start >= n_internal_faces` for every patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryPatch {
    /// Patch name (e.g. `"left"`, `"wall"`, `"inlet"`).
    pub name: String,
    /// Index of the first face of this patch in the global face list.
    pub start: usize,
    /// Number of faces in this patch.
    pub size: usize,
    /// Topological type of the patch (wall, symmetry, empty, …).
    pub kind: PatchKind,
    /// For a [`PatchKind::Cyclic`] patch, the **patch index** of its matching
    /// partner (the other half of the periodic pair); `None` for every
    /// non-cyclic patch, and `None` for a cyclic patch whose partner has not yet
    /// been resolved (e.g. one read from a `polyMesh` whose `neighbourPatch`
    /// ordering is not parsed yet).
    ///
    /// Mirrors `Foam::cyclicPolyPatch::neighbPatchID()`
    /// (`src/meshTools/.../cyclic/cyclicPolyPatch.H`). Local face `i` of this
    /// patch corresponds to local face `i` of the partner patch (OpenFOAM
    /// half0/half1 ordering), so the two halves must have equal `size`.
    pub cyclic_partner: Option<usize>,
}

impl BoundaryPatch {
    /// Construct a patch spanning faces `[start, start + size)` of the global
    /// face array, with the given name and [`PatchKind`].
    ///
    /// The [`cyclic_partner`](Self::cyclic_partner) is left `None`; for a
    /// periodic pair use [`new_cyclic`](Self::new_cyclic) instead so the two
    /// halves know each other.
    pub fn new(name: impl Into<String>, start: usize, size: usize, kind: PatchKind) -> Self {
        Self {
            name: name.into(),
            start,
            size,
            kind,
            cyclic_partner: None,
        }
    }

    /// Construct a [`PatchKind::Cyclic`] (periodic) patch spanning faces
    /// `[start, start + size)` whose matching partner is patch index
    /// `partner_patch`.
    ///
    /// Local face `i` of this patch is paired with local face `i` of the partner
    /// (OpenFOAM half0/half1 ordering); the partner patch must have the same
    /// `size` and must name this patch back as *its* partner. The
    /// [`FvMeshBuilder`] validates that symmetry and derives the across-seam
    /// cell couplings ([`CyclicCoupling`]) at build time.
    ///
    /// OpenFOAM: `cyclicPolyPatch` (`src/meshTools/.../cyclic/cyclicPolyPatch.H`).
    pub fn new_cyclic(
        name: impl Into<String>,
        start: usize,
        size: usize,
        partner_patch: usize,
    ) -> Self {
        Self {
            name: name.into(),
            start,
            size,
            kind: PatchKind::Cyclic,
            cyclic_partner: Some(partner_patch),
        }
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
    /// Generic boundary patch.
    Patch,
    /// No-slip wall.
    Wall,
    /// Symmetry plane.
    Symmetry,
    /// 2-D reduced case (zero-area faces).
    Empty,
    /// Axisymmetric wedge.
    Wedge,
    /// Periodic / matching pair (conformal — faces line up one-to-one).
    Cyclic,
    /// Non-conformal periodic pair — arbitrary mesh interface (AMI). The two
    /// halves' faces do not match one-to-one; each target face couples to a
    /// weighted set of source faces (see [`AmiCoupling`]).
    /// Mirrors OpenFOAM `cyclicAMIPolyPatch`.
    CyclicAmi,
    /// Inter-processor decomposition seam.
    Processor,
}

/// One across-the-seam cell coupling introduced by a [`PatchKind::Cyclic`]
/// (periodic) patch pair.
///
/// A cyclic patch pair makes the domain periodic: a boundary face on one half
/// of the pair is physically the *same* interface as the matching face on the
/// other half. This struct records, for one such matched face pair, the two
/// cells it joins so the FV operators can couple them **exactly like an internal
/// face** — the owner cell of the half0 face (`owner`) is coupled to the owner
/// cell of the half1 face (`neighbour`), contributing an off-diagonal matrix
/// entry across the periodic seam.
///
/// The couplings are appended to the LDU face addressing *after* the internal
/// faces (see [`FvMatrix::new`](crate::ldu_matrix::FvMatrix::new)), so coupling
/// index `i` in [`FvMesh::cyclic_couplings`] occupies LDU face
/// `n_internal_faces + i`.
///
/// Mirrors the coupled-interface contribution of `Foam::cyclicFvPatchField`
/// (`src/finiteVolume/.../cyclic/cyclicFvPatchField.H`), whose
/// `patchNeighbourField()` supplies the value from the partner cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CyclicCoupling {
    /// Owner cell of the half0 (lower-patch-index) face — the "owner" side of
    /// the coupling.
    pub owner: usize,
    /// Owner cell of the matched half1 face — the "neighbour" across the seam.
    pub neighbour: usize,
    /// Global face index of the half0 face (on `patch_a`).
    pub face_a: usize,
    /// Global face index of the matched half1 face (on `patch_b`).
    pub face_b: usize,
    /// Patch index of half0 (the lower of the pair's two indices).
    pub patch_a: usize,
    /// Patch index of half1 (the partner of `patch_a`).
    pub patch_b: usize,
    /// Local face index within each half (`face_a - patches[patch_a].start ==
    /// face_b - patches[patch_b].start`).
    pub local: usize,
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

    /// Across-seam cell couplings from [`PatchKind::Cyclic`] (periodic) patch
    /// pairs, one entry per matched boundary-face pair. Empty for a mesh with no
    /// (resolved) cyclic pairs. Each entry is treated by the FV operators and
    /// the LDU matrix exactly like an internal face joining
    /// [`CyclicCoupling::owner`] and [`CyclicCoupling::neighbour`], appended to
    /// the LDU face addressing after the `n_internal_faces` internal faces.
    pub cyclic_couplings: Vec<CyclicCoupling>,

    /// Across-seam couplings from [`PatchKind::CyclicAmi`] (non-conformal
    /// periodic) patch pairs, one entry per **target** seam face. Empty for a
    /// mesh with no AMI pairs. Each entry couples its target cell to a weighted
    /// set of source cells (the geometric face overlaps); the FV operators and
    /// the LDU matrix append one LDU face per [`AmiWeight`](crate::mesh::AmiWeight)
    /// after the internal faces and the [`cyclic_couplings`](Self::cyclic_couplings)
    /// (see [`ami_ldu_start`](Self::ami_ldu_start)). Mirrors OpenFOAM
    /// `cyclicAMIFvPatchField`.
    pub ami_couplings: Vec<AmiCoupling>,

    // ── Geometry ──────────────────────────────────────────────────────────────
    /// Cell volumes `V[c]` `[m³]`.
    pub cell_volumes: Vec<f64>,
    /// Cell centres `C[c]` `[m]`.
    pub cell_centres: Vec<Vector3>,
    /// Face area vectors `Sf[f]` `[m²]`, pointing from owner toward neighbour
    /// (or outward for boundary faces).
    pub face_area_vectors: Vec<Vector3>,
    /// Face area magnitudes `|Sf[f]|` `[m²]`.
    pub face_areas: Vec<f64>,
    /// Face centres `Cf[f]` `[m]`.
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

    /// Number of across-seam cyclic couplings (length of
    /// [`cyclic_couplings`](Self::cyclic_couplings)).
    pub fn n_cyclic_couplings(&self) -> usize {
        self.cyclic_couplings.len()
    }

    /// LDU face index of cyclic coupling `i`.
    ///
    /// Cyclic couplings live *after* the internal faces in the LDU addressing,
    /// so coupling `i` occupies face `n_internal_faces + i`. The FV operators
    /// use this to write the off-diagonal coefficient of a periodic seam into
    /// the matrix.
    pub fn cyclic_coupling_face(&self, i: usize) -> usize {
        self.n_internal_faces + i
    }

    /// Given a global boundary face index on a resolved [`PatchKind::Cyclic`]
    /// patch, return the global face index of its matching partner face across
    /// the periodic seam.
    ///
    /// Returns `None` for internal faces, for non-cyclic boundary faces, and for
    /// cyclic faces whose partner patch is unresolved. Local face `i` of a
    /// cyclic patch maps to local face `i` of its partner (OpenFOAM half0/half1
    /// ordering).
    pub fn cyclic_partner_face(&self, global_face: usize) -> Option<usize> {
        let (pi, local) = self.patch_for_face(global_face)?;
        let patch = &self.patches[pi];
        if patch.kind != PatchKind::Cyclic {
            return None;
        }
        let pj = patch.cyclic_partner?;
        let partner = self.patches.get(pj)?;
        if local >= partner.size {
            return None;
        }
        Some(partner.start + local)
    }

    /// Build a uniform 1-D **periodic** (cyclic) mesh: `n` equal cells along the
    /// x-axis over `[0, length]`, with the two ends joined by a cyclic patch
    /// pair so the domain wraps around.
    ///
    /// This is the programmatic constructor for a periodic seam (the first-pass
    /// path — the `polyMesh` parser does not yet read cyclic `neighbourPatch`
    /// ordering). The resulting mesh has:
    ///
    /// - `n` cells of width `h = length / n`, centres at `(i + 0.5)·h`, volume
    ///   `h·area` `[m³]`;
    /// - `n − 1` internal faces (area vector `+x·area` `[m²]`);
    /// - two 1-face cyclic patches — `"left"` at `x = 0` (patch 0, outward
    ///   normal `−x`) and `"right"` at `x = length` (patch 1, outward normal
    ///   `+x`) — that are each other's partners.
    ///
    /// The derived [`CyclicCoupling`] joins cell `0` (left owner) to cell `n − 1`
    /// (right owner) with the same cell-to-cell distance `h` as an interior
    /// face, so a periodic problem discretised on this mesh reproduces the
    /// equivalent all-internal ring mesh (see the crate's cyclic V&V tests).
    ///
    /// # Parameters
    /// - `n` — number of cells (must be ≥ 2 for a meaningful periodic loop);
    /// - `length` — domain length `[m]`;
    /// - `area` — uniform cross-sectional face area `[m²]`.
    ///
    /// # Panics
    /// Panics if `n < 2` (a periodic loop needs at least two cells) or if
    /// `length`/`area` are not positive.
    ///
    /// OpenFOAM analogue: a `blockMesh` block with a `cyclic` patch pair on its
    /// two x-normal faces.
    pub fn periodic_1d(n: usize, length: f64, area: f64) -> FvMesh {
        assert!(n >= 2, "periodic_1d needs at least 2 cells, got {n}");
        assert!(length > 0.0, "length must be positive, got {length}");
        assert!(area > 0.0, "area must be positive, got {area}");
        let h = length / n as f64;
        let n_int = n - 1;

        // owner: internal faces couple cell f ↔ cell f+1; then the two boundary
        // faces (left owner = cell 0, right owner = cell n-1).
        let mut owner: Vec<usize> = (0..n_int).collect();
        owner.push(0); // left boundary face (patch 0)
        owner.push(n - 1); // right boundary face (patch 1)
        let neighbour: Vec<usize> = (1..n).collect();

        let cell_volumes = vec![h * area; n];
        let cell_centres = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * h, 0.0, 0.0))
            .collect();

        // Internal face centres at (f+1)*h; then left face at x=0, right at
        // x=length.
        let face_centres: Vec<Vector3> = (0..n_int)
            .map(|f| Vector3::new((f as f64 + 1.0) * h, 0.0, 0.0))
            .chain([Vector3::new(0.0, 0.0, 0.0), Vector3::new(length, 0.0, 0.0)])
            .collect();
        // Internal Sf = +x·area (owner→neighbour); left boundary outward = −x,
        // right boundary outward = +x.
        let face_area_vectors: Vec<Vector3> = (0..n_int)
            .map(|_| Vector3::new(area, 0.0, 0.0))
            .chain([Vector3::new(-area, 0.0, 0.0), Vector3::new(area, 0.0, 0.0)])
            .collect();

        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_int)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new_cyclic("left", n_int, 1, 1),
                BoundaryPatch::new_cyclic("right", n_int + 1, 1, 0),
            ])
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_centres(face_centres)
            .build()
            .expect("periodic_1d builds a consistent mesh")
    }

    /// Validate basic mesh consistency.  Returns `Err` on the first problem found.
    pub fn validate(&self) -> Result<(), MeshError> {
        let check_len = |array, got, expected| {
            if got != expected {
                Err(MeshError::ArrayLengthMismatch {
                    array,
                    expected,
                    got,
                })
            } else {
                Ok(())
            }
        };
        check_len("owner", self.owner.len(), self.n_faces)?;
        check_len("neighbour", self.neighbour.len(), self.n_internal_faces)?;
        check_len("cell_volumes", self.cell_volumes.len(), self.n_cells)?;
        check_len("cell_centres", self.cell_centres.len(), self.n_cells)?;
        check_len(
            "face_area_vectors",
            self.face_area_vectors.len(),
            self.n_faces,
        )?;
        check_len("face_areas", self.face_areas.len(), self.n_faces)?;
        check_len("face_centres", self.face_centres.len(), self.n_faces)?;

        // Patches must cover [n_internal_faces, n_faces) without gaps.
        let mut covered = self.n_internal_faces;
        for p in &self.patches {
            if p.start != covered {
                return Err(MeshError::PatchStartMismatch {
                    name: p.name.clone(),
                    expected: covered,
                    got: p.start,
                });
            }
            covered += p.size;
        }
        if covered != self.n_faces {
            return Err(MeshError::PatchCoverageMismatch {
                covered,
                n_faces: self.n_faces,
            });
        }

        // Cyclic patch pairs must be mutually symmetric and equally sized.
        for (pi, patch) in self.patches.iter().enumerate() {
            let Some(pj) = patch.cyclic_partner else {
                continue;
            };
            let partner = self.patches.get(pj).ok_or(MeshError::CyclicPairMismatch {
                name: patch.name.clone(),
                reason: "partner patch index out of range",
            })?;
            if partner.cyclic_partner != Some(pi) {
                return Err(MeshError::CyclicPairMismatch {
                    name: patch.name.clone(),
                    reason: "partner patch does not name this patch back",
                });
            }
            if partner.size != patch.size {
                return Err(MeshError::CyclicPairMismatch {
                    name: patch.name.clone(),
                    reason: "cyclic pair halves have different face counts",
                });
            }
        }

        // AMI couplings: every target/source cell index must be in range, and
        // every target must have at least one weighted source.
        for cc in &self.ami_couplings {
            if cc.target_cell >= self.n_cells || cc.target_face >= self.n_faces {
                return Err(MeshError::AmiCouplingInvalid {
                    target_face: cc.target_face,
                    reason: "target cell/face index out of range",
                });
            }
            if cc.weights.is_empty() {
                return Err(MeshError::AmiCouplingInvalid {
                    target_face: cc.target_face,
                    reason: "AMI target face has no overlapping source faces",
                });
            }
            for w in &cc.weights {
                if w.source_cell >= self.n_cells || w.source_face >= self.n_faces {
                    return Err(MeshError::AmiCouplingInvalid {
                        target_face: cc.target_face,
                        reason: "source cell/face index out of range",
                    });
                }
            }
        }
        Ok(())
    }
}

/// Builder for `FvMesh` — lets tests and I/O code assemble a mesh incrementally.
#[derive(Debug, Clone, Default)]
pub struct FvMeshBuilder {
    n_cells: usize,
    n_internal_faces: usize,
    owner: Vec<usize>,
    neighbour: Vec<usize>,
    patches: Vec<BoundaryPatch>,
    ami_couplings: Vec<AmiCoupling>,
    cell_volumes: Vec<f64>,
    cell_centres: Vec<Vector3>,
    face_area_vectors: Vec<Vector3>,
    face_areas: Vec<f64>,
    face_centres: Vec<Vector3>,
}

impl FvMeshBuilder {
    /// New empty builder (all arrays empty, all counts zero).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the number of cells.
    pub fn n_cells(mut self, n: usize) -> Self {
        self.n_cells = n;
        self
    }
    /// Set the number of internal faces (faces with both owner and neighbour).
    pub fn n_internal_faces(mut self, n: usize) -> Self {
        self.n_internal_faces = n;
        self
    }
    /// Set the `owner` array (owning cell per face; length == `n_faces`).
    pub fn owner(mut self, v: Vec<usize>) -> Self {
        self.owner = v;
        self
    }
    /// Set the `neighbour` array (neighbour cell per internal face; length ==
    /// `n_internal_faces`).
    pub fn neighbour(mut self, v: Vec<usize>) -> Self {
        self.neighbour = v;
        self
    }
    /// Set the boundary patch descriptors.
    pub fn patches(mut self, v: Vec<BoundaryPatch>) -> Self {
        self.patches = v;
        self
    }
    /// Set the non-conformal-periodic (AMI) seam couplings (one entry per target
    /// seam face). Left empty by default; set by AMI mesh constructors such as
    /// [`FvMesh::periodic_ring_ami`].
    pub fn ami_couplings(mut self, v: Vec<AmiCoupling>) -> Self {
        self.ami_couplings = v;
        self
    }
    /// Set the cell volumes `V[c]` `[m³]` (length == `n_cells`).
    pub fn cell_volumes(mut self, v: Vec<f64>) -> Self {
        self.cell_volumes = v;
        self
    }
    /// Set the cell centres `C[c]` `[m]` (length == `n_cells`).
    pub fn cell_centres(mut self, v: Vec<Vector3>) -> Self {
        self.cell_centres = v;
        self
    }
    /// Set the face area vectors `Sf[f]` `[m²]` (length == `n_faces`).
    pub fn face_area_vectors(mut self, v: Vec<Vector3>) -> Self {
        self.face_area_vectors = v;
        self
    }
    /// Set the face area magnitudes `|Sf[f]|` `[m²]`. If left unset, they are
    /// derived from `face_area_vectors` at build time.
    pub fn face_areas(mut self, v: Vec<f64>) -> Self {
        self.face_areas = v;
        self
    }
    /// Set the face centres `Cf[f]` `[m]` (length == `n_faces`).
    pub fn face_centres(mut self, v: Vec<Vector3>) -> Self {
        self.face_centres = v;
        self
    }

    /// Derive `face_areas` from `face_area_vectors` if not set.
    fn ensure_face_areas(&mut self) {
        if self.face_areas.is_empty() && !self.face_area_vectors.is_empty() {
            self.face_areas = self.face_area_vectors.iter().map(|sf| sf.mag()).collect();
        }
    }

    /// Finalise the mesh: derive `face_areas` if needed, resolve any cyclic
    /// (periodic) patch pairs into [`CyclicCoupling`]s, assemble the [`FvMesh`],
    /// and run [`FvMesh::validate`]. Returns `Err` on the first consistency
    /// problem found.
    ///
    /// Cyclic couplings are derived from every [`PatchKind::Cyclic`] patch whose
    /// [`BoundaryPatch::cyclic_partner`] is set: each matched local face pair
    /// (half0 local `i` ↔ half1 local `i`) becomes one coupling joining the two
    /// faces' owner cells. Each pair is processed once (from the lower patch
    /// index).
    pub fn build(mut self) -> Result<FvMesh, MeshError> {
        self.ensure_face_areas();
        let n_faces = self.owner.len();
        let cyclic_couplings = Self::derive_cyclic_couplings(&self.patches, &self.owner);
        let mesh = FvMesh {
            n_cells: self.n_cells,
            n_internal_faces: self.n_internal_faces,
            n_faces,
            owner: self.owner,
            neighbour: self.neighbour,
            patches: self.patches,
            cyclic_couplings,
            ami_couplings: self.ami_couplings,
            cell_volumes: self.cell_volumes,
            cell_centres: self.cell_centres,
            face_area_vectors: self.face_area_vectors,
            face_areas: self.face_areas,
            face_centres: self.face_centres,
        };
        mesh.validate()?;
        Ok(mesh)
    }

    /// Build the across-seam couplings for every resolved cyclic patch pair.
    ///
    /// A pair `(pi, pj)` with `pi < pj` yields one [`CyclicCoupling`] per local
    /// face: `owner = owner[patches[pi].start + i]`, `neighbour =
    /// owner[patches[pj].start + i]`. Malformed pairs (missing/asymmetric
    /// partner, mismatched size) are skipped here and caught by
    /// [`FvMesh::validate`], which returns a descriptive error.
    fn derive_cyclic_couplings(patches: &[BoundaryPatch], owner: &[usize]) -> Vec<CyclicCoupling> {
        let mut couplings = Vec::new();
        for (pi, patch) in patches.iter().enumerate() {
            if patch.kind != PatchKind::Cyclic {
                continue;
            }
            let Some(pj) = patch.cyclic_partner else {
                continue;
            };
            // Process each pair once, from the lower index; skip malformed pairs
            // (validate() reports them).
            if pj <= pi {
                continue;
            }
            let Some(partner) = patches.get(pj) else {
                continue;
            };
            if partner.cyclic_partner != Some(pi) || partner.size != patch.size {
                continue;
            }
            for local in 0..patch.size {
                let face_a = patch.start + local;
                let face_b = partner.start + local;
                couplings.push(CyclicCoupling {
                    owner: owner[face_a],
                    neighbour: owner[face_b],
                    face_a,
                    face_b,
                    patch_a: pi,
                    patch_b: pj,
                    local,
                });
            }
        }
        couplings
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
            .neighbour(vec![1]) // only internal face 0
            .patches(vec![
                BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![
                Vector3::new(0.25, 0.0, 0.0),
                Vector3::new(0.75, 0.0, 0.0),
            ])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0), // internal face: pointing right (owner→neighbour)
                Vector3::new(1.0, 0.0, 0.0), // right boundary: outward
                Vector3::new(-1.0, 0.0, 0.0), // left boundary: outward
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
        assert_eq!(m.patch_for_face(0), None); // internal
        assert_eq!(m.patch_for_face(1), Some((0, 0))); // right patch (index 0), local face 0
        assert_eq!(m.patch_for_face(2), Some((1, 0))); // left patch (index 1), local face 0
    }

    #[test]
    fn face_areas_derived() {
        let m = two_cell_mesh();
        assert!((m.face_areas[0] - 1.0).abs() < 1e-15);
    }

    #[test]
    fn periodic_1d_topology() {
        // 4-cell periodic ring, length 1, area 1.
        let m = FvMesh::periodic_1d(4, 1.0, 1.0);
        assert_eq!(m.n_cells, 4);
        assert_eq!(m.n_internal_faces, 3);
        assert_eq!(m.n_faces, 5); // 3 internal + 2 boundary
        assert_eq!(m.patches.len(), 2);
        assert_eq!(m.patches[0].kind, PatchKind::Cyclic);
        assert_eq!(m.patches[0].cyclic_partner, Some(1));
        assert_eq!(m.patches[1].cyclic_partner, Some(0));
        // One cyclic coupling: cell 0 (left owner) ↔ cell 3 (right owner).
        assert_eq!(m.n_cyclic_couplings(), 1);
        let cc = m.cyclic_couplings[0];
        assert_eq!(cc.owner, 0);
        assert_eq!(cc.neighbour, 3);
        assert_eq!(cc.patch_a, 0);
        assert_eq!(cc.patch_b, 1);
        // LDU coupling face sits right after the internal faces.
        assert_eq!(m.cyclic_coupling_face(0), 3);
    }

    #[test]
    fn cyclic_partner_face_maps_across_seam() {
        let m = FvMesh::periodic_1d(4, 1.0, 1.0);
        // Left boundary face is global index 3; its partner is the right face 4.
        assert_eq!(m.cyclic_partner_face(3), Some(4));
        assert_eq!(m.cyclic_partner_face(4), Some(3));
        // Internal faces have no cyclic partner.
        assert_eq!(m.cyclic_partner_face(0), None);
    }

    #[test]
    fn cyclic_pair_validation_rejects_asymmetric_partner() {
        // Patch 0 claims patch 1 as partner, but patch 1 is a plain wall.
        let err = FvMeshBuilder::new()
            .n_cells(2)
            .n_internal_faces(1)
            .owner(vec![0, 0, 1])
            .neighbour(vec![1])
            .patches(vec![
                BoundaryPatch::new_cyclic("left", 1, 1, 1),
                BoundaryPatch::new("right", 2, 1, PatchKind::Wall),
            ])
            .cell_volumes(vec![1.0, 1.0])
            .cell_centres(vec![
                Vector3::new(0.25, 0.0, 0.0),
                Vector3::new(0.75, 0.0, 0.0),
            ])
            .face_area_vectors(vec![
                Vector3::new(1.0, 0.0, 0.0),
                Vector3::new(-1.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .face_centres(vec![
                Vector3::new(0.5, 0.0, 0.0),
                Vector3::new(0.0, 0.0, 0.0),
                Vector3::new(1.0, 0.0, 0.0),
            ])
            .build();
        assert!(matches!(err, Err(MeshError::CyclicPairMismatch { .. })));
    }
}
