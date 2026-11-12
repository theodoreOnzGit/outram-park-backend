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
//! - **MULES** (`solveAlphaMULES.H`, `myMULES`, 1,765 lines). A case that
//!   selects `MULES` is run with `implicitUpwind` and the caller is told
//!   ([`TwoPhaseCase::deviations`]).
//! - `momentumMode cellCentered` (`pEqn_2p.H`) and partial-elimination modes
//!   other than `implicit`; `porousInterfaceSharpness != 0`.
//! - Turbulence other than `laminar`; structure tortuosity/anisotropy, local
//!   frames, pumps, heat exchangers, passive structures; `fvOptions`.
//! - Closure selections other than those listed on [`TwoPhaseModels`]; the
//!   wall-boiling mass split `dmdtW` (no ported model writes it); the
//!   `twoParameters` regime map; `byRegime` for anything but dispersion and
//!   contact partition.
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

pub mod case;
pub mod fv;
pub mod one_phase_fc;
mod pbicgstab;
pub mod thermo;

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
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::chf::{
    FlowEnhancementFactor, FlowEnhancementInputs, SuppressionFactor, SuppressionInputs,
};
use crate::genfoam::thermal_hydraulics::closures::heat_transfer::fs_htc::{
    FsForcedConvectionHtc, PoolBoilingHtc,
};
use crate::genfoam::thermal_hydraulics::closures::interfacial::regime_map::RegimeMap1D;
use crate::genfoam::thermal_hydraulics::closures::phase_change::{
    LatentHeatModel, PhaseChangeRateModel, SaturationModel,
};
use crate::genfoam::thermal_hydraulics::structure::power_model::FixedPower;
use crate::genfoam::thermal_hydraulics::structure::power_off::PowerOffCriterion;
use crate::genfoam::thermal_hydraulics::units::ReynoldsNumber;

pub use case::TwoPhaseCase;
use fv::{Bc, Eqn, Geometry, Scalar, Vector};
pub use thermo::PhaseThermo;

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
    /// Interfacial htc per fluid.
    pub ff_htc: [Option<FfHtcModel>; 2],
    /// `virtualMassCoefficientModel constant` value, if present.
    pub virtual_mass: Option<f64>,
    /// `regimeMapModels` (one `oneParameter` map) and the index of the fluid
    /// whose `normalized` phase fraction is its parameter.
    pub regime_map: Option<(RegimeMap1D, usize)>,
    /// Dispersion of fluid 1 (`dispersionModel`: `1` = fluid 1 dispersed).
    pub dispersion_fluid1: ByRegime,
    /// Contact partition: the fluid index the model is given for and the
    /// model; the other fluid takes `1 - f` (`complementary`).
    pub contact_partition: (usize, ByRegime),
    /// `interfacialAreaDensityModel spherical` `cutoffAlpha` (default 0.9).
    pub spherical_cutoff_alpha: f64,
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

