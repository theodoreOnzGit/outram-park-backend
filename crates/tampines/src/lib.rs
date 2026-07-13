//! # TAMPINES
//!
//! **T**hermal-hydraulic **A**rtificial-intelligence **M**ulti-**P**hase
//! **IN**tegrated **E**mulator **S**ystem.
//!
//! TAMPINES is the **central thermal-hydraulic framework** of the OUTRAM PARK
//! suite. It owns all fluid flow, thermal-hydraulics, thermophysical
//! properties, heat transfer, balance-of-plant components, humid-air
//! psychrometrics, and multiphase thermal-hydraulics. It is distinct from
//! [`tampines_steam_tables`], which is only the IAPWS-IF97 property library
//! (one of the backends TAMPINES composes).
//!
//! ## What it composes
//!
//! | Piece | Provided by | Role |
//! |---|---|---|
//! | Single-phase liquid thermal-hydraulics | [`tuas_boussinesq_solver`] | Boussinesq single-phase pipe/component flow |
//! | Compressible / two-phase properties | [`outram_park_fork_coolprop`] | CoolProp-derived thermophysical properties |
//! | IAPWS-IF97 steam/water properties | [`tampines_steam_tables`] | Steam-turbine and choked-flow equations |
//! | Finite-volume building blocks | [`outram_foam_basic_lib`] | Tensor algebra, ODE/polynomial solvers, FV operators |
//! | Process control | [`chem_eng_real_time_process_control_simulator`] | PID / transfer-function control loops |
//! | Equipment-model correlations | [`outram_park_fork_dwsim_libs`] | Pipe/valve/heat-exchanger/expander/pump sizing & rating equations |
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** fluid-flow and thermal-hydraulic component models
//!   (pipes, pumps, valves, heat exchangers, steam generators, turbines,
//!   condensers, cooling towers), balance-of-plant composition, humid-air
//!   psychrometrics, and multiphase thermal-hydraulics (HEM, drift-flux, CHF).
//! - **Does NOT belong here:** raw property-table equations (those live in
//!   `tampines-steam-tables` / `outram-park-fork-coolprop`), reactor physics
//!   (`teh-o-prke`, `outram-mc-libs`, `njoy-outram-park-fork`), or GUI /
//!   visualization code (`outram-park-digital-twin-gui`).
//!
//! ## Status
//!
//! **Scaffold only.** This crate is being built out incrementally; see the
//! `op-dt3` epic in the workspace's beads issue tracker for the live module
//! plan and progress.

#![forbid(unsafe_code)]

pub mod compressible;
pub mod critical_flow;
pub mod error;
pub mod fluids;
pub mod heat_transfer;
pub mod hem;
pub mod humid_air;
pub mod single_phase;

pub use error::TampinesError;
pub use fluids::TampinesFluid;
