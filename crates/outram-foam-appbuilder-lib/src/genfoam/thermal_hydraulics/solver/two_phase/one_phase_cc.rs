// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/onePhase/onePhase.C
//     (correctPhysics, correctTightlyCoupledPhysics, correctEnergy, correctModels,
//      correctCourant, correctContErr, maxDeltaT),
//     include/equations/EEqn_1p.H,
//   and the classes those call: src/phaseModels/fluid/fluid.C (`fluid::rho`, Boussinesq),
//   src/phaseModels/structureModels/{structure.C (correct, linearizedSemiImplicitHeatSource),
//     powerModels/fixedTemperature/fixedTemperature.C},
//   src/physicsModels/phasePairs/FSPair.C (correct), templatedModels/constant/constantModel.C,
//   heatTransferModels/FSHeatTransferCoefficientModels/FSHeatTransferCoefficientModel.C (correctField),
//   and OpenFOAM-ESI v2506 `heThermo::alphaEff`, `species::thermo::T` (THs), `hConstThermo`,
//   `Boussinesq`, `fvMatrix::relax(alpha)`, `PBiCGStab`.
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `two_phase::one_phase_cc` — GeN-Foam's `onePhase` solver, `solveFluidMechanics false`
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! The thermal-hydraulic half of the `2D_MSFR` `transient` stage (gh:#1773, gh:#1361 round 9): a
//! cell-centred, **energy-only** `onePhase` region. The flux is frozen at its restart value
//! (`alphaPhi`, `alphaRhoPhi`, `U` read from the start directory, never updated); `onePhase::correctPhysics`
//! then runs, per PIMPLE outer iteration, `correctModels(true, true)` (`FSPair::correct`) and
//! `correctEnergy` (`EEqn_1p.H`). **`turbulence->correct()` is NOT called** (it sits in `UEqn_1p.H`, which
//! only runs when `solveFluidMechanics` is true), so the energy equation's `alphaEff` is
//! `thermo.alpha + alphat` with `alphat` read once from the restart (gh:#1775: read, never solved).
//!
//! ## What upstream does that is easy to get wrong (all read from the source, none tuned)
//!
//! * **`fluid::rho()` is `rho0` for a `Boussinesq` equation of state** (`fluid.C:795`): `ddt(alpha, rho, he)`,
//!   `alpha rho (U.g)` and `ddt(alpha, rho, K)` use the CONSTANT `rho0`; only the thermo's own `rho(T)`
//!   (mapped to the neutronics as `rhoCool`) varies.
//! * `hConst` + `Boussinesq`: `hs = Cp (T - Tref) + Hsref` (`Boussinesq::H = 0` in v2506), `Tref = Tstd = 298.15`.
//!   `T` comes back from `he` by `species::thermo::T`'s Newton loop (`Ttol = T0 1e-4`), written out here with
//!   upstream's operation order.
//! * `alphaEff = CpByCpv (alpha_thermo + alphat)`, `CpByCpv = 1` for an enthalpy form, `alpha_thermo = mu/Pr`
//!   (`constTransport::alphah = mu rPr`).
//! * The `h` equation is solved by `PBiCGStab`/`diagonal` (the catch-all `".*"` solver wins over `"h.*"`: OpenFOAM
//!   takes the LAST matching pattern), with `fvMatrix::relax(0.9)` from `relaxationFactors.equations`.
//! * `fvc::ddt(p)` is exactly 0 (`p` is never modified) and `K.oldTime()` of a fresh local field is `K`.
//! * The wedge patches carry a ~1e-11 residual flux (read from the restart); like upstream it enters `div`
//!   (diagonal), `contErr` and the Courant number.
//!
//! ## Not ported (refused by name in [`OnePhaseCcCase::read`])
//!
//! ~~`solveFluidMechanics true` (UEqn/pEqn, `porousKEpsilon` solving: gh:#1775)~~ **CORRECTED 2026-11-26 (gh:#1837, round 19):** `solveFluidMechanics true` with `turbulence false` is ported ([`flow`]); ~~`turbulence true` (wall functions, `porousKEpsilon::correct`: gh:#1838, gh:#1839) is still refused~~ **CORRECTED 2026-11-26 (round 20): `turbulence true` is ported ([`turbulence`])**; heat exchangers, pumps (a `momentumSource` pump IS read when the flow is solved),
//! passive structures, tortuosity, local frames; ~~power models other than `fixedTemperature`~~ **CORRECTED 2026-11-27 (gh:#1879, README round 37):**
//! `heatedPin`, `nuclearFuelPin`, `lumpedNuclearStructure` and `interpolatedNuclearFuelPin` are read ([`radial_power`]) and run through
//! `structure::correct` ([`OnePhaseCcSolver::radial_power_models`]); `nuclearFuelPinTest`, the FMU models and `fixedPower` are refused by name; heat-transfer models other than `constant`; thermo other than `heRhoThermo`
//! `hConst` `Boussinesq` `const`; ~~`residualControl` in `PIMPLE`~~ **CORRECTED 2026-11-27 (gh:#1855, round 31): `residualControl` (customPimpleControl), the `Final` selection it causes and the `relaxationFactors` `default` entries are ported ([`pimple`])**; `stopIfFirstResRelChangeAboveRelTol` (upstream aborts with SIGFPE). `dragModels` (`ReynoldsPower`) are read but
//! not evaluated: `Kd` is a momentum coefficient and does not enter the energy equation.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::path::Path;

use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::f64::Time;
use uom::si::time::second;

use crate::genfoam::common::TimeProfile;

mod flow;
mod pimple;
#[cfg(feature = "std")]
use super::radial_power;
mod turbulence;
pub use flow::{OnePhaseFlowCase, PressureSolve, ReynoldsPowerDrag};
use flow::FlowState;
pub use pimple::{LoopStep, PimpleControls, PimpleState, RelaxationDict, RelaxationFactors, ResidualSpec};
pub use turbulence::{
    near_wall_dist, set_values, DiagonalBicgstab, EpsilonWallSpec, NutWallSpec, OnePhaseTurbulenceCase, WallBlender, WallFunctionBlending,
    WallFunctionCoeffs,
};
use turbulence::TurbState;
pub use super::radial_power::fvc_laplacian_uncorrected;
use crate::genfoam::thermal_hydraulics::structure::heat_exchanger::HeatExchangerSet;
use crate::genfoam::thermal_hydraulics::structure::radial_power_model::{RadialPowerModel, StepInputs};

use super::fv::{self, Bc, Eqn, Geometry, Scalar, Vector};
use super::pbicgstab::{pbicgstab_diagonal, DEFAULT_MAX_ITER};
use crate::error::AppBuilderError;

#[cfg(feature = "std")]
use super::case::{bool_entry, scalar, word, Ctx, read_table_time_profile};
#[cfg(feature = "std")]
use crate::io::field_reader::{read_vol_scalar_field, read_vol_vector_field};
#[cfg(feature = "std")]
use crate::io::poly_mesh::{read_cell_zones, read_poly_mesh};
#[cfg(feature = "std")]
use outram_foam_basic_lib::io::{FoamDict, FoamEntry};

/// OpenFOAM `SMALL` for `double`.
const SMALL: f64 = 1e-15;
/// OpenFOAM `Tstd` (ESI v2506).
const TSTD: f64 = 298.15;

/// `fvSolution`/`controlDict` entries the stage reads.
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseCcControls {
    /// `PIMPLE.nOuterCorrectors`, `minNOuterCorrectors`, `residualControl`, `finalOnLastPimpleIterOnly`, `nCorrectors`
    /// (`customPimpleControl`, gh:#1855).
    pub pimple: PimpleControls,
    /// `solvers.h.tolerance` (the entry that wins for `h`).
    pub h_tolerance: f64,
    /// `solvers.h.relTol`.
    pub h_rel_tol: f64,
    /// `solvers.h.minIter`.
    pub h_min_iter: usize,
    /// `solvers.hFinal` (the entry that wins for `hFinal`: used in the flagged iteration only).
    pub h_tolerance_final: f64,
    /// `solvers.hFinal.relTol`.
    pub h_rel_tol_final: f64,
    /// `solvers.hFinal.minIter`.
    pub h_min_iter_final: usize,
    /// `relaxationFactors` (`fields` and `equations`, with their `default` entries and the `Final` names).
    pub relax: RelaxationFactors,
    /// `controlDict.includeKineticEnergy` (default true).
    pub include_kinetic_energy: bool,
    /// `controlDict.maxCo` (default 1).
    pub max_co: f64,
    /// `controlDict.maxDeltaT` (default `GREAT`).
    pub max_delta_t: f64,
}

/// `heatTransferModels."<zone>"` (closed set).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsHtcModel {
    /// `type constant; value v;` (`constantModel`): `htc[celli] = value`.
    Constant(f64),
}

impl FsHtcModel {
    /// `FSHeatTransferCoefficientModel::value(celli)` [W/m^2/K].
    #[must_use]
    pub fn value(&self) -> f64 {
        match self {
            Self::Constant(v) => *v,
        }
    }
}

/// `heRhoThermo<pureMixture<const<hConst<Boussinesq<specie>>>>>`, sensible enthalpy.
#[derive(Debug, Clone, PartialEq)]
pub struct BoussinesqHConst {
    /// `thermodynamics.Cp` [J/kg/K].
    pub cp: f64,
    /// `Tref` (default `Tstd`).
    pub tref: f64,
    /// `Href` (default 0).
    pub hsref: f64,
    /// `equationOfState.rho0`.
    pub rho0: f64,
    /// `equationOfState.T0`.
    pub t0: f64,
    /// `equationOfState.beta`.
    pub beta: f64,
    /// `transport.mu`.
    pub mu: f64,
    /// `1/Pr` (`constTransport::rPr_`).
    pub r_pr: f64,
}

impl BoussinesqHConst {
    /// `hConstThermo::Hs`: `Cp (T - Tref) + Hsref + Boussinesq::H` (`H = 0`).
    #[must_use]
    pub fn hs(&self, t: f64) -> f64 {
        self.cp * (t - self.tref) + self.hsref + 0.0
    }

    /// `Boussinesq::rho(p, T) = rho0 (1 - beta (T - T0))`.
    #[must_use]
    pub fn rho(&self, t: f64) -> f64 {
        self.rho0 * (1.0 - self.beta * (t - self.t0))
    }

