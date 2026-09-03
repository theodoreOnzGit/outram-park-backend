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
//!   not here. The one maintainer-directed exception is [`htr10`] (bead
//!   `op-jyyp`, 2026-08-11): the HTR-10 simulator rewrite's *cited* design
//!   constants and packed-bed reference correlations, kept here with their
//!   V&V unit tests so the example rewrite and its tests share one
//!   provenance-checked source.
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
//! The four modules below are implemented: [`color_maps`] and [`app_scaffold`]
//! are ports of already-working code, [`components`] wraps the physics types
//! it visualizes, and [`animation`] carries the tracer kinematics. The one
//! deliberate stand-in is [`components::InstrumentationVisual`], which stays a
//! label/value placeholder because `nee_soon` exposes no instrumentation
//! readout type to wrap yet.
//!
//! Per `RESPONSIBLE_USE.md`, everything here is **untrusted draft material
//! until human-reviewed** — see the crate README's bookkeeping-status block for
//! the maintainer sign-off state. The example simulators are **offline
//! demonstrations only**.
//!
//! See the workspace's beads issue tracker for the live module plan.

#![forbid(unsafe_code)]

// `animation` is pure `uom` (trait contracts + travel-time math), so it stays
// buildable on Android. The remaining modules depend on the GUI stack
// (`egui`/`eframe`/`egui_plot`/`egui_extras`), which is Android-hostile, so
// they compile only off Android -- matching the target-gated GUI dependencies
// in `Cargo.toml`. On Android the library reduces to `animation`, which keeps
// `cargo check --target aarch64-linux-android` clean (see workspace CLAUDE.md
// Android-portability rule). Desktop builds are unchanged.
pub mod animation;
// `ciet_opcua` is the CIET Educational Simulator v2 half of the OPC-UA
// interface -- plant state, node map, identity strings -- shared by the two CIET
// v2 binaries. It is deliberately GUI-free and physics-free, and like
// `opcua_core` below it builds on Android/Termux with no target gate: the
// headless Termux build of the simulator serves OPC-UA exactly as the desktop
// one does.
/// Not built for wasm: this module's OPC-UA / mDNS / networking stack has no
/// browser equivalent (see the wasm target table in Cargo.toml). Android keeps
/// it. Beads op-okqo.3, op-eeqw.2.
#[cfg(not(target_arch = "wasm32"))]
pub mod ciet_opcua;
// `opcua_core` is the reactor-agnostic OPC-UA server layer `ciet_opcua` is built
// on: transport, server thread, PKI, mDNS discovery and address-space
// construction, parameterised by whichever simulator is being served. GUI-free
// like `ciet_opcua`, and buildable on Android/Termux for the same reason. Named
// `opcua_core` rather than `opcua` so it cannot shadow the `opcua` crate in a
// `use` path.
/// Not built for wasm: this module's OPC-UA / mDNS / networking stack has no
/// browser equivalent (see the wasm target table in Cargo.toml). Android keeps
/// it. Beads op-okqo.3, op-eeqw.2.
#[cfg(not(target_arch = "wasm32"))]
pub mod opcua_core;
// `htr10` is GUI-free cited-constant + correlation data for the HTR-10
// pebble-bed simulator rewrite (bead op-jyyp), so like `animation` it builds
// on Android with no target gate. NOT VALIDATED -- its tests reproduce
// published numbers; they do not validate a simulator.
pub mod htr10;
#[cfg(not(target_os = "android"))]
pub mod app_scaffold;
#[cfg(not(target_os = "android"))]
pub mod color_maps;
#[cfg(not(target_os = "android"))]
pub mod components;
