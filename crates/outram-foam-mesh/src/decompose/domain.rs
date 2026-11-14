// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Algorithms ported from OpenFOAM.com (ESI) v2506, applications/utilities/parallelProcessing/
// decomposePar/domainDecompositionMesh.C (`decomposeMesh`, `addInterProcFace`) and
// domainDecomposition.C (`writeDecomposition`, the polyMesh part), and
// OpenFOAM/meshes/polyMesh/polyMeshInitMesh.C (`initMesh(cellList)`), GPL-3.0-or-later,
// OpenCFD Ltd.

//! `decomposePar`'s mesh decomposition: processor addressing, processor patches and the
//! per-processor `polyMesh` (gh:#1381).
//!
//! [`decompose_mesh`] is `domainDecomposition::decomposeMesh` for a mesh without cyclic
//! patches: internal faces whose two cells sit on one processor stay internal, the others
//! become a face of an inter-processor patch on both sides (the neighbour side turned,
//! recorded as a negative face label), boundary faces go with their cell, and the points of
//! a processor are those its faces use. [`ProcessorAddressing::processor_mesh`] builds the
//! processor's `polyMesh` as `polyMesh(io, points, faces, cells)` does: owner and neighbour
//! by the first-visit rule of `initMesh(cellList)`.
//!
//! **Not ported:** cyclic and `processorCyclic` patches (refused with an error), point, face
//! and cell zones and sets, `hexRef8` data, `pointMesh` patches, moving meshes (`facesInstance`
//! points), weights and decomposition constraints. More than 128 processors is refused: ESI
//! walks `Map<label>::toc()`, which is ascending only while the labels are below the initial
//! table size of 128, and `sortedOrder` on it is computed but never used (see the V&V README).

use std::collections::BTreeMap;
use std::path::Path;

use outram_foam_basic_lib::io::{write_label_list_esi, EsiWriteOptions, IoError, MeshFace, PolyMesh};
use outram_foam_basic_lib::mesh::{BoundaryPatch, PatchKind};

use super::geom::GeomDecomp;
use crate::check_mesh::esi_geometry;
use crate::snappy_hex_mesh::esi_snap::SnapMesh;

/// Why a mesh could not be decomposed.
#[derive(Debug, thiserror::Error)]
pub enum DecomposeError {
    /// The mesh has a cyclic patch (not ported).
    #[error("patch {0} is cyclic: cyclic and processorCyclic patches are not ported")]
    Cyclic(String),
    /// More than 128 processors (ESI's `Map<label>` order stops being ascending).
    #[error("{0} processors: more than 128 is not supported")]
    TooManyProcessors(usize),
    /// `cell_to_proc` has the wrong length or a processor out of range.
    #[error("cell-to-processor list is invalid: {0}")]
    BadCellToProc(String),
    /// Writing a file failed.
    #[error(transparent)]
    Io(#[from] IoError),
}

/// Everything `decomposeMesh` records about one processor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessorAddressing {
    /// `procPointAddressing_`: global point of every local point (ascending).
    pub points: Vec<usize>,
    /// `procFaceAddressing_`: `global face + 1`, negated for a face turned to point out of
    /// the processor's cell (the neighbour side of an inter-processor face).
    pub faces: Vec<i64>,
    /// `procCellAddressing_`: global cell of every local cell (ascending).
    pub cells: Vec<usize>,
    /// `procPatchSize_`: faces of every ordinary patch on this processor.
    pub patch_sizes: Vec<usize>,
    /// `procPatchStartIndex_`: first local face of every ordinary patch.
    pub patch_starts: Vec<usize>,
    /// `procNeighbourProcessors_`: neighbour processor of every processor patch, ascending.
    pub nbr_procs: Vec<usize>,
    /// `procProcessorPatchSize_`.
    pub nbr_sizes: Vec<usize>,
    /// `procProcessorPatchStartIndex_`.
    pub nbr_starts: Vec<usize>,
}

