// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/solvers/twoPhase/**
//     (twoPhase.{C,H}; include/{alphaEqns_2p,UEqns_2p,pEqnf_2p,EEqns_2p}.H;
//      include/alphaEqnsRoutines/solveAlphaImplicitUpwind.H;
//      include/pEqnRoutines/*.H)
//   and the classes those files call: physicsModels/phasePairs/{FFPair,FSPair}.C,
//   physicsModels/phaseChangeModels/{phaseChangeModel,heatDriven/*}.C,
//   physicsModels/phaseChangeModels/latentHeatModels/latentHeatModel.C,
//   physicsModels/dragModels/{FFDragFactor,FSDragFactor,twoPhaseDragFactor}.C,
//   physicsModels/dragModels/twoPhaseDragMultiplierModels/twoPhaseDragMultiplierModel.C,
//   physicsModels/virtualMassModels/virtualMass.C,
//   physicsModels/interfacialAreaModels/spherical/*.C,
//   physicsModels/fluidDiameterModels/fluidDiameterModel.C,
//   physicsModels/heatTransferModels/FSHeatTransferCoefficientModels/
//     {Nusselt,Shah,subModels/superpositionNucleateBoiling,
//      subModels/flowEnhancementFactorModels/COBRA-TF,
//      subModels/suppressionFactorModels/COBRA-TF}/*.C,
//   physicsModels/heatTransferModels/FFHeatTransferCoefficientModels/Nusselt/*.C,
//   physicsModels/regimeMapModels/{regimeMapModelTemplates,oneParameter/oneParameter}.C,
//   physicsModels/templatedModels/byRegime/byRegimeModel.C,
//   phaseModels/fluid/fluid.C, phaseModels/structureModels/structure.C,
//   phaseModels/structureModels/powerModels/fixedPower/fixedPower.C,
//   solvers/thermalHydraulicsModel.C, src/customPimpleControl/customPimpleControl{.C,I.H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `solver::two_phase` — GeN-Foam's porous two-fluid solver (boiling)
//!
//! Port of GeN-Foam's `twoPhase` thermal-hydraulics region solver: two
//! interpenetrating fluids (liquid and vapour) in a porous medium whose solid
//! fraction is an unresolved `structure`, with
//!
//! - **phase fractions** by GeN-Foam's `implicitUpwind` alpha solver
//!   ([`TwoPhaseSolver::correct_alpha`], `solveAlphaImplicitUpwind.H`);
//! - **momentum** in the face-centred form with **implicit partial
//!   elimination** of the interfacial drag ([`TwoPhaseSolver::correct_u_eqns`],
//!   `UEqns_2p.H`; [`TwoPhaseSolver::correct_pressure`], `pEqnf_2p.H`);
//! - a **compressible pressure equation** in `p_rgh` with the two phases'
//!   compressibility, dilation (`dgdt`) and phase-change mass sources
//!   (`calcComp_2p.H`, `solveP_rgh_2p.H`);
//! - two **enthalpy equations** with interfacial heat transfer, the
//!   energy-conservative phase-change sources of `phaseChangeModel::correct`,
//!   the semi-implicit structure heat source and the low-fraction
//!   stabilisation ([`TwoPhaseSolver::correct_energy`], `EEqns_2p.H`);
//! - **void** (the `alpha`/`normalized` fields), the **flow-regime map**
//!   (`oneParameter` + `byRegime` dispatch), and the **CHF/boiling hooks** of
//!   the fluid-structure heat-transfer closures it calls.
//!
//! The closures are the crate's existing ports (`closures::{fs_drag, ff_drag,
//! heat_transfer, phase_change, interfacial}`); this module wires them into the
//! equations as upstream's `FFPair`/`FSPair`/`phaseChangeModel` do. The
//! thermophysical packages are basic-lib's OpenFOAM `heRhoThermo` ports
//! ([`thermo`]), which is what every two-phase tutorial selects.
//!
//! ## What this slice does NOT port (each is a recorded gap)
//!
//! - ~~**MULES** (`solveAlphaMULES.H`, `myMULES`, 1,765 lines). A case that
//!   selects `MULES` is run with `implicitUpwind` and the caller is told
//!   ([`TwoPhaseCase::deviations`]).~~ **CORRECTED 2026-11-26** (gh:#1361 round 7): MULES is ported
//!   ([`mules`], `alpha_mules.rs`; `solvePhase both`, `vanLeer`/`upwind`/`linear` convection, sub-cycling)
//!   and is what a case that selects it runs; `1D_boiling` as shipped agrees with the real binary to the
//!   single-phase round-off (1e-11) up to the boiling onset (README "Round 7"). ~~Still not ported: MULES with
//!   `solvePhase` set (a hard error in [`TwoPhaseCase::read`])~~ **CORRECTED 2026-11-27** (gh:#1799, round 34): MULES with
//!   `solvePhase <fluid>` (the `solveAlpha` lambda) is ported; a `cyclic` patch is refused by name. Still not ported: `limitSum`/`nAlphaSubCycles`-style
//!   `alphaSubCycle` variants GeN-Foam does not call. The earlier substitution is
//!   [`TwoPhaseCase::with_alpha_solver_implicit_upwind`].
//! - `momentumMode cellCentered` (`pEqn_2p.H`) and partial-elimination modes
//!   other than `implicit`; `porousInterfaceSharpness != 0`.
//! - Turbulence other than `laminar` and `RAS` `porousKEpsilon2PhaseCorrected` (gh:#1802, [`turbulence`]; every other
//!   `RASModel`, `DhStruct`, wall-function patches refused by name); structure tortuosity/anisotropy, local
//!   frames, pumps, heat exchangers, passive structures; `fvOptions`.
//! - Closure selections other than those listed on [`TwoPhaseModels`]; ~~the
//!   wall-boiling mass split `dmdtW` (no ported model writes it); the
//!   `twoParameters` regime map; `byRegime` for anything but dispersion and
//!   contact partition.~~ **CORRECTED 2026-11-26** (gh:#1361 round 8): `multiRegimeBoilingTRACE` writes `dmdtW`
//!   ([`TwoPhaseSolver::dmdt_w`]) and `phaseChangeModel::correct` consumes it; the `twoParameters` map is ported
//!   ([`RegimeMapSelection`]); `byRegime` also covers the dispersed diameter and the interfacial-area density
//!   ([`ByRegimeModel`]). ~~Still refused by name: `multiRegimeBoilingTRACECHF`/`VapourTRACE` (their closures exist,
//!   `closures::heat_transfer::boiling_trace_chf`, but their annular-flow sub-models and `lookUpTableCHF` do not),
//!   through-flow boundary conditions (`fixedValue` velocity inlets, `pressureInletOutletVelocity`, `slip`,
//!   `inletOutlet`, `fixedFluxPressure`) and `porousKEpsilon2PhaseCorrected` turbulence: so NONE of the shipped
//!   `1D_PSBT_SC` cases runs end to end yet (README "Round 8").~~ **CORRECTED 2026-11-26** (gh:#1802, README "Round 16"): all four `1D_PSBT_SC` cases read and run; turbulence is [`turbulence`], plain `multiRegimeBoiling` is [`FsHtcModel::MultiRegimeBoiling`].
//!
//! ## Wiring of the consumed models
//!
//! | Consumed | Where it enters |
//! |---|---|
//! | `closures::ff_drag::FfDragCoefficient` | `FFDragFactor::correctField` → `Kd12` → partial elimination in `pEqnf` |
//! | `closures::fs_drag::FsWallFriction` | `FSDragFactor` → `Kd` of each fluid-structure pair → `UEqn` `Sp` |
//! | `closures::ff_drag::TwoPhaseDragMultiplier` | `twoPhaseDragFactor::correct` |
//! | `closures::heat_transfer::{FsForcedConvectionHtc, PoolBoilingHtc, FlowEnhancementFactor, SuppressionFactor}` | `FSPair::correct` htc → `EEqn` structure source and `structure.correct` |
//! | `closures::phase_change::{SaturationModel, LatentHeatModel, PhaseChangeRateModel}` | `phaseChangeModel::correct` → `dmdt`, `T.interface`, enthalpy sources |
//! | `closures::interfacial::regime_map::RegimeMap1D` | `oneParameter::correct` → `byRegime` weights |
//! | `structure::power_model::FixedPower`, `structure::power_off::PowerOffCriterion` | `fixedPower::correct`, `structure::checkPowerOff` |
//! | `structure::radial_power_model::RadialPowerModel` (`heatedPin`, `nuclearFuelPin`, `lumpedNuclearStructure`, `interpolatedNuclearFuelPin`), reader `radial_power` | `powerModel` table, `structure::correct(H1s T1 + H2s T2, H1s + H2s)` (`EEqns_2p.H:331`), `checkPowerOff` (gh:#1884) |

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
mod alpha_mules;
pub(crate) mod foam_regex;
pub mod case;
pub mod gamg_solve;
pub mod fv;
pub mod mules;
pub mod non_orth;
pub mod one_phase_cc;
pub mod one_phase_fc;
pub(crate) mod pbicgstab;
pub(crate) mod pcg;
mod radial_power;
pub mod tensor_laplacian;
pub mod thermo;
pub mod turbulence;

#[cfg(test)]
mod tests;

use outram_foam_basic_lib::krylov::{bicgstab, KrylovSettings, Preconditioner};
use outram_foam_basic_lib::prelude::*;
use petir::mathf::RealMath;
use uom::si::f64::{Length, Pressure, Ratio, ThermalConductivity, ThermodynamicTemperature, Time};
use uom::si::heat_flux_density::watt_per_square_meter;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::genfoam::thermal_hydraulics::closures::ff_drag::{
    FfDragCoefficient, FfInterfacialState, TwoPhaseDragMultiplier,
};
use crate::genfoam::thermal_hydraulics::closures::fs_drag::FsWallFriction;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace::{
    multi_regime_boiling_trace, TraceBoilingInputs,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace_chf::{
    multi_regime_boiling_trace_chf, multi_regime_boiling_vapour_trace, TraceChfInputs, TraceChfResult,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::cachard::{
    CachardLiquid, CachardLiquidInputs, CachardVapour,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::LeidenfrostTemperature;
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf_table::{LookUpTableChf, LookUpTableChfInputs};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::{
    FlowEnhancementFactor, FlowEnhancementInputs, OnsetOfNucleateBoilingTemperature, SubcooledBoilingFraction,
    SuppressionFactor, SuppressionInputs,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::{
    FsForcedConvectionHtc, PoolBoilingHtc,
};
use crate::genfoam::thermal_hydraulics::closures::interfacial::by_regime::ByRegimeModel;
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map::RegimeMap1D;
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map_2d::RegimeMap2D;
use crate::genfoam::thermal_hydraulics::closures::phase_change::{
    LatentHeatModel, PhaseChangeRateModel, SaturationModel,
};
use crate::genfoam::thermal_hydraulics::structure::heat_exchanger::HeatExchangerSet;
use crate::genfoam::thermal_hydraulics::structure::power_model::FixedPower;
use crate::genfoam::thermal_hydraulics::structure::power_off::PowerOffCriterion;
use crate::genfoam::thermal_hydraulics::structure::radial_power_model::{RadialPowerModel, StepInputs};
use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;

pub use case::TwoPhaseCase;
use fv::{Bc, Eqn, Geometry, Scalar, Vector};
use one_phase_cc::flow::UEqnCc;
pub use thermo::PhaseThermo;
pub use turbulence::PorousKEpsilon2PhaseCorrected;

/// A per-cell model value that is either constant or blended by flow regime
/// (`templatedModels/{constant,byRegime}`), here for scalar-valued models.
#[derive(Debug, Clone, PartialEq)]
pub enum ByRegime {
    /// `type constant; value v;`.
    Constant(f64),
    /// `type byRegime; regimeMap ...;` with one constant sub-model per regime
    /// name; the value is `sum_r w_r v_r` over the cell's regime weights
    /// (`regimeMapModel::interpolateValue`).
    PerRegime(Vec<(String, f64)>),
}

impl ByRegime {
    /// Evaluate with the cell's regime weights (empty if no map).
    fn value(&self, weights: &[(String, f64)]) -> f64 {
        match self {
            Self::Constant(v) => *v,
            Self::PerRegime(table) => weights
                .iter()
                .map(|(name, w)| {
                    w * table
                        .iter()
                        .find(|(n, _)| n == name)
                        .map(|(_, v)| *v)
                        .unwrap_or(0.0)
                })
                .sum(),
        }
    }
}

/// Fluid-structure heat-transfer model of one cell zone
/// (`FSHeatTransferCoefficientModels`), the selections the ported tutorials use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsHtcModel {
    /// `NusseltReynoldsPrandtlPower` (`Nusselt`).
    Nusselt(FsForcedConvectionHtc),
    /// `superpositionNucleateBoiling`: `htcFC F + htcPB S`.
    SuperpositionNucleateBoiling {
        /// `forcedConvection`.
        forced_convection: FsForcedConvectionHtc,
        /// `poolBoiling` (`useExplicitHeatFlux false`).
        pool_boiling: PoolBoilingHtc,
        /// `flowEnhancementFactor`.
        flow_enhancement: FlowEnhancementFactor,
        /// `suppressionFactor`.
        suppression: SuppressionFactor,
    },
    /// plain `multiRegimeBoiling` (gh:#1802): the TRACE map with the heat fluxes superposed with exponent `n`,
    /// `qNB = (qFC^n + (S qPB)^n)^(1/n)` when a suppression factor exists (else the TRACE `qBI` form).
    MultiRegimeBoiling(MultiRegimeBoilingModel),
    /// `multiRegimeBoilingTRACE` (gh:#1222): see
    /// [`crate::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace`].
    MultiRegimeBoilingTrace(TraceBoilingModel),
    /// `multiRegimeBoilingTRACECHF` (gh:#1223, gh:#1224): TRACE with the critical-heat-flux and
    /// post-CHF branches, see
    /// [`crate::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace_chf`].
    MultiRegimeBoilingTraceChf(TraceChfModel),
    /// `multiRegimeBoilingVapourTRACE` (gh:#1224): the vapour side, reading the fields the
    /// liquid-side `TRACECHF` model wrote in the same correction.
    MultiRegimeBoilingVapourTrace(CachardVapour),
}

/// The critical-heat-flux model of `multiRegimeBoilingTRACECHF`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChfSelection {
    /// `constantCHF`: `q_CHF` [W/m^2].
    Constant(f64),
    /// `lookUpTableCHF`: index into [`TwoPhaseModels::chf_tables`].
    Table(usize),
}

/// The sub-models of `multiRegimeBoilingTRACECHF`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceChfModel {
    /// The sub-models shared with `multiRegimeBoilingTRACE`.
    pub base: TraceBoilingModel,
    /// `criticalHeatFluxModel` (absent: `q_CHF = 1e15`).
    pub chf: Option<ChfSelection>,
    /// `leidenfrostModel` (absent: `T_LF = T_wall`).
    pub leidenfrost: Option<LeidenfrostTemperature>,
    /// `annularFlowModel` (absent: `h_FB = 0`).
    pub annular: Option<CachardLiquid>,
    /// `absoluteSurfaceRoughness` (default 4e-7 m).
    pub roughness: f64,
}

/// The sub-models of plain `multiRegimeBoiling` (`heatFluxSuperposition true`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultiRegimeBoilingModel {
    /// The sub-models shared with the TRACE variants.
    pub base: TraceBoilingModel,
    /// `suppressionFactorModel` (absent: the TRACE `qBI` form, or plain superposition without an onset model).
    pub suppression: Option<SuppressionFactor>,
    /// `superpositionExponent`.
    pub exponent: f64,
}

/// The sub-models of `multiRegimeBoilingTRACE` (`FSHeatTransferCoefficientModels`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TraceBoilingModel {
    /// `forcedConvectionModel`.
    pub forced_convection: FsForcedConvectionHtc,
    /// `poolBoilingModel` (read with the `Twall`/`T.interface` form, `useExplicitHeatFlux false`).
    pub pool_boiling: PoolBoilingHtc,
    /// `flowEnhancementFactorModel`.
    pub flow_enhancement: FlowEnhancementFactor,
    /// `nucleateBoilingOnsetModel` (absent: `T_ONB = T_sat`).
    pub onset: Option<OnsetOfNucleateBoilingTemperature>,
    /// `subCooledBoilingFractionModel` (absent: `f = 1`).
    pub sub_cooled_fraction: Option<SubcooledBoilingFraction>,
}

/// The `PBiCGStab`/`diagonal` entry of the `solvers` dictionary that the enthalpy equations resolve to
/// ([`TwoPhaseControls::h_pbicgstab`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HPbicgstab {
    /// `tolerance`.
    pub tolerance: f64,
    /// `relTol` (default 0).
    pub rel_tol: f64,
    /// `minIter` (default 0).
    pub min_iter: usize,
}

/// The `PCG`/`DIC` entry of the `solvers` dictionary that `p_rgh` resolves to ([`TwoPhaseControls::p_rgh_pcg`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PRghPcg {
    /// `tolerance`.
    pub tolerance: f64,
    /// `relTol` (default 0).
    pub rel_tol: f64,
    /// `minIter` (default 0).
    pub min_iter: usize,
    /// `maxIter` (default 1000).
    pub max_iter: usize,
}

/// One recorded evaluation of `multiRegimeBoilingTRACE` for one cell (gh:#1361 round 13 diagnostic, see
/// [`TwoPhaseSolver::trace_probe`]): the named intermediates, in the order the model computes them.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceProbeRecord {
    /// `"trace"` (one `multiRegimeBoilingTRACE` cell evaluation) or `"structure"` (one `structure::correct` cell).
    pub kind: &'static str,
    /// Time of the step being taken [s].
    pub time: f64,
    /// PIMPLE outer iteration (1-based) of that step.
    pub outer_iteration: usize,
    /// Cell index.
    pub cell: usize,
    /// `(name, value)` pairs (`alphai`, `Tw`, `Tf`, `Tsat`, `htcFC`, `F`, `htc2pFC`, `qFC`, `Re`, `Pr`, `kappa`, `Dh`,
    /// `p`, `L`, `iAact`; in the boiling branch also `TONB`, `rhoOther`, `absL`, `hPBONB`, `hPB`, `f`, `qSCDmdt`,
    /// `dmdtW`; and `ret`, the heat-transfer coefficient returned).
    pub values: Vec<(&'static str, f64)>,
}

/// The field a `twoParameters` regime map reads on one axis (`parameter1`, `parameter2`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegimeParameter {
    /// `normalized.alpha.<fluid>`: the index of the fluid.
    NormalizedAlpha(usize),
    /// `alphaRhoMagU.<fluid>`: the index of the fluid.
    AlphaRhoMagU(usize),
}

/// The `regimeMapModels` entry of a case.
#[derive(Debug, Clone)]
pub enum RegimeMapSelection {
    /// `oneParameter`, with the index of the fluid whose `normalized` fraction is its parameter.
    OneParameter(RegimeMap1D, usize),
    /// `twoParameters` (gh:#1225).
    TwoParameters {
        /// The map.
        map: RegimeMap2D,
        /// `parameter1` (the x axis).
        parameter1: RegimeParameter,
        /// `parameter2` (the y axis).
        parameter2: RegimeParameter,
    },
}

/// Fluid-fluid (interfacial) heat-transfer model of one fluid
/// (`FFHeatTransferCoefficientModels`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfHtcModel {
    /// `NusseltReynoldsPrandtlPower`: `kappa/max(Dh_d, 1e-4) (A + B Re^C Pr^D)`
    /// with `Re` the pair Reynolds number and `Pr` the continuous-phase Prandtl
    /// number (`NusseltFFHeatTransferCoefficient.C`).
    Nusselt {
        /// `const`.
        a: f64,
        /// `coeff`.
        b: f64,
        /// `expRe`.
        c: f64,
        /// `expPr`.
        d: f64,
    },
    /// `constant`: a fixed htc [W/(m^2 K)].
    Constant(f64),
}

/// The phase-change model (`phaseChangeModel` with `heatDriven`).
#[derive(Debug, Clone)]
pub struct PhaseChangeConfig {
    /// The `heatDriven` mode (`PhaseChangeRateModel` heat-driven variants).
    pub rate: PhaseChangeRateModel,
    /// `latentHeatModel`.
    pub latent_heat: LatentHeatModel,
    /// `latentHeatModel.adjust` (`latentHeatModel::adjust`).
    pub adjust_latent_heat: bool,
    /// `saturationModel`.
    pub saturation: SaturationModel,
    /// `residualInterfacialArea` [1/m] (default 1e-3).
    pub residual_interfacial_area: f64,
}

/// The closure selections of a case (closed set; see the module doc for what
/// is not covered).
#[derive(Debug, Clone)]
pub struct TwoPhaseModels {
    /// Fluid-fluid drag coefficient (`dragModels."f1.f2"`).
    pub ff_drag: FfDragCoefficient,
    /// Fluid-structure friction per fluid per cell (`dragModels."fk.structure"`).
    pub fs_drag: [Vec<Option<FsWallFriction>>; 2],
    /// `twoPhaseDragMultiplierModel`: the model, the multiplier fluid index and
    /// `maxPhi2` (default 1000).
    pub two_phase_multiplier: Option<(TwoPhaseDragMultiplier, usize, f64)>,
    /// Fluid-structure htc per fluid per cell.
    pub fs_htc: [Vec<Option<FsHtcModel>>; 2],
    /// The `lookUpTableCHF` tables the htc models refer to ([`ChfSelection::Table`]).
    pub chf_tables: Vec<LookUpTableChf>,
    /// Interfacial htc per fluid.
    pub ff_htc: [Option<FfHtcModel>; 2],
    /// `virtualMassCoefficientModel constant` value, if present.
    pub virtual_mass: Option<f64>,
    /// `regimeMapModels` (one `oneParameter` map) and the index of the fluid
    /// whose `normalized` phase fraction is its parameter.
    pub regime_map: Option<RegimeMapSelection>,
    /// Dispersion of fluid 1 (`dispersionModel`: `1` = fluid 1 dispersed).
    pub dispersion_fluid1: ByRegime,
    /// Contact partition: the fluid index the model is given for and the
    /// model; the other fluid takes `1 - f` (`complementary`).
    pub contact_partition: (usize, ByRegime),
    /// `interfacialAreaDensityModel spherical` `cutoffAlpha` (default 0.9).
    pub spherical_cutoff_alpha: f64,
    /// `interfacialAreaDensityModel byRegime` with a `spherical` model per regime: its `cutoffAlpha`
    /// (gh:#1225). When `Some`, it replaces [`TwoPhaseModels::spherical_cutoff_alpha`].
    pub interfacial_area_by_regime: Option<ByRegimeModel<f64>>,
    /// `phaseChangeModel`, if present.
    pub phase_change: Option<PhaseChangeConfig>,
}

/// `enthalpyStabilizationMode` (`twoPhase::heStabilizationMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeStabilization {
    /// `cutoff` (upstream default): below `thermoResidualAlpha`, `he` is set to
    /// its interface value after the solve.
    Cutoff,
    /// `source`: a stiff relaxation source towards the interface enthalpy.
    Source,
}

/// The `alpha` solver the case selects (`fvSolution`, `alpha { solver ...; }`;
/// upstream's `twoPhase::alphaEqnsSolver`).
#[derive(Debug, Clone, PartialEq)]
pub enum AlphaSolver {
    /// `implicitUpwind` (`solveAlphaImplicitUpwind.H`, one implicit solve of `solvePhase`).
    ImplicitUpwind,
    /// `MULES` (`solveAlphaMULES.H` + `myMULES`, gh:#1361 round 7): the shipped
    /// tutorials' choice and the DEFAULT whenever a case selects it.
    Mules(MulesControls),
}

/// `fvSolution` `alpha` dictionary entries read by `solveAlphaMULES.H` / `myMULES`.
#[derive(Debug, Clone, PartialEq)]
pub struct MulesControls {
    /// `minAlphaSp` (default `1e-6`; implicitUpwind's default is `1e-4`).
    pub min_alpha_sp: f64,
    /// `adjustSubCycles` (default false).
    pub adjust_sub_cycles: bool,
    /// `alphaMaxCo` when `adjustSubCycles`, else upstream's `0.49`.
    pub alpha_max_co: f64,
    /// `maxSubCycles` (default 50).
    pub max_sub_cycles: usize,
    /// `nSubCycles` (default 1), used when sub-cycles are not adjusted or only one phase is present.
    pub n_sub_cycles: usize,
    /// `nLimiterIter`, `smoothLimiter`, `extremaCoeff`.
    pub limiter: mules::LimiterControls,
    /// `divSchemes` `div(phi,alpha)`.
    pub phi_scheme: mules::AlphaConvection,
    /// `divSchemes` `div(phir,alpha)`.
    pub phir_scheme: mules::AlphaConvection,
    /// `relaxationFactors.fields` of `alpha.<fluid k>`; `None` when the field is not relaxed.
    pub relax: [Option<f64>; 2],
    /// `solvePhase`: `None` for `both` (the default, `solveAlphas`), `Some(k)` for the fluid whose fraction the
    /// `solveAlpha` lambda solves with MULES (the other follows by continuity; gh:#1799).
    pub solve_phase: Option<usize>,
}