    /// `constTransport::alphah(p, T) = mu rPr`.
    #[must_use]
    pub fn alpha_h(&self) -> f64 {
        self.mu * self.r_pr
    }

    /// `species::thermo::THs`: Newton from `t0` (`Ttol = T0 1e-4`, at most 100 iterations; upstream aborts
    /// past that, this returns the last iterate).
    #[must_use]
    pub fn t_from_hs(&self, f: f64, t0: f64) -> f64 {
        let mut test;
        let mut tnew = t0;
        let ttol = t0 * 1.0e-4;
        let mut iter = 0;
        loop {
            test = tnew;
            tnew = test - (self.hs(test) - f) / (self.cp + 0.0);
            iter += 1;
            if (tnew - test).abs() <= ttol || iter > 100 {
                break;
            }
        }
        tnew
    }
}

/// What to do when `solvers.p_rgh` selects `GAMG` (the shipped 2D_MSFR case does).
///
/// `outram_foam_basic_lib`'s GAMG is not a translation of OpenFOAM's `GAMGSolver` (algebraic `|upper|` pairing, a symmetric
/// 2+2 V-cycle, a dense coarsest solve, an unshifted residual norm; README round 21), so the port has no faithful `GAMG`.
/// The substitution is a named deviation, chosen by this value and reported by [`OnePhaseFlowCase::p_rgh_gamg_substituted`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GamgPolicy {
    /// Refuse `GAMG` by name (`only PCG with the DIC preconditioner is ported`).
    Refuse,
    /// Solve `p_rgh` with `PCG`/`DIC` at the entry's own `tolerance`/`relTol` (the deviation of README round 21).
    #[default]
    SubstitutePcgDic,
}

/// The `controlDict` entries a stage of the Allrun chain sets (`startTime`, `endTime`, `deltaT`), typed instead of edited.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlOverrides {
    /// `startTime`.
    pub start_time: f64,
    /// `endTime`.
    pub end_time: f64,
    /// `deltaT`.
    pub delta_t: f64,
}

/// The fluid fields at the end of a stage, in the form the next stage reads them from its start directory
/// (`T`, `p`, `U`, `alphat`, `alphaRhoPhi`, `alphaPhi`, `T.fixedTemperature`, `htc`, `powerDensityNeutronicsToLiquid`).
#[derive(Debug, Clone, PartialEq)]
pub struct FluidFields {
    /// `T` per cell.
    pub t: Vec<f64>,
    /// `p` per cell.
    pub p: Vec<f64>,
    /// `U` per cell.
    pub u: Vec<Vector3>,
    /// `alphat` per cell.
    pub alphat: Vec<f64>,
    /// `alphaRhoPhi`, every face.
    pub alpha_rho_phi: Vec<f64>,
    /// `alphaPhi`, every face.
    pub alpha_phi: Vec<f64>,
    /// `T.fixedTemperature` per cell.
    pub t_fixed: Vec<f64>,
    /// `htc` per cell.
    pub htc: Vec<f64>,
    /// `powerDensityNeutronicsToLiquid` per cell.
    pub power_density: Vec<f64>,
}

/// How a case is read: the stage edits of `Allrun` as values, and where the start fields come from.
#[derive(Debug, Clone)]
pub struct CaseReadOptions {
    /// `GAMG` for `p_rgh`.
    pub gamg: GamgPolicy,
    /// `solveFluidMechanics` of the stage (`None`: as `fvSolution` says). `Allrun` sets `false` for EN and transient.
    pub solve_fluid_mechanics: Option<bool>,
    /// The stage's `startTime`/`endTime`/`deltaT` (`None`: as `controlDict` says).
    pub controls: Option<ControlOverrides>,
    /// Check `controlDict.regionSolvers` (a rootCase has it empty: the stage sets it, so the driver does not check).
    pub check_region_solvers: bool,
    /// The start fields (`None`: read `<startTime>/fluidRegion`), the in-memory form of `cp -r steadyStateTH/15`.
    pub fields: Option<FluidFields>,
    /// CONTROL, NOT upstream (gh:#1879, round 35's named control): build every `nuclearFuelPin` with the ring areas a careful
    /// author would have written instead of upstream's two slips. Never the default.
    pub radial_corrected_ring_areas: bool,
}

impl Default for CaseReadOptions {
    fn default() -> Self {
        Self {
            gamg: GamgPolicy::Refuse,
            solve_fluid_mechanics: None,
            controls: None,
            check_region_solvers: true,
            fields: None,
            radial_corrected_ring_areas: false,
        }
    }
}

/// A read case (all fields at the start time).
#[derive(Debug, Clone)]
pub struct OnePhaseCcCase {
    /// Mesh.
    pub mesh: Arc<FvMesh>,
    /// Gravity [m/s^2].
    pub gravity: Vector3,
    /// Thermophysical package.
    pub thermo: BoussinesqHConst,
    /// Controls.
    pub controls: OnePhaseCcControls,
    /// `startTime`.
    pub start_time: f64,
    /// `endTime`.
    pub end_time: f64,
    /// `deltaT`.
    pub delta_t: f64,
    /// Structure volume fraction per cell (`volumeFraction` of the zone).
    pub alpha_structure: Vec<f64>,
    /// Structure hydraulic diameter per cell (`Dh`).
    pub dh: Vec<f64>,
    /// `volumetricArea.activeStructure`: the `fixedTemperature` power model's area per cell.
    pub ia_act: Vec<f64>,
    /// `structureProperties.heatExchangers` with `iAHX` and `T.HX` (README round 44, gh:#1847); empty when none.
    pub heat_exchangers: HeatExchangerSet,
    /// `T.fixedTemperature` (read from the start directory) per cell.
    pub t_fixed: Vec<f64>,
    /// `temperatureTimeProfile` of each `fixedTemperature` zone (gh:#1813): the cells of the zone and the table of scaling factors.
    pub t_profiles: Vec<(Vec<usize>, TimeProfile)>,
    /// The radial-conduction power models of `structureProperties` (`heatedPin`, `nuclearFuelPin`, `lumpedNuclearStructure`,
    /// `interpolatedNuclearFuelPin`; gh:#1879), one per type in first-appearance order; empty for a `fixedTemperature`-only case.
    pub radial_power: Vec<RadialPowerModel>,
    /// The structure's `powerDensityNeutronics` per cell (`READ_IF_PRESENT`, else the `powerDensity` of the radial models'
    /// regions: `structure::setRegionField`); 0 elsewhere.
    pub power_density_neutronics: Vec<f64>,
    /// A `lumpedNuclearStructure` region has `kappaMatrix != 0`: `fvc::laplacian(Tmatrix)` is evaluated.
    pub radial_needs_laplacian: bool,
    /// `heatTransferModels` per cell.
    pub htc_model: Vec<Option<FsHtcModel>>,
    /// `htc` read from the start directory (cells outside the model zones keep it).
    pub htc0: Vec<f64>,
    /// `T`.
    pub t: Vec<f64>,
    /// `p`.
    pub p: Vec<f64>,
    /// `U`.
    pub u: Vec<Vector3>,
    /// `alphat` (turbulent thermal diffusivity, read, never solved).
    pub alphat: Vec<f64>,
    /// `alphaRhoPhi`, every face (boundary faces included, wedge residuals and all).
    pub alpha_rho_phi: Vec<f64>,
    /// `alphaPhi`, every face.
    pub alpha_phi: Vec<f64>,
    /// `powerDensityNeutronicsToLiquid`.
    pub power_density: Vec<f64>,
    /// `residualFluidStructureRe` (default 10).
    pub min_re: f64,
    /// `solveFluidMechanics true`: the momentum/pressure part of the case (`None` for the frozen-flux region).
    pub flow: Option<OnePhaseFlowCase>,
}

/// Parse `internalField` and the `value` of the named patches of a surface (or any scalar) field file.
///
/// `patches` is `(name, size)`; a patch without a `value` entry gets zeros. Pure text work, so it is
/// available without `std`.
///
/// # Errors
/// [`AppBuilderError::Case`]-free `String` messages are mapped by the caller; here a `String` error.
pub fn parse_scalar_field_text(
    text: &str,
    n_internal: usize,
    patches: &[(String, usize)],
) -> Result<(Vec<f64>, Vec<Vec<f64>>), String> {
    let ip = text.find("internalField").ok_or("no internalField")?;
    let (internal, _) = parse_value(&text[ip + "internalField".len()..], n_internal)?;
    let bp = text.find("boundaryField").ok_or("no boundaryField")?;
    let bf = &text[bp..];
    let mut out = Vec::with_capacity(patches.len());
    for (name, size) in patches {
        let mut found: Option<&str> = None;
        for (pos, _) in bf.match_indices(name.as_str()) {
            let before_ok = bf[..pos].chars().next_back().is_some_and(char::is_whitespace);
            let after = &bf[pos + name.len()..];
            let after_t = after.trim_start();
            if before_ok && after_t.starts_with('{') && after.chars().next().is_some_and(char::is_whitespace) {
                let close = after_t.find('}').ok_or("unterminated patch block")?;
                found = Some(&after_t[1..close]);
                break;
            }
        }
        let block = found.ok_or_else(|| format!("no patch block `{name}`"))?;
        let mut vals = vec![0.0; *size];
        for (pos, _) in block.match_indices("value") {
            let before_ok = block[..pos].chars().next_back().is_none_or(char::is_whitespace);
            if before_ok {
                let (v, _) = parse_value(&block[pos + 5..], *size)?;
                vals = v;
                break;
            }
        }
        out.push(vals);
    }
    Ok((internal, out))
}

/// `uniform x` or `nonuniform List<scalar> N ( ... )` at the start of `s`; returns the values and the rest.
fn parse_value(s: &str, n: usize) -> Result<(Vec<f64>, &str), String> {
    let s = s.trim_start();
    if let Some(r) = s.strip_prefix("uniform") {
        let end = r.find(';').ok_or("uniform without `;`")?;
        let v: f64 = r[..end].trim().parse().map_err(|_| format!("bad uniform `{}`", r[..end].trim()))?;
        return Ok((vec![v; n], &r[end..]));
    }
    if let Some(r) = s.strip_prefix("nonuniform") {
        let r = r.trim_start();
        let r = r.strip_prefix("List<scalar>").unwrap_or(r).trim_start();
        let open = r.find('(').ok_or("nonuniform without `(`")?;
        let count: usize = r[..open].trim().parse().map_err(|_| "bad list size".to_string())?;
        let close = r[open..].find(')').ok_or("unterminated list")? + open;
        let vals: Result<Vec<f64>, _> = r[open + 1..close].split_whitespace().map(str::parse::<f64>).collect();
        let vals = vals.map_err(|_| "bad number in list".to_string())?;
        if vals.len() != count {
            return Err(format!("list says {count} values, has {}", vals.len()));
        }
        return Ok((vals, &r[close + 1..]));
    }
    Err("neither uniform nor nonuniform".to_string())
}

