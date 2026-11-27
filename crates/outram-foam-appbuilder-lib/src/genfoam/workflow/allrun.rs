// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: Tutorials/reactorCases/2D_MSFR/Allrun (`setSteadyStateTH`, `setSteadyStateEN`, `setTransient`)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free Software Foundation, either
// version 3 of the License, or (at your option) any later version. See <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! `2D_MSFR`'s `Allrun` chain from the ROOT case, as values (gh:#1841). `std` only: it reads case files.
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! `Allrun` clones `rootCase` three times and edits each clone with `foamDictionary`. Here the clones are not made: the
//! edits are the fields of [`AllrunEdits`], applied as typed overrides of what the stage reads, and the one thing
//! `cp -r steadyStateTH/15 steadyStateEN/15` does (hand the 15 s fluid fields on) is a move of the
//! [`FluidFields`](crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::FluidFields) of the stage that ran.
//!
//! ```ignore
//! let root = RootCase::new("rootCase", "fluidMesh/polyMesh", "neutroMesh/polyMesh");   // plain polyMesh directories
//! let edits = AllrunEdits::default();                                                   // 15 / 165 / 175, as Allrun
//! let th = root.steady_state_th(&edits)?.run()?;                                        // steadyStateTH
//! let en = root.steady_state_en(th, &edits)?.run()?;                                    // steadyStateEN, from `th`
//! let mut transient = en.start_transient(TransientStart::EnergyNeutro(root.transient_settings(&edits)?))?;
//! while transient.time() < 175.0 { transient.step_adjustable()?; }                      // keff_ frozen
//! ```

use std::path::{Path, PathBuf};

use outram_foam_basic_lib::io::dict::FoamFile;
use outram_foam_basic_lib::io::FoamEntry;
use outram_foam_basic_lib::ldu_matrix::SolverSettings;

use super::energy_neutro::read_albedo_patches;
use super::{
    EigenvalueRunControls, EigenvalueStage, EnergyNeutroDiffusion, NormalisedState, ReactorState, SteadyStateTh,
    ThRunControls, TransientSettings, WorkflowError,
};
use crate::genfoam::multi_region::coupled_loop::CouplingDict;
use crate::genfoam::neutronics::diffusion::upstream_transient::UpstreamTransientControls;
use crate::genfoam::neutronics::xs::CrossSectionData;
use crate::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::{
    CaseReadOptions, ControlOverrides, GamgPolicy, OnePhaseCcCase, OnePhaseCcSolver,
};
use crate::io::nuclear_data::read_nuclear_data;
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};

/// What `Allrun` sets, with its values as defaults.
#[derive(Debug, Clone)]
pub struct AllrunEdits {
    /// `endTimeTH`.
    pub end_time_th: f64,
    /// `endTimeEN`.
    pub end_time_en: f64,
    /// `endTimeTransient`.
    pub end_time_transient: f64,
    /// `deltaT` of `steadyStateTH` and of `transient` (`1e-03`).
    pub delta_t_adjustable: f64,
    /// `deltaT` of `steadyStateEN` (0.25; `adjustTimeStep false`).
    pub delta_t_en: f64,
    /// `writeInterval` of `steadyStateTH` when `writeControl adjustableRunTime` (1, the rootCase and Allrun); `None` for another write control.
    pub write_interval_th: Option<f64>,
    /// `GAMG` for `p_rgh`: the named deviation, on by default (see [`GamgPolicy`]).
    pub gamg: GamgPolicy,
    /// The neutron iteration controls of the neutronics (`neutronicsProperties`: 1e-6 residual, 50 iterations, `ScNo 1`).
    pub neutron_controls: UpstreamTransientControls,
    /// The flux solver's linear tolerance (the tight protocol of rounds 11/14: 1e-14).
    pub neutron_linear: SolverSettings,
}

impl Default for AllrunEdits {
    fn default() -> Self {
        Self {
            end_time_th: 15.0,
            end_time_en: 165.0,
            end_time_transient: 175.0,
            delta_t_adjustable: 1e-3,
            delta_t_en: 0.25,
            write_interval_th: Some(1.0),
            gamg: GamgPolicy::SubstitutePcgDic,
            neutron_controls: UpstreamTransientControls { neutron_iteration_residual: 1e-6, max_neutron_iterations: 50, sc_no: 1.0 },
            neutron_linear: SolverSettings { tolerance: 1e-14, max_iter: 100_000 },
        }
    }
}