/// PIMPLE and solver controls (`fvSolution`).
#[derive(Debug, Clone, PartialEq)]
pub struct TwoPhaseControls {
    /// The alpha solver the case selects. See [`TwoPhaseCase::with_alpha_solver_implicit_upwind`]
    /// for the port's earlier behaviour of running `implicitUpwind` in place of `MULES`.
    pub alpha: AlphaSolver,
    /// `nOuterCorrectors`.
    pub n_outer: usize,
    /// `nCorrectors`.
    pub n_corr: usize,
    /// `correctUntilConvergence` (`customPimpleControl`).
    pub correct_until_convergence: bool,
    /// `solvers.p_rgh.tolerance`.
    pub p_rgh_tolerance: f64,
    /// `solvers."h.*".tolerance`.
    pub h_tolerance: f64,
    /// `solvers."h.*".minIter`.
    pub h_min_iter: usize,
    /// The linear solver the enthalpy equations really get (gh:#1361 round 13): OpenFOAM resolves `h.<fluid>` in the `solvers`
    /// dictionary to the LAST matching pattern, so a catch-all `".*"` declared after `"h.*"` wins. `Some`: that entry is
    /// `PBiCGStab` with the `diagonal` preconditioner (what the case files of every ported tutorial select, and what
    /// upstream's log prints: `diagonalPBiCGStab: Solving for h.liquid`), run by [`pbicgstab::pbicgstab_diagonal`]. `None`:
    /// the port's earlier `symGaussSeidel` sweeps with [`Self::h_tolerance`] ([`TwoPhaseSolver::with_smooth_solver_for_enthalpy`]).
    pub h_pbicgstab: Option<HPbicgstab>,
    /// `Some` for fluid `k` when `h.<fluid k>` resolves (exact keyword, else the last matching pattern) to `smoothSolver` with
    /// `symGaussSeidel`: upstream's `smoothSolver::solve` with every control of the entry (`relTol`, `nSweeps`, `maxIter`, `minIter`;
    /// [`crate::genfoam::common::smooth_solver`], gh:#1957 round 83). No shipped case selects it (their catch-all `".*"` wins);
    /// before round 83 such an entry ran the port's earlier sweeps with [`Self::h_tolerance`], `relTol`/`nSweeps`/`maxIter` ignored.
    pub h_smooth: [Option<crate::genfoam::common::smooth_solver::SmoothSolverControls>; 2],
    /// [`Self::h_pbicgstab`] of `h.<fluid 1>Final`: what the enthalpy solve gets in the outer iteration `customPimpleControl::loop`
    /// flags final (`mesh.data().setFinalIteration(true)` in the iteration whose criteria are met, `customPimpleControl.C:250-257`;
    /// `fvMatrix::solve` selects `psi.select(finalIteration)`). gh:#1957 round 83 (README addendum A): before it the port used the
    /// non-Final entry in every iteration; every shipped case resolves both to the catch-all `".*"`, so no shipped number moves.
    pub h_pbicgstab_final: Option<HPbicgstab>,
    /// [`Self::h_smooth`] of `h.<fluid>Final` (see [`Self::h_pbicgstab_final`]).
    pub h_smooth_final: [Option<crate::genfoam::common::smooth_solver::SmoothSolverControls>; 2],
    /// The linear solver `p_rgh` really gets (gh:#1361 round 17, gh:#1833). `Some`: the case selects `PCG` with the `DIC`
    /// preconditioner, run by [`pcg::pcg_dic`] with OpenFOAM's normalised-residual stopping rule. `None`: the port's
    /// ILU(0)-BiCGStab stopped at `||r||/||b|| < 1e-13` (every other `p_rgh` solver, `GAMG` included, which has no port;
    /// [`TwoPhaseSolver::with_krylov_for_p_rgh`]). On an ill-conditioned `p_rgh` equation that stopping rule leaves
    /// the solution wrong in the 10th digit of `p` (README round 17).
    pub p_rgh_pcg: Option<PRghPcg>,
    /// `Some` when `solvers.p_rgh` selects `GAMG` (**default, upstream's form since gh:#1936, README round 72**): upstream's own `GAMGSolver`
    /// ([`crate::genfoam::common::gamg`]) runs in place of the ILU(0)-BiCGStab; [`TwoPhaseSolver::with_krylov_for_p_rgh`] is the named ablation.
    pub p_rgh_gamg: Option<crate::genfoam::common::gamg::GamgControls>,
    /// `true` (default, gh:#1361 round 17, gh:#1833): the temporary `UEqn.H()` field of `pEqnf_2p.H` carries the constraint
    /// patch field of a `symmetry` mesh patch, so `fvc::flux(H)` through it is zero. `false`
    /// ([`TwoPhaseSolver::with_zero_gradient_h_on_symmetry_patches`]): the port's earlier `zeroGradient`, which leaves
    /// `Sf . H_c` there once the velocity has a component normal to the patch.
    pub h_symmetry_constraint: bool,
    /// `residualControl.p_rgh.tolerance` (with `useFirstPISOInitialResidual`).
    pub residual_control_p_rgh: Option<f64>,
    /// `residualControl.h.<fluid1>.tolerance`.
    pub residual_control_h1: Option<f64>,
    /// `massTransferSafetyFactor`.
    pub mass_transfer_safety_factor: Option<f64>,
    /// `maxTInterfaceDdt` [K/s].
    pub max_t_interface_ddt: Option<f64>,
    /// `enthalpyStabilizationMode`.
    pub he_stabilization: HeStabilization,
    /// Field relaxation factor of `alpha.*`.
    pub relax_alpha: f64,
    /// Field relaxation factor of `dmdt.*`.
    pub relax_dmdt: f64,
    /// Field relaxation factor of `T.interface`.
    pub relax_t_interface: f64,
    /// Field relaxation factor of `h.<fluid k>`.
    pub relax_he: [f64; 2],
    /// Field relaxation factor of `phi.<fluid k>` (`reconstructU_2p.H`: `phik.relax()` against the `phik.storePrevIter()`
    /// of `refsAndControls_2p.H`, i.e. the flux at the start of the outer iteration). Not applied by the port before
    /// gh:#1361 round 8 (no earlier ported case sets one; the PSBT cases set `"phi.*" 0.8`).
    pub relax_phi: [f64; 2],
    /// `relaxationFactors.equations` factor selected by `p_rgh` (`pEqn.relax()`, `solveP_rgh_2p.H`); `None` when no key or pattern matches (no relaxation).
    pub relax_p_rgh_equation: Option<f64>,
    /// `relaxationFactors.equations` factor selected by `h.<fluid k>` (`E1Eqn.relax()`, `EEqns_2p.H`); `None`: no relaxation.
    pub relax_he_equation: [Option<f64>; 2],
    /// `minKdFF` (default 1).
    pub min_kd_ff: f64,
    /// `alpha.minAlphaSp` (default 1e-4).
    pub min_alpha_sp: f64,
    /// `alpha.solvePhase` as a fluid index.
    pub solve_phase: usize,
    /// `pMin` [Pa].
    pub p_min: f64,
    /// `maxDpdt` [Pa/s] (default 1e6).
    pub max_dpdt: f64,
    /// Whether the thermo packages carry `dpdt` (OpenFOAM default `true`).
    pub dpdt: bool,
    /// Reproduce what GeN-Foam 652b3da does in the FIRST time step because
    /// OpenFOAM creates a field's old-time copy lazily (gh:#1361 round 3).
    /// Default `true` (upstream is the specification); see
    /// [`TwoPhaseSolver::without_first_step_lazy_old_time`] for the ablation.
    ///
    /// 1. `fvc::ddt(p_)` in the energy equation is the first request for
    ///    `p.oldTime()`, so in step 1 the old-time pressure is the pressure
    ///    after the first `p_rgh` solve, not the initial field (`dpdt` = 0 in
    ///    the first outer iteration).
    /// 2. `fixedPower::correct` writes `T_[celli]` through `operator[]`, which
    ///    does not call `storeOldTimes`, so the old-time copy created in outer
    ///    iteration 1 carries a stale time index and is overwritten with the
    ///    iteration-1 result at the start of outer iteration 2 (and only then):
    ///    in step 1 the structure is advanced from its iteration-1 temperature
    ///    in every iteration after the first, i.e. it receives one extra step of
    ///    power, `alpha q dt / (alpha rho Cp)`.
    pub lazy_old_time_first_step: bool,
    /// Reproduce UPSTREAM's `customPimpleControl::loop` defect (GeN-Foam 652b3da, gh:#1361 round 4).
    /// **Default `false`: the corrected, clean ending** (maintainer decision 2026-11-12: ports
    /// default to correct behaviour, the upstream defect is the explicit act). When the residual
    /// criteria are first met in the LAST outer iteration, upstream leaves `converged_` set and the
    /// NEXT time step solves nothing. `true` reproduces that
    /// ([`TwoPhaseSolver::with_upstream_stale_pimple_converged_flag`]); the upstream comparison tests
    /// set it. The skipped step still advances time and runs the courant/`maxDeltaT` update; it adds
    /// nothing to the cumulative continuity errors (~~"and the port's unconditional `calcCumulContErrs`"~~
    /// **CORRECTED 2026-11-13:** `calcCumulContErrs` now runs only in the iteration flagged converged,
    /// as upstream's does; checked against the real binary in the `nOuterCorrectors 6` control).
    pub stale_pimple_converged_flag: bool,
    /// The patch values of `p` are the case file's (`READ_IF_PRESENT`, `calculated`, `value`) until the first
    /// `p_ = max(p_rgh_ + rho_*gh_, pMin_)`, as in GeN-Foam 652b3da (gh:#1361 round 5). **Default `true`.**
    /// The thermo packages are constructed from `p`, so for an inlet whose `p_rgh` is fixed at 1.4e5 and whose
    /// `p` file says 1e5, the construction-time patch densities (and their lazily stored old-time copies) belong
    /// to 1e5, not 1.4e5; for an absent phase (`alpha = 0`, `alphaRhof = residualAlpha rho`) that sets the
    /// inlet-face flux in steps 1 and 2. `false` ([`TwoPhaseCase::without_p_patch_from_file`]) is the earlier form.
    pub p_patch_from_file: bool,
    /// Reproduce what `phaseChangeModel::correctInterfacialTemperature` really does
    /// with `maxTInterfaceDdt` (gh:#1361 round 3). Default `true`.
    ///
    /// The code reads `iT_.oldTime()[i]` AFTER `saturationPtr_->correctField(iT_)`
    /// has overwritten `iT_`, and `correctField` ends with
    /// `correctBoundaryConditions()`, which calls `storeOldTimes()`: the old-time
    /// copy is therefore refreshed from the NEW saturation temperature (in step 1
    /// the copy is first created from it), so `(iT - iT0)/dt` is 0 and the
    /// `maxTInterfaceDdt` limiter never limits. Measured: after step 2 of
    /// `1D_boiling` upstream's `T.interface` moved by +4.747 K from a saturation
    /// jump of +37.98 K, where the limiter as documented would allow +1.375 K.
    /// `false` applies the limiter against the temperature at the start of the step
    /// (the documented intent; the port's behaviour before round 3).
    pub t_interface_limiter_sees_new_value: bool,
    /// `tolerance` of the linear solver selected for `alpha.<solved phase>`
    /// (the catch-all `".*"` entry in `1D_boiling`).
    pub alpha_solver_tolerance: f64,
    /// `relTol` of that solver (0 if absent).
    pub alpha_solver_rel_tol: f64,
    /// `true` (default): solve the `alpha` equation with OpenFOAM's
    /// `PBiCGStab`/`diagonal` and the case's tolerances, stopping where upstream
    /// stops (gh:#1361 round 3: the tail of a decaying vapour front is solver
    /// noise in upstream). `false`: solve it to round-off with ILU(0)-BiCGStab (the
    /// port's earlier behaviour,
    /// [`TwoPhaseSolver::with_exact_alpha_solve`]).
    pub alpha_solver_openfoam: bool,
    /// `false` (default): the forced-convection Nusselt htc uses upstream's operation order
    /// `(kappa/Dh)*(A + B*pow(Re,C)*pow(Pr,D)*Tratio)`. `true`: the order the port used before gh:#1361
    /// round 8 ([`FsForcedConvectionHtc::heat_transfer_coefficient_earlier_association`],
    /// [`TwoPhaseSolver::with_earlier_nusselt_association`]).
    pub earlier_nusselt_association: bool,
    /// `false` (default): `T` comes back from `he` as `species::thermo::T` does it (one Newton iterate, stopping at
    /// `|dT| <= 1e-4 T0`, [`PhaseThermo::t_from_he`]). `true`: the converged root the port computed before gh:#1361
    /// round 13 ([`PhaseThermo::t_from_he_converged`], [`TwoPhaseSolver::with_converged_temperature_newton`]).
    pub converged_temperature_newton: bool,
    /// `false` (default): the `enthalpyStabilizationMode cutoff` clamp uses upstream's marker fields
    /// (`aboveThermoResidualAlpha`/`belowThermoResidualAlpha`), set at the START of the outer iteration. `true`: the
    /// port's earlier form, which tested the fractions as the alpha equation had just left them
    /// ([`TwoPhaseSolver::with_live_thermo_residual_markers`]).
    pub live_thermo_residual_markers: bool,
    /// `false` (default): the interfacial heat-flux ratio of `phaseChangeModel::correct` is `dmdtIi/dmdtIi0` with
    /// `dmdtIi = (1 - fw) dmdt`, as upstream. `true`: the port's earlier `dmdt/dmdtIi0` (the same wherever `fw = 0`, i.e.
    /// without wall boiling; [`TwoPhaseSolver::with_earlier_interfacial_flux_ratio`]).
    pub earlier_interfacial_flux_ratio: bool,
}

/// Time-step control (`controlDict`).
#[derive(Debug, Clone, PartialEq)]
pub struct TimeControls {
    /// `adjustTimeStep`.
    pub adjust_time_step: bool,
    /// `maxCo`.
    pub max_co: f64,
    /// `maxCoTwoPhase`.
    pub max_co_two_phase: Option<f64>,
    /// `marginToPhaseChange` [K].
    pub margin_to_phase_change: Option<f64>,
    /// `maxDeltaT` [s].
    pub max_delta_t: f64,
    /// `writeInterval` [s] with `writeControl adjustableRunTime`.
    pub write_interval: Option<f64>,
    /// `startTime` [s].
    pub start_time: f64,
    /// `true` (default): snap the step to the write times exactly as ESI
    /// `Time::adjustDeltaT` does (`max(1, round(nSteps))` steps, growth capped at
    /// 2x and shrink at 0.2x, `writeTimeIndex` from `Time::operator++`), which is
    /// what GeN-Foam 652b3da on v2506 runs (gh:#1361 round 3). `false` keeps the
    /// port's earlier rule (always round the step count UP, from the written
    /// time): see [`TwoPhaseSolver::with_foundation_style_write_time_adjust`].
    pub esi_adjust_delta_t: bool,
}

/// One fluid's field state (`Foam::fluid`).
#[derive(Debug, Clone)]
pub struct Phase {
    /// Phase name (`fluid1`/`fluid2` in `phaseProperties`).
    pub name: String,
    /// `stateOfMatter liquid`.
    pub is_liquid: bool,
    /// Thermophysical package.
    pub thermo: PhaseThermo,
    /// `residualAlpha` (default 1e-6).
    pub residual_alpha: f64,
    /// `thermoResidualAlpha` (default 0).
    pub thermo_residual_alpha: f64,
    /// `fluid::aboveThermoResidualAlpha`: `pos(normalized - thermoResidualAlpha)` (1 where `>= 0`) as set by the last
    /// `FFPair::correct`, i.e. from the fractions of the START of the outer iteration; `EEqns_2p.H`'s `cutoff`
    /// reads it after the alpha equation has changed `normalized` (gh:#1361 round 13).
    pub above_thermo_residual_alpha: Vec<f64>,
    /// `fluid::belowThermoResidualAlpha`: `neg0(normalized - thermoResidualAlpha)` (1 where `<= 0`), same timing.
    pub below_thermo_residual_alpha: Vec<f64>,
    /// `dispersedDiameterModel constant` value [m].
    pub dispersed_diameter: f64,
    /// `dispersedDiameterModel byRegime` (constant sub-models; gh:#1225); when `Some` it replaces
    /// [`Phase::dispersed_diameter`].
    pub dispersed_diameter_by_regime: Option<ByRegimeModel<f64>>,
    /// `alphaRhoMagU.<fluid>` = `alpha rho |U|` (`fluid::correctAlphaRhoMagU`; the mass flux a regime map reads).
    pub alpha_rho_mag_u: Vec<f64>,
    /// Phase fraction (of the whole cell volume).
    pub alpha: Scalar,
    /// Phase fraction normalised by the fluid (moving) fraction.
    pub normalized: Vec<f64>,
    /// Velocity [m/s].
    pub u: Vector,
    /// Phase velocity flux `U_f . Sf` [m^3/s].
    pub phi: Vec<f64>,
    /// `alphaPhi` [m^3/s].
    pub alpha_phi: Vec<f64>,
    /// `alphaRhoPhi` [kg/s].
    pub alpha_rho_phi: Vec<f64>,
    /// Density [kg/m^3].
    pub rho: Scalar,
    /// Sensible enthalpy [J/kg].
    pub he: Scalar,
    /// Temperature [K].
    pub t: Scalar,
    /// `Cp` [J/(kg K)].
    pub cp: Scalar,
    /// `mu` [Pa s].
    pub mu: Scalar,
    /// `kappa` [W/(m K)].
    pub kappa: Scalar,
    /// `psi` [s^2/m^2].
    pub psi: Vec<f64>,
    /// Dilation `dgdt` [1/s].
    pub dgdt: Vec<f64>,
    /// Continuity error [kg/(m^3 s)].
    pub cont_err: Vec<f64>,
    /// Time integral of the continuity error, per unit volume [kg/m^3].
    pub cumul_cont_err: f64,
    /// `|U|`.
    pub mag_u: Vec<f64>,
    /// `dispersion` (1 = dispersed).
    pub dispersion: Vec<f64>,
    /// Fluid hydraulic diameter [m] (`fluidDiameterModel::correctField`).
    pub dh: Vec<f64>,
    /// `RASModel porousKEpsilon2PhaseCorrected` of this fluid (gh:#1802); `None` for `simulationType laminar`.
    pub turbulence: Option<PorousKEpsilon2PhaseCorrected>,
    /// Flow quality.
    pub quality: Vec<f64>,
    /// Lockhart-Martinelli parameter.
    pub xlm: Vec<f64>,
    // old-time values
    alpha_old: Vec<f64>,
    /// Old-time boundary values (`alpha.oldTime().boundaryField()`): OpenFOAM stores
    /// the whole field, so a calculated patch (e.g. `rho` from the patch `p`)
    /// keeps its own old value, not the owner cell's.
    alpha_old_b: Vec<f64>,
    rho_old_b: Vec<f64>,
    rho_old: Vec<f64>,
    he_old: Vec<f64>,
    u_old: Vec<Vector3>,
    phi_old: Vec<f64>,
}

/// The structure (`Foam::structure`) restricted to what the ported tutorials
/// use: volume fraction, hydraulic diameter and `fixedPower` power models.
#[derive(Debug, Clone)]
pub struct Structure {
    /// Structure volume fraction `alpha_s`.
    pub alpha: Scalar,
    /// Structure hydraulic diameter [m] per cell.
    pub dh: Vec<f64>,
    /// `fixedPower` model per cell (cells of power-model zones).
    pub power: Vec<Option<FixedPower>>,
    /// Active-structure interfacial area density [1/m] (`iAact`).
    pub ia_act: Vec<f64>,
    /// Active-structure surface temperature [K] (`Tact`).
    pub t_act: Vec<f64>,
    /// `Twall` (`structure::Twall_`, `T.wall`): zero until the first `structure::correct`, which copies `Tact`.
    pub t_wall: Vec<f64>,
    /// `powerOffCriterionModel`.
    pub power_off: Option<PowerOffCriterion>,
    /// Whether power has been switched off.
    pub powered_off: bool,
    power_old: Vec<Option<FixedPower>>,
    /// The radial-conduction power models (`heatedPin`, `nuclearFuelPin`, `lumpedNuclearStructure`,
    /// `interpolatedNuclearFuelPin`; gh:#1884), with their state, one per model type in `structureProperties`.
    pub radial: Vec<RadialPowerModel>,
    /// The structure's `powerDensityNeutronics` per cell [W/m^3] (the radial models' `q`).
    pub q_neutronics: Vec<f64>,
    /// `heatFlux.structure` (`H Twall - HT`) ~~on the radial models' cells, 0 elsewhere~~ on EVERY structure cell
    /// ([`Self::cells`]; `structure.C:1057-1071`, gh:#1967 round 83, as round 82 fixed for onePhase), 0 elsewhere (its constructor value).
    /// Written only: no ported closure reads it (`useExplicitHeatFlux true` of Shah/Gorenflo is refused by name).
    pub heat_flux: Vec<f64>,
    /// `structure::cells_`: the cells of every `structureProperties` zone, in dictionary order ([`Self::with_cells`]; empty for a
    /// structure built by [`Self::new`] alone, whose `heatFlux` is then set on the radial models' cells only).
    pub cells: Vec<usize>,
    /// `alpha.passiveStructure` (`structure::alphapas_`, `structure.C:138-148`, `:430-460`): a copy of the structure field as
    /// constructed, i.e. `alpha.structure` as read from the start directory, else 0, then the `passiveProperties.volumeFraction` of
    /// each passive zone. The twoPhase reader refuses `passiveProperties` and a start-directory `alpha.structure` by name, so it is 0
    /// in every cell (gh:#1967 round 83; written only, no ported equation reads it).
    pub alpha_passive: Vec<f64>,
    /// Whether a `lumpedNuclearStructure` region needs `fvc::laplacian(Tmatrix)` (`kappaMatrix != 0`).
    pub radial_needs_laplacian: bool,
    /// CONTROLS, not upstream (gh:#1884): each deliberately breaks one ingredient of the radial wiring.
    pub radial_controls: RadialControls,
    /// `structureProperties.heatExchangers` with `iAHX` and `T.HX` (README round 44, gh:#1847); empty when none.
    pub heat_exchangers: HeatExchangerSet,
}

/// Ablations of the radial-power wiring, for the controls of README round 41 only (all `false` = upstream).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RadialControls {
    /// `HT` from the temperatures BEFORE the energy solve (`EEqn_1p.H` style) instead of `EEqns_2p.H:331`'s post-solve `T1`, `T2`.
    pub ht_pre_solve: bool,
    /// `fvc::laplacian(Tmatrix)` of the lumped model taken as 0.
    pub no_laplacian: bool,
    /// `Tact` never handed back from the models (stays at its constructor value).
    pub freeze_tact: bool,
}

/// Run-time diagnostics of one time step.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StepReport {
    /// Time at the end of the step [s].
    pub time: f64,
    /// Step size [s].
    pub dt: f64,
    /// PIMPLE outer iterations performed.
    pub outer_iterations: usize,
    /// Initial residual of the last outer iteration's first `p_rgh` solve.
    pub p_rgh_initial_residual: f64,
    /// Initial residual of the last `h.<fluid1>` solve.
    pub h1_initial_residual: f64,
    /// Courant number used to set the next step.
    pub courant: f64,
    /// The residual criteria were first met in the LAST outer iteration (`corr == nOuterCorrectors`).
    /// Upstream's `customPimpleControl::loop` then leaves `converged_` set and the NEXT step solves
    /// nothing (gh:#1361 round 4). By default this port ends the loop cleanly and never skips the
    /// next step (the corrected behaviour, the default since the maintainer decision of 2026-11-12);
    /// this flag records where upstream would have, and is the same in both modes.
    pub upstream_would_skip_next_step: bool,
    /// Outer iteration (1-based) at whose START the residual criteria were met, so that iteration
    /// ran as the "final" one (`converged_` set); 0 when they were never met in this step.
    pub converged_at: usize,
}

