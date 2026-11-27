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
//!   included); `read` of a case that STARTS with `eigenvalueNeutronics false` is still refused (its `fluxStar_`/`prec_` would have to be read from a
//!   start directory); `regionSolvers.neutroRegion pointKinetics` (the PK variant) is refused by name (gh:#1922);
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
use crate::genfoam::neutronics::pseudo_time::{PseudoTimeClock, PseudoTimeControls, PseudoTimeEigenvalue};
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use crate::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use crate::genfoam::thermo_mechanics::legacy_fvm::{LegacyFvmThermoMechanics, PhysicsLog};
use crate::genfoam::workflow::reactor_state::ReactorState;
use crate::io::nuclear_data::read_nuclear_data;
use crate::io::poly_mesh::{read_cell_zones, zone_of_cell};

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
        for (name, kind) in &names {
            let poly = root.join(format!("constant/{name}/polyMesh"));
            let dm = DeformableMesh::read(&poly, PointCellsOrder::CellAscending).map_err(|e| case_err(root, format!("{name}: {e}")))?;
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
                    if !eig {
                        return Err(case_err(root, "eigenvalueNeutronics false at the START of a run (fluxStar_/prec_ read from a start directory) is not ported; run the steady state and RegionSolvers::restart_transient (gh:#1919); the pointKinetics variant is gh:#1922"));
                    }
                    // `neutronics.C`: `keff_` and `pTarget_` come from `<startTime>/uniform/reactorState` (the real log: `Initial keff = 1.003457`)
                    let rs = ReactorState::read(&root.join("0")).map_err(|e| case_err(root, format!("0/uniform/reactorState: {e}")))?;
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
                    solver.set_initial_guess(&vec![vec![1.0; n]; 2], k0);
                    let conditions = vec![DispPatchCondition::ZeroGradient; dm.mesh().patches.len()];
                    for f in ["TFuel", "TClad", "TCool", "TStruct", "TStructMech", "rhoCool"] {
                        fields.set_scalar(f, vec![0.0; n]);
                    }
                    fields.set_scalar("powerDensity", vec![0.0; n]);
                    fields.set_scalar("secondaryPowerDensity", vec![0.0; n]);
                    fields.set_vector("disp", vec![Vector3::ZERO; n]);
                    RegionSolver::Neutronics(NeutronicsRegion { mesh: dm.clone(), solver, p_target, k_eff: k0, power: 0.0, p_tot_old: p_target, first_step: true, conditions, transient: None })
                }
                "legacyThermoMechanics" => {
                    let tm = LegacyFvmCase::read(root, name, TStructGamgPolicy::SubstitutePbicgDilu)?.build()?;
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
        let start_time = get("startTime").unwrap_or(0.0);
        let clock = PseudoTimeClock::starting_at(&clock_controls(c, root)?, start_time);
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
    /// `implicitPredictor true`, a laplacian or ddt scheme other than `Gauss linear corrected` and `Euler`, `eigenvalueNeutronics true`.
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
                (RegionSolver::OnePhase(_), "onePhase") | (RegionSolver::Neutronics(_), "diffusionNeutronics") | (RegionSolver::ThermoMechanics(_), "legacyThermoMechanics")
            );
            if !same {
                return Err(case_err(root, format!("regionSolvers.{k} {w}: a restart that changes the solver of a region is not ported (`pointKinetics` is gh:#1922)")));
            }
        }
        let coupling = read_dict(root, "constant/multiRegionCouplingDict")?;
        let mappings = CouplingDict::mappings_from_dict(&coupling.dict, root)?;
        let deformation = MeshDeformationDict::from_coupling_dict(&coupling.dict).map_err(|e| case_err(root, format!("meshDeformation: {e}")))?;
        let Some(ni) = self.regions.iter().position(|r| matches!(r.solver, RegionSolver::Neutronics(_))) else {
            return Err(case_err(root, "the restart needs a diffusionNeutronics region"));
        };
        let nname = self.regions[ni].name.clone();
        let np = read_dict(root, &format!("constant/{nname}/neutronicsProperties"))?;
        if np.dict.get_word("eigenvalueNeutronics").is_none_or(|w| w == "true" || w == "yes" || w == "on") {
            return Err(case_err(root, "restart_transient needs `eigenvalueNeutronics false`"));
        }
        let fvs = read_dict(root, &format!("system/{nname}/fvSolution"))?;
        let nt = fvs.dict.get_dict("neutronTransport");
        let flag = |k: &str| nt.and_then(|d| d.get_word(k)).is_some_and(|w| w == "true" || w == "yes" || w == "on");
        if flag("implicitPredictor") {
            return Err(case_err(root, "neutronTransport.implicitPredictor true is not ported (implicitPredictor.H); gh:#1919 ports integralPredictor.H only"));
        }
        let integral_predictor = flag("integralPredictor") && !self.switches.no_integral_predictor;
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
        let controls = StationaryStepControls { ddt: NeutronicsDdtScheme::Euler, laplacian, neutron_iteration_residual: res, max_neutron_iterations: max_it };

        // the neutronics mesh as the restart reads it
        let keep_original = self.switches.restart_on_original_mesh;
        let RegionSolver::Neutronics(n) = &mut self.regions[ni].solver else { unreachable!("found as Neutronics") };
        let fresh = if keep_original { n.mesh.restarted_at(&n.mesh.original_points().to_vec()) } else { n.mesh.restarted_at_current_points() }
            .map_err(|e| case_err(root, format!("{nname}: restart mesh: {e}")))?;
        n.mesh = fresh.clone();
        n.solver = n.solver.clone().with_integral_predictor(integral_predictor);
        n.solver.rebind_mesh(fresh.mesh().clone(), None).map_err(|e| case_err(root, e.to_string()))?;
        n.first_step = false;
        n.transient = Some(NeutronicsTransient { controls, last_step: None });
        let geometry = OverlapMesh::from_deformable(&fresh).map_err(|e| case_err(root, format!("{nname}: overlap geometry: {e}")))?;
        self.regions[ni].fv = fresh.mesh().clone();
        self.geometries[ni] = geometry;
        let rebuild_phi = !self.switches.restart_keeps_phi;
        for r in self.regions.iter_mut() {
            match &mut r.solver {
                RegionSolver::OnePhase(f) => f.solver.restart_at(start_time, rebuild_phi),
                RegionSolver::ThermoMechanics(t) => t.solver.restart_at(),
                RegionSolver::Neutronics(_) => {}
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
            RegionSolver::Neutronics(_) => {}
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

    /// `solvers[i].maxDeltaT()`.
    fn max_delta_t_of(&self, i: usize) -> f64 {
        match &self.regions[i].solver {
            RegionSolver::OnePhase(f) => f.solver.max_delta_t(),
            RegionSolver::Neutronics(n) => {
                diffusion_max_delta_t(self.time, self.control_delta_t, self.max_delta_t, self.max_power_variation, self.dt, n.solver.pseudo_time_power(), n.p_tot_old)
            }
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
                }
                RegionSolver::ThermoMechanics(t) => rec.max_sigma_eq = t.last_log.as_ref().and_then(|l| l.max_sigma_eq),
                RegionSolver::OnePhase(_) => {}
            }
        }
        Ok(rec)
    }
}
