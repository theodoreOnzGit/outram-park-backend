//! Egui-based hybrid digitiser GUI (graphreader-style), exposed as a library
//! function so more than one binary can open the same window.
//!
//! **Automatic pass first, then human verification — recorded, not
//! assumed.** The interaction model follows graphreader.com: load a plot
//! image; click two reference points per axis and type their values; choose
//! linear or log per axis; auto-trace the curve; then drag / add / delete
//! individual points with the mouse; finally mark the dataset reviewed and
//! export. Every hand edit is recorded per point (`HandPlaced` /
//! `HandCorrected` with the operator name), any edit after a review resets
//! the status to `UNREVIEWED`, and the export always carries the full
//! calibration + provenance record.
//!
//! Desktop-only by policy: this module only compiles under this crate's
//! default `gui` feature (default everywhere except Android — see this
//! crate's `Cargo.toml`), and its egui/eframe dependencies are target-gated
//! off Android; [`run`] itself branches internally so its one caller — the
//! `kovan` binary — gets Android-safe behaviour for free.
//!
//! (Was `digitise-gui`, called from a now-retired `kovan-digitise-gui`
//! binary, before the digitiser moved from `kovan-literature` into this
//! crate 2026-08-21 — see this crate's `NOTICE`. The wrapper binary was named
//! `kovan-gui` at that point too, then renamed to plain `kovan` the same day
//! per GitHub issue #30's final 3-binary spec — `kovan` (GUI), `kovan-cli`
//! (agent CLI), `kovan-tui` (terminal UI).)
//!
//! [`run`] opens [`crate::app::DigitiseApp`] — the app shell, not a member
//! of this module. It used to be a private `desktop` submodule nested
//! directly under here; GH issue #35 checkpoint §22 (`op-1arj`) moved it out
//! to `crate::app` 2026-09-01, since the digitiser is one panel of the app
//! shell, not its owner. This module's own job stays exactly what its doc
//! above says: open the window, and stay a redirect stub on Android.

/// Open the digitiser window, optionally pre-loading `image_arg` as the plot
/// image. Blocks until the window is closed. On Android, prints a redirect
/// message and returns `Ok(())` immediately instead of opening a window.
#[cfg(target_os = "android")]
pub fn run(_image_arg: Option<String>) -> Result<(), String> {
    eprintln!(
        "kovan is desktop-only; on Android/Termux use \
         kovan-cli digitise (automatic) or kovan-tui (interactive review)."
    );
    Ok(())
}

/// Open the digitiser window, optionally pre-loading `image_arg` as the plot
/// image. Blocks until the window is closed.
#[cfg(not(target_os = "android"))]
pub fn run(image_arg: Option<String>) -> Result<(), String> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "kovan",
        options,
        Box::new(move |_cc| {
            let mut app = crate::app::DigitiseApp::default();
            if let Some(path) = image_arg {
                app.load_image(&path);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| e.to_string())
}