/// A `rootCase` directory and the two plain (uncompressed) `polyMesh` directories of its regions.
#[derive(Debug, Clone)]
pub struct RootCase {
    /// `rootCase`.
    pub root: PathBuf,
    /// `constant/fluidRegion/polyMesh` (plain files; its walls are `type wall`).
    pub fluid_poly_mesh: PathBuf,
    /// `constant/neutroRegion/polyMesh`.
    pub neutro_poly_mesh: PathBuf,
}

fn case_err(msg: String) -> WorkflowError {
    WorkflowError::Case(msg)
}

/// `multiRegionCouplingDict_steadyStateTH` maps nothing (`mappings { fluidRegion { } }`): a single region has no source.
///
/// # Errors
///
/// [`WorkflowError::Case`] if the file cannot be read or maps a field.
pub fn check_steady_state_th_mapping(path: &Path) -> Result<(), WorkflowError> {
    let f = FoamFile::read(path).map_err(|e| case_err(format!("{}: {e}", path.display())))?;
    let Some(mappings) = f.dict.get_dict("mappings") else { return Ok(()) };
    for (target, entry) in mappings.iter() {
        if let FoamEntry::SubDict(d) = entry {
            if d.iter().next().is_some() {
                return Err(case_err(format!("{}: mappings.{target} maps a field; steadyStateTH is the fluid region alone", path.display())));
            }
        }
    }
    Ok(())
}

/// `internalField uniform x;` of a field file.
fn uniform_internal_field(path: &Path) -> Result<f64, WorkflowError> {
    let text = std::fs::read_to_string(path).map_err(|e| case_err(format!("{}: {e}", path.display())))?;
    let at = text.find("internalField").ok_or_else(|| case_err(format!("{}: no internalField", path.display())))?;
    let rest = text[at + "internalField".len()..].trim_start();
    let rest = rest.strip_prefix("uniform").ok_or_else(|| case_err(format!("{}: internalField is not uniform", path.display())))?;
    let end = rest.find(';').ok_or_else(|| case_err(format!("{}: internalField without `;`", path.display())))?;
    rest[..end].trim().parse().map_err(|_| case_err(format!("{}: bad internalField", path.display())))
}

