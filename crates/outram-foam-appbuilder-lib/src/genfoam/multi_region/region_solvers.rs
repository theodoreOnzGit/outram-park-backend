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
//! One time step of `GeN-Foam.C` (every region is its own solver; ONE `multiPhysicsSolver` entry, gh:#1950, README round 75, runs the Picard loop
//! of its members instead: `multiPhysicsSolver.C:156-186`, see [`RegionSolvers::step`]):
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
use crate::genfoam::neutronics::xs::variables::VariableLaw;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{CaseReadOptions, ControlOverrides, GamgPolicy, OnePhaseCcCase, OnePhaseCcSolver};
use crate::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use crate::genfoam::thermo_mechanics::legacy_fvm::{LegacyFvmThermoMechanics, PhysicsLog};
use crate::genfoam::workflow::reactor_state::ReactorState;
use crate::io::nuclear_data::{read_neutron_fv_solution, read_nuclear_data};
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
    /// `onePhase::residual_` (`getResidual()`, gh:#1950): zeroed by `correctPhysics()`, `max`ed with the `h` solve's initial residual by every
    /// `correctEnergy()` (so `correctTightlyCoupledPhysics()` keeps accumulating into it).
    pub residual: f64,
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
    /// The cross-section data and the cell-to-zone map: `xs_.correct()` of every `solveNeutronics.H` re-evaluates them at the feedback
    /// fields (gh:#1944; a case with `xsVariables {}` has nothing to refresh).
    xs: CrossSectionData,
    zone_map: Vec<usize>,
    /// `axialOrientation` of `nuclearData` (default `(0 0 1)`, `XS.C:76`).
    axial_orientation: Vector3,
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
    /// `diffusionNeutronics::residual_` of the last `correctPhysics()` (`getResidual()`, gh:#1950).
    pub residual: f64,
}

/// What one pass of a `multiPhysicsSolver` loop did, for the comparison with the real log (gh:#1950).
#[derive(Debug, Clone, Default)]
pub struct MultiIterationLog {
    /// The neutronics region's `StationaryStep` of this pass (`power:`, `neutroResidual` of each neutron iteration).
    pub neutronics: Option<StationaryStep>,
    /// The thermo-mechanics solves of this pass in order: `(field, initial residual, iterations)`.
    pub thermo_mechanics: Vec<(String, f64, usize)>,
}

/// A `multiPhysicsSolver` of the `regionSolvers` (gh:#1950, README round 75; `multiPhysicsSolver.C`): the Picard loop of its member regions,
/// read from `multiRegionCouplingDict` `multiPhysicsSolvers.<name>` (`solvers`, `minResidual`, `maxIterations`).
#[derive(Debug, Clone, PartialEq)]
pub struct MultiPhysics {
    /// The solver's name in `regionSolvers`.
    pub name: String,
    /// Indices into [`RegionSolvers::regions`] of the member solvers, in `solvers` dictionary order.
    pub members: Vec<usize>,
    /// `minResidual`.
    pub min_residual: f64,
    /// `maxIterations`.
    pub max_iterations: usize,
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
    /// `keff_` as `reactorState` holds it (read at the start directory, never changed by point kinetics; written as `keff`, gh:#1935).
    pub k_eff: f64,
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
    /// What `<region>.baffleLess/polyMesh` holds when `remove_baffles` (gh:#1935): `Err` is a refusal by name, raised when the directory is written.
    pub baffle_less: Option<Result<crate::io::foam_ascii_mesh::PolyMeshOut, String>>,
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
    /// NAMED ABLATION (gh:#1932, README round 68): do NOT refuse a step that reaches a third neutron iteration with `aitkenAcceleration true`
    /// (the behaviour of rounds 3-66: Aitken silently not performed). Only for controls whose state differs from upstream's on purpose; it is
    /// not a port of `aitkenAcceleration.H` and no number of a run with it is comparable with upstream.
    pub aitken_not_refused: bool,
    /// NAMED ABLATION (gh:#1948, README round 75): the neutronics linear solves keep the FIXED tight form of rounds 56-74 (`PCG`/`PBiCG` at
    /// tolerance 1e-14, `relTol` 0, `maxIter` 100000) instead of the entries of the region's `fvSolution`. The default (false) is upstream's.
    pub tight_neutronics_solvers: bool,
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

    /// Do not refuse `aitkenAcceleration true` at a third neutron iteration (named ablation of gh:#1932; see [`Self::aitken_not_refused`]).
    #[must_use]
    pub fn without_aitken_refusal(mut self) -> Self {
        self.aitken_not_refused = true;
        self
    }