/// The result of `decomposeMesh`: the cell-to-processor list and every processor's addressing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decomposition {
    /// Number of processors.
    pub n_procs: usize,
    /// `cellToProc_`.
    pub cell_to_proc: Vec<usize>,
    /// Addressing per processor.
    pub procs: Vec<ProcessorAddressing>,
}

/// Cell centres exactly as ESI's `primitiveMesh::cellCentres()` computes them (the
/// `esi_geometry` port, bit-identical to the binary on every mesh of the checkMesh differential).
pub fn esi_cell_centres(mesh: &PolyMesh) -> Vec<[f64; 3]> {
    let snap = SnapMesh {
        points: mesh.points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: mesh.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: mesh.faces.iter().map(|f| f.owner).collect(),
        neighbour: mesh.faces.iter().take(mesh.n_internal_faces).map(|f| f.neighbour.unwrap_or(0)).collect(),
    };
    esi_geometry(&snap).cell_centres
}

/// `decomposePar` for the geometric methods: decompose the cell centres with `method`, then
/// [`decompose_mesh`].
///
/// # Errors
/// As [`decompose_mesh`].
pub fn decompose_par(mesh: &PolyMesh, method: &GeomDecomp) -> Result<Decomposition, DecomposeError> {
    let cell_to_proc = method.decompose(&esi_cell_centres(mesh));
    decompose_mesh(mesh, &cell_to_proc, method.n_domains())
}

/// `domainDecomposition::decomposeMesh` after `distributeCells`.
///
/// # Errors
/// [`DecomposeError`] for a cyclic patch, more than 128 processors, or a bad `cell_to_proc`.
pub fn decompose_mesh(mesh: &PolyMesh, cell_to_proc: &[usize], n_procs: usize) -> Result<Decomposition, DecomposeError> {
    if n_procs > 128 {
        return Err(DecomposeError::TooManyProcessors(n_procs));
    }
    if let Some(p) = mesh.patches.iter().find(|p| matches!(p.kind, PatchKind::Cyclic | PatchKind::CyclicAmi)) {
        return Err(DecomposeError::Cyclic(p.name.clone()));
    }
    if cell_to_proc.len() != mesh.n_cells {
        return Err(DecomposeError::BadCellToProc(format!(
            "{} entries for {} cells",
            cell_to_proc.len(),
            mesh.n_cells
        )));
    }
    if let Some(&bad) = cell_to_proc.iter().find(|&&p| p >= n_procs) {
        return Err(DecomposeError::BadCellToProc(format!("processor {bad} of {n_procs}")));
    }
    let n_patches = mesh.patches.len();
    // invertOneToMany(nProcs, cellToProc): cells of every processor, ascending
    let mut cells: Vec<Vec<usize>> = vec![Vec::new(); n_procs];
    for (c, &p) in cell_to_proc.iter().enumerate() {
        cells[p].push(c);
    }
    let mut faces: Vec<Vec<i64>> = vec![Vec::new(); n_procs];
    // internal faces with both cells on one processor
    for (f, face) in mesh.faces.iter().take(mesh.n_internal_faces).enumerate() {
        let (o, n) = (face.owner, face.neighbour.unwrap_or(0));
        if cell_to_proc[o] == cell_to_proc[n] {
            faces[cell_to_proc[o]].push(f as i64 + 1);
        }
    }
    // ordinary patches: faces go with their cell
    let mut patch_sizes = vec![vec![0usize; n_patches]; n_procs];
    let mut patch_starts = vec![vec![0usize; n_patches]; n_procs];
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for p in 0..n_procs {
            patch_starts[p][pi] = faces[p].len();
        }
        for f in patch.start..patch.start + patch.size {
            let p = cell_to_proc[mesh.faces[f].owner];
            faces[p].push(f as i64 + 1);
            patch_sizes[p][pi] += 1;
        }
    }
    // inter-processor faces, per processor and neighbour (BTreeMap = ascending toc order)
    let mut inter: Vec<BTreeMap<usize, Vec<i64>>> = vec![BTreeMap::new(); n_procs];
    for (f, face) in mesh.faces.iter().take(mesh.n_internal_faces).enumerate() {
        let op = cell_to_proc[face.owner];
        let np = cell_to_proc[face.neighbour.unwrap_or(0)];
        if op != np {
            inter[op].entry(np).or_default().push(f as i64 + 1);
            inter[np].entry(op).or_default().push(-(f as i64 + 1));
        }
    }
    let mut procs = Vec::with_capacity(n_procs);
    for p in 0..n_procs {
        let mut nbr_procs = Vec::new();
        let mut nbr_sizes = Vec::new();
        let mut nbr_starts = Vec::new();
        for (&nbr, list) in &inter[p] {
            nbr_procs.push(nbr);
            nbr_sizes.push(list.len());
            nbr_starts.push(faces[p].len());
            faces[p].extend_from_slice(list);
        }
        // points in use, ascending (bitSet::sortedToc)
        let mut in_use = vec![false; mesh.points.len()];
        for &fl in &faces[p] {
            for &v in &mesh.faces[(fl.unsigned_abs() - 1) as usize].verts {
                in_use[v] = true;
            }
        }
        let points: Vec<usize> = (0..in_use.len()).filter(|&i| in_use[i]).collect();
        procs.push(ProcessorAddressing {
            points,
            faces: std::mem::take(&mut faces[p]),
            cells: std::mem::take(&mut cells[p]),
            patch_sizes: std::mem::take(&mut patch_sizes[p]),
            patch_starts: std::mem::take(&mut patch_starts[p]),
            nbr_procs,
            nbr_sizes,
            nbr_starts,
        });
    }
    Ok(Decomposition { n_procs, cell_to_proc: cell_to_proc.to_vec(), procs })
}