#[cfg(feature = "std")]
fn last_match<'a>(d: &'a FoamDict, name: &str) -> Option<&'a FoamEntry> {
    // OpenFOAM: an exact keyword first, else the LAST matching pattern.
    if let Some(e) = d.get(name) {
        return Some(e);
    }
    d.iter().filter(|(k, _)| super::case::key_matches(k, name)).last().map(|(_, e)| e)
}


/// `PIMPLE.residualControl` as `solutionControl::read(false)` and `customPimpleControl::criteriaSatisfied` read it: every entry a
/// dictionary with `tolerance` and `relTol` (both mandatory), optional `useFirstPISOInitialResidual`.
///
/// `stopIfFirstResRelChangeAboveRelTol true` is refused by name: upstream divides `last/first` of the first-PISO residual pair, which is
/// `(0, r1)` at outer iteration 2 of the first step, and the real binary aborts with a floating point exception there (gh:#1855, README round 31).
#[cfg(feature = "std")]
fn read_residual_control(ctx: &Ctx, pimple: &FoamDict) -> Result<Vec<ResidualSpec>, AppBuilderError> {
    let mut out = Vec::new();
    if pimple.get("residualControl").is_some() && pimple.get_dict("residualControl").is_none() {
        return Err(ctx.e("PIMPLE.residualControl must be a dictionary of dictionaries"));
    }
    let Some(rc) = pimple.get_dict("residualControl") else { return Ok(out) };
    for (key, entry) in rc.iter() {
        let FoamEntry::SubDict(d) = entry else {
            return Err(ctx.e(format!("residualControl.{key}: residual data must be specified as a dictionary")));
        };
        let use_first_piso = bool_entry(d, "useFirstPISOInitialResidual").unwrap_or(false);
        if use_first_piso && bool_entry(d, "stopIfFirstResRelChangeAboveRelTol").unwrap_or(false) {
            return Err(ctx.e(format!(
                "residualControl.{key}: stopIfFirstResRelChangeAboveRelTol aborts upstream with a floating point exception (division by the zero first-PISO residual); refused"
            )));
        }
        out.push(ResidualSpec {
            name: key.to_string(),
            abs_tol: ctx.f64(d, "tolerance")?,
            rel_tol: ctx.f64(d, "relTol")?,
            use_first_piso,
        });
    }
    Ok(out)
}

/// `fvSolution.relaxationFactors` as `solution::read` reads it: `fields` and `equations` sub-dictionaries, or (neither present)
/// the old flat form, where `equations` is the whole dictionary and `fields` the entries starting with `p` or `rho`.
///
/// Only constant scalars are ported (a `Function1` entry is refused). A `fields` entry `<f>Final` without the plain `<f>` (or `default`)
/// for `h` or `p_rgh` is refused: upstream stores `prevIter` for a field only when the plain name is relaxed, so the final-iteration
/// `relax()` would abort (or use a stale value).
#[cfg(feature = "std")]
fn read_relaxation_factors(ctx: &Ctx, fs: &FoamDict) -> Result<RelaxationFactors, AppBuilderError> {
    let Some(r) = fs.get_dict("relaxationFactors") else { return Ok(RelaxationFactors::default()) };
    let scalars = |d: &FoamDict, what: &str, keep: &dyn Fn(&str) -> bool| -> Result<RelaxationDict, AppBuilderError> {
        let mut v = Vec::new();
        for (k, e) in d.iter() {
            match e {
                FoamEntry::Scalar(x) => {
                    if keep(k) {
                        v.push((k.to_string(), *x));
                    }
                }
                _ => return Err(ctx.e(format!("relaxationFactors.{what}.{k}: only a constant scalar is ported"))),
            }
        }
        Ok(RelaxationDict::new(v))
    };
    let (fd, ed) = (r.get_dict("fields"), r.get_dict("equations"));
    let all = |_: &str| true;
    let (fields, equations) = match (fd, ed) {
        (None, None) => (scalars(r, "(flat)", &|k: &str| k.starts_with('p') || k.starts_with("rho"))?, scalars(r, "(flat)", &all)?),
        (f, e) => (
            match f {
                Some(d) => scalars(d, "fields", &all)?,
                None => RelaxationDict::default(),
            },
            match e {
                Some(d) => scalars(d, "equations", &all)?,
                None => RelaxationDict::default(),
            },
        ),
    };
    let out = RelaxationFactors { fields, equations, ignore_default: false };
    for f in ["h", "p_rgh"] {
        if out.field(f, true).is_some() && !out.relaxes_field(f) {
            return Err(ctx.e(format!("relaxationFactors.fields: `{f}Final` without `{f}` or `default`: upstream stores no previous iteration for `{f}` and aborts; refused")));
        }
    }
    Ok(out)
}

#[cfg(feature = "std")]
impl OnePhaseCcCase {
    /// Read a GeN-Foam `onePhase` region case with `solveFluidMechanics false`: dictionaries under
    /// `root/constant/fluidRegion`, `root/system`, fields under `root/<startTime>/fluidRegion`, mesh from
    /// `poly_mesh_dir`.
    ///
    /// # Errors
    /// [`AppBuilderError::Case`] for a missing or unreadable file and for every selection not ported (see the
    /// module documentation).
    pub fn read(root: &Path, poly_mesh_dir: &Path) -> Result<Self, AppBuilderError> {
        Self::read_with_options(root, poly_mesh_dir, &CaseReadOptions::default())
    }

