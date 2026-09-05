//! The CIET Educational Simulator, version 2.
//!
//! CIET (Compact Integral Effects Test) is a scaled thermal-hydraulic facility
//! for fluoride-salt-cooled high-temperature reactor (FHR) research. This module
//! is an educational, **offline** simulator of its loop: a heater branch, a CTAH
//! (coolant-to-air heat exchanger) branch, a DHX (DRACS heat exchanger) branch,
//! and the DRACS natural-circulation loop with its TCHX.
//!
//! ## Provenance
//!
//! Ported from the **CIET Educational Simulator v1**, which lives (and stays)
//! at `crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/`.
//! Same licence, **GPL-3.0**. Same physics: every solver call, correlation,
//! calibration constant, killswitch threshold and timestep-pacing rule is v1's,
//! translated without change of behaviour. The v1 tree was not modified.
//!
//! Artwork credits (DWSIM process-object icons and the CIET/SAM nodalisation
//! diagram replica) are unchanged and are reproduced on the citations page.
//!
//! ## What v2 changed, and only that
//!
//! | Area | v1 | v2 |
//! |---|---|---|
//! | Shared handle | `Arc<Mutex<CIETState>>` | `Arc<RwLock<CietState>>` — GUI reads and OPC-UA reads no longer serialise |
//! | Plant state | a struct private to the GUI binary | [`outram_park_digital_twin_engine::ciet_opcua::state::CietState`] in the crate library, shared with the OPC-UA layer |
//! | Frequency response | evaluated in `eframe::App::ui`, once per repaint | evaluated by the physics thread, once per timestep |
//! | Remote interface | none | an OPC-UA (IEC 62541) server on a parallel thread |
//! | Headless | not possible | `--headless`, and the default on Android/Termux |
//! | Temperature `dbg!` firehose | on every timestep | off by default, `--verbose-temperatures` to restore |
//!
//! ## Verification status (honest statement)
//!
//! The maintainer has done validation work on **v1's** physics. This port is a
//! faithful translation of that physics, but **the port equivalence itself has
//! not been verified** — no v1-versus-v2 trajectory comparison has been run.
//! Do not describe v2 as validated. Per `RESPONSIBLE_USE.md` it is for
//! education, research and capability building only: not for facility
//! operation, reactor control, licensing, safety-critical decisions, or
//! real-time plant monitoring.

pub mod app;

#[cfg(not(target_os = "android"))]
pub use app::CIETApp;

/// Open the simulator window on an already-running simulation.
///
/// `ciet_state` must be the handle the physics thread (started by `main.rs`) is
/// integrating; `opcua_status` records whether the embedded OPC-UA server came
/// up. Blocks until the user closes the window.
///
/// Desktop only. The Android/Termux build has no windowing stack and uses
/// `crate::headless::run` instead.
#[cfg(not(target_os = "android"))]
pub fn ciet_simulator_v2(
    ciet_state: outram_park_digital_twin_engine::ciet_opcua::state::SharedCietState,
    opcua_status: crate::opcua_startup::OpcuaStatus,
) -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "CIET Simulator V2 Powered by TUAS",
        native_options,
        Box::new(move |cc| {
            // image support,
            // from
            // https://github.com/emilk/egui/tree/master/examples/images
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(CIETApp::new(cc, ciet_state, opcua_status)))
        }),
    )
}
