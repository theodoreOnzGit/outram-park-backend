//! Homogeneous Equilibrium Model (HEM) transient pipe flow: a density-based
//! KNP central-upwind solver on `W = [ρ, ρu, ρE]` (epic gh #896, issue
//! gh #902).
//!
//! # What this module answers
//!
//! *How does a 1-D pipe of water (or any fluid with an equation of state)
//! depressurise, flash and discharge, when both phases share one velocity, one
//! pressure and one temperature?* That is the 3-equation HEM rung, below
//! drift-flux (slip) and two-fluid (thermal non-equilibrium), which live in
//! `tampines::multiphase_1d`.
//!
//! # Where it sits (maintainer decision D2, 2026-10-16)
//!
//! HEM is a thermodynamic model, and Edwards–O'Brien is this crate's
//! critical-flow validation case, so the solver lives **here**, in
//! `tampines-steam-tables`. `tampines` re-exports it as `tampines::hem` and adds
//! a CoolProp closure, because only `tampines` depends on both property crates.
//! No dependency edge is added either way.
//!
//! # Entry points
//!
//! - [`HemPipe`]: the solver. `HemPipe::new(geometry, eos, initial, numerics)`,
//!   then [`HemPipe::set_boundary`], [`HemPipe::step`] or
//!   [`HemPipe::run_until`], then [`HemPipe::snapshot`]. Its type parameter
//!   defaults to [`Iapws97Hem`], so `HemPipe` alone means water.
//! - Closures: [`Iapws97Hem`] (IAPWS-IF97 water/steam) and [`IdealGasHem`]
//!   (the Sod limit), both implementing the compile-time trait
//!   [`HemEquationOfState`].
//! - Configuration: [`HemPipeGeometry`], [`HemInitialCondition`] /
//!   [`PrimitiveState`], [`HemBoundary`] / [`PipeEnd`], [`HemNumerics`]
//!   (with [`TimeStepControl`], [`TimeIntegrator`] and the basic-lib
//!   [`FluxLimiter`]).
//! - Errors: [`HemError`]. Nothing is clamped: a negative density or pressure
//!   is an error and the step is undone.
//!
//! # Physics, and what is not here
//!
//! Inviscid, adiabatic, frictionless, horizontal, equilibrium two-phase (both
//! phases at `T_sat(p)` inside the dome, no slip). The sound speed is the
//! homogeneous-**equilibrium** one throughout ([`eos`] module doc). Not here:
//! wall friction, gravity, heat transfer, slip, metastable liquid.
//!
//! # V&V
//!
//! Methodology and results: `verification_and_validation/hem_pipe_vs_exact_riemann_and_edwards.md`.
//! Tests: `tests/hem_pipe_verification.rs` (Sod, positivity, Courant,
//! conservation, pinned defaults, the HEM water double rarefaction) and
//! `tests/hem_pipe_edwards.rs` (Edwards–O'Brien against GS-1).
//!
//! > Research, education and V&V only. Not for nuclear facility operation,
//! > reactor control, licensing or safety-critical decisions. See
//! > `RESPONSIBLE_USE.md`.

pub mod eos;
mod error;
mod iapws97;
mod pipe;

#[cfg(test)]
mod tests;

pub use eos::{pressure_from_rho_e, BreakFlow, HemEquationOfState, HemThermoState, IdealGasHem};
pub use error::HemError;
pub use iapws97::{Iapws97Hem, P_RHO_E_REL_TOL};
pub use outram_foam_basic_lib::limiters::FluxLimiter;
pub use pipe::{
    HemBoundary, HemCellState, HemInitialCondition, HemNumerics, HemPipe, HemPipeGeometry,
    HemRunSummary, HemSnapshot, HemStepReport, PipeEnd, PrimitiveState, TimeIntegrator,
    TimeStepControl, FIXED_STEP_COURANT_LIMIT,
};