    /// [`Self::read`] with the stage edits of `Allrun` as values (see [`CaseReadOptions`]).
    ///
    /// # Errors
    /// As [`Self::read`]; `GAMG` is refused unless `options.gamg` substitutes it.
    #[allow(clippy::too_many_lines)]
    pub fn read_with_options(root: &Path, poly_mesh_dir: &Path, options: &CaseReadOptions) -> Result<Self, AppBuilderError> {
        let ctx = Ctx { root: root.to_path_buf() };
        let region = "fluidRegion";
        let mesh = read_poly_mesh(poly_mesh_dir)?;
        let zones = read_cell_zones(poly_mesh_dir)?;
        let g = Geometry::new(mesh.clone());
        let n = g.n_cells();
        let zone_cells = |names: &str| -> Result<Vec<usize>, AppBuilderError> {
            let mut cells = Vec::new();
            for z in names.split(':') {
                let zone = zones
                    .iter()
                    .find(|cz| cz.name == z)
                    .ok_or_else(|| ctx.e(format!("unknown cellZone `{z}`")))?;
                cells.extend(zone.cells.iter().copied());
            }
            Ok(cells)
        };

        let gd = ctx.read(&format!("constant/{region}/g"))?;
        let gravity = match gd.get("value") {
            Some(FoamEntry::List(v)) if v.len() == 3 => Vector3::new(
                v[0].as_scalar().unwrap_or(0.0),
                v[1].as_scalar().unwrap_or(0.0),
                v[2].as_scalar().unwrap_or(0.0),
            ),
            _ => return Err(ctx.e("g: missing `value (x y z)`")),
        };

        let cd = ctx.read("system/controlDict")?;
        let (start_time, end_time, delta_t) = match options.controls {
            Some(c) => (c.start_time, c.end_time, c.delta_t),
            None => (scalar(&cd, "startTime").unwrap_or(0.0), ctx.f64(&cd, "endTime")?, ctx.f64(&cd, "deltaT")?),
        };
        let include_kinetic_energy = bool_entry(&cd, "includeKineticEnergy").unwrap_or(true);
        let max_co = scalar(&cd, "maxCo").unwrap_or(1.0);
        let max_delta_t = scalar(&cd, "maxDeltaT").unwrap_or(1.0e300);
        let is_one_phase = |name: &str| -> bool {
            cd.get_dict("regionSolvers").and_then(|rs| word(rs, name)).as_deref() == Some("onePhase")
        };
        let in_multi = cd
            .get_dict("regionSolvers")
            .map(|rs| rs.iter().any(|(_, e)| matches!(e, FoamEntry::Word(w) if w == "multiPhysicsSolver")))
            .unwrap_or(false);
        if options.check_region_solvers && !is_one_phase(region) && !in_multi {
            return Err(ctx.e("regionSolvers: fluidRegion is neither onePhase nor inside a multiPhysicsSolver"));
        }

        // ---- fvSolution
        let fs = ctx.read(&format!("system/{region}/fvSolution"))?;
        let pimple = ctx.sub(&fs, "PIMPLE")?;
        let solve_flow = match (options.solve_fluid_mechanics, bool_entry(pimple, "solveFluidMechanics")) {
            (Some(b), _) | (None, Some(b)) => b,
            (None, None) => return Err(ctx.e("PIMPLE.solveFluidMechanics must be given")),
        };
        if bool_entry(pimple, "solveEnergy") != Some(true) {
            return Err(ctx.e("PIMPLE.solveEnergy must be true"));
        }
        let n_outer = scalar(pimple, "nOuterCorrectors").unwrap_or(1.0) as usize;
        let residual_control = read_residual_control(&ctx, pimple)?;
        let pimple_controls = PimpleControls {
            n_outer,
            min_n_outer: scalar(pimple, "minNOuterCorrectors").unwrap_or(1.0) as usize,
            residual_control,
            final_on_last_pimple_iter_only: bool_entry(pimple, "finalOnLastPimpleIterOnly").unwrap_or(false),
            n_correctors: scalar(pimple, "nCorrectors").unwrap_or(1.0) as usize,
            n_non_orth: scalar(pimple, "nNonOrthogonalCorrectors").unwrap_or(0.0) as usize,
        };
        let solvers = ctx.sub(&fs, "solvers")?;
        let h_solver = |name: &str| -> Result<(f64, f64, usize), AppBuilderError> {
            match last_match(solvers, name) {
                Some(FoamEntry::SubDict(d)) => {
                    if word(d, "solver").as_deref() != Some("PBiCGStab")
                        || word(d, "preconditioner").as_deref() != Some("diagonal")
                    {
                        return Err(ctx.e(format!("solvers for `{name}`: only PBiCGStab with the diagonal preconditioner is ported")));
                    }
                    Ok((
                        scalar(d, "tolerance").unwrap_or(1e-6),
                        scalar(d, "relTol").unwrap_or(0.0),
                        scalar(d, "minIter").unwrap_or(0.0) as usize,
                    ))
                }
                _ => Err(ctx.e(format!("fvSolution: no solver entry matches `{name}`"))),
            }
        };
        let (h_tolerance, h_rel_tol, h_min_iter) = h_solver("h")?;
        // `hFinal` is selected in the flagged iteration only; like `kFinal` an absent entry is an error upstream when it is selected
        let (h_tolerance_final, h_rel_tol_final, h_min_iter_final) = if last_match(solvers, "hFinal").is_some() {
            h_solver("hFinal")?
        } else {
            (h_tolerance, h_rel_tol, h_min_iter)
        };
        let relax = read_relaxation_factors(&ctx, &fs)?;
        let controls = OnePhaseCcControls {
            pimple: pimple_controls,
            h_tolerance,
            h_rel_tol,
            h_min_iter,
            h_tolerance_final,
            h_rel_tol_final,
            h_min_iter_final,
            relax,
            include_kinetic_energy,
            max_co,
            max_delta_t,
        };

        // ---- turbulence: alphat is read, never solved
        let tp = ctx.read(&format!("constant/{region}/turbulenceProperties"))?;
        match word(&tp, "simulationType").as_deref() {
            Some("RAS") => {
                let ras = ctx.sub(&tp, "RAS")?;
                if word(ras, "RASModel").as_deref() != Some("porousKEpsilon") {
                    return Err(ctx.e("RASModel other than porousKEpsilon is not ported"));
                }
            }
            _ => return Err(ctx.e("simulationType other than RAS is not ported")),
        }

        // ---- thermo
        let th = ctx.read(&format!("constant/{region}/thermophysicalProperties"))?;
        let tt = ctx.sub(&th, "thermoType")?;
        for (k, want) in [
            ("type", "heRhoThermo"),
            ("mixture", "pureMixture"),
            ("transport", "const"),
            ("thermo", "hConst"),
            ("equationOfState", "Boussinesq"),
            ("energy", "sensibleEnthalpy"),
        ] {
            if word(tt, k).as_deref() != Some(want) {
                return Err(ctx.e(format!("thermoType.{k} must be {want} (the only package ported here)")));
            }
        }
        let mx = ctx.sub(&th, "mixture")?;
        let eos = ctx.sub(mx, "equationOfState")?;
        let thd = ctx.sub(mx, "thermodynamics")?;
        let tr = ctx.sub(mx, "transport")?;
        let thermo = BoussinesqHConst {
            cp: ctx.f64(thd, "Cp")?,
            tref: scalar(thd, "Tref").unwrap_or(TSTD),
            hsref: scalar(thd, "Href").unwrap_or(0.0),
            rho0: ctx.f64(eos, "rho0")?,
            t0: ctx.f64(eos, "T0")?,
            beta: ctx.f64(eos, "beta")?,
            mu: ctx.f64(tr, "mu")?,
            r_pr: 1.0 / ctx.f64(tr, "Pr")?,
        };

        // ---- phaseProperties: structure, power model, closures
        let pp = ctx.read(&format!("constant/{region}/phaseProperties"))?;
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_structure = vec![0.0; n];
        let mut dh = vec![1.0; n];
        let mut ia_act = vec![0.0; n];
        let mut fixed_cells = vec![false; n];
        let mut momentum_source = vec![Vector3::ZERO; n];
        let mut t_fixed_default = vec![0.0; n];
        let mut t_profiles: Vec<(Vec<usize>, TimeProfile)> = Vec::new();
        for (key, e) in sp.iter() {
            let FoamEntry::SubDict(zd) = e else { continue };
            if key == "heatExchangers" {
                // `structure.C:265` skips it as a zone key; read after the loop (`constructHeatExchangers`, README round 44)
                continue;
            }
            if key == "powerOffCriterionModel" {
                return Err(ctx.e(format!("structureProperties.{key} is not ported")));
            }
            let cells = zone_cells(key)?;
            if solve_flow {
                if let Some(FoamEntry::List(v)) = zd.get("momentumSource") {
                    if zd.get("momentumSourceTimeProfile").is_some() || zd.get_dict("momentumSourceTimeProfile").is_some() {
                        return Err(ctx.e("momentumSourceTimeProfile is not ported"));
                    }
                    if v.len() != 3 {
                        return Err(ctx.e("momentumSource must be a vector"));
                    }
                    let src = Vector3::new(
                        v[0].as_scalar().unwrap_or(0.0),
                        v[1].as_scalar().unwrap_or(0.0),
                        v[2].as_scalar().unwrap_or(0.0),
                    );
                    for &c in &cells {
                        momentum_source[c] = src;
                    }
                }
            }
            let vf = ctx.f64(zd, "volumeFraction")?;
            let d = ctx.f64(zd, "Dh")?;
            for &c in &cells {
                alpha_structure[c] = vf;
                dh[c] = d;
            }
            for (k2, _) in zd.iter() {
                if !matches!(k2, "volumeFraction" | "Dh" | "powerModel" | "momentumSource") {
                    return Err(ctx.e(format!("structureProperties.{key}.{k2} is not ported")));
                }
            }
            if let Some(pm) = zd.get_dict("powerModel") {
                let ty = word(pm, "type").unwrap_or_default();
                if radial_power::RADIAL_TYPES.contains(&ty.as_str()) {
                    // read after this loop, with the structure alpha of every zone (gh:#1879)
                    continue;
                }
                if ty != "fixedTemperature" {
                    return Err(ctx.e(radial_power::refusal_reason(&ty)));
                }
                match pm.get_dict("temperatureTimeProfile") {
                    Some(d) => t_profiles.push((cells.clone(), read_table_time_profile(&ctx, d, "temperatureTimeProfile")?)),
                    None if pm.get("temperatureTimeProfile").is_some() => {
                        return Err(ctx.e("temperatureTimeProfile must be a dictionary"))
                    }
                    None => {}
                }
                let ia = ctx.f64(pm, "volumetricArea")?;
                let t_pm = scalar(pm, "T");
                for &c in &cells {
                    ia_act[c] = ia;
                    fixed_cells[c] = true;
                    if let Some(t) = t_pm {
                        t_fixed_default[c] = t;
                    }
                }
            }
        }
        let pm_dict = ctx.sub(&pp, "physicsModels")?;
        let htcd = ctx.sub(pm_dict, "heatTransferModels")?;
        let mut htc_model: Vec<Option<FsHtcModel>> = vec![None; n];
        for (zk, e) in htcd.iter() {
            let FoamEntry::SubDict(md) = e else { continue };
            let model = match ctx.word(md, "type")?.as_str() {
                "constant" => FsHtcModel::Constant(ctx.f64(md, "value")?),
                other => return Err(ctx.e(format!("heatTransferModels.{zk}: `{other}` is not ported here"))),
            };
            for c in zone_cells(zk)? {
                htc_model[c] = Some(model);
            }
        }
        // dragModels: the energy equation does not see `Kd`; the solved flow does (`ReynoldsPower` only)
        let dragd = ctx.sub(pm_dict, "dragModels")?;
        let mut drag: Vec<Option<ReynoldsPowerDrag>> = vec![None; n];
        if solve_flow {
            for (zk, e) in dragd.iter() {
                let FoamEntry::SubDict(md) = e else { continue };
                if ctx.word(md, "type")? != "ReynoldsPower" {
                    return Err(ctx.e(format!("dragModels.{zk}: only ReynoldsPower is ported for the solved flow")));
                }
                let model = ReynoldsPowerDrag {
                    coeff: ctx.f64(md, "coeff")?,
                    exp: ctx.f64(md, "exp")?,
                    constant: scalar(md, "const").unwrap_or(0.0),
                };
                for c in zone_cells(zk)? {
                    drag[c] = Some(model);
                }
            }
        }
        let min_re = scalar(&pp, "residualFluidStructureRe").unwrap_or(10.0);
        let heat_exchangers = match sp.get_dict("heatExchangers") {
            Some(hxd) => super::case::read_heat_exchangers(&ctx, hxd, poly_mesh_dir, &g, &zone_cells)?,
            None => HeatExchangerSet::default(),
        };

        // ---- fields at the start time
        let tdir = alloc::format!("{start_time}");
        let fdir = root.join(&tdir).join(region);
        let sfield = |name: &str| read_vol_scalar_field(&fdir.join(name), n);
        let hand = options.fields.as_ref();
        // ---- the radial-conduction power models (gh:#1879): built here because they need every zone's structure alpha
        let has_radial = sp.iter().any(|(_, e)| match e {
            FoamEntry::SubDict(zd) => zd
                .get_dict("powerModel")
                .and_then(|pm| word(pm, "type"))
                .is_some_and(|t| radial_power::RADIAL_TYPES.contains(&t.as_str())),
            _ => false,
        });
        let q_file = if has_radial && fdir.join("powerDensityNeutronics").exists() { Some(sfield("powerDensityNeutronics")?) } else { None };
        let radial = radial_power::read_radial_power_models(
            &ctx,
            sp,
            &zone_cells,
            &alpha_structure,
            &mesh.cell_volumes,
            &fdir,
            options.radial_corrected_ring_areas,
            q_file,
            None,
        )?;
        if hand.is_some() && !radial.models.is_empty() {
            return Err(ctx.e("radial power models cannot continue from handed-over fluid fields (their own state is not handed over)"));
        }
        if radial.needs_laplacian && !solve_flow {
            // `fvc::laplacian(Tmatrix)` is the `uncorrected` form only (the flow reader checks the same entry when it runs)
            let schemes = ctx.read("system/fluidRegion/fvSchemes")?;
            let d = ctx.sub(&schemes, "laplacianSchemes")?;
            let uncorrected = match d.get("default") {
                Some(FoamEntry::Tokens(vs)) => vs.iter().any(|v| matches!(v, outram_foam_basic_lib::io::FoamValue::Word(w) if w == "uncorrected")),
                _ => false,
            };
            if !uncorrected {
                return Err(ctx.e("lumpedNuclearStructure with kappaMatrix != 0 needs laplacianSchemes `Gauss linear uncorrected` (the only fvc::laplacian ported)"));
            }
        }
        if let Some(h) = hand {
            if [h.t.len(), h.p.len(), h.u.len(), h.alphat.len(), h.t_fixed.len(), h.htc.len(), h.power_density.len()].iter().any(|&l| l != n)
                || h.alpha_phi.len() != g.n_faces()
                || h.alpha_rho_phi.len() != g.n_faces()
            {
                return Err(ctx.e("the handed-over fluid fields do not match the mesh"));
            }
        }
        let t = match hand { Some(h) => h.t.clone(), None => sfield("T")? };
        let p = match hand { Some(h) => h.p.clone(), None => sfield("p")? };
        let alphat = match hand { Some(h) => h.alphat.clone(), None => sfield("alphat")? };
        // upstream creates what a ROOT case lacks: `T.fixedTemperature` (`T` of the power-model dictionary in its zone,
        // 0 elsewhere), zero `htc` and `powerDensityNeutronicsToLiquid`; only a case that solves the flow may start
        // from them (the frozen-flux region needs a restart, its flux comes from the file)
        let exists = |name: &str| fdir.join(name).exists();
        let t_fixed = if let Some(h) = hand {
            h.t_fixed.clone()
        } else if solve_flow && !exists("T.fixedTemperature") {
            t_fixed_default
        } else {
            sfield("T.fixedTemperature")?
        };
        let htc0 = if let Some(h) = hand {
            h.htc.clone()
        } else if solve_flow && !exists("htc") {
            vec![0.0; n]
        } else {
            sfield("htc")?
        };
        let power_density = if let Some(h) = hand {
            h.power_density.clone()
        } else if solve_flow && !exists("powerDensityNeutronicsToLiquid") {
            vec![0.0; n]
        } else {
            sfield("powerDensityNeutronicsToLiquid")?
        };
        if solve_flow && pp.get_dict("fluidProperties").is_some() {
            return Err(ctx.e("fluidProperties/initialPowerDensity/initialAlpha are not ported for the solved flow"));
        }
        let u = match hand { Some(h) => h.u.clone(), None => read_vol_vector_field(&fdir.join("U"), n)? };
        let patches: Vec<(String, usize)> = mesh.patches.iter().map(|q| (q.name.clone(), q.size)).collect();
        let ni = g.n_internal();
        let surface = |name: &str| -> Result<Vec<f64>, AppBuilderError> {
            let path = fdir.join(name);
            let text = std::fs::read_to_string(&path).map_err(|e| AppBuilderError::Io { path: path.clone(), source: e })?;
            let (internal, per_patch) =
                parse_scalar_field_text(&text, ni, &patches).map_err(|m| ctx.e(format!("{name}: {m}")))?;
            let mut out = vec![0.0; g.n_faces()];
            out[..ni].copy_from_slice(&internal);
            for (q, vals) in mesh.patches.iter().zip(per_patch) {
                for (k, v) in vals.into_iter().enumerate() {
                    out[q.start + k] = v;
                }
            }
            Ok(out)
        };
        let flow = if solve_flow {
            let mut fc = flow::read_flow_case(&ctx, &g, &tdir, pimple, &fs, &pp, momentum_source, drag, options.gamg)?;
            fc.turbulence = turbulence::read_turbulence_case(&ctx, &g, &tdir, &fs, &tp, sp, poly_mesh_dir, &zone_cells)?;
            Some(fc)
        } else {
            None
        };
        let (alpha_rho_phi, alpha_phi) = if hand.is_none() && solve_flow && !exists("alphaPhi") {
            // `fluid::initAlphaPhis`: alphaPhi = interpolate(alpha) phi, alphaRhoPhi = interpolate(rho0) alphaPhi
            let fcase = flow.as_ref().expect("flow case");
            let mut uf = Vector::uniform(&g, Vector3::ZERO);
            uf.c = u.clone();
            uf.bc = fcase.u_bc.clone();
            uf.correct_bc(&g);
            let phi = fv::flux(&g, &uf);
            let alpha_s = Scalar {
                c: alpha_structure.iter().map(|s| 1.0 - s).collect(),
                b: (ni..g.n_faces()).map(|f| 1.0 - alpha_structure[mesh.owner[f]]).collect(),
                bc: vec![Bc::Calculated; mesh.patches.len()],
            };
            let af = fv::interp_linear(&g, &alpha_s);
            let ap: Vec<f64> = (0..g.n_faces()).map(|f| af[f] * phi[f]).collect();
            let arp: Vec<f64> = ap.iter().map(|v| thermo.rho0 * v).collect();
            (arp, ap)
        } else {
            match hand { Some(h) => (h.alpha_rho_phi.clone(), h.alpha_phi.clone()), None => (surface("alphaRhoPhi")?, surface("alphaPhi")?) }
        };

        Ok(Self {
            mesh,
            gravity,
            thermo,
            controls,
            start_time,
            end_time,
            delta_t,
            alpha_structure,
            dh,
            radial_power: radial.models,
            power_density_neutronics: radial.q_neutronics,
            radial_needs_laplacian: radial.needs_laplacian,
            ia_act,
            heat_exchangers,
            t_fixed,
            t_profiles,
            htc_model,
            htc0,
            t,
            p,
            u,
            alphat,
            alpha_rho_phi,
            alpha_phi,
            power_density,
            min_re,
            flow,
        })
    }
}