/// `face::reverseFace`: the first vertex stays, the rest run backwards.
fn reverse_face(verts: &[usize]) -> Vec<usize> {
    let n = verts.len();
    let mut out = Vec::with_capacity(n);
    if n > 0 {
        out.push(verts[0]);
        for i in 1..n {
            out.push(verts[n - i]);
        }
    }
    out
}

impl ProcessorAddressing {
    /// Number of processor patches.
    pub fn n_processor_patches(&self) -> usize {
        self.nbr_procs.len()
    }

    /// The processor's `polyMesh` as `domainDecomposition::writeDecomposition` builds it:
    /// points, faces (turned where the label is negative), cells from the global cells' faces
    /// through the face lookup, owner and neighbour by `initMesh(cellList)`'s first-visit rule,
    /// the ordinary patches cloned with their new start and size, then the processor patches
    /// `procBoundary<me>to<nbr>`.
    pub fn processor_mesh(&self, mesh: &PolyMesh, proc_no: usize) -> PolyMesh {
        let n_points = mesh.points.len();
        let mut point_lookup = vec![usize::MAX; n_points];
        for (i, &g) in self.points.iter().enumerate() {
            point_lookup[g] = i;
        }
        let points = self.points.iter().map(|&g| mesh.points[g]).collect();
        let mut face_lookup = vec![usize::MAX; mesh.faces.len()];
        let mut proc_faces: Vec<Vec<usize>> = Vec::with_capacity(self.faces.len());
        for (i, &fl) in self.faces.iter().enumerate() {
            let g = (fl.unsigned_abs() - 1) as usize;
            face_lookup[g] = i;
            let orig = &mesh.faces[g].verts;
            let verts = if fl >= 0 { orig.clone() } else { reverse_face(orig) };
            proc_faces.push(verts.iter().map(|&v| point_lookup[v]).collect());
        }
        // faces of every global cell, in the order primitiveMesh::calcCells fills them
        let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); mesh.n_cells];
        for (f, face) in mesh.faces.iter().enumerate() {
            cell_faces[face.owner].push(f);
        }
        for (f, face) in mesh.faces.iter().take(mesh.n_internal_faces).enumerate() {
            cell_faces[face.neighbour.unwrap_or(0)].push(f);
        }
        // initMesh(cellList): first visit owns, second is the neighbour
        let nf = proc_faces.len();
        let mut owner = vec![usize::MAX; nf];
        let mut neighbour = vec![usize::MAX; nf];
        let mut marked = vec![false; nf];
        let mut n_internal = 0;
        for (lc, &gc) in self.cells.iter().enumerate() {
            for &gf in &cell_faces[gc] {
                let lf = face_lookup[gf];
                if marked[lf] {
                    neighbour[lf] = lc;
                    n_internal += 1;
                } else {
                    owner[lf] = lc;
                    marked[lf] = true;
                }
            }
        }
        let faces = (0..nf)
            .map(|lf| MeshFace {
                verts: proc_faces[lf].clone(),
                owner: owner[lf],
                neighbour: if lf < n_internal { Some(neighbour[lf]) } else { None },
            })
            .collect();
        let mut patches = Vec::new();
        for (pi, p) in mesh.patches.iter().enumerate() {
            patches.push(BoundaryPatch::new(p.name.clone(), self.patch_starts[pi], self.patch_sizes[pi], p.kind));
        }
        for k in 0..self.nbr_procs.len() {
            patches.push(BoundaryPatch::new(
                format!("procBoundary{proc_no}to{}", self.nbr_procs[k]),
                self.nbr_starts[k],
                self.nbr_sizes[k],
                PatchKind::Processor,
            ));
        }
        PolyMesh { points, faces, n_internal_faces: n_internal, n_cells: self.cells.len(), patches }
    }

    /// `boundaryProcAddressing`: the ordinary patches map to themselves, processor patches to -1.
    pub fn boundary_addressing(&self, n_mesh_patches: usize) -> Vec<i64> {
        let mut v: Vec<i64> = (0..n_mesh_patches as i64).collect();
        v.resize(n_mesh_patches + self.nbr_procs.len(), -1);
        v
    }
}

