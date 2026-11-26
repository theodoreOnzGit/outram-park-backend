// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam) and OpenFOAM-ESI
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.C (deformMesh, line 363),
//                    src/classes/thermalHydraulics/solvers/thermalHydraulicsModel.C (deformMesh, line 380),
//                    src/classes/thermoMechanics/thermoMechanics.C (deformMesh, line 353)
//   OpenFOAM-ESI v2506: src/finiteVolume/interpolation/volPointInterpolation/{volPointInterpolation.C,
//                    volPointInterpolate.C}, src/finiteVolume/fvMesh/fvMesh.C (movePoints),
//                    src/OpenFOAM/meshes/primitiveMesh/primitiveMeshPointCells.C (calcPointCells),
//                    src/OpenFOAM/meshes/primitiveMesh/PrimitivePatch/PrimitivePatchPointAddressing.C (calcPointFaces)
//   Upstream copyright: (C) 2015-2022 EPFL; (C) 2011-2016 OpenFOAM Foundation, 2017-2025 OpenCFD Ltd.
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `deformMesh`: thermo-mechanical displacement moves a region's mesh (gh:#1784)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## What upstream does (and where `movePoints` is NOT)
//!
//! `multiRegion/meshHandler` contains no `movePoints`. The deformation is `deformMesh()` of each region
//! solver, the same five lines in `neutronics.C`, `thermalHydraulicsModel.C` and `thermoMechanics.C`:
//!
//! ```text
//! if (couplingDict.found("meshDeformation") && region listed in it)
//!     pointDisp = volPointInterpolation::New(mesh_).interpolate(<displacementField>);
//!     mesh_.movePoints(originalPoints_ + pointDisp.internalField());
//! ```
//!
//! | upstream | here |
//! |---|---|
//! | `multiRegionCouplingDict/meshDeformation { <region> { displacementField <f>; } }` | [`MeshDeformationDict`] |
//! | `originalPoints_(mesh_.points())` | [`DeformableMesh::original_points`] (the points as read) |
//! | `volPointInterpolation::makeWeights` (`makeInternalWeights`, `makeBoundaryWeights`) | [`VolPointInterpolation::build`] |
//! | `volPointInterpolation::interpolate` (`interpolateInternalField`, `interpolateBoundaryField`) | [`VolPointInterpolation::interpolate`] |
//! | `fvMesh::movePoints` (`primitiveMesh::updateGeom`) | [`DeformableMesh::move_points`] |
//! | `deformMesh()` | [`DeformableMesh::deform_mesh`] |
//! | the `disp` volVectorField and its `correctBoundaryConditions()` | [`VolDisplacement`] |
//!
//! The weights are rebuilt from the CURRENT geometry before every interpolation, as upstream does: `volPointInterpolation` is a
//! `MoveableMeshObject`, so `fvMesh::movePoints` calls its `movePoints()`, which calls `makeWeights()` again. The points always
//! restart from `originalPoints_`, so the displacement is not cumulative.
//!
//! ## `pointCells` order (the summation order of the weights)
//!
//! ESI builds `primitiveMesh::pointCells` three ways depending on what is cached: inverted `cellPoints`, a walk over `pointFaces`, or
//! a sweep over the cells. A freshly read `fvMesh` takes the cell sweep (ascending cell index per point), measured on the
//! real binary in gh:#1784 round 26; that is [`PointCellsOrder::CellAscending`], the default. The point-face walk is kept as
//! [`PointCellsOrder::PointFaceWalk`] because a different cache state upstream would give it, and it changes the last bit.
//!
//! ## Refused, by name (nothing here is guessed)
//!
//! `symmetry`/`symmetryPlane`/`wedge`/`empty`/`cyclic`/`cyclicAMI`/`processor` patches (point constraints and coupled boundaries are not
//! ported), a displacement patch condition other than `zeroGradient` or `fixedValue`, any key but `displacementField` in a
//! `meshDeformation` entry, and `removeBaffles` on a deformed region (upstream moves the original mesh only, the baffle-less copy
//! keeps its points). `fvMesh::movePoints` also sets `meshPhi` and the old-time volumes `V0`/`V00`, which `ddt` of a moving mesh
//! consumes in the thermal-hydraulics solver: NOT ported (gh:#1846).
//!
//! ## Units
//!
//! Points, displacements and volumes are plain `f64` metres, as in `outram-foam-basic-lib`'s mesh.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::io::dict::FoamDict;
use outram_foam_basic_lib::io::FoamEntry;
use outram_foam_basic_lib::io::FoamValue;
use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind, Vector3};

use crate::io::poly_mesh::{compute_cell_geometry, compute_face_geometry};

/// Errors of the mesh-deformation port.
#[derive(Debug, Clone, PartialEq)]
pub enum MeshDeformationError {
    /// An upstream option or mesh feature this port does not implement (named).
    Refused {
        /// What was asked for.
        what: String,
        /// Why it is not available.
        why: String,
    },
    /// Inconsistent input (sizes, missing entries).
    Invalid(String),
}

