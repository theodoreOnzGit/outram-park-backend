//! Headless run loop: physics plus OPC-UA, no GUI.
//!
//! **v2 addition** — v1 was GUI-only.
//!
//! This is the *only* run mode on Android/Termux, which has no windowing stack
//! (`eframe` pulls in `android-activity` and the whole `wgpu` surface stack, both
//! out of scope for Termux per the workspace Android-portability rule). The
//! physics and the OPC-UA layer are pure Rust and build there unchanged, so a
//! phone can run the full CIET model and serve it to an OPC-UA client on the
//! same hotspot.
//!
//! `--headless` selects the same path on desktop, so the maintainer can test it
//! without an Android device.
//!
//! **This module must stay `egui`-free.** An `egui` import here would break the
//! Android build, which is the entire reason it exists.

use std::io::Write;
use std::thread;
use std::time::Duration;

use outram_park_digital_twin_engine::ciet_opcua::state::{CietState, SharedCietState};

use crate::opcua_startup::{connection_banner, OpcuaStatus};

/// How often the one-line status is printed, seconds of wall-clock time.
///
/// Two seconds is slow enough to read in a terminal over SSH and fast enough to
/// see a transient develop.
const STATUS_INTERVAL_SECONDS: u64 = 2;

/// Run the simulator with no GUI, printing a periodic status line, until the
/// process is killed.
///
/// `ciet_state` is the handle the physics thread — already started by `main` —
/// is integrating; this function only reads it. Never returns: press Ctrl-C (or
/// kill the process) to stop.
///
/// Prints the connection banner once up front, then one line every
/// [`STATUS_INTERVAL_SECONDS`] seconds carrying simulated time, wall-clock
/// elapsed time, heater power, BT-11 and BT-12 (heater inlet and outlet) and
/// FM-40 (CTAH-branch mass flow) — enough to see at a glance whether the loop is
/// heating, whether flow has established, and whether the solver is keeping up
/// with real time.
pub fn run(ciet_state: SharedCietState, opcua_status: &OpcuaStatus) -> ! {
    print!("{}", connection_banner(opcua_status));
    println!(
        " Headless mode. Status every {STATUS_INTERVAL_SECONDS} s. Ctrl-C to stop.\n\
         Columns: sim = simulated time (s), wall = elapsed real time (s),\n\
         heater = electrical power (kW), BT-11/BT-12 = heater inlet/outlet (degC),\n\
         FM-40 = CTAH-branch mass flow (kg/s, negative is reverse flow),\n\
         step = wall-clock cost of the last timestep (ms).\n"
    );
    let _ = std::io::stdout().flush();

    loop {
        // Snapshot under a read lock, then release it before formatting, so the
        // physics thread is never blocked on our string building.
        let snapshot: CietState = ciet_state.read().unwrap().clone();

        println!(
            "sim {:>9.1} s | wall {:>9.1} s | heater {:>6.2} kW | \
             BT-11 {:>7.2} C | BT-12 {:>7.2} C | FM-40 {:>7.4} kg/s | step {:>6.1} ms",
            snapshot.simulation_time_seconds,
            snapshot.elapsed_time_seconds,
            snapshot.heater_power_kilowatts,
            snapshot.bt_11_heater_inlet_deg_c,
            snapshot.bt_12_heater_outlet_deg_c,
            snapshot.fm40_ctah_branch_kg_per_s,
            snapshot.calc_time_ms,
        );
        let _ = std::io::stdout().flush();

        thread::sleep(Duration::from_secs(STATUS_INTERVAL_SECONDS));
    }
}
