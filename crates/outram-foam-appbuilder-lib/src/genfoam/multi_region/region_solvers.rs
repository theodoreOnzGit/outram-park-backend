// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: GeN-Foam/GeN-Foam.C (the main loop), src/classes/multiRegion/regionSolvers/regionSolvers.C,
//     src/classes/multiRegion/meshHandler/{meshHandler.C (mapAllFields, createBaffleLessMeshes), meshHandlerTemplates.C (map)},
//     GeN-Foam/include/setDeltaT.C, src/classes/neutronics/diffusion/diffusionNeutronics.C (correctPhysics, maxDeltaT),
//     src/classes/thermoMechanics/{thermoMechanics.C (TStructFromTH, TFuel defaults, correctBaffleLessFields),
//     legacyThermoMechanics/legacyThermoMechanics.C (maxDeltaT)},
//     src/classes/thermalHydraulics/solvers/{thermalHydraulicsModel.C (correctBaffleLessFields), onePhase/onePhase.C (maxDeltaT)}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `regionSolvers` + the `GeN-Foam.C` main loop for the three-region `2D_fullCoupling` case (gh:#1915, #1914, #1475; README round 57)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! One time step of `GeN-Foam.C` (no `multiPhysicsSolver` is configured in this case: every region is its own solver):
//!
//! 1. `runTime++` (`time += deltaT`);
//! 2. `solvers.mapper().mapAllFields(runTime)`: for target region `i` and source region `j` in `regionSolvers` order, each
//!    `sourceFields[k]` onto `targetFields[k]` by `meshToMesh::mapTgtToSrc(.., plusEqOp, ..)` (round 28's exact operator),
//!    for a target field that exists; a source of a region in `removeBaffles` is its `.baffleLess` copy;
//! 3. per solver in order: `deformMesh()`, `correctPhysics()`, `correctBaffleLessFields()`;
//! 4. `adjustDeltaT` (`setDeltaT.C`): the smallest `maxDeltaT()` of the solvers, at most 1.2 `deltaT`.
//!
//! The regions are [`RegionSolver`] variants (enum dispatch, no `dyn`): the cell-centred `onePhase` kernel
//! ([`OnePhaseCcSolver`]), `diffusionNeutronics` on its deformable mesh ([`DiffusionNeutronics`] + [`DeformableMesh`], README round 56) and
//! `legacyThermoMechanics` ([`LegacyFvmThermoMechanics`], rounds 23-27).
//!
//! ## Deliberately not ported here (refused by name)
//!
//! * a `regionSolvers` entry other than `onePhase`, `diffusionNeutronics`, `legacyThermoMechanics`;
//! * ~~`eigenvalueNeutronics false` (the transient with `integralPredictor`: gh:#1916/#1919)~~ **CHANGED 2026-11-27 (gh:#1919, README round 60)**: ported as
//!   [`RegionSolvers::restart_transient`] (a restart of the steady state into `solveNeutronics.H` with `eigenvalueNeutronics false`, `integralPredictor.H`
//!   included); ~~`read` of a case that STARTS with `eigenvalueNeutronics false` is still refused (its `fluxStar_`/`prec_` would have to be read from a
//!   start directory)~~ **CHANGED 2026-11-27 (gh:#1926, README round 66)**: `read` of such a case is ported (the regions are constructed from the written start
//!   directory; the in-memory restart stays callable); ~~`regionSolvers.neutroRegion pointKinetics` (the PK variant) is refused by name (gh:#1922)~~ **CHANGED 2026-11-27 (gh:#1922, README
//!   round 61)**: ported as [`RegionSolver::PointKinetics`], reached only through [`RegionSolvers::restart_transient`];
//! * `removeBaffles` on a region whose mesh HAS baffles (two faces on the same points; gh:#1236). For a mesh without any, upstream's
//!   baffle-less mesh is the same mesh (`Writing 0 duplicate faces`) and `correctBaffleLessFields` is a copy of the primitive field: the
//!   `.baffleLess` source is then the region's own field as of the end of its last `correctPhysics()`;
//! * `-initializeMappedFields`: upstream's mapped fields are `NO_READ` (TM) or `READ_IF_PRESENT` with default values (neutronics,
//!   fluid), so a run from the SHIPPED `0/` starts from the constructors' defaults, which are used here (TM `TStructFromTH = TFuel = 1`,
//!   `powerDensityNeutronics = 0`; neutronics `TFuel = TClad = TCool = TStruct = 0`, `disp = 0`; fluid `powerDensityNeutronics = 0`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;
use std::path::Path;

use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::FoamEntry;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

use super::cell_overlap::OverlapMesh;
use super::coupled_loop::{adjust_delta_t, CouplingDict, MappingEntry, RegionMeshMap};
use super::mesh_deformation::{DeformableMesh, DispPatchCondition, MeshDeformationDict, PointCellsOrder, StepClock, VolDisplacement};
use crate::error::AppBuilderError;
use crate::genfoam::neutronics::diffusion::external_source::{NeutronicsDdtScheme, NeutronicsLaplacianScheme, StationaryStep, StationaryStepControls};
use crate::genfoam::neutronics::diffusion::upstream_transient::diffusion_max_delta_t;
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::point_kinetics::case::{read_pk_case, PkCaseDicts};
use crate::genfoam::neutronics::point_kinetics::model::{feedback_cell_field, PkFeedbackFields, PkMeshData, PkStepReport, PointKineticNeutronics};
use crate::genfoam::neutronics::point_kinetics::LinearSolveForm;
use crate::genfoam::neutronics::pseudo_time::{PseudoTimeClock, PseudoTimeControls, PseudoTimeEigenvalue};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use crate::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use crate::genfoam::thermo_mechanics::legacy_fvm::{LegacyFvmThermoMechanics, PhysicsLog};
use crate::genfoam::workflow::reactor_state::ReactorState;
use crate::io::nuclear_data::read_nuclear_data;
use crate::io::field_reader::{read_vol_scalar_field, read_vol_vector_field};
use crate::io::poly_mesh::{parse_points, read_cell_zones, read_face_zones, zone_of_cell};

/// `GREAT` of OpenFOAM (`doubleScalarGreat`), the `maxDeltaT` of a solver that has none.
const GREAT: f64 = 1.0e15;

fn case_err(root: &Path, msg: impl Into<String>) -> AppBuilderError {
    AppBuilderError::Case { case: root.to_path_buf(), msg: msg.into() }
}

/// The named volume fields one region holds for the mapper: cell values of scalar and vector `volField`s (the boundary values follow
/// the owner cell for the `zeroGradient` fields of this case, so they are not stored).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RegionFields {
    scalars: Vec<(String, Vec<f64>)>,
    vectors: Vec<(String, Vec<Vector3>)>,
}