/// The two-phase solver. Build from a GeN-Foam case with
/// [`TwoPhaseCase::read`] then [`TwoPhaseSolver::new`], and advance with
/// [`TwoPhaseSolver::step`].
#[derive(Debug, Clone)]
pub struct TwoPhaseSolver {
    /// Mesh geometry.
    pub geom: Geometry,
    /// Gravity [m/s^2].
    pub g: Vector3,
    /// `gh` per cell and `ghf` per face [m^2/s^2].
    pub gh: Scalar,
    ghf: Vec<f64>,
    /// Static pressure [Pa].
    pub p: Scalar,
    /// `p_rgh` [Pa].
    pub p_rgh: Scalar,
    p_old: Vec<f64>,
    /// Whether `p.oldTime()` has been created yet. In OpenFOAM a field's old-time
    /// copy exists only once something has asked for it: GeN-Foam's first request
    /// for `p.oldTime()` is the `fvc::ddt(p_)` of the energy equation, so during the
    /// FIRST time step the old-time pressure is the pressure at that first call (after
    /// the first `p_rgh` solve), not the initial field, and `dpdt` is exactly 0 in the
    /// first outer iteration (gh:#1361 round 3). From the second step on the old-time
    /// copy is stored at every time increment, as usual.
    p_old_stored: bool,
    /// `true` until the first outer iteration of step 1 has advanced the structure
    /// (see [`TwoPhaseControls::lazy_old_time_first_step`], item 2).
    structure_old_time_stale: bool,
    /// `true` until the first `solveAlpha` (`solvePhase` set, step 1, one sub-cycle) has run: upstream creates the
    /// OTHER fluid's `alpha.oldTime()` lazily, at its first request (`correctContErrs`, `fvm::ddt(alpha, rho, U)`),
    /// which comes AFTER the lambda has assigned the field, so in step 1 that old-time copy holds the value the
    /// first outer iteration's MULES left (gh:#1799; see [`TwoPhaseControls::lazy_old_time_first_step`]).
    other_alpha_old_stale: bool,
    /// Mixture density over the moving fraction [kg/m^3].
    pub rho_mix: Scalar,
    /// Total volumetric flux [m^3/s].
    pub phi: Vec<f64>,
    /// The two fluids.
    pub fluids: [Phase; 2],
    /// The structure.
    pub structure: Structure,
    /// Closure selections.
    pub models: TwoPhaseModels,
    /// Solution controls.
    pub controls: TwoPhaseControls,
    /// The cached `GAMG` agglomeration of the mesh (gh:#1936).
    pub gamg: gamg_solve::GamgSolveState,
    /// Time controls.
    pub time_controls: TimeControls,
    /// Interfacial temperature [K] (`T.interface`).
    pub t_interface: Vec<f64>,
    t_interface_old: Vec<f64>,
    /// `iT_.oldTime()` as OpenFOAM leaves it: the raw saturation temperature of the
    /// step's first outer iteration (see `t_interface_limiter_sees_new_value`).
    t_interface_raw0: Vec<f64>,
    /// Outer iteration (1-based) in progress.
    corr_in_step: usize,
    /// Sub-cycles of the last MULES alpha solve (diagnostic; 0 before the first).
    pub alpha_sub_cycles: usize,
    /// `twoPhase::CoNum_` as the last `maxDeltaT` left it (used for the MULES sub-cycle count; gh:#1361 round 7).
    co_num: f64,
    /// `twoPhase::bothPhasesArePresent_` (construction: `max(alpha1 alpha2) >= 1e-4`; then `maxDeltaT`).
    both_phases_present: bool,
    /// `customPimpleControl::converged_`, kept across steps only when
    /// [`TwoPhaseControls::stale_pimple_converged_flag`] is set.
    pimple_converged: bool,
    /// Per outer iteration of the LAST step: (first-PISO `p_rgh` initial residual, last `h.<fluid1>`
    /// initial residual) at the end of that iteration, i.e. the two numbers
    /// `customPimpleControl::criteriaSatisfied` prints as "abs tol" at the start of the next one
    /// (diagnostic; gh:#1361 round 5).
    pub pimple_trace: Vec<(f64, f64)>,
    /// ESI `Time::writeTimeIndex_` (`adjustableRunTime` only).
    write_time_index: i64,
    /// `mesh.time().timeIndex()` (0 at construction, +1 per `Time::operator++`).
    steps_begun: i64,
    /// The fluid temperatures at the start of the last energy solve (used by the pre-solve control only).
    radial_t_pre: Option<[Vec<f64>; 2]>,
    /// Interfacial area density [1/m].
    pub interfacial_area: Vec<f64>,
    /// Phase-change rate fluid 1 → fluid 2 [kg/(m^3 s)].
    pub dmdt: Vec<f64>,
    /// `(fluid, initial residual, iterations)` of every enthalpy solve of the last time step, in solve order (upstream's log
    /// `...: Solving for h.<fluid>`; gh:#1957 round 83).
    pub h_solve_log: Vec<(usize, f64, usize)>,
    /// Latent heat [J/kg].
    pub latent_heat: Vec<f64>,
    /// Diagnostic (gh:#1361 round 13): when `Some`, every evaluation of `multiRegimeBoilingTRACE` appends the
    /// intermediates it computed (see [`TraceProbeRecord`]). `None` (the default) records nothing.
    pub trace_probe: Option<Vec<TraceProbeRecord>>,
    /// Wall (sub-cooled boiling) mass-transfer source `dmdtW` [kg/(m^3 s)], written by the
    /// `multiRegimeBoilingTRACE` model in `FSPair::correct` and consumed, then zeroed, by the next
    /// `phaseChangeModel::correct` (gh:#1222).
    pub dmdt_w: Vec<f64>,
    /// `CHFBoolMultiRegimeBoilingTRACECHF`: 0 pre-CHF, 1 post-CHF film boiling, 2 transition.
    pub chf_bool: Vec<f64>,
    /// `wfTransitionBoiling`.
    pub wf_transition_boiling: Vec<f64>,
    /// `LeidenfrostTemperature`.
    pub t_leidenfrost: Vec<f64>,
    /// Fluid-fluid drag coefficient [kg/(m^3 s)].
    pub kd_ff: Vec<f64>,
    /// Interfacial htc of each fluid [W/(m^2 K)].
    pub htc_ff: [Vec<f64>; 2],
    /// Fluid-structure drag coefficient of each fluid [kg/(m^3 s)].
    pub kd_fs: [Vec<f64>; 2],
    /// Fluid-structure htc of each fluid (partition applied) [W/(m^2 K)].
    pub htc_fs: [Vec<f64>; 2],
    /// Fluid-structure Reynolds number of each fluid.
    pub re_fs: [Vec<f64>; 2],
    /// Contact partition of each fluid.
    pub contact: [Vec<f64>; 2],
    /// Current time [s].
    pub time: f64,
    /// Current step size [s].
    pub dt: f64,
    // per-iteration caches
    alpha_rho_vm: Vec<f64>,
    he_src: [EnergySource; 2],
    regime_weights: Vec<Vec<(String, f64)>>,
    alpha_dispersed: Vec<f64>,
    dh_dispersed: Vec<f64>,
    rho_continuous: Vec<f64>,
    nu_continuous: Vec<f64>,
    pr_continuous: Vec<f64>,
    re_ff: Vec<f64>,
}

/// Interfacial and phase-change energy sources of one fluid, per unit
/// volume: `su` on the right-hand side, `sp` an implicit left-hand-side
/// coefficient (`fvm::Sp`, a sink when positive), and `su_sp` a left-hand-side
/// `fvm::SuSp` coefficient whose sign decides implicit or explicit.
#[derive(Debug, Clone, Default)]
struct EnergySource {
    su: Vec<f64>,
    sp: Vec<f64>,
    su_sp: Vec<f64>,
}

impl TwoPhaseSolver {
    /// Build the solver from a read case, reproducing `twoPhase`'s constructor:
    /// phase-fraction normalisation by the moving fraction, `alphaPhi` /
    /// `alphaRhoPhi` initialisation (`fluid::initAlphaPhis`), the initial
    /// `T.interface = TSat(p)` and latent heat (`phaseChangeModel` ctor).
    #[must_use]
    pub fn new(case: TwoPhaseCase) -> Self {
        let geom = Geometry::new_openfoam(case.mesh.clone());
        let n = geom.n_cells();
        let nf = geom.n_faces();
        let ni = geom.n_internal();
        let m = geom.mesh.clone();
        let g = case.gravity;
        let mut gh = Scalar::uniform(&geom, 0.0, vec![Bc::Calculated; m.patches.len()]);
        for c in 0..n {
            gh.c[c] = g.dot(m.cell_centres[c]);
        }
        let mut ghf = vec![0.0; nf];
        for f in 0..nf {
            ghf[f] = g.dot(m.face_centres[f]);
            if f >= ni {
                gh.b[f - ni] = ghf[f];
            }
        }
        let mut s = Self {
            g,
            gh,
            ghf,
            p: case.p.clone(),
            p_rgh: case.p_rgh.clone(),
            p_old: case.p.c.clone(),
            p_old_stored: !case.controls.lazy_old_time_first_step,
            structure_old_time_stale: case.controls.lazy_old_time_first_step,
            other_alpha_old_stale: case.controls.lazy_old_time_first_step,
            rho_mix: case.p.clone(),
            phi: vec![0.0; nf],
            fluids: case.fluids.clone(),
            structure: case.structure.clone(),
            models: case.models.clone(),
            controls: case.controls.clone(),
            gamg: gamg_solve::GamgSolveState::default(),
            time_controls: case.time_controls.clone(),
            t_interface: vec![0.0; n],
            t_interface_old: vec![0.0; n],
            t_interface_raw0: vec![0.0; n],
            corr_in_step: 0,
            alpha_sub_cycles: 0,
            co_num: 0.0,
            both_phases_present: false,
            pimple_converged: false,
            pimple_trace: Vec::new(),
            write_time_index: 0,
            steps_begun: 0,
            radial_t_pre: None,
            interfacial_area: vec![0.0; n],
            dmdt: vec![0.0; n],
            h_solve_log: Vec::new(),
            latent_heat: vec![0.0; n],
            trace_probe: None,
            dmdt_w: vec![0.0; n],
            chf_bool: vec![0.0; n],
            wf_transition_boiling: vec![0.0; n],
            t_leidenfrost: vec![0.0; n],
            kd_ff: vec![0.0; n],
            htc_ff: [vec![0.0; n], vec![0.0; n]],
            kd_fs: [vec![0.0; n], vec![0.0; n]],
            htc_fs: [vec![0.0; n], vec![0.0; n]],
            re_fs: [vec![10.0; n], vec![10.0; n]],
            contact: [vec![0.0; n], vec![0.0; n]],
            time: case.start_time,
            dt: case.delta_t,
            alpha_rho_vm: vec![0.0; n],
            he_src: [EnergySource::default(), EnergySource::default()],
            regime_weights: vec![Vec::new(); n],
            alpha_dispersed: vec![0.0; n],
            dh_dispersed: vec![0.0; n],
            rho_continuous: vec![0.0; n],
            nu_continuous: vec![0.0; n],
            pr_continuous: vec![0.0; n],
            re_ff: vec![10.0; n],
            geom,
        };
        // twoPhase.C:315-327: normalise the fluid fractions to the moving one.
        for c in 0..n {
            let moving = 1.0 - s.structure.alpha.c[c];
            let sum = s.fluids[0].alpha.c[c] + s.fluids[1].alpha.c[c];
            let corr = moving / sum;
            s.fluids[0].alpha.c[c] *= corr;
            s.fluids[1].alpha.c[c] *= corr;
        }
        // gh:#1801: an `inletOutlet` phase-fraction patch reads the flux, so the initial `phi = fvc::flux(U)` comes first
        for k in 0..2 {
            let u = s.fluids[k].u.clone();
            s.fluids[k].phi = fv::flux_openfoam(&s.geom, &u);
        }
        {
            let phis = [s.fluids[0].phi.clone(), s.fluids[1].phi.clone()];
            for k in 0..2 {
                s.fluids[k].alpha.correct_bc_with_flux(&s.geom, [&phis[0], &phis[1]]);
            }
        }
        s.update_normalized();
        s.both_phases_present = {
            let ni0 = s.geom.n_internal();
            let cells = (0..n).map(|c| s.fluids[0].alpha.c[c] * s.fluids[1].alpha.c[c]);
            let faces = (ni0..nf)
                .filter(|&f| s.geom.active[f])
                .map(|f| s.fluids[0].alpha.b[f - ni0] * s.fluids[1].alpha.b[f - ni0]);
            cells.chain(faces).fold(f64::MIN, f64::max) >= 1e-4
        };
        // thermo of the initial state, then fluxes (fluid::initAlphaPhis).
        for k in 0..2 {
            s.thermo_correct(k, true);
            let u = s.fluids[k].u.clone();
            s.fluids[k].phi = fv::flux_openfoam(&s.geom, &u);
            let af = fv::interp_linear_openfoam(&s.geom, &s.fluids[k].alpha);
            let rf = fv::interp_linear_openfoam(&s.geom, &s.fluids[k].rho);
            let ap: Vec<f64> = (0..nf).map(|f| af[f] * s.fluids[k].phi[f]).collect();
            s.fluids[k].alpha_rho_phi = (0..nf).map(|f| rf[f] * ap[f]).collect();
            s.fluids[k].alpha_phi = ap;
            s.fluids[k].mag_u = s.fluids[k].u.c.iter().map(|v| v.mag()).collect();
            // fluid::initAlphaPhis: `alphaRhoMagU_ = (*this)*this->rho()*magU_` (no file for it)
            s.fluids[k].alpha_rho_mag_u = (0..n)
                .map(|c| s.fluids[k].alpha.c[c] * s.fluids[k].rho.c[c] * s.fluids[k].mag_u[c])
                .collect();
        }
        s.phi = (0..nf)
            .map(|f| s.fluids[0].alpha_phi[f] + s.fluids[1].alpha_phi[f])
            .collect();
        s.update_mixture_density();
        // p on patches is `calculated`. Upstream keeps the file's patch values until the first
        // `p_ = max(p_rgh_ + rho_*gh_, pMin_)`; the earlier form set `p = p_rgh + rho gh` here.
        if !s.controls.p_patch_from_file {
            for f in ni..nf {
                if s.geom.active[f] {
                    s.p.b[f - ni] = s.p_rgh.b[f - ni] + s.rho_mix.b[f - ni] * s.ghf[f];
                }
            }
        }
        // Structure surface temperature starts at the power models' T.
        for c in 0..n {
            if let Some(pm) = &s.structure.power[c] {
                s.structure.t_act[c] = pm.surface_temperature().get::<kelvin>();
            }
        }
        // phaseChangeModel ctor: T.interface = TSat(p); L from the model.
        if let Some(pc) = s.models.phase_change.clone() {
            for c in 0..n {
                s.t_interface[c] = pc
                    .saturation
                    .t_sat(Pressure::new::<pascal>(s.p.c[c]))
                    .get::<kelvin>();
            }
            s.correct_latent_heat(&pc);
        }
        s.t_interface_old = s.t_interface.clone();
        s
    }

    /// Moving (fluid) volume fraction `1 - alpha_s` of cell `c`.
    fn moving(&self, c: usize) -> f64 {
        1.0 - self.structure.alpha.c[c]
    }

    fn update_normalized(&mut self) {
        let n = self.geom.n_cells();
        for k in 0..2 {
            for c in 0..n {
                self.fluids[k].normalized[c] =
                    self.fluids[k].alpha.c[c] / (1.0 - self.structure.alpha.c[c]);
            }
        }
    }

    /// `rho_ = (alpha1 rho1 + alpha2 rho2)/movingAlpha` on cells and faces.
    fn update_mixture_density(&mut self) {
        let ni = self.geom.n_internal();
        let n = self.geom.n_cells();
        for c in 0..n {
            self.rho_mix.c[c] = (self.fluids[0].alpha.c[c] * self.fluids[0].rho.c[c]
                + self.fluids[1].alpha.c[c] * self.fluids[1].rho.c[c])
                / self.moving(c);
        }
        for f in ni..self.geom.n_faces() {
            if !self.geom.active[f] {
                continue;
            }
            let i = f - ni;
            let mv = 1.0 - self.structure.alpha.b[i];
            self.rho_mix.b[i] = (self.fluids[0].alpha.b[i] * self.fluids[0].rho.b[i]
                + self.fluids[1].alpha.b[i] * self.fluids[1].rho.b[i])
                / mv;
        }
    }

    /// Advance one time step (`twoPhase::correctPhysics` inside upstream's
    /// time loop), then set the next step size (`twoPhase::maxDeltaT`).
    ///
    /// # Errors
    /// [`crate::error::AppBuilderError::Diverged`] when a field turns
    /// non-finite, or when the step size is not positive or has collapsed below the resolution of
    /// the time (`time + dt == time`).
    pub fn step(&mut self) -> Result<StepReport, crate::error::AppBuilderError> {
        // A step that cannot advance the clock (`dt` collapsed below the resolution of `time`, or is not
        // positive) would let the caller loop forever: `maxDeltaT` can only grow a zero step by a factor
        // of zero. Measured (gh:#1361 round 5): the shipped-tolerance 1D_boiling run with the upstream
        // PIMPLE option and the earlier construction-time `p` patch reached a Courant number that took
        // `dt` from 1.9e-3 to 2.4e-7 and then below the ulp of t = 9.9498 s, where it spun with `dt`
        // effectively 0. Upstream has no such guard (it would spin too); this is a port-side guard.
        if self.dt.is_nan() || self.dt <= 0.0 || self.time + self.dt == self.time {
            return Err(crate::error::AppBuilderError::Diverged { iter: 0, residual: f64::NAN });
        }
        let n = self.geom.n_cells();
        self.h_solve_log.clear();
        for k in 0..2 {
            let f = &mut self.fluids[k];
            f.alpha_old = f.alpha.c.clone();
            f.rho_old = f.rho.c.clone();
            f.alpha_old_b = f.alpha.b.clone();
            f.rho_old_b = f.rho.b.clone();
            f.he_old = f.he.c.clone();
            f.u_old = f.u.c.clone();
            f.phi_old = f.phi.clone();
            if let Some(t) = f.turbulence.as_mut() {
                t.begin_time_step();
            }
        }
        if self.p_old_stored {
            self.p_old = self.p.c.clone();
        }
        self.t_interface_old = self.t_interface.clone();
        self.structure.power_old = self.structure.power.clone();
        self.time += self.dt;
        self.steps_begun += 1;
        if let Some(wi) = self.time_controls.write_interval {
            // `Time::operator++`: move the write index.
            let write_index = (((self.time - self.time_controls.start_time) + 0.5 * self.dt) / wi) as i64;
            if write_index > self.write_time_index {
                self.write_time_index = write_index;
            }
        }

        let mut report = StepReport {
            time: self.time,
            dt: self.dt,
            ..StepReport::default()
        };
        let n_outer = self.controls.n_outer.max(1);
        let mut corr = 0;
        self.pimple_trace.clear();
        // customPimpleControl::loop(), call by call (as `OnePhaseFcSolver::step`)
        loop {
            corr += 1;
            if corr == n_outer + 1 {
                // Upstream leaves `converged_` set here (the defect); the default clears it.
                if !self.controls.stale_pimple_converged_flag {
                    self.pimple_converged = false;
                }
                break;
            }
            if self.pimple_converged {
                self.pimple_converged = false;
                break;
            }
            if corr > 1 && self.criteria_satisfied(&report) {
                self.pimple_converged = true;
                report.upstream_would_skip_next_step = corr == n_outer;
                report.converged_at = corr;
            }
            self.corr_in_step = corr;
            self.correct_models()?;
            let (p_res, n_piso) = self.correct_fluid_mechanics()?;
            let _ = n_piso;
            report.p_rgh_initial_residual = p_res;
            // `twoPhase::calcCumulContErrs` is the last line of `correctFluidMechanics` and runs
            // `if (pimple_.finalIter())`. That inherited `finalIter()` is `converged_ || corr_ ==
            // nCorrPIMPLE` on the BASE class's `corr_`, which `customPimpleControl::loop` never
            // advances (its own `corr_` shadows it), so it is true only in the iteration flagged as
            // converged: a step that never meets its criteria, and a skipped step, add nothing.
            // (Round 5; the port added in every step before. Same as `OnePhaseFcSolver::step`.)
            if self.pimple_converged {
                self.calc_cumul_cont_errs();
            }
            report.h1_initial_residual = self.correct_energy();
            report.outer_iterations = corr;
            self.pimple_trace.push((report.p_rgh_initial_residual, report.h1_initial_residual));
        }
        for k in 0..2 {
            if self.fluids[k].alpha.c.iter().any(|v| !v.is_finite())
                || self.fluids[k].t.c.iter().any(|v| !v.is_finite())
            {
                return Err(crate::error::AppBuilderError::Diverged {
                    iter: corr,
                    residual: f64::NAN,
                });
            }
        }
        let _ = n;
        report.courant = self.courant();
        if self.time_controls.adjust_time_step {
            // `maxDeltaT` first runs `correctCourant` and sets `bothPhasesArePresent_` (state the next
            // step's MULES sub-cycle count reads).
            self.co_num = self.co_num_upstream();
            self.both_phases_present = self.phases_present();
            self.dt = self.max_delta_t(report.courant);
        }
        self.adjust_to_write_time();
        Ok(report)
    }

    /// `customPimpleControl::criteriaSatisfied` for the residual controls the
    /// ported cases use (absolute tolerances; `relTol 0`).
    fn criteria_satisfied(&self, r: &StepReport) -> bool {
        let mut checked = false;
        let mut ok = true;
        if let Some(tol) = self.controls.residual_control_p_rgh {
            checked = true;
            ok &= r.p_rgh_initial_residual < tol;
        }
        if let Some(tol) = self.controls.residual_control_h1 {
            checked = true;
            ok &= r.h1_initial_residual < tol;
        }
        checked && ok
    }

    // ───────────────────────── correctModels ─────────────────────────

    /// `twoPhase::correctModels`: regime maps, `FFPair`, both `FSPair`s and
    /// `twoPhaseDragFactor`.
    fn correct_models(&mut self) -> Result<(), crate::error::AppBuilderError> {
        let n = self.geom.n_cells();
        // oneParameter::correct / twoParameters::correct
        match &self.models.regime_map {
            None => {}
            Some(RegimeMapSelection::OneParameter(map, k)) => {
                for c in 0..n {
                    self.regime_weights[c] = map.regime_weights(self.fluids[*k].normalized[c]);
                }
            }
            Some(RegimeMapSelection::TwoParameters { map, parameter1, parameter2 }) => {
                let field = |par: &RegimeParameter, c: usize| match *par {
                    RegimeParameter::NormalizedAlpha(k) => self.fluids[k].normalized[c],
                    RegimeParameter::AlphaRhoMagU(k) => self.fluids[k].alpha_rho_mag_u[c],
                };
                let mut weights = Vec::with_capacity(n);
                for c in 0..n {
                    let w = map
                        .regime_weights(field(parameter1, c), field(parameter2, c))
                        .map_err(|e| crate::error::AppBuilderError::Case {
                            case: crate::path::PathBuf::from("twoParameters regime map"),
                            msg: e,
                        })?;
                    weights.push(w);
                }
                self.regime_weights = weights;
            }
        }
        self.correct_ff_pair();
        for k in 0..2 {
            self.correct_fs_pair(k)?;
        }
        self.correct_two_phase_drag_factor();
        Ok(())
    }

    /// `FFPair::correct` (FFPair.C:441-730).
    fn correct_ff_pair(&mut self) {
        let n = self.geom.n_cells();
        // `fluid::correctThermoResidualMarkers` (FFPair.C:659-660): `pos(x)` is 1 for `x >= 0`, `neg0(x)` 1 for `x <= 0`
        for k in 0..2 {
            let f = &mut self.fluids[k];
            for c in 0..n {
                let x = f.normalized[c] - f.thermo_residual_alpha;
                f.above_thermo_residual_alpha[c] = f64::from(x >= 0.0);
                f.below_thermo_residual_alpha[c] = f64::from(x <= 0.0);
            }
        }
        // dispersion, diameters (fluidDiameterModel::correctField)
        for c in 0..n {
            let d1 = self.models.dispersion_fluid1.value(&self.regime_weights[c]);
            self.fluids[0].dispersion[c] = d1;
            self.fluids[1].dispersion[c] = 1.0 - d1;
        }
        let mut dh_cont = vec![0.0; n];
        for c in 0..n {
            let dhs = self.structure.dh[c];
            for k in 0..2 {
                let d = self.fluids[k].dispersion[c];
                self.fluids[k].dh[c] = if d == 0.0 {
                    dhs
                } else {
                    let dd = match &self.fluids[k].dispersed_diameter_by_regime {
                        Some(m) => m.value(&self.regime_weights[c], |v| *v),
                        None => self.fluids[k].dispersed_diameter,
                    };
                    d * dd + (1.0 - d) * dhs
                };
            }
            let (f1, f2) = (&self.fluids[0], &self.fluids[1]);
            let (d1, d2) = (f1.dispersion[c], f2.dispersion[c]);
            self.alpha_dispersed[c] = f1.alpha.c[c] * d1 + f2.alpha.c[c] * d2;
            let alpha_cont = f1.alpha.c[c] * d2 + f2.alpha.c[c] * d1;
            self.dh_dispersed[c] = f1.dh[c] * d1 + f2.dh[c] * d2;
            dh_cont[c] = f1.dh[c] * d2 + f2.dh[c] * d1;
            // spherical::value
            let a_sum = self.alpha_dispersed[c] + alpha_cont;
            let a = self.alpha_dispersed[c] / a_sum;
            let dh_d = self.dh_dispersed[c];
            let spherical = |cut: f64| {
                a_sum
                    * if a < cut {
                        6.0 * a / dh_d
                    } else {
                        6.0 * a / dh_d * (1.0 - a) / (1.0 - cut)
                    }
            };
            self.interfacial_area[c] = match &self.models.interfacial_area_by_regime {
                Some(m) => m.value(&self.regime_weights[c], |cut| spherical(*cut)),
                None => spherical(self.models.spherical_cutoff_alpha),
            };
            self.rho_continuous[c] = f1.rho.c[c] * d2 + f2.rho.c[c] * d1;
            let nu1 = f1.mu.c[c] / f1.rho.c[c];
            let nu2 = f2.mu.c[c] / f2.rho.c[c];
            self.nu_continuous[c] = nu1 * d2 + nu2 * d1;
            let pr1 = f1.cp.c[c] * f1.mu.c[c] / f1.kappa.c[c];
            let pr2 = f2.cp.c[c] * f2.mu.c[c] / f2.kappa.c[c];
            self.pr_continuous[c] = pr1 * d2 + pr2 * d1;
        }
        // relative velocity, Re, quality, XLM
        let mut mag_ur = vec![0.0; n];
        for c in 0..n {
            mag_ur[c] = (self.fluids[0].u.c[c] - self.fluids[1].u.c[c]).mag();
            self.re_ff[c] = (mag_ur[c] * self.dh_dispersed[c] / self.nu_continuous[c]).max(10.0);
            self.correct_quality(c);
        }
        // Drag (FFDragFactor::correctField) and its floor (minKdFF).
        let (l, v) = self.liquid_vapour();
        for c in 0..n {
            let (fl, fv) = (&self.fluids[l], &self.fluids[v]);
            let state = FfInterfacialState {
                vapour_fraction: Ratio::new::<ratio>(fv.normalized[c]),
                reynolds_number: ReynoldsNumber::new::<ratio>(self.re_ff[c]),
                rho_vapour: mass_density(fv.rho.c[c]),
                rho_liquid: mass_density(fl.rho.c[c]),
                rho_continuous: mass_density(self.rho_continuous[c]),
                dh_dispersed: Length::new::<meter>(self.dh_dispersed[c]),
                dh_continuous: Length::new::<meter>(dh_cont[c]),
                u_vapour: velocity(fv.mag_u[c]),
                u_liquid: velocity(fl.mag_u[c]),
                u_relative: velocity(mag_ur[c]),
            };
            let cd = self.models.ff_drag.drag_coefficient(&state).get::<ratio>();
            let kd = (self.fluids[0].alpha.c[c] + self.fluids[1].alpha.c[c])
                * (0.5 / self.dh_dispersed[c])
                * self.rho_continuous[c]
                * mag_ur[c]
                * cd;
            self.kd_ff[c] = if self.controls.min_kd_ff != 0.0 {
                kd.max(self.controls.min_kd_ff)
            } else {
                kd
            };
            // virtualMass::correct: alphaRhoVm = Vm alphaDispersed rhoContinuous
            self.alpha_rho_vm[c] = self.models.virtual_mass.unwrap_or(0.0)
                * self.alpha_dispersed[c]
                * self.rho_continuous[c];
        }
        // Energy part: htcs, markers, phase change, enthalpy sources.
        for k in 0..2 {
            if let Some(model) = self.models.ff_htc[k] {
                for c in 0..n {
                    let dhi = self.dh_dispersed[c].max(1e-4);
                    let kap = self.fluids[k].kappa.c[c];
                    self.htc_ff[k][c] = match model {
                        FfHtcModel::Nusselt { a, b, c: ce, d } => {
                            let nu = if b != 0.0 {
                                if ce == d {
                                    a + b * (self.re_ff[c] * self.pr_continuous[c]).r_powf(ce)
                                } else {
                                    a + b
                                        * self.re_ff[c].r_powf(ce)
                                        * self.pr_continuous[c].r_powf(d)
                                }
                            } else {
                                a
                            };
                            kap / dhi * nu
                        }
                        FfHtcModel::Constant(h) => h,
                    };
                }
            }
        }
        let htc_saved = self.htc_ff.clone();
        let mut zeroed = vec![false; n];
        if let Some(pc) = self.models.phase_change.clone() {
            self.correct_phase_change(&pc, &mut zeroed);
        } else {
            self.correct_interface_temperature_no_phase_change();
        }
        // FFPair heSources: -(htc iA (he/Cp + iT - T) - Sp(htc iA/Cp, he)).
        for k in 0..2 {
            let fl = &self.fluids[k];
            let src = &mut self.he_src[k];
            if src.su.len() != n {
                *src = EnergySource {
                    su: vec![0.0; n],
                    sp: vec![0.0; n],
                    su_sp: vec![0.0; n],
                };
            }
            for c in 0..n {
                let htc = if zeroed[c] { 0.0 } else { self.htc_ff[k][c] };
                let h_ia = htc * self.interfacial_area[c];
                src.su[c] += h_ia
                    * (fl.he.c[c] / fl.cp.c[c] + self.t_interface[c] - fl.t.c[c]);
                src.sp[c] += h_ia / fl.cp.c[c];
            }
        }
        self.htc_ff = htc_saved;
    }