impl RootCase {
    /// A root case with its two mesh directories.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, fluid_poly_mesh: impl Into<PathBuf>, neutro_poly_mesh: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), fluid_poly_mesh: fluid_poly_mesh.into(), neutro_poly_mesh: neutro_poly_mesh.into() }
    }

    /// `steadyStateTH`: `setSteadyStateTH`'s edits as typed settings, the fluid region built from `0/fluidRegion`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] for an unreadable or refused case file (the case reader names what it refuses, `GAMG`
    /// included when `edits.gamg` is [`GamgPolicy::Refuse`]), or a coupling dictionary that maps a field.
    pub fn steady_state_th(&self, edits: &AllrunEdits) -> Result<EigenvalueStage, WorkflowError> {
        check_steady_state_th_mapping(&self.root.join("constant/multiRegionCouplingDict_steadyStateTH"))?;
        let options = CaseReadOptions {
            gamg: edits.gamg,
            solve_fluid_mechanics: Some(true),
            controls: Some(ControlOverrides { start_time: 0.0, end_time: edits.end_time_th, delta_t: edits.delta_t_adjustable }),
            check_region_solvers: false,
            fields: None,
            ..CaseReadOptions::default()
        };
        let case = OnePhaseCcCase::read_with_options(&self.root, &self.fluid_poly_mesh, &options)
            .map_err(|e| case_err(format!("{e}")))?;
        let stage = SteadyStateTh::new(OnePhaseCcSolver::new(case))?;
        Ok(EigenvalueStage::SteadyStateTh {
            stage,
            controls: ThRunControls {
                end_time: edits.end_time_th,
                delta_t: edits.delta_t_adjustable,
                adjust_time_step: true,
                write_interval: edits.write_interval_th,
            },
        })
    }

    /// `steadyStateEN`: `setSteadyStateEN`'s edits (`startTime = endTimeTH`, `solveFluidMechanics false`,
    /// `adjustTimeStep false`, `0/neutroRegion` and `0/uniform` added) with the fluid fields of `th` standing for
    /// `cp -r steadyStateTH/15`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] if `th` is not the state of a `steadyStateTH` stage or a case file is refused.
    pub fn steady_state_en(&self, th: NormalisedState, edits: &AllrunEdits) -> Result<EigenvalueStage, WorkflowError> {
        let NormalisedState::SteadyStateTh { stage, .. } = th else {
            return Err(case_err("steadyStateEN starts from the state of steadyStateTH".into()));
        };
        let options = CaseReadOptions {
            gamg: edits.gamg,
            solve_fluid_mechanics: Some(false),
            controls: Some(ControlOverrides { start_time: edits.end_time_th, end_time: edits.end_time_en, delta_t: edits.delta_t_en }),
            check_region_solvers: false,
            fields: Some(stage.fluid.fluid_fields()),
            ..CaseReadOptions::default()
        };
        let case = OnePhaseCcCase::read_with_options(&self.root, &self.fluid_poly_mesh, &options)
            .map_err(|e| case_err(format!("{e}")))?;
        let fluid = OnePhaseCcSolver::new(case);
        let mesh = read_poly_mesh(&self.neutro_poly_mesh).map_err(|e| case_err(format!("neutro polyMesh: {e}")))?;
        let input = read_nuclear_data(&self.root.join("constant/neutroRegion/nuclearData"))
            .map_err(|e| case_err(format!("nuclearData: {e}")))?;
        let xs = CrossSectionData::from_input(&input).map_err(|e| case_err(format!("cross sections: {e}")))?;
        let zones = read_cell_zones(&self.neutro_poly_mesh).map_err(|e| case_err(format!("cellZones: {e}")))?;
        let order: Vec<&str> = (0..xs.zone_count()).filter_map(|i| xs.zone(i).map(|z| z.name())).collect();
        let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).map_err(|e| case_err(format!("zone map: {e}")))?;
        let flux_file = self.root.join("0/neutroRegion/defaultFlux");
        let albedo = read_albedo_patches(&flux_file)?;
        let initial_flux = uniform_internal_field(&flux_file)?;
        let initial_prec = uniform_internal_field(&self.root.join("0/neutroRegion/defaultPrec"))?;
        let reactor = ReactorState::read(&self.root.join("0"))?;
        let stage = EnergyNeutroDiffusion::new(
            fluid,
            mesh,
            xs,
            zone_map,
            &albedo,
            edits.neutron_controls,
            edits.neutron_linear,
            reactor,
            initial_flux,
            initial_prec,
        )?;
        Ok(EigenvalueStage::EnergyNeutroDiffusion {
            stage,
            controls: EigenvalueRunControls { start_time: edits.end_time_th, end_time: edits.end_time_en, delta_t: edits.delta_t_en },
        })
    }

    /// The `transient` stage's settings: `energyNeutroLoop` of `multiRegionCouplingDict_transient`, `deltaT` of the
    /// Allrun, `maxDeltaT` and `maxPowerVariation` of the `controlDict`.
    ///
    /// # Errors
    ///
    /// [`WorkflowError::Case`] if a file cannot be read.
    pub fn transient_settings(&self, edits: &AllrunEdits) -> Result<TransientSettings, WorkflowError> {
        let path = self.root.join("constant/multiRegionCouplingDict_transient");
        let f = FoamFile::read(&path).map_err(|e| case_err(format!("{}: {e}", path.display())))?;
        let coupling = CouplingDict::from_dict(&f.dict, "energyNeutroLoop", &self.root).map_err(|e| case_err(format!("{e}")))?;
        let cd_path = self.root.join("system/controlDict");
        let cd = FoamFile::read(&cd_path).map_err(|e| case_err(format!("{}: {e}", cd_path.display())))?;
        let scalar = |key: &str, default: f64| match cd.dict.get(key) {
            Some(FoamEntry::Scalar(x)) => *x,
            _ => default,
        };
        Ok(TransientSettings {
            coupling,
            control_delta_t: edits.delta_t_adjustable,
            max_delta_t: scalar("maxDeltaT", 1.0e300),
            max_power_variation: scalar("maxPowerVariation", 0.025),
        })
    }
}