/// Write `<case>/processor<N>/constant/polyMesh/` for every processor: the five mesh files and
/// the four `*ProcAddressing` files, as `decomposePar` does. `IOstream::minPrecision(10)` is
/// applied: the precision is the larger of `opts.precision` and 10.
///
/// # Errors
/// [`DecomposeError::Io`] if a file cannot be written.
pub fn write_processor_cases(
    case_dir: impl AsRef<Path>,
    mesh: &PolyMesh,
    decomposition: &Decomposition,
    opts: &EsiWriteOptions,
) -> Result<(), DecomposeError> {
    let mut o = opts.clone();
    o.precision = o.precision.max(10);
    let location = "constant/polyMesh";
    for (p, addr) in decomposition.procs.iter().enumerate() {
        let dir = case_dir.as_ref().join(format!("processor{p}")).join("constant").join("polyMesh");
        addr.processor_mesh(mesh, p).write_esi_processor(&dir, location, &o, p, &addr.nbr_procs)?;
        let labels = |v: &[usize]| v.iter().map(|&x| x as i64).collect::<Vec<_>>();
        write_label_list_esi(dir.join("pointProcAddressing"), location, "pointProcAddressing", &labels(&addr.points), &o)?;
        write_label_list_esi(dir.join("faceProcAddressing"), location, "faceProcAddressing", &addr.faces, &o)?;
        write_label_list_esi(dir.join("cellProcAddressing"), location, "cellProcAddressing", &labels(&addr.cells), &o)?;
        write_label_list_esi(
            dir.join("boundaryProcAddressing"),
            location,
            "boundaryProcAddressing",
            &addr.boundary_addressing(mesh.patches.len()),
            &o,
        )?;
    }
    Ok(())
}