impl core::fmt::Display for MeshDeformationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MeshDeformationError::Refused { what, why } => write!(f, "mesh deformation: `{what}` is not ported: {why}"),
            MeshDeformationError::Invalid(m) => write!(f, "mesh deformation: {m}"),
        }
    }
}

fn refused(what: impl Into<String>, why: impl Into<String>) -> MeshDeformationError {
    MeshDeformationError::Refused { what: what.into(), why: why.into() }
}

/// How `primitiveMesh::pointCells` is ordered, which fixes the order of the inverse-distance sums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointCellsOrder {
    /// `calcPointCells`, cell-sweep branch: the cells of a point in ascending cell index. What a freshly read `fvMesh` gets
    /// (measured, gh:#1784 round 26). The default.
    CellAscending,
    /// `calcPointCells`, `hasPointFaces()` branch: walk `pointFaces` (ascending face index), owner then neighbour per face, each
    /// cell once. What upstream would give if `pointFaces` were cached first.
    PointFaceWalk,
}

/// `multiRegionCouplingDict/meshDeformation`: the regions that deform and the name of their displacement field.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshDeformationDict {
    /// `(region, displacementField)` in dictionary order.
    pub regions: Vec<(String, String)>,
}

impl MeshDeformationDict {
    /// Read `meshDeformation` from a parsed `multiRegionCouplingDict`. No `meshDeformation` entry means no region deforms (upstream's
    /// `couplingDict.found("meshDeformation")`).
    ///
    /// # Errors
    /// [`MeshDeformationError::Refused`] for a key other than `displacementField` in a region entry;
    /// [`MeshDeformationError::Invalid`] for a malformed entry.
    pub fn from_coupling_dict(d: &FoamDict) -> Result<Self, MeshDeformationError> {
        let mut regions = Vec::new();
        let Some(md) = d.get_dict("meshDeformation") else {
            return Ok(MeshDeformationDict { regions });
        };
        for (region, e) in md.iter() {
            let FoamEntry::SubDict(rd) = e else {
                return Err(MeshDeformationError::Invalid(format!("meshDeformation.{region} is not a dictionary")));
            };
            let mut field = None;
            for (k, v) in rd.iter() {
                if k == "displacementField" {
                    field = match v {
                        FoamEntry::Word(w) => Some(w.clone()),
                        _ => return Err(MeshDeformationError::Invalid(format!("meshDeformation.{region}.displacementField is not a word"))),
                    };
                } else {
                    return Err(refused(format!("meshDeformation.{region}.{k}"), "upstream reads only `displacementField`"));
                }
            }
            let field = field.ok_or_else(|| MeshDeformationError::Invalid(format!("meshDeformation.{region}: no displacementField")))?;
            regions.push((region.to_string(), field));
        }
        Ok(MeshDeformationDict { regions })
    }

    /// The displacement field of `region`, or `None` when it does not deform.
    pub fn displacement_field(&self, region: &str) -> Option<&str> {
        self.regions.iter().find(|(r, _)| r == region).map(|(_, f)| f.as_str())
    }

    /// Ablation: no region deforms (the named, visible switch; deformation is on whenever the dictionary lists the region).
    pub fn without_deformation(mut self) -> Self {
        self.regions.clear();
        self
    }
}

/// A patch condition of the displacement `volVectorField` (`correctBoundaryConditions()`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DispPatchCondition {
    /// `zeroGradient`: the boundary value is the adjacent cell value.
    ZeroGradient,
    /// `fixedValue` with a uniform value.
    FixedValue(Vector3),
}

/// Read the patch conditions of a displacement field file (`boundaryField`, one entry per mesh patch, in patch order).
///
/// # Errors
/// [`MeshDeformationError::Refused`] for a condition other than `zeroGradient` / `fixedValue` (named);
/// [`MeshDeformationError::Invalid`] for a missing patch or a non-uniform or malformed value.
pub fn parse_disp_patch_conditions(
    field: &FoamDict,
    patches: &[BoundaryPatch],
) -> Result<Vec<DispPatchCondition>, MeshDeformationError> {
    let bf = field.get_dict("boundaryField").ok_or_else(|| MeshDeformationError::Invalid("no boundaryField".into()))?;
    let mut out = Vec::new();
    for p in patches {
        let Some(FoamEntry::SubDict(d)) = bf.get(&p.name) else {
            return Err(MeshDeformationError::Invalid(format!("no boundaryField entry for patch `{}`", p.name)));
        };
        let ty = match d.get("type") {
            Some(FoamEntry::Word(w)) => w.as_str(),
            _ => return Err(MeshDeformationError::Invalid(format!("patch `{}`: no type", p.name))),
        };
        out.push(match ty {
            "zeroGradient" => DispPatchCondition::ZeroGradient,
            "fixedValue" => {
                let mut words = Vec::new();
                let mut nums = Vec::new();
                match d.get("value") {
                    Some(FoamEntry::Tokens(v)) | Some(FoamEntry::List(v)) => flatten(v, &mut words, &mut nums),
                    _ => return Err(MeshDeformationError::Invalid(format!("patch `{}`: no value", p.name))),
                }
                if words.first().map(String::as_str) != Some("uniform") || nums.len() != 3 {
                    return Err(refused(format!("patch `{}` fixedValue value", p.name), "only `uniform (x y z)`"));
                }
                DispPatchCondition::FixedValue(Vector3::new(nums[0], nums[1], nums[2]))
            }
            other => return Err(refused(format!("patch `{}` type {other}", p.name), "only zeroGradient and fixedValue")),
        });
    }
    Ok(out)
}