    /// Indices of the liquid and the vapour fluid.
    fn liquid_vapour(&self) -> (usize, usize) {
        if self.fluids[0].is_liquid {
            (0, 1)
        } else {
            (1, 0)
        }
    }

    /// Flow quality and Lockhart-Martinelli parameter (FFPair.C:507-584).
    fn correct_quality(&mut self, i: usize) {
        let (a, b) = (&self.fluids[0], &self.fluids[1]);
        let m1 = a.alpha.c[i] * a.rho.c[i] * a.mag_u[i];
        let m2 = b.alpha.c[i] * b.rho.c[i] * b.mag_u[i];
        let sum = (m1 + m2).max(1e-69);
        let (x1, x2) = (m1 / sum, m2 / sum);
        let (mu1, mu2) = (a.mu.c[i], b.mu.c[i]);
        let (r1, r2) = (a.rho.c[i], b.rho.c[i]);
        let mut xlm1 = if x1 < 1e-6 {
            a.min_xlm()
        } else if x2 < 1e-6 {
            a.max_xlm()
        } else {
            ((mu1 / mu2).r_powf(0.1) * (x1 / x2).r_powf(0.9) * (r2 / r1).sqrt())
                .max(a.min_xlm())
                .min(a.max_xlm())
        };
        let mut xlm2 = if x2 < 1e-6 {
            b.min_xlm()
        } else if x1 < 1e-6 {
            b.max_xlm()
        } else {
            ((mu2 / mu1).r_powf(0.1) * (x2 / x1).r_powf(0.9) * (r1 / r2).sqrt())
                .max(b.min_xlm())
                .min(b.max_xlm())
        };
        let cn = (1.0 / (xlm1 * xlm2)).sqrt();
        xlm1 *= cn;
        xlm2 *= cn;
        self.fluids[0].quality[i] = x1;
        self.fluids[1].quality[i] = x2;
        self.fluids[0].xlm[i] = xlm1;
        self.fluids[1].xlm[i] = xlm2;
    }

    /// The interface temperature when no phase-change model is selected
    /// (FFPair.C:665-697): an htc-weighted mean where both fluids are above
    /// `thermoResidualAlpha`, else the present fluid's temperature.
    fn correct_interface_temperature_no_phase_change(&mut self) {
        let n = self.geom.n_cells();
        for c in 0..n {
            let (f1, f2) = (&self.fluids[0], &self.fluids[1]);
            let pos1 = f1.normalized[c] - f1.thermo_residual_alpha > 0.0;
            let pos2 = f2.normalized[c] - f2.thermo_residual_alpha > 0.0;
            let (h1, h2) = (self.htc_ff[0][c], self.htc_ff[1][c]);
            self.t_interface[c] = if pos1 && pos2 {
                (h1 * f1.t.c[c] + h2 * f2.t.c[c]) / (h1 + h2).max(1e-6)
            } else if pos1 {
                f1.t.c[c]
            } else if pos2 {
                f2.t.c[c]
            } else {
                0.0
            };
        }
        for k in 0..2 {
            self.he_src[k] = EnergySource {
                su: vec![0.0; n],
                sp: vec![0.0; n],
                su_sp: vec![0.0; n],
            };
        }
    }

    /// `latentHeatModel::correctField` + `adjust` (latentHeatModel.C).
    fn correct_latent_heat(&mut self, pc: &PhaseChangeConfig) {
        let n = self.geom.n_cells();
        let (l, v) = self.liquid_vapour();
        let l_sign = if self.fluids[0].is_liquid { 1.0 } else { -1.0 };
        for c in 0..n {
            let it = ThermodynamicTemperature::new::<kelvin>(self.t_interface[c]);
            let p = self.p.c[c];
            let hv_i = self.fluids[v].thermo.he(p, self.t_interface[c]);
            let hl_i = self.fluids[l].thermo.he(p, self.t_interface[c]);
            let value = pc
                .latent_heat
                .latent_heat(it, specific_energy(hv_i), specific_energy(hl_i))
                .value;
            let mut lc = l_sign * value;
            if pc.adjust_latent_heat {
                let d = self.dmdt[c];
                let (neg, pos) = (f64::from(d < 0.0), f64::from(d > 0.0));
                let dv = self.fluids[v].he.c[c] - hv_i;
                let dl = self.fluids[l].he.c[c] - hl_i;
                if l_sign > 0.0 {
                    lc += neg * dv - pos * dl;
                } else {
                    lc -= pos * dv - neg * dl;
                }
            }
            self.latent_heat[c] = lc;
        }
    }

    /// `phaseChangeModel::correct` with `heatDrivenPhaseChange` (phaseChangeModel.C:375-605,
    /// heatDrivenPhaseChange.C). Writes `dmdt`, `T.interface`, `L` and the
    /// energy-conservative enthalpy sources; marks the cells whose
    /// interfacial htc is zeroed for the `FFPair` sources.
    fn correct_phase_change(&mut self, pc: &PhaseChangeConfig, zeroed: &mut [bool]) {
        let n = self.geom.n_cells();
        // limitInterfacialArea
        if pc.residual_interfacial_area != 0.0 {
            for c in 0..n {
                self.interfacial_area[c] = self.interfacial_area[c].max(pc.residual_interfacial_area);
            }
        }
        // correctInterfacialTemperature (prevIter = value before this update)
        let it_prev = self.t_interface.clone();
        for c in 0..n {
            let mut it = pc
                .saturation
                .t_sat(Pressure::new::<pascal>(self.p.c[c]))
                .get::<kelvin>();
            if let Some(max_dtdt) = self.controls.max_t_interface_ddt {
                // `iT_.oldTime()` after `correctField` refreshed it: see
                // `TwoPhaseControls::t_interface_limiter_sees_new_value`.
                let it0 = if self.controls.t_interface_limiter_sees_new_value {
                    // outer iteration 1: the old-time copy is refreshed from the raw
                    // TSat just computed; later iterations still see that value.
                    if self.corr_in_step <= 1 {
                        self.t_interface_raw0[c] = it;
                    }
                    self.t_interface_raw0[c]
                } else {
                    self.t_interface_old[c]
                };
                let rate = ((it - it0) / self.dt).max(-max_dtdt).min(max_dtdt);
                it = it0 + rate * self.dt;
            }
            let f = self.controls.relax_t_interface;
            self.t_interface[c] = it_prev[c] + f * (it - it_prev[c]);
        }
        // latent heat
        self.correct_latent_heat(pc);
        // correctInterfacialDmdt
        let dmdt_prev = self.dmdt.clone();
        let mut dmdt = vec![0.0; n];
        for c in 0..n {
            let q1 = self.htc_ff[0][c] * self.interfacial_area[c] * (self.fluids[0].t.c[c] - self.t_interface[c]);
            let q2 = self.htc_ff[1][c] * self.interfacial_area[c] * (self.fluids[1].t.c[c] - self.t_interface[c]);
            let l = self.latent_heat[c];
            dmdt[c] = pc
                .rate
                .mass_transfer_rate(power_density(q1), power_density(q2), specific_energy(l))
                .value;
            // `dmdt_ = dmdtI_ + dmdtW_` (phaseChangeModel::correct)
            dmdt[c] += self.dmdt_w[c];
        }
        // limitMassTransfer
        let div1 = fv::div(&self.geom, &self.fluids[0].alpha_rho_phi);
        let div2 = fv::div(&self.geom, &self.fluids[1].alpha_rho_phi);
        for c in 0..n {
            let a1 = self.fluids[0].alpha.c[c];
            let a2 = self.fluids[1].alpha.c[c];
            let d = &mut dmdt[c];
            *d = d.max(0.0) * f64::from(a1 > 0.0) + d.min(0.0) * f64::from(a2 > 0.0);
            if let Some(sf) = self.controls.mass_transfer_safety_factor {
                if *d > 0.0 {
                    let maxd = (sf * a1 * self.fluids[0].rho.c[c] / self.dt - div1[c]).max(0.0);
                    *d = d.min(maxd);
                } else if *d < 0.0 {
                    let mind = (-sf * a2 * self.fluids[1].rho.c[c] / self.dt + div2[c]).min(0.0);
                    *d = d.max(mind);
                }
            }
        }
        // dmdt.relax()
        let f = self.controls.relax_dmdt;
        for c in 0..n {
            self.dmdt[c] = dmdt_prev[c] + f * (dmdt[c] - dmdt_prev[c]);
        }
        // energy-conservative heSources; fw = dmdtW/dmdt where |dmdt| > dmdtMin (gh:#1222: the
        // `multiRegimeBoilingTRACE` model writes dmdtW; with no such model dmdtW = 0 and fw = 0)
        let dmdt_min = 1e-2;
        for k in 0..2 {
            self.he_src[k] = EnergySource {
                su: vec![0.0; n],
                sp: vec![0.0; n],
                su_sp: vec![0.0; n],
            };
        }
        for c in 0..n {
            let d = self.dmdt[c];
            let p = self.p.c[c];
            let he1i = self.fluids[0].thermo.he(p, self.t_interface[c]);
            let he2i = self.fluids[1].thermo.he(p, self.t_interface[c]);
            if d.abs() > dmdt_min {
                let ia = self.interfacial_area[c];
                let l = self.latent_heat[c];
                let q1 = ia * self.htc_ff[0][c] * (self.fluids[0].t.c[c] - self.t_interface[c]);
                let q2 = ia * self.htc_ff[1][c] * (self.fluids[1].t.c[c] - self.t_interface[c]);
                let mut d0 = (q1 + q2) / l;
                d0 = if d0 >= 0.0 { d0.max(dmdt_min) } else { d0.min(-dmdt_min) };
                let fw = self.dmdt_w[c] / d;
                let dmdt_ii = (1.0 - fw) * d;
                let dmdt_wi = fw * d;
                // upstream: `scalar f(dmdtIi/dmdtIi0)`, the INTERFACIAL part of the (relaxed) dmdt over the
                // energy-conservative one; the port divided the total `dmdt` (gh:#1361 round 13)
                let fr = if self.controls.earlier_interfacial_flux_ratio { d / d0 } else { dmdt_ii / d0 };
                let dl = dmdt_ii * l;
                let mut c1 = (dl - fr * q2) / self.fluids[0].he.c[c];
                let mut c2 = (dl - fr * q1) / self.fluids[1].he.c[c];
                // wall contribution
                if dmdt_wi > 0.0 {
                    c1 += (dmdt_wi * l) / self.fluids[0].he.c[c];
                } else if dmdt_wi < 0.0 {
                    c2 += (dmdt_wi * l) / self.fluids[1].he.c[c];
                }
                if let Some(pr) = self.trace_probe.as_mut() {
                    pr.push(TraceProbeRecord {
                        kind: "pc",
                        time: self.time,
                        outer_iteration: self.corr_in_step,
                        cell: c,
                        values: vec![
                            ("dmdt", d),
                            ("fw", fw),
                            ("dmdtW", self.dmdt_w[c]),
                            ("iA", ia),
                            ("L", l),
                            ("htc1", self.htc_ff[0][c]),
                            ("htc2", self.htc_ff[1][c]),
                            ("q1", q1),
                            ("q2", q2),
                            ("f", fr),
                            ("dmdtI0", d0),
                            ("c1", c1),
                            ("c2", c2),
                            ("he1", self.fluids[0].he.c[c]),
                            ("he2", self.fluids[1].he.c[c]),
                            ("iT", self.t_interface[c]),
                            ("T1", self.fluids[0].t.c[c]),
                            ("T2", self.fluids[1].t.c[c]),
                        ],
                    });
                }
                self.he_src[0].su_sp[c] = c1;
                self.he_src[1].su_sp[c] = c2;
                zeroed[c] = true;
            }
            // fvm::Sp(dmdt12, he1) - dmdt21 he1I  /  fvm::Sp(dmdt21, he2) - dmdt12 he2I
            let (d12, d21) = (d.max(0.0), (-d).max(0.0));
            self.he_src[0].sp[c] += d12;
            self.he_src[0].su[c] += d21 * he1i;
            self.he_src[1].sp[c] += d21;
            self.he_src[1].su[c] += d12 * he2i;
        }
        // `dmdtW_` is reset after use: the wall models write it again in `FSPair::correct`
        for v in &mut self.dmdt_w {
            *v = 0.0;
        }
    }

    /// `FSPair::correct` for fluid `k` (FSPair.C:285-456).
    fn correct_fs_pair(&mut self, k: usize) -> Result<(), crate::error::AppBuilderError> {
        let n = self.geom.n_cells();
        let o = 1 - k;
        for c in 0..n {
            let fl = &self.fluids[k];
            let nu = fl.mu.c[c] / fl.rho.c[c];
            self.re_fs[k][c] = (fl.normalized[c] * fl.u.c[c].mag() * self.structure.dh[c] / nu).max(10.0);
        }
        // Kd from the friction model in its zones (isotropic FSDragFactor).
        for c in 0..n {
            if let Some(model) = self.models.fs_drag[k][c] {
                let fl = &self.fluids[k];
                let fd = model
                    .friction_factor(ReynoldsNumber::new::<ratio>(self.re_fs[k][c]))
                    .get::<ratio>();
                self.kd_fs[k][c] = 0.5 / self.structure.dh[c]
                    * (1.0 - self.structure.alpha.c[c])
                    * fl.rho.c[c]
                    * fl.mag_u[c]
                    * fd;
            }
        }
        // contact partition
        let (owner_k, model) = &self.models.contact_partition;
        for c in 0..n {
            let f = model.value(&self.regime_weights[c]);
            self.contact[k][c] = if *owner_k == k { f } else { 1.0 - f };
        }
        // htc
        for c in 0..n {
            if let Some(model) = self.models.fs_htc[k][c] {
                match model {
                    FsHtcModel::MultiRegimeBoilingTraceChf(m) => {
                        let probing = self.trace_probe.is_some();
                        let r = if probing { self.trace_chf_value_probed(k, o, c, m) } else { self.trace_chf_value(k, o, c, m) }
                            .map_err(|e| crate::error::AppBuilderError::Case {
                            case: crate::path::PathBuf::from("multiRegimeBoilingTRACECHF"),
                            msg: e,
                        })?;
                        self.htc_fs[k][c] = r.htc;
                        if let Some(d) = r.dmdt_w {
                            self.dmdt_w[c] = d;
                        }
                        self.chf_bool[c] = r.chf_bool;
                        self.wf_transition_boiling[c] = r.wf_tb;
                        self.t_leidenfrost[c] = r.t_leidenfrost;
                    }
                    FsHtcModel::MultiRegimeBoilingVapourTrace(m) => {
                        self.htc_fs[k][c] = self.vapour_trace_value(k, c, m);
                    }
                    _ => {
                        let probing = self.trace_probe.is_some();
                        let (htc, q_sc) = match model {
                            FsHtcModel::MultiRegimeBoilingTrace(m) if probing => self.trace_boiling_value_probed(k, o, c, m),
                            _ => self.fs_htc_value(k, o, c, model),
                        };
                        self.htc_fs[k][c] = htc;
                        if let Some(q) = q_sc {
                            // `dmdtW[celli] = iAact*qSCDmdti/mag(L)` (multiRegimeBoilingTRACE.C)
                            self.dmdt_w[c] = self.structure.ia_act[c] * q / self.latent_heat[c].abs();
                        }
                    }
                }
            }
        }
        for c in 0..n {
            self.htc_fs[k][c] *= self.contact[k][c];
        }
        Ok(())
    }

    /// `multiRegimeBoilingTRACECHF::value` for cell `c` of fluid `k` (gh:#1223, gh:#1224).
    ///
    /// # Errors
    /// A `lookUpTableCHF` query outside its table (upstream's `FatalError`).
    fn trace_chf_value(&self, k: usize, o: usize, c: usize, m: TraceChfModel) -> Result<TraceChfResult, String> {
        self.trace_chf_value_logged(k, o, c, m, None)
    }

    /// [`Self::trace_chf_value`] with the intermediates appended to `log` (gh:#1361 round 13 diagnostic).
    fn trace_chf_value_logged(
        &self,
        k: usize,
        o: usize,
        c: usize,
        m: TraceChfModel,
        log: Option<&core::cell::RefCell<Vec<(&'static str, f64)>>>,
    ) -> Result<TraceChfResult, String> {
        let push = |name: &'static str, v: f64| {
            if let Some(l) = log {
                l.borrow_mut().push((name, v));
            }
        };
        let fl = &self.fluids[k];
        let ot = &self.fluids[o];
        let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
        let tw = self.structure.t_wall[c];
        let tf = fl.t.c[c];
        let tsat = self.t_interface[c];
        let b = m.base;
        let fc_in = (
            ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
            Ratio::new::<ratio>(pr),
            ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
            Length::new::<meter>(fl.dh[c]),
            Some((temperature(tw), temperature(tf))),
        );
        let fc = if self.controls.earlier_nusselt_association {
            b.forced_convection.heat_transfer_coefficient_earlier_association(fc_in.0, fc_in.1, fc_in.2, fc_in.3, fc_in.4)
        } else {
            b.forced_convection.heat_transfer_coefficient(fc_in.0, fc_in.1, fc_in.2, fc_in.3, fc_in.4)
        }
        .get::<watt_per_square_meter_kelvin>();
        let f = b
            .flow_enhancement
            .value(FlowEnhancementInputs {
                inverse_lockhart_martinelli: Ratio::new::<ratio>(1.0 / fl.xlm[c]),
                liquid_void_fraction: Ratio::new::<ratio>(fl.normalized[c]),
                mixture_reynolds_number: ReynoldsNumber::new::<ratio>(10.0),
                single_phase_reynolds_number: ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
            })
            .get::<ratio>();
        let l_abs = self.latent_heat[c].abs();
        // the critical heat flux (only evaluated where upstream reaches it: above the onset of
        // nucleate boiling; the table can be out of range elsewhere without being asked)
        let q_chf_at = |p_c: f64| -> Result<Option<f64>, String> {
            match m.chf {
                None => Ok(None),
                Some(ChfSelection::Constant(v)) => Ok(Some(v)),
                Some(ChfSelection::Table(i)) => {
                    let f0 = &self.fluids[0];
                    let inp = LookUpTableChfInputs {
                        flow_quality: fl.quality[c],
                        p: p_c,
                        rho_liquid: fl.rho.c[c],
                        rho_vapour: ot.rho.c[c],
                        mag_u_liquid: fl.mag_u[c],
                        mag_u_vapour: ot.mag_u[c],
                        alpha_liquid: fl.normalized[c],
                        dh: self.structure.dh[c],
                        h_liquid_saturated: f0.thermo.he(p_c, tsat),
                        latent_heat: l_abs,
                        h_liquid: f0.he.c[c],
                    };
                    self.models.chf_tables[i].value(&inp).map(Some).map_err(|e| {
                        format!("cell {c}, p = {p_c} Pa, T_sat = {tsat} K, G = {}: {e}", inp.rho_liquid * inp.mag_u_liquid)
                    })
                }
            }
        };
        let inputs = TraceChfInputs {
            alpha_other: 1.0 - fl.normalized[c],
            t_wall: tw,
            t_fluid: tf,
            t_sat: tsat,
            p: self.p.c[c],
            area: self.structure.ia_act[c],
            latent_heat: l_abs,
            htc_2p_fc: fc * f,
            htc_film_condensation: None,
            has_onb_model: b.onset.is_some(),
            q_chf: None,
            roughness: m.roughness,
        };
        // `qCHFi` is evaluated only above the onset of nucleate boiling: decide that first with
        // the same inputs, then fill it in
        let t_onb = |htc: f64| -> f64 {
            match b.onset {
                Some(model) => model
                    .temperature(
                        uom::si::f64::HeatTransfer::new::<watt_per_square_meter_kelvin>(htc),
                        temperature(tf),
                        temperature(tsat),
                        mass_density(ot.rho.c[c]),
                        specific_energy(l_abs),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                    )
                    .get::<kelvin>(),
                None => tsat,
            }
        };
        let needs_chf = tw >= tsat && tw >= t_onb(inputs.htc_2p_fc);
        let inputs = TraceChfInputs { q_chf: if needs_chf { q_chf_at(self.p.c[c])? } else { None }, ..inputs };
        push("alphai", inputs.alpha_other);
        push("Tw", tw);
        push("Tf", tf);
        push("Tsat", tsat);
        push("htcFC", fc);
        push("F", f);
        push("htc2pFC", inputs.htc_2p_fc);
        push("qFC", inputs.htc_2p_fc * (tw - tf));
        push("Re", self.re_fs[k][c]);
        push("Pr", pr);
        push("kappa", fl.kappa.c[c]);
        push("Dh", self.structure.dh[c]);
        push("p", self.p.c[c]);
        push("L", self.latent_heat[c]);
        push("Area", self.structure.ia_act[c]);
        if needs_chf {
            push("TONB", t_onb(inputs.htc_2p_fc));
            push("qCHF", inputs.q_chf.unwrap_or(1e15));
            push(
                "TCHF",
                crate::genfoam::thermal_hydraulics::closures::heat_transfer::boiling_trace_chf::gorenflo_t_chf(
                    tsat,
                    inputs.p,
                    inputs.q_chf.unwrap_or(1e15),
                    inputs.roughness,
                ),
            );
        }
        let pool = |t_wall: f64| -> f64 {
            b.pool_boiling
                .heat_transfer_coefficient_from_delta_t(
                    uom::si::f64::TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(t_wall - tsat),
                    Pressure::new::<pascal>(self.p.c[c]),
                )
                .get::<watt_per_square_meter_kelvin>()
        };
        let scbf = b.sub_cooled_fraction.map(|model| {
            move |q: f64| -> f64 {
                model
                    .fraction(
                        uom::si::f64::HeatFluxDensity::new::<watt_per_square_meter>(q),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                        Length::new::<meter>(self.structure.dh[c]),
                        ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                        Ratio::new::<ratio>(pr),
                        temperature(tsat),
                        temperature(tf),
                    )
                    .get::<ratio>()
            }
        });
        let tlf = m.leidenfrost.map(|model| {
            move || {
                model
                    .temperature(Pressure::new::<pascal>(self.p.c[c]), temperature(tsat))
                    .get::<kelvin>()
            }
        });
        let annular = |t_wall: f64| -> (f64, f64) {
            match m.annular {
                None => (0.0, 0.0),
                Some(model) => model.value(&CachardLiquidInputs {
                    t_wall,
                    t_sat: tsat,
                    t_liquid: tf,
                    alpha_vapour: ot.normalized[c],
                    p: self.p.c[c],
                    rho_liquid: fl.rho.c[c],
                    rho_vapour: ot.rho.c[c],
                    kappa_vapour: ot.kappa.c[c],
                    mu_vapour: ot.mu.c[c],
                    dh_vapour: ot.dh[c],
                }),
            }
        };
        let r = multi_regime_boiling_trace_chf(&inputs, t_onb, pool, scbf, tlf, annular);
        push("ret", r.htc);
        push("chfBool", r.chf_bool);
        push("wfTB", r.wf_tb);
        push("TLF", r.t_leidenfrost);
        if let Some(d) = r.dmdt_w {
            push("dmdtW", d);
        }
        Ok(r)
    }

    /// [`Self::trace_chf_value`] recording its intermediates into [`Self::trace_probe`].
    fn trace_chf_value_probed(&mut self, k: usize, o: usize, c: usize, m: TraceChfModel) -> Result<TraceChfResult, String> {
        let log = core::cell::RefCell::new(Vec::new());
        let r = self.trace_chf_value_logged(k, o, c, m, Some(&log))?;
        let record = TraceProbeRecord {
            kind: "chf",
            time: self.time,
            outer_iteration: self.corr_in_step,
            cell: c,
            values: log.into_inner(),
        };
        if let Some(p) = self.trace_probe.as_mut() {
            p.push(record);
        }
        Ok(r)
    }

