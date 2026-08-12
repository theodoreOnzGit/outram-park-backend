//! **Widget Studio** — a gallery app for developing and quality-controlling
//! the OUTRAM PARK visual component library one widget at a time.
//!
//! This is deliberately *not* a simulator. A simulator shows a whole plant and
//! hides each widget's weaknesses in the crowd; the studio shows **one widget**
//! driven across its full state range by hand, next to a readout of exactly
//! which physical quantities are driving what. That is what makes a bad widget
//! obvious.
//!
//! ## Why it exists
//!
//! An inventory of `src/components/` (2026-08-03) found one genuine widget
//! (`PipeVisual`), one modest one (`ReactorVesselVisual`), four that draw a
//! single flat rectangle coloured by one scalar, and four with no
//! state-derived detail at all. The studio is where that gets fixed, widget by
//! widget, with a quality bar applied to each in turn (bead `op-wqk.14`).
//!
//! ## The turbine (first widget)
//!
//! The steam turbine is driven by
//! [`ThreePhaseElectricGeneratorTurbine`] from `tampines-steam-tables` — a
//! real lumped torque balance, not an animation. Drive torque and electrical
//! load are yours to set; the model integrates rotor angular velocity
//! explicitly, and the blades turn at `theta = omega * t`. Spin-up, loading
//! and coast-down are all visible directly.
//!
//! Run it:
//! ```text
//! cargo run -p outram-park-digital-twin-engine --example widget_studio --release
//! ```
//!
//! **Untrusted AI-assisted draft pending human V&V.** Offline demonstration
//! only — education / research / V&V, per the workspace `RESPONSIBLE_USE.md`.
//! The 250 MW generator preset's parameters are illustrative placeholders, not
//! measurements from a real machine. Not for reactor operation, licensing, or
//! safety-critical decisions.

// Android build: no windowing GUI. An empty main keeps `cargo check` clean.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
mod bend_tab;
#[cfg(not(target_os = "android"))]
mod pipes;

/// Condenser gallery: both cooling-water arrangements, state-driven and
/// physics-backed side by side.
mod condenser_tab;

/// Cooling-tower gallery: natural-draught and induced-draught, under live
/// psychrometric controls.
mod cooling_tower_tab;

/// Excursion overlay: the destructive annotation, driven by its trigger
/// directly instead of by wrecking a reactor.
mod excursion_tab;

/// Reactor-vessel gallery: every scoped reactor architecture, side by side.
mod reactor_tab;

/// Pump gallery: centrifugal, vertical canned-rotor and axial propeller.
mod pump_tab;

/// Steam-generator gallery: vertical U-tube, horizontal U-tube, helical coil.
mod steam_generator_tab;
#[cfg(not(target_os = "android"))]
mod studio;

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Widget Studio — OUTRAM PARK (offline demo)",
        native_options,
        Box::new(|_cc| Ok(Box::new(studio::WidgetStudio::default()))),
    )
}