fn flatten(v: &[FoamValue], words: &mut Vec<String>, nums: &mut Vec<f64>) {
    for x in v {
        match x {
            FoamValue::Word(w) => words.push(w.clone()),
            FoamValue::Scalar(s) => nums.push(*s),
            FoamValue::List(l) => flatten(l, words, nums),
            _ => {}
        }
    }
}

/// The displacement `volVectorField`: cell values and the per-boundary-face values `correctBoundaryConditions()` leaves.
#[derive(Debug, Clone, PartialEq)]
pub struct VolDisplacement {
    /// One value per cell, metres.
    pub cells: Vec<Vector3>,
    /// One value per boundary face (patch order, as the mesh's boundary faces), metres.
    pub boundary: Vec<Vector3>,
}

impl VolDisplacement {
    /// Cell values plus `correctBoundaryConditions()` of the given patch conditions.
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] when the sizes do not match the mesh.
    pub fn from_cells(
        mesh: &DeformableMesh,
        cells: Vec<Vector3>,
        conditions: &[DispPatchCondition],
    ) -> Result<Self, MeshDeformationError> {
        let t = &mesh.topology;
        if cells.len() != t.n_cells || conditions.len() != t.patches.len() {
            return Err(MeshDeformationError::Invalid("displacement size does not match the mesh".into()));
        }
        let mut boundary = vec![Vector3::ZERO; t.faces.len() - t.n_internal];
        for (p, c) in t.patches.iter().zip(conditions) {
            for f in p.start..p.start + p.size {
                boundary[f - t.n_internal] = match c {
                    DispPatchCondition::ZeroGradient => cells[t.owner[f]],
                    DispPatchCondition::FixedValue(v) => *v,
                };
            }
        }
        Ok(VolDisplacement { cells, boundary })
    }
}

/// Mesh connectivity (never changes under point motion).
#[derive(Debug, Clone)]
pub struct MeshTopology {
    faces: Vec<Vec<usize>>,
    owner: Vec<usize>,
    neighbour: Vec<usize>,
    patches: Vec<BoundaryPatch>,
    n_cells: usize,
    n_internal: usize,
    n_points: usize,
    point_cells: Vec<Vec<usize>>,
    /// `primitivePatch::meshPoints()` of the boundary faces (order of first appearance).
    boundary_points: Vec<usize>,
    /// `primitivePatch::pointFaces()`: per boundary point, boundary-local face indices (ascending).
    boundary_point_faces: Vec<Vec<usize>>,
    /// Per mesh point, its index in `boundary_points`, or `usize::MAX`.
    boundary_slot: Vec<usize>,
}

impl MeshTopology {
    /// `polyMesh::faces()`: the point labels of every face.
    pub fn faces(&self) -> &[Vec<usize>] {
        &self.faces
    }

    /// `polyMesh::faceOwner()`.
    pub fn owner(&self) -> &[usize] {
        &self.owner
    }

    /// `polyMesh::faceNeighbour()`.
    pub fn neighbour(&self) -> &[usize] {
        &self.neighbour
    }

    /// `primitiveMesh::pointCells`.
    pub fn point_cells(&self) -> &[Vec<usize>] {
        &self.point_cells
    }
}

fn build_point_cells(
    order: PointCellsOrder,
    faces: &[Vec<usize>],
    owner: &[usize],
    neighbour: &[usize],
    n_cells: usize,
    n_points: usize,
) -> Vec<Vec<usize>> {
    let mut pc: Vec<Vec<usize>> = vec![Vec::new(); n_points];
    match order {
        PointCellsOrder::CellAscending => {
            // `calcPointCells`, third branch: for each cell, each of its points once. A cell's faces are the faces it owns and
            // neighbours; only the SET of points per cell matters for the (ascending) cell order.
            let mut cell_faces: Vec<Vec<usize>> = vec![Vec::new(); n_cells];
            for (f, &o) in owner.iter().enumerate() {
                cell_faces[o].push(f);
            }
            for (f, &n) in neighbour.iter().enumerate() {
                cell_faces[n].push(f);
            }
            let mut used = vec![usize::MAX; n_points];
            for (c, cf) in cell_faces.iter().enumerate() {
                for &f in cf {
                    for &p in &faces[f] {
                        if used[p] != c {
                            used[p] = c;
                            pc[p].push(c);
                        }
                    }
                }
            }
        }
        PointCellsOrder::PointFaceWalk => {
            // `calcPointCells`, `hasPointFaces()` branch: pointFaces ascending; owner then neighbour per face, each cell once.
            let mut point_faces: Vec<Vec<usize>> = vec![Vec::new(); n_points];
            for (f, fp) in faces.iter().enumerate() {
                for &p in fp {
                    point_faces[p].push(f);
                }
            }
            for p in 0..n_points {
                for &f in &point_faces[p] {
                    let o = owner[f];
                    if !pc[p].contains(&o) {
                        pc[p].push(o);
                    }
                    if f < neighbour.len() {
                        let n = neighbour[f];
                        if !pc[p].contains(&n) {
                            pc[p].push(n);
                        }
                    }
                }
            }
        }
    }
    pc
}

