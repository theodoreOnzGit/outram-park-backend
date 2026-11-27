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
//! `symmetry`/`symmetryPlane`/`wedge`/~~`empty`~~/`cyclic`/`cyclicAMI`/`processor` patches (point constraints and coupled boundaries are not
//! ported; **CORRECTED 2026-11-27 (gh:#1856, README round 56): `empty` IS ported**, it is not a patch of `volPointInterpolation` and has no point constraint, and the 2D `neutroRegion` of `2D_fullCoupling` moves bit-identically to the real binary), a displacement patch condition other than `zeroGradient` or `fixedValue`, any key but `displacementField` in a
//! `meshDeformation` entry, and `removeBaffles` on a deformed region (upstream moves the original mesh only, the baffle-less copy
//! keeps its points). ~~`fvMesh::movePoints` also sets `meshPhi` and the old-time volumes `V0`/`V00`, which `ddt` of a moving mesh
//! consumes in the thermal-hydraulics solver: NOT ported (gh:#1846).~~ **CORRECTED 2026-11-26 (gh:#1852, round 30):** ported,
//! bit-identical to ESI v2506: [`DeformableMesh::move_points_at`] / [`DeformableMesh::deform_mesh_at`] keep `moving()`,
//! `oldPoints`, `V0`, `V00` (lazy, as `fvMesh::V00()`) and `meshPhi` (reusing `outram_foam_mesh::dynamic_mesh::mesh_phi`), and
//! [`DeformableMesh::euler_fvm_ddt`] is the moving-mesh `EulerDdtScheme::fvmDdt` that consumes `V` and `V0`. GeN-Foam itself
//! references none of them (no `meshPhi`, `makeRelative`, `V0` or `V00` in `src/`); `ddt` is their only consumer. The older
//! [`DeformableMesh::move_points`] / [`DeformableMesh::deform_mesh`] are the time-less, geometry-only form of round 26.
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
    /// The displacement of a `volVectorField` as `deformMesh()` reads it: the cell values and, patch after patch, the boundary
    /// values the field holds (after `correctBoundaryConditions()`).
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] never today; reserved for size checks against a mesh in [`DeformableMesh::deform_mesh_at`].
    pub fn from_vol_field(field: &outram_foam_basic_lib::fields::vol_field::VolVectorField) -> Result<Self, MeshDeformationError> {
        let cells = field.internal.as_slice().to_vec();
        let mut boundary = Vec::new();
        for (pi, p) in field.boundary.iter().enumerate() {
            // an `empty` patch field holds no values (`flatBoundaryField` skips it, `volPointInterpolation.C`): pad it so
            // that the list stays indexed by boundary face; the padded values are never read (not a patch face)
            let size = field.mesh.patches.get(pi).map_or(p.values.len(), |q| q.size);
            if p.values.len() == size {
                boundary.extend_from_slice(p.values.as_slice());
            } else if p.values.len() == 0 {
                boundary.extend(core::iter::repeat(Vector3::ZERO).take(size));
            } else {
                return Err(MeshDeformationError::Invalid(format!("patch {pi} of the displacement field has {} values for {size} faces", p.values.len())));
            }
        }
        Ok(VolDisplacement { cells, boundary })
    }

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
    /// `boundaryIsPatchFace_`: per boundary face, false on an `empty` patch (`volPointInterpolation.C:67-90`).
    is_patch_face: Vec<bool>,
    /// `isPatchPoint_`: per mesh point, on a face of a non-empty patch (`volPointInterpolation.C:77-90`).
    is_patch_point: Vec<bool>,
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
            if !topology.is_patch_point[p] {
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
            if topology.is_patch_point[p] {
                sum_weights[p] = 0.0;
                for (j, &bf) in pfaces.iter().enumerate() {
                    if topology.is_patch_face[bf] {
                        let facei = topology.n_internal + bf;
                        pw[j] = 1.0 / sub(points[p], face_centres[facei]).mag();
                        sum_weights[p] += pw[j];
                    }
                }
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
            if topology.is_patch_point[p] {
                for w in boundary_point_weights[i].iter_mut() {
                    *w /= sum_weights[p];
                }
            }
        }
        VolPointInterpolation { point_weights, boundary_point_weights }
    }

    /// `interpolate(vf)`: `interpolateInternalField` for the non-patch points, `interpolateBoundaryField` for the patch points.
    pub fn interpolate(&self, topology: &MeshTopology, disp: &VolDisplacement) -> Vec<Vector3> {
        let mut pf = vec![Vector3::ZERO; topology.n_points];
        for p in 0..topology.n_points {
            if !topology.is_patch_point[p] {
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
            if !topology.is_patch_point[p] {
                continue;
            }
            let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 0.0_f64);
            for (j, &bf) in topology.boundary_point_faces[i].iter().enumerate() {
                if !topology.is_patch_face[bf] {
                    continue;
                }
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

/// `EulerDdtScheme::fvmDdt(rho, vf)` for a `volScalarField` coefficient on the cell volumes `vsc` (`Vsc()`) and `vsc_old` (`Vsc0()` on a
/// moving mesh, `Vsc()` on a static one), `EulerDdtScheme.C:437-471`: `diag = rDeltaT*rho*Vsc`, `source = rDeltaT*rho0*vf0*Vsc0`, in that
/// operation order. The one body of the operator: [`DeformableMesh::euler_fvm_ddt`] and the neutronics region solvers call it.
///
/// # Panics
/// If a slice is shorter than `rho`.
#[must_use]
pub fn euler_ddt_diag_source(
    delta_t: f64,
    rho: &[f64],
    rho_old: &[f64],
    vf_old: &[f64],
    vsc: &[f64],
    vsc_old: &[f64],
) -> (Vec<f64>, Vec<f64>) {
    let r_delta_t = 1.0 / delta_t;
    let n = rho.len();
    let diag = (0..n).map(|c| r_delta_t * rho[c] * vsc[c]).collect();
    let source = (0..n).map(|c| r_delta_t * rho_old[c] * vf_old[c] * vsc_old[c]).collect();
    (diag, source)
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
    state: MovingMeshState,
}

/// The time `fvMesh::movePoints` reads from `Time`: `time().timeIndex()` (incremented by `runTime++`) and `time().deltaTValue()`
/// (gh:#1852). The mesh is read at time index 0, as upstream's `fvMesh` constructor takes `curTimeIndex_(time().timeIndex())`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepClock {
    /// `time().timeIndex()`: 1 after the first `runTime++`.
    pub time_index: i64,
    /// `time().deltaTValue()`, seconds.
    pub delta_t: f64,
}

/// What `fvMesh` / `polyMesh` remember between `movePoints` calls (gh:#1852): `moving_`, `curMotionTimeIndex_` and `oldPoints_`
/// (`polyMesh.C:1160-1198`), `curTimeIndex_`, `V0_` and `V00_` (`fvMesh.C:164-216`), and the mesh motion flux `meshPhi`
/// (`fvMesh.C:905-925`, `fvGeometryScheme.C:44-95`). Private: read it through [`DeformableMesh`]'s accessors.
#[derive(Debug, Clone, PartialEq)]
struct MovingMeshState {
    /// `fvMesh::curTimeIndex_`.
    cur_time_index: i64,
    /// `polyMesh::curMotionTimeIndex_`.
    cur_motion_time_index: i64,
    /// `polyMesh::moving()`: false until the first `movePoints`, true for good after.
    moving: bool,
    /// `V0_` (null until the first `storeOldVol`).
    v0: Option<Vec<f64>>,
    /// `V00_` (null until something asks for `V00()`; afterwards `storeOldVol` keeps it up to date).
    v00: Option<Vec<f64>>,
    /// `oldPointsPtr_`: the points at the start of the time step in which the mesh last moved.
    old_points: Option<Vec<Vector3>>,
    /// `phiPtr_`: `meshPhi`, internal faces then boundary faces in face order (null until the first `movePoints`).
    mesh_phi: Option<Vec<f64>>,
    /// `phiPtr_->oldTime()` (`fvMesh.C:927-934`): `meshPhi` as the previous time index's last `movePoints` left it; `None` until the
    /// mesh moves in a second time index (README round 56).
    mesh_phi_old: Option<Vec<f64>>,
    /// `phiPtr_->timeIndex()`.
    mesh_phi_time_index: i64,
}

impl MovingMeshState {
    fn at_time_index(time_index: i64) -> Self {
        MovingMeshState {
            cur_time_index: time_index,
            cur_motion_time_index: time_index,
            moving: false,
            v0: None,
            v00: None,
            old_points: None,
            mesh_phi: None,
            mesh_phi_old: None,
            mesh_phi_time_index: time_index,
        }
    }
}

impl DeformableMesh {
    /// Build from raw `polyMesh` lists (`points`, `faces`, `owner`, `neighbour`, `boundary`).
    ///
    /// # Errors
    /// [`MeshDeformationError::Refused`] for a symmetry, wedge, cyclic or processor patch (`empty` is accepted since gh:#1856);
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
                // `empty` is not a patch of `volPointInterpolation` (README round 56): its faces and the points only on them
                // are interpolated as interior; no point constraint (`emptyPointPatchField` has no `evaluate`)
                PatchKind::Patch | PatchKind::Wall | PatchKind::Empty => {}
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
        let mut is_patch_face = vec![true; faces.len() - n_internal];
        let mut is_patch_point = vec![false; n_points];
        for pa in &patches {
            for f in pa.start..pa.start + pa.size {
                if pa.kind == PatchKind::Empty {
                    is_patch_face[f - n_internal] = false;
                } else {
                    for &q in &faces[f] {
                        is_patch_point[q] = true;
                    }
                }
            }
        }
        let topology = Arc::new(MeshTopology {
            is_patch_face,
            is_patch_point,
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
        Ok(DeformableMesh {
            topology,
            original_points: points.clone(),
            points,
            mesh,
            face_centres,
            state: MovingMeshState::at_time_index(0),
        })
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
            if w[0] == "type" && !matches!(w[1], "patch" | "wall" | "mappedWall" | "empty") {
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

    /// The mesh was read at time index `time_index` instead of 0 (`curTimeIndex_(time().timeIndex())`,
    /// `curMotionTimeIndex_(time().timeIndex())`). Only for a mesh built after `runTime++`.
    #[must_use]
    pub fn at_time_index(mut self, time_index: i64) -> Self {
        self.state = MovingMeshState::at_time_index(time_index);
        self
    }

    /// `fvMesh::moving()`: false until the first [`move_points_at`](Self::move_points_at), true afterwards.
    #[must_use]
    pub fn moving(&self) -> bool {
        self.state.moving
    }

    /// `fvMesh::V0()`: the cell volumes at the start of the time step in which the mesh last moved; `None` before the first move
    /// (upstream: `FatalError "V0 is not available"`).
    #[must_use]
    pub fn v0(&self) -> Option<&[f64]> {
        self.state.v0.as_deref()
    }

    /// `fvMesh::V00()`: built from `V0()` the first time it is asked for (`fvMeshGeometry.C:233`), updated by every later
    /// `storeOldVol`.
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] ("V0 is not available") before the first move.
    pub fn v00(&mut self) -> Result<&[f64], MeshDeformationError> {
        if self.state.v00.is_none() {
            let v0 = self
                .state
                .v0
                .as_ref()
                .ok_or_else(|| MeshDeformationError::Invalid("V0 is not available".into()))?;
            self.state.v00 = Some(v0.clone());
        }
        Ok(self.state.v00.as_deref().unwrap_or(&[]))
    }

    /// `polyMesh::oldPoints()`: the points at the start of the time step in which the mesh last moved (`None` before any move).
    #[must_use]
    pub fn old_points(&self) -> Option<&[Vector3]> {
        self.state.old_points.as_deref()
    }

    /// `fvMesh::phi()` (`meshPhi`): the swept volume of every face over `deltaT`, internal faces then boundary faces; `None`
    /// before the first move.
    #[must_use]
    pub fn mesh_phi(&self) -> Option<&[f64]> {
        self.state.mesh_phi.as_deref()
    }

    /// `meshPhi.oldTime()` (gh:#1856 item 5; `fvMesh::movePoints` `fvMesh.C:927-934`, `GeometricField::oldTime`/`storeOldTimes`): the
    /// mesh motion flux the last `movePoints` of the PREVIOUS time index left. Within the first time index in which the mesh moved
    /// there is no old field yet; a consumer asking then gets a copy of the current one, which is what this returns (`None` before
    /// the first move). GeN-Foam and `EulerDdtScheme` never read it (`backward`, `CrankNicolson` and `ddtCorr` would).
    #[must_use]
    pub fn mesh_phi_old(&self) -> Option<&[f64]> {
        self.state.mesh_phi_old.as_deref().or(self.state.mesh_phi.as_deref())
    }

    /// `fvMesh::Vsc()` without sub-cycling: `V()`.
    #[must_use]
    pub fn vsc(&self) -> &[f64] {
        &self.mesh.cell_volumes
    }

    /// `fvMesh::Vsc0()` without sub-cycling: `V0()`.
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] ("V0 is not available") before the first move.
    pub fn vsc0(&self) -> Result<&[f64], MeshDeformationError> {
        self.state.v0.as_deref().ok_or_else(|| MeshDeformationError::Invalid("V0 is not available".into()))
    }

    /// `fvMesh::movePoints(p)` with its time bookkeeping (`fvMesh.C:891-955`, `polyMesh.C:1160-1238`,
    /// `fvGeometryScheme.C:44-95`), in upstream's order:
    ///
    /// 1. if `curTimeIndex_ < timeIndex`: `storeOldVol(V())` (`V00 = V0` when `V00` exists; `V0 = V`);
    /// 2. create `meshPhi` (zero) on the first call;
    /// 3. `polyMesh::movePoints`: `moving = true`; once per time index, the CURRENT points become `oldPoints`; then the points are
    ///    replaced and the geometry recomputed ([`move_points`](Self::move_points));
    /// 4. `setMeshPhi`: `meshPhi[f] = f.sweptVol(oldPoints, points) * (1.0/deltaT)`, every face (the reused, ESI-bit-identical
    ///    `outram_foam_mesh::dynamic_mesh::mesh_phi`).
    ///
    /// Called again within the same time index (a Picard iteration of `multiPhysicsSolver::correctPhysics`), `V0` and `oldPoints`
    /// stay those of the step start and `meshPhi` is the swept volume from the step start to the new points.
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] for a wrong point count or `delta_t <= 0`;
    /// [`MeshDeformationError::Refused`] for the first move at the time index the mesh was read at (upstream's `oldPoints()` would
    /// then be the already-moved points; `runTime++` comes before `deformMesh()` in every GeN-Foam loop).
    pub fn move_points_at(&mut self, new_points: Vec<Vector3>, clock: StepClock) -> Result<(), MeshDeformationError> {
        if new_points.len() != self.points.len() {
            return Err(MeshDeformationError::Invalid(format!(
                "Size of newPoints {} does not correspond to current mesh points size {}",
                new_points.len(),
                self.points.len()
            )));
        }
        if !(clock.delta_t > 0.0) {
            return Err(MeshDeformationError::Invalid(format!("deltaT must be positive, got {}", clock.delta_t)));
        }
        if self.state.old_points.is_none() && self.state.cur_motion_time_index == clock.time_index {
            return Err(refused(
                "movePoints at the time index the mesh was read at",
                "polyMesh::oldPoints() would be the moved points; increment the time (runTime++) first, as GeN-Foam.C does",
            ));
        }
        // fvMesh::movePoints (1): storeOldVol(V())
        if self.state.cur_time_index < clock.time_index {
            if self.state.v00.is_some() {
                if let Some(v0) = &self.state.v0 {
                    self.state.v00 = Some(v0.clone());
                }
            }
            self.state.v0 = Some(self.mesh.cell_volumes.clone());
            self.state.cur_time_index = clock.time_index;
        }
        // (2) the meshPhi field is created, zero, on the first call
        let n_faces = self.topology.faces.len();
        if self.state.mesh_phi.is_none() {
            self.state.mesh_phi = Some(vec![0.0; n_faces]);
            self.state.mesh_phi_time_index = clock.time_index;
        } else if self.state.mesh_phi_time_index != clock.time_index {
            // `phiPtr_->oldTime()`, then `storeOldTimes()` at the assignment of `setMeshPhi`: the old field is the current one
            self.state.mesh_phi_old = self.state.mesh_phi.clone();
            self.state.mesh_phi_time_index = clock.time_index;
        }
        // (3) polyMesh::movePoints
        self.state.moving = true;
        if self.state.cur_motion_time_index != clock.time_index {
            self.state.old_points = Some(self.points.clone());
            self.state.cur_motion_time_index = clock.time_index;
        }
        self.move_points(new_points)?;
        // (4) fvGeometryScheme::setMeshPhi
        let raw = |p: &[Vector3]| -> Vec<[f64; 3]> { p.iter().map(|q| [q.x, q.y, q.z]).collect() };
        let old = raw(self.state.old_points.as_deref().unwrap_or(&[]));
        let cur = raw(&self.points);
        self.state.mesh_phi = Some(outram_foam_mesh::dynamic_mesh::mesh_phi(&self.topology.faces, &old, &cur, clock.delta_t));
        Ok(())
    }

    /// `deformMesh()` with the time state: `movePoints(originalPoints_ + volPointInterpolation::interpolate(disp))` through
    /// [`move_points_at`](Self::move_points_at). Returns the interpolated point displacement.
    ///
    /// # Errors
    /// As [`DeformableMesh::move_points_at`].
    pub fn deform_mesh_at(&mut self, disp: &VolDisplacement, clock: StepClock) -> Result<Vec<Vector3>, MeshDeformationError> {
        let pd = self.interpolate_displacement(disp);
        let moved = self
            .original_points
            .iter()
            .zip(&pd)
            .map(|(o, d)| Vector3::new(o.x + d.x, o.y + d.y, o.z + d.z))
            .collect();
        self.move_points_at(moved, clock)?;
        Ok(pd)
    }

    /// `EulerDdtScheme::fvmDdt(rho, vf)` for a `volScalarField` coefficient (`EulerDdtScheme.C:437-471`): the matrix `diag` and
    /// `source`, the consumer of `V`, `V0` and `moving()` in the neutronics region's flux and precursor equations.
    /// `diag = rDeltaT*rho*Vsc`; `source = rDeltaT*rho0*vf0*Vsc0` on a moving mesh, `... *Vsc` on a static one.
    ///
    /// # Errors
    /// [`MeshDeformationError::Invalid`] for a wrong size.
    pub fn euler_fvm_ddt(
        &self,
        delta_t: f64,
        rho: &[f64],
        rho_old: &[f64],
        vf_old: &[f64],
    ) -> Result<(Vec<f64>, Vec<f64>), MeshDeformationError> {
        let n = self.mesh.n_cells;
        if rho.len() != n || rho_old.len() != n || vf_old.len() != n {
            return Err(MeshDeformationError::Invalid("ddt field size does not match the mesh".into()));
        }
        let vsc = self.vsc();
        let vsc_old: &[f64] = if self.state.moving { self.vsc0()? } else { vsc };
        Ok(euler_ddt_diag_source(delta_t, rho, rho_old, vf_old, vsc, vsc_old))
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

    fn expansion(m: &DeformableMesh, k: f64) -> VolDisplacement {
        let c = vec![DispPatchCondition::ZeroGradient; 6];
        let cells = m.mesh().cell_centres.iter().map(|x| Vector3::new(k * x.x, 0.5 * k * x.y, 0.0)).collect::<Vec<_>>();
        VolDisplacement::from_cells(m, cells, &c).unwrap()
    }

    #[test]
    fn moving_state_follows_fvmesh_move_points() {
        let mut m = mesh(3, PointCellsOrder::CellAscending);
        let v_orig = m.mesh().cell_volumes.clone();
        let p_orig = m.points().to_vec();
        assert!(!m.moving() && m.v0().is_none() && m.mesh_phi().is_none() && m.old_points().is_none());
        assert!(m.v00().is_err() && m.vsc0().is_err());
        // movePoints before runTime++ is refused (oldPoints would be the moved points)
        let d1 = expansion(&m, 0.01);
        assert!(matches!(m.deform_mesh_at(&d1, StepClock { time_index: 0, delta_t: 0.5 }), Err(MeshDeformationError::Refused { .. })));
        // step 1
        m.deform_mesh_at(&d1, StepClock { time_index: 1, delta_t: 0.5 }).unwrap();
        assert!(m.moving());
        assert_eq!(m.v0().unwrap(), v_orig.as_slice());
        assert_eq!(m.old_points().unwrap(), p_orig.as_slice());
        let v1 = m.mesh().cell_volumes.clone();
        assert_eq!(m.v00().unwrap(), v_orig.as_slice()); // built lazily from V0
        // meshPhi is the swept volume over deltaT: its sum over a cell's faces tracks V - V0 (planar hex faces: up to round-off)
        let phi = m.mesh_phi().unwrap().to_vec();
        assert_eq!(phi.len(), m.topology().faces().len());
        let mut net = vec![0.0; v1.len()];
        for (f, &o) in m.topology().owner().iter().enumerate() {
            net[o] += phi[f] * 0.5;
        }
        for (f, &n) in m.topology().neighbour().iter().enumerate() {
            net[n] -= phi[f] * 0.5;
        }
        for c in 0..v1.len() {
            assert!((net[c] - (v1[c] - v_orig[c])).abs() < 1e-12, "cell {c}");
        }
        // meshPhi.oldTime() (gh:#1856 item 5): in the first time index in which the mesh moves there is no old field yet, a request gets
        // a copy of the current one
        assert_eq!(m.mesh_phi_old().unwrap(), phi.as_slice());
        // second call in the SAME time index (a Picard iteration): V0 and oldPoints stay
        let d15 = expansion(&m, 0.015);
        m.deform_mesh_at(&d15, StepClock { time_index: 1, delta_t: 0.5 }).unwrap();
        let phi_call2 = m.mesh_phi().unwrap().to_vec();
        assert_ne!(phi_call2, phi);
        assert_eq!(m.v0().unwrap(), v_orig.as_slice());
        assert_eq!(m.old_points().unwrap(), p_orig.as_slice());
        assert_ne!(m.mesh().cell_volumes, v1);
        // step 2: V0 <- V of the end of step 1; V00 <- the old V0
        let v_end1 = m.mesh().cell_volumes.clone();
        let p_end1 = m.points().to_vec();
        let d2 = expansion(&m, 0.02);
        m.deform_mesh_at(&d2, StepClock { time_index: 2, delta_t: 0.5 }).unwrap();
        assert_eq!(m.v0().unwrap(), v_end1.as_slice());
        assert_eq!(m.old_points().unwrap(), p_end1.as_slice());
        assert_eq!(m.v00().unwrap(), v_orig.as_slice());
        // the old meshPhi of step 2 is the one the last movePoints of step 1 left (call 2), and stays it within step 2
        assert_eq!(m.mesh_phi_old().unwrap(), phi_call2.as_slice());
        let phi_step2_call1 = m.mesh_phi().unwrap().to_vec();
        m.deform_mesh_at(&expansion(&m, 0.021), StepClock { time_index: 2, delta_t: 0.5 }).unwrap();
        assert_eq!(m.mesh_phi_old().unwrap(), phi_call2.as_slice());
        assert_ne!(m.mesh_phi().unwrap(), phi_step2_call1.as_slice());
        // the time-less deform_mesh keeps no state
        let mut n = mesh(3, PointCellsOrder::CellAscending);
        let dn = expansion(&n, 0.01);
        n.deform_mesh(&dn).unwrap();
        assert!(!n.moving() && n.v0().is_none());
        // a bad deltaT is refused
        let same = n.points().to_vec();
        assert!(n.move_points_at(same, StepClock { time_index: 1, delta_t: 0.0 }).is_err());
    }

    #[test]
    fn euler_ddt_uses_v_and_v0_on_a_moving_mesh_and_v_on_a_static_one() {
        let mut m = mesh(2, PointCellsOrder::CellAscending);
        let n = m.mesh().n_cells;
        let (rho, old) = (vec![2.0; n], vec![3.0; n]);
        let (d, s) = m.euler_fvm_ddt(0.5, &rho, &rho, &old).unwrap();
        assert_eq!(d[0], 2.0 * 2.0 * 1.0);
        assert_eq!(s[0], 2.0 * 2.0 * 3.0 * 1.0);
        let dd = expansion(&m, 0.05);
        m.deform_mesh_at(&dd, StepClock { time_index: 1, delta_t: 0.5 }).unwrap();
        let (d, s) = m.euler_fvm_ddt(0.5, &rho, &rho, &old).unwrap();
        for c in 0..n {
            assert_eq!(d[c], 2.0 * 2.0 * m.mesh().cell_volumes[c]);
            assert_eq!(s[c], 2.0 * 2.0 * 3.0 * m.v0().unwrap()[c]);
            assert_ne!(d[c] / 4.0, s[c] / 12.0);
        }
        assert!(m.euler_fvm_ddt(0.5, &rho[..1], &rho, &old).is_err());
    }
}