/// The `onePhase` region, energy only, frozen flux.
#[derive(Debug, Clone)]
pub struct OnePhaseCcSolver {
    case: OnePhaseCcCase,
    geom: Geometry,
    /// `alpha = 1 - structure` (cells).
    alpha: Vec<f64>,
    /// Interpolated `alpha` on faces (constant).
    alpha_f: Vec<f64>,
    he: Vec<f64>,
    he_old: Vec<f64>,
    t: Vec<f64>,
    /// `Tact`: `T.fixedTemperature` on the power-model cells, 0 elsewhere.
    t_act: Vec<f64>,
    /// `T_.oldTime()` of the `fixedTemperature` models: `Tact` at the start of the time step (gh:#1813).
    t_act_old: Vec<f64>,
    /// `iAact_` = the `fixedTemperature` area plus `iA` of every radial model (`structure` constructor: `iAact_ += iter()->iA()`).
    ia_act: Vec<f64>,
    /// The radial-conduction power models (gh:#1879), state included.
    radial: Vec<RadialPowerModel>,
    /// `structureProperties.heatExchangers` with `iAHX` and `T.HX` (README round 44, gh:#1847).
    heat_exchangers: HeatExchangerSet,
    /// The structure's `powerDensityNeutronics` per cell.
    q_neutronics: Vec<f64>,
    /// `T.prevIter()` stored by `EEqn_1p.H` before the solve (`HT = Hs*T.prevIter()`).
    t_prev: Vec<f64>,
    /// `heatFlux.structure` (`H Twall - HT`) on the radial models' cells, 0 elsewhere.
    structure_heat_flux: Vec<f64>,
    /// CONTROL (gh:#1879): build `HT` from the post-solve `T` instead of `T.prevIter()`.
    ctl_ht_post_solve: bool,
    /// CONTROL (gh:#1879): evaluate `fvc::laplacian(Tmatrix)` as 0.
    ctl_no_laplacian: bool,
    htc: Vec<f64>,
    re: Vec<f64>,
    cont_err: Vec<f64>,
    power_density: Vec<f64>,
    dt: f64,
    time: f64,
    /// `U` (cells and boundary; frozen in the energy-only region).
    u: Vector,
    /// `p` (frozen in the energy-only region).
    p: Vec<f64>,
    /// `p.oldTime()`: `None` until first requested (OpenFOAM stores old times lazily).
    p_old: Option<Vec<f64>>,
    /// Reproduce the lazy first-step old-time pressure of `fvc::ddt(p_)` (default true).
    lazy_p_old: bool,
    steps_begun: usize,
    alpha_rho_phi: Vec<f64>,
    alpha_phi: Vec<f64>,
    /// The solver's `phi_` (Courant number).
    phi_total: Vec<f64>,
    flow: Option<FlowState>,
    /// `porousKEpsilon` (`turbulence true`).
    turb: Option<TurbState>,
    /// `customPimpleControl` and the region's `solverPerformanceDict` / `finalIteration` flag (gh:#1855).
    pimple: PimpleState,
    /// `he.prevIter()` (stored at each `loop()` when `fields` relaxes `h`).
    he_prev: Vec<f64>,
    /// `p_rgh.prevIter()` (stored at each `loop()` when `fields` relaxes `p_rgh`).
    p_rgh_prev: Vec<f64>,
    /// `Kd.prevIter()` (`myOps::storePrevIterIfRelax`).
    kd_prev: Vec<f64>,
    /// CONTROL (gh:#1866): see [`OnePhaseCcSolver::with_first_piso_residual_of_first_pass_only`].
    first_piso_first_pass_only: bool,
    /// `htc.prevIter()` (`myOps::storePrevIterIfRelax`).
    htc_prev: Vec<f64>,
    /// `(initial residual, iterations)` of the `h` solve of each outer iteration of the last [`Self::correct_physics`].
    h_solves: Vec<(f64, usize)>,
}

/// What one PIMPLE outer iteration of the energy equation reported.
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseCcReport {
    /// Initial residual of the `h` solve of each outer iteration.
    pub h_residuals: Vec<f64>,
    /// `T (avg min max)` after each outer iteration: (volume-weighted mean, min, max).
    pub t_stats: Vec<(f64, f64, f64)>,
    /// Outer iterations run in this call (0 for a step the stale `converged_` flag skips).
    pub outer_iterations: usize,
    /// The outer iteration that ran with `isFinalIteration()` true (the flagged one), if any.
    pub final_iteration_at: Option<usize>,
}