    /// `multiRegimeBoilingVapourTRACE::value` for cell `c` of the vapour fluid `k`: reads the
    /// fields `trace_chf_value` wrote for the liquid pair of this correction.
    fn vapour_trace_value(&self, k: usize, c: usize, m: CachardVapour) -> f64 {
        let fl = &self.fluids[k];
        let annular = |_t_wall: f64| -> f64 { m.value(fl.normalized[c], self.p.c[c], fl.kappa.c[c], fl.dh[c]) };
        multi_regime_boiling_vapour_trace(
            self.structure.t_wall[c],
            self.chf_bool[c],
            self.wf_transition_boiling[c],
            self.t_leidenfrost[c],
            annular,
        )
    }

    /// One cell of a fluid-structure htc model.
    ///
    /// Returns the htc and, for `multiRegimeBoilingTRACE` in its boiling branch, the sub-cooled
    /// boiling term `q_SC` the model turns into `dmdtW`.
    fn fs_htc_value(&self, k: usize, o: usize, c: usize, model: FsHtcModel) -> (f64, Option<f64>) {
        let fl = &self.fluids[k];
        let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
        let tw = self.structure.t_act[c];
        let tf = fl.t.c[c];
        let fc = |m: FsForcedConvectionHtc| {
            let (re, prn, kap, dh, tt) = (
                ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                Ratio::new::<ratio>(pr),
                ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                Length::new::<meter>(fl.dh[c]),
                Some((temperature(tw), temperature(tf))),
            );
            if self.controls.earlier_nusselt_association {
                m.heat_transfer_coefficient_earlier_association(re, prn, kap, dh, tt)
            } else {
                m.heat_transfer_coefficient(re, prn, kap, dh, tt)
            }
            .get::<watt_per_square_meter_kelvin>()
        };
        match model {
            FsHtcModel::Nusselt(m) => (fc(m), None),
            FsHtcModel::MultiRegimeBoilingTrace(m) => self.trace_boiling_value(k, o, c, m),
            FsHtcModel::MultiRegimeBoiling(m) => self.multi_regime_boiling_value(k, o, c, m),
            // evaluated by `correct_fs_pair`, which keeps their side-effect fields
            FsHtcModel::MultiRegimeBoilingTraceChf(_) | FsHtcModel::MultiRegimeBoilingVapourTrace(_) => (0.0, None),
            FsHtcModel::SuperpositionNucleateBoiling {
                forced_convection,
                pool_boiling,
                flow_enhancement,
                suppression,
            } => {
                let ot = &self.fluids[o];
                // COBRA-TF two-phase Reynolds (COBRA-TFFlowEnhancementFactor.C)
                let a = fl.normalized[c];
                let oa = ot.normalized[c];
                let g_mix = fl.u.c[c] * (a * fl.rho.c[c]) + ot.u.c[c] * (oa * ot.rho.c[c]);
                let re2 = (g_mix.mag() * self.structure.dh[c] / (a * fl.mu.c[c] + oa * ot.mu.c[c])).max(10.0);
                let f = flow_enhancement
                    .value(FlowEnhancementInputs {
                        inverse_lockhart_martinelli: Ratio::new::<ratio>(1.0 / fl.xlm[c]),
                        liquid_void_fraction: Ratio::new::<ratio>(a),
                        mixture_reynolds_number: ReynoldsNumber::new::<ratio>(re2),
                        single_phase_reynolds_number: ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                    })
                    .get::<ratio>();
                let s = suppression
                    .value(SuppressionInputs {
                        mixture_reynolds_number: ReynoldsNumber::new::<ratio>(re2),
                        t_fluid: temperature(tf),
                        t_wall: temperature(tw),
                        t_sat: temperature(self.t_interface[c]),
                    })
                    .get::<ratio>();
                let pb = pool_boiling
                    .heat_transfer_coefficient_from_delta_t(
                        uom::si::f64::TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(tw - tf),
                        Pressure::new::<pascal>(self.p.c[c]),
                    )
                    .get::<watt_per_square_meter_kelvin>();
                (fc(forced_convection) * f + pb * s, None)
            }
        }
    }

    /// plain `multiRegimeBoiling::value` (`heatFluxSuperposition true`) for cell `c` of fluid `k` (gh:#1802), translating
    /// `multiRegimeBoilingFSHeatTransferCoefficient.C` (`value`, `qNB`, `setDmdtW`). Returns the htc and, in the boiling branch,
    /// `q_SC` multiplied by the sign of the latent heat so that `iAact q/|L|` equals upstream's `iAact q/L`.
    fn multi_regime_boiling_value(&self, k: usize, o: usize, c: usize, m: MultiRegimeBoilingModel) -> (f64, Option<f64>) {
        let b = m.base;
        let fl = &self.fluids[k];
        let ot = &self.fluids[o];
        let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
        let (tw, tf, tsat) = (self.structure.t_wall[c], fl.t.c[c], self.t_interface[c]);
        let re = ReynoldsNumber::new::<ratio>(self.re_fs[k][c]);
        let fc = b
            .forced_convection
            .heat_transfer_coefficient(
                re,
                Ratio::new::<ratio>(pr),
                ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                Length::new::<meter>(fl.dh[c]),
                Some((temperature(tw), temperature(tf))),
            )
            .get::<watt_per_square_meter_kelvin>();
        let f = b
            .flow_enhancement
            .value(FlowEnhancementInputs {
                inverse_lockhart_martinelli: Ratio::new::<ratio>(1.0 / fl.xlm[c]),
                liquid_void_fraction: Ratio::new::<ratio>(fl.normalized[c]),
                mixture_reynolds_number: ReynoldsNumber::new::<ratio>(10.0),
                single_phase_reynolds_number: re,
            })
            .get::<ratio>();
        let htc_2p_fc = fc * f;
        if tw < tsat {
            return (htc_2p_fc, None);
        }
        let l_abs = self.latent_heat[c].abs();
        let t_onb = match b.onset {
            Some(model) => model
                .temperature(
                    uom::si::f64::HeatTransfer::new::<watt_per_square_meter_kelvin>(htc_2p_fc),
                    temperature(tf),
                    temperature(tsat),
                    mass_density(ot.rho.c[c]),
                    specific_energy(l_abs),
                    ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                )
                .get::<kelvin>(),
            None => tsat,
        };
        if tw < t_onb {
            return (htc_2p_fc, None);
        }
        let pool = |t_wall: f64| -> f64 {
            b.pool_boiling
                .heat_transfer_coefficient_from_delta_t(
                    uom::si::f64::TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(t_wall - tsat),
                    Pressure::new::<pascal>(self.p.c[c]),
                )
                .get::<watt_per_square_meter_kelvin>()
        };
        let q_fc = htc_2p_fc * (tw - tf);
        let q_pb = pool(tw) * (tw - tsat);
        let e = m.exponent;
        let q_nb = match m.suppression {
            Some(s) => {
                let a = fl.normalized[c];
                let oa = ot.normalized[c];
                let g_mix = fl.u.c[c] * (a * fl.rho.c[c]) + ot.u.c[c] * (oa * ot.rho.c[c]);
                let re2 = (g_mix.mag() * self.structure.dh[c] / (a * fl.mu.c[c] + oa * ot.mu.c[c])).max(10.0);
                let sv = s
                    .value(SuppressionInputs {
                        mixture_reynolds_number: ReynoldsNumber::new::<ratio>(re2),
                        t_fluid: temperature(tf),
                        t_wall: temperature(tw),
                        t_sat: temperature(tsat),
                    })
                    .get::<ratio>();
                (q_fc.r_powf(e) + (sv * q_pb).r_powf(e)).r_powf(1.0 / e)
            }
            None if b.onset.is_some() => {
                let q_bi = pool(t_onb) * (t_onb - tsat);
                (q_fc.r_powf(e) + (q_pb - q_bi).r_powf(e)).r_powf(1.0 / e)
            }
            None => (q_fc.r_powf(e) + q_pb.r_powf(e)).r_powf(1.0 / e),
        };
        // `setDmdtW` only with a TONB model (no sub-cooled boiling otherwise)
        let q_sc = b.onset.map(|_| {
            let frac = match b.sub_cooled_fraction {
                Some(model) => model
                    .fraction(
                        uom::si::f64::HeatFluxDensity::new::<watt_per_square_meter>(q_nb),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                        Length::new::<meter>(self.structure.dh[c]),
                        re,
                        Ratio::new::<ratio>(pr),
                        temperature(tsat),
                        temperature(tf),
                    )
                    .get::<ratio>(),
                None => 1.0,
            };
            frac * (q_nb - q_fc) * if self.latent_heat[c] < 0.0 { -1.0 } else { 1.0 }
        });
        let dt = tw - tf;
        let dt = if dt >= 0.0 { dt.max(1e-3) } else { (-dt).min(-1e-3) };
        (q_nb / dt, q_sc)
    }

    /// `multiRegimeBoilingTRACE::value` for cell `c` of fluid `k` (gh:#1222). The wall temperature is
    /// the `Twall` field of the structure (zero until its first `correct`), the saturation
    /// temperature `T.interface`, the pool-boiling model the `Twall`/`T.interface` form.
    fn trace_boiling_value(&self, k: usize, o: usize, c: usize, m: TraceBoilingModel) -> (f64, Option<f64>) {
        self.trace_boiling_value_logged(k, o, c, m, None)
    }

    /// [`Self::trace_boiling_value`] with the intermediates appended to `log` (a diagnostic; the arithmetic is the
    /// same code, so the record cannot drift from the value).
    fn trace_boiling_value_logged(
        &self,
        k: usize,
        o: usize,
        c: usize,
        m: TraceBoilingModel,
        log: Option<&core::cell::RefCell<Vec<(&'static str, f64)>>>,
    ) -> (f64, Option<f64>) {
        let push = |name: &'static str, v: f64| {
            if let Some(l) = log {
                l.borrow_mut().push((name, v));
            }
        };
        let fl = &self.fluids[k];
        let ot = &self.fluids[o];
        let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
        let tw = self.structure.t_wall[c];
        let tf = fl.t.c[c];
        let tsat = self.t_interface[c];
        let fc = if self.controls.earlier_nusselt_association {
            m.forced_convection.heat_transfer_coefficient_earlier_association(
                ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                Ratio::new::<ratio>(pr),
                ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                Length::new::<meter>(fl.dh[c]),
                Some((temperature(tw), temperature(tf))),
            )
        } else {
            m.forced_convection.heat_transfer_coefficient(
                ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                Ratio::new::<ratio>(pr),
                ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                Length::new::<meter>(fl.dh[c]),
                Some((temperature(tw), temperature(tf))),
            )
        }
        .get::<watt_per_square_meter_kelvin>();
        // flow enhancement (Chen needs XLM, RezkallahSims the liquid fraction; COBRA-TF is not
        // reachable from this reader)
        let f = m
            .flow_enhancement
            .value(FlowEnhancementInputs {
                inverse_lockhart_martinelli: Ratio::new::<ratio>(1.0 / fl.xlm[c]),
                liquid_void_fraction: Ratio::new::<ratio>(fl.normalized[c]),
                mixture_reynolds_number: ReynoldsNumber::new::<ratio>(10.0),
                single_phase_reynolds_number: ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
            })
            .get::<ratio>();
        let htc_2p_fc = fc * f;
        push("alphai", 1.0 - fl.normalized[c]);
        push("Tw", tw);
        push("Tf", tf);
        push("Tsat", tsat);
        push("htcFC", fc);
        push("F", f);
        push("htc2pFC", htc_2p_fc);
        push("qFC", htc_2p_fc * (tw - tf));
        push("Re", self.re_fs[k][c]);
        push("Pr", pr);
        push("kappa", fl.kappa.c[c]);
        push("Dh", self.structure.dh[c]);
        push("p", self.p.c[c]);
        push("L", self.latent_heat[c]);
        push("iAact", self.structure.ia_act[c]);
        let inputs = TraceBoilingInputs {
            alpha_other: 1.0 - fl.normalized[c],
            t_wall: tw,
            t_fluid: tf,
            t_sat: tsat,
            htc_2p_fc,
            htc_film_condensation: None,
            has_onb_model: m.onset.is_some(),
        };
        let l_abs = self.latent_heat[c].abs();
        let t_onb = |htc: f64| -> f64 {
            match m.onset {
                Some(model) => model
                    .temperature(
                        uom::si::f64::HeatTransfer::new::<watt_per_square_meter_kelvin>(htc),
                        temperature(tf),
                        temperature(tsat),
                        mass_density(ot.rho.c[c]),
                        specific_energy(l_abs),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                    )
                    .get::<kelvin>(),
                None => tsat,
            }
        };
        let pool = |t_wall: f64| -> f64 {
            m.pool_boiling
                .heat_transfer_coefficient_from_delta_t(
                    uom::si::f64::TemperatureInterval::new::<uom::si::temperature_interval::kelvin>(t_wall - tsat),
                    Pressure::new::<pascal>(self.p.c[c]),
                )
                .get::<watt_per_square_meter_kelvin>()
        };
        let scbf = m.sub_cooled_fraction.map(|model| {
            move |q: f64| -> f64 {
                model
                    .fraction(
                        uom::si::f64::HeatFluxDensity::new::<watt_per_square_meter>(q),
                        ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                        Length::new::<meter>(self.structure.dh[c]),
                        ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                        Ratio::new::<ratio>(pr),
                        temperature(tsat),
                        temperature(tf),
                    )
                    .get::<ratio>()
            }
        });
        let pool_calls = core::cell::Cell::new(0_usize);
        let t_onb_logged = |htc: f64| -> f64 {
            let v = t_onb(htc);
            push("TONB", v);
            push("rhoOther", ot.rho.c[c]);
            push("absL", l_abs);
            v
        };
        let pool_logged = |t_wall: f64| -> f64 {
            let v = pool(t_wall);
            let i = pool_calls.get();
            pool_calls.set(i + 1);
            push(if i == 0 { "hPBONB" } else { "hPB" }, v);
            v
        };
        let scbf_logged = scbf.map(|fr| {
            move |q: f64| -> f64 {
                let v = fr(q);
                push("f", v);
                v
            }
        });
        let r = multi_regime_boiling_trace(&inputs, t_onb_logged, pool_logged, scbf_logged);
        push("ret", r.htc);
        if let Some(q) = r.q_subcooled_dmdt {
            push("qSCDmdt", q);
            push("dmdtW", self.structure.ia_act[c] * q / self.latent_heat[c].abs());
        }
        (r.htc, r.q_subcooled_dmdt)
    }

    /// One cell of `multiRegimeBoilingTRACE` with its intermediates recorded into [`Self::trace_probe`]
    /// (gh:#1361 round 13); the value returned is exactly [`Self::trace_boiling_value`]'s.
    fn trace_boiling_value_probed(&mut self, k: usize, o: usize, c: usize, m: TraceBoilingModel) -> (f64, Option<f64>) {
        let log = core::cell::RefCell::new(Vec::new());
        let r = self.trace_boiling_value_logged(k, o, c, m, Some(&log));
        let values = log.into_inner();
        let record = TraceProbeRecord { kind: "trace", time: self.time, outer_iteration: self.corr_in_step, cell: c, values };
        if let Some(p) = self.trace_probe.as_mut() {
            p.push(record);
        }
        r
    }

    /// `twoPhaseDragFactor::correct` (twoPhaseDragFactor.C:96-118) with
    /// `twoPhaseDragMultiplierModel::value`.
    fn correct_two_phase_drag_factor(&mut self) {
        let Some((model, mk, max_phi2)) = self.models.two_phase_multiplier else {
            return;
        };
        let n = self.geom.n_cells();
        for c in 0..n {
            let fm = &self.fluids[mk];
            let phi2 = model
                .multiplier(Ratio::new::<ratio>(fm.normalized[c]), Ratio::new::<ratio>(fm.xlm[c]))
                .get::<ratio>();
            let kd_tot_u = phi2.min(max_phi2)
                * self.kd_fs[mk][c]
                * fm.normalized[c].max(1e-9).r_powi(2)
                * fm.mag_u[c];
            for k in 0..2 {
                self.kd_fs[k][c] = self.contact[k][c] * kd_tot_u / self.fluids[k].mag_u[c].max(1e-9);
            }
        }
    }

    // ───────────────────────── fluid mechanics ─────────────────────────

    /// `twoPhase::correctFluidMechanics`: alpha, U and p equations, then the
    /// continuity errors. Returns the first PISO `p_rgh` initial residual and
    /// the number of PISO correctors run.
    fn correct_fluid_mechanics(&mut self) -> Result<(f64, usize), crate::error::AppBuilderError> {
        self.correct_alpha();
        self.correct_turbulence();
        let u_eqns = [self.correct_u_eqn(0), self.correct_u_eqn(1)];
        let r = self.correct_pressure(&u_eqns);
        self.correct_cont_errs();
        Ok(r)
    }