/// PIMPLE and solver controls (`fvSolution`).
#[derive(Debug, Clone, PartialEq)]
pub struct TwoPhaseControls {
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
    /// set it. The skipped step still advances time, runs the courant/`maxDeltaT` update and the
    /// port's unconditional `calcCumulContErrs`.
    pub stale_pimple_converged_flag: bool,
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
    /// `dispersedDiameterModel constant` value [m].
    pub dispersed_diameter: f64,
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
    /// Active-structure surface temperature [K] (`Tact`, `Twall`).
    pub t_act: Vec<f64>,
    /// `powerOffCriterionModel`.
    pub power_off: Option<PowerOffCriterion>,
    /// Whether power has been switched off.
    pub powered_off: bool,
    power_old: Vec<Option<FixedPower>>,
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
    /// `customPimpleControl::converged_`, kept across steps only when
    /// [`TwoPhaseControls::stale_pimple_converged_flag`] is set.
    pimple_converged: bool,
    /// ESI `Time::writeTimeIndex_` (`adjustableRunTime` only).
    write_time_index: i64,
    /// Interfacial area density [1/m].
    pub interfacial_area: Vec<f64>,
    /// Phase-change rate fluid 1 → fluid 2 [kg/(m^3 s)].
    pub dmdt: Vec<f64>,
    /// Latent heat [J/kg].
    pub latent_heat: Vec<f64>,
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
        let geom = Geometry::new(case.mesh.clone());
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
            rho_mix: case.p.clone(),
            phi: vec![0.0; nf],
            fluids: case.fluids.clone(),
            structure: case.structure.clone(),
            models: case.models.clone(),
            controls: case.controls.clone(),
            time_controls: case.time_controls.clone(),
            t_interface: vec![0.0; n],
            t_interface_old: vec![0.0; n],
            t_interface_raw0: vec![0.0; n],
            corr_in_step: 0,
            pimple_converged: false,
            write_time_index: 0,
            interfacial_area: vec![0.0; n],
            dmdt: vec![0.0; n],
            latent_heat: vec![0.0; n],
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
        for k in 0..2 {
            s.fluids[k].alpha.correct_bc(&s.geom);
        }
        s.update_normalized();
        // thermo of the initial state, then fluxes (fluid::initAlphaPhis).
        for k in 0..2 {
            s.thermo_correct(k, true);
            let u = s.fluids[k].u.clone();
            s.fluids[k].phi = fv::flux(&s.geom, &u);
            let af = fv::interp_linear(&s.geom, &s.fluids[k].alpha);
            let rf = fv::interp_linear(&s.geom, &s.fluids[k].rho);
            let ap: Vec<f64> = (0..nf).map(|f| af[f] * s.fluids[k].phi[f]).collect();
            s.fluids[k].alpha_rho_phi = (0..nf).map(|f| rf[f] * ap[f]).collect();
            s.fluids[k].alpha_phi = ap;
            s.fluids[k].mag_u = s.fluids[k].u.c.iter().map(|v| v.mag()).collect();
        }
        s.phi = (0..nf)
            .map(|f| s.fluids[0].alpha_phi[f] + s.fluids[1].alpha_phi[f])
            .collect();
        s.update_mixture_density();
        // p on patches is `calculated`: p = p_rgh + rho gh.
        for f in ni..nf {
            if s.geom.active[f] {
                s.p.b[f - ni] = s.p_rgh.b[f - ni] + s.rho_mix.b[f - ni] * s.ghf[f];
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
    /// non-finite.
    pub fn step(&mut self) -> Result<StepReport, crate::error::AppBuilderError> {
        let n = self.geom.n_cells();
        for k in 0..2 {
            let f = &mut self.fluids[k];
            f.alpha_old = f.alpha.c.clone();
            f.rho_old = f.rho.c.clone();
            f.alpha_old_b = f.alpha.b.clone();
            f.rho_old_b = f.rho.b.clone();
            f.he_old = f.he.c.clone();
            f.u_old = f.u.c.clone();
            f.phi_old = f.phi.clone();
        }
        if self.p_old_stored {
            self.p_old = self.p.c.clone();
        }
        self.t_interface_old = self.t_interface.clone();
        self.structure.power_old = self.structure.power.clone();
        self.time += self.dt;
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
            }
            self.corr_in_step = corr;
            self.correct_models();
            let (p_res, n_piso) = self.correct_fluid_mechanics()?;
            let _ = n_piso;
            report.p_rgh_initial_residual = p_res;
            report.h1_initial_residual = self.correct_energy();
            report.outer_iterations = corr;
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
        self.calc_cumul_cont_errs();
        let _ = n;
        report.courant = self.courant();
        if self.time_controls.adjust_time_step {
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
    fn correct_models(&mut self) {
        let n = self.geom.n_cells();
        // oneParameter::correct
        if let Some((map, k)) = &self.models.regime_map {
            for c in 0..n {
                self.regime_weights[c] = map.regime_weights(self.fluids[*k].normalized[c]);
            }
        }
        self.correct_ff_pair();
        for k in 0..2 {
            self.correct_fs_pair(k);
        }
        self.correct_two_phase_drag_factor();
    }

    /// `FFPair::correct` (FFPair.C:441-730).
    fn correct_ff_pair(&mut self) {
        let n = self.geom.n_cells();
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
                    d * self.fluids[k].dispersed_diameter + (1.0 - d) * dhs
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
            let cut = self.models.spherical_cutoff_alpha;
            self.interfacial_area[c] = a_sum
                * if a < cut {
                    6.0 * a / self.dh_dispersed[c]
                } else {
                    6.0 * a / self.dh_dispersed[c] * (1.0 - a) / (1.0 - cut)
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
        // energy-conservative heSources (fw = 0: no ported model writes dmdtW)
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
                let fr = d / d0;
                let dl = d * l;
                self.he_src[0].su_sp[c] = (dl - fr * q2) / self.fluids[0].he.c[c];
                self.he_src[1].su_sp[c] = (dl - fr * q1) / self.fluids[1].he.c[c];
                zeroed[c] = true;
            }
            // fvm::Sp(dmdt12, he1) - dmdt21 he1I  /  fvm::Sp(dmdt21, he2) - dmdt12 he2I
            let (d12, d21) = (d.max(0.0), (-d).max(0.0));
            self.he_src[0].sp[c] += d12;
            self.he_src[0].su[c] += d21 * he1i;
            self.he_src[1].sp[c] += d21;
            self.he_src[1].su[c] += d12 * he2i;
        }
    }

    /// `FSPair::correct` for fluid `k` (FSPair.C:285-456).
    fn correct_fs_pair(&mut self, k: usize) {
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
                self.htc_fs[k][c] = self.fs_htc_value(k, o, c, model);
            }
        }
        for c in 0..n {
            self.htc_fs[k][c] *= self.contact[k][c];
        }
    }