impl RegionFields {
    /// A scalar field, if the region has it (`foundObject`).
    #[must_use]
    pub fn scalar(&self, name: &str) -> Option<&[f64]> {
        self.scalars.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice())
    }

    /// A vector field, if the region has it.
    #[must_use]
    pub fn vector(&self, name: &str) -> Option<&[Vector3]> {
        self.vectors.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice())
    }

    /// Create or replace a scalar field.
    pub fn set_scalar(&mut self, name: &str, values: Vec<f64>) {
        match self.scalars.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = values,
            None => self.scalars.push((name.to_string(), values)),
        }
    }

    /// Create or replace a vector field.
    pub fn set_vector(&mut self, name: &str, values: Vec<Vector3>) {
        match self.vectors.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = values,
            None => self.vectors.push((name.to_string(), values)),
        }
    }

    fn scalar_mut(&mut self, name: &str) -> Option<&mut Vec<f64>> {
        self.scalars.iter_mut().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    fn vector_mut(&mut self, name: &str) -> Option<&mut Vec<Vector3>> {
        self.vectors.iter_mut().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// The cell-centred `onePhase` fluid region.
#[derive(Debug, Clone)]
pub struct FluidRegion {
    /// The solver.
    pub solver: OnePhaseCcSolver,
}

/// `diffusionNeutronics` with its deformable mesh (`eigenvalueNeutronics true`).
#[derive(Debug, Clone)]
pub struct NeutronicsRegion {
    /// The mesh `deformMesh()` moves.
    pub mesh: DeformableMesh,
    /// The diffusion model bound to the (moved) mesh.
    pub solver: DiffusionNeutronics,
    /// `pTarget`.
    pub p_target: f64,
    /// `keff_` of the last step.
    pub k_eff: f64,
    /// `power()` of the last step.
    pub power: f64,
    /// `pTotOld_` (`power()` at the start of the last `correctPhysics()`).
    pub p_tot_old: f64,
    first_step: bool,
    conditions: Vec<DispPatchCondition>,
    /// `Some` once [`RegionSolvers::restart_transient`] switched the region to `eigenvalueNeutronics false` (gh:#1919, README round 60).
    pub transient: Option<NeutronicsTransient>,
}

/// The neutronics region in `eigenvalueNeutronics false` mode: `solveNeutronics.H` as [`DiffusionNeutronics::upstream_stationary_step`]
/// (`liquidFuel false`, `Euler`, `Gauss linear corrected`) with `integralPredictor.H` when `fvSolution` selects it.
#[derive(Debug, Clone)]
pub struct NeutronicsTransient {
    /// The step controls read from `fvSchemes`/`fvSolution`.
    pub controls: StationaryStepControls,
    /// The report of the last step (power, neutron iterations, residuals, the integral predictor's numbers).
    pub last_step: Option<StationaryStep>,
    /// `neutronTransport.aitkenAcceleration true`: a step that reaches a third neutron iteration is refused by name (gh:#1932).
    pub aitken_acceleration: bool,
}

/// `pointKineticNeutronics` on its (moved, no longer deforming) mesh: the neutronics region of the PK variant of `2D_fullCoupling`
/// (`regionSolvers.neutroRegion pointKinetics`, gh:#1922, README round 61).
#[derive(Debug, Clone)]
pub struct PointKineticRegion {
    /// The neutronics mesh as the restart reads it (the moved points of the steady state).
    pub mesh: DeformableMesh,
    /// The class: constructed from the READ `flux0`, `flux1`, `powerDensity`, `secondaryPowerDensity`, `prec0..5` ([`PointKineticNeutronics::from_read_fields`]).
    pub pk: PointKineticNeutronics,
    /// The report of the last `correctPhysics()`: the weighted averages and every reactivity term (`pointKineticsInfo.H`).
    pub last: Option<PkStepReport>,
    /// GEM (`GEMFlowFaceZone` of the fluid region `fluidRegionName`), when the case gives a `GEMReactivityMap` (gh:#1927).
    pub gem: Option<GemFaces>,
}

/// The faces `calcGEMLevelAndReactivity` sums `phi*|Sf|` over (`phiFaces_`, `phiMagSf_`), and the region whose `phi` it reads.
#[derive(Debug, Clone, PartialEq)]
pub struct GemFaces {
    /// Index of the fluid region (`fluidRegionName`).
    pub fluid_region: usize,
    /// `(face label, |Sf|)` in zone order.
    pub faces: Vec<(usize, f64)>,
}

/// `legacyThermoMechanics` (finite-volume form).
#[derive(Debug, Clone)]
pub struct ThermoMechanicsRegion {
    /// The solver.
    pub solver: LegacyFvmThermoMechanics,
    /// The log of the last `correctPhysics()`.
    pub last_log: Option<PhysicsLog>,
}

/// One region's solver: enum dispatch over the three models of the case (`regionSolvers` entries).
#[derive(Debug, Clone)]
pub enum RegionSolver {
    /// `onePhase`.
    OnePhase(FluidRegion),
    /// `diffusionNeutronics`.
    Neutronics(NeutronicsRegion),
    /// `legacyThermoMechanics`.
    ThermoMechanics(ThermoMechanicsRegion),
    /// `pointKinetics` (only after [`RegionSolvers::restart_transient`] of a case whose `regionSolvers.neutroRegion` is `pointKinetics`).
    PointKinetics(PointKineticRegion),
}

/// A region: name, mesh for the mapper, fields, solver.
#[derive(Debug, Clone)]
pub struct Region {
    /// Region name (`fluidRegion`, ...).
    pub name: String,
    /// The mesh the mapper addresses (the ORIGINAL cells: `meshToMesh` addressing is built once, `meshHandler.C:192-201`).
    pub fv: Arc<FvMesh>,
    /// Fields the mapper reads and writes.
    pub fields: RegionFields,
    /// The solver.
    pub solver: RegionSolver,
    /// `removeBaffles` for this region (the sources are then its `.baffleLess` copies).
    pub remove_baffles: bool,
}

/// Which couplings the run applies. All on is upstream; each off is a control (README round 57).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CouplingSwitches {
    /// `(target region, target field)` pairs whose mapping is skipped.
    pub disabled_targets: Vec<(String, String)>,
    /// `deformMesh()` of the regions listed in `meshDeformation` is skipped.
    pub no_mesh_deformation: bool,
    /// CONTROL of [`RegionSolvers::restart_transient`] (README round 60, control d): keep the face flux `phi` of the last step instead of
    /// rebuilding it from `U`, as an uninterrupted run would.
    pub restart_keeps_phi: bool,
    /// CONTROL of [`RegionSolvers::restart_transient`] (control b): keep the neutronics mesh at the ORIGINAL points instead of the moved
    /// ones a restart reads.
    pub restart_on_original_mesh: bool,
    /// CONTROL of [`RegionSolvers::restart_transient`] (control a): `integralPredictor false`.
    pub no_integral_predictor: bool,
}

impl CouplingSwitches {
    /// Skip every mapping into `target_region.target_field`.
    #[must_use]
    pub fn without_mapping_into(mut self, target_region: &str, target_field: &str) -> Self {
        self.disabled_targets.push((target_region.to_string(), target_field.to_string()));
        self
    }

    /// Skip `deformMesh()`.
    #[must_use]
    pub fn without_mesh_deformation(mut self) -> Self {
        self.no_mesh_deformation = true;
        self
    }

    /// Restart without rebuilding `phi` from `U` (control d of round 60).
    #[must_use]
    pub fn with_restart_keeping_phi(mut self) -> Self {
        self.restart_keeps_phi = true;
        self
    }

    /// Restart on the original neutronics points (control b of round 60).
    #[must_use]
    pub fn with_restart_on_original_mesh(mut self) -> Self {
        self.restart_on_original_mesh = true;
        self
    }

    /// Run the transient with `integralPredictor false` (control a of round 60).
    #[must_use]
    pub fn without_integral_predictor(mut self) -> Self {
        self.no_integral_predictor = true;
        self
    }
}

/// What one step did (for the comparison).
#[derive(Debug, Clone)]
pub struct StepRecord {
    /// 1-based step.
    pub step: usize,
    /// `runTime.value()` after `runTime++`.
    pub time: f64,
    /// The `deltaT` this step used.
    pub dt: f64,
    /// `keff_`.
    pub k_eff: f64,
    /// `power:`.
    pub power: f64,
    /// `Max sigmaEq` of the thermo-mechanics, if computed.
    pub max_sigma_eq: Option<f64>,
    /// The neutronics report of a transient step (`eigenvalueNeutronics false`), else `None`.
    pub neutronics: Option<StationaryStep>,
}

/// The main loop of `GeN-Foam.C` over the regions of a case.
#[derive(Debug, Clone)]
pub struct RegionSolvers {
    /// The regions in `regionSolvers` order.
    pub regions: Vec<Region>,
    maps: Vec<(usize, usize, RegionMeshMap)>,
    mappings: Vec<MappingEntry>,
    deformation: MeshDeformationDict,
    switches: CouplingSwitches,
    control_delta_t: f64,
    max_delta_t: f64,
    min_delta_t: f64,
    max_power_variation: f64,
    adjust_time_step: bool,
    end_time: f64,
    time: f64,
    dt: f64,
    step: usize,
    /// `runTime` (`Time::operator++`, `setDeltaT`, `run`): the clock of every PseudoTime loop, with `adjustableRunTime` write alignment.
    clock: PseudoTimeClock,
    /// The `OverlapMesh` geometry of each region's mesh as the maps were built (the neutronics one is rebuilt at a restart).
    geometries: Vec<OverlapMesh>,
    /// `controlDict` `writeControl adjustableRunTime` `writeInterval`, `None` for any other write control.
    write_interval: Option<f64>,
}

/// The `PseudoTimeControls` a `controlDict` gives (`pTarget` is not read by the loop).
fn clock_controls(c: &outram_foam_basic_lib::io::dict::FoamDict, root: &Path) -> Result<PseudoTimeControls, AppBuilderError> {
    let get = |k: &str| c.get_scalar(k);
    let write_interval = match c.get_word("writeControl") {
        Some(w) if w == "adjustableRunTime" || w == "adjustable" => {
            Some(get("writeInterval").ok_or_else(|| case_err(root, "controlDict: writeControl adjustableRunTime without writeInterval"))?)
        }
        _ => None,
    };
    Ok(PseudoTimeControls {
        delta_t: get("deltaT").ok_or_else(|| case_err(root, "controlDict: no deltaT"))?,
        end_time: get("endTime").ok_or_else(|| case_err(root, "controlDict: no endTime"))?,
        write_interval,
        adjust_time_step: c.get_word("adjustTimeStep").is_some_and(|w| w == "true" || w == "yes" || w == "on"),
        max_delta_t: get("maxDeltaT").unwrap_or(GREAT),
        min_delta_t: get("minDeltaT").unwrap_or(1e-300),
        max_power_variation: get("maxPowerVariation").unwrap_or(0.025),
        p_target: 0.0,
    })
}

/// `writeControl timeStep`: `writeInterval` as a step count (`Time::operator++`), else `None` (gh:#1931).
fn write_every_steps(c: &outram_foam_basic_lib::io::dict::FoamDict) -> Option<usize> {
    match c.get_word("writeControl") {
        Some("timeStep") => c.get_scalar("writeInterval").map(|v| v as usize),
        _ => None,
    }
}

/// `meshHandler`: one `meshToMesh` per (target, source) pair of the mappings dictionary, built on the meshes as they are now.
fn build_maps(regions: &[Region], mappings: &[MappingEntry], geometries: &[OverlapMesh], root: &Path) -> Result<Vec<(usize, usize, RegionMeshMap)>, AppBuilderError> {
    let mut maps = Vec::new();
    for m in mappings {
        let (Some(ti), Some(si)) = (regions.iter().position(|r| r.name == m.target_region), regions.iter().position(|r| r.name == m.source_region)) else {
            continue;
        };
        let map = RegionMeshMap::new(&regions[si].fv, &regions[ti].fv, Some((&geometries[si], &geometries[ti]))).map_err(|e| case_err(root, e))?;
        maps.push((ti, si, map));
    }
    Ok(maps)
}

/// The name of the time directory of `root` whose value is `t` (`Time::timeName`: `100` for 100, `99.958333...` as written).
fn time_dir_name(root: &Path, t: f64) -> Result<String, AppBuilderError> {
    let entries = std::fs::read_dir(root).map_err(|e| case_err(root, format!("{e}")))?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.parse::<f64>().is_ok_and(|v| v == t) {
            return Ok(name);
        }
    }
    Err(case_err(root, format!("no time directory for t = {t}")))
}

