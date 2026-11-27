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
//! ~~`solveFluidMechanics true` (UEqn/pEqn, `porousKEpsilon` solving: gh:#1775)~~ **CORRECTED 2026-11-26 (gh:#1837, round 19):** `solveFluidMechanics true` with `turbulence false` is ported ([`flow`]); ~~`turbulence true` (wall functions, `porousKEpsilon::correct`: gh:#1838, gh:#1839) is still refused~~ **CORRECTED 2026-11-26 (round 20): `turbulence true` is ported ([`turbulence`])**; ~~heat exchangers~~ (**CORRECTED 2026-11-27, README round 44, gh:#1847: ported, same-region secondary zone only**), pumps (a `momentumSource` pump IS read when the flow is solved),
//! passive structures, tortuosity, local frames; ~~power models other than `fixedTemperature`~~ **CORRECTED 2026-11-27 (gh:#1879, README round 37):**
//! `heatedPin`, `nuclearFuelPin`, `lumpedNuclearStructure` and `interpolatedNuclearFuelPin` are read ([`radial_power`]) and run through
//! `structure::correct` ([`OnePhaseCcSolver::radial_power_models`]); `nuclearFuelPinTest`, the FMU models and `fixedPower` are refused by name; heat-transfer models other than `constant`; thermo other than `heRhoThermo`
//! `hConst` `Boussinesq` `const`; ~~`residualControl` in `PIMPLE`~~ **CORRECTED 2026-11-27 (gh:#1855, round 31): `residualControl` (customPimpleControl), the `Final` selection it causes and the `relaxationFactors` `default` entries are ported ([`pimple`])**; `stopIfFirstResRelChangeAboveRelTol` (upstream aborts with SIGFPE). `dragModels` (`ReynoldsPower`) are read but
//! not evaluated: `Kd` is a momentum coefficient and does not enter the energy equation. **UPDATED 2026-11-27 (gh:#1870, README round 62):** with
//! the flow solved, `ReynoldsPower`, `Churchill`, `Rehme` and per-axis (`localX/Y/Z`) `dragModels` ARE evaluated ([`flow`]), a pump
//! `momentumSourceTimeProfile` is applied, `PBiCG`/`DILU` is selectable for `h`, `k`, `epsilon`; `localTortuosity` (the tensor-diffusivity Laplacian, gh:#1229 round 64) is ported through `tensor_laplacian`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;
#[cfg(feature = "std")]
use std::path::Path;

use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use uom::si::f64::{Length, Ratio, ThermalConductivity, Time};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::time::second;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::FsForcedConvectionHtc;
use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;

use crate::genfoam::common::TimeProfile;
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;

pub(super) mod flow;
mod pimple;
#[cfg(feature = "std")]
use super::radial_power;
mod turbulence;
pub use flow::{CellDrag, DragLaw, OnePhaseFlowCase, PressureSolve, ReynoldsPowerDrag};
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
use super::tensor_laplacian;
use super::thermo::PhaseThermo;
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

/// The linear solver the energy equation `h` gets (`solvers` entry: the exact keyword, else the LAST matching pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HSolver {
    /// `PBiCGStab` with the `diagonal` preconditioner (`pbicgstab_diagonal`; what the shipped `2D_fullCoupling` selects through its
    /// catch-all `".*"`: the log reads `diagonalPBiCGStab: Solving for h`).
    #[default]
    PBiCGStabDiagonal,
    /// `smoothSolver` with the `symGaussSeidel` smoother, `nSweeps 1`, `relTol 0`
    /// (`one_phase::smooth_solve_sym_gauss_seidel_openfoam`, differenced against the real binary in README round 46 and, for the
    /// cell-centred region, in round 55 variant V3).
    SmoothSymGaussSeidel,
    /// `PBiCG` with the `DILU` preconditioner (`legacy_fvm::pbicg::pbicg_dilu`, README rounds 23 and 62): the shipped `2D_FFTF`'s
    /// `"(h|k|epsilon)"` entries (the real log reads `DILUPBiCG: Solving for h`).
    PbicgDilu,
}

/// `fvSolution`/`controlDict` entries the stage reads.
#[derive(Debug, Clone, PartialEq)]
pub struct OnePhaseCcControls {
    /// `PIMPLE.nOuterCorrectors`, `minNOuterCorrectors`, `residualControl`, `finalOnLastPimpleIterOnly`, `nCorrectors`
    /// (`customPimpleControl`, gh:#1855).
    pub pimple: PimpleControls,
    /// The linear solver of `h`.
    pub h_solver: HSolver,
    /// The linear solver of `hFinal`.
    pub h_solver_final: HSolver,
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
    /// `type NusseltReynoldsPrandtlPower` (`NusseltFSHeatTransferCoefficient::value`, gh:#1913 round 55):
    /// `(kappa/Dh) (A + B pow(Re Pr, C))` (`C == D`) or `(kappa/Dh) (A + B pow(Re, C) pow(Pr, D))`, from the closure
    /// [`FsForcedConvectionHtc`]. `expTc != 0` (needs `T.wall`) is refused by name when the case is read.
    Nusselt(FsForcedConvectionHtc),
}

impl FsHtcModel {
    /// `FSHeatTransferCoefficientModel::value(celli)` [W/m^2/K] for the models that need no field (`constant`); `None` for a
    /// model that does ([`Self::value_with`]).
    #[must_use]
    pub fn value(&self) -> Option<f64> {
        match self {
            Self::Constant(v) => Some(*v),
            Self::Nusselt(_) => None,
        }
    }

