//! CIET three-branch primary loop plus the DRACS passive decay-heat
//! removal loop.
//!
//! This module assembles the Compact Integral Effects Test (CIET)
//! facility as three primary-loop branches — the heater branch, the
//! CTAH (Coiled Tube Air Heater) branch used for forced circulation,
//! and the DHX (DRACS Heat Exchanger) branch — coupled to the DRACS
//! natural-circulation loop and its TCHX (Thermosyphon-Cooled Heat
//! eXchanger). It provides:
//!
//! - [`components`] — the extra CIET components specific to the
//!   three-branch layout (the rest are reused from the isothermal and
//!   steady-state natural-circulation modules).
//! - [`solver_functions`] — the thermal-hydraulic link-up, fluid-flow
//!   and timestep-advance routines for the DRACS loop and the three
//!   primary-loop branches.
//! - [`ciet_educational_simulator_loop_prototypes`] — successive
//!   real-time educational simulator prototypes (versions 1-3).
//!
//! Temperatures are in K / degC, mass flow in kg/s, power/heat transfer
//! in W, pressure drop in Pa, and lengths in m throughout (carried as
//! `uom` dimensioned quantities on the public component APIs).

/// this version of ciet is optimised for real-time
/// simulation. It will not be validated, but it will be
/// fun to play with as a simulator. Useful for education and etc.
///
/// Also included here is some csv data for ciet's ctah loop forced circulation
/// transients
pub mod ciet_educational_simulator_loop_prototypes;

/// CIET needs Thermo-hydraulic equations solved in TUAS
///
/// Writing them out explicitly in a procedural form is quite
/// cumbersome. It is much more concise to have these functions
/// here
///
/// Here there are functions for connecting the
/// heat transfer entities in:
///
/// - dracs loop
/// - pri loop DHX branch
/// - pri loop heater branch
/// - pri loop CTAH branch (forced circ)
///
/// Also I need to solve fluid flow in
/// - dracs loop
/// - pri loop, DHX, heater and CTAH branch
///
/// I would also need to be able to block flow in pri loop DHX
/// and CTAH branch as well, so as to isolate the loops.
///
pub mod solver_functions;

/// adds extra components specific to the three branch
/// simulation,
/// the other components were borrowed from the isothermal
/// test and steady state natural circulation modules
pub mod components;