fn read_dict(root: &Path, rel: &str) -> Result<FoamFile, AppBuilderError> {
    FoamFile::read_esi(root.join(rel)).map_err(|e| case_err(root, format!("{rel}: {e}")))
}

fn has_baffles(mesh: &DeformableMesh) -> bool {
    let mut seen: Vec<Vec<usize>> = mesh
        .topology()
        .faces()
        .iter()
        .map(|f| {
            let mut s = f.clone();
            s.sort_unstable();
            s
        })
        .collect();
    seen.sort();
    seen.windows(2).any(|w| w[0] == w[1])
}

/// The step controls of `solveNeutronics.H` with `eigenvalueNeutronics false` read from region `nname`'s `fvSolution` (`neutronTransport`) and
/// `fvSchemes`; and whether `integralPredictor.H` runs. `no_integral_predictor` is the CONTROL of round 60.
///
/// # Errors
/// A laplacian or ddt scheme other than `Gauss linear corrected` and `Euler`: refused by name (~~`implicitPredictor true`~~: **CHANGED 2026-11-27 (gh:#1926)**, ported as `implicitPredictor.H`).
fn neutronics_transient_controls(root: &Path, nname: &str, no_integral_predictor: bool) -> Result<(StationaryStepControls, bool, bool), AppBuilderError> {
    let fvs = read_dict(root, &format!("system/{nname}/fvSolution"))?;
    let nt = fvs.dict.get_dict("neutronTransport");
    let flag = |k: &str| nt.and_then(|d| d.get_word(k)).is_some_and(|w| w == "true" || w == "yes" || w == "on");
    // `implicitPredictor.H` is ported (gh:#1926, README round 66); `aitkenAcceleration.H` is not (its lists are uninitialised upstream)
    let implicit_predictor = flag("implicitPredictor");
    let integral_predictor = flag("integralPredictor") && !no_integral_predictor;
    let res = nt.and_then(|d| d.get_scalar("neutronIterationResidual")).unwrap_or(1e-5);
    let max_it = nt.and_then(|d| d.get_scalar("maxNeutronIterations")).unwrap_or(10.0) as usize;
    let schemes = std::fs::read_to_string(root.join(format!("system/{nname}/fvSchemes"))).map_err(|e| case_err(root, format!("fvSchemes: {e}")))?;
    let laplacian = if schemes.contains("Gauss linear corrected;") {
        NeutronicsLaplacianScheme::Corrected
    } else {
        return Err(case_err(root, "fvSchemes: only `laplacianSchemes default Gauss linear corrected` is ported for the transient step"));
    };
    if !schemes.contains("Euler;") {
        return Err(case_err(root, "fvSchemes: only `ddtSchemes default Euler` is ported for the transient step"));
    }
    Ok((StationaryStepControls { ddt: NeutronicsDdtScheme::Euler, laplacian, neutron_iteration_residual: res, max_neutron_iterations: max_it }, integral_predictor, implicit_predictor))
}

/// `fvSolution` `neutronTransport.aitkenAcceleration` (default false). The shipped `2D_fullCoupling` sets it `true`, and with one neutron iteration per
/// step upstream's `aitkenAcceleration.H` does not change `fluxStar_` (it acts from the third iteration), so the flag alone is not refused: the
/// STEP that reaches a third neutron iteration is (see [`RegionSolvers::step`]; gh:#1932, maintainer decision 2026-11-27).
fn aitken_requested(root: &Path, nname: &str) -> Result<bool, AppBuilderError> {
    let fvs = read_dict(root, &format!("system/{nname}/fvSolution"))?;
    Ok(fvs.dict.get_dict("neutronTransport").and_then(|d| d.get_word("aitkenAcceleration")).is_some_and(|w| w == "true" || w == "yes" || w == "on"))
}

/// `neutronicsProperties.eigenvalueNeutronics` of a neutronics region of kind `kind` is `false` (a point-kinetics region always is): the
/// region is constructed from the WRITTEN fields of the start directory (gh:#1926, README round 66).
fn neutronics_starts_from_files(root: &Path, name: &str, kind: &str) -> Result<bool, AppBuilderError> {
    match kind {
        "pointKinetics" => Ok(true),
        "diffusionNeutronics" => {
            let np = read_dict(root, &format!("constant/{name}/neutronicsProperties"))?;
            Ok(!np.dict.get_word("eigenvalueNeutronics").is_some_and(|w| w == "true" || w == "yes" || w == "on"))
        }
        _ => Ok(false),
    }
}

/// `<time>/<region>/polyMesh/points` if the written directory holds it (the moved points of a deformed mesh; `polyMesh` reads `points` from
/// the latest time directory that has them, `faces` and the rest from `constant`).
fn read_start_points(root: &Path, region: &str, time_dir: &str) -> Result<Option<Vec<Vector3>>, AppBuilderError> {
    let path = root.join(time_dir).join(region).join("polyMesh/points");
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| case_err(root, format!("{}: {e}", path.display())))?;
    let pts = parse_points(&text, "points")?;
    Ok(Some(pts.iter().map(|p| Vector3::new(p[0], p[1], p[2])).collect()))
}

/// The fields of the neutronics region a written directory holds (`createNeutronicsFields.H`, all `READ_IF_PRESENT`; the fluxes and
/// precursors are required here because the `defaultFlux`/`defaultPrec` fall-back of an absent file is not ported for a written start).
struct NeutronicsFiles {
    flux: Vec<Vec<f64>>,
    flux_star: Vec<Vec<f64>>,
    prec: Vec<Vec<f64>>,
    scalars: Vec<(String, Vec<f64>)>,
    disp: Option<Vec<Vector3>>,
}

fn read_neutronics_files(root: &Path, region: &str, time_dir: &str, n: usize, groups: usize, prec_groups: usize, need_star: bool) -> Result<NeutronicsFiles, AppBuilderError> {
    let dir = root.join(time_dir).join(region);
    let list = |prefix: &str, count: usize, required: bool| -> Result<Vec<Vec<f64>>, AppBuilderError> {
        let mut out = Vec::new();
        for i in 0..count {
            let path = dir.join(format!("{prefix}{i}"));
            if !path.is_file() {
                if required {
                    return Err(case_err(root, format!("{}: not in the written start directory (the default field of an absent file is not ported)", path.display())));
                }
                return Ok(Vec::new());
            }
            out.push(read_vol_scalar_field(&path, n)?);
        }
        Ok(out)
    };
    let flux = list("flux", groups, true)?;
    let flux_star = list("fluxStar", groups, need_star)?;
    let prec = list("prec", prec_groups, true)?;
    let mut scalars = Vec::new();
    for name in ["TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool", "powerDensity", "secondaryPowerDensity"] {
        let path = dir.join(name);
        if path.is_file() {
            scalars.push((name.to_string(), read_vol_scalar_field(&path, n)?));
        }
    }
    let disp_path = dir.join("disp");
    let disp = if disp_path.is_file() { Some(read_vol_vector_field(&disp_path, n)?) } else { None };
    Ok(NeutronicsFiles { flux, flux_star, prec, scalars, disp })
}