/// `volPointInterpolation`: the inverse-distance weights for the current geometry.
#[derive(Debug, Clone)]
pub struct VolPointInterpolation {
    /// `pointWeights_`: per mesh point (empty for a patch point), normalised, in `pointCells` order.
    point_weights: Vec<Vec<f64>>,
    /// `boundaryPointWeights_`: per boundary point, normalised, in `pointFaces` order.
    boundary_point_weights: Vec<Vec<f64>>,
}

fn sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

impl VolPointInterpolation {
    /// `makeWeights()` for `points` with the face and cell centres of the geometry they belong to.
    /// Every boundary face is a patch face (empty and coupled patches are refused when the mesh is built).
    pub fn build(
        topology: &MeshTopology,
        points: &[Vector3],
        face_centres: &[Vector3],
        cell_centres: &[Vector3],
    ) -> Self {
        let n_points = topology.n_points;
        let mut sum_weights = vec![0.0_f64; n_points];
        // makeInternalWeights: points not on a patch
        let mut point_weights: Vec<Vec<f64>> = vec![Vec::new(); n_points];
        for p in 0..n_points {
            if topology.boundary_slot[p] == usize::MAX {
                let pcp = &topology.point_cells[p];
                let mut pw = vec![0.0_f64; pcp.len()];
                for (i, &c) in pcp.iter().enumerate() {
                    pw[i] = 1.0 / sub(points[p], cell_centres[c]).mag();
                    sum_weights[p] += pw[i];
                }
                point_weights[p] = pw;
            }
        }
        // makeBoundaryWeights: patch points override sumWeights
        let mut boundary_point_weights: Vec<Vec<f64>> = vec![Vec::new(); topology.boundary_points.len()];
        for (i, &p) in topology.boundary_points.iter().enumerate() {
            let pfaces = &topology.boundary_point_faces[i];
            let mut pw = vec![0.0_f64; pfaces.len()];
            sum_weights[p] = 0.0;
            for (j, &bf) in pfaces.iter().enumerate() {
                let facei = topology.n_internal + bf;
                pw[j] = 1.0 / sub(points[p], face_centres[facei]).mag();
                sum_weights[p] += pw[j];
            }
            boundary_point_weights[i] = pw;
        }
        // normalise
        for p in 0..n_points {
            for w in point_weights[p].iter_mut() {
                *w /= sum_weights[p];
            }
        }
        for (i, &p) in topology.boundary_points.iter().enumerate() {
            for w in boundary_point_weights[i].iter_mut() {
                *w /= sum_weights[p];
            }
        }
        VolPointInterpolation { point_weights, boundary_point_weights }
    }

    /// `interpolate(vf)`: `interpolateInternalField` for the non-patch points, `interpolateBoundaryField` for the patch points.
    pub fn interpolate(&self, topology: &MeshTopology, disp: &VolDisplacement) -> Vec<Vector3> {
        let mut pf = vec![Vector3::ZERO; topology.n_points];
        for p in 0..topology.n_points {
            if topology.boundary_slot[p] == usize::MAX {
                let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
                for (i, &c) in topology.point_cells[p].iter().enumerate() {
                    let w = self.point_weights[p][i];
                    let v = disp.cells[c];
                    x += w * v.x;
                    y += w * v.y;
                    z += w * v.z;
                }
                pf[p] = Vector3::new(x, y, z);
            }
        }
        for (i, &p) in topology.boundary_points.iter().enumerate() {
            let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
            for (j, &bf) in topology.boundary_point_faces[i].iter().enumerate() {
                let w = self.boundary_point_weights[i][j];
                let v = disp.boundary[bf];
                x += w * v.x;
                y += w * v.y;
                z += w * v.z;
            }
            pf[p] = Vector3::new(x, y, z);
        }
        pf
    }
}

/// A region's mesh that `deformMesh()` can move: the points as read (`originalPoints_`), the current points and the geometry
/// `fvMesh::movePoints` leaves behind.
#[derive(Debug, Clone)]
pub struct DeformableMesh {
    topology: Arc<MeshTopology>,
    original_points: Vec<Vector3>,
    points: Vec<Vector3>,
    mesh: Arc<FvMesh>,
    face_centres: Vec<Vector3>,
}

