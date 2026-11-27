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
//! * `eigenvalueNeutronics false` (the transient with `integralPredictor`: gh:#1916/#1919);
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
use super::coupled_loop::{adjust_delta_t, initial_delta_t, CouplingDict, MappingEntry, RegionMeshMap};
use super::mesh_deformation::{DeformableMesh, DispPatchCondition, MeshDeformationDict, PointCellsOrder, StepClock, VolDisplacement};
use crate::error::AppBuilderError;
use crate::genfoam::neutronics::diffusion::external_source::NeutronicsLaplacianScheme;
use crate::genfoam::neutronics::diffusion::upstream_transient::diffusion_max_delta_t;
use crate::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings};
use crate::genfoam::neutronics::pseudo_time::PseudoTimeEigenvalue;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;
use crate::genfoam::thermo_mechanics::legacy_fvm::case::{LegacyFvmCase, TStructGamgPolicy};
use crate::genfoam::thermo_mechanics::legacy_fvm::{LegacyFvmThermoMechanics, PhysicsLog};
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
                        return Err(case_err(root, "eigenvalueNeutronics false (the transient neutronics in the three-region loop) is not ported: gh:#1916, #1919"));
                    }
                    let k0 = np.dict.get_scalar("keff").ok_or_else(|| case_err(root, "neutronicsProperties: no keff"))?;
                    let p_target = np.dict.get_scalar("pTarget").ok_or_else(|| case_err(root, "neutronicsProperties: no pTarget"))?;
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
                    RegionSolver::Neutronics(NeutronicsRegion { mesh: dm.clone(), solver, p_target, k_eff: k0, power: 0.0, p_tot_old: p_target, first_step: true, conditions })
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
        // meshHandler: one meshToMesh per (target, source) pair of the mappings dictionary, built once on the original meshes
        let mut maps = Vec::new();
        for m in &mappings {
            let (Some(ti), Some(si)) = (regions.iter().position(|r| r.name == m.target_region), regions.iter().position(|r| r.name == m.source_region)) else {
                continue;
            };
            let map = RegionMeshMap::new(&regions[si].fv, &regions[ti].fv, Some((&geometries[si], &geometries[ti]))).map_err(|e| case_err(root, e))?;
            maps.push((ti, si, map));
        }
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
            time: 0.0,
            dt: initial_delta_t(control_delta_t, control_delta_t),
            step: 0,
        };
        for i in 0..me.regions.len() {
            me.correct_baffle_less_fields(i);
        }
        Ok(me)
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
        self.time < self.end_time - 1e-9 * self.dt
    }

    /// A region by name.
    #[must_use]
    pub fn region(&self, name: &str) -> Option<&Region> {
        self.regions.iter().find(|r| r.name == name)
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
        self.time += self.dt;
        let dt_used = self.dt;
        self.map_all_fields()?;
        for i in 0..self.regions.len() {
            self.deform_mesh(i)?;
            self.correct_physics(i)?;
            self.correct_baffle_less_fields(i);
        }
        // adjustDeltaT
        let maxes: Vec<f64> = (0..self.regions.len()).map(|i| self.max_delta_t_of(i)).collect();
        if let Some(new) = adjust_delta_t(self.adjust_time_step, &maxes, self.min_delta_t, self.dt) {
            self.dt = new;
        }
        let mut rec = StepRecord { step: self.step, time: self.time, dt: dt_used, k_eff: 0.0, power: 0.0, max_sigma_eq: None };
        for r in &self.regions {
            match &r.solver {
                RegionSolver::Neutronics(n) => {
                    rec.k_eff = n.k_eff;
                    rec.power = n.power;
                }
                RegionSolver::ThermoMechanics(t) => rec.max_sigma_eq = t.last_log.as_ref().and_then(|l| l.max_sigma_eq),
                RegionSolver::OnePhase(_) => {}
            }
        }
        Ok(rec)
    }
}