    /// Keep the fixed tight neutronics linear solves of rounds 56-74 (named ablation of gh:#1948; see [`Self::tight_neutronics_solvers`]).
    #[must_use]
    pub fn with_tight_neutronics_solvers(mut self) -> Self {
        self.tight_neutronics_solvers = true;
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
    /// Iterations of the `multiPhysicsSolver` loop of this step (0 when the case has none).
    pub multi_iterations: usize,
    /// The `residual` the loop ended with (`multiPhysicsSolver::correctPhysics`; it never resets inside a step).
    pub multi_residual: f64,
    /// One entry per pass of the `multiPhysicsSolver` loop (empty without one).
    pub iterations: Vec<MultiIterationLog>,
    /// Every `h` solve of the fluid region in this step, in order, `(initial residual, iterations)` (all passes of a `multiPhysicsSolver`).
    pub h_solves: Vec<(f64, usize)>,
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
    /// `Time::timeIndex` of the start: `index` of `<startTime>/uniform/time` (`Time` reads it at a restart), or the steps an in-memory restart already
    /// ran; added to the clock's own index only in the written `uniform/time` (gh:#1935; the clock's `timeStep` write rule still counts from 0).
    start_time_index: usize,
    /// The `OverlapMesh` geometry of each region's mesh as the maps were built (the neutronics one is rebuilt at a restart).
    geometries: Vec<OverlapMesh>,
    /// The `.baffleLess` geometry of a `removeBaffles` region read from the start directory, the source side of its maps (`None`: the baffled one).
    source_geometries: Vec<Option<OverlapMesh>>,
    /// `controlDict` `writeControl adjustableRunTime` `writeInterval`, `None` for any other write control.
    write_interval: Option<f64>,
    /// The `multiPhysicsSolver` of `regionSolvers`, if the case selects one (the only entry then).
    multi: Option<MultiPhysics>,
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
/// `meshHandler.C:192-201` builds each map with the SOURCE region's `.baffleLess` mesh when `removeBaffles` lists it (`source_geometries[si]`, read from
/// the start directory by [`read_baffle_less_geometry`]), the baffled mesh otherwise; the target is always the region's own mesh.
fn build_maps(
    regions: &[Region],
    mappings: &[MappingEntry],
    geometries: &[OverlapMesh],
    source_geometries: &[Option<OverlapMesh>],
    root: &Path,
) -> Result<Vec<(usize, usize, RegionMeshMap)>, AppBuilderError> {
    let mut maps = Vec::new();
    for m in mappings {
        let (Some(ti), Some(si)) = (regions.iter().position(|r| r.name == m.target_region), regions.iter().position(|r| r.name == m.source_region)) else {
            continue;
        };
        let source_geometry = source_geometries.get(si).and_then(Option::as_ref).unwrap_or(&geometries[si]);
        let map = RegionMeshMap::new(&regions[si].fv, &regions[ti].fv, Some((source_geometry, &geometries[ti]))).map_err(|e| case_err(root, e))?;
        maps.push((ti, si, map));
    }
    Ok(maps)
}

/// The geometry of region `name`'s `.baffleLess` mesh as the start directory holds it (`<start>/<name>.baffleLess/polyMesh`, written by the real
/// `createBaffleLessMeshes` at every start: `Writing mesh to time <start>`), for the maps whose SOURCE is that region (gh:#1950, README round 75).
/// `None` when the directory is absent (a case that starts from time 0 with identical meshes uses the baffled geometry, as rounds 54-74 did).
/// The port does not BUILD the merged mesh (gh:#1236); reading the one the real binary wrote is what a restart of its own run has.
///
/// # Errors
/// [`AppBuilderError`] for a mesh that cannot be read or whose cell count is not the region's.
fn read_baffle_less_geometry(root: &Path, name: &str, time_dir: &str, n_cells: usize) -> Result<Option<OverlapMesh>, AppBuilderError> {
    let dir = root.join(time_dir).join(format!("{name}.baffleLess")).join("polyMesh");
    if !dir.join("faces").is_file() {
        return Ok(None);
    }
    let dm = DeformableMesh::read(&dir, PointCellsOrder::CellAscending).map_err(|e| case_err(root, format!("{}: {e}", dir.display())))?;
    if dm.mesh().n_cells != n_cells {
        return Err(case_err(root, format!("{}: {} cells, the region has {n_cells}", dir.display(), dm.mesh().n_cells)));
    }
    Ok(Some(OverlapMesh::from_deformable(&dm).map_err(|e| case_err(root, format!("{name}.baffleLess: overlap geometry: {e}")))?))
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

/// The linear-solver settings of a `diffusionNeutronics` region (gh:#1948, README round 75).
///
/// Upstream: every `neutroEq.solve()` of `fluxEq.H` and `precEq.H` picks its solver from `mesh.solverDict(<field name>)` of the region's
/// `fvSolution` (`fvMatrix::solve()` -> `solverDict(psi.select(finalInnerIter))`, never the `Final` entry: no `pimpleControl` loops the neutronics
/// mesh), with `lduMatrix::solver::readControls` defaults (`lduMatrixSolver.C:195-208`). The `neutronTransport` keys
/// (`neutronIterationResidual`, `maxNeutronIterations`) are read by `solveNeutronics.H:18-25` but its do-while stops after ONE pass when
/// `eigenvalueNeutronics` is true (`!eigenvalueNeutronics_`), so the eigenvalue path consults neither.
/// `tight` is the named ablation: the fixed tight form of rounds 56-74.
///
/// # Errors
/// An `fvSolution` that cannot be read, an entry the port does not translate (reader's refusals), or a flux entry that selects `GAMG`
/// (the reader would run `PCG`/`DIC` in its place, a deviation this region refuses by name rather than carries).
fn neutronics_diffusion_settings(root: &Path, name: &str, xs: &CrossSectionData, tight: bool) -> Result<DiffusionSettings, AppBuilderError> {
    let base = DiffusionSettings {
        k_tolerance: 1e-13,
        flux_tolerance: 1e-12,
        max_outer_iterations: 1,
        linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        ..DiffusionSettings::default()
    };
    if tight {
        return Ok(base);
    }
    let path = root.join(format!("system/{name}/fvSolution"));
    let sol = read_neutron_fv_solution(&path, xs.energy_groups(), xs.prec_groups(), 0).map_err(|e| case_err(root, format!("{}: {e}", path.display())))?;
    if sol.gamg_substituted {
        return Err(case_err(root, format!("system/{name}/fvSolution: the flux entry selects GAMG; GAMG for the neutronics flux equation is not ported (the reader would run PCG/DIC instead)")));
    }
    Ok(base.with_fv_solution(sol.entries))
}

/// The latest time directory of the case (`startFrom latestTime`): the largest numeric directory name.
fn latest_time(root: &Path) -> Result<f64, AppBuilderError> {
    let mut best: Option<f64> = None;
    let rd = std::fs::read_dir(root).map_err(|e| case_err(root, format!("read_dir: {e}")))?;
    for e in rd.flatten() {
        if !e.path().is_dir() {
            continue;
        }
        if let Some(t) = e.file_name().to_str().and_then(|n| n.parse::<f64>().ok()) {
            best = Some(best.map_or(t, |b| b.max(t)));
        }
    }
    best.ok_or_else(|| case_err(root, "startFrom latestTime: no time directory"))
}

/// `multiPhysicsSolver::createSolvers(name)` (`multiPhysicsSolver.C:78-131`): the member `(region, solver kind)` pairs in the dictionary order of
/// `multiPhysicsSolvers.<name>.solvers`, `minResidual` and `maxIterations` (both mandatory: `get<>`). A nested `multiPhysicsSolver` and a
/// non-empty `includeFluidMechanicsInLoop` (the fluid mechanics inside the loop) are refused by name.
///
/// # Errors
/// [`AppBuilderError`] for a missing entry or a refused selection.
fn read_multi_physics(root: &Path, name: &str) -> Result<(MultiPhysics, Vec<(String, String)>), AppBuilderError> {
    let coupling = read_dict(root, "constant/multiRegionCouplingDict")?;
    let d = coupling
        .dict
        .get_dict("multiPhysicsSolvers")
        .and_then(|m| m.get_dict(name))
        .ok_or_else(|| case_err(root, format!("multiRegionCouplingDict: no multiPhysicsSolvers.{name}")))?;
    let solvers = d.get_dict("solvers").ok_or_else(|| case_err(root, format!("multiPhysicsSolvers.{name}: no solvers")))?;
    let mut members = Vec::new();
    for (k, v) in solvers.iter() {
        match v {
            FoamEntry::Word(w) if w == "multiPhysicsSolver" => {
                return Err(case_err(root, format!("multiPhysicsSolvers.{name}.solvers.{k}: a nested multiPhysicsSolver is not ported (gh:#1950)")));
            }
            FoamEntry::Word(w) => members.push((k.to_string(), w.clone())),
            _ => return Err(case_err(root, format!("multiPhysicsSolvers.{name}.solvers.{k} is not a word"))),
        }
    }
    if d.get("includeFluidMechanicsInLoop").is_some() {
        return Err(case_err(root, format!("multiPhysicsSolvers.{name}.includeFluidMechanicsInLoop: the fluid mechanics inside the loop is not ported (gh:#1950)")));
    }
    let min_residual = d.get_scalar("minResidual").ok_or_else(|| case_err(root, format!("multiPhysicsSolvers.{name}: no minResidual")))?;
    let max_iterations = d.get_scalar("maxIterations").ok_or_else(|| case_err(root, format!("multiPhysicsSolvers.{name}: no maxIterations")))? as usize;
    Ok((MultiPhysics { name: name.to_string(), members: Vec::new(), min_residual, max_iterations }, members))
}

/// `axialOrientation` of `nuclearData` (`XS.C:76`, `lookupOrDefault`, default `(0 0 1)`): the entry `axialOrientation (x y z);` when present.
fn axial_orientation_of(root: &Path, region: &str) -> Result<Vector3, AppBuilderError> {
    let path = root.join(format!("constant/{region}/nuclearData"));
    let text = std::fs::read_to_string(&path).map_err(|e| case_err(root, format!("{}: {e}", path.display())))?;
    let Some(line) = text.lines().find(|l| l.trim_start().starts_with("axialOrientation")) else { return Ok(Vector3::new(0.0, 0.0, 1.0)) };
    let inner: String = line.split_once('(').and_then(|(_, r)| r.split_once(')')).map(|(v, _)| v.to_string()).unwrap_or_default();
    let v: Vec<f64> = inner.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() != 3 {
        return Err(case_err(root, format!("{region}: nuclearData axialOrientation `{}` is not a vector", line.trim())));
    }
    Ok(Vector3::new(v[0], v[1], v[2]))
}

/// The per-cell raw feedback parameters of `XS::correct()` in `xsVariables` order (`setNeutronicsVariables.H`): the region's own field of that
/// name, `radExp`/`axExp` from `disp`; a `log` variable clamped `max(x, 0.01)` and a `sqrt` one `max(x, 0)` as upstream does before the law.
fn xs_cell_parameters(n: &NeutronicsRegion, fields: &RegionFields) -> Result<Vec<f64>, AppBuilderError> {
    use crate::genfoam::neutronics::diffusion::expansion::expansion_from_displacement;
    let cells = n.mesh.mesh().n_cells;
    let vars = n.xs.variables();
    let needs_expansion = vars.iter().any(|v| v.name == "axExp" || v.name == "radExp");
    let expansion = if needs_expansion {
        let disp = fields.vector("disp").ok_or_else(|| case_err(Path::new(""), "XS variable radExp/axExp: the region has no `disp`"))?;
        Some(expansion_from_displacement(n.mesh.mesh(), disp, n.axial_orientation))
    } else {
        None
    };
    let mut out = vec![0.0; cells * vars.len()];
    for (j, v) in vars.iter().enumerate() {
        let column: Vec<f64> = match (v.name.as_str(), &expansion) {
            ("axExp", Some(e)) => e.ax_exp.clone(),
            ("radExp", Some(e)) => e.rad_exp.clone(),
            (name, _) => fields.scalar(name).ok_or_else(|| case_err(Path::new(""), format!("XS variable {name}: the region has no such field")))?.to_vec(),
        };
        for (c, x) in column.iter().enumerate() {
            out[c * vars.len() + j] = match v.law {
                VariableLaw::Log => x.max(0.01),
                VariableLaw::Sqrt => x.max(0.0),
                VariableLaw::Linear => *x,
            };
        }
    }
    Ok(out)
}

/// `solveNeutronics.H:119-120`: `secondaryPowerDensity_ = powerDensity_ / max(secondaryPowerVolumeFraction, SMALL) * fractionToSecondaryPower;`
/// then `powerDensity_ /= max(fuelFraction, SMALL) * (1.0 - fractionToSecondaryPower);`, each zone's own constants per cell. Returns
/// `(powerDensity, Some(secondaryPowerDensity))`; the secondary field only when some zone has `fractionToSecondaryPower != 0` (gh:#1944: the
/// ESFR core zones have `fuelFraction 0.46`, the 2-D tutorials `1`, so the division was a no-op until now).
fn fuel_fraction_scaled_power(n: &NeutronicsRegion, power_density: Vec<f64>) -> (Vec<f64>, Option<Vec<f64>>) {
    const SMALL: f64 = 1.0e-15;
    let any_secondary = (0..n.xs.zone_count()).any(|z| n.xs.zone(z).is_some_and(|d| d.fraction_to_secondary_power() != 0.0));
    let mut secondary = if any_secondary { Some(vec![0.0; power_density.len()]) } else { None };
    let mut out = power_density;
    for (c, x) in out.iter_mut().enumerate() {
        let Some(z) = n.xs.zone(n.zone_map[c]) else { continue };
        let frac = z.fraction_to_secondary_power();
        if let Some(sec) = secondary.as_mut() {
            sec[c] = *x / z.secondary_power_volume_fraction().max(SMALL) * frac;
        }
        *x /= z.fuel_fraction().max(SMALL) * (1.0 - frac);
    }
    (out, secondary)
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

/// `Time::timeIndex` of a restart: the `index` entry of `<startTime>/uniform/time` (0 when the directory or the entry is absent).
fn read_start_time_index(root: &Path, start_time: f64) -> usize {
    let Ok(td) = time_dir_name(root, start_time) else { return 0 };
    let Ok(text) = std::fs::read_to_string(root.join(td).join("uniform/time")) else { return 0 };
    text.lines().find_map(|l| l.strip_prefix("index").and_then(|r| r.trim().trim_end_matches(';').trim().parse::<usize>().ok())).unwrap_or(0)
}

/// `<time>/<region>/meshPhi` of a start directory (`READ_IF_PRESENT` by `fvMesh`, kept until the mesh moves): internal faces then every
/// boundary face (an `empty` patch has no entries: zeros), `None` when the file is absent.
fn read_start_mesh_phi(root: &Path, region: &str, time_dir: &str, dm: &DeformableMesh) -> Result<Option<Vec<f64>>, AppBuilderError> {
    use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::parse_scalar_field_text;
    let path = root.join(time_dir).join(region).join("meshPhi");
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| case_err(root, format!("{}: {e}", path.display())))?;
    let mesh = dm.mesh();
    let patches: Vec<(String, usize)> = mesh.patches.iter().map(|q| (q.name.clone(), q.size)).collect();
    let (internal, per_patch) = parse_scalar_field_text(&text, mesh.n_internal_faces, &patches).map_err(|m| case_err(root, format!("{region}/meshPhi: {m}")))?;
    let mut out = vec![0.0; mesh.n_faces];
    out[..mesh.n_internal_faces].copy_from_slice(&internal);
    for (q, vals) in mesh.patches.iter().zip(per_patch) {
        for (k, v) in vals.into_iter().enumerate() {
            out[q.start + k] = v;
        }
    }
    Ok(Some(out))
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

mod baffle_less;
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
        let mut multi: Option<MultiPhysics> = None;
        for (k, v) in rs.iter() {
            match v {
                FoamEntry::Word(w) if w == "multiPhysicsSolver" => {
                    if multi.is_some() || !names.is_empty() {
                        return Err(case_err(root, "regionSolvers: only ONE multiPhysicsSolver, and no other entry beside it, is ported (gh:#1950)"));
                    }
                    let (m, members) = read_multi_physics(root, k)?;
                    for (n, kind) in members {
                        names.push((n, kind));
                    }
                    multi = Some(m);
                }
                FoamEntry::Word(w) => {
                    if multi.is_some() {
                        return Err(case_err(root, "regionSolvers: an entry beside a multiPhysicsSolver is not ported (gh:#1950)"));
                    }
                    names.push((k.to_string(), w.clone()))
                }
                _ => return Err(case_err(root, format!("regionSolvers.{k} is not a word"))),
            }
        }
        // `removeBafflesDict.getOrDefault<bool>(name, false)`: the shipped ESFR writes `fluidRegion 1;` (a number is a `Switch` too, gh:#1950)
        let rb = |name: &str| -> bool {
            c.get_dict("removeBaffles")
                .is_some_and(|d| d.get_word(name).is_some_and(|w| w == "true" || w == "yes" || w == "on") || d.get_scalar(name).is_some_and(|x| x == 1.0))
        };
        let coupling = read_dict(root, "constant/multiRegionCouplingDict")?;
        let mappings = CouplingDict::mappings_from_dict(&coupling.dict, root)?;
        let deformation = MeshDeformationDict::from_coupling_dict(&coupling.dict).map_err(|e| case_err(root, format!("meshDeformation: {e}")))?;

        let mut regions = Vec::new();
        let mut geometries: Vec<OverlapMesh> = Vec::new();
        let mut source_geometries: Vec<Option<OverlapMesh>> = Vec::new();
        // `startFrom latestTime` (`Time::setControls`): the latest time directory of the case (the written steady state of a transient case)
        let start_from_latest = c.get_word("startFrom").is_some_and(|w| w == "latestTime");
        let start_time = if start_from_latest { latest_time(root)? } else { get("startTime").unwrap_or(0.0) };
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
            let baffle_less = if remove_baffles { Some(baffle_less::read_baffle_less(&poly, &dm)) } else { None };
            // gh:#1944 (README round 74): `createBaffleLessMeshes` merges duplicate faces (`Writing 21252 duplicate faces`) and leaves every CELL
            // as it was (same count, same order, same shape: the real log's overlap volumes of the `.baffleLess` maps equal the baffled ones to 1e-14),
            // so the `meshToMesh` sources `<region>.baffleLess` hold the region's own cell values and their weights are the baffled mesh's (1 for
            // identical cells). The merged mesh itself is not built: refused by name only where it would be WRITTEN (`baffle_less` above).
            let _ = has_baffles(&dm);
            geometries.push(OverlapMesh::from_deformable(&dm).map_err(|e| case_err(root, format!("{name}: overlap geometry: {e}")))?);
            // the maps FROM a `removeBaffles` region use its baffle-less mesh; a start directory that holds it (a transient started from a written
            // steady state) gives the geometry the neutronics mesh, moved by the steady state, actually overlaps (gh:#1950)
            let bl_start = if start_time == 0.0 { "0".to_string() } else { time_dir_name(root, start_time)? };
            source_geometries.push(if remove_baffles { read_baffle_less_geometry(root, name, &bl_start, dm.mesh().n_cells)? } else { None });
            let n = dm.mesh().n_cells;
            let fv = dm.mesh().clone();
            let mut fields = RegionFields::default();
            // `meshPhi` READ from the start directory stays what it was until the mesh moves (`fvMesh` constructor, README round 69)
            if let Some(td) = &time_dir {
                if let Some(phi) = read_start_mesh_phi(root, name, td, &dm)? {
                    fields.set_scalar("meshPhi", phi);
                }
            }
            let solver = match kind.as_str() {
                "onePhase" => {
                    // `GAMG` for `p_rgh` runs upstream's own `GAMGSolver` (round 67, `GamgPolicy::Native`); a case that selects `PCG` is unaffected (gh:#1944)
                    let mut opts = CaseReadOptions { gamg: GamgPolicy::Native, ..CaseReadOptions::default() };
                    if start_from_latest {
                        // `startFrom latestTime`: the fluid's own reader takes `startTime` from the dictionary, which still says 0 (gh:#1950)
                        opts.controls = Some(ControlOverrides { start_time, end_time, delta_t: control_delta_t });
                    }
                    let s = OnePhaseCcSolver::new(OnePhaseCcCase::read_with_options(root, &poly, &opts)?);
                    fields.set_scalar("powerDensityNeutronics", vec![0.0; n]);
                    fields.set_scalar("powerDensityNeutronicsToLiquid", vec![0.0; n]);
                    RegionSolver::OnePhase(FluidRegion { solver: s, residual: 0.0 })
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
                    // gh:#1948 (README round 75): the linear solves are the case's `fvSolution` entries (default); the earlier fixed tight form
                    // is the named ablation `CouplingSwitches::with_tight_neutronics_solvers`
                    let settings = neutronics_diffusion_settings(root, name, &xs, switches.tight_neutronics_solvers)?;
                    // a case with `xsVariables` constructs at the feedback fields of the constructor (all zero: `0/neutroRegion` carries no
                    // value; `log` clamped to 0.01 as `setNeutronicsVariables.H` does); every step refreshes them (`xs_.correct()`, gh:#1944)
                    let built = if xs.variables().is_empty() {
                        DiffusionNeutronics::new(dm.mesh().clone(), &xs, &zone_map, &[], &flux_boundary, settings)
                    } else {
                        let row: Vec<f64> = xs.variables().iter().map(|v| if v.law == VariableLaw::Log { 0.01 } else { 0.0 }).collect();
                        let params: Vec<f64> = (0..n).flat_map(|_| row.iter().copied()).collect();
                        DiffusionNeutronics::new_with_cell_parameters(dm.mesh().clone(), &xs, &zone_map, &params, &flux_boundary, settings)
                    };
                    let mut solver = built
                        .map_err(|e| case_err(root, format!("{name}: diffusion solver: {e}")))?
                        .with_eigenvalue_laplacian(NeutronicsLaplacianScheme::Corrected);
                    if eig {
                        let groups = solver.energy_groups();
                        solver.set_initial_guess(&vec![vec![1.0; n]; groups], k0);
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
                        transient = Some(NeutronicsTransient { controls, last_step: None, aitken_acceleration: aitken_requested(root, name)?, residual: 0.0 });
                        first_step = false;
                    }
                    RegionSolver::Neutronics(NeutronicsRegion { mesh: dm.clone(), solver, p_target, k_eff: k0, power: 0.0, p_tot_old: p_target, first_step, conditions, xs, zone_map, axial_orientation: axial_orientation_of(root, name)?, transient })
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
                    {
                        let rs_dir = root.join(td);
                        let k_eff = ReactorState::read(&rs_dir).map_err(|e| case_err(root, format!("{}/uniform/reactorState: {e}", rs_dir.display())))?.k_eff;
                        RegionSolver::PointKinetics(PointKineticRegion { mesh: dm.clone(), pk, last: None, gem: None, k_eff })
                    }
                }
                "legacyThermoMechanics" => {
                    // `disp`, `fuelDisp`, `CRDisp` are READ from the start directory (a written one holds them as lists)
                    let tm_dir = if start_time == 0.0 { "0".to_string() } else { time_dir_name(root, start_time)? };
                    let tm = LegacyFvmCase::read_at(root, name, TStructGamgPolicy::Native, &tm_dir)?.build()?;
                    fields.set_scalar("TStructFromTH", vec![1.0; n]);
                    fields.set_scalar("TFuel", vec![1.0; n]);
                    fields.set_scalar("powerDensityNeutronics", vec![0.0; n]);
                    fields.set_vector("meshDisp", tm.mesh_displacement().to_vec());
                    RegionSolver::ThermoMechanics(ThermoMechanicsRegion { solver: tm, last_log: None })
                }
                other => return Err(case_err(root, format!("regionSolvers.{name} {other}: only onePhase, diffusionNeutronics and legacyThermoMechanics are ported in this loop"))),
            };
            regions.push(Region { name: name.clone(), fv, fields, solver, remove_baffles, baffle_less });
        }
        let maps = build_maps(&regions, &mappings, &geometries, &source_geometries, root)?;
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
            start_time_index: read_start_time_index(root, start_time),
            geometries,
            source_geometries,
            write_interval,
            multi: None,
        };
        // the member indices of a `multiPhysicsSolver` are the regions' own (members were pushed in order)
        me.multi = multi.map(|mut m| {
            m.members = (0..me.regions.len()).collect();
            m
        });
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
        n.transient = Some(NeutronicsTransient { controls, last_step: None, aitken_acceleration: aitken_requested(root, &nname)?, residual: 0.0 });
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
        self.maps = build_maps(&self.regions, &self.mappings, &self.geometries, &self.source_geometries, root)?;
        self.deformation = deformation;
        self.control_delta_t = cc.delta_t;
        self.max_delta_t = cc.max_delta_t;
        self.min_delta_t = cc.min_delta_t;
        self.max_power_variation = cc.max_power_variation;
        self.adjust_time_step = cc.adjust_time_step;
        self.end_time = cc.end_time;
        self.write_interval = cc.write_interval;
        self.start_time_index += self.clock.time_index();
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
        let k_eff = n.k_eff;
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
        self.regions[ni].solver = RegionSolver::PointKinetics(PointKineticRegion { mesh: fresh, pk, last: None, gem: gem_faces, k_eff });
        let rebuild_phi = !self.switches.restart_keeps_phi;
        for r in self.regions.iter_mut() {
            match &mut r.solver {
                RegionSolver::OnePhase(f) => f.solver.restart_at(start_time, rebuild_phi),
                RegionSolver::ThermoMechanics(t) => t.solver.restart_at(),
                RegionSolver::Neutronics(_) | RegionSolver::PointKinetics(_) => {}
            }
        }
        self.mappings = mappings;
        self.maps = build_maps(&self.regions, &self.mappings, &self.geometries, &self.source_geometries, root)?;
        self.deformation = deformation;
        self.control_delta_t = cc.delta_t;
        self.max_delta_t = cc.max_delta_t;
        self.min_delta_t = cc.min_delta_t;
        self.max_power_variation = cc.max_power_variation;
        self.adjust_time_step = cc.adjust_time_step;
        self.end_time = cc.end_time;
        self.write_interval = cc.write_interval;
        self.start_time_index += self.clock.time_index();
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
                f.residual = f.solver.correct_physics().0;
            }
            RegionSolver::Neutronics(n) if n.transient.is_some() => {
                // `diffusionNeutronics::correctPhysics()` with `eigenvalueNeutronics false`: `pTotOld_ = power()`, then `solveNeutronics.H`
                n.p_tot_old = n.solver.pseudo_time_power();
                if !n.xs.variables().is_empty() {
                    let params = xs_cell_parameters(n, &r.fields)?;
                    n.solver.refresh_cross_sections(&n.xs, &n.zone_map, &params).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                }
                let Some(tr) = n.transient.as_mut() else { unreachable!("guarded by is_some") };
                let step = n.solver.upstream_stationary_step(self.time, dt, &tr.controls).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                n.power = step.power;
                tr.residual = step.solver_residual;
                tr.last_step = Some(step);
                // `powerDensity_ = sum flux sigmaPow`, `/= max(fuelFraction, SMALL)(1 - fractionToSecondaryPower)`: both 1 and 0 here; the flux is not
                // renormalised in a transient, so no `power/pTarget` scaling (the eigenvalue branch rescales the post-`normFluxes` flux)
                let (pd, sec) = fuel_fraction_scaled_power(n, n.solver.upstream_power_density());
                r.fields.set_scalar("powerDensity", pd);
                if let Some(sec) = sec {
                    r.fields.set_scalar("secondaryPowerDensity", sec);
                }
            }
            RegionSolver::Neutronics(n) => {
                n.p_tot_old = n.solver.pseudo_time_power();
                if !n.xs.variables().is_empty() {
                    let params = xs_cell_parameters(n, &r.fields)?;
                    n.solver.refresh_cross_sections(&n.xs, &n.zone_map, &params).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                }
                let step = n.solver.eigenvalue_time_step(n.p_target, n.first_step).map_err(|e| case_err(Path::new(""), e.to_string()))?;
                n.first_step = false;
                n.k_eff = step.k_eff;
                n.power = step.power;
                let scale = step.power / n.p_target;
                let (pd, sec) = fuel_fraction_scaled_power(n, n.solver.upstream_power_density().iter().map(|x| x * scale).collect());
                r.fields.set_scalar("powerDensity", pd);
                if let Some(sec) = sec {
                    r.fields.set_scalar("secondaryPowerDensity", sec);
                }
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

    /// `solvers[i].correctTightlyCoupledPhysics()` of a member of a `multiPhysicsSolver` after its first iteration: the fluid runs ONE
    /// `correctModels` and `correctEnergy` (`onePhase.C:172-183`, no `PIMPLE` loop, no `begin_time_step`: the old-time values stay those of the
    /// time step), the neutronics and the thermo-mechanics call `correctPhysics()` (`diffusionNeutronics.C:214`, `legacyThermoMechanics.C:573`).
    fn correct_tightly_coupled_physics(&mut self, i: usize) -> Result<(), AppBuilderError> {
        let r = &mut self.regions[i];
        if let RegionSolver::OnePhase(f) = &mut r.solver {
            if let Some(q) = r.fields.scalar("powerDensityNeutronics") {
                f.solver.set_structure_power_density(q);
            }
            if let Some(q) = r.fields.scalar("powerDensityNeutronicsToLiquid") {
                f.solver.set_power_density(q);
            }
            // `residual_ = max(residual_, EEqn.solve()...)`: `correctTightlyCoupledPhysics` does not zero it
            f.residual = f.residual.max(f.solver.correct_tightly_coupled_physics());
            return Ok(());
        }
        self.correct_physics(i)
    }

    /// `solvers[i].getResidual()`.
    fn residual_of(&self, i: usize) -> f64 {
        match &self.regions[i].solver {
            RegionSolver::OnePhase(f) => f.residual,
            RegionSolver::Neutronics(n) => n.transient.as_ref().map_or(0.0, |t| t.residual),
            RegionSolver::PointKinetics(_) => 0.0,
            RegionSolver::ThermoMechanics(t) => t.solver.residual(),
        }
    }

    /// `multiPhysicsSolver::correctPhysics()` (`multiPhysicsSolver.C:156-186`): the members' Picard loop within one time step. Iteration 0 is
    /// `correctPhysics()` of each member, the later ones `correctTightlyCoupledPhysics()` (no `includeFluidMechanicsInLoop`); every pass maps the
    /// members' fields first (`mapTheseFields`) and `residual` is the running maximum of the members' `getResidual()` over ALL passes (it is never
    /// reset inside the step). The old-time values of the neutronics and thermo-mechanics members are held across the passes, as `oldTime()` is.
    ///
    /// # Errors
    /// [`AppBuilderError`] from a mapping, a deformation or a solver.
    fn correct_multi_physics(&mut self, m: &MultiPhysics) -> Result<(usize, f64, Vec<MultiIterationLog>), AppBuilderError> {
        for &i in &m.members {
            match &mut self.regions[i].solver {
                RegionSolver::Neutronics(n) if n.transient.is_some() => n.solver.hold_stationary_old_time(),
                RegionSolver::ThermoMechanics(t) => t.solver.hold_old_time(),
                _ => {}
            }
        }
        // the main loop calls `solvers[i].deformMesh()` on the multiPhysicsSolver first: it deforms every member (`multiPhysicsSolver.C:139-145`)
        for &i in &m.members {
            self.deform_mesh(i)?;
        }
        let mut residual = 0.0_f64;
        let mut iter = 0usize;
        let mut logs: Vec<MultiIterationLog> = Vec::new();
        let outcome = loop {
            let mut log = MultiIterationLog::default();
            self.map_all_fields()?;
            for &i in &m.members {
                self.deform_mesh(i)?;
                if iter > 0 {
                    self.correct_tightly_coupled_physics(i)?;
                } else {
                    self.correct_physics(i)?;
                }
                self.correct_baffle_less_fields(i);
                match &self.regions[i].solver {
                    RegionSolver::Neutronics(n) => log.neutronics = n.transient.as_ref().and_then(|t| t.last_step.clone()),
                    RegionSolver::ThermoMechanics(t) => {
                        log.thermo_mechanics = t.last_log.as_ref().map(|l| l.solves.iter().map(|x| (x.field.to_string(), x.initial_residual, x.n_iterations)).collect()).unwrap_or_default();
                    }
                    _ => {}
                }
            }
            for &i in &m.members {
                residual = residual.max(self.residual_of(i));
            }
            logs.push(log);
            iter += 1;
            if !(residual > m.min_residual && iter < m.max_iterations) {
                break (iter, residual);
            }
        };
        for &i in &m.members {
            match &mut self.regions[i].solver {
                RegionSolver::Neutronics(n) => n.solver.release_stationary_old_time(),
                RegionSolver::ThermoMechanics(t) => t.solver.release_old_time(),
                _ => {}
            }
        }
        Ok((outcome.0, outcome.1, logs))
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
        let (mut multi_iterations, mut multi_residual, mut iterations) = (0usize, 0.0_f64, Vec::new());
        if let Some(m) = self.multi.clone() {
            (multi_iterations, multi_residual, iterations) = self.correct_multi_physics(&m)?;
        } else {
            for i in 0..self.regions.len() {
                self.deform_mesh(i)?;
                self.correct_physics(i)?;
                self.correct_baffle_less_fields(i);
            }
        }
        // adjustDeltaT
        let maxes: Vec<f64> = (0..self.regions.len()).map(|i| self.max_delta_t_of(i)).collect();
        if let Some(new) = adjust_delta_t(self.adjust_time_step, &maxes, self.min_delta_t, self.dt) {
            // `runTime.setDeltaT(new, true)`: with `writeControl adjustableRunTime` the step is aligned to the next write time
            self.clock.set_delta_t(new, true);
            self.dt = self.clock.delta_t();
        }
        let mut rec = StepRecord { step: self.step, time: self.time, dt: dt_used, k_eff: 0.0, power: 0.0, max_sigma_eq: None, neutronics: None, multi_iterations, multi_residual, iterations, h_solves: Vec::new() };
        for r in &self.regions {
            match &r.solver {
                RegionSolver::Neutronics(n) => {
                    rec.k_eff = n.k_eff;
                    rec.power = n.power;
                    rec.neutronics = n.transient.as_ref().and_then(|t| t.last_step.clone());
                    if let (Some(tr), Some(ns)) = (&n.transient, &rec.neutronics) {
                        if tr.aitken_acceleration && ns.neutron_iterations >= 3 && !self.switches.aitken_not_refused {
                            return Err(case_err(
                                Path::new(&r.name),
                                format!("step {}: {} neutron iterations with neutronTransport.aitkenAcceleration true: aitkenAcceleration.H acts from the third iteration and reads three UNINITIALISED List<scalar> (undefined behaviour upstream), so there is no upstream number to match; refused by name, no zero-initialised variant (gh:#1932, maintainer decision 2026-11-27)", self.step, ns.neutron_iterations),
                            ));
                        }
                    }
                }
                RegionSolver::PointKinetics(p) => rec.power = p.pk.power(),
                RegionSolver::ThermoMechanics(t) => rec.max_sigma_eq = t.last_log.as_ref().and_then(|l| l.max_sigma_eq),
                RegionSolver::OnePhase(f) => rec.h_solves = f.solver.h_solves().to_vec(),
            }
        }
        Ok(rec)
    }
}

#[cfg(test)]
mod neutronics_settings_tests {
    use super::*;
    use crate::genfoam::common::linear_solvers::{LinearSolverForm, NeutronSolverKind};