impl DeformableMesh {
    /// Build from raw `polyMesh` lists (`points`, `faces`, `owner`, `neighbour`, `boundary`).
    ///
    /// # Errors
    /// [`MeshDeformationError::Refused`] for a symmetry, wedge, empty, cyclic or processor patch;
    /// [`MeshDeformationError::Invalid`] for inconsistent lists.
    pub fn new(
        points: Vec<Vector3>,
        faces: Vec<Vec<usize>>,
        owner: Vec<usize>,
        neighbour: Vec<usize>,
        patches: Vec<BoundaryPatch>,
        order: PointCellsOrder,
    ) -> Result<Self, MeshDeformationError> {
        for p in &patches {
            match p.kind {
                PatchKind::Patch | PatchKind::Wall => {}
                other => {
                    return Err(refused(
                        format!("patch `{}` of kind {other:?}", p.name),
                        "point constraints and coupled boundaries of volPointInterpolation are not ported",
                    ))
                }
            }
        }
        if owner.len() != faces.len() || neighbour.len() > faces.len() {
            return Err(MeshDeformationError::Invalid("owner/neighbour sizes do not match the faces".into()));
        }
        let n_internal = neighbour.len();
        let n_cells = owner.iter().chain(neighbour.iter()).copied().max().map_or(0, |m| m + 1);
        let n_points = points.len();
        if faces.iter().any(|f| f.iter().any(|&p| p >= n_points)) {
            return Err(MeshDeformationError::Invalid("a face names a point that does not exist".into()));
        }
        let point_cells = build_point_cells(order, &faces, &owner, &neighbour, n_cells, n_points);
        // the boundary primitivePatch: meshPoints in order of first appearance, pointFaces ascending
        let mut boundary_slot = vec![usize::MAX; n_points];
        let mut boundary_points = Vec::new();
        let mut boundary_point_faces: Vec<Vec<usize>> = Vec::new();
        for (bf, f) in faces[n_internal..].iter().enumerate() {
            for &p in f {
                if boundary_slot[p] == usize::MAX {
                    boundary_slot[p] = boundary_points.len();
                    boundary_points.push(p);
                    boundary_point_faces.push(Vec::new());
                }
                boundary_point_faces[boundary_slot[p]].push(bf);
            }
        }
        let topology = Arc::new(MeshTopology {
            faces,
            owner,
            neighbour,
            patches,
            n_cells,
            n_internal,
            n_points,
            point_cells,
            boundary_points,
            boundary_point_faces,
            boundary_slot,
        });
        let (mesh, face_centres) = Self::geometry(&topology, &points)?;
        Ok(DeformableMesh { topology, original_points: points.clone(), points, mesh, face_centres })
    }

    /// Read `constant/<region>/polyMesh` (std). Refuses what [`DeformableMesh::new`] refuses, and by raw patch `type` name
    /// (`cyclicAMI`, `cyclicSlip`, ... would otherwise read as plain patches).
    ///
    /// # Errors
    /// As [`DeformableMesh::new`], plus file errors as [`MeshDeformationError::Invalid`].
    #[cfg(feature = "std")]
    pub fn read(poly_mesh_dir: &std::path::Path, order: PointCellsOrder) -> Result<Self, MeshDeformationError> {
        use crate::io::poly_mesh::{parse_boundary, parse_faces, parse_index_list, parse_points};
        let read = |name: &str| {
            std::fs::read_to_string(poly_mesh_dir.join(name)).map_err(|e| MeshDeformationError::Invalid(format!("{name}: {e}")))
        };
        let e = |x: crate::error::AppBuilderError| MeshDeformationError::Invalid(x.to_string());
        let btxt = read("boundary")?;
        let toks: Vec<&str> = btxt.split(|c: char| c.is_whitespace() || c == ';').filter(|t| !t.is_empty()).collect();
        for w in toks.windows(2) {
            if w[0] == "type" && !matches!(w[1], "patch" | "wall" | "mappedWall") {
                return Err(refused(format!("polyPatch type {}", w[1]), "only patch, wall and mappedWall"));
            }
        }
        let points = parse_points(&read("points")?, "points").map_err(e)?;
        let faces = parse_faces(&read("faces")?, "faces").map_err(e)?;
        let owner = parse_index_list(&read("owner")?, "owner").map_err(e)?;
        let neighbour = parse_index_list(&read("neighbour")?, "neighbour").map_err(e)?;
        let patches = parse_boundary(&btxt, "boundary").map_err(e)?;
        let points = points.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect();
        Self::new(points, faces, owner, neighbour, patches, order)
    }

