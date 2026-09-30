//! # `htgr_sim_v1` -- HTGR educational simulator (~~scaffold~~ demo)
//!
//! **Status: ~~scaffold~~ **CHANGED 2026-09-28** (maintainer: "no longer a
//! scaffold, but a demo") — demo.** A demo here means an **offline
//! demonstration** for education and research, per `RESPONSIBLE_USE.md`: it
//! runs end to end on real workspace physics, but it is **not validated** and
//! is not an operational, licensing or safety tool. "Demo" claims no V&V
//! beyond what each module's own doc comments record.
//!
//! A first-cut interactive simulator for a **helium-cooled, graphite-moderated
//! pebble-bed High-Temperature Gas-cooled Reactor (HTGR)**, built on the
//! `outram-park-digital-twin-engine` crate's reusable widgets and app scaffold
//! from the start -- deliberately **not** re-deriving local widget/threading
//! boilerplate the way the older `fhr_sim_v2` example did.
//!
//! The plant is laid out as an HTR-10-style machine: a pebble bed in one
//! pressure vessel, a once-through helical-coil steam generator with the
//! helium circulator above it in a second pressure vessel beside it, and a hot
//! gas duct cross-vessel between the two. See [`app::schematic`] for what that
//! arrangement is and where it comes from.
//!
//! ## Read this first (2026-09-29)
//!
//! - One delayed-neutron fraction everywhere, `beta_eff = 7.26e-3` (gh:#387).
//! - **The delayed-neutron precursors start EMPTY** and fill over the first
//!   minutes (within 1 % of equilibrium after 245 s at 10 MW); from a cold
//!   start at 0 $ the power first drops, so early-transient numbers are not
//!   plant behaviour. See `physics::kinetics`' "Read this first".
//! - Decay heat starts at equilibrium (conservative).
//! - Rod worth vs feedback reference: demo-grade (gh:#408).
//! - Reactor building not credited (conservative): leak straight to the stack
//!   (gh:#409).
//!
//! ## What this is (and is not)
//!
//! ~~This is a **scaffold**: a working, compiling skeleton~~ **CHANGED
//! 2026-09-28** — this is a **demo** (see the status note above): a working
//! simulator with the real
//! cross-crate structure wired up (engine widgets, engine app scaffold,
//! `teh-o-prke` prompt kinetics, `tampines-steam-tables` steam properties). It
//! is **not** a validated HTGR model -- several thermal-hydraulic correlations
//! are first-cut placeholders, ~~and the delayed-neutron kinetics is a local
//! stand-in for the forthcoming `teh_o_prke::DelayedNeutronLayer`~~
//! (**CORRECTED 2026-09-28** — `physics::kinetics` uses
//! `teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer` itself). See the
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
mod headless;
#[cfg(not(target_os = "android"))]
mod physics;
#[cfg(not(target_os = "android"))]
mod runtime;

#[cfg(not(target_os = "android"))]
use app::HtgrSimApp;

/// Launch the HTGR simulator window.
#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    env_logger::init(); // `RUST_LOG=debug` for logs.

    // Headless mode: run the plant with no GUI and print a CSV trace.
    //
    //     cargo run --release --example htgr_sim_v1 -- --headless [steps] [sample_every]
    //
    // Exists so the physics can be run and observed without eframe -- for
    // recording the pre-refactor reference baseline (bead op-fbou) and for
    // regression tests. See `headless`.
    let args: Vec<String> = std::env::args().collect();

    // The Map tab's static "Bounding air ingress" table (#453) as CSV; no
    // plant is run (a bounding case, not a transient, #420).
    //
    //     cargo run --release --example htgr_sim_v1 -- --bounding-air-ingress
    if args.iter().any(|a| a == "--bounding-air-ingress") {
        headless::print_bounding_air_ingress();
        return Ok(());
    }

    if args.iter().any(|a| a == "--headless") {
        let nums: Vec<usize> = args[1..]
            .iter()
            .filter(|a| !a.starts_with("--"))
            .filter_map(|a| a.parse().ok())
            .collect();
        let cfg = headless::HeadlessConfig {
            steps: nums.first().copied().unwrap_or(600),
            sample_every: nums.get(1).copied().unwrap_or(60),
            ..Default::default()
        };
        eprintln!(
            "htgr_sim_v1 headless: {} steps, sampling every {}",
            cfg.steps, cfg.sample_every
        );
        headless::run_and_print(&cfg);
        return Ok(());
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1600.0, 900.0]),
        ..Default::default()
    };

    eframe::run_native(
        "HTGR Simulator v1.1 (demo, not validated) -- OUTRAM PARK",
        native_options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(HtgrSimApp::new(cc)))
        }),
    )
}
