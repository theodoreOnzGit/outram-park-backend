// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/multiPhysicsSolver/multiPhysicsSolver.C
//     (createSolvers, correctPhysics, getResidual, maxDeltaT),
//     src/classes/multiRegion/meshHandler/{meshHandler.C, meshHandlerTemplates.C} (mapping dictionary,
//     `map`), src/GeN-Foam/include/setDeltaT.C, src/classes/common/solver/solver.C (`deltaTFactor`),
//   and OpenFOAM-ESI v2506 `meshToMesh` (`mapTgtToSrc`, `multiplyWeightedOp`).
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Case-driven `multiPhysicsSolver` pieces (gh:#1774, gh:#1361 round 9)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! * [`CouplingDict`]: `constant/multiRegionCouplingDict` (`multiPhysicsSolvers.<name>.{solvers, minResidual,
//!   maxIterations, includeFluidMechanicsInLoop}` and the `mappings`).
//! * [`correct_physics_loop`]: `multiPhysicsSolver::correctPhysics`, literally: `residual` is declared once
//!   outside the loop and only ever `max`-ed (it is never reset between iterations), `mapTheseFields` runs at
//!   the top of every iteration, a solver whose `correctOnlyEnergy_` is set runs `correctTightlyCoupledPhysics`
//!   from the second iteration on.
//! * [`IdenticalMeshMap`]: `meshToMesh(src, tgt, imCellVolumeWeight, pmAABB, false)` between two meshes that
//!   are the same mesh. **Measured on upstream** (`upstream_driver/dump_mesh_to_mesh`, 2D_MSFR fluid/neutro
//!   meshes, 8595 cells, both directions): the addressing is the identity (one overlap per cell, itself) and
//!   every weight is EXACTLY `1.0`; `mapTgtToSrc` then computes `result = result (1 - 1) + 1.0 src`, i.e. a copy.
//!   The weight is `overlap/V` from `tetOverlapVolume`; for a cell with itself it is exactly 1 on this mesh, so it
//!   is neither rescaled nor approximated here, and [`IdenticalMeshMap::new`] REFUSES a pair of meshes that are not
//!   bitwise the same geometry (~~a different mesh needs the general supermesh, which is not ported: gh:#651~~
//!   **CORRECTED 2026-11-26 (gh:#651, round 28):** the general operator exists, [`super::cell_overlap`], bit-identical to
//!   ESI; ~~it is not yet wired into this type, which keeps refusing, so a loop on different meshes uses
//!   `MeshToMesh::cell_volume_weight` directly~~ **CORRECTED 2026-11-26 (gh:#1854, round 30):** [`RegionMeshMap`] is the wiring:
//!   with the regions' polyhedral meshes it is the general operator, without them it is this type). Also here:
//!   [`correct_physics_loop_with_deformation`] (`deformMesh()` before each solver's `correctPhysics()`) and
//!   [`deform_region_mesh`] (`deformMesh()` of one region, from the mapped `disp`).
//! * [`adjust_delta_t`] / [`initial_delta_t`]: `setDeltaT.C`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[cfg(feature = "std")]
use std::path::Path;

use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::prelude::Vector3;

use super::cell_overlap::{CellVolumeWeightAddressing, OverlapMesh};
use super::coupling_fields::MeshHandler;
use super::mesh_deformation::{DeformableMesh, MeshDeformationDict, MeshDeformationError, StepClock, VolDisplacement};
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::dict::FoamFile;
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry};

use crate::error::AppBuilderError;

/// `solver::deltaTFactor` (`solver.C:52`).
pub const DELTA_T_FACTOR: f64 = 1.2;

/// One `mappings.<target>.<source>` entry: `targetFields` are filled from `sourceFields` of the source region.
#[derive(Debug, Clone, PartialEq)]
pub struct MappingEntry {
    /// The region whose fields are written (the outer keyword).
    pub target_region: String,
    /// The region whose fields are read (the inner keyword).
    pub source_region: String,
    /// `sourceFields`.
    pub source_fields: Vec<String>,
    /// `targetFields`.
    pub target_fields: Vec<String>,
}

