//! # outram-park-digital-twin-engine
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

// `animation` is pure `uom` (trait contracts + travel-time math), so it stays
// buildable on Android. The remaining modules depend on the GUI stack
// (`egui`/`eframe`/`egui_plot`/`egui_extras`), which is Android-hostile, so
// they compile only off Android -- matching the target-gated GUI dependencies
// in `Cargo.toml`. On Android the library reduces to `animation`, which keeps
// `cargo check --target aarch64-linux-android` clean (see workspace CLAUDE.md
// Android-portability rule). Desktop builds are unchanged.
pub mod animation;
#[cfg(not(target_os = "android"))]
pub mod app_scaffold;
#[cfg(not(target_os = "android"))]
pub mod color_maps;
#[cfg(not(target_os = "android"))]
pub mod components;
