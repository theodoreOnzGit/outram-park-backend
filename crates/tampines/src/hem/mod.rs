//! Homogeneous Equilibrium Model (HEM) two-phase flow: the steam-water state,
//! and the transient HEM pipe solver.
//!
//! # What is here
//!
//! - [`HemSteamCv`]: thin re-export of [`tampines_steam_tables`]'s validated
//!   HEM control-volume type, the pressure/enthalpy/quality steam-water state
//!   used by [`crate::critical_flow`]'s choked-flow solvers.
//! - [`HemPipe`]: the density-based KNP HEM pipe solver on `[ρ, ρu, ρE]`
//!   (gh #902). The solver lives in `tampines_steam_tables::hem` (maintainer
//!   decision D2, gh #896); it is re-exported here, and this module adds
//!   - [`CoolPropHem`], a CoolProp closure (only `tampines` depends on both
//!     property crates), and
//!   - [`HemEos`], an enum choosing IAPWS-IF97, CoolProp or an ideal gas at
//!     run time.
//!
//!   `HemPipe` here is `HemPipeOf<HemEos>`, so one type covers every closure:
//!
//!   ```no_run
//!   use tampines::hem::*;
//!   use uom::si::f64::*;
//!   use uom::si::{area::square_meter, length::meter, pressure::megapascal,
//!                 thermodynamic_temperature::kelvin, time::second, velocity::meter_per_second};
//!
//!   let geometry = HemPipeGeometry::new(
//!       Length::new::<meter>(1.0), Area::new::<square_meter>(1e-3), 50).unwrap();
//!   let initial = HemInitialCondition::Uniform(PrimitiveState::PressureTemperature {
//!       pressure: Pressure::new::<megapascal>(2.0),
//!       temperature: ThermodynamicTemperature::new::<kelvin>(400.0),
//!       velocity: Velocity::new::<meter_per_second>(0.0),
//!   });
//!   let mut pipe = HemPipe::new(geometry, HemEos::Iapws97, initial, HemNumerics::default()).unwrap();
//!   pipe.run_until(Time::new::<second>(1e-3)).unwrap();
//!   ```
//!
//!   Errors are [`HemError`], which converts into [`crate::TampinesError`]
//!   with `?`.
//!
//! # What is not here
//!
//! Slip (drift-flux) and thermal non-equilibrium (two-fluid) are in
//! [`crate::multiphase_1d`]. The pipe solver is inviscid and frictionless; see
//! `tampines_steam_tables::hem` for its full scope, its deviations from
//! upstream `rhoCentralFoam`, and its V&V record.

mod coolprop_eos;

pub use coolprop_eos::{CoolPropHem, HemEos, COOLPROP_P_RHO_E_REL_TOL};
pub use tampines_steam_tables::hem::{
    pressure_from_rho_e, BreakFlow, FluxLimiter, HemBoundary, HemCellState, HemEquationOfState,
    HemError, HemInitialCondition, HemNumerics, HemPipeGeometry, HemRunSummary, HemSnapshot,
    HemStepReport, HemThermoState, Iapws97Hem, IdealGasHem, PipeEnd, PrimitiveState,
    TimeIntegrator, TimeStepControl, FIXED_STEP_COURANT_LIMIT,
};

/// The generic solver, `HemPipeOf<E>` for any closure `E`; use it to run with
/// a concrete closure type (e.g. `HemPipeOf<CoolPropHem>`).
pub use tampines_steam_tables::hem::HemPipe as HemPipeOf;

/// The HEM pipe solver with its closure chosen at run time by [`HemEos`]
/// (IAPWS-IF97, CoolProp or ideal gas). See `tampines_steam_tables::hem::HemPipe`
/// for the methods.
pub type HemPipe = HemPipeOf<HemEos>;

/// The HEM two-phase steam/water control volume: pressure, specific
/// enthalpy, specific entropy, quality, and derived properties (viscosity,
/// speed of sound, critical mass flux, ...).
///
/// Alias for [`tampines_steam_tables`]'s `TampinesSteamTableCV` -- see that
/// type's own documentation for its full getter/setter surface and
/// construction paths ((p,h), (p,s), (h,s), (T,p,x), saturation-line
/// constructors).
pub use tampines_steam_tables::prelude::TampinesSteamTableCV as HemSteamCv;
