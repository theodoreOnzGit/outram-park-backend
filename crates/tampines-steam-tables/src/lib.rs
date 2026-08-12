//! # TAMPINES Steam Tables
//!
//! In-house Rust implementation of the IAPWS-IF97 industrial formulation for
//! the thermodynamic and transport properties of water and steam, used by the
//! TAMPINES (Thermo-hydraulic Artificial-intelligence Multi-Phase INtegrated
//! Emulator System) solver. Unlike the upstream `rust-steam` library it draws
//! from, every public property function takes and returns `uom` dimensioned
//! quantities (SI units) rather than bare `f64`.
//!
//! ## Organisation
//!
//! Properties are grouped by IAPWS-IF97 region:
//!
//! - Region 1 — subcooled liquid (273.15–623.15 K, up to 100 MPa, below saturation)
//! - Region 2 — vapour / superheated steam (incl. a metastable subregion)
//! - Region 3 — single-phase near-critical liquid/vapour + supercritical fluid
//! - Region 4 — vapour-liquid equilibrium (the saturation line)
//! - Region 5 — ultra-high-temperature steam (> 800 °C, up to ~2273 K)
//!
//! Forward equations are `(p,T)` / `(v,T)` flashes; the backward (inverse)
//! equations solve from `(p,h)`, `(p,s)`, or `(h,s)`. The user-facing
//! region-dispatch entry points (both a functional API and the
//! `TampinesSteamTableCV` control-volume type) live in [`interfaces`]. Transport
//! and miscellaneous properties are in [`dynamic_viscosity`],
//! [`thermal_conductivity`], [`surface_tension`], and [`dielectric_constant`].
//! Nozzle / turbine and HEM choked-flow equations are in
//! [`steam_turbine_equations`].
//!
//! All quantities are SI: pressure in Pa, temperature in K, specific enthalpy
//! and energy in J/kg, specific entropy and heat capacity in J/(kg·K), specific
//! volume in m³/kg, density in kg/m³.

/// allows for easy importing as with most rust
/// crates.
pub mod prelude;
#[warn(missing_docs)]
/// constants for the steam table calculations
pub mod constants;

/// region 1
///
/// Temperature from 273.15 to 623.15 K
/// Pressure from 0 to 100 MPa
///
/// Up to the saturation line.
/// This I believe is subcooled liquid region
pub mod region_1_subcooled_liquid;

/// region 2
///
/// vapour region
pub mod region_2_vapour;

/// region 3
///
/// single phase liquid and vapour
/// region, also includes supercritical region
/// and critical point
///
/// auxilliary equation for region 2 and 3 are also put here
///
///
pub mod region_3_single_phase_plus_supercritical_steam;

/// region 4
///
/// two phase region
/// where vapour liq equilibrium exists
pub mod region_4_vap_liq_equilibrium;

/// region 5
///
/// superheated steam region (ultra high temp)
pub mod region_5_steam_at_800_plus_degc;

/// backward equations ph boundary equations
/// overall equation
pub mod backward_eqn_ph_region_1_to_4;

/// backward equations ps boundary equations
/// overall equation
pub mod backward_eqn_ps_region_1_to_4;

/// backward equations hs boundary equations
/// overall equation
pub mod backward_eqn_hs_region_1_to_4;

/// dynamic viscosity calcs
pub mod dynamic_viscosity;

/// thermal conductivity calcs
pub mod thermal_conductivity;

/// public facing interfaces where the user
/// simply inputs pressure and temperature
/// or pressure and enthalpy etc
/// and gets all the required data automatically
///
/// the logic for splitting between regions is
/// mostly here
pub mod interfaces;

/// surface tension
/// important for boiling
pub mod surface_tension;

/// dielectric constant
pub mod dielectric_constant;

/// useful equations for steam turbines
/// These include nozzles, impulse turbines
/// and reaction turbines at some steady
/// state,
/// as well as angular momentum balance
pub mod steam_turbine_equations;

/// reference openfoam algorithms which will be combined with steam
/// tables for solving simple two phase flow problems
pub mod openfoam_algorithms;

/// Re-export of the 1-D compressible PIMPLE pipe array solver
/// (`TampinesSteamArray`) and its error type (`TampinesSteamArrayError`) from
/// [`openfoam_algorithms::rhoPimpleFoam`], surfaced at the crate root for
/// convenience. `TampinesSteamArray` backs each finite-volume cell with an
/// IAPWS-IF97 `(p,h)` flash so a 1-D pipe can carry two-phase steam-water flow.
pub use openfoam_algorithms::rhoPimpleFoam::{
    AdvectionTerminalState, SolverMode, TampinesSteamArray, TampinesSteamArrayError,
};

// pool boiling code for use within the fhr sim v1
mod fhr_sim_debugging_tests;