/// `multiPhysicsSolvers.<name>` and the `mappings` of a `multiRegionCouplingDict`.
#[derive(Debug, Clone, PartialEq)]
pub struct CouplingDict {
    /// Region name and solver type, in dictionary order.
    pub solvers: Vec<(String, String)>,
    /// `minResidual`.
    pub min_residual: f64,
    /// `maxIterations`.
    pub max_iterations: usize,
    /// `includeFluidMechanicsInLoop` (regions that keep the full `correctPhysics` after the first iteration).
    pub include_fluid_mechanics_in_loop: Vec<String>,
    /// `mappings`, in dictionary order.
    pub mappings: Vec<MappingEntry>,
}

#[cfg(feature = "std")]
fn word_list(e: &FoamEntry) -> Option<Vec<String>> {
    match e {
        FoamEntry::List(v) | FoamEntry::Tokens(v) => v.iter().map(|x| x.as_word().map(str::to_string)).collect(),
        FoamEntry::Word(w) => Some(vec![w.clone()]),
        _ => None,
    }
}

#[cfg(feature = "std")]
impl CouplingDict {
    /// Parse the dictionary and pick `multiPhysicsSolvers.<name>`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if an entry is missing or malformed.
    pub fn from_dict(d: &FoamDict, name: &str, case: &Path) -> Result<Self, AppBuilderError> {
        let e = |m: String| AppBuilderError::Case { case: case.to_path_buf(), msg: m };
        let mps = d.get_dict("multiPhysicsSolvers").ok_or_else(|| e("no multiPhysicsSolvers".into()))?;
        let sd = mps.get_dict(name).ok_or_else(|| e(format!("no multiPhysicsSolvers.{name}")))?;
        let solvers_d = sd.get_dict("solvers").ok_or_else(|| e("no solvers".into()))?;
        let mut solvers = Vec::new();
        for (k, v) in solvers_d.iter() {
            match v {
                FoamEntry::Word(w) => solvers.push((k.to_string(), w.clone())),
                _ => return Err(e(format!("solvers.{k} is not a word"))),
            }
        }
        let min_residual = match sd.get("minResidual") {
            Some(FoamEntry::Scalar(x)) => *x,
            _ => return Err(e("no minResidual".into())),
        };
        let max_iterations = match sd.get("maxIterations") {
            Some(FoamEntry::Scalar(x)) => *x as usize,
            _ => return Err(e("no maxIterations".into())),
        };
        let include_fluid_mechanics_in_loop =
            sd.get("includeFluidMechanicsInLoop").and_then(word_list).unwrap_or_default();
        let mut mappings = Vec::new();
        if let Some(md) = d.get_dict("mappings") {
            for (target, te) in md.iter() {
                let FoamEntry::SubDict(td) = te else { continue };
                for (source, se) in td.iter() {
                    let FoamEntry::SubDict(ed) = se else { continue };
                    let src = ed.get("sourceFields").and_then(word_list).unwrap_or_default();
                    let tgt = ed.get("targetFields").and_then(word_list).unwrap_or_default();
                    if src.len() != tgt.len() {
                        return Err(e(format!("mappings.{target}.{source}: source and target field counts differ")));
                    }
                    mappings.push(MappingEntry {
                        target_region: target.to_string(),
                        source_region: source.to_string(),
                        source_fields: src,
                        target_fields: tgt,
                    });
                }
            }
        }
        Ok(Self { solvers, min_residual, max_iterations, include_fluid_mechanics_in_loop, mappings })
    }

    /// Read `<case>/constant/multiRegionCouplingDict` and pick `multiPhysicsSolvers.<name>`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] if the file is unreadable or an entry is missing.
    pub fn read(case: &Path, name: &str) -> Result<Self, AppBuilderError> {
        let p = case.join("constant/multiRegionCouplingDict");
        let f = FoamFile::read(&p).map_err(|er| AppBuilderError::Case { case: case.to_path_buf(), msg: format!("multiRegionCouplingDict: {er}") })?;
        Self::from_dict(&f.dict, name, case)
    }
}

// The accessors below need no files: available without `std` (gh:#1814: the workflow module uses them).
impl CouplingDict {
    /// `correctOnlyEnergy_` per solver: true unless the region is in `includeFluidMechanicsInLoop`.
    #[must_use]
    pub fn correct_only_energy(&self) -> Vec<bool> {
        self.solvers.iter().map(|(r, _)| !self.include_fluid_mechanics_in_loop.contains(r)).collect()
    }

