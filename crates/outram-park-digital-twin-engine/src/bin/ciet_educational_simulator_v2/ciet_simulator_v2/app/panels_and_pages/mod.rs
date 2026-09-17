//! The simulator's pages, plus the physics and plant-state modules they read.
//!
//! ## Provenance
//!
//! Ported from the CIET Educational Simulator **v1**
//! (`crates/tuas_boussinesq_solver/examples/ciet_educational_simulator/ciet_simulator_v1/app/panels_and_pages/mod.rs`),
//! GPL-3.0, same licence.
//!
//! **What v2 changed here:** the [`Panel`] enum gained an
//! [`Panel::OpcuaServer`] variant for the new "OPC-UA Server" page, and the
//! `egui`-using page modules are now gated off Android. The physics modules
//! ([`full_simulation`], [`nat_circ_simulation`]) and the plant-state module
//! ([`ciet_data`]) are `egui`-free and stay ungated, because the headless
//! Termux build runs exactly the same physics with no GUI at all.

/// Which page the user currently has open.
///
/// Closed set, matched exhaustively in `app.rs` — adding a variant is a compile
/// error at every dispatch site until it is handled, which is the point.
/// v1 derived `serde::Serialize`/`Deserialize` here for `eframe`'s
/// app-persistence blob. v2 drops that: v1's restore path was already
/// commented out, so the blob was written and never read, and keeping it forced
/// a `Default` impl on `CIETApp` that would have handed out a shared-state
/// handle disconnected from the running physics thread.
///
/// Desktop only: there are no pages on Android, which runs headless.
#[cfg(not(target_os = "android"))]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Panel {
    /// The CIET schematic with live temperatures and the main controls.
    MainPage,
    /// CTAH pump pressure control and its trends.
    CTAHPump,
    /// CTAH (coolant-to-air heat exchanger) trends.
    CTAH,
    /// Heater power control and BT-11 / BT-12 trends.
    Heater,
    /// DHX shell-and-tube heat exchanger trends and DHX branch valve.
    DHX,
    /// TCHX (thermosyphon-cooled heat exchanger) trends.
    TCHX,
    /// Advanced heater control: steady power, sine perturbation, step response.
    FrequencyResponseAndTransients,
    /// The CIET nodalisation diagram (a static image).
    NodalisedDiagram,
    /// On-the-fly heater-mesh recalibration.
    OnlineCalibration,
    /// **v2 addition.** How to connect an OPC-UA client, the security caveats,
    /// and the full node table.
    OpcuaServer,
}

#[cfg(not(target_os = "android"))]
pub mod main_page;

#[cfg(not(target_os = "android"))]
pub mod heater_page;

#[cfg(not(target_os = "android"))]
pub mod ctah_page;

/// page for controlling pumps and valves along the CTAH
#[cfg(not(target_os = "android"))]
pub mod ctah_pump_page;

/// page for controlling valves along the dhx branch
/// and for seeing the DHX more closely
#[cfg(not(target_os = "android"))]
pub mod dhx_page;

/// the shared plant state (re-exported from the crate library) and the
/// GUI-local plot/CSV history. `egui`-free, so it builds on Android.
pub mod ciet_data;

/// contains code for natural circulation only. `egui`-free.
pub mod nat_circ_simulation;

#[cfg(not(target_os = "android"))]
pub mod tchx_page;

/// contains code for fine control, step and frequency response
#[cfg(not(target_os = "android"))]
pub mod frequency_response_and_transients;

/// contains code for the full educational simulator of CIET,
/// both forced and natural circulation. `egui`-free: this is the physics
/// thread, and it is what the headless Termux build runs.
pub mod full_simulation;

/// contains code on the fly adjustment of heater, and other components.
/// Also re-exports `HeaterType`, which the physics thread needs, so the module
/// itself is ungated and only its `egui` page is gated.
pub mod online_calibration;

/// citation and disclaimer page code
#[cfg(not(target_os = "android"))]
pub mod citations_and_disclaimers;

/// **v2 addition.** the "OPC-UA Server" page: endpoint URLs, security
/// warnings, connection troubleshooting and the full node table.
#[cfg(not(target_os = "android"))]
pub mod opcua_page;