impl OnePhaseCcSolver {
    /// Build the region at the case's start state (`onePhase::onePhase` constructor: phase fractions,
    /// `contErr`, thermo `he`).
    #[must_use]
    pub fn new(case: OnePhaseCcCase) -> Self {
        let geom = Geometry::new(case.mesh.clone());
        let n = geom.n_cells();
        // fluid_ = 1.0 - structure_
        let alpha: Vec<f64> = case.alpha_structure.iter().map(|s| 1.0 - s).collect();
        let ni0 = geom.n_internal();
        let sa = Scalar {
            c: alpha.clone(),
            b: (ni0..geom.n_faces()).map(|f| alpha[geom.mesh.owner[f]]).collect(),
            bc: vec![Bc::Calculated; geom.mesh.patches.len()],
        };
        let alpha_f = fv::interp_linear(&geom, &sa);
        let he: Vec<f64> = case.t.iter().map(|&t| case.thermo.hs(t)).collect();
        // `correctContErr`: ddt(alpha, rho0) is exactly 0 at construction (old time = current).
        let dv = fv::div(&geom, &case.alpha_rho_phi);
        let cont_err: Vec<f64> = (0..n).map(|c| 0.0 + dv[c]).collect();
        let mut u = Vector::uniform(&geom, Vector3::ZERO);
        u.c = case.u.clone();
        if let Some(fc) = case.flow.as_ref() {
            u.bc = fc.u_bc.clone();
        }
        u.correct_bc(&geom);
        let flow = case.flow.as_ref().map(|fc| flow::FlowState::new(&geom, &case, fc, &u));
        let turb = case
            .flow
            .as_ref()
            .and_then(|fc| fc.turbulence.as_ref().map(|tc| TurbState::new(&geom, tc, &fc.nut, &fc.nut_boundary, &case.alphat)));
        let (case_p, case_arp, case_ap) = (case.p.clone(), case.alpha_rho_phi.clone(), case.alpha_phi.clone());
        let mut t_act: Vec<f64> = (0..n).map(|c| if case.ia_act[c] != 0.0 { case.t_fixed[c] } else { 0.0 }).collect();
        // `structure` constructor: `iAact_ += iter()->iA(); iter()->correctT(Tact_)` for every model of the table
        let mut ia_act = case.ia_act.clone();
        let radial = case.radial_power.clone();
        for m in &radial {
            for &c in m.cells() {
                ia_act[c] += m.interfacial_area(c);
                t_act[c] = m.surface_temperature(c);
            }
        }
        let q_neutronics = case.power_density_neutronics.clone();
        let heat_exchangers = case.heat_exchangers.clone();
        let t_act_old = t_act.clone();
        let htc = case.htc0.clone();
        let re = vec![case.min_re; n];
        let (t, power_density, dt, time) = (case.t.clone(), case.power_density.clone(), case.delta_t, case.start_time);
        let mut s = Self {
            case,
            geom,
            alpha,
            alpha_f,
            he_old: he.clone(),
            he,
            t,
            t_act,
            t_act_old,
            ia_act,
            radial,
            heat_exchangers,
            q_neutronics,
            t_prev: Vec::new(),
            structure_heat_flux: vec![0.0; n],
            ctl_ht_post_solve: false,
            ctl_no_laplacian: false,
            htc,
            re,
            cont_err,
            power_density,
            dt,
            time,
            u,
            p: case_p,
            p_old: None,
            lazy_p_old: true,
            steps_begun: 0,
            alpha_rho_phi: case_arp,
            phi_total: case_ap.clone(),
            alpha_phi: case_ap,
            flow,
            turb,
            pimple: PimpleState::new(),
            he_prev: Vec::new(),
            p_rgh_prev: Vec::new(),
            kd_prev: Vec::new(),
            first_piso_first_pass_only: false,
            htc_prev: Vec::new(),
            h_solves: Vec::new(),
        };
        // the structure constructor calls `correctT` once, which runs `temperatureUpdate()` at the start time
        s.temperature_update();
        s.update_re();
        s
    }

    /// Read a case directory and build the solver.
    ///
    /// # Errors
    /// See [`OnePhaseCcCase::read`].
    #[cfg(feature = "std")]
    pub fn from_case_dir(root: &Path, poly_mesh_dir: &Path) -> Result<Self, AppBuilderError> {
        Ok(Self::new(OnePhaseCcCase::read(root, poly_mesh_dir)?))
    }

    /// The fields as the stage writes them at its end (`alphaPhi`/`alphaRhoPhi` as last set at the start of the last `pEqn`,
    /// the lag upstream writes): what the next stage reads from its start directory.
    #[must_use]
    pub fn fluid_fields(&self) -> FluidFields {
        FluidFields {
            t: self.t.clone(),
            p: self.p.clone(),
            u: self.u.c.clone(),
            alphat: self.turb.as_ref().map_or_else(|| self.case.alphat.clone(), |t| t.alphat.c.clone()),
            alpha_rho_phi: self.alpha_rho_phi.clone(),
            alpha_phi: self.alpha_phi.clone(),
            t_fixed: self.case.t_fixed.clone(),
            htc: self.htc.clone(),
            power_density: self.power_density.clone(),
        }
    }

    /// The case as read.
    #[must_use]
    pub fn case(&self) -> &OnePhaseCcCase {
        &self.case
    }

    /// Temperature per cell [K].
    #[must_use]
    pub fn temperature(&self) -> &[f64] {
        &self.t
    }

    /// Sensible enthalpy per cell [J/kg].
    #[must_use]
    pub fn enthalpy(&self) -> &[f64] {
        &self.he
    }

    /// The thermo's own density `rho(T)` per cell (what the coupling maps as `thermo:rho`).
    #[must_use]
    pub fn thermo_density(&self) -> Vec<f64> {
        self.t.iter().map(|&t| self.case.thermo.rho(t)).collect()
    }

    /// `Re` per cell (`FSPair::correct`).
    #[must_use]
    pub fn reynolds(&self) -> &[f64] {
        &self.re
    }

    /// `htc` per cell.
    #[must_use]
    pub fn htc(&self) -> &[f64] {
        &self.htc
    }

    /// Replace `powerDensityNeutronicsToLiquid` (what `meshHandler::mapTheseFields` writes).
    pub fn set_power_density(&mut self, pd: &[f64]) {
        self.power_density.clear();
        self.power_density.extend_from_slice(pd);
    }

    /// Time of the last completed step start (`runTime.value()`).
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// `runTime++` with step `dt`: stores the old-time enthalpy (OpenFOAM stores it at the first
    /// modification in a time step; coupled iterations within the step reuse it).
    pub fn begin_time_step(&mut self, dt: f64) {
        self.dt = dt;
        self.time += dt;
        self.he_old = self.he.clone();
        self.t_act_old = self.t_act.clone();
        let uc = self.u.c.clone();
        if let Some(fs) = self.flow.as_mut() {
            fs.u_old = uc;
            fs.solves.clear();
        }
        // `p.oldTime()` is stored at the first non-const access of a new time step; in the FIRST step OpenFOAM has no
        // old-time copy yet and `fvc::ddt(p_)` (the energy equation) is the first request (round 3's quirk).
        self.p_old = if self.steps_begun == 0 && self.lazy_p_old { None } else { Some(self.p.clone()) };
        self.pimple.begin_time_step();
        self.h_solves.clear();
        if let Some(t) = self.turb.as_mut() {
            t.k_old = t.k.c.clone();
            t.step_solves.clear();
            t.epsilon_old = t.epsilon.c.clone();
        }
        self.steps_begun += 1;
    }

    /// DEPRECATED (gh:#1837): the old-time pressure of step 1 is the initial pressure, not the pressure after the first
    /// `p_rgh` solve that upstream's lazy `oldTime()` gives. The ablation of the default.
    #[must_use]
    pub fn without_first_step_lazy_old_pressure(mut self) -> Self {
        self.lazy_p_old = false;
        self
    }

    /// `U` per cell.
    #[must_use]
    pub fn velocity(&self) -> &[Vector3] {
        &self.u.c
    }

    /// `p` per cell [Pa].
    #[must_use]
    pub fn pressure(&self) -> &[f64] {
        &self.p
    }

    /// `p_rgh` per cell [Pa] (`None` for the frozen-flux region).
    #[must_use]
    pub fn p_rgh(&self) -> Option<&[f64]> {
        self.flow.as_ref().map(|f| f.p_rgh.c.as_slice())
    }

    /// `alphaPhi` on every face, as last set (at the start of the last `pEqn`).
    #[must_use]
    pub fn alpha_phi(&self) -> &[f64] {
        &self.alpha_phi
    }

    /// The pressure solves of the last time step (empty without flow).
    #[must_use]
    pub fn pressure_solves(&self) -> &[PressureSolve] {
        self.flow.as_ref().map_or(&[], |f| f.solves.as_slice())
    }

    /// `FSPair::correct` quantities that do not need the fluid dynamics: `Re` (`nu = mu/rho(T)`).
    fn update_re(&mut self) {
        let th = &self.case.thermo;
        for c in 0..self.geom.n_cells() {
            let nu = th.mu / th.rho(self.t[c]);
            // normalized = alpha / (1 - structure)
            let norm = self.alpha[c] / (1.0 - self.case.alpha_structure[c]);
            let re = norm * self.u.c[c].mag() * self.case.dh[c] / nu;
            self.re[c] = re.max(self.case.min_re);
        }
    }

    /// `fixedTemperature::temperatureUpdate` (gh:#1813): for each zone with a `temperatureTimeProfile`,
    /// `T_[c] = f(t) * (T_old[c] / f(t - deltaT))` with `t = timeOutputValue()`, `T_old` the value at the start of the step
    /// (`T_.oldTime()`, stored once per step, so repeating it within a step gives the same result). Then `Tact` takes it.
    fn temperature_update(&mut self) {
        for (cells, profile) in &self.case.t_profiles {
            let f_now = profile.value(Time::new::<second>(self.time));
            let f_old = profile.value(Time::new::<second>(self.time - self.dt));
            for &c in cells {
                self.t_act[c] = f_now * (self.t_act_old[c] / f_old);
            }
        }
    }