    /// The mapping entries in the order `mapTheseFields` visits them: target region `i`, then source `j`,
    /// both over `regions` (the solver names in order), for entries present in the dictionary.
    #[must_use]
    pub fn mapping_order(&self, regions: &[String]) -> Vec<&MappingEntry> {
        let mut out = Vec::new();
        for ti in regions {
            for sj in regions {
                if let Some(m) = self.mappings.iter().find(|m| &m.target_region == ti && &m.source_region == sj) {
                    out.push(m);
                }
            }
        }
        out
    }
}

/// What [`correct_physics_loop`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoopOutcome {
    /// `iterN` after the loop.
    pub iterations: usize,
    /// The final (never reset) `residual`.
    pub residual: f64,
    /// `residual < minResidual` after the last iteration (upstream prints "converged after N iterations").
    pub converged: bool,
}

/// `multiPhysicsSolver::correctPhysics`.
///
/// `map` is `mapTheseFields`; `correct(i, tight)` runs solver `i` (`tight` is `correctTightlyCoupledPhysics`,
/// chosen when `iterN > 0 && correct_only_energy[i]`); `residual_of(i)` is `getResidual()`. Region meshes
/// are the caller's closures' business, so there is no `dyn`.
pub fn correct_physics_loop<M, C, R>(
    min_residual: f64,
    max_iterations: usize,
    correct_only_energy: &[bool],
    map: M,
    correct: C,
    residual_of: R,
) -> LoopOutcome
where
    M: FnMut(),
    C: FnMut(usize, bool),
    R: FnMut(usize) -> f64,
{
    correct_physics_loop_with_deformation(min_residual, max_iterations, correct_only_energy, map, |_| {}, correct, residual_of)
}

/// `multiPhysicsSolver::correctPhysics` WITH the `deformMesh()` call upstream makes in every iteration
/// (`multiPhysicsSolver.C:174-186`): `mapTheseFields()`; per solver `deformMesh()` THEN `correctPhysics()` (or the tight
/// variant) THEN `correctBaffleLessFields()`. `deform(i)` is solver `i`'s `deformMesh()` (a no-op for a region not listed in
/// `meshDeformation`, see [`deform_region_mesh`]). [`correct_physics_loop`] is this with no deformation.
pub fn correct_physics_loop_with_deformation<M, D, C, R>(
    min_residual: f64,
    max_iterations: usize,
    correct_only_energy: &[bool],
    mut map: M,
    mut deform: D,
    mut correct: C,
    mut residual_of: R,
) -> LoopOutcome
where
    M: FnMut(),
    D: FnMut(usize),
    C: FnMut(usize, bool),
    R: FnMut(usize) -> f64,
{
    let mut iter_n = 0usize;
    let mut residual = 0.0_f64;
    loop {
        map();
        for (i, &only_energy) in correct_only_energy.iter().enumerate() {
            deform(i);
            correct(i, iter_n > 0 && only_energy);
        }
        for i in 0..correct_only_energy.len() {
            residual = residual.max(residual_of(i));
        }
        iter_n += 1;
        if !(residual > min_residual && iter_n < max_iterations) {
            break;
        }
    }
    LoopOutcome { iterations: iter_n, residual, converged: residual < min_residual }
}

/// `deformMesh()` of one region (`neutronics.C:363-408`): when `meshDeformation` lists `region`, take the displacement field it
/// names (`displacementField`) from the region's coupling fields (`mesh_.lookupObject<volVectorField>`) and move the region's mesh
/// to `originalPoints_ + volPointInterpolation::interpolate(disp)` with the time state of `fvMesh::movePoints`
/// ([`DeformableMesh::deform_mesh_at`]). Returns whether the mesh moved (a region not listed is left alone, as upstream).
///
/// # Errors
/// [`MeshDeformationError::Invalid`] if the named field is missing or not sized to the mesh, or as
/// [`DeformableMesh::deform_mesh_at`].
pub fn deform_region_mesh(
    dict: &MeshDeformationDict,
    region: &str,
    mesh: &mut DeformableMesh,
    handler: &MeshHandler,
    clock: StepClock,
) -> Result<bool, MeshDeformationError> {
    let Some(field_name) = dict.displacement_field(region) else {
        return Ok(false);
    };
    let field = handler
        .region(region)
        .and_then(|r| r.vector(field_name))
        .ok_or_else(|| MeshDeformationError::Invalid(format!("region {region} has no vector field `{field_name}`")))?;
    let disp = VolDisplacement::from_vol_field(field)?;
    mesh.deform_mesh_at(&disp, clock)?;
    Ok(true)
}

