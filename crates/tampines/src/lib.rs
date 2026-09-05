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
//! ## Reaching real time: evaluate a surrogate, not the full model
//!
//! This crate holds the physics a transient evaluates *most often* — fluid
//! properties, HEM critical flow, drift-flux closures, packed-bed conjugate
//! heat transfer, the KTA and ZBS correlations. That makes it the inner loop
//! of every coupled run, and a digital twin at interactive rates cannot afford
//! a full property or closure solve per tick per node.
//!
//! **The intended answer is a surrogate, and it is [`raffles`].** `raffles` is
//! this workspace's port of RAVEN's uncertainty-quantification core, where a
//! reduced-order model is a first-class object: sample the expensive model
//! over its state space once, fit a cheap stand-in, evaluate that per tick.
//!
//! This is written down because it is not the obvious move. The natural
//! reaction to a slow closure is to put a lookup table next to it, and a dozen
//! bespoke tables scattered through this crate is precisely the outcome to
//! avoid — they drift from the correlations they approximate, and none of them
//! carries an error estimate. Prefer a `raffles` surrogate over a local table,
//! and if you find yourself writing a table anyway, treat it as a stopgap and
//! say so at the call site.
//!
//! Two constraints on any implementation:
//!
//! - **The surrogate must stay Android-clean.** Dense linear algebra for
//!   fitting comes from the pure-Rust `faer` already in the workspace
//!   dependencies — never a system BLAS or `ndarray-linalg`, per the
//!   workspace's Android/Termux rule.
//! - **A surrogate without an error bound is not usable here.** These feed
//!   safety-relevant transients, and a fast wrong answer is worse than a slow
//!   right one.
//!
//! The ROM layer itself does not exist yet — `raffles::surrogate` is currently
//! a stub. Tracked in the workspace beads as `op-38my`; the dependency is
//! wired ahead of it so the intent is visible from the manifest.
//!
//! ## Status
//!
//! **Scaffold only.** This crate is being built out incrementally; see the
//! `op-dt3` epic in the workspace's beads issue tracker for the live module
//! plan and progress.

#![forbid(unsafe_code)]

pub mod balance_of_plant;
pub mod components;
pub mod compressible;
pub mod cooling_tower;
pub mod critical_flow;
pub mod error;
pub mod fluids;
pub mod gas_phase;
pub mod heat_transfer;
pub mod hem;
pub mod humid_air;
pub mod multiphase_1d;
pub mod pebble_bed;
pub mod single_phase;

pub use error::TampinesError;
pub use fluids::TampinesFluid;