/// `pointKineticNeutronics`'s constructor from the READ fields (`pointKineticNeutronics.C:62-905`): the case dictionaries of region `nname`
/// (`nuclearData`, `neutronicsProperties`, `externalSource`, `<start>/uniform/reactorState`), the feedback cell fields from its `cellZones`
/// (`setFeedbackCellField`: a missing zone is upstream's FatalError), the fluxes, power densities and precursors as read. Returns the class and
/// the GEM request, if any (its faces need the fluid region: [`RegionSolvers::gem_faces`]).
///
/// # Errors
/// [`AppBuilderError`] naming what is refused or cannot be read.
#[allow(clippy::too_many_arguments)]
fn build_point_kinetics(
    root: &Path,
    nname: &str,
    start_time: f64,
    fresh: &DeformableMesh,
    fluxes: Vec<Vec<f64>>,
    prec: Vec<Vec<f64>>,
    power_density: Vec<f64>,
    secondary: Vec<f64>,
) -> Result<(PointKineticNeutronics, Option<crate::genfoam::neutronics::point_kinetics::case::GemRequest>), AppBuilderError> {
    let np = read_dict(root, &format!("constant/{nname}/neutronicsProperties"))?;
    if np.dict.get_word("liquidFuel").is_some_and(|w| w == "true" || w == "yes" || w == "on") {
        return Err(case_err(root, "neutronicsProperties.liquidFuel true: the liquid-fuel precursor transport (gh:#1882) is not wired into the loop (gh:#1905)"));
    }
    let nd = read_dict(root, &format!("constant/{nname}/nuclearData"))?;
    let es_rel = format!("constant/{nname}/externalSource");
    let es = if root.join(&es_rel).is_file() { Some(read_dict(root, &es_rel)?) } else { None };
    // `reactorState` of the start directory (`neutronics.C:79-90`: `<time>/uniform/reactorState`)
    let rs_rel = format!("{}/uniform/reactorState", time_dir_name(root, start_time)?);
    let rs = if root.join(&rs_rel).is_file() { Some(read_dict(root, &rs_rel)?) } else { None };
    let dicts = PkCaseDicts { nuclear: &nd.dict, neutronics_properties: &np.dict, external_source: es.as_ref().map(|f| &f.dict), reactor_state: rs.as_ref().map(|f| &f.dict) };
    let case = read_pk_case(&dicts, start_time).map_err(|e| case_err(root, e))?;
    let zones = read_cell_zones(&root.join(format!("constant/{nname}/polyMesh")))?;
    let zone_cells = |names: &[String]| -> Result<Vec<Vec<usize>>, AppBuilderError> {
        names
            .iter()
            .map(|n| zones.iter().find(|z| &z.name == n).map(|z| z.cells.clone()).ok_or_else(|| case_err(root, format!("{nname}: feedback zone {n} is not a cellZone"))))
            .collect()
    };
    let cfg = case.cfg.clone();
    let cells = fresh.mesh().n_cells;
    let mesh = PkMeshData {
        volume: fresh.mesh().cell_volumes.clone(),
        fuel: feedback_cell_field(cells, &zone_cells(&case.zones.fuel)?),
        cool: feedback_cell_field(cells, &zone_cells(&case.zones.cool)?),
        structure: feedback_cell_field(cells, &zone_cells(&case.zones.structure)?),
        structure_mech: feedback_cell_field(cells, &zone_cells(&case.zones.structure_mech)?),
        driveline: feedback_cell_field(cells, &zone_cells(&case.zones.driveline)?),
    };
    let mut pk = PointKineticNeutronics::from_read_fields(cfg, mesh, fluxes, power_density, secondary, prec, LinearSolveForm::default())
        .map_err(|e| case_err(root, format!("{nname}: pointKineticNeutronics: {e}")))?
        .with_read_references(case.refs);
    if let Some(g) = &case.gem {
        pk = pk.with_gem(g.map.clone(), g.sodium_level_ref).map_err(|e| case_err(root, format!("{nname}: GEM: {e}")))?;
    }
    Ok((pk, case.gem))
}

mod written;
pub use written::WrittenState;

impl RegionSolvers {
    /// The faces `calcGEMLevelAndReactivity` sums over (`GEMFlowFaceZone` of the fluid region `fluidRegionName`; `magSf = mag(THMesh.faceAreas())`).
    ///
    /// # Errors
    /// [`AppBuilderError`] if the fluid region or the face zone does not exist.
    fn gem_faces(&self, root: &Path, g: &crate::genfoam::neutronics::point_kinetics::case::GemRequest) -> Result<GemFaces, AppBuilderError> {
        let fi = self.regions.iter().position(|r| r.name == g.fluid_region && matches!(r.solver, RegionSolver::OnePhase(_)));
        let Some(fi) = fi else {
            return Err(case_err(root, format!("GEM: fluidRegionName {} is not a onePhase region of this case", g.fluid_region)));
        };
        let fzs = read_face_zones(&root.join(format!("constant/{}/polyMesh", g.fluid_region)))?;
        let Some(fz) = fzs.iter().find(|z| z.name == g.face_zone) else {
            return Err(case_err(root, format!("GEM: GEMFlowFaceZone {} is not a faceZone of {}", g.face_zone, g.fluid_region)));
        };
        let fv = &self.regions[fi].fv;
        let faces: Vec<(usize, f64)> = fz.faces.iter().map(|&f| (f, fv.face_area_vectors[f].mag())).collect();
        Ok(GemFaces { fluid_region: fi, faces })
    }
}