/// The `meshToMesh` `cellVolumeWeight` map between two region meshes (gh:#1854), whatever their cells.
///
/// With the regions' polyhedral meshes ([`OverlapMesh`]) it is upstream's general operator, built once as `meshHandler.C:192-201`
/// does (see [`MeshToMesh::cell_volume_weight`]). Without them it is [`IdenticalMeshMap`], which refuses a pair that is not
/// bitwise the same cells. The two agree bit for bit on identical meshes (unit test).
#[derive(Debug, Clone, PartialEq)]
pub enum RegionMeshMap {
    /// Two identical meshes: weights exactly 1.
    Identical(IdenticalMeshMap),
    /// The exact overlap addressing; upstream's source is the RECEIVING region.
    General(CellVolumeWeightAddressing),
}

impl RegionMeshMap {
    /// Build the map that moves a field of `provider` onto `receiver`.
    ///
    /// # Errors
    /// A message naming what is missing or differs.
    pub fn new(
        provider: &FvMesh,
        receiver: &FvMesh,
        geometry: Option<(&OverlapMesh, &OverlapMesh)>,
    ) -> Result<Self, String> {
        match geometry {
            Some((provider_geometry, receiver_geometry)) => {
                if provider_geometry.n_cells() != provider.n_cells || receiver_geometry.n_cells() != receiver.n_cells {
                    return Err("an OverlapMesh does not have the cell count of its FvMesh".into());
                }
                Ok(Self::General(CellVolumeWeightAddressing::new_normalised(receiver_geometry, provider_geometry)))
            }
            None => IdenticalMeshMap::new(provider, receiver)
                .map(Self::Identical)
                .map_err(|e| format!("{e} (pass the polyhedral meshes: RegionMeshMap::new(.., Some((provider, receiver))))")),
        }
    }

    /// `mapTgtToSrc(provider_field, plusEqOp, receiver_field)` for a scalar cell list.
    pub fn map_scalar(&self, provider_field: &[f64], receiver_field: &mut [f64]) {
        match self {
            Self::Identical(m) => m.map(provider_field, receiver_field),
            Self::General(a) => a.map_tgt_to_src_scalar(provider_field, receiver_field),
        }
    }

    /// `mapTgtToSrc` for a vector cell list.
    ///
    /// # Panics
    /// Panics on mismatched sizes.
    pub fn map_vector(&self, provider_field: &[Vector3], receiver_field: &mut [Vector3]) {
        match self {
            Self::Identical(m) => {
                assert_eq!(provider_field.len(), receiver_field.len(), "identical meshes: sizes must match");
                for c in 0..receiver_field.len() {
                    let f = 1.0 - m.weights[c];
                    let r = receiver_field[c];
                    let p = provider_field[c];
                    let w = m.weights[c];
                    receiver_field[c] = Vector3::new(r.x * f + w * p.x, r.y * f + w * p.y, r.z * f + w * p.z);
                }
            }
            Self::General(a) => a.map_tgt_to_src_vector(provider_field, receiver_field),
        }
    }

    /// `mapSrcToTgt(receiver_field, plusEqOp, provider_field)`: the other direction (`heatExchanger.C:265-267`). Scalars.
    ///
    /// # Panics
    /// Panics on mismatched sizes.
    pub fn map_reverse_scalar(&self, receiver_field: &[f64], provider_field: &mut [f64]) {
        match self {
            Self::Identical(m) => m.map(receiver_field, provider_field),
            Self::General(a) => a.map_src_to_tgt_scalar(receiver_field, provider_field),
        }
    }
}

/// `setDeltaT(runTime, solvers)`: the initial step `min(deltaT_now, controlDict deltaT)`.
#[must_use]
pub fn initial_delta_t(current: f64, control_delta_t: f64) -> f64 {
    current.min(control_delta_t)
}