    fn xs_of_case() -> (std::path::PathBuf, CrossSectionData) {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/cases/fullcoupling_three");
        let input = read_nuclear_data(&root.join("constant/neutroRegion/nuclearData")).expect("nuclearData");
        let xs = CrossSectionData::from_input(&input).expect("xs");
        (root, xs)
    }

    /// The default reads the case's entries; the ablation keeps the fixed tight form (gh:#1948).
    #[test]
    fn default_reads_fv_solution_and_ablation_keeps_the_tight_form() {
        let (root, xs) = xs_of_case();
        let s = neutronics_diffusion_settings(&root, "neutroRegion", &xs, false).expect("settings");
        match s.form {
            LinearSolverForm::UpstreamFvSolution(e) => {
                assert_eq!(e.flux.kind, NeutronSolverKind::PcgDic);
                assert_eq!((e.flux.tolerance, e.flux.rel_tol, e.flux.max_iter), (1e-14, 0.0, 1000));
            }
            other => panic!("default form is {other:?}"),
        }
        let t = neutronics_diffusion_settings(&root, "neutroRegion", &xs, true).expect("tight");
        assert_eq!(t.linear.max_iter, 100_000);
        assert!(matches!(t.form, LinearSolverForm::Upstream { .. }));
        assert!(CouplingSwitches::default().with_tight_neutronics_solvers().tight_neutronics_solvers);
    }