impl RegionSolvers {
    /// Read the case: `system/controlDict` (`regionSolvers`, `removeBaffles`, the time controls), `constant/multiRegionCouplingDict`
    /// (`mappings`, `meshDeformation`) and each region's files, as `regionSolvers(runTime)` and `meshHandler` do.
    ///
    /// # Errors
    /// [`AppBuilderError`] naming the first thing the port does not implement (see the module documentation) or cannot read.
    #[allow(clippy::too_many_lines)]
    pub fn read(root: &Path, switches: CouplingSwitches) -> Result<Self, AppBuilderError> {
        let ctl = read_dict(root, "system/controlDict")?;
        let c = &ctl.dict;
        let get = |k: &str| c.get_scalar(k);
        let control_delta_t = get("deltaT").ok_or_else(|| case_err(root, "controlDict: no deltaT"))?;
        let end_time = get("endTime").ok_or_else(|| case_err(root, "controlDict: no endTime"))?;
        let adjust_time_step = c.get_word("adjustTimeStep").is_some_and(|w| w == "true" || w == "yes" || w == "on");
        let rs = c.get_dict("regionSolvers").ok_or_else(|| case_err(root, "controlDict: no regionSolvers"))?;
        let mut names: Vec<(String, String)> = Vec::new();
        for (k, v) in rs.iter() {
            match v {
                FoamEntry::Word(w) => names.push((k.to_string(), w.clone())),
                _ => return Err(case_err(root, format!("regionSolvers.{k} is not a word"))),
            }
        }
        let rb = |name: &str| -> bool { c.get_dict("removeBaffles").and_then(|d| d.get_word(name)).is_some_and(|w| w == "true" || w == "yes" || w == "on") };
        let coupling = read_dict(root, "constant/multiRegionCouplingDict")?;
        let mappings = CouplingDict::mappings_from_dict(&coupling.dict, root)?;
        let deformation = MeshDeformationDict::from_coupling_dict(&coupling.dict).map_err(|e| case_err(root, format!("meshDeformation: {e}")))?;

        let mut regions = Vec::new();
        let mut geometries: Vec<OverlapMesh> = Vec::new();
        let start_time = get("startTime").unwrap_or(0.0);
        // GEM faces of a point-kinetics region built from files: they need the fluid region, so they are attached after the loop
        let mut gem_requests: Vec<(usize, crate::genfoam::neutronics::point_kinetics::case::GemRequest)> = Vec::new();
        for (name, kind) in &names {
            let poly = root.join(format!("constant/{name}/polyMesh"));
            let mut dm = DeformableMesh::read(&poly, PointCellsOrder::CellAscending).map_err(|e| case_err(root, format!("{name}: {e}")))?;
            // a neutronics region with `eigenvalueNeutronics false` (or point kinetics) is constructed from the written start directory: its
            // mesh is a FRESH mesh at the points `<t>/<region>/polyMesh/points` (gh:#1926, README round 66; `restart_transient` does the same
            // from the port's own state)
            let from_files = neutronics_starts_from_files(root, name, kind)?;
            let time_dir = if from_files { Some(time_dir_name(root, start_time)?) } else { None };
            if let Some(td) = &time_dir {
                let points = match read_start_points(root, name, td)? {
                    Some(p) => p,
                    None => dm.points().to_vec(),
                };
                dm = dm.restarted_at(&points).map_err(|e| case_err(root, format!("{name}: restart mesh: {e}")))?;
            }
            let remove_baffles = rb(name);
            if remove_baffles && has_baffles(&dm) {
                return Err(case_err(root, format!("{name}: removeBaffles on a mesh with baffles is not ported (gh:#1236); only a mesh without duplicate faces is")));
            }
            geometries.push(OverlapMesh::from_deformable(&dm).map_err(|e| case_err(root, format!("{name}: overlap geometry: {e}")))?);
            let n = dm.mesh().n_cells;
            let fv = dm.mesh().clone();
            let mut fields = RegionFields::default();
            let solver = match kind.as_str() {
                "onePhase" => {
                    let s = OnePhaseCcSolver::from_case_dir(root, &poly)?;
                    fields.set_scalar("powerDensityNeutronics", vec![0.0; n]);
                    fields.set_scalar("powerDensityNeutronicsToLiquid", vec![0.0; n]);
                    RegionSolver::OnePhase(FluidRegion { solver: s })
                }
                "diffusionNeutronics" => {
                    let np = read_dict(root, &format!("constant/{name}/neutronicsProperties"))?;
                    let eig = np.dict.get_word("eigenvalueNeutronics").is_some_and(|w| w == "true" || w == "yes" || w == "on");
                    // `neutronics.C`: `keff_` and `pTarget_` come from `<startTime>/uniform/reactorState` (the real log: `Initial keff = 1.003457`)
                    let rs_dir = root.join(time_dir.as_deref().unwrap_or("0"));
                    let rs = ReactorState::read(&rs_dir).map_err(|e| case_err(root, format!("{}/uniform/reactorState: {e}", rs_dir.display())))?;
                    let (k0, p_target) = (rs.k_eff, rs.p_target);
                    let input = read_nuclear_data(&root.join(format!("constant/{name}/nuclearData")))?;
                    let xs = CrossSectionData::from_input(&input).map_err(|e| case_err(root, format!("{name}: cross sections: {e}")))?;
                    let zones = read_cell_zones(&poly)?;
                    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).map_or("", |z| z.name())).collect();
                    let zone_map = zone_of_cell(&zones, &order, n)?;
                    // `0/<region>/defaultFlux`: every patch `fixedValue 0` except the `empty` constraint patch (checked against the shipped file in the test)
                    let flux_boundary: Vec<BoundaryCondition<f64>> = dm
                        .mesh()
                        .patches
                        .iter()
                        .map(|p| if p.name == "frontAndBack" { BoundaryCondition::Empty } else { BoundaryCondition::FixedValue(0.0) })
                        .collect();
                    let settings = DiffusionSettings {
                        k_tolerance: 1e-13,
                        flux_tolerance: 1e-12,
                        max_outer_iterations: 1,
                        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
                        ..DiffusionSettings::default()
                    };
                    let mut solver = DiffusionNeutronics::new(dm.mesh().clone(), &xs, &zone_map, &[], &flux_boundary, settings)
                        .map_err(|e| case_err(root, format!("{name}: diffusion solver: {e}")))?
                        .with_eigenvalue_laplacian(NeutronicsLaplacianScheme::Corrected);
                    if eig {
                        solver.set_initial_guess(&vec![vec![1.0; n]; 2], k0);
                    }
                    let conditions = vec![DispPatchCondition::ZeroGradient; dm.mesh().patches.len()];
                    for f in ["TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool"] {
                        fields.set_scalar(f, vec![0.0; n]);
                    }
                    fields.set_scalar("powerDensity", vec![0.0; n]);
                    fields.set_scalar("secondaryPowerDensity", vec![0.0; n]);
                    fields.set_vector("disp", vec![Vector3::ZERO; n]);
                    let (mut transient, mut first_step) = (None, true);
                    if !eig {
                        // `solveNeutronics.H` with `eigenvalueNeutronics false` from the READ `fluxStar<g>`, `flux<g>`, `prec<k>` (`createNeutronicsFields.H:119-215`)
                        let td = time_dir.as_deref().ok_or_else(|| case_err(root, "internal: no start directory"))?;
                        let nf = read_neutronics_files(root, name, td, n, solver.energy_groups(), solver.prec_groups(), true)?;
                        let (controls, integral_predictor, implicit_predictor) = neutronics_transient_controls(root, name, switches.no_integral_predictor)?;
                        solver = solver.with_integral_predictor(integral_predictor).with_implicit_predictor(implicit_predictor);
                        solver.set_read_fields(&nf.flux, &nf.flux_star, &nf.prec, k0);
                        for (nm, v) in nf.scalars {
                            fields.set_scalar(&nm, v);
                        }
                        if let Some(d) = nf.disp {
                            fields.set_vector("disp", d);
                        }
                        transient = Some(NeutronicsTransient { controls, last_step: None, aitken_acceleration: aitken_requested(root, name)? });
                        first_step = false;
                    }
                    RegionSolver::Neutronics(NeutronicsRegion { mesh: dm.clone(), solver, p_target, k_eff: k0, power: 0.0, p_tot_old: p_target, first_step, conditions, transient })
                }
                "pointKinetics" => {
                    // `pointKineticNeutronics` constructed from the written start directory: `flux<g>`, `prec<k>`, `powerDensity`,
                    // `secondaryPowerDensity` READ (`pointKineticNeutronics.C:522-690`), `<t>/uniform/reactorState`
                    let td = time_dir.as_deref().ok_or_else(|| case_err(root, "internal: no start directory"))?;
                    let dir = root.join(td).join(name);
                    let count = |prefix: &str| (0..64).take_while(|i| dir.join(format!("{prefix}{i}")).is_file()).count();
                    let nf = read_neutronics_files(root, name, td, n, count("flux"), count("prec"), false)?;
                    for f in ["TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool"] {
                        fields.set_scalar(f, vec![0.0; n]);
                    }
                    fields.set_scalar("powerDensity", vec![0.0; n]);
                    fields.set_scalar("secondaryPowerDensity", vec![0.0; n]);
                    for (nm, v) in &nf.scalars {
                        fields.set_scalar(nm, v.clone());
                    }
                    let power_density = fields.scalar("powerDensity").map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0; n]);
                    let secondary = fields.scalar("secondaryPowerDensity").map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0; n]);
                    let (pk, gem_request) = build_point_kinetics(root, name, start_time, &dm, nf.flux, nf.prec, power_density, secondary)?;
                    if let Some(g) = gem_request {
                        gem_requests.push((regions.len(), g));
                    }
                    RegionSolver::PointKinetics(PointKineticRegion { mesh: dm.clone(), pk, last: None, gem: None })
                }
                "legacyThermoMechanics" => {
                    // `disp`, `fuelDisp`, `CRDisp` are READ from the start directory (a written one holds them as lists)
                    let tm_dir = if start_time == 0.0 { "0".to_string() } else { time_dir_name(root, start_time)? };
                    let tm = LegacyFvmCase::read_at(root, name, TStructGamgPolicy::SubstitutePbicgDilu, &tm_dir)?.build()?;
                    fields.set_scalar("TStructFromTH", vec![1.0; n]);
                    fields.set_scalar("TFuel", vec![1.0; n]);
                    fields.set_scalar("powerDensityNeutronics", vec![0.0; n]);
                    fields.set_vector("meshDisp", tm.mesh_displacement().to_vec());
                    RegionSolver::ThermoMechanics(ThermoMechanicsRegion { solver: tm, last_log: None })
                }
                other => return Err(case_err(root, format!("regionSolvers.{name} {other}: only onePhase, diffusionNeutronics and legacyThermoMechanics are ported in this loop"))),
            };
            regions.push(Region { name: name.clone(), fv, fields, solver, remove_baffles });
        }
        let maps = build_maps(&regions, &mappings, &geometries, root)?;
        let mut clock = PseudoTimeClock::starting_at(&clock_controls(c, root)?, start_time);
        if let Some(n) = write_every_steps(c) {
            clock = clock.with_write_every_steps(n);
        }
        let write_interval = clock_controls(c, root)?.write_interval;
        let mut me = Self {
            regions,
            maps,
            mappings,
            deformation,
            switches,
            control_delta_t,
            max_delta_t: c.get_scalar("maxDeltaT").unwrap_or(GREAT),
            min_delta_t: c.get_scalar("minDeltaT").unwrap_or(1e-300),
            max_power_variation: c.get_scalar("maxPowerVariation").unwrap_or(0.025),
            adjust_time_step,
            end_time,
            time: clock.value(),
            dt: clock.delta_t(),
            step: 0,
            clock,
            geometries,
            write_interval,
        };
        for (idx, req) in &gem_requests {
            let g = me.gem_faces(root, req)?;
            if let RegionSolver::PointKinetics(p) = &mut me.regions[*idx].solver {
                p.gem = Some(g);
            }
        }
        for i in 0..me.regions.len() {
            me.correct_baffle_less_fields(i);
        }
        Ok(me)
    }


    /// RESTART as `GeN-Foam -case <transient>` does from the written state of this run, in memory (gh:#1919, README round 60): the case
    /// at `root` is the steady-state case staged as the tutorial's `Allrun` stages the transient (`startTime` = the steady state's last time,
    /// `eigenvalueNeutronics false`, `multiRegionCouplingDict_transient`).
    ///
    /// What a restart reads or rebuilds that the running process held differently, each from upstream:
    ///
    /// * the neutronics mesh is READ at the moved points (`<t>/neutroRegion/polyMesh/points`), as a fresh mesh (no `V0`, no old points),
    ///   and `meshToMesh` is built on it (`meshHandler.C:192-201` runs in the constructor);
    /// * the fluid's `phi` is rebuilt from `U` (`fluid.C:509-578`), the PIMPLE state and the lazy old-time copies are new ([`OnePhaseCcSolver::restart_at`]);
    /// * the clock, the controls and the coupling dictionary are those of the transient `controlDict`/`multiRegionCouplingDict`;
    /// * the neutronics region switches to `solveNeutronics.H` with `eigenvalueNeutronics false`: `fluxStar_`, `prec_`, `keff_` are the read ones
    ///   (the state's own), `neutronTransport` of the region's `fvSolution` gives `integralPredictor`, `neutronIterationResidual` and
    ///   `maxNeutronIterations`.
    ///
    /// Everything else (thermo-mechanics fields, the fluid's cell fields) is exactly what the steady state wrote (17 digits).
    ///
    /// # Errors
    /// [`AppBuilderError`] naming what is refused: a `regionSolvers` entry that changes kind (the `pointKinetics` variant is gh:#1922),
    /// ~~`implicitPredictor true`~~ (**CHANGED 2026-11-27, gh:#1926**: ported), a laplacian or ddt scheme other than `Gauss linear corrected` and `Euler`, `eigenvalueNeutronics true`.
    pub fn restart_transient(&mut self, root: &Path) -> Result<(), AppBuilderError> {
        let ctl = read_dict(root, "system/controlDict")?;
        let c = &ctl.dict;
        let cc = clock_controls(c, root)?;
        let start_time = c.get_scalar("startTime").ok_or_else(|| case_err(root, "controlDict: no startTime"))?;
        let rs = c.get_dict("regionSolvers").ok_or_else(|| case_err(root, "controlDict: no regionSolvers"))?;
        for (k, v) in rs.iter() {
            let FoamEntry::Word(w) = v else { return Err(case_err(root, format!("regionSolvers.{k} is not a word"))) };
            let Some(r) = self.regions.iter().find(|r| r.name == k) else {
                return Err(case_err(root, format!("regionSolvers.{k}: the restart has no such region in the steady state")));
            };
            let same = matches!(
                (&r.solver, w.as_str()),
                (RegionSolver::OnePhase(_), "onePhase")
                    | (RegionSolver::Neutronics(_), "diffusionNeutronics" | "pointKinetics")
                    | (RegionSolver::ThermoMechanics(_), "legacyThermoMechanics")
            );
            if !same {
                return Err(case_err(root, format!("regionSolvers.{k} {w}: a restart that changes the solver of a region (other than diffusionNeutronics to pointKinetics) is not ported")));
            }
        }
        let coupling = read_dict(root, "constant/multiRegionCouplingDict")?;
        let mappings = CouplingDict::mappings_from_dict(&coupling.dict, root)?;
        let deformation = MeshDeformationDict::from_coupling_dict(&coupling.dict).map_err(|e| case_err(root, format!("meshDeformation: {e}")))?;
        let Some(ni) = self.regions.iter().position(|r| matches!(r.solver, RegionSolver::Neutronics(_))) else {
            return Err(case_err(root, "the restart needs a diffusionNeutronics region"));
        };
        let nname = self.regions[ni].name.clone();
        let pk_variant = rs.get(&nname).is_some_and(|e| matches!(e, FoamEntry::Word(w) if w == "pointKinetics"));
        let np = read_dict(root, &format!("constant/{nname}/neutronicsProperties"))?;
        if np.dict.get_word("eigenvalueNeutronics").is_none_or(|w| w == "true" || w == "yes" || w == "on") {
            return Err(case_err(root, "restart_transient needs `eigenvalueNeutronics false`"));
        }
        if pk_variant {
            self.restart_transient_point_kinetics(root, ni, &nname, start_time, &cc, mappings, deformation)?;
            if let Some(n) = write_every_steps(c) {
                self.clock = self.clock.with_write_every_steps(n);
            }
            return Ok(());
        }
        let (controls, integral_predictor, implicit_predictor) = neutronics_transient_controls(root, &nname, self.switches.no_integral_predictor)?;

        // the neutronics mesh as the restart reads it
        let keep_original = self.switches.restart_on_original_mesh;
        let RegionSolver::Neutronics(n) = &mut self.regions[ni].solver else { unreachable!("found as Neutronics") };
        let fresh = if keep_original { n.mesh.restarted_at(&n.mesh.original_points().to_vec()) } else { n.mesh.restarted_at_current_points() }
            .map_err(|e| case_err(root, format!("{nname}: restart mesh: {e}")))?;
        n.mesh = fresh.clone();
        n.solver = n.solver.clone().with_integral_predictor(integral_predictor).with_implicit_predictor(implicit_predictor);
        n.solver.rebind_mesh(fresh.mesh().clone(), None).map_err(|e| case_err(root, e.to_string()))?;
        n.first_step = false;
        n.transient = Some(NeutronicsTransient { controls, last_step: None, aitken_acceleration: aitken_requested(root, &nname)? });
        let geometry = OverlapMesh::from_deformable(&fresh).map_err(|e| case_err(root, format!("{nname}: overlap geometry: {e}")))?;
        self.regions[ni].fv = fresh.mesh().clone();
        self.geometries[ni] = geometry;
        let rebuild_phi = !self.switches.restart_keeps_phi;
        for r in self.regions.iter_mut() {
            match &mut r.solver {
                RegionSolver::OnePhase(f) => f.solver.restart_at(start_time, rebuild_phi),
                RegionSolver::ThermoMechanics(t) => t.solver.restart_at(),
                RegionSolver::Neutronics(_) | RegionSolver::PointKinetics(_) => {}
            }
        }
        self.mappings = mappings;
        self.maps = build_maps(&self.regions, &self.mappings, &self.geometries, root)?;
        self.deformation = deformation;
        self.control_delta_t = cc.delta_t;
        self.max_delta_t = cc.max_delta_t;
        self.min_delta_t = cc.min_delta_t;
        self.max_power_variation = cc.max_power_variation;
        self.adjust_time_step = cc.adjust_time_step;
        self.end_time = cc.end_time;
        self.write_interval = cc.write_interval;
        self.clock = PseudoTimeClock::starting_at(&cc, start_time);
        if let Some(n) = write_every_steps(c) {
            self.clock = self.clock.with_write_every_steps(n);
        }
        self.time = self.clock.value();
        self.dt = self.clock.delta_t();
        self.step = 0;
        for i in 0..self.regions.len() {
            self.correct_baffle_less_fields(i);
        }
        Ok(())
    }

    /// The PK variant of [`Self::restart_transient`] (`regionSolvers.neutroRegion pointKinetics`; gh:#1922, README round 61).
    ///
    /// `pointKineticNeutronics`'s constructor at the start time (`pointKineticNeutronics.C:62-905`): `power_ = pTarget` of `reactorState` (the
    /// steady state's `pTarget`), precursor powers in equilibrium (`reactorState` has no `precursorPowers`), `fluxes_` the READ `flux0`, `flux1`
    /// (their sum is `oneGroupFlux_`), `powerDensity_`, `secondaryPowerDensity_` and `prec_` read; the references `T*Ref_` are taken at the first
    /// step (`solvePointKinetics.H`, `timeIndex == startTimeIndex + 1`). The neutronics mesh is the moved one and the maps are rebuilt on it, as
    /// for the diffusion transient; the fluid and the thermo-mechanics restart exactly as there.
    #[allow(clippy::too_many_arguments)]
    fn restart_transient_point_kinetics(
        &mut self,
        root: &Path,
        ni: usize,
        nname: &str,
        start_time: f64,
        cc: &PseudoTimeControls,
        mappings: Vec<MappingEntry>,
        deformation: MeshDeformationDict,
    ) -> Result<(), AppBuilderError> {
        let keep_original = self.switches.restart_on_original_mesh;
        let RegionSolver::Neutronics(n) = &mut self.regions[ni].solver else { unreachable!("found as Neutronics") };
        let fresh = if keep_original { n.mesh.restarted_at(&n.mesh.original_points().to_vec()) } else { n.mesh.restarted_at_current_points() }
            .map_err(|e| case_err(root, format!("{nname}: restart mesh: {e}")))?;
        let cells = fresh.mesh().n_cells;
        let fluxes: Vec<Vec<f64>> = n.solver.state().flux().iter().map(|f| f.internal.as_slice().to_vec()).collect();
        let prec: Vec<Vec<f64>> = n.solver.state().precursors().iter().map(|f| f.internal.as_slice().to_vec()).collect();
        let power_density = self.regions[ni].fields.scalar("powerDensity").map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0; cells]);
        let secondary = self.regions[ni].fields.scalar("secondaryPowerDensity").map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0; cells]);
        let (pk, gem_request) = build_point_kinetics(root, nname, start_time, &fresh, fluxes, prec, power_density, secondary)?;
        let gem_faces = match &gem_request {
            None => None,
            Some(g) => Some(self.gem_faces(root, g)?),
        };
        let geometry = OverlapMesh::from_deformable(&fresh).map_err(|e| case_err(root, format!("{nname}: overlap geometry: {e}")))?;
        self.regions[ni].fv = fresh.mesh().clone();
        self.geometries[ni] = geometry;
        self.regions[ni].solver = RegionSolver::PointKinetics(PointKineticRegion { mesh: fresh, pk, last: None, gem: gem_faces });
        let rebuild_phi = !self.switches.restart_keeps_phi;
        for r in self.regions.iter_mut() {
            match &mut r.solver {
                RegionSolver::OnePhase(f) => f.solver.restart_at(start_time, rebuild_phi),
                RegionSolver::ThermoMechanics(t) => t.solver.restart_at(),
                RegionSolver::Neutronics(_) | RegionSolver::PointKinetics(_) => {}
            }
        }
        self.mappings = mappings;
        self.maps = build_maps(&self.regions, &self.mappings, &self.geometries, root)?;
        self.deformation = deformation;
        self.control_delta_t = cc.delta_t;
        self.max_delta_t = cc.max_delta_t;
        self.min_delta_t = cc.min_delta_t;
        self.max_power_variation = cc.max_power_variation;
        self.adjust_time_step = cc.adjust_time_step;
        self.end_time = cc.end_time;
        self.write_interval = cc.write_interval;
        self.clock = PseudoTimeClock::starting_at(cc, start_time);
        self.time = self.clock.value();
        self.dt = self.clock.delta_t();
        self.step = 0;
        for i in 0..self.regions.len() {
            self.correct_baffle_less_fields(i);
        }
        Ok(())
    }

    /// The case root's `endTime`.
    #[must_use]
    pub fn end_time(&self) -> f64 {
        self.end_time
    }

    /// `runTime.value()`.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The `deltaT` the NEXT step will use.
    #[must_use]
    pub fn delta_t(&self) -> f64 {
        self.dt
    }

    /// Steps taken.
    #[must_use]
    pub fn step_count(&self) -> usize {
        self.step
    }

    /// `runTime.run()`: `time < endTime` (to the time-name rounding upstream applies: the last step lands within `1e-9 deltaT` of it).
    #[must_use]
    pub fn running(&self) -> bool {
        // `Time::run()`: `value() < endTime - 0.5 deltaT` (gh:#1919: the earlier `1e-9 deltaT` form counted the same 476 steps)
        self.clock.run()
    }

    /// A region by name.
    #[must_use]
    pub fn region(&self, name: &str) -> Option<&Region> {
        self.regions.iter().find(|r| r.name == name)
    }

    /// Prescribe a vector field of a region (a test or a driver supplying what an absent region would map), e.g. the neutronics `disp`.
    ///
    /// # Errors
    /// [`AppBuilderError`] if the region does not exist or the length is not its cell count.
    pub fn prescribe_vector(&mut self, region: &str, name: &str, values: Vec<Vector3>) -> Result<(), AppBuilderError> {
        let Some(r) = self.regions.iter_mut().find(|r| r.name == region) else {
            return Err(case_err(Path::new(""), format!("no region {region}")));
        };
        if values.len() != r.fv.n_cells {
            return Err(case_err(Path::new(""), format!("{region}.{name}: {} values for {} cells", values.len(), r.fv.n_cells)));
        }
        r.fields.set_vector(name, values);
        Ok(())
    }

    /// Replace the fluid region's `U`, `p`, `T` by the values a written time directory holds, to be called BEFORE [`Self::restart_transient`]
    /// (a restart that reads the written fields instead of continuing from the port's own steady state; gh:#1925, README round 61).
    ///
    /// # Errors
    /// [`AppBuilderError`] if the region is not a `onePhase` fluid or a length is not its cell count.
    pub fn adopt_written_fluid_state(&mut self, region: &str, u: &[Vector3], p: &[f64], t: &[f64]) -> Result<(), AppBuilderError> {
        let Some(r) = self.regions.iter_mut().find(|r| r.name == region) else {
            return Err(case_err(Path::new(""), format!("no region {region}")));
        };
        let RegionSolver::OnePhase(f) = &mut r.solver else {
            return Err(case_err(Path::new(""), format!("{region} is not a onePhase fluid region")));
        };
        let n = f.solver.temperature().len();
        if u.len() != n || p.len() != n || t.len() != n {
            return Err(case_err(Path::new(""), format!("{region}: written state has the wrong length for {n} cells")));
        }
        f.solver.adopt_written_state(u, p, t);
        Ok(())
    }

    /// `correctBaffleLessFields()` of region `i`: publish the solver's outputs the other regions map from.
    fn correct_baffle_less_fields(&mut self, i: usize) {
        let r = &mut self.regions[i];
        match &r.solver {
            RegionSolver::OnePhase(f) => {
                let (fuel, clad) = f.solver.fuel_clad_temperatures();
                r.fields.set_scalar("T", f.solver.temperature().to_vec());
                r.fields.set_scalar("thermo:rho", f.solver.thermo_density());
                r.fields.set_scalar("T.fuelAvForNeutronics", fuel);
                r.fields.set_scalar("T.cladAvForNeutronics", clad);
                r.fields.set_scalar("T.passiveStructure", f.solver.passive_structure_temperature().to_vec());
            }
            RegionSolver::Neutronics(_) | RegionSolver::PointKinetics(_) => {}
            RegionSolver::ThermoMechanics(t) => {
                r.fields.set_vector("meshDisp", t.solver.mesh_displacement().to_vec());
                // `TStruct` is mapped into the neutronics `TStructMech` by the transient `multiRegionCouplingDict` (README round 60)
                r.fields.set_scalar("TStruct", t.solver.t_struct().to_vec());
            }
        }
    }

    /// `meshHandler::mapAllFields`.
    ///
    /// # Errors
    /// [`AppBuilderError`] when a source field a mapping names does not exist (upstream's FatalError).
    fn map_all_fields(&mut self) -> Result<(), AppBuilderError> {
        for ti in 0..self.regions.len() {
            for si in 0..self.regions.len() {
                let Some(entry) = self.mappings.iter().find(|m| m.target_region == self.regions[ti].name && m.source_region == self.regions[si].name).cloned() else {
                    continue;
                };
                let Some((_, _, map)) = self.maps.iter().find(|(t, s, _)| *t == ti && *s == si) else { continue };
                for (sf, tf) in entry.source_fields.iter().zip(&entry.target_fields) {
                    if self.switches.disabled_targets.iter().any(|(r, f)| r == &entry.target_region && f == tf) {
                        continue;
                    }
                    if self.regions[ti].fields.scalar(tf).is_some() {
                        let Some(src) = self.regions[si].fields.scalar(sf).map(<[f64]>::to_vec) else {
                            return Err(case_err(Path::new(""), format!("mapping {sf} -> {tf}: no scalar source field {sf} in {}", entry.source_region)));
                        };
                        if let Some(dst) = self.regions[ti].fields.scalar_mut(tf) {
                            map.map_scalar(&src, dst);
                        }
                    } else if self.regions[ti].fields.vector(tf).is_some() {
                        let Some(src) = self.regions[si].fields.vector(sf).map(<[Vector3]>::to_vec) else {
                            return Err(case_err(Path::new(""), format!("mapping {sf} -> {tf}: no vector source field {sf} in {}", entry.source_region)));
                        };
                        if let Some(dst) = self.regions[ti].fields.vector_mut(tf) {
                            map.map_vector(&src, dst);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// `solvers[i].deformMesh()` (`neutronics.C:363-408`).
    fn deform_mesh(&mut self, i: usize) -> Result<(), AppBuilderError> {
        let clock = StepClock { time_index: self.step as i64, delta_t: self.dt };
        let skip = self.switches.no_mesh_deformation;
        let listed = self.deformation.displacement_field(&self.regions[i].name).map(str::to_string);
        let r = &mut self.regions[i];
        let (Some(field), false) = (listed, skip) else { return Ok(()) };
        match &mut r.solver {
            RegionSolver::Neutronics(n) => {
                let Some(cells) = r.fields.vector(&field).map(<[Vector3]>::to_vec) else {
                    return Err(case_err(Path::new(""), format!("{}: no displacement field {field}", r.name)));
                };
                let disp = VolDisplacement::from_cells(&n.mesh, cells, &n.conditions).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                n.mesh.deform_mesh_at(&disp, clock).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                let v0 = n.mesh.v0().map(<[f64]>::to_vec);
                n.solver.rebind_mesh(n.mesh.mesh().clone(), v0).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                Ok(())
            }
            _ => Err(case_err(Path::new(""), format!("{}: meshDeformation of a region that is not the diffusion neutronics is not ported (gh:#1918)", r.name))),
        }
    }

    /// `solvers[i].correctPhysics()`.
    fn correct_physics(&mut self, i: usize) -> Result<(), AppBuilderError> {
        let dt = self.dt;
        // GEM reads the FLUID region's `phi` (`calcGEMLevelAndReactivity`): taken before the neutronics region is borrowed
        let gem_flux = match &self.regions[i].solver {
            RegionSolver::PointKinetics(PointKineticRegion { gem: Some(g), .. }) => Some(self.gem_face_flux(g)?),
            _ => None,
        };
        let r = &mut self.regions[i];
        match &mut r.solver {
            RegionSolver::OnePhase(f) => {
                if let Some(q) = r.fields.scalar("powerDensityNeutronics") {
                    f.solver.set_structure_power_density(q);
                }
                if let Some(q) = r.fields.scalar("powerDensityNeutronicsToLiquid") {
                    f.solver.set_power_density(q);
                }
                f.solver.begin_time_step(dt);
                let _ = f.solver.correct_physics();
            }
            RegionSolver::Neutronics(n) if n.transient.is_some() => {
                // `diffusionNeutronics::correctPhysics()` with `eigenvalueNeutronics false`: `pTotOld_ = power()`, then `solveNeutronics.H`
                n.p_tot_old = n.solver.pseudo_time_power();
                let Some(tr) = n.transient.as_mut() else { unreachable!("guarded by is_some") };
                let step = n.solver.upstream_stationary_step(self.time, dt, &tr.controls).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                n.power = step.power;
                tr.last_step = Some(step);
                // `powerDensity_ = sum flux sigmaPow`, `/= max(fuelFraction, SMALL)(1 - fractionToSecondaryPower)`: both 1 and 0 here; the flux is not
                // renormalised in a transient, so no `power/pTarget` scaling (the eigenvalue branch rescales the post-`normFluxes` flux)
                r.fields.set_scalar("powerDensity", n.solver.upstream_power_density());
            }
            RegionSolver::Neutronics(n) => {
                n.p_tot_old = n.solver.pseudo_time_power();
                let step = n.solver.eigenvalue_time_step(n.p_target, n.first_step).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                n.first_step = false;
                n.k_eff = step.k_eff;
                n.power = step.power;
                let scale = step.power / n.p_target;
                r.fields.set_scalar("powerDensity", n.solver.upstream_power_density().iter().map(|x| x * scale).collect());
            }
            RegionSolver::PointKinetics(p) => {
                // `pointKineticNeutronics::correctPhysics()`: `pTotOld_ = power_`, then `solvePointKinetics.H` on the fields `mapAllFields` just wrote
                let get = |name: &str| r.fields.scalar(name).map(<[f64]>::to_vec).unwrap_or_default();
                let fields = PkFeedbackFields {
                    t_fuel: get("TFuel"),
                    t_clad: get("TClad"),
                    t_cool: get("TCool"),
                    rho_cool: get("rhoCool"),
                    t_struct: get("TStruct"),
                    t_struct_mech: get("TStructMech"),
                };
                if let Some(flux) = gem_flux {
                    // `correctReactivity.H:40`: `calcGEMLevelAndReactivity()` before the reactivity sum
                    p.pk.compute_gem(flux);
                }
                let rep = p.pk.step(self.time, dt, &fields).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                p.last = Some(rep);
                r.fields.set_scalar("powerDensity", p.pk.power_density.clone());
                r.fields.set_scalar("secondaryPowerDensity", p.pk.secondary_power_density.clone());
            }
            RegionSolver::ThermoMechanics(t) => {
                let get = |name: &str| r.fields.scalar(name).map(<[f64]>::to_vec).unwrap_or_default();
                let (th, fuel, pow) = (get("TStructFromTH"), get("TFuel"), get("powerDensityNeutronics"));
                t.solver.set_coupling_fields(&th, &fuel, &pow);
                t.solver.set_delta_t(dt);
                t.last_log = Some(t.solver.correct_physics());
            }
        }
        Ok(())
    }

    /// `sum_i phi[face_i]*|Sf_i|` over the GEM zone, in zone order, of the fluid region's `phi` (`fluid_.phi()`).
    fn gem_face_flux(&self, g: &GemFaces) -> Result<f64, AppBuilderError> {
        let RegionSolver::OnePhase(f) = &self.regions[g.fluid_region].solver else {
            return Err(case_err(Path::new(""), "GEM: the fluid region is not onePhase"));
        };
        let Some(phi) = f.solver.face_flux() else { return Err(case_err(Path::new(""), "GEM: the fluid region has no phi (frozen flux)")) };
        let mut sum = 0.0;
        for &(face, mag_sf) in &g.faces {
            sum += phi[face] * mag_sf;
        }
        Ok(sum)
    }

    /// `solvers[i].maxDeltaT()`.
    fn max_delta_t_of(&self, i: usize) -> f64 {
        match &self.regions[i].solver {
            RegionSolver::OnePhase(f) => f.solver.max_delta_t(),
            RegionSolver::Neutronics(n) => {
                diffusion_max_delta_t(self.time, self.control_delta_t, self.max_delta_t, self.max_power_variation, self.dt, n.solver.pseudo_time_power(), n.p_tot_old)
            }
            RegionSolver::PointKinetics(p) => p.pk.max_delta_t(self.dt, Some(self.max_delta_t), Some(self.max_power_variation)),
            RegionSolver::ThermoMechanics(_) => GREAT,
        }
    }

    /// One pass of the `while (runTime.run())` loop of `GeN-Foam.C`.
    ///
    /// # Errors
    /// [`AppBuilderError`] from a mapping, a deformation or a solver.
    pub fn step(&mut self) -> Result<StepRecord, AppBuilderError> {
        // runTime++
        self.step += 1;
        let dt_used = self.dt;
        self.clock.advance();
        self.time = self.clock.value();
        self.map_all_fields()?;
        for i in 0..self.regions.len() {
            self.deform_mesh(i)?;
            self.correct_physics(i)?;
            self.correct_baffle_less_fields(i);
        }
        // adjustDeltaT
        let maxes: Vec<f64> = (0..self.regions.len()).map(|i| self.max_delta_t_of(i)).collect();
        if let Some(new) = adjust_delta_t(self.adjust_time_step, &maxes, self.min_delta_t, self.dt) {
            // `runTime.setDeltaT(new, true)`: with `writeControl adjustableRunTime` the step is aligned to the next write time
            self.clock.set_delta_t(new, true);
            self.dt = self.clock.delta_t();
        }
        let mut rec = StepRecord { step: self.step, time: self.time, dt: dt_used, k_eff: 0.0, power: 0.0, max_sigma_eq: None, neutronics: None };
        for r in &self.regions {
            match &r.solver {
                RegionSolver::Neutronics(n) => {
                    rec.k_eff = n.k_eff;
                    rec.power = n.power;
                    rec.neutronics = n.transient.as_ref().and_then(|t| t.last_step.clone());
                    if let (Some(tr), Some(ns)) = (&n.transient, &rec.neutronics) {
                        if tr.aitken_acceleration && ns.neutron_iterations >= 3 {
                            return Err(case_err(
                                Path::new(&r.name),
                                format!("step {}: {} neutron iterations with neutronTransport.aitkenAcceleration true: aitkenAcceleration.H acts from the third iteration and reads three UNINITIALISED List<scalar> (undefined behaviour upstream), so there is no upstream number to match; refused by name, no zero-initialised variant (gh:#1932, maintainer decision 2026-11-27)", self.step, ns.neutron_iterations),
                            ));
                        }
                    }
                }
                RegionSolver::PointKinetics(p) => rec.power = p.pk.power(),
                RegionSolver::ThermoMechanics(t) => rec.max_sigma_eq = t.last_log.as_ref().and_then(|l| l.max_sigma_eq),
                RegionSolver::OnePhase(_) => {}
            }
        }
        Ok(rec)
    }
}