    fn geometry(t: &MeshTopology, points: &[Vector3]) -> Result<(Arc<FvMesh>, Vec<Vector3>), MeshDeformationError> {
        let raw: Vec<[f64; 3]> = points.iter().map(|p| [p.x, p.y, p.z]).collect();
        let (face_centres, face_area_vectors, face_areas) =
            compute_face_geometry(&raw, &t.faces, "faces").map_err(|x| MeshDeformationError::Invalid(x.to_string()))?;
        let (cell_volumes, cell_centres) =
            compute_cell_geometry(&face_centres, &face_area_vectors, &t.owner, &t.neighbour, t.n_cells);
        let mesh = FvMeshBuilder::new()
            .n_cells(t.n_cells)
            .n_internal_faces(t.n_internal)
            .owner(t.owner.clone())
            .neighbour(t.neighbour.clone())
            .patches(t.patches.clone())
            .cell_volumes(cell_volumes)
            .cell_centres(cell_centres)
            .face_area_vectors(face_area_vectors)
            .face_areas(face_areas)
            .face_centres(face_centres.clone())
            .build()
            .map_err(|x| MeshDeformationError::Invalid(x.to_string()))?;
        Ok((Arc::new(mesh), face_centres))
    }

    /// `originalPoints_`.
    pub fn original_points(&self) -> &[Vector3] {
        &self.original_points
    }

    /// The current points.
    pub fn points(&self) -> &[Vector3] {
        &self.points
    }

    /// The connectivity.
    pub fn topology(&self) -> &MeshTopology {
        &self.topology
    }

    /// The geometry of the current points (face centres, face area vectors, cell centres, cell volumes).
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// `volPointInterpolation::makeWeights()` for the CURRENT geometry.
    pub fn weights(&self) -> VolPointInterpolation {
        VolPointInterpolation::build(&self.topology, &self.points, &self.face_centres, &self.mesh.cell_centres)
    }

    /// `volPointInterpolation::interpolate(disp)` with the weights of the current geometry.
    pub fn interpolate_displacement(&self, disp: &VolDisplacement) -> Vec<Vector3> {
        self.weights().interpolate(&self.topology, disp)
    }