    /// `structure::correct(HT, H)` for the radial power models (gh:#1879; `structure.C`): for each model
    /// `correct(HT, H)` then `correctT(Tact_)`, then `Twall = Tact` and `heatFlux = H Twall - HT` on the model cells.
    /// `HT = Hs*T.prevIter()` and `H = Hs` (`FSPair_.htc()`). A `lumpedNuclearStructure` first takes
    /// `fvc::laplacian(Tmatrix)` of its own `Tmatrix` as it stands (upstream does it at the start of its `correct`).
    ///
    /// # Panics
    /// On a singular radial matrix (upstream's `Foam::solve` is a `FatalError`: the case is unphysical).
    fn structure_correct(&mut self) {
        // after the power models: the heat exchangers' `correct(HT, H, THX)` (structure.C:1000-1018): `sHT = htc*T` with the
        // CURRENT temperature, `HT = Hs*T.prevIter()` (README round 44)
        if self.heat_exchangers.is_present() {
            let n = self.geom.n_cells();
            let ht: Vec<f64> = (0..n).map(|c| self.htc[c] * self.t_prev[c]).collect();
            let s_ht: Vec<f64> = (0..n).map(|c| self.htc[c] * self.t[c]).collect();
            let h = self.htc.clone();
            self.heat_exchangers.correct(&ht, &h, &h, &s_ht);
        }
        if self.radial.is_empty() {
            return;
        }
        let n = self.geom.n_cells();
        let tp: &[f64] = if self.ctl_ht_post_solve { &self.t } else { &self.t_prev };
        let ht: Vec<f64> = (0..n).map(|c| self.htc[c] * tp[c]).collect();
        let h = self.htc.clone();
        let volumes = self.geom.mesh.cell_volumes.clone();
        let time_index = self.steps_begun as i64;
        for i in 0..self.radial.len() {
            let lap = match (self.radial[i].tmatrix(), self.ctl_no_laplacian) {
                (Some(tm), false) => fvc_laplacian_uncorrected(&self.geom, tm),
                _ => vec![0.0; n],
            };
            let step = StepInputs { dt: self.dt, time_index, q_neutronics: self.q_neutronics.clone(), laplacian_t_matrix: lap, volumes: volumes.clone() };
            if let Err(e) = self.radial[i].correct(&ht, &h, &step) {
                panic!("radial power model {}: {e}", self.radial[i].type_name());
            }
            for &c in self.radial[i].cells() {
                self.t_act[c] = self.radial[i].surface_temperature(c);
                self.structure_heat_flux[c] = h[c] * self.t_act[c] - ht[c];
            }
        }
    }

    /// The radial power models with their state (gh:#1879).
    #[must_use]
    pub fn radial_power_models(&self) -> &[RadialPowerModel] {
        &self.radial
    }

    /// `heatFlux.structure` on the radial models' cells (`H Twall - HT`; 0 elsewhere).
    #[must_use]
    pub fn structure_heat_flux(&self) -> &[f64] {
        &self.structure_heat_flux
    }

    /// `(T.fuelAvForNeutronics, T.cladAvForNeutronics)` per cell: the `TFuelAv`/`TCladAv` the radial models write into the
    /// structure for the neutronics (0 where no model sets them, as the fields' `zeroGradient 0` initialisation).
    #[must_use]
    pub fn fuel_clad_temperatures(&self) -> (Vec<f64>, Vec<f64>) {
        let n = self.geom.n_cells();
        let (mut fuel, mut clad) = (vec![0.0; n], vec![0.0; n]);
        for m in &self.radial {
            for &c in m.cells() {
                let (f, cl) = m.coupling_temperatures(c);
                fuel[c] = f;
                clad[c] = cl;
            }
        }
        (fuel, clad)
    }

    /// Hand the structure's `powerDensityNeutronics` [W/m^3] over from the neutronics (the radial models' `q`).
    pub fn set_structure_power_density(&mut self, q: &[f64]) {
        self.q_neutronics.clear();
        self.q_neutronics.extend_from_slice(q);
    }

    /// `Tact` (`T.activeStructure`) per cell.
    #[must_use]
    pub fn active_structure_temperature(&self) -> &[f64] {
        &self.t_act
    }

    /// CONTROL, NOT upstream (gh:#1879): build `HT` from the post-solve `T` instead of `T.prevIter()`.
    #[must_use]
    pub fn with_post_solve_temperature_for_ht(mut self) -> Self {
        self.ctl_ht_post_solve = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1879): `fvc::laplacian(Tmatrix)` of the lumped model taken as 0.
    #[must_use]
    pub fn without_lumped_laplacian(mut self) -> Self {
        self.ctl_no_laplacian = true;
        self
    }

    /// `correctModels(true, true)`: `Re`, then `htc_[cell] = value` on the model cells.
    fn correct_models(&mut self) {
        self.update_re();
        self.correct_kd();
        // `myOps::storePrevIterIfRelax(htc_)`, the model values, `htc_.relax()` (`fields` entry `htc`, `htcFinal` when final)
        let a = self.case.controls.relax.field("htc", self.pimple.final_iteration);
        if a.is_some() {
            self.htc_prev = self.htc.clone();
        }
        for c in 0..self.geom.n_cells() {
            if let Some(m) = self.case.htc_model[c] {
                self.htc[c] = m.value();
            }
        }
        if let Some(a) = a {
            for c in 0..self.geom.n_cells() {
                self.htc[c] = self.htc_prev[c] + a * (self.htc[c] - self.htc_prev[c]);
            }
        }
    }

    /// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`), see `one_phase_fc::relax_diag`: boundary
    /// coefficients are folded into the diagonal; a boundary face with a negative convective internal
    /// coefficient is not neutral. Returns `D' - D` per cell.
    fn relax_diag(&self, e: &mut Eqn, factor: f64, inflow2: &[f64]) -> Vec<f64> {
        let n = self.geom.n_cells();
        let mut sum_off = vec![0.0; n];
        for f in 0..self.geom.n_internal() {
            sum_off[e.ldu.owner[f]] += e.ldu.upper[f].abs();
            sum_off[e.ldu.neighbour[f]] += e.ldu.lower[f].abs();
        }
        let mut delta = vec![0.0; n];
        for c in 0..n {
            let d0 = e.ldu.diag[c];
            let d = (d0 + inflow2[c]).abs().max(sum_off[c]) / factor;
            e.ldu.diag[c] = d;
            delta[c] = d - d0;
        }
        delta
    }