/// `adjustDeltaT(runTime, solvers)`: `Some(new deltaT)` when `adjustTimeStep` is on and the limit is finite,
/// else `None` (the step is unchanged). `max_delta_ts` are the solvers' `maxDeltaT()`; `min_delta_t` is
/// `controlDict minDeltaT` (default `VSMALL`); the result is `min(deltaTFactor deltaT, limit)`. (`Time::setDeltaT`
/// then calls `adjustDeltaT`, which only acts for `writeControl adjustableRunTime`: not the case here.)
#[must_use]
pub fn adjust_delta_t(adjust_time_step: bool, max_delta_ts: &[f64], min_delta_t: f64, current: f64) -> Option<f64> {
    if !adjust_time_step {
        return None;
    }
    let mut dt = f64::MAX; // VGREAT
    for &m in max_delta_ts {
        dt = dt.min(m);
    }
    dt = dt.max(min_delta_t);
    let root_vgreat = 1.0e150;
    if dt < root_vgreat {
        Some((DELTA_T_FACTOR * current).min(dt))
    } else {
        None
    }
}

/// The `meshToMesh` `cellVolumeWeight` map between two identical meshes (see the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct IdenticalMeshMap {
    /// The weight of each cell's single overlap (exactly 1.0, measured on upstream).
    pub weights: Vec<f64>,
}

impl IdenticalMeshMap {
    /// Build the map; refuses meshes that are not bitwise the same cells, centres and volumes.
    ///
    /// # Errors
    /// A message naming what differs.
    pub fn new(src: &FvMesh, tgt: &FvMesh) -> Result<Self, String> {
        if src.n_cells != tgt.n_cells || src.n_faces != tgt.n_faces {
            return Err("meshes differ in cell or face count".into());
        }
        for c in 0..src.n_cells {
            if src.cell_volumes[c].to_bits() != tgt.cell_volumes[c].to_bits() {
                return Err(format!("cell {c}: volumes differ; the general cellVolumeWeight operator (multi_region::cell_overlap, gh:#651) is not wired into IdenticalMeshMap"));
            }
            if src.cell_centres[c] != tgt.cell_centres[c] {
                return Err(format!("cell {c}: centres differ; the general cellVolumeWeight operator (multi_region::cell_overlap, gh:#651) is not wired into IdenticalMeshMap"));
            }
        }
        Ok(Self { weights: vec![1.0; src.n_cells] })
    }