    /// One cell of a fluid-structure htc model.
    fn fs_htc_value(&self, k: usize, o: usize, c: usize, model: FsHtcModel) -> f64 {
        let fl = &self.fluids[k];
        let pr = fl.cp.c[c] * fl.mu.c[c] / fl.kappa.c[c];
        let tw = self.structure.t_act[c];
        let tf = fl.t.c[c];
        let fc = |m: FsForcedConvectionHtc| {
            m.heat_transfer_coefficient(
                ReynoldsNumber::new::<ratio>(self.re_fs[k][c]),
                Ratio::new::<ratio>(pr),
                ThermalConductivity::new::<watt_per_meter_kelvin>(fl.kappa.c[c]),
                Length::new::<meter>(fl.dh[c]),
                Some((temperature(tw), temperature(tf))),
            )
            .get::<watt_per_square_meter_kelvin>()
        };
        match model {
            FsHtcModel::Nusselt(m) => fc(m),
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
                fc(forced_convection) * f + pb * s
            }
        }
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
        let u_eqns = [self.correct_u_eqn(0), self.correct_u_eqn(1)];
        let r = self.correct_pressure(&u_eqns);
        self.correct_cont_errs();
        Ok(r)
    }

    /// `solveAlphaImplicitUpwind.H` for `solvePhase`, the other fluid by
    /// continuity, then `correctContErrs` and the normalised fractions
    /// (`alphaEqns_2p.H`).
    fn correct_alpha(&mut self) {
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
                    eqn.ldu.diag[g.mesh.owner[f]] += fl.phi[f];
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
                    eqn.ldu.diag[g.mesh.owner[f]] += fl.phi[f];
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
        fl.alpha.correct_bc(&g);
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
    /// elimination configuration (no `fvm::ddt`, no `Sp(Kd12)`), laminar.
    fn correct_u_eqn(&self, k: usize) -> VecEqn {
        let g = &self.geom;
        let n = g.n_cells();
        let nf = g.n_faces();
        let ni = g.n_internal();
        let fl = &self.fluids[k];
        let ot = &self.fluids[1 - k];
        let mut e = VecEqn::new(g);
        // fvm::div(alphaRhoPhi, U)  (Gauss upwind; U zeroGradient)
        let zg = Scalar::uniform(g, 0.0, g.default_bcs());
        e.eqn.div_upwind(g, &fl.alpha_rho_phi, &zg);
        // - fvm::laplacian(alpha rho nuEff, U) - fvc::div(alpha rho nuEff dev2(T(grad U)))
        let mut arn = Scalar::uniform(g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
        for c in 0..n {
            arn.c[c] = fl.alpha.c[c] * fl.mu.c[c];
        }
        for i in 0..(nf - ni) {
            arn.b[i] = fl.alpha.b[i] * fl.mu.b[i];
        }
        let gamma = fv::interp_linear(g, &arn);
        e.eqn.neg_laplacian(g, &gamma, &zg);
        let dev2 = dev2_div(g, &fl.u, &arn);
        for c in 0..n {
            let v = g.mesh.cell_volumes[c];
            e.src[0][c] += v * dev2[c].x;
            e.src[1][c] += v * dev2[c].y;
            e.src[2][c] += v * dev2[c].z;
        }
        // fvm::Sp((1/3) tr(Kd), U) + (dev(Kd) & U); isotropic Kd ⇒ dev = 0
        e.eqn.sp(g, &self.kd_fs[k]);
        // virtual mass
        if self.models.virtual_mass.is_some() {
            let mut vm = VecEqn::new(g);
            let ones = vec![1.0; n];
            for comp in 0..3 {
                let old: Vec<f64> = fl.u_old.iter().map(|u| cmp(*u, comp)).collect();
                for c in 0..n {
                    vm.src[comp][c] += g.mesh.cell_volumes[c] / self.dt * old[c];
                }
            }
            for c in 0..n {
                vm.eqn.ldu.diag[c] += g.mesh.cell_volumes[c] / self.dt * ones[c];
            }
            vm.eqn.div_upwind(g, &fl.phi, &zg);
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
                vm.src[0][c] += v * r.x;
                vm.src[1][c] += v * r.y;
                vm.src[2][c] += v * r.z;
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
        // refsAndControls_2p.H: alphaf by upwind (implicitUpwind) on phi_.
        let af: [Vec<f64>; 2] = [
            fv::interp_upwind(&g, &self.fluids[0].alpha, &self.phi),
            fv::interp_upwind(&g, &self.fluids[1].alpha, &self.phi),
        ];
        let moving_af: Vec<f64> = (0..nf).map(|f| af[0][f] + af[1][f]).collect();
        // time-derivative coefficients
        let ar_by_dt = |fl: &Phase, alpha: &[f64], rho: &[f64], ab: &[f64], rb: &[f64]| {
            let mut s = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                s.c[c] = alpha[c].max(fl.residual_alpha) * rho[c];
            }
            for i in 0..(nf - ni) {
                s.b[i] = ab[i].max(fl.residual_alpha) * rb[i];
            }
            let v = fv::interp_linear(&g, &s);
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
        let kdf = fv::interp_linear(&g, &kd12);
        let afk: Vec<Vec<f64>> = (0..2)
            .map(|k| {
                let mut a = Scalar::uniform(&g, 0.0, g.default_bcs());
                a.c = u_eqns[k].eqn.a(&g);
                a.correct_bc(&g);
                let ai = fv::interp_linear(&g, &a);
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
            // calcPhiS_2p.H
            let mut rmix = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            rmix.c = self.rho_mix.c.clone();
            rmix.b = self.rho_mix.b.clone();
            let sng = fv::sn_grad(&g, &rmix);
            let r1f = fv::interp_linear(&g, &self.fluids[0].rho);
            let r2f = fv::interp_linear(&g, &self.fluids[1].rho);
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
                let mut hv = Vector::uniform(&g, Vector3::ZERO);
                hv.c = h;
                hv.correct_bc(&g);
                let fh = fv::flux(&g, &hv);
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
            // calcComp_2p.H
            let p_rgh_0 = self.p_rgh.c.clone();
            let mut comp_expl = [vec![0.0; n], vec![0.0; n]];
            let mut comp_c = [vec![0.0; n], vec![0.0; n]];
            for k in 0..2 {
                let fl = &self.fluids[k];
                let d_arp = fv::div(&g, &fl.alpha_rho_phi);
                let d_ap = fv::div(&g, &fl.alpha_phi);
                let sign = if k == 0 { 1.0 } else { -1.0 };
                for c in 0..n {
                    let ddt_ar = (fl.alpha.c[c] * fl.rho.c[c] - fl.alpha_old[c] * fl.rho_old[c]) / dt;
                    let ddt_a = (fl.alpha.c[c] - fl.alpha_old[c]) / dt;
                    let mut e = (ddt_ar + d_arp[c] - (ddt_a + d_ap[c]) * fl.rho.c[c]) / fl.rho.c[c];
                    if self.models.phase_change.is_some() {
                        e += sign * self.dmdt[c] / fl.rho.c[c];
                    }
                    comp_expl[k][c] = e;
                    comp_c[k][c] = fl.alpha.c[c] * fl.psi[c] / fl.rho.c[c];
                }
            }
            // pEqn = comp1 + comp2 + div(phiHbyA) - laplacian(rAUf, p_rgh)
            let mut peqn = Eqn::new(&g);
            peqn.neg_laplacian(&g, &rau_mix, &self.p_rgh);
            let dphi = fv::div(&g, &phi_hbya_mix);
            for c in 0..n {
                let v = g.mesh.cell_volumes[c];
                let cc = comp_c[0][c] + comp_c[1][c];
                peqn.ldu.diag[c] += v * cc / dt;
                peqn.source[c] += v * cc / dt * p_rgh_0[c];
                peqn.source[c] -= v * (comp_expl[0][c] + comp_expl[1][c] + dphi[c]);
            }
            let mut x = self.p_rgh.c.clone();
            let res = solve_krylov(&peqn, &mut x, self.controls.p_rgh_tolerance);
            if corr == 1 {
                first_res = res;
            }
            self.p_rgh.c = x;
            self.p_rgh.correct_bc(&g);
            // fluxes
            let p_flux = Eqn::laplacian_flux(&g, &rau_mix, &self.p_rgh);
            for f in 0..nf {
                if !g.active[f] {
                    continue;
                }
                self.phi[f] = phi_hbya_mix[f] + p_flux[f];
                let msf = p_flux[f] / rau_mix[f];
                for k in 0..2 {
                    self.fluids[k].phi[f] = phi_hbya[k][f] + star[k][f] * msf;
                }
            }
            for k in 0..2 {
                for c in 0..n {
                    self.fluids[k].dgdt[c] =
                        -(comp_expl[k][c] + comp_c[k][c] * (self.p_rgh.c[c] - p_rgh_0[c]) / dt);
                }
            }
            // reconstructU_2p.H (porousInterfaceSharpness 0)
            for k in 0..2 {
                let u = fv::reconstruct(&g, &self.fluids[k].phi);
                self.fluids[k].u.c = u;
                self.fluids[k].u.correct_bc(&g);
                self.fluids[k].mag_u = self.fluids[k].u.c.iter().map(|v| v.mag()).collect();
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
            }
        }
        if !self.p_old_stored {
            // `fvc::ddt(p_)` creates `p.oldTime()` as a copy of the current field.
            self.p_old = self.p.c.clone();
            self.p_old_stored = true;
        }
        let dpdt: Vec<f64> = (0..n)
            .map(|c| ((self.p.c[c] - self.p_old[c]) / self.dt).max(-self.controls.max_dpdt).min(self.controls.max_dpdt))
            .collect();
        let mut res1 = 0.0;
        for k in 0..2 {
            let fl = &self.fluids[k];
            let mut e = Eqn::new(&g);
            e.ddt(&g, &mul(&fl.alpha.c, &fl.rho.c), &mul(&fl.alpha_old, &fl.rho_old), &fl.he_old, self.dt);
            e.div_upwind(&g, &fl.alpha_rho_phi, &fl.he);
            // - laplacian(interpolate(alpha) interpolate(alphaEff), he)
            let mut ae = Scalar::uniform(&g, 0.0, vec![Bc::Calculated; g.mesh.patches.len()]);
            for c in 0..n {
                ae.c[c] = fl.kappa.c[c] / fl.cp.c[c];
            }
            for i in 0..ae.b.len() {
                ae.b[i] = if fl.cp.b[i] > 0.0 { fl.kappa.b[i] / fl.cp.b[i] } else { 0.0 };
            }
            let a_f = fv::interp_linear(&g, &fl.alpha);
            let ae_f = fv::interp_linear(&g, &ae);
            let gam: Vec<f64> = a_f.iter().zip(&ae_f).map(|(a, b)| a * b).collect();
            e.neg_laplacian(&g, &gam, &fl.he);
            // alpha rho (U & g) + alpha dpdt
            let mut rhs = vec![0.0; n];
            for c in 0..n {
                rhs[c] = fl.alpha.c[c] * fl.rho.c[c] * fl.u.c[c].dot(self.g);
                if self.controls.dpdt {
                    rhs[c] += fl.alpha.c[c] * dpdt[c];
                }
            }
            e.su(&g, &rhs);
            // contErr SuSp
            let ce: Vec<f64> = fl.cont_err.iter().map(|v| -v).collect();
            e.su_sp(&g, &ce, &fl.he.c);
            // FFPair heSources (interfacial + phase change)
            let src = &self.he_src[k];
            if src.su.len() == n {
                e.su(&g, &src.su);
                e.sp(&g, &src.sp);
                e.su_sp(&g, &src.su_sp, &fl.he.c);
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
            let prev = fl.he.c.clone();
            let mut x = prev.clone();
            let (sweeps, ini, _) = super::one_phase::smooth_solve_sym_gauss_seidel(
                &e.ldu,
                &e.source,
                &mut x,
                SolverSettings { tolerance: self.controls.h_tolerance, max_iter: 1000 },
            );
            if sweeps == 0 && self.controls.h_min_iter > 0 {
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
                for c in 0..n {
                    let pos = fl.normalized[c] - fl.thermo_residual_alpha > 0.0;
                    let v = if pos { fl.he.c[c] } else { hei[c] };
                    fl.he.c[c] = v.max(100.0);
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
            }
        }
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
            } else if let Some(t) = fl.thermo.t_from_he(fl.he.c[c], p.c[c], fl.t.c[c]) {
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

    /// `twoPhase::calcCumulContErrs` (final iteration): the volume-weighted
    /// time integral of each fluid's continuity error [kg/m^3].
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

/// A vector momentum equation: shared LDU coefficients, one source per
/// component (as OpenFOAM's `fvVectorMatrix`).
#[derive(Debug, Clone)]
struct VecEqn {
    eqn: Eqn,
    src: [Vec<f64>; 3],
}

impl TwoPhaseSolver {
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
        let n = g.n_cells();
        Self {
            eqn: Eqn::new(g),
            src: [vec![0.0; n], vec![0.0; n], vec![0.0; n]],
        }
    }

    fn su_sp(&mut self, g: &Geometry, coeff: &[f64], u: &[Vector3]) {
        for c in 0..g.n_cells() {
            let v = g.mesh.cell_volumes[c];
            self.eqn.ldu.diag[c] += v * coeff[c].max(0.0);
            let e = v * coeff[c].min(0.0);
            self.src[0][c] -= e * u[c].x;
            self.src[1][c] -= e * u[c].y;
            self.src[2][c] -= e * u[c].z;
        }
    }

    fn scale_rows(&mut self, g: &Geometry, s: &[f64]) {
        self.eqn.scale_rows(g, s);
        for comp in 0..3 {
            for c in 0..g.n_cells() {
                self.src[comp][c] *= s[c];
            }
        }
    }

    fn add(&mut self, o: &VecEqn) {
        self.eqn.add(&o.eqn);
        for comp in 0..3 {
            for (a, b) in self.src[comp].iter_mut().zip(&o.src[comp]) {
                *a += b;
            }
        }
    }

    /// `H()` per unit volume.
    fn h(&self, g: &Geometry, u: &Vector) -> Vec<Vector3> {
        let hs: Vec<Vec<f64>> = (0..3)
            .map(|comp| {
                let x: Vec<f64> = u.c.iter().map(|v| cmp(*v, comp)).collect();
                self.eqn.h(g, &self.src[comp], &x)
            })
            .collect();
        (0..g.n_cells()).map(|c| Vector3::new(hs[0][c], hs[1][c], hs[2][c])).collect()
    }
}

fn cmp(v: Vector3, i: usize) -> f64 {
    match i {
        0 => v.x,
        1 => v.y,
        _ => v.z,
    }
}

fn mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x * y).collect()
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
    let m = &g.mesh;
    let n = g.n_cells();
    let ni = g.n_internal();
    let nf = g.n_faces();
    type T3 = [[f64; 3]; 3];
    let mut gr = vec![[[0.0; 3]; 3]; n];
    let face_u = |f: usize| -> Vector3 {
        if f < ni {
            let w = g.weights[f];
            u.c[m.owner[f]] * w + u.c[m.neighbour[f]] * (1.0 - w)
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
            let a = dev2t(&gr[o], gamma.c[o]);
            let b = dev2t(&gr[m.neighbour[f]], gamma.c[m.neighbour[f]]);
            let mut r = [[0.0; 3]; 3];
            for i in 0..3 {
                for j in 0..3 {
                    r[i][j] = w * a[i][j] + (1.0 - w) * b[i][j];
                }
            }
            r
        } else {
            let nh = m.face_area_vectors[f] * (1.0 / m.face_areas[f]);
            let nv = [nh.x, nh.y, nh.z];
            let sng = (u.b[f - ni] - u.c[o]) * g.delta[f];
            let sng = [sng.x, sng.y, sng.z];
            let mut gb = gr[o];
            for j in 0..3 {
                let ndg: f64 = (0..3).map(|k| nv[k] * gr[o][k][j]).sum();
                for i in 0..3 {
                    gb[i][j] += nv[i] * (sng[j] - ndg);
                }
            }
            dev2t(&gb, gamma.b[f - ni])
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
            dispersed_diameter,
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
            power_off,
            powered_off: false,
        }
    }
}