    /// `EEqn_1p.H`; returns the initial residual of the `h` solve and the `T (avg min max)` it prints.
    fn correct_energy(&mut self) -> (f64, (f64, f64, f64)) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let rdt = 1.0 / self.dt;
        let th = self.case.thermo.clone();
        let rho0 = th.rho0;
        let cp = th.cp;
        let phi = &self.alpha_rho_phi;
        let he_scalar = Scalar {
            c: self.he.clone(),
            b: vec![0.0; g.n_faces() - ni],
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        // `T.storePrevIter()` (EEqn_1p.H, before the solve): the temperature `structure_.correct(Hs*T.prevIter(), Hs)` sees
        if !self.radial.is_empty() {
            self.t_prev = self.t.clone();
        }
        // fvm::ddt(alpha, rho0, he)
        let mut e = Eqn::new(&g);
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.ldu.diag[c] += rdt * self.alpha[c] * rho0 * v;
            e.source[c] += rdt * self.alpha[c] * rho0 * self.he_old[c] * v;
        }
        e.div_upwind(&g, phi, &he_scalar);
        let ce: Vec<f64> = self.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &self.he);
        // - laplacian(interpolate(alpha) interpolate(alphaEff), he), alphaEff = alpha_thermo + alphat
        let ah = th.alpha_h();
        let ae = Scalar {
            c: (0..n).map(|c| ah + self.turb.as_ref().map_or(self.case.alphat[c], |t| t.alphat.c[c])).collect(),
            b: vec![0.0; g.n_faces() - ni],
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        let ae_f = fv::interp_linear(&g, &ae);
        let gam: Vec<f64> = (0..g.n_faces()).map(|f| self.alpha_f[f] * ae_f[f]).collect();
        e.neg_laplacian(&g, &gam, &he_scalar);
        // right-hand side: alpha rho0 (U.g); alpha ddt(p) = 0 exactly (p is never modified)
        let mut rhs = vec![0.0; n];
        for c in 0..n {
            rhs[c] = self.alpha[c] * rho0 * self.u.c[c].dot(self.case.gravity);
        }
        e.su(&g, &rhs);
        // `EEqn -= alpha*fvc::ddt(p_)`: `p.oldTime()` is created by this very call in step 1 (see `begin_time_step`)
        {
            let p_now = &self.p;
            let p_old = self.p_old.get_or_insert_with(|| p_now.clone());
            let dp: Vec<f64> = (0..n).map(|c| self.alpha[c] * (rdt * (p_now[c] - p_old[c]))).collect();
            e.su(&g, &dp);
        }
        if self.case.controls.include_kinetic_energy {
            let mut k = Scalar {
                c: vec![0.0; n],
                b: vec![0.0; g.n_faces() - ni],
                bc: vec![Bc::Calculated; g.mesh.patches.len()],
            };
            for c in 0..n {
                let u = self.u.c[c] * 0.5;
                k.c[c] = u.dot(self.u.c[c]);
            }
            // wedge/wall boundary values of K: the cell value (a wedge patch value is the cell value
            // rotated; the rotation preserves |U|)
            for f in ni..g.n_faces() {
                k.b[f - ni] = if self.flow.is_some() {
                    let ub = self.u.b[f - ni];
                    (ub * 0.5).dot(ub)
                } else {
                    k.c[g.mesh.owner[f]]
                };
            }
            let kf = fv::interp_upwind(&g, &k, phi);
            let flux: Vec<f64> = (0..g.n_faces()).map(|f| phi[f] * kf[f]).collect();
            let div_k = fv::div(&g, &flux);
            let mut s = vec![0.0; n];
            for c in 0..n {
                // K.oldTime() of a fresh local field is K itself; rho = rho0 for both times
                let ddt_k = rdt * (self.alpha[c] * rho0 * k.c[c] - self.alpha[c] * rho0 * k.c[c]);
                s[c] = -(ddt_k + div_k[c] - self.cont_err[c] * k.c[c]);
            }
            e.su(&g, &s);
        }
        // structure_.linearizedSemiImplicitHeatSource(fluid, Hs)
        let mut su = vec![0.0; n];
        let mut sp = vec![0.0; n];
        for c in 0..n {
            let ih = self.ia_act[c] * self.htc[c];
            su[c] = ih * (self.t_act[c] - self.t[c] + self.he[c] / cp);
            sp[c] = ih / cp;
        }
        if self.heat_exchangers.is_active() {
            // `structure.C:1148-1156`: `Q += iAHX*H*(THX-T+he/Cp) - fvm::Sp(iAHX*H/Cp, he)` BEFORE `EEqn -= Q`: the source and the
            // diagonal receive `(V*act + V*hx)` as ONE sum (negations are exact), not two additions (README round 44)
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                let (su_hx, sp_hx) = self.heat_exchangers.source_and_coeff(c, self.htc[c], self.t[c], self.he[c], cp);
                e.source[c] += v * su[c] + v * su_hx;
                e.ldu.diag[c] += v * sp[c] + v * sp_hx;
            }
        } else {
            e.su(&g, &su);
            e.sp(&g, &sp);
        }
        // - alpha * powerDensity
        let pd: Vec<f64> = (0..n).map(|c| self.alpha[c] * self.power_density[c]).collect();
        e.su(&g, &pd);
        // EEqn.relax()
        if let Some(fct) = self.case.controls.relax.equation("h", self.pimple.final_iteration).filter(|f| *f > 0.0) {
            let mut inflow2 = vec![0.0; n];
            for f in ni..g.n_faces() {
                if g.active[f] {
                    inflow2[g.mesh.owner[f]] += 2.0 * (-phi[f]).max(0.0);
                }
            }
            let delta = self.relax_diag(&mut e, fct, &inflow2);
            for c in 0..n {
                e.source[c] += delta[c] * self.he[c];
            }
        }
        let mut x = self.he.clone();
        // `EEqn.solve()` selects `hFinal` when `isFinalIteration()`
        let fin = self.pimple.final_iteration;
        let ctl = &self.case.controls;
        let (tol, rel_tol, min_iter) =
            if fin { (ctl.h_tolerance_final, ctl.h_rel_tol_final, ctl.h_min_iter_final) } else { (ctl.h_tolerance, ctl.h_rel_tol, ctl.h_min_iter) };
        let perf = pbicgstab_diagonal(&e.ldu, &e.source, &mut x, tol, rel_tol, DEFAULT_MAX_ITER, min_iter);
        self.pimple.record("h", perf.initial_residual);
        self.h_solves.push((perf.initial_residual, perf.iterations));
        self.he = x;
        // `he.relax()`: `he = prevIter + alpha (he - prevIter)`, the `fields` entry `h` (`hFinal` in the flagged iteration)
        if let Some(a) = self.case.controls.relax.field("h", fin) {
            for c in 0..n {
                self.he[c] = self.he_prev[c] + a * (self.he[c] - self.he_prev[c]);
            }
        }
        // thermo.correct(): T from he by Newton, seeded with the current T
        for c in 0..n {
            self.t[c] = th.t_from_hs(self.he[c], self.t[c]);
        }
        let vols = &g.mesh.cell_volumes;
        let (mut sv, mut st) = (0.0, 0.0);
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for c in 0..n {
            sv += vols[c];
            st += vols[c] * self.t[c];
            tmin = tmin.min(self.t[c]);
            tmax = tmax.max(self.t[c]);
        }
        // `structure_.correct(Hs*T.prevIter(), Hs)` runs at the END of EEqn_1p.H: `fixedTemperature::correctT` -> `temperatureUpdate`
        self.temperature_update();
        self.structure_correct();
        (perf.initial_residual, (st / sv, tmin, tmax))
    }

    /// `onePhase::correctPhysics` with `solveEnergy true`, `solveFluidMechanics false`: `nOuterCorrectors`
    /// outer iterations, each `correctModels` then `correctEnergy`. Returns the residual
    /// (`max` of the initial residuals) and the per-iteration report.
    pub fn correct_physics(&mut self) -> (f64, OnePhaseCcReport) {
        let mut residual = 0.0_f64;
        let mut rep = OnePhaseCcReport { h_residuals: Vec::new(), t_stats: Vec::new(), outer_iterations: 0, final_iteration_at: None };
        self.h_solves.clear();
        let pc = self.case.controls.pimple.clone();
        // `while (pimple_.loop())`
        while self.pimple.loop_step(&pc) == LoopStep::Run {
            let outer = self.pimple.corr;
            self.store_prev_iter_fields();
            if self.pimple.final_iteration && rep.final_iteration_at.is_none() {
                rep.final_iteration_at = Some(outer);
            }
            self.correct_models();
            if self.flow.is_some() {
                self.correct_fluid_mechanics(outer);
                if let Some(fs) = self.flow.as_ref() {
                    self.phi_total = fs.phi_solver.clone();
                }
            }
            let (r, st) = self.correct_energy();
            residual = residual.max(r);
            rep.h_residuals.push(r);
            rep.t_stats.push(st);
            rep.outer_iterations = outer;
        }
        (residual, rep)
    }

    /// `solutionControl::storePrevIterFields()` for the fields the cell-centred region relaxes with a `fields` entry: `h` and `p_rgh`
    /// (`Kd` and `htc` store theirs in `FSPair::correct`).
    fn store_prev_iter_fields(&mut self) {
        let r = &self.case.controls.relax;
        if r.relaxes_field("h") {
            self.he_prev = self.he.clone();
        }
        if r.relaxes_field("p_rgh") {
            if let Some(fs) = self.flow.as_ref() {
                self.p_rgh_prev = fs.p_rgh.c.clone();
            }
        }
    }

    /// The `(initial residual, iterations)` of the `h` solve of each outer iteration of the last step.
    #[must_use]
    pub fn h_solves(&self) -> &[(f64, usize)] {
        &self.h_solves
    }

    /// CONTROL, NOT upstream (gh:#1855): ignore `PIMPLE.residualControl`, as the port did before round 31 (it refused the entry).
    #[must_use]
    pub fn without_residual_control(mut self) -> Self {
        self.pimple = self.pimple.without_residual_control();
        self
    }

    /// CONTROL, NOT upstream (gh:#1866): run ONE pressure pass whatever `nNonOrthogonalCorrectors` says (the port before round 33,
    /// which refused the entry); the single pass then selects `p_rghFinal` by `finalInnerIter()` for `nNonOrthogonalCorrectors 0`.
    #[must_use]
    pub fn without_non_orthogonal_correctors(mut self) -> Self {
        self.case.controls.pimple.n_non_orth = 0;
        if let Some(f) = self.case.flow.as_mut() {
            f.n_non_orth_correctors = 0;
        }
        self
    }

    /// CONTROL, NOT upstream (gh:#1866): `updateFirstPISOPrevPIMPLE()` after the FIRST pressure pass only, as if it were not inside
    /// the `correctNonOrthogonal` loop.
    #[must_use]
    pub fn with_first_piso_residual_of_first_pass_only(mut self) -> Self {
        self.first_piso_first_pass_only = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1855): ignore the `default` entries of `relaxationFactors` (the port before round 31).
    #[must_use]
    pub fn without_relaxation_default(mut self) -> Self {
        self.case.controls.relax.ignore_default = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1855): `p_rghFinal` only at the last corrector of the last outer iteration (the port before round 31).
    #[must_use]
    pub fn with_legacy_p_final_on_last_outer_only(mut self) -> Self {
        self.pimple = self.pimple.with_legacy_p_final_on_last_outer_only();
        self
    }

    /// The state of `customPimpleControl` (the stale flag, `isFinalIteration()`).
    #[must_use]
    pub fn pimple_state(&self) -> &PimpleState {
        &self.pimple
    }

    /// `onePhase::correctTightlyCoupledPhysics`: one `correctModels` and one `correctEnergy` (no PIMPLE loop).
    pub fn correct_tightly_coupled_physics(&mut self) -> f64 {
        self.correct_models();
        let (r, _) = self.correct_energy();
        r
    }

    /// `onePhase::correctCourant`: `(CoNum_, meanCoNum_)` = (max, mean) with the current `dt`.
    #[must_use]
    pub fn courant(&self) -> (f64, f64) {
        let g = &self.geom;
        let sum = fv::surface_sum_mag(g, &self.phi_total);
        let n = g.n_cells();
        let vols = &g.mesh.cell_volumes;
        let mut maxv = f64::MIN;
        let mut gsum = 0.0;
        let mut gvol = 0.0;
        for c in 0..n {
            let sp = sum[c] / self.alpha[c];
            maxv = maxv.max(sp / vols[c]);
            gsum += sp;
            gvol += vols[c];
        }
        (0.5 * maxv * self.dt, 0.5 * (gsum / gvol) * self.dt)
    }

    /// `onePhase::maxDeltaT`.
    #[must_use]
    pub fn max_delta_t(&self) -> f64 {
        let (co, _) = self.courant();
        let new_dt = self.case.controls.max_delta_t;
        let max_fact = self.case.controls.max_co / (co + SMALL);
        let fact = (max_fact.min(1.0 + 0.1 * max_fact)).min(1.2);
        (fact * self.dt).min(new_dt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newton_recovers_temperature_of_a_linear_enthalpy() {
        let th = BoussinesqHConst { cp: 1600.0, tref: TSTD, hsref: 0.0, rho0: 4125.0, t0: 900.0, beta: 2e-4, mu: 0.01, r_pr: 0.125 };
        let t = 1036.2258577635569;
        let back = th.t_from_hs(th.hs(t), 1000.0);
        assert!((back - t).abs() / t < 1e-14);
        assert_eq!(th.rho(900.0), 4125.0);
        assert_eq!(th.alpha_h(), 0.01 * 0.125);
    }

    #[test]
    fn surface_field_text_with_wedge_values_parses() {
        let text = "FoamFile{}\ninternalField nonuniform List<scalar> 2\n(\n1.5\n-2\n)\n;\nboundaryField\n{\n    front\n    {\n        type wedge;\n        value nonuniform List<scalar> 2\n(\n1e-11\n2e-11\n)\n;\n    }\n    wall\n    {\n        type calculated;\n        value uniform 0;\n    }\n}\n";
        let patches = vec![("front".to_string(), 2), ("wall".to_string(), 3)];
        let (i, b) = parse_scalar_field_text(text, 2, &patches).expect("parse");
        assert_eq!(i, vec![1.5, -2.0]);
        assert_eq!(b[0], vec![1e-11, 2e-11]);
        assert_eq!(b[1], vec![0.0; 3]);
    }
}
