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
//!   bitwise the same geometry (a different mesh needs the general supermesh, which is not ported: gh:#651).
//! * [`adjust_delta_t`] / [`initial_delta_t`]: `setDeltaT.C`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
#[cfg(feature = "std")]
use std::path::Path;

use outram_foam_basic_lib::mesh::FvMesh;
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
    mut map: M,
    mut correct: C,
    mut residual_of: R,
) -> LoopOutcome
where
    M: FnMut(),
    C: FnMut(usize, bool),
    R: FnMut(usize) -> f64,
{
    let mut iter_n = 0usize;
    let mut residual = 0.0_f64;
    loop {
        map();
        for (i, &only_energy) in correct_only_energy.iter().enumerate() {
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
                return Err(format!("cell {c}: volumes differ; the general cellVolumeWeight supermesh is not ported (gh:#651)"));
            }
            if src.cell_centres[c] != tgt.cell_centres[c] {
                return Err(format!("cell {c}: centres differ; the general cellVolumeWeight supermesh is not ported (gh:#651)"));
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
}