    /// A loose entry is carried through, and a GAMG flux entry is refused by name.
    #[test]
    fn loose_entries_are_read_and_gamg_is_refused() {
        let (_root, xs) = xs_of_case();
        let dir = std::env::temp_dir().join(format!("r75_nsettings_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("system/neutroRegion")).unwrap();
        let hdr = "FoamFile { version 2.0; format ascii; class dictionary; object fvSolution; }\n";
        std::fs::write(dir.join("system/neutroRegion/fvSolution"), format!("{hdr}solvers {{ \"prec.*|precStar.*\" {{ solver PBiCG; preconditioner DILU; tolerance 1e-6; relTol 1e-3; }} \"flux.*\" {{ solver PCG; preconditioner DIC; tolerance 1e-6; relTol 1e-3; }} }}")).unwrap();
        let s = neutronics_diffusion_settings(&dir, "neutroRegion", &xs, false).expect("loose");
        match s.form {
            LinearSolverForm::UpstreamFvSolution(e) => {
                assert_eq!((e.flux.tolerance, e.flux.rel_tol), (1e-6, 1e-3));
                assert_eq!((e.precursor.tolerance, e.precursor.rel_tol), (1e-6, 1e-3));
            }
            other => panic!("form {other:?}"),
        }
        std::fs::write(dir.join("system/neutroRegion/fvSolution"), format!("{hdr}solvers {{ \"flux.*\" {{ solver GAMG; tolerance 1e-6; smoother DICGaussSeidel; }} }}")).unwrap();
        assert!(neutronics_diffusion_settings(&dir, "neutroRegion", &xs, false).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod multi_physics_tests {
    use super::*;

    fn copy_dir(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();
        for e in std::fs::read_dir(from).unwrap().flatten() {
            let (src, dst) = (e.path(), to.join(e.file_name()));
            if src.is_dir() {
                copy_dir(&src, &dst);
            } else {
                std::fs::copy(&src, &dst).unwrap();
            }
        }
    }

    /// The staged small case (2-D `fullCoupling` steady state) with its three `regionSolvers` replaced by ONE `multiPhysicsSolver`.
    fn staged(tag: &str, extra: &str) -> std::path::PathBuf {
        let case = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/cases/fullcoupling_three");
        let root = std::env::temp_dir().join(format!("r75_multi_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(&case, &root);
        let ctl = std::fs::read_to_string(root.join("system/controlDict")).unwrap();
        let start = ctl.find("regionSolvers").unwrap();
        let end = start + ctl[start..].find('}').unwrap() + 1;
        let ctl = format!("{}regionSolvers {{ loop multiPhysicsSolver; }}{}", &ctl[..start], &ctl[end..]);
        let ctl = ctl.replace("endTime 100;", "endTime 3;");
        std::fs::write(root.join("system/controlDict"), ctl).unwrap();
        let cd = std::fs::read_to_string(root.join("constant/multiRegionCouplingDict")).unwrap();
        let block = format!(
            "multiPhysicsSolvers {{ loop {{ solvers {{ fluidRegion onePhase; neutroRegion diffusionNeutronics; thermoMechanicalRegion legacyThermoMechanics; }} minResidual 1e-30; maxIterations 2; {extra} }} }}\n"
        );
        let at = cd.find("meshDeformation").unwrap();
        std::fs::write(root.join("constant/multiRegionCouplingDict"), format!("{}{block}{}", &cd[..at], &cd[at..])).unwrap();
        root
    }

    /// `latest_time` is the largest numeric directory; `startFrom latestTime` needs one.
    #[test]
    fn the_latest_time_is_the_largest_numeric_directory() {
        let dir = std::env::temp_dir().join(format!("r75_latest_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for d in ["0", "200", "200.5", "constant", "system"] {
            std::fs::create_dir_all(dir.join(d)).unwrap();
        }
        assert_eq!(latest_time(&dir).unwrap(), 200.5);
        let empty = std::env::temp_dir().join(format!("r75_latest_empty_{}", std::process::id()));
        std::fs::create_dir_all(&empty).unwrap();
        assert!(latest_time(&empty).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&empty);
    }

    /// `read_baffle_less_geometry`: absent directory -> `None` (the baffled geometry is used), a mesh of the region's cell count -> the geometry, another
    /// cell count -> an error naming it.
    #[test]
    fn the_baffle_less_start_mesh_is_read_when_present() {
        let case = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/cases/fullcoupling_three");
        let root = std::env::temp_dir().join(format!("r75_bl_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        copy_dir(&case.join("constant/fluidRegion/polyMesh"), &root.join("200/fluidRegion.baffleLess/polyMesh"));
        let n = DeformableMesh::read(&case.join("constant/fluidRegion/polyMesh"), PointCellsOrder::CellAscending).unwrap().mesh().n_cells;
        assert!(read_baffle_less_geometry(&root, "fluidRegion", "200", n).unwrap().is_some());
        assert!(read_baffle_less_geometry(&root, "fluidRegion", "100", n).unwrap().is_none());
        assert!(read_baffle_less_geometry(&root, "fluidRegion", "200", n + 1).is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The `multiPhysicsSolvers` dictionary is read in solver order with `minResidual`/`maxIterations`; a nested solver and
    /// `includeFluidMechanicsInLoop` are refused by name.
    #[test]
    fn the_multi_physics_dictionary_is_read_and_its_unported_forms_are_refused() {
        let root = staged("dict", "");
        let (m, members) = read_multi_physics(&root, "loop").unwrap();
        assert_eq!((m.min_residual, m.max_iterations), (1e-30, 2));
        assert_eq!(members.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"]);
        assert!(read_multi_physics(&root, "other").is_err());
        let with_fluid = staged("fluidmech", "includeFluidMechanicsInLoop (fluidRegion);");
        assert!(read_multi_physics(&with_fluid, "loop").unwrap_err().to_string().contains("includeFluidMechanicsInLoop"));
        let nested = staged("nested", "");
        let cd = std::fs::read_to_string(nested.join("constant/multiRegionCouplingDict")).unwrap().replace("thermoMechanicalRegion legacyThermoMechanics; }", "thermoMechanicalRegion multiPhysicsSolver; }");
        std::fs::write(nested.join("constant/multiRegionCouplingDict"), cd).unwrap();
        assert!(read_multi_physics(&nested, "loop").unwrap_err().to_string().contains("nested"));
        for r in [root, with_fluid, nested] {
            let _ = std::fs::remove_dir_all(&r);
        }
    }

    /// Two steps of the Picard loop on the small staged case: every step runs `maxIterations` passes (the residual never falls under 1e-30), each pass logs
    /// its thermo-mechanics solves, the regions are read in `solvers` order, and the residual is the running maximum of the members' (`residual_of`).
    #[test]
    fn the_multi_physics_loop_runs_its_passes_on_the_small_case() {
        let root = staged("loop", "");
        let mut run = RegionSolvers::read(&root, CouplingSwitches::default()).unwrap();
        assert_eq!(run.regions.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["fluidRegion", "neutroRegion", "thermoMechanicalRegion"]);
        for _ in 0..2 {
            let rec = run.step().unwrap();
            assert_eq!(rec.multi_iterations, 2);
            assert_eq!(rec.iterations.len(), 2);
            assert!(rec.iterations.iter().all(|i| !i.thermo_mechanics.is_empty()));
            assert!(rec.multi_residual.is_finite() && rec.multi_residual >= 0.0);
            assert!(rec.k_eff > 0.0 && rec.power > 0.0);
        }
        let residuals: Vec<f64> = (0..run.regions.len()).map(|i| run.residual_of(i)).collect();
        assert!(residuals.iter().all(|r| r.is_finite()));
        let _ = std::fs::remove_dir_all(&root);
    }
}
