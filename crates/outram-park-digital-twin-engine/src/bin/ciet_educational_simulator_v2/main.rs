//! CIET Educational Simulator **v2** — entry point.
//!
//! CIET (Compact Integral Effects Test) is a scaled thermal-hydraulic facility
//! for FHR research. This binary is an **offline educational simulator** of its
//! loop, with an embedded OPC-UA (IEC 62541) server so the model can be driven
//! by standard industrial tooling on a bench or in a classroom.
//!
//! ## Provenance
//!
//! Ported from the **CIET Educational Simulator v1**, which lives (and stays)
//! at `crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/`.
//! Same licence, GPL-3.0. The physics is v1's, unchanged; see
//! [`ciet_simulator_v2`] for the full v1-versus-v2 table and the honest
//! statement of what has and has not been verified.
//!
//! ## Threads
//!
//! | Thread | Started by | Role |
//! |---|---|---|
//! | physics | this function | integrates the CIET loop, one timestep at a time |
//! | OPC-UA server | this function (via [`opcua_startup`]) | serves the plant state to remote clients |
//! | plot recorder | `CIETApp::new` (GUI only) | samples the state into trend history |
//! | main | — | the `eframe` window, or the headless status printer |
//!
//! All four share one `Arc<RwLock<CietState>>`. v1 started its physics thread
//! from inside the GUI app; v2 starts it here so the *same* physics runs with no
//! GUI at all.
//!
//! ## Scope (`RESPONSIBLE_USE.md`)
//!
//! Education, research and capability building only. Never to be connected to
//! live operational systems, plant systems, safety-critical infrastructure,
//! real-time plant monitoring, or institutional production systems. Not for
//! reactor control, licensing decisions or safety-critical decision-making.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use outram_park_digital_twin_engine::ciet_opcua::state::{new_shared_state, SharedCietState};
use outram_park_digital_twin_engine::ciet_opcua::user_controls::{
    new_shared_user_controls, SharedUserControls,
};

mod cli;
mod headless;
mod opcua_startup;

/// The simulator itself: physics, pages, and (on desktop) the `eframe` app.
///
/// Ungated, because the physics inside it is `egui`-free and the headless
/// Android build runs it. Only the app and the pages are desktop-only.
pub mod ciet_simulator_v2;

use ciet_simulator_v2::app::panels_and_pages::full_simulation::educational_ciet_loop_version_4;
use cli::{CliOptions, HELP_TEXT};
use opcua_startup::{start_opcua_server, OpcuaStatus};

/// Parse the command line, start the simulation and the OPC-UA server, then hand
/// off to either the GUI or the headless status printer.
///
/// Exit codes: `0` on a clean exit or `--help`, `2` on a bad command line, and
/// `1` if the window itself fails to open.
fn main() {
    let options = match CliOptions::parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("ciet_educational_simulator_v2: {error}");
            eprintln!("Run with --help for the list of options.");
            std::process::exit(2);
        }
    };

    if options.help {
        print!("{HELP_TEXT}");
        return;
    }

    // Log to stderr if the user asked for it (`RUST_LOG=debug`). Same as v1,
    // moved out here so the headless path gets it too.
    env_logger::init();

    // One shared plant state. The physics thread writes outputs into it and the
    // GUI writes the controls its own widgets own; the OPC-UA server only READS
    // it.
    let ciet_state: SharedCietState = new_shared_state();

    // Remote (OPC-UA) control requests are queued here rather than written
    // straight into the plant state, and the physics thread drains them at the
    // top of each timestep. That separation is what stops a GUI repaint from
    // silently erasing a client's write -- see `ciet_opcua::user_controls`.
    let user_controls: SharedUserControls = new_shared_user_controls();

    // The physics thread. v1 started this from inside `CIETApp::new`; starting it
    // here is what makes headless operation possible.
    let physics_state = ciet_state.clone();
    let physics_user_controls = user_controls.clone();
    let verbose_temperatures = options.verbose_temperatures;
    std::thread::spawn(move || {
        educational_ciet_loop_version_4(physics_state, physics_user_controls, verbose_temperatures);
    });

    // The OPC-UA server, on its own thread with its own tokio runtime. A failure
    // here (typically "port already in use") is reported and then ignored -- the
    // simulator must still run.
    let opcua_status: OpcuaStatus =
        start_opcua_server(ciet_state.clone(), user_controls.clone(), &options);
    if let OpcuaStatus::Failed(error) = &opcua_status {
        eprintln!("ciet_educational_simulator_v2: OPC-UA server did not start: {error}");
        eprintln!("Carrying on without the remote interface.");
    }

    run(options, ciet_state, opcua_status);
}

/// Desktop: open the window, unless `--headless` was given.
#[cfg(not(target_os = "android"))]
fn run(options: CliOptions, ciet_state: SharedCietState, opcua_status: OpcuaStatus) {
    if options.headless {
        // `headless::run` diverges -- it never returns -- so control cannot fall
        // through into the GUI path below. If its signature ever changes to
        // return, add an explicit `return` here.
        headless::run(ciet_state, &opcua_status);
    }

    print!("{}", opcua_startup::connection_banner(&opcua_status));
    println!("Starting CIET Educational Simulator v2 (GUI)...");

    if let Err(error) = ciet_simulator_v2::ciet_simulator_v2(ciet_state, opcua_status) {
        eprintln!("ciet_educational_simulator_v2: could not open the window: {error}");
        eprintln!("On a headless machine or over SSH, run with --headless instead.");
        std::process::exit(1);
    }
}

/// Android/Termux: headless unconditionally.
///
/// There is no windowing stack here and `eframe` is not even a dependency on
/// this target, so this path never mentions it. The physics and the OPC-UA
/// server are the same code the desktop build runs.
#[cfg(target_os = "android")]
fn run(_options: CliOptions, ciet_state: SharedCietState, opcua_status: OpcuaStatus) {
    headless::run(ciet_state, &opcua_status);
}