    /// `mapTgtToSrc` with `plusEqOp`: `result[c] = result[c] (1 - w) + w src[c]`, ESI's operation order
    /// (`result *= 1 - sum(w)`, then `result += w src`).
    pub fn map(&self, src: &[f64], result: &mut [f64]) {
        for c in 0..result.len() {
            result[c] *= 1.0 - self.weights[c];
            result[c] += self.weights[c] * src[c];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_runs_once_when_converged_and_to_the_cap_otherwise() {
        let mut maps = 0;
        let mut calls = Vec::new();
        let out = correct_physics_loop(5e-5, 3, &[true, true], || maps += 1, |i, t| calls.push((i, t)), |_| 1e-6);
        assert_eq!((out.iterations, out.converged, maps), (1, true, 1));
        assert_eq!(calls, vec![(0, false), (1, false)]);
        // a residual that stays above the threshold runs to the cap, tight from the second iteration;
        // and is never reset (a later small residual cannot lower it)
        let mut calls = Vec::new();
        let mut k = 0;
        let out = correct_physics_loop(
            5e-5,
            3,
            &[true, false],
            || {},
            |i, t| calls.push((i, t)),
            |_| {
                k += 1;
                if k == 1 { 1.0 } else { 0.0 }
            },
        );
        assert_eq!((out.iterations, out.converged), (3, false));
        assert_eq!(calls[2..4], [(0, true), (1, false)]);
    }

    #[test]
    fn delta_t_follows_set_delta_t_c() {
        assert_eq!(initial_delta_t(1e-3, 5e-4), 5e-4);
        assert_eq!(adjust_delta_t(false, &[1.0], 0.0, 1e-3), None);
        // factor 1.2 growth cap
        assert_eq!(adjust_delta_t(true, &[1.0, 5.0], 0.0, 1e-3), Some(1.2e-3));
        // a smaller limit wins
        assert_eq!(adjust_delta_t(true, &[5e-4], 0.0, 1e-3), Some(5e-4));
        // no finite limit: unchanged
        assert_eq!(adjust_delta_t(true, &[f64::MAX], 0.0, 1e-3), None);
    }

    #[test]
    fn identical_map_accepts_the_same_mesh_and_refuses_a_different_one() {
        use crate::genfoam::multi_region::outer_iteration::lumped_cell_mesh;
        let a = lumped_cell_mesh(1.0);
        let b = lumped_cell_mesh(1.0);
        let c = lumped_cell_mesh(2.0);
        let m = IdenticalMeshMap::new(&a, &b).expect("same mesh");
        assert_eq!(m.weights, vec![1.0]);
        let e = IdenticalMeshMap::new(&a, &c).expect_err("different volume");
        assert!(e.contains("volumes differ"), "{e}");
    }

    #[test]
    fn identical_map_is_a_copy() {
        let m = IdenticalMeshMap { weights: vec![1.0; 3] };
        let mut r = vec![7.0, -2.0, 1e300];
        m.map(&[1.0, 2.0, 3.0], &mut r);
        assert_eq!(r, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn loop_with_deformation_calls_deform_before_correct_for_each_solver_in_every_iteration() {
        let events: core::cell::RefCell<Vec<String>> = core::cell::RefCell::new(Vec::new());
        let mut k = 0;
        let out = correct_physics_loop_with_deformation(
            5e-5,
            2,
            &[true, false],
            || events.borrow_mut().push("map".into()),
            |i| events.borrow_mut().push(format!("deform{i}")),
            |i, t| events.borrow_mut().push(format!("correct{i}{}", if t { "T" } else { "" })),
            |_| {
                k += 1;
                1.0
            },
        );
        assert_eq!((out.iterations, out.converged), (2, false));
        assert_eq!(
            events.borrow().as_slice(),
            ["map", "deform0", "correct0", "deform1", "correct1", "map", "deform0", "correct0T", "deform1", "correct1"]
        );
    }

    #[test]
    fn region_mesh_map_general_equals_identical_on_identical_meshes_and_refuses_otherwise() {
        use crate::genfoam::multi_region::outer_iteration::lumped_cell_mesh;
        let (a, b) = (lumped_cell_mesh(1.0), lumped_cell_mesh(1.0));
        let g = OverlapMesh::box_grid(1, 1, 1, [0.0; 3], [1.0; 3]);
        let identical = RegionMeshMap::new(&a, &b, None).unwrap();
        let general = RegionMeshMap::new(&a, &b, Some((&g, &g))).unwrap();
        assert!(matches!(identical, RegionMeshMap::Identical(_)) && matches!(general, RegionMeshMap::General(_)));
        let (mut r1, mut r2) = ([7.5], [7.5]);
        identical.map_scalar(&[3.25], &mut r1);
        general.map_scalar(&[3.25], &mut r2);
        assert_eq!(r1[0].to_bits(), r2[0].to_bits());
        let (mut v1, mut v2) = ([Vector3::new(1.0, 2.0, 3.0)], [Vector3::new(1.0, 2.0, 3.0)]);
        identical.map_vector(&[Vector3::new(-1.0, 0.5, 9.0)], &mut v1);
        general.map_vector(&[Vector3::new(-1.0, 0.5, 9.0)], &mut v2);
        assert_eq!(v1, v2);
        let (mut b1, mut b2) = ([1.0], [1.0]);
        identical.map_reverse_scalar(&[4.0], &mut b1);
        general.map_reverse_scalar(&[4.0], &mut b2);
        assert_eq!(b1[0].to_bits(), b2[0].to_bits());
        // a different mesh needs the polyhedral geometry; a wrong cell count is refused
        let c = lumped_cell_mesh(2.0);
        let e = RegionMeshMap::new(&a, &c, None).unwrap_err();
        assert!(e.contains("volumes differ") && e.contains("pass the polyhedral meshes"), "{e}");
        let g2 = OverlapMesh::box_grid(2, 1, 1, [0.0; 3], [1.0; 3]);
        assert!(RegionMeshMap::new(&a, &c, Some((&g, &g2))).is_err());
        // different meshes with their geometry: the receiver cell takes the provider's value (one overlap, weight 1)
        let half = OverlapMesh::box_grid(1, 1, 1, [0.0; 3], [0.5, 1.0, 1.0]);
        let m = RegionMeshMap::new(&a, &c, Some((&g, &half))).unwrap();
        let mut r = [100.0];
        m.map_scalar(&[8.0], &mut r);
        assert_eq!(r[0], 8.0);
    }
}
