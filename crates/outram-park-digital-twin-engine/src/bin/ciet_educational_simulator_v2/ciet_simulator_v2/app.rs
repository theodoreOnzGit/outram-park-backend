//! The simulator's application layer: the pages, the shared-state plumbing and
//! (on desktop) the `eframe` app itself.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app.rs`),
//! GPL-3.0, same licence.
//!
//! **What v2 changed here.** v1's `app.rs` was one file holding the `CIETApp`
//! struct, its `eframe::App` impl and the module declarations. v2 splits it:
//! the `egui`-dependent app moved to [`ciet_app`] so that this module — and
//! with it [`panels_and_pages::full_simulation`], the physics thread — still
//! compiles on Android/Termux, where there is no windowing stack. The physics
//! and plant-state modules are ungated; every `egui` page is gated off Android.

/// The pages, the plant state, and the physics threads.
///
/// Partly `egui`-free on purpose: see the module's own docs for which
/// submodules survive on Android.
pub mod panels_and_pages;

/// Widget-placement helpers, the temperature colour map and the plot-history
/// recorder thread. All `egui`-dependent, so desktop only.
#[cfg(not(target_os = "android"))]
pub mod useful_functions;

/// The `eframe` application. Desktop only.
#[cfg(not(target_os = "android"))]
pub mod ciet_app;

#[cfg(not(target_os = "android"))]
pub use ciet_app::CIETApp;
