//! # `distillation_sim_v1` -- transient distillation column simulator
//!
//! An interactive simulator for an 8-stage benzene/toluene distillation
//! column, built on `outram-park-digital-twin-engine`'s app scaffold and a
//! new `components::distillation_column` widget, driving
//! `outram-park-fork-dwsim-libs`'s validated transient
//! [`outram_park_fork_dwsim_libs::columns::dynamic::DynamicColumn`] model.
//! Structured as a close mirror of the `htgr_sim_v1` example -- same
//! app-scaffold usage, same restart-on-crash flow -- for a distillation
//! column instead of a reactor.
//!
//! ## What this is (and is not)
//!
//! A **working, tested simulator on a validated default plant configuration**
//! (the same benzene/toluene case kopi-beans `op-6rhz`'s own V&V test
//! independently checks against a steady MESH solve). It is **not** a
//! validated match to any real, physically-built column -- the property
//! package is `PropertyPackageModel::Ideal`, and no comparison against
//! measured dynamic-distillation data has been made. See
//! [`physics::column_config`] and `physics::mod`'s tests for exactly what is
//! and is not checked. Not for operational, licensing, or safety use.
//!
//! ## Module map
//!
//! - [`app`] -- the `eframe::App`, panels, schematic, and shared state.
//! - [`physics`] -- the plant model: [`physics::DistillationPlant`] wraps
//!   [`outram_park_fork_dwsim_libs::columns::dynamic::DynamicColumn`].

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// GUI (egui/eframe) example -- out of scope for Android, same reasoning as
// htgr_sim_v1: the real entry point and its egui-using modules are gated off
// Android and replaced by an empty `main`, so the example target still
// builds (to a no-op) there.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod physics;

#[cfg(not(target_os = "android"))]
use app::DistColSimApp;

/// Launch the distillation-column simulator window.
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    env_logger::init(); // `RUST_LOG=debug` for logs.

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Distillation Column Simulator v1 -- OUTRAM PARK",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(DistColSimApp::new(cc)))
        }),
    )
}