    /// `fvMesh::movePoints(p)`: replace the points and recompute the geometry (`primitiveMesh::updateGeom`).
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] if `new_points` has the wrong size.
    pub fn move_points(&mut self, new_points: Vec<Vector3>) -> Result<(), MeshDeformationError> {
        if new_points.len() != self.points.len() {
            return Err(MeshDeformationError::Invalid(format!(
                "Size of newPoints {} does not correspond to current mesh points size {}",
                new_points.len(),
                self.points.len()
            )));
        }
        let (mesh, face_centres) = Self::geometry(&self.topology, &new_points)?;
        self.points = new_points;
        self.mesh = mesh;
        self.face_centres = face_centres;
        Ok(())
    }

    /// `deformMesh()`: `movePoints(originalPoints_ + volPointInterpolation::interpolate(disp).internalField())`.
    /// Returns the interpolated point displacement.
    ///
    /// # Errors
    /// As [`DeformableMesh::move_points`].
    pub fn deform_mesh(&mut self, disp: &VolDisplacement) -> Result<Vec<Vector3>, MeshDeformationError> {
        let pd = self.interpolate_displacement(disp);
        let moved = self
            .original_points
            .iter()
            .zip(&pd)
            .map(|(o, d)| Vector3::new(o.x + d.x, o.y + d.y, o.z + d.z))
            .collect();
        self.move_points(moved)?;
        Ok(pd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::io::dict::FoamFile;

    /// An `nx x ny x nz` unit-cell hex block, internal faces first, boundary faces grouped by six patches
    /// (`xmin xmax ymin ymax zmin zmax`), faces wound outward from the owner.
    fn block(nx: usize, ny: usize, nz: usize, kinds: [PatchKind; 6]) -> (Vec<Vector3>, Vec<Vec<usize>>, Vec<usize>, Vec<usize>, Vec<BoundaryPatch>) {
        let pid = |i: usize, j: usize, k: usize| i + (nx + 1) * (j + (ny + 1) * k);
        let cid = |i: usize, j: usize, k: usize| i + nx * (j + ny * k);
        let mut points = Vec::new();
        for k in 0..=nz {
            for j in 0..=ny {
                for i in 0..=nx {
                    points.push(Vector3::new(i as f64, j as f64, k as f64));
                }
            }
        }
        let (mut faces, mut owner, mut neighbour) = (Vec::new(), Vec::new(), Vec::new());
        for k in 0..nz {
            for j in 0..ny {
                for i in 0..nx {
                    let c = cid(i, j, k);
                    if i + 1 < nx {
                        faces.push(vec![pid(i + 1, j, k), pid(i + 1, j + 1, k), pid(i + 1, j + 1, k + 1), pid(i + 1, j, k + 1)]);
                        owner.push(c);
                        neighbour.push(cid(i + 1, j, k));
                    }
                    if j + 1 < ny {
                        faces.push(vec![pid(i, j + 1, k), pid(i, j + 1, k + 1), pid(i + 1, j + 1, k + 1), pid(i + 1, j + 1, k)]);
                        owner.push(c);
                        neighbour.push(cid(i, j + 1, k));
                    }
                    if k + 1 < nz {
                        faces.push(vec![pid(i, j, k + 1), pid(i + 1, j, k + 1), pid(i + 1, j + 1, k + 1), pid(i, j + 1, k + 1)]);
                        owner.push(c);
                        neighbour.push(cid(i, j, k + 1));
                    }
                }
            }
        }
        let n_internal = faces.len();
        let mut patches = Vec::new();
        let names = ["xmin", "xmax", "ymin", "ymax", "zmin", "zmax"];
        for (s, name) in names.iter().enumerate() {
            let start = faces.len();
            let (a, b) = match s {
                0 | 1 => (ny, nz),
                2 | 3 => (nx, nz),
                _ => (nx, ny),
            };
            for q in 0..b {
                for p in 0..a {
                    let (f, c) = match s {
                        0 => (vec![pid(0, p, q), pid(0, p, q + 1), pid(0, p + 1, q + 1), pid(0, p + 1, q)], cid(0, p, q)),
                        1 => (vec![pid(nx, p, q), pid(nx, p + 1, q), pid(nx, p + 1, q + 1), pid(nx, p, q + 1)], cid(nx - 1, p, q)),
                        2 => (vec![pid(p, 0, q), pid(p + 1, 0, q), pid(p + 1, 0, q + 1), pid(p, 0, q + 1)], cid(p, 0, q)),
                        3 => (vec![pid(p, ny, q), pid(p, ny, q + 1), pid(p + 1, ny, q + 1), pid(p + 1, ny, q)], cid(p, ny - 1, q)),
                        4 => (vec![pid(p, q, 0), pid(p, q + 1, 0), pid(p + 1, q + 1, 0), pid(p + 1, q, 0)], cid(p, q, 0)),
                        _ => (vec![pid(p, q, nz), pid(p + 1, q, nz), pid(p + 1, q + 1, nz), pid(p, q + 1, nz)], cid(p, q, nz - 1)),
                    };
                    faces.push(f);
                    owner.push(c);
                }
            }
            patches.push(BoundaryPatch::new((*name).to_string(), start, faces.len() - start, kinds[s]));
        }
        let _ = n_internal;
        (points, faces, owner, neighbour, patches)
    }

    fn plain() -> [PatchKind; 6] {
        [PatchKind::Patch; 6]
    }

    fn mesh(n: usize, order: PointCellsOrder) -> DeformableMesh {
        let (p, f, o, nb, pa) = block(n, n, n, plain());
        DeformableMesh::new(p, f, o, nb, pa, order).expect("block")
    }

    fn volume(m: &DeformableMesh) -> f64 {
        m.mesh().cell_volumes.iter().sum()
    }

    #[test]
    fn uniform_displacement_translates_every_point_and_keeps_volumes() {
        let mut m = mesh(3, PointCellsOrder::CellAscending);
        let v = Vector3::new(0.25, -0.5, 0.125);
        let c = vec![DispPatchCondition::ZeroGradient; 6];
        let d = VolDisplacement::from_cells(&m, vec![v; 27], &c).unwrap();
        let v0 = volume(&m);
        let pd = m.deform_mesh(&d).unwrap();
        for p in &pd {
            assert!((p.x - v.x).abs() < 1e-15 && (p.y - v.y).abs() < 1e-15 && (p.z - v.z).abs() < 1e-15, "{p:?}");
        }
        assert!((volume(&m) - v0).abs() < 1e-12);
        assert!((m.points()[0].x - (0.0 + pd[0].x)).abs() == 0.0);
    }

    #[test]
    fn expansion_field_grows_the_volume_and_is_not_cumulative() {
        let mut m = mesh(4, PointCellsOrder::CellAscending);
        let c = vec![DispPatchCondition::ZeroGradient; 6];
        // a dilatation about the centre (1.5? the block is 4 wide: centre 2): d = 0.01 (x - 2)
        let centres = m.mesh().cell_centres.clone();
        let cells = centres.iter().map(|x| Vector3::new(0.01 * (x.x - 2.0), 0.01 * (x.y - 2.0), 0.01 * (x.z - 2.0))).collect();
        let d = VolDisplacement::from_cells(&m, cells, &c).unwrap();
        let v0 = volume(&m);
        m.deform_mesh(&d).unwrap();
        let v1 = volume(&m);
        assert!(v1 > v0, "{v0} -> {v1}");
        // the same field again moves from the ORIGINAL points: points change only through the re-weighting, not by doubling
        let p1 = m.points().to_vec();
        m.deform_mesh(&d).unwrap();
        let moved = m.points().iter().zip(&p1).map(|(a, b)| (a.x - b.x).abs()).fold(0.0_f64, f64::max);
        assert!(moved < 1e-2 * 0.5, "second deformation must not accumulate: {moved}");
    }

    #[test]
    fn both_pointcells_orders_hold_the_same_cells_and_agree_to_round_off() {
        let a = mesh(3, PointCellsOrder::CellAscending);
        let b = mesh(3, PointCellsOrder::PointFaceWalk);
        for (x, y) in a.topology().point_cells().iter().zip(b.topology().point_cells()) {
            let (mut x, mut y) = (x.clone(), y.clone());
            x.sort_unstable();
            y.sort_unstable();
            assert_eq!(x, y);
        }
        let c = vec![DispPatchCondition::FixedValue(Vector3::new(0.1, 0.0, 0.0)); 6];
        let cells = a.mesh().cell_centres.iter().map(|x| Vector3::new(0.01 * x.y, 0.02 * x.z, 0.005 * x.x)).collect::<Vec<_>>();
        let da = VolDisplacement::from_cells(&a, cells.clone(), &c).unwrap();
        let pa = a.interpolate_displacement(&da);
        let pb = b.interpolate_displacement(&da);
        for (p, q) in pa.iter().zip(&pb) {
            assert!((p.x - q.x).abs() < 1e-15 && (p.y - q.y).abs() < 1e-15 && (p.z - q.z).abs() < 1e-15);
        }
    }

    #[test]
    fn stale_weights_differ_from_the_weights_of_the_moved_geometry() {
        // the control of the differential test: weights of the ORIGINAL geometry used after the mesh moved
        let mut m = mesh(4, PointCellsOrder::CellAscending);
        let c = vec![DispPatchCondition::ZeroGradient; 6];
        let cells: Vec<Vector3> = m.mesh().cell_centres.iter().map(|x| Vector3::new(0.05 * x.x * x.y, 0.0, 0.03 * x.z)).collect();
        let d = VolDisplacement::from_cells(&m, cells, &c).unwrap();
        let stale = m.weights();
        m.deform_mesh(&d).unwrap();
        let fresh = m.interpolate_displacement(&d);
        let old = stale.interpolate(m.topology(), &d);
        let worst = fresh.iter().zip(&old).map(|(a, b)| (a.x - b.x).abs() + (a.z - b.z).abs()).fold(0.0_f64, f64::max);
        assert!(worst > 1e-6, "re-weighting must matter: {worst}");
    }

    #[test]
    fn refuses_constraint_patches_by_name() {
        let mut k = plain();
        k[4] = PatchKind::Symmetry;
        let (p, f, o, nb, pa) = block(2, 2, 2, k);
        let e = DeformableMesh::new(p, f, o, nb, pa, PointCellsOrder::CellAscending).unwrap_err();
        assert!(matches!(e, MeshDeformationError::Refused { .. }), "{e}");
        assert!(e.to_string().contains("zmin"), "{e}");
    }

    #[test]
    fn dictionary_reader_and_ablation() {
        let ok = FoamFile::parse("meshDeformation { neutroRegion { displacementField disp; } }").unwrap().dict;
        let d = MeshDeformationDict::from_coupling_dict(&ok).unwrap();
        assert_eq!(d.displacement_field("neutroRegion"), Some("disp"));
        assert_eq!(d.displacement_field("fluidRegion"), None);
        assert!(d.clone().without_deformation().displacement_field("neutroRegion").is_none());
        let none = FoamFile::parse("mappings { }").unwrap().dict;
        assert!(MeshDeformationDict::from_coupling_dict(&none).unwrap().regions.is_empty());
        let bad = FoamFile::parse("meshDeformation { neutroRegion { displacementField disp; scale 2; } }").unwrap().dict;
        assert!(matches!(MeshDeformationDict::from_coupling_dict(&bad), Err(MeshDeformationError::Refused { .. })));
    }

    #[test]
    fn patch_conditions_are_read_and_unported_ones_refused() {
        let (_, _, _, _, pa) = block(1, 1, 1, plain());
        let mut txt = String::from("boundaryField {");
        for (i, p) in pa.iter().enumerate() {
            if i == 0 {
                txt.push_str(&format!("{} {{ type fixedValue; value uniform (1 2 3); }} ", p.name));
            } else {
                txt.push_str(&format!("{} {{ type zeroGradient; }} ", p.name));
            }
        }
        txt.push('}');
        let f = FoamFile::parse(&txt).unwrap().dict;
        let c = parse_disp_patch_conditions(&f, &pa).unwrap();
        assert_eq!(c[0], DispPatchCondition::FixedValue(Vector3::new(1.0, 2.0, 3.0)));
        assert_eq!(c[1], DispPatchCondition::ZeroGradient);
        let g = FoamFile::parse(&txt.replace("zeroGradient", "GFTractionDisplacement")).unwrap().dict;
        assert!(matches!(parse_disp_patch_conditions(&g, &pa), Err(MeshDeformationError::Refused { .. })));
    }

    #[test]
    fn move_points_refuses_a_wrong_size() {
        let mut m = mesh(2, PointCellsOrder::CellAscending);
        assert!(m.move_points(vec![Vector3::ZERO; 3]).is_err());
    }
}