    /// `solveAlphaImplicitUpwind.H` for `solvePhase`, the other fluid by
    /// continuity, then `correctContErrs` and the normalised fractions
    /// (`alphaEqns_2p.H`).
    fn correct_alpha(&mut self) {
        if let AlphaSolver::Mules(mc) = self.controls.alpha.clone() {
            self.correct_alpha_mules(&mc);
            self.correct_cont_errs();
            self.update_normalized();
            return;
        }
        let g = self.geom.clone();
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let s = self.controls.solve_phase;
        let o = 1 - s;
        let min_alpha_sp = self.controls.min_alpha_sp.max(1e-69);
        let mut sp = vec![0.0; n];
        let mut su = vec![0.0; n];
        for c in 0..n {
            let a = self.fluids[s].alpha.c[c];
            let d = self.fluids[s].dgdt[c];
            if a >= min_alpha_sp && d < 0.0 {
                sp[c] += d / a;
            } else {
                su[c] += d;
            }
        }
        let fl = &self.fluids[s];
        let phis = [self.fluids[0].phi.clone(), self.fluids[1].phi.clone()];
        let phis_ref = [&phis[0][..], &phis[1][..]];
        let mut eqn = Eqn::new(&g);
        if self.controls.alpha_solver_openfoam {
            // Assembled in the order OpenFOAM does, so the matrix is bit-identical:
            // `EulerDdtScheme::fvmDdt` (diag `rDeltaT*V`, source `rDeltaT*psi0*V`), the
            // convection matrix on its own (`gaussConvectionScheme::fvmDiv`: `lower =
            // -w phi`, `upper = lower + phi`, `negSumDiag`), their sum, then
            // `fvm::Sp`/`Su`, and the boundary diagonal LAST (`fvMatrix::addBoundaryDiag`
            // at solve time).
            let r_delta_t = 1.0 / self.dt;
            let mut div_diag = vec![0.0; n];
            for f in 0..ni {
                let (ow, nb) = (g.mesh.owner[f], g.mesh.neighbour[f]);
                let fl_f = fl.phi[f];
                let w = if self.phi[f] >= 0.0 { 1.0 } else { 0.0 };
                let lower = -w * fl_f;
                let upper = lower + fl_f;
                eqn.ldu.lower[f] = lower;
                eqn.ldu.upper[f] = upper;
                div_diag[ow] -= lower;
                div_diag[nb] -= upper;
            }
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                eqn.ldu.diag[c] = r_delta_t * v + div_diag[c];
                eqn.source[c] = r_delta_t * fl.alpha_old[c] * v;
            }
            let neg_sp: Vec<f64> = sp.iter().map(|v| -v).collect();
            eqn.sp(&g, &neg_sp);
            eqn.su(&g, &su);
            for f in ni..nf {
                if g.active[f] {
                    let (ic, bc) = fl.alpha.convection_coeffs(&g, f, phis_ref);
                    eqn.ldu.diag[g.mesh.owner[f]] += fl.phi[f] * ic;
                    if bc != 0.0 {
                        eqn.source[g.mesh.owner[f]] += -fl.phi[f] * bc;
                    }
                }
            }
        } else {
            let ones = vec![1.0; n];
            eqn.ddt(&g, &ones, &ones, &fl.alpha_old, self.dt);
            // Gauss upwind with weights by the TOTAL flux phi_, applied to phi_s.
            for f in 0..ni {
                let (ow, nb) = (g.mesh.owner[f], g.mesh.neighbour[f]);
                let fl_f = fl.phi[f];
                if self.phi[f] >= 0.0 {
                    eqn.ldu.diag[ow] += fl_f;
                    eqn.ldu.lower[f] -= fl_f;
                } else {
                    eqn.ldu.upper[f] += fl_f;
                    eqn.ldu.diag[nb] -= fl_f;
                }
            }
            for f in ni..nf {
                if g.active[f] {
                    let (ic, bc) = fl.alpha.convection_coeffs(&g, f, phis_ref);
                    eqn.ldu.diag[g.mesh.owner[f]] += fl.phi[f] * ic;
                    if bc != 0.0 {
                        eqn.source[g.mesh.owner[f]] += -fl.phi[f] * bc;
                    }
                }
            }
            let neg_sp: Vec<f64> = sp.iter().map(|v| -v).collect();
            eqn.sp(&g, &neg_sp);
            eqn.su(&g, &su);
        }
        let prev = fl.alpha.c.clone();
        let mut x = prev.clone();
        if self.controls.alpha_solver_openfoam {
            let _ = pbicgstab::pbicgstab_diagonal(
                &eqn.ldu,
                &eqn.source,
                &mut x,
                self.controls.alpha_solver_tolerance,
                self.controls.alpha_solver_rel_tol,
                pbicgstab::DEFAULT_MAX_ITER,
                0,
            );
        } else {
            let _ = solve_krylov(&eqn, &mut x, 0.0);
        }
        let fa = self.controls.relax_alpha;
        let fl = &mut self.fluids[s];
        for c in 0..n {
            let relaxed = prev[c] + fa * (x[c] - prev[c]);
            let moving = 1.0 - self.structure.alpha.c[c];
            fl.alpha.c[c] = relaxed.max(0.0).min(moving);
        }
        fl.alpha.correct_bc_with_flux(&g, phis_ref);
        // otherAlpha = 1 - alphas - solveAlpha (cells and faces)
        for c in 0..n {
            self.fluids[o].alpha.c[c] = 1.0 - self.structure.alpha.c[c] - self.fluids[s].alpha.c[c];
        }
        for i in 0..(nf - ni) {
            self.fluids[o].alpha.b[i] = 1.0 - self.structure.alpha.b[i] - self.fluids[s].alpha.b[i];
        }
        // fluxes
        let af = fv::interp_upwind(&g, &self.fluids[s].alpha, &self.phi);
        let mut ap_s: Vec<f64> = (0..nf).map(|f| af[f] * self.fluids[s].phi[f]).collect();
        let mut ap_o: Vec<f64> = (0..nf).map(|f| self.phi[f] - ap_s[f]).collect();
        for f in ni..nf {
            if g.active[f] {
                ap_s[f] = self.fluids[s].phi[f] * self.fluids[s].alpha.b[f - ni];
                ap_o[f] = self.fluids[o].phi[f] * self.fluids[o].alpha.b[f - ni];
            }
        }
        for (k, ap) in [(s, ap_s), (o, ap_o)] {
            let rf = fv::interp_upwind(&g, &self.fluids[k].rho, &self.phi);
            self.fluids[k].alpha_rho_phi = (0..nf).map(|f| rf[f] * ap[f]).collect();
            self.fluids[k].alpha_phi = ap;
        }
        self.correct_cont_errs();
        self.update_normalized();
    }

    /// `twoPhase::correctContErrs`.
    fn correct_cont_errs(&mut self) {
        let n = self.geom.n_cells();
        for k in 0..2 {
            let dv = fv::div(&self.geom, &self.fluids[k].alpha_rho_phi);
            let sign = if k == 0 { 1.0 } else { -1.0 };
            let has_pc = self.models.phase_change.is_some();
            let fl = &mut self.fluids[k];
            for c in 0..n {
                fl.cont_err[c] = (fl.alpha.c[c] * fl.rho.c[c] - fl.alpha_old[c] * fl.rho_old[c]) / self.dt
                    + dv[c]
                    + if has_pc { sign * self.dmdt[c] } else { 0.0 };
            }
        }
    }

    /// `UEqns_2p.H` for fluid `k` in the face-centred, implicit-partial-
    /// elimination configuration (no `fvm::ddt`, no `Sp(Kd12)`), laminar or RAS.
    ///
    /// Assembled as `fvMatrix` holds it (gh:#1909 B4, README round 58): `UEqn = div + ((-laplacian) - fvc::div(dev2)) + Sp((1/3) tr Kd1s)
    /// + (dev(Kd1s) & U) == fvOptions`, then `+= virtual-mass force`, then `+= SuSp(-contErr)`; each `fvm` term is its own matrix with its own
    /// `negSumDiag` and `fvMatrix::operator+=` adds it whole, the boundary coefficients stay per face in `internalCoeffs`/`boundaryCoeffs`
    /// (`A()` takes `cmptAv` of the first, `H()` the component of both), and there is no `relax` (`UEqns_2p.H:191,194` comment it out).
    fn correct_u_eqn(&self, k: usize) -> VecEqn {
        let g = &self.geom;
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let fl = &self.fluids[k];
        let ot = &self.fluids[1 - k];
        let mut e = VecEqn::new(g);
        let phis = [&self.fluids[0].phi[..], &self.fluids[1].phi[..]];
        // fvm::div(alphaRhoPhi, U)  (Gauss upwind). The boundary part follows the velocity patch types
        // (gh:#1801): `zeroGradient` puts `phi` on the diagonal, `fixedValue` puts `-phi U_b` in the source.
        e.cc.eqn.div_upwind_internal(g, &fl.alpha_rho_phi);
        e.boundary_convection(g, &fl.alpha_rho_phi, &fl.u, phis);
        // alpha rho nuEff, `nuEff = nut + mu/rho` (RAS, gh:#1802) or `mu/rho` (laminar): `(alpha rho) nuEff`, in upstream's order
        let mut arn = Scalar::uniform(g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
        match &fl.turbulence {
            Some(t) => {
                for c in 0..n {
                    arn.c[c] = fl.alpha.c[c] * fl.rho.c[c] * (t.nut.c[c] + fl.mu.c[c] / fl.rho.c[c]);
                }
                for i in 0..(nf - ni) {
                    arn.b[i] = fl.alpha.b[i] * fl.rho.b[i] * (t.nut.b[i] + fl.mu.b[i] / fl.rho.b[i]);
                }
            }
            None => {
                for c in 0..n {
                    arn.c[c] = fl.alpha.c[c] * fl.rho.c[c] * (fl.mu.c[c] / fl.rho.c[c]);
                }
                for i in 0..(nf - ni) {
                    arn.b[i] = fl.alpha.b[i] * fl.rho.b[i] * (fl.mu.b[i] / fl.rho.b[i]);
                }
            }
        }
        let gamma = fv::interp_linear_openfoam(g, &arn);
        // (-fvm::laplacian(alpha rho nuEff, U)) - fvc::div(alpha rho nuEff dev2(T(grad U))): one matrix, added whole
        let mut lap = VecEqn::new(g);
        lap.cc.eqn.neg_laplacian_internal(g, &gamma);
        lap.boundary_laplacian(g, &gamma, &fl.u, phis);
        let dev2 = dev2_div_openfoam(g, &fl.u, &arn);
        lap.cc.sub_field(g, &dev2);
        e.add(&lap);
        // + fvm::Sp((1/3) tr(Kd1s), U) + (dev(Kd1s) & U): `Kd = diag(v, v, v)`, `tr = v + v + v`, `dev(Kd) = Kd - (1/3) tr` is an ulp of `v`
        let mut sp_c = vec![0.0; n];
        let mut dev_u = vec![Vector3::ZERO; n];
        for c in 0..n {
            let v = self.kd_fs[k][c];
            let tr = v + v + v;
            sp_c[c] = (1.0 / 3.0) * tr;
            let dv = v - (1.0 / 3.0) * tr;
            let u = fl.u.c[c];
            dev_u[c] = Vector3::new(dv * u.x, dv * u.y, dv * u.z);
        }
        let mut sp = VecEqn::new(g);
        sp.cc.eqn.sp(g, &sp_c);
        e.add(&sp);
        e.cc.add_field(g, &dev_u);
        // virtual mass force `alphaRhoVm * (fvm::ddt(U1) + fvm::div(phi1, U1) + fvm::SuSp(-div(phi1), U1) - (...))`, `UEqn += force`
        if self.models.virtual_mass.is_some() {
            let mut vm = VecEqn::new(g);
            let ones = vec![1.0; n];
            for comp in 0..3 {
                let old: Vec<f64> = fl.u_old.iter().map(|u| cmp(*u, comp)).collect();
                for c in 0..n {
                    vm.cc.src[comp][c] += g.mesh.cell_volumes[c] / self.dt * old[c];
                }
            }
            for c in 0..n {
                vm.cc.eqn.ldu.diag[c] += g.mesh.cell_volumes[c] / self.dt * ones[c];
            }
            let mut vdiv = VecEqn::new(g);
            vdiv.cc.eqn.div_upwind_internal(g, &fl.phi);
            vdiv.boundary_convection(g, &fl.phi, &fl.u, phis);
            vm.add(&vdiv);
            let div_phi = fv::div(g, &fl.phi);
            let neg_div: Vec<f64> = div_phi.iter().map(|v| -v).collect();
            vm.su_sp(g, &neg_div, &fl.u.c);
            // explicit: -(ddt(U2) + div(phi2, U2) - div(phi2) U2) on the LHS
            let div_o = fv::div(g, &ot.phi);
            let conv_o = div_upwind_explicit_vec(g, &ot.phi, &ot.u);
            for c in 0..n {
                let ddt_o = (ot.u.c[c] - ot.u_old[c]) * (1.0 / self.dt);
                let r = ddt_o + conv_o[c] - ot.u.c[c] * div_o[c];
                let v = g.mesh.cell_volumes[c];
                vm.cc.src[0][c] += v * r.x;
                vm.cc.src[1][c] += v * r.y;
                vm.cc.src[2][c] += v * r.z;
            }
            vm.scale_rows(g, &self.alpha_rho_vm);
            e.add(&vm);
        }
        // contErr compensation (SuSp, c = 1)
        let ce: Vec<f64> = fl.cont_err.iter().map(|v| -v).collect();
        e.su_sp(g, &ce, &fl.u.c);
        e
    }

    /// `pEqnf_2p.H`, implicit partial elimination, with
    /// `refsAndControls_2p.H`, `calcPhiS_2p.H`, `calcComp_2p.H`,
    /// `solveP_rgh_2p.H`, `reconstructU_2p.H`, `updatePP_rghRho_2p.H`.
    /// Returns the first PISO initial residual and the PISO count.
    fn correct_pressure(&mut self, u_eqns: &[VecEqn; 2]) -> (f64, usize) {
        let g = self.geom.clone();
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let dt = self.dt;
        // refsAndControls_2p.H: alphaf by upwind on phi_ for implicitUpwind, `fvc::interpolate` (linear) otherwise
        // (MULES; gh:#1361 round 7).
        let af: [Vec<f64>; 2] = if matches!(self.controls.alpha, AlphaSolver::ImplicitUpwind) {
            [
                fv::interp_upwind(&g, &self.fluids[0].alpha, &self.phi),
                fv::interp_upwind(&g, &self.fluids[1].alpha, &self.phi),
            ]
        } else {
            [fv::interp_linear_openfoam(&g, &self.fluids[0].alpha), fv::interp_linear_openfoam(&g, &self.fluids[1].alpha)]
        };
        let moving_af: Vec<f64> = (0..nf).map(|f| af[0][f] + af[1][f]).collect();
        // refsAndControls_2p.H: `phi1.storePrevIter(); phi2.storePrevIter();`
        let phi_prev_iter: [Vec<f64>; 2] = [self.fluids[0].phi.clone(), self.fluids[1].phi.clone()];
        // time-derivative coefficients
        let ar_by_dt = |fl: &Phase, alpha: &[f64], rho: &[f64], ab: &[f64], rb: &[f64]| {
            let mut s = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                s.c[c] = alpha[c].max(fl.residual_alpha) * rho[c];
            }
            for i in 0..(nf - ni) {
                s.b[i] = ab[i].max(fl.residual_alpha) * rb[i];
            }
            let v = fv::interp_linear_openfoam(&g, &s);
            v.into_iter().map(|x| x / dt).collect::<Vec<f64>>()
        };
        let mut arf = Vec::new();
        let mut arf0 = Vec::new();
        for k in 0..2 {
            let fl = &self.fluids[k];
            arf.push(ar_by_dt(fl, &fl.alpha.c, &fl.rho.c, &fl.alpha.b, &fl.rho.b));
            // old-time boundary values: OpenFOAM's `oldTime()` stores the whole
            // field, boundary included (gh:#1361 round 3: the old `rho` patch of the
            // vapour is evaluated at the old patch pressure, not the owner's value).
            let ab0: Vec<f64> = fl.alpha_old_b.clone();
            let rb0: Vec<f64> = fl.rho_old_b.clone();
            arf0.push(ar_by_dt(fl, &fl.alpha_old, &fl.rho_old, &ab0, &rb0));
        }
        let mut kd12 = Scalar::uniform(&g, 0.0, g.default_bcs());
        kd12.c = self.kd_ff.clone();
        kd12.correct_bc(&g);
        let kdf = fv::interp_linear_openfoam(&g, &kd12);
        let afk: Vec<Vec<f64>> = (0..2)
            .map(|k| {
                let mut a = Scalar::uniform(&g, 0.0, g.default_bcs());
                a.c = u_eqns[k].a(&g);
                a.correct_bc(&g);
                let ai = fv::interp_linear_openfoam(&g, &a);
                (0..nf).map(|f| ai[f] + arf[k][f]).collect()
            })
            .collect();
        let kd_by_af: Vec<Vec<f64>> =
            (0..2).map(|k| (0..nf).map(|f| kdf[f] / afk[k][f]).collect()).collect();
        let rauf: Vec<Vec<f64>> = (0..2)
            .map(|k| {
                let o = 1 - k;
                (0..nf)
                    .map(|f| 1.0 / (afk[k][f] + kdf[f] * (1.0 + afk[k][f] / afk[o][f])))
                    .collect()
            })
            .collect();
        let star: Vec<Vec<f64>> = (0..2)
            .map(|k| {
                let o = 1 - k;
                (0..nf)
                    .map(|f| (af[k][f] + kd_by_af[o][f] * (af[0][f] + af[1][f])) * rauf[k][f])
                    .collect()
            })
            .collect();
        let mut first_res = 0.0;
        let mut n_piso = 0;
        for corr in 1..=self.controls.n_corr.max(1) {
            n_piso = corr;
            self.update_mixture_density();
            for c in 0..n {
                self.p_rgh.c[c] = self.p.c[c] - self.rho_mix.c[c] * self.gh.c[c];
            }
            // correctBoundaryFlux_2p.H: the flux of a `fixedValue`/`slip` velocity patch is `Sf . U_b`
            for k in 0..2 {
                for f in ni..nf {
                    if g.active[f] && self.fluids[k].u.flux_is_fixed(&g, f) {
                        self.fluids[k].phi[f] = g.mesh.face_area_vectors[f].dot(self.fluids[k].u.b[f - ni]);
                    }
                }
            }
            // calcPhiS_2p.H
            let mut rmix = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            rmix.c = self.rho_mix.c.clone();
            rmix.b = self.rho_mix.b.clone();
            let sng = fv::sn_grad(&g, &rmix);
            let r1f = fv::interp_linear_openfoam(&g, &self.fluids[0].rho);
            let r2f = fv::interp_linear_openfoam(&g, &self.fluids[1].rho);
            let mut phi_s = [vec![0.0; nf], vec![0.0; nf]];
            for f in 0..nf {
                if !g.active[f] {
                    continue;
                }
                let gh_sn = self.ghf[f] * sng[f] * g.mesh.face_areas[f];
                let drho = r1f[f] - r2f[f];
                let g_sf = self.g.dot(g.mesh.face_area_vectors[f]);
                phi_s[0][f] = -af[0][f] * (gh_sn - af[1][f] / moving_af[f] * drho * g_sf);
                phi_s[1][f] = -af[1][f] * (gh_sn + af[0][f] / moving_af[f] * drho * g_sf);
                for k in 0..2 {
                    phi_s[k][f] += arf0[k][f] * self.fluids[k].phi_old[f];
                }
            }
            // phiH_k = flux(H_k) + phiS_k
            let mut phi_h = [vec![0.0; nf], vec![0.0; nf]];
            for k in 0..2 {
                let h = u_eqns[k].h(&g, &self.fluids[k].u);
                let mut hv = if self.controls.h_symmetry_constraint {
                    Vector::uniform_with_constraint_patches(&g, Vector3::ZERO)
                } else {
                    Vector::uniform(&g, Vector3::ZERO)
                };
                hv.c = h;
                hv.correct_bc(&g);
                let fh = fv::flux_openfoam(&g, &hv);
                for f in 0..nf {
                    phi_h[k][f] = fh[f] + phi_s[k][f];
                }
            }
            let mut phi_hbya = [vec![0.0; nf], vec![0.0; nf]];
            for k in 0..2 {
                let o = 1 - k;
                for f in 0..nf {
                    phi_hbya[k][f] =
                        rauf[k][f] * ((1.0 + kd_by_af[o][f]) * phi_h[k][f] + kd_by_af[o][f] * phi_h[o][f]);
                }
            }
            let phi_hbya_mix: Vec<f64> =
                (0..nf).map(|f| af[0][f] * phi_hbya[0][f] + af[1][f] * phi_hbya[1][f]).collect();
            let rau_mix: Vec<f64> = (0..nf)
                .map(|f| (af[0][f] * star[0][f] + af[1][f] * star[1][f]).abs())
                .collect();
            // setSnGradP_rgh_2p.H: `fixedFluxPressure` gradient = (phiHbyA_b - phib)/(magSf rAUf_b),
            // `phib = alphaf1 phi1 + alphaf2 phi2` on the patch
            for (pi, patch) in g.mesh.patches.iter().enumerate() {
                if let Bc::FixedGradient(gr) = &mut self.p_rgh.bc[pi] {
                    for kk in 0..patch.size {
                        let f = patch.start + kk;
                        let phib = af[0][f] * self.fluids[0].phi[f] + af[1][f] * self.fluids[1].phi[f];
                        gr[kk] = (phi_hbya_mix[f] - phib) / (g.mesh.face_areas[f] * rau_mix[f]);
                    }
                }
            }
            // calcComp_2p.H
            let p_rgh_0 = self.p_rgh.c.clone();
            let rdt = 1.0 / dt;
            let mut comp_expl = [vec![0.0; n], vec![0.0; n]];
            let mut comp_pc = [vec![0.0; n], vec![0.0; n]];
            let mut comp_c = [vec![0.0; n], vec![0.0; n]];
            for k in 0..2 {
                let fl = &self.fluids[k];
                let d_arp = fv::div(&g, &fl.alpha_rho_phi);
                let d_ap = fv::div(&g, &fl.alpha_phi);
                for c in 0..n {
                    // `fvc::ddt(alpha, rho) = rDeltaT (alpha rho - alpha0 rho0)`, `fvc::ddt(alpha) = rDeltaT (alpha - alpha0)` (EulerDdtScheme)
                    let ddt_ar = rdt * (fl.alpha.c[c] * fl.rho.c[c] - fl.alpha_old[c] * fl.rho_old[c]);
                    let ddt_a = rdt * (fl.alpha.c[c] - fl.alpha_old[c]);
                    comp_expl[k][c] = (ddt_ar + d_arp[c] - (ddt_a + d_ap[c]) * fl.rho.c[c]) / fl.rho.c[c];
                    if self.models.phase_change.is_some() {
                        comp_pc[k][c] = self.dmdt[c] / fl.rho.c[c];
                    }
                    comp_c[k][c] = fl.alpha.c[c] * fl.psi[c] / fl.rho.c[c];
                }
            }
            // `pEqn = pEqnComp1 + pEqnComp2 + pEqnIncomp` as `fvMatrix` holds it (gh:#1909 B4; the two-phase twin of `one_phase_fc`'s round 46
            // assembly): `pEqnCompK = (field)/rho + (alpha psi/rho) p_rghCorr (+/- dmdt/rho)`, the field subtracted from ITS OWN matrix's source
            // (`volField + matrix`: `source -= V field`); `pEqnIncomp = fvc::div(phiHbyA) - fvm::laplacian(rAUf, p_rgh)` keeps its boundary
            // coefficients in `internalCoeffs_`/`boundaryCoeffs_` (`pic`, `pbc`), which join the diagonal and the source only in
            // `solveSegregated`; the three matrices are added whole, `((comp1 + comp2) + incomp)`.
            let mut inc = Eqn::new(&g);
            inc.neg_laplacian_internal(&g, &rau_mix);
            let (pic, pbc) = Eqn::neg_laplacian_boundary_coeffs(&g, &rau_mix, &self.p_rgh);
            let dphi = fv::div(&g, &phi_hbya_mix);
            let mut peqn = inc.clone();
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                let mut d = [0.0; 2];
                let mut s = [0.0; 2];
                for k in 0..2 {
                    d[k] = rdt * v * comp_c[k][c];
                    s[k] = 0.0 + rdt * v * p_rgh_0[c] * comp_c[k][c];
                    s[k] -= v * comp_expl[k][c];
                }
                if self.models.phase_change.is_some() {
                    s[0] -= v * comp_pc[0][c];
                    s[1] += v * comp_pc[1][c];
                }
                peqn.ldu.diag[c] = (d[0] + d[1]) + inc.ldu.diag[c];
                peqn.source[c] = (s[0] + s[1]) + (-(v * dphi[c]));
            }
            // `pEqn.relax()`: `relaxationFactors.equations` selects `p_rgh` (`".*" 1` in the shipped cases): `fvMatrix::relax(1)` is not the identity
            if let Some(a) = self.controls.relax_p_rgh_equation {
                peqn.relax_scalar(&g, a, &pic, &p_rgh_0);
            }
            let folded = peqn.folded(&g, &pic, &pbc);
            let mut x = self.p_rgh.c.clone();
            let res = if let Some(ctl) = self.controls.p_rgh_gamg {
                // upstream's GAMGSolver (gh:#1936, README round 72)
                self.gamg.solve(&g, &folded.ldu, &folded.source, &mut x, &ctl).initial_residual
            } else if let Some(pc) = self.controls.p_rgh_pcg {
                // upstream: `DICPCG: Solving for p_rgh` (the case's `solver PCG; preconditioner DIC;`)
                pcg::pcg_dic(&folded.ldu, &folded.source, &mut x, pc.tolerance, pc.rel_tol, pc.max_iter, pc.min_iter)
                    .initial_residual
            } else {
                solve_krylov(&folded, &mut x, self.controls.p_rgh_tolerance)
            };
            if corr == 1 {
                first_res = res;
            }
            self.p_rgh.c = x;
            self.p_rgh.correct_bc(&g);
            // fluxes: `pEqnIncomp.flux()` (`fvMatrix::flux()`, `internalCoeffs psi_P - boundaryCoeffs` on a boundary face)
            let p_flux = inc.flux_of_coeffs(&g, &self.p_rgh.c, &pic, &pbc);
            for f in 0..nf {
                if !g.active[f] {
                    continue;
                }
                self.phi[f] = phi_hbya_mix[f] + p_flux[f];
                let msf = p_flux[f] / rau_mix[f];
                for k in 0..2 {
                    // `phi1 = ...` does not assign a `fixedValue` flux patch (`fixedValueFvsPatchField::operator=` is empty)
                    if f >= ni && self.fluids[k].u.flux_is_fixed(&g, f) {
                        continue;
                    }
                    self.fluids[k].phi[f] = phi_hbya[k][f] + star[k][f] * msf;
                }
            }
            for k in 0..2 {
                let sign = if k == 0 { 1.0 } else { -1.0 };
                for c in 0..n {
                    let e = comp_expl[k][c] + sign * comp_pc[k][c];
                    self.fluids[k].dgdt[c] = -(e + comp_c[k][c] * (self.p_rgh.c[c] - p_rgh_0[c]) * rdt);
                }
            }
            // reconstructU_2p.H (porousInterfaceSharpness 0): `phik.relax()`
            for k in 0..2 {
                let a = self.controls.relax_phi[k];
                if a < 1.0 {
                    for f in 0..nf {
                        self.fluids[k].phi[f] = phi_prev_iter[k][f] + a * (self.fluids[k].phi[f] - phi_prev_iter[k][f]);
                    }
                }
            }
            // `U1.correctBoundaryConditions()` reads the (relaxed) fluxes of both fluids (`pressureInletOutletVelocity`)
            let phis_now = [self.fluids[0].phi.clone(), self.fluids[1].phi.clone()];
            for k in 0..2 {
                let u = fv::reconstruct_openfoam(&g, &self.fluids[k].phi);
                self.fluids[k].u.c = u;
                self.fluids[k].u.correct_bc_with_flux(&g, [&phis_now[0], &phis_now[1]]);
                self.fluids[k].mag_u = self.fluids[k].u.c.iter().map(|v| v.mag()).collect();
                // fluid::correctAlphaRhoMagU
                self.fluids[k].alpha_rho_mag_u = (0..n)
                    .map(|c| self.fluids[k].alpha.c[c] * self.fluids[k].rho.c[c] * self.fluids[k].mag_u[c])
                    .collect();
            }
            // updatePP_rghRho_2p.H
            for c in 0..n {
                self.p.c[c] = (self.p_rgh.c[c] + self.rho_mix.c[c] * self.gh.c[c]).max(self.controls.p_min);
                self.p_rgh.c[c] = self.p.c[c] - self.rho_mix.c[c] * self.gh.c[c];
            }
            for k in 0..2 {
                for c in 0..n {
                    let d = self.fluids[k].psi[c] * (self.p_rgh.c[c] - p_rgh_0[c]);
                    self.fluids[k].rho.c[c] += d;
                }
            }
            self.update_mixture_density();
            for c in 0..n {
                self.p_rgh.c[c] = self.p.c[c] - self.rho_mix.c[c] * self.gh.c[c];
            }
            self.p_rgh.correct_bc(&g);
            for f in ni..nf {
                if g.active[f] {
                    self.p.b[f - ni] = self.p_rgh.b[f - ni] + self.rho_mix.b[f - ni] * self.ghf[f];
                }
            }
            let converged = res < self.controls.p_rgh_tolerance;
            if self.controls.correct_until_convergence && converged {
                break;
            }
        }
        (first_res, n_piso)
    }

    // ───────────────────────── energy ─────────────────────────

    /// Diagnostic (gh:#1361 round 13): record the diagonal and source of cells 20-29 of the fluid-1 energy matrix after
    /// the named assembly stage of `EEqns_2p.H` (`S0` ddt, div, laplacian and the `alpha rho U.g` source; `S1` after the
    /// continuity-error `SuSp`; `S2` after the `FFPair` heat sources; `S3` after the structure heat source).
    fn push_stage_probe(
        probe: &mut Option<Vec<TraceProbeRecord>>,
        time: f64,
        corr: usize,
        stage: &'static str,
        e: &Eqn,
    ) {
        let n = e.ldu.diag.len();
        if let Some(p) = probe.as_mut() {
            for c in 20..30.min(n) {
                p.push(TraceProbeRecord {
                    kind: stage,
                    time,
                    outer_iteration: corr,
                    cell: c,
                    values: vec![("diag", e.ldu.diag[c]), ("source", e.source[c])],
                });
            }
        }
    }

    /// `EEqns_2p.H`. Returns the initial residual of the fluid-1 solve.
    fn correct_energy(&mut self) -> f64 {
        let g = self.geom.clone();
        let n = g.n_cells();
        // structure_.checkPowerOff()
        if !self.structure.powered_off {
            if let Some(crit) = &mut self.structure.power_off {
                if crit.check(Time::new::<second>(self.time), 0.0) {
                    self.structure.powered_off = true;
                }
            }
            if self.structure.powered_off {
                for pm in self.structure.power.iter_mut().chain(self.structure.power_old.iter_mut()).flatten() {
                    pm.power_off();
                }
                // `structure::checkPowerOff`: `powerOff()` of every model of the table (the radial ones too, gh:#1884)
                for m in &mut self.structure.radial {
                    m.power_off();
                }
            }
        }
        if self.structure.radial_controls.ht_pre_solve {
            self.radial_t_pre = Some([self.fluids[0].t.c.clone(), self.fluids[1].t.c.clone()]);
        }
        if !self.p_old_stored {
            // `fvc::ddt(p_)` creates `p.oldTime()` as a copy of the current field.
            self.p_old = self.p.c.clone();
            self.p_old_stored = true;
        }
        let dpdt: Vec<f64> = (0..n)
            .map(|c| (((self.p.c[c] - self.p_old[c]) * (1.0 / self.dt))).max(-self.controls.max_dpdt).min(self.controls.max_dpdt))
            .collect();
        let mut res1 = 0.0;
        for k in 0..2 {
            let fl = &self.fluids[k];
            // `E1Eqn` as `fvMatrix` holds it (gh:#1909 B4, the two-phase twin of `one_phase_fc`'s round 46 energy assembly): `fvm::ddt` and
            // `fvm::div` are separate matrices added whole (`ddt + div - laplacian == ...`), the boundary coefficients stay per face in
            // `internalCoeffs_`/`boundaryCoeffs_` (`eic`, `ebc`) until `fvMatrix::relax` and `solveSegregated`
            let rdt = 1.0 / self.dt;
            let (ni, nf) = (g.n_internal(), g.n_faces());
            let mut eic = vec![0.0; nf - ni];
            let mut ebc = vec![0.0; nf - ni];
            let mut e = Eqn::new(&g);
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                e.ldu.diag[c] += rdt * fl.alpha.c[c] * fl.rho.c[c] * v;
                e.source[c] += rdt * fl.alpha_old[c] * fl.rho_old[c] * fl.he_old[c] * v;
            }
            let mut m_div = Eqn::new(&g);
            m_div.div_upwind_internal(&g, &fl.alpha_rho_phi);
            for f in ni..nf {
                if g.active[f] {
                    // `gaussConvectionScheme::fvmDiv`: `internalCoeffs = flux valueInternalCoeffs`, `boundaryCoeffs = -flux valueBoundaryCoeffs`
                    let phi = fl.alpha_rho_phi[f];
                    if fl.he.is_fixed(&g, f) {
                        eic[f - ni] += phi * 0.0;
                        ebc[f - ni] += -phi * fl.he.face(&g, f);
                    } else {
                        eic[f - ni] += phi * 1.0;
                        ebc[f - ni] += -phi * 0.0;
                    }
                }
            }
            e.add(&m_div);
            // - laplacian(interpolate(alpha) interpolate(alphaEff), he)
            let mut ae = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                ae.c[c] = fl.kappa.c[c] / fl.cp.c[c];
            }
            for i in 0..ae.b.len() {
                ae.b[i] = if fl.cp.b[i] > 0.0 { fl.kappa.b[i] / fl.cp.b[i] } else { 0.0 };
            }
            // `thermo.alphaEff(alphat) = CpByCpv (alpha_ + alphat)` (gh:#1802; CpByCpv = 1 for enthalpy)
            if let Some(t) = &fl.turbulence {
                for c in 0..n {
                    ae.c[c] += t.alphat.c[c];
                }
                for i in 0..ae.b.len() {
                    ae.b[i] = if fl.cp.b[i] > 0.0 { fl.kappa.b[i] / fl.cp.b[i] + t.alphat.b[i] } else { 0.0 };
                }
            }
            let a_f = fv::interp_linear_openfoam(&g, &fl.alpha);
            let ae_f = fv::interp_linear_openfoam(&g, &ae);
            let gam: Vec<f64> = a_f.iter().zip(&ae_f).map(|(a, b)| a * b).collect();
            let mut m_lap = Eqn::new(&g);
            m_lap.neg_laplacian_internal(&g, &gam);
            let (lic, lbc) = Eqn::neg_laplacian_boundary_coeffs(&g, &gam, &fl.he);
            for i in 0..(nf - ni) {
                eic[i] += lic[i];
                ebc[i] += lbc[i];
            }
            e.add(&m_lap);
            // `== alpha rho (U & g) + fvOptions`: `source += V field`; then `E1Eqn -= alpha1 dpdt`: `source += V field`
            let mut rhs = vec![0.0; n];
            for c in 0..n {
                rhs[c] = fl.alpha.c[c] * fl.rho.c[c] * fl.u.c[c].dot(self.g);
            }
            e.su(&g, &rhs);
            if self.controls.dpdt {
                let dp: Vec<f64> = (0..n).map(|c| fl.alpha.c[c] * dpdt[c]).collect();
                e.su(&g, &dp);
            }
            if k == 0 {
                Self::push_stage_probe(&mut self.trace_probe, self.time, self.corr_in_step, "S0", &e);
            }
            // contErr SuSp
            let ce: Vec<f64> = fl.cont_err.iter().map(|v| -v).collect();
            e.su_sp(&g, &ce, &fl.he.c);
            if k == 0 {
                Self::push_stage_probe(&mut self.trace_probe, self.time, self.corr_in_step, "S1", &e);
            }
            // FFPair heSources (interfacial + phase change)
            let src = &self.he_src[k];
            if src.su.len() == n {
                // `E += FFPair::heSource(he)`: the sources are ONE matrix added whole (its internal split between the
                // interfacial and the phase-change part is not reproduced: the port holds their sum, README round 58)
                let mut mh = Eqn::new(&g);
                mh.su(&g, &src.su);
                mh.sp(&g, &src.sp);
                mh.su_sp(&g, &src.su_sp, &fl.he.c);
                e.add(&mh);
            }
            if k == 0 {
                Self::push_stage_probe(&mut self.trace_probe, self.time, self.corr_in_step, "S2", &e);
            }
            // structure linearizedSemiImplicitHeatSource
            let mut su = vec![0.0; n];
            let mut sp = vec![0.0; n];
            for c in 0..n {
                let ih = self.structure.ia_act[c] * self.htc_fs[k][c];
                su[c] = ih * (self.structure.t_act[c] - fl.t.c[c] + fl.he.c[c] / fl.cp.c[c]);
                sp[c] = ih / fl.cp.c[c];
            }
            e.su(&g, &su);
            e.sp(&g, &sp);
            if k == 0 {
                Self::push_stage_probe(&mut self.trace_probe, self.time, self.corr_in_step, "S3", &e);
            }
            // stabilisation source
            let t = fl.thermo_residual_alpha;
            let hei: Vec<f64> = (0..n).map(|c| fl.thermo.he(self.p.c[c], self.t_interface[c])).collect();
            if self.controls.he_stabilization == HeStabilization::Source && t != 0.0 {
                let tau = 1e-12;
                let mut cs = vec![0.0; n];
                let mut cu = vec![0.0; n];
                for c in 0..n {
                    if fl.normalized[c] - t < 0.0 {
                        cs[c] = t * fl.rho.c[c] / tau;
                        cu[c] = cs[c] * hei[c];
                    }
                }
                e.sp(&g, &cs);
                e.su(&g, &cu);
            }
            if k == 0 && self.trace_probe.is_some() {
                let mut recs = Vec::with_capacity(n + e.ldu.upper.len());
                for c in 0..n {
                    recs.push(TraceProbeRecord {
                        kind: "eqn",
                        time: self.time,
                        outer_iteration: self.corr_in_step,
                        cell: c,
                        values: vec![
                            ("diag", e.ldu.diag[c]),
                            ("source", e.source[c]),
                            ("he", fl.he.c[c]),
                            ("alphaEff", ae.c[c]),
                            ("Cp", fl.cp.c[c]),
                            ("rho", fl.rho.c[c]),
                            ("T", fl.t.c[c]),
                            ("alpha2", self.fluids[1].alpha.c[c]),
                            ("norm2", self.fluids[1].normalized[c]),
                            ("T2", self.fluids[1].t.c[c]),
                            ("he2", self.fluids[1].he.c[c]),
                            ("iT", self.t_interface[c]),
                            ("dmdt", self.dmdt[c]),
                            ("rho2", self.fluids[1].rho.c[c]),
                        ],
                    });
                }
                for f in 0..e.ldu.upper.len() {
                    recs.push(TraceProbeRecord {
                        kind: "eqnf",
                        time: self.time,
                        outer_iteration: self.corr_in_step,
                        cell: f,
                        values: vec![("upper", e.ldu.upper[f]), ("lower", e.ldu.lower[f])],
                    });
                }
                if let Some(p) = self.trace_probe.as_mut() {
                    p.extend(recs);
                }
            }
            if k == 1 && self.trace_probe.is_some() {
                let recs: Vec<TraceProbeRecord> = (0..n)
                    .map(|c| TraceProbeRecord {
                        kind: "eqn2",
                        time: self.time,
                        outer_iteration: self.corr_in_step,
                        cell: c,
                        values: vec![
                            ("diag", e.ldu.diag[c]),
                            ("source", e.source[c]),
                            ("he2", self.fluids[1].he.c[c]),
                            ("he2i", hei[c]),
                            ("alphaEff2", ae.c[c]),
                            ("Cp2", self.fluids[1].cp.c[c]),
                            ("rho2", self.fluids[1].rho.c[c]),
                            ("T2", self.fluids[1].t.c[c]),
                            ("pos2", self.fluids[1].above_thermo_residual_alpha[c]),
                            ("neg2", self.fluids[1].below_thermo_residual_alpha[c]),
                        ],
                    })
                    .collect();
                if let Some(p) = self.trace_probe.as_mut() {
                    p.extend(recs);
                }
            }
            let fl = &self.fluids[k];
            // `E1Eqn.relax()` (`relaxationFactors.equations`, `".*" 1` in the shipped cases), then `solveSegregated` folds the boundary coefficients
            if let Some(a) = self.controls.relax_he_equation[k] {
                e.relax_scalar(&g, a, &eic, &fl.he.c);
            }
            let e = e.folded(&g, &eic, &ebc);
            let prev = fl.he.c.clone();
            let mut x = prev.clone();
            // the converged (final) outer iteration solves with the `h.<fluid>Final` entry (gh:#1957 round 83)
            let (h_pb, h_sm) = if self.pimple_converged {
                (self.controls.h_pbicgstab_final, self.controls.h_smooth_final[k])
            } else {
                (self.controls.h_pbicgstab, self.controls.h_smooth[k])
            };
            let (sweeps, ini) = if let Some(pb) = h_pb {
                // upstream resolves `h.<fluid>` to the LAST matching pattern of `solvers` (`".*"`: PBiCGStab, diagonal)
                let perf = pbicgstab::pbicgstab_diagonal(
                    &e.ldu,
                    &e.source,
                    &mut x,
                    pb.tolerance,
                    pb.rel_tol,
                    pbicgstab::DEFAULT_MAX_ITER,
                    pb.min_iter,
                );
                (perf.iterations, perf.initial_residual)
            } else if let Some(ctl) = h_sm {
                // gh:#1957 round 83: `smoothSolver::solve` with every control of the `h.<fluid>` entry
                let perf = crate::genfoam::common::smooth_solver::smooth_solver_sym_gauss_seidel(&e.ldu, &e.source, &mut x, &ctl);
                (perf.iterations, perf.initial_residual)
            } else {
                let (sweeps, ini, _) = super::one_phase::smooth_solve_sym_gauss_seidel(
                    &e.ldu,
                    &e.source,
                    &mut x,
                    SolverSettings { tolerance: self.controls.h_tolerance, max_iter: 1000 },
                );
                (sweeps, ini)
            };
            self.h_solve_log.push((k, ini, sweeps));
            if h_pb.is_none() && h_sm.is_none() && sweeps == 0 && self.controls.h_min_iter > 0 {
                let _ = super::one_phase::smooth_solve_sym_gauss_seidel(
                    &e.ldu,
                    &e.source,
                    &mut x,
                    SolverSettings { tolerance: 0.0, max_iter: self.controls.h_min_iter },
                );
            }
            if k == 0 {
                res1 = ini;
            }
            let fr = self.controls.relax_he[k];
            let fl = &mut self.fluids[k];
            for c in 0..n {
                fl.he.c[c] = prev[c] + fr * (x[c] - prev[c]);
            }
            if self.controls.he_stabilization == HeStabilization::Cutoff {
                if self.controls.live_thermo_residual_markers {
                    for c in 0..n {
                        let pos = fl.normalized[c] - fl.thermo_residual_alpha > 0.0;
                        let v = if pos { fl.he.c[c] } else { hei[c] };
                        fl.he.c[c] = v.max(100.0);
                    }
                } else {
                    // `he = max(pos*he + neg*hei, 100)` with the markers of the start of the iteration
                    for c in 0..n {
                        let v = fl.above_thermo_residual_alpha[c] * fl.he.c[c]
                            + fl.below_thermo_residual_alpha[c] * hei[c];
                        fl.he.c[c] = v.max(100.0);
                    }
                }
            }
            fl.he.correct_bc(&g);
        }
        for k in 0..2 {
            self.thermo_correct(k, false);
        }
        // structure_.correct(H1s T1 + H2s T2, H1s + H2s)
        for c in 0..n {
            let h = self.htc_fs[0][c] + self.htc_fs[1][c];
            let ht = self.htc_fs[0][c] * self.fluids[0].t.c[c] + self.htc_fs[1][c] * self.fluids[1].t.c[c];
            if let Some(old) = &self.structure.power_old[c] {
                let mut pm = old.clone();
                let tn = pm.correct(
                    uom::si::f64::HeatTransfer::new::<watt_per_square_meter_kelvin>(h),
                    uom::si::f64::HeatFluxDensity::new::<watt_per_square_meter>(ht),
                    Time::new::<second>(self.dt),
                    Time::new::<second>(self.time),
                );
                self.structure.t_act[c] = tn.get::<kelvin>();
                self.structure.power[c] = Some(pm);
                if let Some(p) = self.trace_probe.as_mut() {
                    p.push(TraceProbeRecord {
                        kind: "structure",
                        time: self.time,
                        outer_iteration: self.corr_in_step,
                        cell: c,
                        values: vec![("H", h), ("HT", ht), ("Tact", tn.get::<kelvin>())],
                    });
                }
            }
        }
        // the radial models of the table (`structure::correct`: `iter()->correct(HT, H); iter()->correctT(Tact_)`, gh:#1884)
        self.radial_structure_correct();
        // `structure::correct` ends with `heatFlux = H Twall - HT` over `cells_` (structure.C:1057-1071; gh:#1967 round 83)
        self.update_structure_heat_flux();
        // `structure::correct` ends with `Twall = Tact` in its cells
        self.structure.t_wall = self.structure.t_act.clone();
        if self.structure_old_time_stale {
            // fixedPower::correct: `T_.oldTime()` is created in outer iteration 1
            // with a stale time index; the first request of outer iteration 2
            // overwrites it with the iteration-1 result (item 2 of
            // `TwoPhaseControls::lazy_old_time_first_step`).
            self.structure.power_old = self.structure.power.clone();
            self.structure_old_time_stale = false;
        }
        res1
    }

    /// `structure::correct(HT, H)` for the radial power models (gh:#1884; `structure.C:987-1000`, `EEqns_2p.H:331`): `HT = H1s T1 + H2s T2`
    /// and `H = H1s + H2s` (the htc fields already carry the phase partition), each model `correct(HT, H)`, then `correctT(Tact_)`,
    /// then `heatFlux = H Twall - HT` on the model cells. A `lumpedNuclearStructure` first takes `fvc::laplacian(Tmatrix)`.
    ///
    /// # Panics
    /// On a singular radial matrix (upstream's `Foam::solve` is a `FatalError`: the case is unphysical).
    fn radial_structure_correct(&mut self) {
        if self.structure.radial.is_empty() {
            return;
        }
        let n = self.geom.n_cells();
        let ctl = self.structure.radial_controls;
        let (t1, t2): (Vec<f64>, Vec<f64>) = match (&self.radial_t_pre, ctl.ht_pre_solve) {
            (Some(pre), true) => (pre[0].clone(), pre[1].clone()),
            _ => (self.fluids[0].t.c.clone(), self.fluids[1].t.c.clone()),
        };
        let h: Vec<f64> = (0..n).map(|c| self.htc_fs[0][c] + self.htc_fs[1][c]).collect();
        let ht: Vec<f64> = (0..n).map(|c| self.htc_fs[0][c] * t1[c] + self.htc_fs[1][c] * t2[c]).collect();
        let volumes = self.geom.mesh.cell_volumes.clone();
        for i in 0..self.structure.radial.len() {
            let lap = match (self.structure.radial[i].tmatrix(), ctl.no_laplacian) {
                (Some(tm), false) => radial_power::fvc_laplacian_uncorrected(&self.geom, tm),
                _ => vec![0.0; n],
            };
            let step = StepInputs {
                dt: self.dt,
                time_index: self.steps_begun,
                q_neutronics: self.structure.q_neutronics.clone(),
                laplacian_t_matrix: lap,
                volumes: volumes.clone(),
            };
            if let Err(e) = self.structure.radial[i].correct(&ht, &h, &step) {
                panic!("radial power model {}: {e}", self.structure.radial[i].type_name());
            }
            for &c in self.structure.radial[i].cells() {
                let tact = self.structure.radial[i].surface_temperature(c);
                if !ctl.freeze_tact {
                    self.structure.t_act[c] = tact;
                }
                self.structure.heat_flux[c] = h[c] * self.structure.t_act[c] - ht[c];
            }
        }
    }

    /// The end of `structure::correct(H1s T1 + H2s T2, H1s + H2s)` (`structure.C:1057-1071`, `EEqns_2p.H:331`): `heatFlux = H Tact - HT`
    /// on every structure cell (`cells_`, [`Structure::cells`]), from the post-solve temperatures (gh:#1967 round 83). The radial models'
    /// cells keep the value [`Self::radial_structure_correct`] set (the same expression; its `ht_pre_solve` CONTROL uses the pre-solve
    /// temperatures there). Before round 83 the fixedPower and power-free structure cells kept `heatFlux` 0.
    fn update_structure_heat_flux(&mut self) {
        let n = self.geom.n_cells();
        let mut radial = vec![false; n];
        for m in &self.structure.radial {
            for &c in m.cells() {
                radial[c] = true;
            }
        }
        for i in 0..self.structure.cells.len() {
            let c = self.structure.cells[i];
            if radial[c] {
                continue;
            }
            let h = self.htc_fs[0][c] + self.htc_fs[1][c];
            let ht = self.htc_fs[0][c] * self.fluids[0].t.c[c] + self.htc_fs[1][c] * self.fluids[1].t.c[c];
            self.structure.heat_flux[c] = h * self.structure.t_act[c] - ht;
        }
    }

    /// `(T.fuelAvForNeutronics, T.cladAvForNeutronics)` per cell: the `TFuelAv`/`TCladAv` the radial models hand to the neutronics
    /// (0 where no model sets them).
    #[must_use]
    pub fn fuel_clad_temperatures(&self) -> (Vec<f64>, Vec<f64>) {
        let n = self.geom.n_cells();
        let (mut fuel, mut clad) = (vec![0.0; n], vec![0.0; n]);
        for m in &self.structure.radial {
            for &c in m.cells() {
                let (f, cl) = m.coupling_temperatures(c);
                fuel[c] = f;
                clad[c] = cl;
            }
        }
        (fuel, clad)
    }

    /// Hand the structure's `powerDensityNeutronics` [W/m^3] over from the neutronics (the `nuclearFuelPin`/`lumpedNuclearStructure` `q`).
    pub fn set_structure_power_density(&mut self, q: &[f64]) {
        self.structure.q_neutronics.clear();
        self.structure.q_neutronics.extend_from_slice(q);
    }

    /// CONTROL, NOT upstream (gh:#1884): build the radial models' `HT` from the temperatures before the energy solve.
    #[must_use]
    pub fn with_pre_solve_temperature_for_radial_ht(mut self) -> Self {
        self.structure.radial_controls.ht_pre_solve = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1884): `fvc::laplacian(Tmatrix)` of the lumped model taken as 0.
    #[must_use]
    pub fn without_radial_lumped_laplacian(mut self) -> Self {
        self.structure.radial_controls.no_laplacian = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1884): `Tact` is never handed back from the radial models.
    #[must_use]
    pub fn without_radial_tact_hand_back(mut self) -> Self {
        self.structure.radial_controls.freeze_tact = true;
        self
    }

    /// CONTROL, NOT upstream (gh:#1884): the structure constructor's `iAact_ += iter()->iA()` forgotten for the radial models.
    #[must_use]
    pub fn without_radial_interfacial_area(mut self) -> Self {
        for m in &self.structure.radial {
            for &c in m.cells() {
                self.structure.ia_act[c] -= m.interfacial_area(c);
            }
        }
        self
    }

    /// `heThermo::correct` (`T` from `he`, then `rho`, `psi`, `Cp`, `mu`,
    /// `kappa`); `init` evaluates `he` from `T` instead (thermo construction).
    fn thermo_correct(&mut self, k: usize, init: bool) {
        let g = &self.geom;
        let n = g.n_cells();
        let ni = g.n_internal();
        let p = &self.p;
        let fl = &mut self.fluids[k];
        for c in 0..n {
            if init {
                fl.he.c[c] = fl.thermo.he(p.c[c], fl.t.c[c]);
            } else if let Some(t) = if self.controls.converged_temperature_newton {
                fl.thermo.t_from_he_converged(fl.he.c[c], p.c[c], fl.t.c[c])
            } else {
                fl.thermo.t_from_he(fl.he.c[c], p.c[c], fl.t.c[c])
            } {
                fl.t.c[c] = t;
            }
            let pp = fl.thermo.props(p.c[c], fl.t.c[c]);
            fl.rho.c[c] = pp.rho;
            fl.psi[c] = pp.psi;
            fl.cp.c[c] = pp.cp;
            fl.mu.c[c] = pp.mu;
            fl.kappa.c[c] = pp.kappa;
        }
        // boundaries: fixed T ⇒ fixedEnergy he; else he and T follow the cell.
        fl.t.correct_bc(g);
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let i = f - ni;
            let pb = p.b[i];
            fl.he.b[i] = fl.thermo.he(pb, fl.t.b[i]);
            if let Bc::Fixed(v) = &mut fl.he.bc[g.patch_of[i]] {
                let pat = &g.mesh.patches[g.patch_of[i]];
                v[f - pat.start] = fl.he.b[i];
            }
            let pp = fl.thermo.props(pb, fl.t.b[i]);
            fl.rho.b[i] = pp.rho;
            fl.cp.b[i] = pp.cp;
            fl.mu.b[i] = pp.mu;
            fl.kappa.b[i] = pp.kappa;
        }
    }

    /// `twoPhase::calcCumulContErrs`, called in the iteration flagged converged (its `if (pimple_.finalIter())`
    /// is true only there, see `step`): the volume-weighted time integral of each fluid's continuity error
    /// [kg/m^3] over the step just taken.
    fn calc_cumul_cont_errs(&mut self) {
        let m = &self.geom.mesh;
        let tot_v: f64 = m.cell_volumes.iter().sum();
        for k in 0..2 {
            let d: f64 = (0..m.n_cells)
                .map(|c| self.fluids[k].cont_err[c] * m.cell_volumes[c] * self.dt)
                .sum();
            self.fluids[k].cumul_cont_err += d / tot_v;
        }
    }

    // ───────────────────────── time step ─────────────────────────

    /// `twoPhase::correctCourant`: the larger of the mixture and the relative
    /// Courant numbers.
    #[must_use]
    pub fn courant(&self) -> f64 {
        let g = &self.geom;
        let sum = fv::surface_sum_mag(g, &self.phi);
        let rel: Vec<f64> = (0..g.n_faces())
            .map(|f| self.fluids[0].phi[f] - self.fluids[1].phi[f])
            .collect();
        let sum_r = fv::surface_sum_mag(g, &rel);
        let mut co = 0.0_f64;
        let mut co_r = 0.0_f64;
        for c in 0..g.n_cells() {
            let d = self.moving(c) * g.mesh.cell_volumes[c];
            co = co.max(0.5 * sum[c] / d * self.dt);
            co_r = co_r.max(0.5 * sum_r[c] / d * self.dt);
        }
        co.max(co_r)
    }

    /// `twoPhase::maxDeltaT`.
    fn max_delta_t(&self, co: f64) -> f64 {
        let tc = &self.time_controls;
        let n = self.geom.n_cells();
        let phase1 = self.fluids[0].alpha.c.iter().cloned().fold(f64::MIN, f64::max) >= 1e-6;
        let phase2 = self.fluids[1].alpha.c.iter().cloned().fold(f64::MIN, f64::max) >= 1e-6;
        let both = phase1 && phase2;
        let mut max_co = tc.max_co;
        if let Some(mc2) = tc.max_co_two_phase {
            if both {
                max_co = mc2;
            } else if let Some(margin) = tc.margin_to_phase_change {
                let dt1 = (0..n)
                    .map(|c| (self.fluids[0].t.c[c] - self.t_interface[c]).abs())
                    .fold(f64::MAX, f64::min);
                let dt2 = (0..n)
                    .map(|c| (self.fluids[1].t.c[c] - self.t_interface[c]).abs())
                    .fold(f64::MAX, f64::min);
                if (phase1 && !phase2 && dt1 < margin) || (phase2 && !phase1 && dt2 < margin) {
                    max_co = mc2;
                }
            }
        }
        let f = 0.1;
        let fact = max_co / (co + 1e-15);
        let dfact = fact.min(1.0 + f * fact).min(1.0 + f);
        (dfact * self.dt).min(tc.max_delta_t).max(1e-69)
    }

    /// OpenFOAM `Time::adjustDeltaT` for `writeControl adjustableRunTime`:
    /// shrink the step evenly so a write time is hit exactly.
    fn adjust_to_write_time(&mut self) {
        let Some(wi) = self.time_controls.write_interval else { return };
        if self.time_controls.esi_adjust_delta_t {
            // ESI `Time::adjustDeltaT` (OpenFOAM-v2506 src/OpenFOAM/db/Time/Time.C:102).
            let to_next = (((self.write_time_index + 1) as f64) * wi
                - (self.time - self.time_controls.start_time))
                .max(0.0);
            let n_steps = to_next / self.dt;
            if n_steps < f64::from(i32::MAX) {
                let n = n_steps.round().max(1.0);
                let new_dt = to_next / n;
                self.dt = if new_dt >= self.dt { new_dt.min(2.0 * self.dt) } else { new_dt.max(0.2 * self.dt) };
            }
            return;
        }
        let next = ((self.time / wi + 1e-9).floor() + 1.0) * wi;
        let to_next = next - self.time;
        let n_steps = (to_next / self.dt - 1e-9).ceil().max(1.0);
        self.dt = to_next / n_steps;
    }
}

