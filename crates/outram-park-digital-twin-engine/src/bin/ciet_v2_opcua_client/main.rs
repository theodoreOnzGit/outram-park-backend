//! # CIET v2 OPC-UA demo client
//!
//! A small desktop app that finds a running **CIET Educational Simulator v2** on
//! the local network, connects to it over OPC-UA, shows its live outputs and writes
//! its controls. It is the "you can drive the reactor simulator from another
//! laptop" demonstration.
//!
//! ```bash
//! cargo run --release -p outram-park-digital-twin-engine --bin ciet_v2_opcua_client
//! ```
//!
//! ## Scope: an OFFLINE educational demonstration client
//!
//! `RESPONSIBLE_USE.md` binds this binary directly, and OPC-UA being a
//! plant-connectivity protocol makes the boundary worth stating twice. This client
//! exists so an **offline educational simulator** can be driven from another
//! machine, for teaching, capability building and verification work. It must
//! **never** be connected to live operational systems, plant systems,
//! safety-critical infrastructure, real-time plant monitoring, reactor control,
//! licensing or safety-critical decision-making, operational digital-twin
//! deployments, or institutional production systems. Nothing it displays is
//! authoritative for any operational, licensing or safety purpose.
//!
//! ## Security: there is none, and the app says so
//!
//! The session uses `SecurityPolicy::None`, `MessageSecurityMode::None` and an
//! anonymous identity token, because that is what the simulator serves. **No
//! authentication and no encryption**: anyone who can reach the port can read every
//! output and write every control, including the heater power. A permanent banner
//! says this while a session is up — it is not dismissible and must not be moved
//! behind a tab.
//!
//! ## It listens; it does not scan
//!
//! Discovery is passive subscription to mDNS announcements. This client never
//! scans, sweeps, probes or fingerprints a network, and only ever contacts an
//! address that a simulator announced or the user typed in. A simulator that does
//! not advertise itself is invisible to it, and the fallback for that is *asking
//! for the address*, not going to look for it.
//!
//! ## Nothing is fabricated
//!
//! Any quantity the client has not actually read is shown as `--`. There is no
//! placeholder number, no zero default, no interpolation and no carry-over between
//! sessions.
//!
//! ## Module map
//!
//! | Module | Role | GUI-free? |
//! |---|---|---|
//! | [`endpoint`] | URL normalisation, namespace resolution, failure diagnosis | yes, and unit tested |
//! | [`nodes`] | CIET node-map enums ↔ OPC-UA `NodeId` | yes, and unit tested |
//! | [`shared_state`] | the one `Arc<RwLock<T>>` the GUI and worker share | yes, and unit tested |
//! | [`values`] | OPC-UA `Variant` → CIET reading, the type gate | yes, and unit tested |
//! | [`worker`] | the OPC-UA session, on its own thread and runtime | yes, partly unit tested |
//! | [`browse`] | passive mDNS discovery, polled on a timer | yes |
//! | [`drafts`] | control edit state and read-back comparison | yes, and unit tested |
//! | [`ui`] | panels, the security banner, formatting | no |
//! | [`app`] | `eframe::App`: panel dispatch and repaint pacing | no |
//!
//! Everything the client knows about the interface comes from
//! [`node_map`](outram_park_digital_twin_engine::ciet_opcua::node_map) by iterating
//! `CietSignal::ALL`, `CietControl::ALL` and `CietSwitch::ALL`. There is no
//! hand-copied node list anywhere in this binary, and the namespace index is read
//! from the server rather than assumed to be `2`.
//!
//! ## Android / Termux
//!
//! `egui`/`eframe` are Android-hostile and windowing GUI is out of scope there per
//! the workspace Android-portability rule. Because this is a `[[bin]]`, a blanked
//! file would fail with "main function not found", so the Android build gets a stub
//! [`main`] that prints a one-line notice and every desktop item is gated with
//! `#[cfg(not(target_os = "android"))]`. Precedent:
//! `njoy-outram-park-fork`'s `examples/gpu_wmp_bench.rs`.
//!
//! On Termux, use the simulator's own headless mode instead — the
//! `ciet_opcua` library module (server, node map, discovery) builds and runs on
//! Android with no target gate.

// ---------------------------------------------------------------------------
// Android: windowing GUI is out of scope. A `[[bin]]` still needs a `main`.
// ---------------------------------------------------------------------------

/// Android stub entry point.
///
/// Prints a one-line notice and exits 0. The desktop GUI needs `eframe`, which
/// pulls in `android-activity` and the whole windowing stack — out of scope for
/// Android per the workspace portability rule.
#[cfg(target_os = "android")]
fn main() {
    eprintln!(
        "ciet_v2_opcua_client: desktop-only (egui/eframe windowing GUI). \
         On Termux, run the simulator's own headless mode instead."
    );
}

// ---------------------------------------------------------------------------
// Desktop.
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod browse;
#[cfg(not(target_os = "android"))]
mod drafts;
#[cfg(not(target_os = "android"))]
mod endpoint;
#[cfg(not(target_os = "android"))]
mod nodes;
#[cfg(not(target_os = "android"))]
mod shared_state;
#[cfg(not(target_os = "android"))]
mod ui;
#[cfg(not(target_os = "android"))]
mod values;
#[cfg(not(target_os = "android"))]
mod worker;

/// Initial window size, logical points. Wide enough for the output grid's four
/// columns and the two stacked trend plots without horizontal scrolling.
#[cfg(not(target_os = "android"))]
const INITIAL_WINDOW_SIZE: [f32; 2] = [1280.0, 900.0];

/// Desktop entry point: open the window and run the app.
///
/// Prints the scope and security statement to stderr before opening the window, so
/// it is in the terminal scrollback and in any log a user pastes into a bug report
/// — not only on screen where it can be missed.
///
/// # Errors
///
/// Returns whatever `eframe::run_native` returns: a windowing or graphics failure,
/// typically a missing display or an unavailable GPU surface.
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    eprintln!("CIET v2 OPC-UA demo client -- OFFLINE educational demonstration only.");
    eprintln!(
        "  No authentication, no encryption. Anyone on the network can read and write \
         these values."
    );
    eprintln!(
        "  Never point this at live operational systems, plant systems, safety-critical \
         infrastructure, or institutional production systems (RESPONSIBLE_USE.md)."
    );
    eprintln!("  Discovery is passive mDNS listening. This client does not scan the network.");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(INITIAL_WINDOW_SIZE),
        ..Default::default()
    };

    eframe::run_native(
        "CIET v2 OPC-UA demo client (offline demonstration)",
        native_options,
        Box::new(|creation_context| {
            Ok(Box::new(app::CietOpcUaClientApp::new(
                &creation_context.egui_ctx,
            )))
        }),
    )
}
