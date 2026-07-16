//! # `htgr_sim_v1` -- HTGR educational simulator (scaffold)
//!
//! A first-cut interactive simulator for a **helium-cooled, graphite-moderated
//! prismatic-block High-Temperature Gas-cooled Reactor (HTGR)**, built on the
//! `outram-park-digital-twin-engine` crate's reusable widgets and app scaffold
//! from the start -- deliberately **not** re-deriving local widget/threading
//! boilerplate the way the older `fhr_sim_v2` example did.
//!
//! ## What this is (and is not)
//!
//! This is a **scaffold**: a working, compiling skeleton with the real
//! cross-crate structure wired up (engine widgets, engine app scaffold,
//! `teh-o-prke` prompt kinetics, `tampines-steam-tables` steam properties). It
//! is **not** a validated HTGR model -- several thermal-hydraulic correlations
//! are first-cut placeholders, and the delayed-neutron kinetics is a local
//! stand-in for the forthcoming `teh_o_prke::DelayedNeutronLayer`. See the
//! module docs (`app`, `physics`) and beads `op-wqk.9.1`..`op-wqk.9.4` for the
//! exact real-vs-placeholder breakdown. Per this workspace's data policy, all
//! parameters are round, order-of-magnitude illustrative numbers, not any
//! specific licensed design. Not for operational, licensing, or safety use.
//!
//! ## Module map
//!
//! - [`app`] -- the `eframe::App`, panels, schematic, and shared state, all
//!   built on `outram_park_digital_twin_engine::app_scaffold` +
//!   `::components`.
//! - [`physics`] -- the HTGR plant model: reactor kinetics, helium primary
//!   loop, steam secondary loop.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// GUI (egui/eframe) example -- out of scope for Android, whose windowing stack
// is absent and whose `android-activity` glue this crate does not provide. The
// real entry point and its egui-using modules are gated off Android and
// replaced by an empty `main`, so the example target still builds (to a no-op)
// there and `cargo check --examples --target aarch64-linux-android` stays clean.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod physics;

#[cfg(not(target_os = "android"))]
use app::HtgrSimApp;

/// Launch the HTGR simulator window.
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    env_logger::init(); // `RUST_LOG=debug` for logs.

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HTGR Simulator v1 (scaffold) -- OUTRAM PARK",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(HtgrSimApp::new(cc)))
        }),
    )
}
