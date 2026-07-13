//! # outram-park-digital-twin-gui
//!
//! Reusable visualization framework for OUTRAM PARK digital twins.
//!
//! Provides visual process objects (Pipe, Pump, Valve, HeatExchanger, Steam
//! Generator, Turbine, Condenser, Cooling Tower, Reactor Vessel,
//! Instrumentation) whose rendering derives directly from physics state:
//! cell count drives displayed cells, temperature drives cell colour, mass
//! flow drives tracer direction, residence time drives tracer travel time.
//!
//! ## Design philosophy
//!
//! Avoid separating physics and rendering unnecessarily. Each visual
//! component bundles physics state (from [`tampines`]/[`nee_soon`]),
//! its visual representation, and its animation logic together, rather than
//! maintaining a physics model and a separate rendering model that must be
//! kept in sync by hand.
//!
//! ## What it composes
//!
//! | Piece | Provided by | Role |
//! |---|---|---|
//! | Thermal-hydraulic physics | [`tampines`] | Component state (temperature, pressure, flow, quality, ...) to visualize |
//! | Reactor-vessel / instrumentation | [`nee_soon`] | Neutronics/kinetics state to visualize |
//! | Process control | [`chem_eng_real_time_process_control_simulator`] | Controller state (setpoints, PID output) to visualize |
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** visual process object wrappers, colour-map functions,
//!   tracer/animation logic, the `eframe::App` threading/locking scaffold
//!   reusable across digital-twin GUI applications.
//! - **Does NOT belong here:** any new physics -- if a visualization needs a
//!   physical quantity `tampines`/`nee_soon` don't yet expose, add it there,
//!   not here.
//!
//! ## Android / portability
//!
//! This crate makes **no Android-portability claim** -- unlike the rest of
//! the workspace, GUI dependencies (`egui`/`eframe`/`egui_plot`/`egui_extras`)
//! are real dependencies here, not confined to `examples/`, since this
//! crate's entire purpose is presentation.
//!
//! ## Status
//!
//! **Scaffold only.** This crate is being built out incrementally; see the
//! `op-wqk` epic in the workspace's beads issue tracker for the live module
//! plan and progress.

#![forbid(unsafe_code)]

pub mod animation;
pub mod color_maps;
pub mod components;