/// A vector momentum equation as `fvVectorMatrix`: the diagonal, the off-diagonals, the source per component and the per-boundary-face
/// `internalCoeffs`/`boundaryCoeffs` kept apart (REUSE of `UEqnCc`, gh:#1909 B4; the earlier form folded `cmptAv(internalCoeffs)` into the
/// diagonal as each boundary term was assembled).
struct VecEqn {
    cc: UEqnCc,
}

impl TwoPhaseSolver {
    /// The ablation of the lazy `alpha.oldTime()` of the OTHER fluid of the `solveAlpha` lambda (`solvePhase` set): its
    /// old-time copy is the start-of-step value in step 1 too, which is what the port did before gh:#1799 round 34 (step 1 was
    /// then off by p 3e-6, alpha 1.6e-6 against the real binary). Nothing else of
    /// [`TwoPhaseControls::lazy_old_time_first_step`] changes.
    #[must_use]
    pub fn with_eager_other_alpha_old_time(mut self) -> Self {
        self.other_alpha_old_stale = false;
        self
    }

    /// The ablation of [`TwoPhaseControls::lazy_old_time_first_step`]: the first
    /// time step treats the old-time pressure as the INITIAL field and the
    /// structure's old-time temperature as its value at the start of the step,
    /// as every later step does. This is what the port did before gh:#1361
    /// round 3; it differs from GeN-Foam 652b3da in step 1 only.
    #[must_use]
    pub fn without_first_step_lazy_old_time(mut self) -> Self {
        self.controls.lazy_old_time_first_step = false;
        self.p_old_stored = true;
        self.structure_old_time_stale = false;
        self.other_alpha_old_stale = false;
        self
    }

    /// ABLATION (gh:#1957 round 83): an `h.<fluid>` entry that resolves to `smoothSolver` runs the round 16 sweeps
    /// (`one_phase::smooth_solve_sym_gauss_seidel` with [`TwoPhaseControls::h_tolerance`]: `nSweeps 1`, `relTol`, `nSweeps`, `maxIter` ignored)
    /// instead of `smoothSolver::solve` with every control of the entry.
    #[must_use]
    pub fn with_round16_h_smoother(mut self) -> Self {
        self.controls.h_smooth = [None, None];
        self.controls.h_smooth_final = [None, None];
        self
    }

    /// ABLATION (gh:#1957 round 83): `k`/`epsilon` of both fluids solved with the round 16 smoother
    /// ([`turbulence::PorousKEpsilon2PhaseCorrected::with_round16_smoother`]).
    #[must_use]
    pub fn with_round16_k_epsilon_smoother(mut self) -> Self {
        for f in self.fluids.iter_mut() {
            if let Some(t) = f.turbulence.as_mut() {
                t.round16_smoother = true;
            }
        }
        self
    }