    /// `FSHeatTransferCoefficientModel::value(celli)` with the cell's `Re`, `Pr` (the `fluid` placeholder fields), `kappa` (the
    /// placeholder field) and `Dh` (`fluid_.Dh()`).
    #[must_use]
    pub fn value_with(&self, re: f64, pr: f64, kappa: f64, dh: f64) -> f64 {
        match self {
            Self::Constant(v) => *v,
            Self::Nusselt(m) => m
                .heat_transfer_coefficient(
                    ReynoldsNumber::new::<ratio>(re),
                    Ratio::new::<ratio>(pr),
                    ThermalConductivity::new::<watt_per_meter_kelvin>(kappa),
                    Length::new::<meter>(dh),
                    None,
                )
                .get::<watt_per_square_meter_kelvin>(),
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

/// The passive sub-structure of `structure` (`structure.C:372-475`, `correct`: 1020-1055), per cell.
#[derive(Debug, Clone, PartialEq)]
pub struct PassiveStructure {
    /// `iApas_` (`volumetricArea.passiveStructure`) per cell; 0 outside the zones with `passiveProperties`.
    pub ia: Vec<f64>,
    /// `alphaRhoCppas_ = max(alphapas rhoCp, 1e-69)` per cell.
    pub alpha_rho_cp: Vec<f64>,
    /// `Tpas_` at the start time per cell (the dictionary `T` where the field is not on disk).
    pub t: Vec<f64>,
}

/// A read case (all fields at the start time).
#[derive(Debug, Clone)]
pub struct OnePhaseCcCase {
    /// Mesh.
    pub mesh: Arc<FvMesh>,
    /// Gravity [m/s^2].
    pub gravity: Vector3,
    /// Thermophysical package (`Boussinesq`+`hConst`, or `icoPolynomial`+`hPolynomial`+`polynomial`; gh:#1913 round 55).
    pub thermo: PhaseThermo,
    /// `simulationType laminar` (`Stokes`: `nut = 0`, `alphat = 0`, no turbulence fields on disk; gh:#1913 round 55).
    pub laminar: bool,
    /// `structureProperties.<zone>.passiveProperties` (`structure.C:372`); `None` when no zone has one.
    pub passive: Option<PassiveStructure>,
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
    /// `structureProperties.<zone>.localTortuosity` (gh:#1229, round 64): the diagonal of the global-frame tortuosity tensor per cell
    /// (identity outside the tortuous zones); `None` when no zone has one (`structure_.tortuosityPtr().valid()` false: the scalar-gamma
    /// Laplacians of `UEqn_1p.H`/`EEqn_1p.H`).
    pub tortuosity: Option<Vec<[f64; 3]>>,
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
        let v: f64 = outram_foam_basic_lib::io::parse_foam_f64(r[..end].trim()).ok_or_else(|| format!("bad uniform `{}`", r[..end].trim()))?;
        return Ok((vec![v; n], &r[end..]));
    }
    if let Some(r) = s.strip_prefix("nonuniform") {
        let r = r.trim_start();
        let r = r.strip_prefix("List<scalar>").unwrap_or(r).trim_start();
        let open = r.find('(').ok_or("nonuniform without `(`")?;
        let count: usize = r[..open].trim().parse().map_err(|_| "bad list size".to_string())?;
        let close = r[open..].find(')').ok_or("unterminated list")? + open;
        let vals: Result<Vec<f64>, _> = r[open + 1..close].split_whitespace().map(|w| outram_foam_basic_lib::io::parse_foam_f64(w).ok_or(())).collect();
        let vals = vals.map_err(|_| "bad number in list".to_string())?;
        if vals.len() != count {
            return Err(format!("list says {count} values, has {}", vals.len()));
        }
        return Ok((vals, &r[close + 1..]));
    }
    Err("neither uniform nor nonuniform".to_string())
}

/// One `FSDragCoefficientModel` dictionary: `ReynoldsPower` (`coeff`, `exp`, `const`), `Churchill` (`surfaceRoughness`) or `Rehme`
/// (`numberOfPins`, `pinDiameter`, `wireDiameter`, `wireLeadLen`, `wetWrapPerimeter`); every other model is refused by name.
#[cfg(feature = "std")]
fn read_drag_law(ctx: &Ctx, what: &str, md: &FoamDict) -> Result<DragLaw, AppBuilderError> {
    match ctx.word(md, "type")?.as_str() {
        "ReynoldsPower" => Ok(DragLaw::ReynoldsPower(ReynoldsPowerDrag {
            coeff: ctx.f64(md, "coeff")?,
            exp: ctx.f64(md, "exp")?,
            constant: scalar(md, "const").unwrap_or(0.0),
        })),
        "Churchill" => Ok(DragLaw::Wall(FsWallFriction::Churchill { surface_roughness: ctx.f64(md, "surfaceRoughness")? })),
        "Rehme" => Ok(DragLaw::Wall(FsWallFriction::rehme_from_geometry(
            ctx.f64(md, "numberOfPins")?,
            ctx.f64(md, "pinDiameter")?,
            ctx.f64(md, "wireDiameter")?,
            ctx.f64(md, "wireLeadLen")?,
            ctx.f64(md, "wetWrapPerimeter")?,
        ))),
        other => Err(ctx.e(format!("dragModels.{what}: `{other}` is not ported for the solved flow (ReynoldsPower, Churchill, Rehme)"))),
    }
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
        let g = Geometry::new_openfoam(mesh.clone());
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
        let h_solver = |name: &str| -> Result<(f64, f64, usize, HSolver), AppBuilderError> {
            match last_match(solvers, name) {
                Some(FoamEntry::SubDict(d)) => {
                    let kind = match (word(d, "solver").as_deref(), word(d, "preconditioner").as_deref(), word(d, "smoother").as_deref()) {
                        (Some("PBiCGStab"), Some("diagonal"), _) => HSolver::PBiCGStabDiagonal,
                        (Some("PBiCG"), Some("DILU"), _) => HSolver::PbicgDilu,
                        (Some("smoothSolver"), _, Some("symGaussSeidel")) => {
                            // the translated solver takes no `relTol` and runs `nSweeps 1` (README round 55)
                            if scalar(d, "relTol").unwrap_or(0.0) != 0.0 {
                                return Err(ctx.e(format!("solvers for `{name}`: smoothSolver with relTol != 0 is not ported")));
                            }
                            if scalar(d, "nSweeps").unwrap_or(1.0) != 1.0 {
                                return Err(ctx.e(format!("solvers for `{name}`: smoothSolver with nSweeps != 1 is not ported")));
                            }
                            if scalar(d, "maxIter").unwrap_or(1000.0) != 1000.0 {
                                return Err(ctx.e(format!("solvers for `{name}`: smoothSolver with maxIter != 1000 is not ported")));
                            }
                            HSolver::SmoothSymGaussSeidel
                        }
                        _ => {
                            return Err(ctx.e(format!(
                                "solvers for `{name}`: only PBiCGStab/diagonal, PBiCG/DILU and smoothSolver with symGaussSeidel are ported"
                            )))
                        }
                    };
                    Ok((
                        scalar(d, "tolerance").unwrap_or(1e-6),
                        scalar(d, "relTol").unwrap_or(0.0),
                        scalar(d, "minIter").unwrap_or(0.0) as usize,
                        kind,
                    ))
                }
                _ => Err(ctx.e(format!("fvSolution: no solver entry matches `{name}`"))),
            }
        };
        let (h_tolerance, h_rel_tol, h_min_iter, h_kind) = h_solver("h")?;
        // `hFinal` is selected in the flagged iteration only; like `kFinal` an absent entry is an error upstream when it is selected
        let (h_tolerance_final, h_rel_tol_final, h_min_iter_final, h_kind_final) = if last_match(solvers, "hFinal").is_some() {
            h_solver("hFinal")?
        } else {
            (h_tolerance, h_rel_tol, h_min_iter, h_kind)
        };
        let relax = read_relaxation_factors(&ctx, &fs)?;
        let controls = OnePhaseCcControls {
            pimple: pimple_controls,
            h_solver: h_kind,
            h_solver_final: h_kind_final,
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
        let laminar = word(&tp, "simulationType").as_deref() == Some("laminar");
        match word(&tp, "simulationType").as_deref() {
            // `laminar` with no `laminar` sub-dictionary is `Stokes` (`laminarModel::New`): `nut = 0`, `nuEff = nu`, and
            // `ThermalDiffusivity::alphat()` is a zero field (gh:#1913 round 55); a `laminar { model ...; }` selection is refused
            Some("laminar") => {
                if tp.get("laminar").is_some() {
                    return Err(ctx.e("turbulenceProperties.laminar: only the default `Stokes` laminar model is ported"));
                }
            }
            Some("RAS") => {
                let ras = ctx.sub(&tp, "RAS")?;
                if word(ras, "RASModel").as_deref() != Some("porousKEpsilon") {
                    return Err(ctx.e("RASModel other than porousKEpsilon is not ported"));
                }
            }
            _ => return Err(ctx.e("simulationType other than RAS and laminar is not ported")),
        }

        // ---- thermo
        let th = ctx.read(&format!("constant/{region}/thermophysicalProperties"))?;
        let tt = ctx.sub(&th, "thermoType")?;
        let thermo = if word(tt, "equationOfState").as_deref() == Some("icoPolynomial") {
            // `icoPolynomial` + `hPolynomial` + `polynomial` (the 2D_fullCoupling sodium): the stack of `two_phase::thermo`, `psi = 0`
            for (k, want) in [("type", "heRhoThermo"), ("mixture", "pureMixture"), ("transport", "polynomial"), ("thermo", "hPolynomial"), ("energy", "sensibleEnthalpy")] {
                if word(tt, k).as_deref() != Some(want) {
                    return Err(ctx.e(format!("thermoType.{k} must be {want} with icoPolynomial (the only package ported besides Boussinesq/hConst/const)")));
                }
            }
            super::case::read_thermo(&ctx, &format!("constant/{region}/thermophysicalProperties"))?
        } else {
            for (k, want) in [
                ("type", "heRhoThermo"),
                ("mixture", "pureMixture"),
                ("transport", "const"),
                ("thermo", "hConst"),
                ("equationOfState", "Boussinesq"),
                ("energy", "sensibleEnthalpy"),
            ] {
                if word(tt, k).as_deref() != Some(want) {
                    return Err(ctx.e(format!("thermoType.{k} must be {want} (the packages ported here: Boussinesq/hConst/const and icoPolynomial/hPolynomial/polynomial)")));
                }
            }
            let mx = ctx.sub(&th, "mixture")?;
            let eos = ctx.sub(mx, "equationOfState")?;
            let thd = ctx.sub(mx, "thermodynamics")?;
            let tr = ctx.sub(mx, "transport")?;
            PhaseThermo::BoussinesqHConst(BoussinesqHConst {
                cp: ctx.f64(thd, "Cp")?,
                tref: scalar(thd, "Tref").unwrap_or(TSTD),
                hsref: scalar(thd, "Href").unwrap_or(0.0),
                rho0: ctx.f64(eos, "rho0")?,
                t0: ctx.f64(eos, "T0")?,
                beta: ctx.f64(eos, "beta")?,
                mu: ctx.f64(tr, "mu")?,
                r_pr: 1.0 / ctx.f64(tr, "Pr")?,
            })
        };

        // ---- phaseProperties: structure, power model, closures
        let pp = ctx.read(&format!("constant/{region}/phaseProperties"))?;
        let sp = ctx.sub(&pp, "structureProperties")?;
        let mut alpha_structure = vec![0.0; n];
        let mut dh = vec![1.0; n];
        let mut ia_act = vec![0.0; n];
        let mut fixed_cells = vec![false; n];
        let mut momentum_source = vec![Vector3::ZERO; n];
        let mut tortuosity: Option<Vec<[f64; 3]>> = None;
        let mut momentum_profiles: Vec<(Vec<usize>, Vector3, TimeProfile)> = Vec::new();
        let mut t_fixed_default = vec![0.0; n];
        let mut passive: Option<PassiveStructure> = None;
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
                    if v.len() != 3 {
                        return Err(ctx.e("momentumSource must be a vector"));
                    }
                    if zd.get("momentumSourceTimeProfile").is_some() && zd.get_dict("momentumSourceTimeProfile").is_none() {
                        return Err(ctx.e("momentumSourceTimeProfile must be a dictionary"));
                    }
                    let src = Vector3::new(
                        v[0].as_scalar().unwrap_or(0.0),
                        v[1].as_scalar().unwrap_or(0.0),
                        v[2].as_scalar().unwrap_or(0.0),
                    );
                    for &c in &cells {
                        momentum_source[c] = src;
                    }
                    if let Some(td) = zd.get_dict("momentumSourceTimeProfile") {
                        momentum_profiles.push((cells.clone(), src, read_table_time_profile(&ctx, td, "momentumSourceTimeProfile")?));
                    }
                }
            }
            if let Some(lt) = zd.get("localTortuosity") {
                // `structure.C:663-711`: `diag(localTortuosity)` in the global frame (a `localReferenceFrame` is refused below)
                let FoamEntry::List(v) = lt else {
                    return Err(ctx.e("localTortuosity must be a vector"));
                };
                if v.len() != 3 {
                    return Err(ctx.e("localTortuosity must be a vector"));
                }
                let d = [v[0].as_scalar().unwrap_or(1.0), v[1].as_scalar().unwrap_or(1.0), v[2].as_scalar().unwrap_or(1.0)];
                let field = tortuosity.get_or_insert_with(|| vec![[1.0; 3]; n]);
                for &c in &cells {
                    field[c] = d;
                }
            }
            let vf = ctx.f64(zd, "volumeFraction")?;
            let d = ctx.f64(zd, "Dh")?;
            for &c in &cells {
                alpha_structure[c] = vf;
                dh[c] = d;
            }
            if let Some(pd) = zd.get_dict("passiveProperties") {
                // `structure.C:372-475`: `volumetricArea`; `rho` and `Cp` or `rhoCp` (all three, or none, is a FatalError);
                // `volumeFraction` (else the zone's, `alphapas_ = *this` when no `alpha.structure` field is on disk) and `T`
                for (k3, _) in pd.iter() {
                    if !matches!(k3, "volumetricArea" | "rho" | "Cp" | "rhoCp" | "volumeFraction" | "T") {
                        return Err(ctx.e(format!("structureProperties.{key}.passiveProperties.{k3} is not ported")));
                    }
                }
                let (has_rho, has_cp, has_rho_cp) = (pd.get("rho").is_some(), pd.get("Cp").is_some(), pd.get("rhoCp").is_some());
                let rho_cp = if has_rho && has_cp && has_rho_cp {
                    return Err(ctx.e(format!("structureProperties.{key}.passiveProperties: specify either rhoCp or both rho and Cp but not all of them")));
                } else if has_rho && has_cp {
                    ctx.f64(pd, "rho")? * ctx.f64(pd, "Cp")?
                } else if has_rho_cp {
                    ctx.f64(pd, "rhoCp")?
                } else {
                    return Err(ctx.e(format!("structureProperties.{key}.passiveProperties: specify either rhoCp or both rho and Cp")));
                };
                let pas_alpha = scalar(pd, "volumeFraction").unwrap_or(vf);
                let pas_ia = ctx.f64(pd, "volumetricArea")?;
                let pas_t = ctx.f64(pd, "T")?;
                let ps = passive.get_or_insert_with(|| PassiveStructure { ia: vec![0.0; n], alpha_rho_cp: vec![0.0; n], t: vec![0.0; n] });
                for &c in &cells {
                    ps.ia[c] = pas_ia;
                    ps.alpha_rho_cp[c] = pas_alpha * rho_cp;
                    ps.t[c] = pas_t;
                }
            }
            for (k2, _) in zd.iter() {
                if !matches!(k2, "volumeFraction" | "Dh" | "powerModel" | "momentumSource" | "momentumSourceTimeProfile" | "passiveProperties" | "localTortuosity") {
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
                "NusseltReynoldsPrandtlPower" | "Nusselt" => {
                    if scalar(md, "expTc").unwrap_or(0.0) != 0.0 {
                        return Err(ctx.e(format!("heatTransferModels.{zk}: `expTc != 0` needs the structure wall temperature `T.wall` and is not ported")));
                    }
                    FsHtcModel::Nusselt(super::case::read_nusselt(&ctx, md)?)
                }
                other => return Err(ctx.e(format!("heatTransferModels.{zk}: `{other}` is not ported here"))),
            };
            for c in zone_cells(zk)? {
                htc_model[c] = Some(model);
            }
        }
        // dragModels: the energy equation does not see `Kd`; the solved flow does (`ReynoldsPower` only)
        let dragd = ctx.sub(pm_dict, "dragModels")?;
        let mut drag: Vec<Option<CellDrag>> = vec![None; n];
        if solve_flow {
            for (zk, e) in dragd.iter() {
                let FoamEntry::SubDict(md) = e else { continue };
                let cell = if md.get("type").is_some() {
                    CellDrag { isotropic: true, laws: [Some(read_drag_law(&ctx, zk, md)?), None, None] }
                } else {
                    // `FSDragFactor`: no `type` -> per-axis models `localX`, `localY`, `localZ` (an absent axis has no model)
                    for (k, _) in md.iter() {
                        if !matches!(k, "localX" | "localY" | "localZ") {
                            return Err(ctx.e(format!("dragModels.{zk}.{k} is not ported (localX, localY, localZ)")));
                        }
                    }
                    let mut laws = [None, None, None];
                    for (i, key) in ["localX", "localY", "localZ"].iter().enumerate() {
                        if let Some(sd) = md.get_dict(key) {
                            laws[i] = Some(read_drag_law(&ctx, &alloc::format!("{zk}.{key}"), sd)?);
                        }
                    }
                    CellDrag { isotropic: false, laws }
                };
                for c in zone_cells(zk)? {
                    drag[c] = Some(cell);
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
        let alphat = match hand { Some(h) => h.alphat.clone(), None if laminar => vec![0.0; n], None => sfield("alphat")? };
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
            let mut fc = flow::read_flow_case(&ctx, &g, &tdir, pimple, &fs, &pp, momentum_source, drag, options.gamg, laminar)?;
            fc.momentum_profiles = momentum_profiles;
            fc.turbulence = if laminar {
                None
            } else {
                let tc = turbulence::read_turbulence_case(&ctx, &g, &tdir, &fs, &tp, sp, poly_mesh_dir, &zone_cells)?;
                if tc.is_some() && thermo.boussinesq_rho0().is_none() {
                    return Err(ctx.e("RAS `turbulence true` is ported for the Boussinesq equation of state only (the model's `rho` is the constant `rho0`)"));
                }
                tc
            };
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
            let phi = fv::flux_openfoam(&g, &uf);
            let alpha_s = Scalar {
                c: alpha_structure.iter().map(|s| 1.0 - s).collect(),
                b: (ni..g.n_faces()).map(|f| 1.0 - alpha_structure[mesh.owner[f]]).collect(),
                bc: vec![Bc::Calculated; mesh.patches.len()],
            };
            let af = fv::interp_linear_openfoam(&g, &alpha_s);
            let ap: Vec<f64> = (0..g.n_faces()).map(|f| af[f] * phi[f]).collect();
            let arp: Vec<f64> = match thermo.boussinesq_rho0() {
                Some(rho0) => ap.iter().map(|v| rho0 * v).collect(),
                None => {
                    // `fluid.C:718`: `interpolate(this->rho()) alphaPhi`, `rho()` the thermo's `rho(p, T)` (cells; the boundary at the
                    // patch temperature: `fixedValue` T or the owner value)
                    let mut ts = Scalar { c: t.clone(), b: vec![0.0; g.n_faces() - ni], bc: fcase.t_bc.clone() };
                    ts.correct_bc(&g);
                    let rho_s = Scalar {
                        c: (0..n).map(|c| thermo.props(p[c], t[c]).rho).collect(),
                        b: (ni..g.n_faces()).map(|f| thermo.props(p[mesh.owner[f]], ts.b[f - ni]).rho).collect(),
                        bc: vec![Bc::Calculated; mesh.patches.len()],
                    };
                    let rf = fv::interp_linear_openfoam(&g, &rho_s);
                    (0..g.n_faces()).map(|f| rf[f] * ap[f]).collect()
                }
            };
            (arp, ap)
        } else {
            match hand { Some(h) => (h.alpha_rho_phi.clone(), h.alpha_phi.clone()), None => (surface("alphaRhoPhi")?, surface("alphaPhi")?) }
        };

        if let Some(td) = &tortuosity {
            tensor_laplacian::tortuosity_fields(&g, td).map_err(|m| ctx.e(m))?;
        }

        Ok(Self {
            mesh,
            gravity,
            thermo,
            laminar,
            passive,
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
            tortuosity,
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
    /// CONTROLS, NOT upstream (gh:#1229): the tortuosity left out of the momentum / the energy Laplacian (scalar-gamma form).
    ctl_no_tort_momentum: bool,
    ctl_no_tort_energy: bool,
    htc: Vec<f64>,
    re: Vec<f64>,
    /// `FSPair::lRe_`: the per-axis Reynolds number `max(alpha/nu U_k lDh_k, minRe)` (`FSPair.C:371`; the signed component, `lDh = Dh` as no
    /// `localDhAnisotropy` is ported) a per-axis drag model (`localX`/`localY`/`localZ`, `cmpt() >= 0`) evaluates (gh:#1870 round 62).
    lre: Vec<[f64; 3]>,
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
    kd_prev: Vec<[f64; 3]>,
    /// CONTROL (gh:#1866): see [`OnePhaseCcSolver::with_first_piso_residual_of_first_pass_only`].
    first_piso_first_pass_only: bool,
    /// `htc.prevIter()` (`myOps::storePrevIterIfRelax`).
    htc_prev: Vec<f64>,
    /// `(initial residual, iterations)` of the `h` solve of each outer iteration of the last [`Self::correct_physics`].
    h_solves: Vec<(f64, usize)>,
    /// `fluid::rho()` per cell: `rho0` for a `Boussinesq` equation of state, else the thermo's `rho(p, T)` (gh:#1913 round 55).
    rho: Vec<f64>,
    /// `fluid::rho()` on every boundary face (index `face - n_internal`).
    rho_b: Vec<f64>,
    /// `fluid::rho().oldTime()` per cell: its value at the start of the time step.
    rho_old: Vec<f64>,
    /// `fluid.Cp()` placeholder field (`fluid.C:597`, `EEqn_1p.H` end): `thermo.Cp()` as of the last `thermo.correct()`.
    cp_f: Vec<f64>,
    /// `fluid.kappa()` placeholder field: `thermo.kappa() = Cp alpha_` (`heThermo::kappa`).
    kappa_f: Vec<f64>,
    /// `fluid.Pr()` placeholder field: `Cp mu/kappa`.
    pr_f: Vec<f64>,
    /// `T.passiveStructure` per cell (the passive sub-structure; empty without `passiveProperties`).
    t_pas: Vec<f64>,
    /// `T.passiveStructure.oldTime()`: its value at the start of the time step.
    t_pas_old: Vec<f64>,
    /// CONTROL (gh:#1913): ignore the passive structure (no heat exchange, no `Tpas` update).
    ctl_no_passive: bool,
    /// CONTROL (gh:#1913): evaluate the `htc` models once (the first `correctModels`) and keep that field.
    ctl_freeze_htc: bool,
    /// `htc` has been evaluated once (for [`Self::with_frozen_htc`]).
    htc_evaluated: bool,
    /// A restart's first step (gh:#1919, README round 60): `Tpas.oldTime()` does not exist yet in the new process, is created by the first
    /// `structure::correct` as a copy, and the first `Tpas.correctBoundaryConditions()` (`storeOldTimes()`, the field's `timeIndex_` still that
    /// of the read) then re-stores it from the ALREADY UPDATED field, so the later outer iterations of the step integrate from that value.
    /// Set by [`Self::restart_at`], cleared by the first [`Self::passive_correct`].
    pas_old_lazy: bool,
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
        let geom = Geometry::new_openfoam(case.mesh.clone());
        let n = geom.n_cells();
        // fluid_ = 1.0 - structure_
        let alpha: Vec<f64> = case.alpha_structure.iter().map(|s| 1.0 - s).collect();
        let ni0 = geom.n_internal();
        let sa = Scalar {
            c: alpha.clone(),
            b: (ni0..geom.n_faces()).map(|f| alpha[geom.mesh.owner[f]]).collect(),
            bc: vec![Bc::Calculated; geom.mesh.patches.len()],
        };
        let alpha_f = fv::interp_linear_openfoam(&geom, &sa);
        let he: Vec<f64> = (0..case.t.len()).map(|c| case.thermo.he(case.p[c], case.t[c])).collect();
        // `correctContErr`: ddt(alpha, rho) is exactly 0 at construction (old time = current).
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
        let lre = vec![[case.min_re; 3]; n];
        let (t, power_density, dt, time) = (case.t.clone(), case.power_density.clone(), case.delta_t, case.start_time);
        let t_pas = case.passive.as_ref().map_or_else(Vec::new, |ps| ps.t.clone());
        let t_bc0: Vec<Bc> = case.flow.as_ref().map_or_else(|| vec![Bc::ZeroGradient; case.mesh.patches.len()], |f| f.t_bc.clone());
        let mut ts0 = Scalar { c: case.t.clone(), b: vec![0.0; geom.n_faces() - ni0], bc: t_bc0 };
        ts0.correct_bc(&geom);
        let (rho, rho_b) = match case.thermo.boussinesq_rho0() {
            Some(r0) => (vec![r0; n], vec![r0; geom.n_faces() - ni0]),
            None => (
                (0..n).map(|c| case.thermo.props(case.p[c], case.t[c]).rho).collect(),
                (ni0..geom.n_faces()).map(|f| case.thermo.props(case.p[geom.mesh.owner[f]], ts0.b[f - ni0]).rho).collect(),
            ),
        };
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
            ctl_no_tort_momentum: false,
            ctl_no_tort_energy: false,
            htc,
            re,
            lre,
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
            rho_old: rho.clone(),
            rho,
            rho_b,
            cp_f: vec![0.0; n],
            kappa_f: vec![0.0; n],
            pr_f: vec![0.0; n],
            t_pas_old: t_pas.clone(),
            t_pas,
            ctl_no_passive: false,
            ctl_freeze_htc: false,
            htc_evaluated: false,
            pas_old_lazy: false,
        };
        s.update_thermo_fields();
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
        (0..self.t.len()).map(|c| self.case.thermo.props(self.p[c], self.t[c]).rho).collect()
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
        self.rho_old = self.rho.clone();
        self.t_pas_old = self.t_pas.clone();
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

    /// What a RESTART from the written state at `start_time` rebuilds that an uninterrupted run keeps (gh:#1919, README round 60): a new
    /// process has no old-time copies (so the lazy first-step `p.oldTime()` of round 3 applies again), a fresh PIMPLE control state, the
    /// face flux `phi = fvc::flux(U)` (it is NOT written: `fluid.C:509-578` rebuilds it, while `alphaPhi`/`alphaRhoPhi` are read and
    /// kept), `magU = mag(U)`, and every old-time field equal to the field read. The cell fields (`T`, `U`, `p`, `p_rgh`, `h`, ...)
    /// are written at full precision and are kept as they are. `rebuild_phi` false is the CONTROL that keeps the last step's `phi` (README round 60, control d).
    pub fn restart_at(&mut self, start_time: f64, rebuild_phi: bool) {
        self.time = start_time;
        self.steps_begun = 0;
        self.p_old = None;
        self.pimple = PimpleState::new();
        self.h_solves.clear();
        // the thermo is constructed from the READ `T` and `p`: `h = he(p, T)` and `rho = rho(p, T)` (the running process holds the `h` its
        // last solve left and the density of the temperature before it)
        self.he = (0..self.t.len()).map(|c| self.case.thermo.he(self.p[c], self.t[c])).collect();
        self.update_thermo_fields();
        self.he_old = self.he.clone();
        self.rho_old = self.rho.clone();
        self.t_pas_old = self.t_pas.clone();
        self.pas_old_lazy = true;
        self.t_act_old = self.t_act.clone();
        // `onePhase` constructor `correctContErr()`: `ddt(alpha, rho)` is exactly 0 (old time = current), `contErr = div(alphaRhoPhi)`
        let dv = fv::div(&self.geom, &self.alpha_rho_phi);
        for c in 0..self.geom.n_cells() {
            self.cont_err[c] = 0.0 + dv[c];
        }
        let uc = self.u.c.clone();
        let phi = fv::flux_openfoam(&self.geom, &self.u);
        // `onePhase.C:133`: `phi_ = fluid_.alphaPhi()`, the field READ from disk (what the last step left in `alphaPhi`)
        let alpha_phi = self.alpha_phi.clone();
        if rebuild_phi {
            self.phi_total = alpha_phi.clone();
        }
        if let Some(fs) = self.flow.as_mut() {
            if rebuild_phi {
                fs.phi = phi;
                fs.phi_solver = alpha_phi;
            }
            fs.u_old = uc;
            fs.mag_u = self.u.c.iter().map(|v| v.mag()).collect();
            fs.solves.clear();
        }
    }

    /// A restart that READS the written fields (gh:#1925, README round 61): replace `U`, `p` and `T` by the values a time directory holds, BEFORE
    /// [`Self::restart_at`] rebuilds everything a new process derives from them (`h`, `rho`, `phi = fvc::flux(U)`, `contErr`, `magU`).
    ///
    /// Why it exists: a restart from the port's OWN steady state inherits that state's round-off (`U` differs from upstream's written one by
    /// 9.8e-14 relative), and the first `p_rgh` source is a cancellation of face fluxes (`contErr`, `div(phiHbyA)`: 1e-10 of fluxes of 2e-3),
    /// so a 1e-13 input noise becomes 1.9e-7 in the first initial residual. Fed upstream's own written `U`, `p`, `T`, every input of the first
    /// pressure corrector (`alpha`, `rho`, `p`, `T`, `U`, `phi`, `alphaPhi`, `alphaRhoPhi`, `contErr`, the momentum `A`/`H`/diagonal, `rAUf`,
    /// `phiHbyA`) is bit-identical (README round 61), i.e. the restart emulation itself is exact.
    ///
    /// # Panics
    /// If a length is not the cell count.
    pub fn adopt_written_state(&mut self, u: &[Vector3], p: &[f64], t: &[f64]) {
        assert!(u.len() == self.u.c.len() && p.len() == self.p.len() && t.len() == self.t.len(), "adopt_written_state: length is not the cell count");
        self.u.c.copy_from_slice(u);
        let g = self.geom.clone();
        self.u.correct_bc(&g);
        self.p.copy_from_slice(p);
        self.t.copy_from_slice(t);
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

    /// `phi` of the fluid (`fluid_.phi()`, the field `findObject<surfaceScalarField>("phi")` finds: the superficial flux) on every face, `None` for
    /// the frozen-flux region (gh:#1927, GEM).
    #[must_use]
    pub fn face_flux(&self) -> Option<&[f64]> {
        self.flow.as_ref().map(|f| f.phi.as_slice())
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
        for c in 0..self.geom.n_cells() {
            // `fluid_.thermo().nu()` = `mu/rho` of the THERMO (`rho(T)` of a Boussinesq fluid, not `fluid::rho()` = `rho0`)
            let pp = self.case.thermo.props(self.p[c], self.t[c]);
            let nu = pp.mu / pp.rho;
            // normalized = alpha / (1 - structure)
            let norm = self.alpha[c] / (1.0 - self.case.alpha_structure[c]);
            let re = norm * self.u.c[c].mag() * self.case.dh[c] / nu;
            self.re[c] = re.max(self.case.min_re);
            // `FSPair::correct`: `alphaByNu = normalized/nu`, `lRe[k] = max(alphaByNu*U[k]*lDh[k], minRe)`
            let alpha_by_nu = norm / nu;
            let (u, dh) = (self.u.c[c], self.case.dh[c]);
            self.lre[c] = [
                (alpha_by_nu * u.x * dh).max(self.case.min_re),
                (alpha_by_nu * u.y * dh).max(self.case.min_re),
                (alpha_by_nu * u.z * dh).max(self.case.min_re),
            ];
        }
    }

    /// `thermo.correct()` for the fields the solver keeps beside `T` and `he`: `fluid::rho()` (the thermo's `rho(p, T)`; for a
    /// `Boussinesq` fluid `fluid::rho()` stays `rho0`), and the placeholder fields `Cp`, `kappa = Cp alpha`, `Pr = Cp mu/kappa`
    /// (`fluid.C:597`, `EEqn_1p.H`; `alpha = alphah = kappa/Cp` for a polynomial transport: `polynomialTransport::alphah`).
    fn update_thermo_fields(&mut self) {
        let n = self.geom.n_cells();
        let ni = self.geom.n_internal();
        let th = &self.case.thermo;
        for c in 0..n {
            let pp = th.props(self.p[c], self.t[c]);
            let alpha_h = th.alpha_h_const().unwrap_or(pp.kappa / pp.cp);
            self.cp_f[c] = pp.cp;
            self.kappa_f[c] = pp.cp * alpha_h;
            self.pr_f[c] = pp.cp * pp.mu / self.kappa_f[c];
        }
        if th.boussinesq_rho0().is_none() {
            let t_bc = self.t_patch_bcs();
            let mut ts = Scalar { c: self.t.clone(), b: vec![0.0; self.geom.n_faces() - ni], bc: t_bc };
            ts.correct_bc(&self.geom);
            for c in 0..n {
                self.rho[c] = th.props(self.p[c], self.t[c]).rho;
            }
            for f in ni..self.geom.n_faces() {
                self.rho_b[f - ni] = th.props(self.p[self.geom.mesh.owner[f]], ts.b[f - ni]).rho;
            }
        }
    }

    /// The temperature patch conditions: the case's (`fixedValue`, `zeroGradient`, `empty`) with the flow, `zeroGradient` without.
    fn t_patch_bcs(&self) -> Vec<Bc> {
        self.case.flow.as_ref().map_or_else(|| vec![Bc::ZeroGradient; self.geom.mesh.patches.len()], |f| f.t_bc.clone())
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
        self.passive_correct();
        // after the power models: the heat exchangers' `correct(HT, H, THX)` (structure.C:1000-1018): `sHT = htc*T` with the
        // CURRENT temperature, `HT = Hs*T.prevIter()` (README round 44)
        if self.heat_exchangers.is_present() {
            let n = self.geom.n_cells();
            let ht: Vec<f64> = (0..n).map(|c| self.htc[c] * self.t_prev[c]).collect();
            let t_sec = if self.heat_exchangers.secondary_uses_prev_iter_t { &self.t_prev } else { &self.t };
            let s_ht: Vec<f64> = (0..n).map(|c| self.htc[c] * t_sec[c]).collect();
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

    /// `structure::correct` for the passive sub-structure (`structure.C:1020-1055`), the explicit-in-`HT` form of
    /// `ddt(alphaRhoCppas, Tpas) = iApas HT - Sp(iApas H, Tpas)` cell by cell where `iApas != 0`:
    /// `Tpas = (iA HT + (alphaRhoCp/dt) Tpas0)/(alphaRhoCp/dt + iA H)`, `HT = Hs T.prevIter()`, `Tpas0 = Tpas.oldTime()`.
    fn passive_correct(&mut self) {
        let Some(ps) = self.case.passive.as_ref().filter(|_| !self.ctl_no_passive) else { return };
        let dt = self.dt;
        for c in 0..self.geom.n_cells() {
            let ia = ps.ia[c];
            if ia == 0.0 {
                continue;
            }
            let ht = self.htc[c] * self.t_prev[c];
            let a = ps.alpha_rho_cp[c] / dt;
            self.t_pas[c] = (ia * ht + a * self.t_pas_old[c]) / (a + ia * self.htc[c]);
        }
        if self.pas_old_lazy {
            // `Tpas_.correctBoundaryConditions()` -> `storeOldTimes()` of a field whose old time was created in this very step
            self.t_pas_old = self.t_pas.clone();
            self.pas_old_lazy = false;
        }
    }

    /// CONTROL, NOT upstream (gh:#1913): the passive structure ignored (it neither exchanges heat nor is updated).
    #[must_use]
    pub fn without_passive_structure(mut self) -> Self {
        self.ctl_no_passive = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1913): the `htc` models are evaluated once and the field is then kept.
    #[must_use]
    pub fn with_frozen_htc(mut self) -> Self {
        self.ctl_freeze_htc = true;
        self
    }

    /// `T.passiveStructure` per cell (empty without `passiveProperties`).
    #[must_use]
    pub fn passive_structure_temperature(&self) -> &[f64] {
        &self.t_pas
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

    /// `T.HX` (`THXPtr_`): the heat exchangers' surface temperature per cell [K]; empty without heat exchangers (README round 44).
    #[must_use]
    pub fn heat_exchanger_surface_temperature(&self) -> &[f64] {
        &self.heat_exchangers.t_hx
    }

    /// CONTROL, NOT upstream (README round 44, K4): no heat exchangers, whatever `structureProperties.heatExchangers` says.
    #[must_use]
    pub fn without_heat_exchangers(mut self) -> Self {
        self.heat_exchangers = HeatExchangerSet::default();
        self
    }

    /// CONTROL, NOT upstream (README round 46, N6): every primary cell takes its NEAREST secondary cell with weight 1 instead of the
    /// `meshToMesh` overlap weights (as `OnePhaseFcSolver::with_nearest_cell_heat_exchanger_mapping`, round 44 C2).
    #[must_use]
    pub fn with_nearest_cell_heat_exchanger_mapping(mut self) -> Self {
        self.heat_exchangers.nearest_cell_mapping = true;
        self
    }

    /// CONTROL, NOT upstream (README round 44, K5): the secondary side's `htc*T` with `T.prevIter()` instead of the current `T`.
    #[must_use]
    pub fn with_heat_exchanger_secondary_prev_iter_temperature(mut self) -> Self {
        self.heat_exchangers.secondary_uses_prev_iter_t = true;
        self
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

    /// CONTROL, NOT upstream (gh:#1229): the `UEqn_1p.H` Laplacian and `dev2` divergence use the scalar `alpha rho nuEff` (tortuosity ignored).
    #[must_use]
    pub fn without_tortuosity_in_momentum(mut self) -> Self {
        self.ctl_no_tort_momentum = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1229): the `EEqn_1p.H` Laplacian uses the scalar `alphaEff` (tortuosity ignored).
    #[must_use]
    pub fn without_tortuosity_in_energy(mut self) -> Self {
        self.ctl_no_tort_energy = true;
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
        let skip = self.ctl_freeze_htc && self.htc_evaluated;
        self.htc_evaluated = true;
        for c in 0..self.geom.n_cells() {
            if skip {
                break;
            }
            if let Some(m) = self.case.htc_model[c] {
                self.htc[c] = match m.value() {
                    Some(v) => v,
                    None => m.value_with(self.re[c], self.pr_f[c], self.kappa_f[c], self.case.dh[c]),
                };
            }
        }
        if let Some(a) = a {
            for c in 0..self.geom.n_cells() {
                self.htc[c] = self.htc_prev[c] + a * (self.htc[c] - self.htc_prev[c]);
            }
        }
    }

    /// `fvMatrix::relax(alpha)` (ESI v2506 `fvMatrix.C:1101`) for the scalar energy matrix, with the boundary coefficients kept
    /// apart as `fvMatrix` keeps them (`ic` = `internalCoeffs` per boundary face, not folded into `diag`): `D += |ic|` per boundary face,
    /// `D = max(|D|, sumOff)`, `D /= alpha`, `D -= ic`; returns `D' - D0` per cell (the source gets `(D' - D0) psi`).
    fn relax_scalar(&self, e: &mut Eqn, ic: &[f64], factor: f64) -> Vec<f64> {
        let g = &self.geom;
        let (n, ni) = (g.n_cells(), g.n_internal());
        let mut sum_off = vec![0.0; n];
        for f in 0..ni {
            sum_off[e.ldu.owner[f]] += e.ldu.upper[f].abs();
            sum_off[e.ldu.neighbour[f]] += e.ldu.lower[f].abs();
        }
        let d0 = e.ldu.diag.clone();
        let mut d = d0.clone();
        for f in ni..g.n_faces() {
            if g.active[f] {
                d[g.mesh.owner[f]] += ic[f - ni].abs();
            }
        }
        for c in 0..n {
            d[c] = d[c].abs().max(sum_off[c]);
        }
        for v in &mut d {
            *v /= factor;
        }
        for f in ni..g.n_faces() {
            if g.active[f] {
                d[g.mesh.owner[f]] -= ic[f - ni];
            }
        }
        let delta: Vec<f64> = (0..n).map(|c| d[c] - d0[c]).collect();
        e.ldu.diag = d;
        delta
    }

    /// `species::thermo::THs` from `he` seeded with `t_old` (`heRhoThermo::calculate`): the `Boussinesq`/`hConst` form for a
    /// Boussinesq fluid, [`PhaseThermo::t_from_he`] otherwise (upstream aborts past 100 iterations; this returns the seed).
    fn t_from_he_cell(&self, he: f64, p: f64, t_old: f64) -> f64 {
        match &self.case.thermo {
            PhaseThermo::BoussinesqHConst(b) => b.t_from_hs(he, t_old),
            other => other.t_from_he(he, p, t_old).unwrap_or(t_old),
        }
    }

    /// `EEqn_1p.H`; returns the initial residual of the `h` solve and the `T (avg min max)` it prints.
    #[allow(clippy::too_many_lines)]
    fn correct_energy(&mut self) -> (f64, (f64, f64, f64)) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let ni = g.n_internal();
        let nb = g.n_faces() - ni;
        let rdt = 1.0 / self.dt;
        let th = self.case.thermo.clone();
        let phi = self.alpha_rho_phi.clone();
        // the `he` patches follow the `T` patches (`fixedEnergy`: `he = hs(T_b)`; `gradientEnergy` with `zeroGradient` T:
        // gradient 0, so the value and matrix coefficients of a zero-gradient patch)
        let t_bc = self.t_patch_bcs();
        let mut t_s = Scalar { c: self.t.clone(), b: vec![0.0; nb], bc: t_bc.clone() };
        t_s.correct_bc(&g);
        let he_bc: Vec<Bc> = g
            .mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, patch)| match &t_bc[pi] {
                Bc::Fixed(v) => Bc::Fixed((0..patch.size).map(|k| th.he(self.p[g.mesh.owner[patch.start + k]], v[k])).collect()),
                Bc::Empty => Bc::Empty,
                _ => Bc::ZeroGradient,
            })
            .collect();
        let mut he_scalar = Scalar { c: self.he.clone(), b: vec![0.0; nb], bc: he_bc };
        he_scalar.correct_bc(&g);
        // `T.storePrevIter()` (EEqn_1p.H, before the solve): the temperature `structure_.correct(Hs*T.prevIter(), Hs)` sees
        if !self.radial.is_empty() || self.heat_exchangers.is_present() || self.case.passive.is_some() {
            self.t_prev = self.t.clone();
        }
        // fvm::ddt(alpha, rho, he)
        let mut e = Eqn::new(&g);
        let mut ic = vec![0.0; nb];
        let mut bco = vec![0.0; nb];
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.ldu.diag[c] += rdt * self.alpha[c] * self.rho[c] * v;
            e.source[c] += rdt * self.alpha[c] * self.rho_old[c] * self.he_old[c] * v;
        }
        // fvm::div(alphaRhoPhi, he, "div(alphaRhoPhi,he)"): the boundary coefficients stay apart (`internalCoeffs`, `boundaryCoeffs`)
        e.div_upwind_internal(&g, &phi);
        for f in ni..g.n_faces() {
            if g.active[f] {
                let (vi, vb) = he_scalar.convection_coeffs(&g, f, [&[], &[]]);
                ic[f - ni] += phi[f] * vi;
                bco[f - ni] += -phi[f] * vb;
            }
        }
        let ce: Vec<f64> = self.cont_err.iter().map(|v| -v).collect();
        e.su_sp(&g, &ce, &self.he);
        // - laplacian(interpolate(alpha) interpolate(alphaEff), he), alphaEff = CpByCpv (alpha_thermo + alphat)
        let alpha_h_at = |p: f64, t: f64| -> f64 {
            th.alpha_h_const().unwrap_or_else(|| {
                let pp = th.props(p, t);
                pp.kappa / pp.cp
            })
        };
        let ae = Scalar {
            c: (0..n).map(|c| alpha_h_at(self.p[c], self.t[c]) + self.turb.as_ref().map_or(self.case.alphat[c], |t| t.alphat.c[c])).collect(),
            b: (ni..g.n_faces()).map(|f| alpha_h_at(self.p[g.mesh.owner[f]], t_s.b[f - ni]) + 0.0).collect(),
            bc: vec![Bc::Calculated; g.mesh.patches.len()],
        };
        let (lic, lbc) = if let Some(td) = self.case.tortuosity.as_ref().filter(|_| !self.ctl_no_tort_energy) {
            // `EEqn_1p.H:22-27`: `fvm::laplacian(interpolate(alpha) interpolate(tortuosity alphaEff), he)`, the tensor-gamma
            // `gaussLaplacianScheme::fvmLaplacian` (`tensor_laplacian`, gh:#1229)
            let (tc, tb) = tensor_laplacian::tortuosity_fields(&g, td).expect("validated when the case was read");
            let ta_c: Vec<_> = (0..n).map(|c| tensor_laplacian::scale(ae.c[c], &tc[c])).collect();
            let ta_b: Vec<_> = (0..nb).map(|i| tensor_laplacian::scale(ae.b[i], &tb[i])).collect();
            let taf = tensor_laplacian::interpolate(&g, &ta_c, &ta_b);
            let gam_t: Vec<_> = (0..g.n_faces()).map(|f| tensor_laplacian::scale(self.alpha_f[f], &taf[f])).collect();
            let tg = tensor_laplacian::decompose(&g, &gam_t);
            tensor_laplacian::neg_laplacian_internal(&mut e, &g, &tg.gamma_mag_sf);
            tensor_laplacian::add_neg_laplacian_scalar_correction(&mut e, &g, &tg, &he_scalar);
            tensor_laplacian::neg_laplacian_boundary_coeffs(&g, &tg.gamma_mag_sf, &he_scalar)
        } else {
            let ae_f = fv::interp_linear_openfoam(&g, &ae);
            let gam: Vec<f64> = (0..g.n_faces()).map(|f| self.alpha_f[f] * ae_f[f]).collect();
            e.neg_laplacian_internal(&g, &gam);
            Eqn::neg_laplacian_boundary_coeffs(&g, &gam, &he_scalar)
        };
        for i in 0..nb {
            ic[i] += lic[i];
            bco[i] += lbc[i];
        }
        // right-hand side: alpha rho (U.g); alpha ddt(p) = 0 exactly (p is never modified)
        let mut rhs = vec![0.0; n];
        for c in 0..n {
            rhs[c] = self.alpha[c] * self.rho[c] * self.u.c[c].dot(self.case.gravity);
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
                b: vec![0.0; nb],
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
            let kf = fv::interp_upwind(&g, &k, &phi);
            let flux: Vec<f64> = (0..g.n_faces()).map(|f| phi[f] * kf[f]).collect();
            let div_k = fv::div(&g, &flux);
            let mut s = vec![0.0; n];
            for c in 0..n {
                // K.oldTime() of a fresh local field is K itself
                let ddt_k = rdt * (self.alpha[c] * self.rho[c] * k.c[c] - self.alpha[c] * self.rho_old[c] * k.c[c]);
                s[c] = -(ddt_k + div_k[c] - self.cont_err[c] * k.c[c]);
            }
            e.su(&g, &s);
        }
        // structure_.linearizedSemiImplicitHeatSource(fluid, Hs): active, passive and heat-exchanger terms, `Cp = fluid.Cp()`
        let mut su = vec![0.0; n];
        let mut sp = vec![0.0; n];
        for c in 0..n {
            let ih = self.ia_act[c] * self.htc[c];
            su[c] = ih * (self.t_act[c] - self.t[c] + self.he[c] / self.cp_f[c]);
            sp[c] = ih / self.cp_f[c];
        }
        let mut su_pas = vec![0.0; n];
        let mut sp_pas = vec![0.0; n];
        if let Some(ps) = self.case.passive.as_ref().filter(|_| !self.ctl_no_passive) {
            for c in 0..n {
                let ih = ps.ia[c] * self.htc[c];
                su_pas[c] = ih * (self.t_pas[c] - self.t[c] + self.he[c] / self.cp_f[c]);
                sp_pas[c] = ih / self.cp_f[c];
            }
        }
        // the matrix `Q = (S_act - Sp_act) + S_pas - Sp_pas (+ HX)` subtracted from the equation: the source and the diagonal receive
        // `V*act + V*pas (+ V*hx)` as ONE left-associated sum (negations are exact); with no passive structure `V*pas` is exactly 0
        let hx = self.heat_exchangers.is_active();
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            let (mut s_tot, mut d_tot) = (v * su[c] + v * su_pas[c], v * sp[c] + v * sp_pas[c]);
            if hx {
                // `structure.C:1148-1156`: `Q += iAHX*H*(THX-T+he/Cp) - fvm::Sp(iAHX*H/Cp, he)` BEFORE `EEqn -= Q` (README round 44)
                let (su_hx, sp_hx) = self.heat_exchangers.source_and_coeff(c, self.htc[c], self.t[c], self.he[c], self.cp_f[c]);
                s_tot += v * su_hx;
                d_tot += v * sp_hx;
            }
            e.source[c] += s_tot;
            e.ldu.diag[c] += d_tot;
        }
        // - alpha * powerDensity
        let pd: Vec<f64> = (0..n).map(|c| self.alpha[c] * self.power_density[c]).collect();
        e.su(&g, &pd);
        // EEqn.relax()
        if let Some(fct) = self.case.controls.relax.equation("h", self.pimple.final_iteration).filter(|f| *f > 0.0) {
            let delta = self.relax_scalar(&mut e, &ic, fct);
            for c in 0..n {
                e.source[c] += delta[c] * self.he[c];
            }
        }
        // `solveSegregated`: the boundary coefficients join the diagonal and the source at the solve
        for f in ni..g.n_faces() {
            if g.active[f] {
                let o = g.mesh.owner[f];
                e.ldu.diag[o] += ic[f - ni];
                e.source[o] += bco[f - ni];
            }
        }
        let mut x = self.he.clone();
        // `EEqn.solve()` selects `hFinal` when `isFinalIteration()`
        let fin = self.pimple.final_iteration;
        let ctl = &self.case.controls;
        let (tol, rel_tol, min_iter) =
            if fin { (ctl.h_tolerance_final, ctl.h_rel_tol_final, ctl.h_min_iter_final) } else { (ctl.h_tolerance, ctl.h_rel_tol, ctl.h_min_iter) };
        let kind = if fin { ctl.h_solver_final } else { ctl.h_solver };
        let (initial_residual, iterations) = match kind {
            HSolver::PBiCGStabDiagonal => {
                let perf = pbicgstab_diagonal(&e.ldu, &e.source, &mut x, tol, rel_tol, DEFAULT_MAX_ITER, min_iter);
                (perf.initial_residual, perf.iterations)
            }
            HSolver::PbicgDilu => {
                let perf = crate::genfoam::thermo_mechanics::legacy_fvm::pbicg::pbicg_dilu(&e.ldu, &e.source, &mut x, tol, rel_tol, DEFAULT_MAX_ITER, min_iter);
                (perf.initial_residual, perf.iterations)
            }
            HSolver::SmoothSymGaussSeidel => {
                let (sweeps, ini, _fin) = super::super::one_phase::smooth_solve_sym_gauss_seidel_openfoam(
                    &e.ldu,
                    &e.source,
                    &mut x,
                    outram_foam_basic_lib::prelude::SolverSettings { tolerance: tol, max_iter: DEFAULT_MAX_ITER },
                    min_iter,
                );
                (ini, sweeps)
            }
        };
        let perf = (initial_residual, iterations);
        self.pimple.record("h", perf.0);
        self.h_solves.push((perf.0, perf.1));
        self.he = x;
        // `he.relax()`: `he = prevIter + alpha (he - prevIter)`, the `fields` entry `h` (`hFinal` in the flagged iteration)
        if let Some(a) = self.case.controls.relax.field("h", fin) {
            for c in 0..n {
                self.he[c] = self.he_prev[c] + a * (self.he[c] - self.he_prev[c]);
            }
        }
        // thermo.correct(): T from he by Newton, seeded with the current T; then rho, Cp, kappa, Pr
        for c in 0..n {
            self.t[c] = self.t_from_he_cell(self.he[c], self.p[c], self.t[c]);
        }
        self.update_thermo_fields();
        let vols = &g.mesh.cell_volumes;
        let (mut sv, mut st) = (0.0, 0.0);
        let (mut tmin, mut tmax) = (f64::MAX, f64::MIN);
        for c in 0..n {
            sv += vols[c];
            st += vols[c] * self.t[c];
            tmin = tmin.min(self.t[c]);
            tmax = tmax.max(self.t[c]);
        }
        // `min(T)`/`max(T)` of a `volScalarField` run over the cells AND the boundary faces (a `fixedValue` inlet is in them)
        let mut t_now = Scalar { c: self.t.clone(), b: vec![0.0; nb], bc: self.t_patch_bcs() };
        t_now.correct_bc(&g);
        for f in ni..g.n_faces() {
            if g.active[f] {
                tmin = tmin.min(t_now.b[f - ni]);
                tmax = tmax.max(t_now.b[f - ni]);
            }
        }
        // `structure_.correct(Hs*T.prevIter(), Hs)` runs at the END of EEqn_1p.H: `fixedTemperature::correctT` -> `temperatureUpdate`
        self.temperature_update();
        self.structure_correct();
        (perf.0, (st / sv, tmin, tmax))
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