    /// Reproduce upstream GeN-Foam 652b3da's `customPimpleControl::loop` DEFECT
    /// ([`TwoPhaseControls::stale_pimple_converged_flag`] `= true`): a step whose criteria were first
    /// met in the last outer iteration leaves `converged_` set and the NEXT step solves nothing.
    /// The explicit deviation from correct behaviour, for comparison against the upstream binary
    /// only; the default ends the loop cleanly.
    #[must_use]
    pub fn with_upstream_stale_pimple_converged_flag(mut self) -> Self {
        self.controls.stale_pimple_converged_flag = true;
        self
    }
}

impl TwoPhaseSolver {
    /// Evaluate the Nusselt forced-convection htc in the operation order of the port before gh:#1361 round 8
    /// ([`TwoPhaseControls::earlier_nusselt_association`] `= true`); an ulp different from upstream's.
    #[must_use]
    pub fn with_earlier_nusselt_association(mut self) -> Self {
        self.controls.earlier_nusselt_association = true;
        self
    }

    /// DEPRECATED (gh:#1361 round 13): return `T` from `he` as the converged Newton root
    /// ([`TwoPhaseControls::converged_temperature_newton`] `= true`), as the port did before it translated
    /// `species::thermo::T`'s one-iterate stopping rule. Differs from the real binary by up to 1e-8 relative in `T`.
    #[must_use]
    pub fn with_converged_temperature_newton(mut self) -> Self {
        self.controls.converged_temperature_newton = true;
        self
    }

    /// DEPRECATED (gh:#1361 round 13): solve the enthalpy equations with `smoothSolver`/`symGaussSeidel` and the first
    /// matching `solvers` entry (`h.*`), as the port did before it resolved `h.<fluid>` as OpenFOAM does (the LAST matching
    /// pattern: `".*"`, `PBiCGStab`/`diagonal`). The `symGaussSeidel` sweeps stop at the nominal tolerance with a solution error of
    /// 1e-8 relative in the stiff cells next to a stabilised (`thermoResidualAlpha`) region, where upstream's solve reaches 1e-17.
    #[must_use]
    pub fn with_smooth_solver_for_enthalpy(mut self) -> Self {
        self.controls.h_pbicgstab = None;
        self.controls.h_smooth = [None, None];
        self.controls.h_pbicgstab_final = None;
        self.controls.h_smooth_final = [None, None];
        self
    }

    /// DEPRECATED (gh:#1361 round 17, gh:#1833): evaluate the `UEqn.H()` field of `pEqnf_2p.H` as `zeroGradient` on a `symmetry`
    /// mesh patch, as the port did before it applied OpenFOAM's constraint patch field there.
    #[must_use]
    pub fn with_zero_gradient_h_on_symmetry_patches(mut self) -> Self {
        self.controls.h_symmetry_constraint = false;
        self
    }

    /// DEPRECATED (gh:#1361 round 17, gh:#1833): solve `p_rgh` with the port's ILU(0)-BiCGStab stopped at
    /// `||r||_2/||b||_2 < 1e-13` even when the case selects `PCG`/`DIC`, as the port did before it translated
    /// `PCG::scalarSolve`. On `PhaseII_Ex3` that leaves `p_rgh` wrong by 2.7e-3 Pa after the first corrector (`p` 1.5e-9 relative
    /// at the end of the step with the symmetry constraint in place; README round 17).
    #[must_use]
    pub fn with_krylov_for_p_rgh(mut self) -> Self {
        self.controls.p_rgh_pcg = None;
        self.controls.p_rgh_gamg = None;
        self
    }

    /// DEPRECATED (gh:#1361 round 13): test `normalized - thermoResidualAlpha > 0` at the time of the `cutoff` clamp
    /// ([`TwoPhaseControls::live_thermo_residual_markers`] `= true`) instead of upstream's start-of-iteration marker
    /// fields. A cell whose fraction crosses the threshold inside an iteration is treated differently from upstream.
    #[must_use]
    pub fn with_live_thermo_residual_markers(mut self) -> Self {
        self.controls.live_thermo_residual_markers = true;
        self
    }

    /// DEPRECATED (gh:#1361 round 13): use `dmdt/dmdtIi0` for the interfacial flux ratio of `phaseChangeModel::correct`
    /// ([`TwoPhaseControls::earlier_interfacial_flux_ratio`] `= true`), the port's form before it translated upstream's
    /// `dmdtIi/dmdtIi0`. Differs wherever wall boiling (`fw != 0`) acts in a cell with `|dmdt| > 1e-2`.
    #[must_use]
    pub fn with_earlier_interfacial_flux_ratio(mut self) -> Self {
        self.controls.earlier_interfacial_flux_ratio = true;
        self
    }
}

impl TwoPhaseSolver {
    /// Solve the `alpha` equation to round-off ([`TwoPhaseControls::alpha_solver_openfoam`]
    /// `= false`), instead of stopping where OpenFOAM's `PBiCGStab` does.
    #[must_use]
    pub fn with_exact_alpha_solve(mut self) -> Self {
        self.controls.alpha_solver_openfoam = false;
        self
    }
}

impl TwoPhaseSolver {
    /// Keep the port's earlier write-time rule instead of ESI `Time::adjustDeltaT`
    /// ([`TimeControls::esi_adjust_delta_t`] `= false`).
    #[must_use]
    pub fn with_foundation_style_write_time_adjust(mut self) -> Self {
        self.time_controls.esi_adjust_delta_t = false;
        self
    }
}

impl TwoPhaseSolver {
    /// The ablation of [`TwoPhaseControls::t_interface_limiter_sees_new_value`]:
    /// `maxTInterfaceDdt` limits against `T.interface` at the start of the step, as
    /// the source reads, not as OpenFOAM's old-time storage makes it behave.
    #[must_use]
    pub fn with_documented_t_interface_rate_limiter(mut self) -> Self {
        self.controls.t_interface_limiter_sees_new_value = false;
        self
    }
}

impl VecEqn {
    fn new(g: &Geometry) -> Self {
        Self { cc: UEqnCc::new(g) }
    }

    /// The boundary part of `fvm::div(phi, U)` (`gaussConvectionScheme::fvmDiv`): per boundary face `internalCoeffs = phi *
    /// valueInternalCoeffs`, `boundaryCoeffs = -phi * valueBoundaryCoeffs`, kept apart from the diagonal and the source as `fvMatrix`
    /// keeps them (gh:#1909 B4). `phis` are the two fluxes `pressureInletOutletVelocity` reads.
    fn boundary_convection(&mut self, g: &Geometry, phi: &[f64], u: &Vector, phis: [&[f64]; 2]) {
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let c = u.patch_coeffs(g, f, phis);
            for k in 0..3 {
                self.cc.ic[f - ni][k] += phi[f] * c.value_internal[k];
                self.cc.bcoef[f - ni][k] += -phi[f] * c.value_boundary[k];
            }
        }
    }

    /// The boundary part of `-fvm::laplacian(gamma, U)`: `internalCoeffs = -(pGamma * gradientInternalCoeffs)`, `boundaryCoeffs =
    /// pGamma * gradientBoundaryCoeffs` (`zeroGradient` contributes zeros), per boundary face.
    fn boundary_laplacian(&mut self, g: &Geometry, gamma: &[f64], u: &Vector, phis: [&[f64]; 2]) {
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if !g.active[f] {
                continue;
            }
            let c = u.patch_coeffs(g, f, phis);
            let pg = gamma[f] * g.mesh.face_areas[f];
            let d = g.delta[f];
            for k in 0..3 {
                self.cc.ic[f - ni][k] += -(pg * (-d * c.gradient_internal_diag[k]));
                self.cc.bcoef[f - ni][k] += pg * c.gradient_boundary[k];
            }
        }
    }

    /// `fvm::SuSp(coeff, U)` added to the left-hand side.
    fn su_sp(&mut self, g: &Geometry, coeff: &[f64], u: &[Vector3]) {
        self.cc.su_sp(g, coeff, u);
    }

    /// `fvMatrix::operator*=(volScalarField::Internal)`: the diagonal, both off-diagonals, the source, and `internalCoeffs`/`boundaryCoeffs`
    /// (by the owner cell's value) are scaled.
    fn scale_rows(&mut self, g: &Geometry, s: &[f64]) {
        self.cc.eqn.scale_rows(g, s);
        for comp in 0..3 {
            for c in 0..g.n_cells() {
                self.cc.src[comp][c] *= s[c];
            }
        }
        let ni = g.n_internal();
        for f in ni..g.n_faces() {
            if g.active[f] {
                let sc = s[g.mesh.owner[f]];
                for k in 0..3 {
                    self.cc.ic[f - ni][k] *= sc;
                    self.cc.bcoef[f - ni][k] *= sc;
                }
            }
        }
    }

    /// `fvMatrix::operator+=(fvMatrix)`: every array of the right matrix added whole.
    fn add(&mut self, o: &VecEqn) {
        self.cc.add(&o.cc);
    }

    /// `UEqn.A()` per unit volume (`(D + cmptAv(internalCoeffs))/V`).
    fn a(&self, g: &Geometry) -> Vec<f64> {
        self.cc.a(g)
    }

    /// `UEqn.H()` per unit volume, cell values.
    fn h(&self, g: &Geometry, u: &Vector) -> Vec<Vector3> {
        self.cc.h(g, &u.c)
    }
}

fn cmp(v: Vector3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

/// `fvc::div(phi, U)` with Gauss upwind, per unit volume (zeroGradient U).
fn div_upwind_explicit_vec(g: &Geometry, phi: &[f64], u: &Vector) -> Vec<Vector3> {
    let m = &g.mesh;
    let ni = g.n_internal();
    let mut out = vec![Vector3::ZERO; g.n_cells()];
    for f in 0..ni {
        let (o, n) = (m.owner[f], m.neighbour[f]);
        let uf = if phi[f] >= 0.0 { u.c[o] } else { u.c[n] };
        out[o] += uf * phi[f];
        out[n] -= uf * phi[f];
    }
    for f in ni..g.n_faces() {
        if g.active[f] {
            out[m.owner[f]] += u.b[f - ni] * phi[f];
        }
    }
    for (c, v) in out.iter_mut().enumerate() {
        *v /= m.cell_volumes[c];
    }
    out
}

/// `fvc::div(gamma dev2(T(fvc::grad(U))))`, Gauss linear, per unit volume.
/// The gradient's boundary value follows `gaussGrad::correctBoundaryConditions`:
/// `grad_b = grad_P + n (snGrad_b(U) - n . grad_P)`.
fn dev2_div(g: &Geometry, u: &Vector, gamma: &Scalar) -> Vec<Vector3> {
    dev2_div_impl(g, u, gamma, false)
}

/// [`dev2_div`] with OpenFOAM's own linear interpolation, `lambda (P - N) + N` (`surfaceInterpolationScheme.C:270`) for the face
/// velocity and the face tensor, instead of `w P + (1 - w) N` (gh:#1894 round 46; used by `one_phase_fc`).
pub(crate) fn dev2_div_openfoam(g: &Geometry, u: &Vector, gamma: &Scalar) -> Vec<Vector3> {
    dev2_div_impl(g, u, gamma, true)
}

fn dev2_div_impl(g: &Geometry, u: &Vector, gamma: &Scalar, openfoam: bool) -> Vec<Vector3> {
    dev2_div_impl_t(g, u, gamma, openfoam, None)
}

/// `fvc::div(gamma tortuosity & dev2(T(fvc::grad(U))), "div(alphaRhoPhiNu,U)")` of `UEqn_1p.H` (gh:#1229 round 64): `gamma` the scalar
/// `alpha rho nuEff` and `tort` the tortuosity tensor `(cells, boundary faces)` of [`tensor_laplacian`]; the face tensor is
/// `(gamma tort) & dev2(T(grad U))`, interpolated linearly like the scalar-gamma form of [`dev2_div`].
fn dev2_div_tortuous(g: &Geometry, u: &Vector, gamma: &Scalar, tort: (&[tensor_laplacian::Tensor3], &[tensor_laplacian::Tensor3])) -> Vec<Vector3> {
    dev2_div_impl_t(g, u, gamma, false, Some(tort))
}

fn dev2_div_impl_t(
    g: &Geometry,
    u: &Vector,
    gamma: &Scalar,
    openfoam: bool,
    tort: Option<(&[tensor_laplacian::Tensor3], &[tensor_laplacian::Tensor3])>,
) -> Vec<Vector3> {
    let m = &g.mesh;
    let n = g.n_cells();
    let ni = g.n_internal();
    let nf = g.n_faces();
    type T3 = [[f64; 3]; 3];
    let mut gr = vec![[[0.0; 3]; 3]; n];
    let face_u = |f: usize| -> Vector3 {
        if f < ni {
            let w = g.weights[f];
            if openfoam {
                let (p, q) = (u.c[m.owner[f]], u.c[m.neighbour[f]]);
                Vector3::new(w * (p.x - q.x) + q.x, w * (p.y - q.y) + q.y, w * (p.z - q.z) + q.z)
            } else {
                u.c[m.owner[f]] * w + u.c[m.neighbour[f]] * (1.0 - w)
            }
        } else if g.cyclic_nbr[f - ni] != usize::MAX {
            // a coupled patch: `lerp(U_N, U_P, w)` from the cells (`gaussGrad::gradf` through `interpolate`, gh:#1905)
            g.cyclic_lerp_vector(f, &u.c, g.cyclic_w[f - ni])
        } else {
            u.b[f - ni]
        }
    };
    for f in 0..nf {
        if !g.active[f] {
            continue;
        }
        let s = m.face_area_vectors[f];
        let s = [s.x, s.y, s.z];
        let uf = face_u(f);
        let uf = [uf.x, uf.y, uf.z];
        let o = m.owner[f];
        for i in 0..3 {
            for j in 0..3 {
                gr[o][i][j] += s[i] * uf[j];
                if f < ni {
                    gr[m.neighbour[f]][i][j] -= s[i] * uf[j];
                }
            }
        }
    }
    for (c, t) in gr.iter_mut().enumerate() {
        for row in t.iter_mut() {
            for v in row.iter_mut() {
                *v /= m.cell_volumes[c];
            }
        }
    }
    let dev2t = |t: &T3, gam: f64| -> T3 {
        let mut r = [[0.0; 3]; 3];
        let tr = t[0][0] + t[1][1] + t[2][2];
        for i in 0..3 {
            for j in 0..3 {
                r[i][j] = gam * (t[j][i] - if i == j { 2.0 / 3.0 * tr } else { 0.0 });
            }
        }
        r
    };
    let mut out = vec![Vector3::ZERO; n];
    for f in 0..nf {
        if !g.active[f] {
            continue;
        }
        let o = m.owner[f];
        let xf: T3 = if f < ni {
            let w = g.weights[f];
            let nb = m.neighbour[f];
            let (a, b) = match tort {
                None => (dev2t(&gr[o], gamma.c[o]), dev2t(&gr[nb], gamma.c[nb])),
                Some((tc, _)) => (
                    tensor_laplacian::tensor_and(&tensor_laplacian::scale(gamma.c[o], &tc[o]), &dev2t(&gr[o], 1.0)),
                    tensor_laplacian::tensor_and(&tensor_laplacian::scale(gamma.c[nb], &tc[nb]), &dev2t(&gr[nb], 1.0)),
                ),
            };
            let mut r = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    r[i][j] = if openfoam { w * (a[i][j] - b[i][j]) + b[i][j] } else { w * a[i][j] + (1.0 - w) * b[i][j] };
                }
            }
            r
        } else if g.cyclic_nbr[f - ni] != usize::MAX {
            // a coupled patch (gh:#1905): the face tensor is `lerp(X_N, X_P, w)` of the CELL expression `X = gamma dev2(T(grad U))`
            // (`dotInterpolate` of the volTensorField; the gradient's own patch values are not used)
            let nb = g.cyclic_nbr[f - ni];
            let w = g.cyclic_w[f - ni];
            let (a, b) = match tort {
                None => (dev2t(&gr[nb], gamma.c[nb]), dev2t(&gr[o], gamma.c[o])),
                Some((tc, _)) => (
                    tensor_laplacian::tensor_and(&tensor_laplacian::scale(gamma.c[nb], &tc[nb]), &dev2t(&gr[nb], 1.0)),
                    tensor_laplacian::tensor_and(&tensor_laplacian::scale(gamma.c[o], &tc[o]), &dev2t(&gr[o], 1.0)),
                ),
            };
            let onet = 1.0 - w;
            let mut r = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    r[i][j] = onet * a[i][j] + w * b[i][j];
                }
            }
            r
        } else {
            let nh = m.face_area_vectors[f] * (1.0 / m.face_areas[f]);
            let nv = [nh.x, nh.y, nh.z];
            // a `wedge` patch: the gradient tensor is `transform(faceT, G_c)` (`wedgeFvPatchField::evaluate` of the
            // tensor field) and `snGrad = 0.5 deltaCoeffs (cellT.U_c - U_c)`; every other patch keeps the cell gradient
            // and `(U_b - U_c) deltaCoeffs`
            let (sng, mut gb) = match &u.bc[g.patch_of[f - ni]] {
                fv::VectorBc::Wedge { face_t, cell_t } => {
                    let uc = [u.c[o].x, u.c[o].y, u.c[o].z];
                    let mut r = [0.0; 3];
                    for i in 0..3 {
                        r[i] = cell_t[i][0] * uc[0] + cell_t[i][1] * uc[1] + cell_t[i][2] * uc[2];
                    }
                    let h = 0.5 * g.delta[f];
                    let sng = [h * (r[0] - uc[0]), h * (r[1] - uc[1]), h * (r[2] - uc[2])];
                    let mut rt = [[0.0; 3]; 3];
                    for i in 0..3 {
                        for j in 0..3 {
                            let mut acc = 0.0;
                            for l in 0..3 {
                                let mut ft = 0.0;
                                for k in 0..3 {
                                    ft += face_t[i][k] * gr[o][k][l];
                                }
                                acc += ft * face_t[j][l];
                            }
                            rt[i][j] = acc;
                        }
                    }
                    (sng, rt)
                }
                _ => {
                    let sng = (u.b[f - ni] - u.c[o]) * g.delta[f];
                    ([sng.x, sng.y, sng.z], gr[o])
                }
            };
            for j in 0..3 {
                let ndg: f64 = (0..3).map(|k| nv[k] * gb[k][j]).sum();
                for i in 0..3 {
                    gb[i][j] += nv[i] * (sng[j] - ndg);
                }
            }
            match tort {
                None => dev2t(&gb, gamma.b[f - ni]),
                Some((_, tb)) => tensor_laplacian::tensor_and(&tensor_laplacian::scale(gamma.b[f - ni], &tb[f - ni]), &dev2t(&gb, 1.0)),
            }
        };
        let s = m.face_area_vectors[f];
        let s = [s.x, s.y, s.z];
        let mut v = [0.0; 3];
        for j in 0..3 {
            v[j] = (0..3).map(|i| s[i] * xf[i][j]).sum();
        }
        let v = Vector3::new(v[0], v[1], v[2]);
        out[o] += v;
        if f < ni {
            out[m.neighbour[f]] -= v;
        }
    }
    for (c, v) in out.iter_mut().enumerate() {
        *v /= m.cell_volumes[c];
    }
    out
}

/// Solve with ILU(0)-preconditioned BiCGStab after checking OpenFOAM's
/// normalised initial residual against `tol` (`lduMatrix::solver::normFactor`,
/// `small_ = 1e-20`). Returns that initial residual; a residual already below
/// `tol` leaves `x` untouched, as upstream's zero-iteration solve does.
fn solve_krylov(e: &Eqn, x: &mut Vec<f64>, tol: f64) -> f64 {
    let n = x.len();
    let ax = e.ldu.multiply(x);
    let x_ref = x.iter().sum::<f64>() / n.max(1) as f64;
    let mut sum_a = e.ldu.diag.clone();
    for f in 0..e.ldu.n_internal_faces {
        sum_a[e.ldu.neighbour[f]] += e.ldu.lower[f];
        sum_a[e.ldu.owner[f]] += e.ldu.upper[f];
    }
    let nf: f64 = (0..n)
        .map(|c| {
            let t = sum_a[c] * x_ref;
            (ax[c] - t).abs() + (e.source[c] - t).abs()
        })
        .sum::<f64>()
        + 1e-20;
    let ini = (0..n).map(|c| (e.source[c] - ax[c]).abs()).sum::<f64>() / nf;
    if ini < tol {
        return ini;
    }
    let pre = Preconditioner::ilu0(&e.ldu);
    let settings = KrylovSettings {
        tolerance: 1e-13,
        max_iter: 2000,
        restart: 30,
    };
    let (sol, _) = bicgstab(&e.ldu, &e.source, Some(x.as_slice()), &pre, &settings);
    *x = sol;
    ini
}

fn mass_density(v: f64) -> uom::si::f64::MassDensity {
    uom::si::f64::MassDensity::new::<uom::si::mass_density::kilogram_per_cubic_meter>(v)
}

fn velocity(v: f64) -> uom::si::f64::Velocity {
    uom::si::f64::Velocity::new::<uom::si::velocity::meter_per_second>(v)
}

fn temperature(v: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(v)
}

fn specific_energy(v: f64) -> uom::si::f64::AvailableEnergy {
    uom::si::f64::AvailableEnergy::new::<uom::si::available_energy::joule_per_kilogram>(v)
}

fn power_density(v: f64) -> uom::si::f64::VolumetricPowerDensity {
    uom::si::f64::VolumetricPowerDensity::new::<uom::si::volumetric_power_density::watt_per_cubic_meter>(v)
}

impl Phase {
    /// A fluid at uniform state from its case-read fields.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        g: &Geometry,
        name: &str,
        is_liquid: bool,
        thermo: PhaseThermo,
        alpha: Scalar,
        u: Vector,
        t: Scalar,
        he_bc: Vec<Bc>,
        residual_alpha: f64,
        thermo_residual_alpha: f64,
        dispersed_diameter: f64,
    ) -> Self {
        let n = g.n_cells();
        let nf = g.n_faces();
        let calc = vec![Bc::Calculated; g.mesh.patches.len()];
        Self {
            name: name.to_string(),
            is_liquid,
            thermo,
            residual_alpha,
            thermo_residual_alpha,
            above_thermo_residual_alpha: vec![0.0; n],
            below_thermo_residual_alpha: vec![0.0; n],
            dispersed_diameter,
            dispersed_diameter_by_regime: None,
            alpha_rho_mag_u: vec![0.0; n],
            normalized: vec![0.0; n],
            phi: vec![0.0; nf],
            alpha_phi: vec![0.0; nf],
            alpha_rho_phi: vec![0.0; nf],
            rho: Scalar::uniform(g, 0.0, calc.clone()),
            he: Scalar::uniform(g, 0.0, he_bc),
            cp: Scalar::uniform(g, 0.0, calc.clone()),
            mu: Scalar::uniform(g, 0.0, calc.clone()),
            kappa: Scalar::uniform(g, 0.0, calc),
            psi: vec![0.0; n],
            dgdt: vec![0.0; n],
            cont_err: vec![0.0; n],
            cumul_cont_err: 0.0,
            mag_u: vec![0.0; n],
            dispersion: vec![0.0; n],
            dh: vec![0.0; n],
            turbulence: None,
            quality: vec![0.0; n],
            xlm: vec![1e4; n],
            alpha_old: alpha.c.clone(),
            alpha_old_b: alpha.b.clone(),
            rho_old_b: Vec::new(),
            rho_old: vec![0.0; n],
            he_old: vec![0.0; n],
            u_old: u.c.clone(),
            phi_old: vec![0.0; nf],
            alpha,
            u,
            t,
        }
    }

    /// `minXLM` (default 1e-4).
    fn min_xlm(&self) -> f64 {
        1e-4
    }

    /// `maxXLM` (default `1/minXLM`).
    fn max_xlm(&self) -> f64 {
        1e4
    }
}

impl Structure {
    /// The structure's cells (`structure::cells_`, `structure.C:283-286`), on which `structure::correct` sets `Twall` and
    /// `heatFlux` (gh:#1967 round 83).
    #[must_use]
    pub fn with_cells(mut self, cells: Vec<usize>) -> Self {
        self.cells = cells;
        self
    }

    /// A structure from per-cell volume fraction, hydraulic diameter and power
    /// models (`iAact` is each model's interfacial area).
    #[must_use]
    pub fn new(
        g: &Geometry,
        alpha: Vec<f64>,
        dh: Vec<f64>,
        power: Vec<Option<FixedPower>>,
        power_off: Option<PowerOffCriterion>,
    ) -> Self {
        let n = g.n_cells();
        let mut a = Scalar::uniform(g, 0.0, g.default_bcs());
        a.c = alpha;
        a.correct_bc(g);
        let ia_act = power
            .iter()
            .map(|p| {
                p.as_ref()
                    .map(|m| m.interfacial_area().get::<uom::si::reciprocal_length::reciprocal_meter>())
                    .unwrap_or(0.0)
            })
            .collect();
        Self {
            alpha: a,
            dh,
            power_old: power.clone(),
            power,
            ia_act,
            t_act: vec![0.0; n],
            t_wall: vec![0.0; n],
            power_off,
            powered_off: false,
            radial: Vec::new(),
            q_neutronics: vec![0.0; n],
            heat_flux: vec![0.0; n],
            cells: Vec::new(),
            alpha_passive: vec![0.0; n],
            radial_needs_laplacian: false,
            radial_controls: RadialControls::default(),
            heat_exchangers: HeatExchangerSet::default(),
        }
    }

    /// Add the radial power models (gh:#1884): the `structure` constructor's `iAact_ += iter()->iA()` and
    /// `iter()->correctT(Tact_)` (`structure.C:836-837`) for every model, plus the structure's `powerDensityNeutronics`.
    #[must_use]
    pub fn with_radial_power_models(mut self, models: Vec<RadialPowerModel>, q_neutronics: Vec<f64>, needs_laplacian: bool) -> Self {
        for m in &models {
            for &c in m.cells() {
                self.ia_act[c] += m.interfacial_area(c);
                self.t_act[c] = m.surface_temperature(c);
            }
        }
        self.radial = models;
        self.q_neutronics = q_neutronics;
        self.radial_needs_laplacian = needs_laplacian;
        self
    }
}
