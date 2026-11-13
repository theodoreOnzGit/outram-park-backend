//! BUGIS — **B**rowser-based **U**ser **G**raphics **I**nteraction **S**ystem
//! (backronym set by the maintainer, 2026-11-02).
//!
//! > Research, education and V&V only. See the workspace `RESPONSIBLE_USE.md`.
//!
//! The workspace's home for colour theming in general (maintainer,
//! 2026-11-02), and the one place the egui simulators get their look from.
//!
//! **`no_std` + `alloc`, AGPL-3.0-only** (maintainer, 2026-11-02). The core
//! needs neither `std` nor a GUI:
//!
//! - [`Rgb`], an 8-bit sRGB colour;
//! - [`gruvbox`], the Gruvbox palette and its colours by role ([`gruvbox::Roles`]);
//! - [`colour_maps`], the temperature and steam-quality maps and Crameri's
//!   perceptually uniform maps (moved from `outram-park-digital-twin-engine`).
//!
//! The **`egui` feature** (on by default) adds the GUI half: [`Theme`]
//! (Gruvbox Dark, the default, and Light) as [`egui::Visuals`], a Dark/Light
//! [`selector`], and the [`palette`] as `egui::Color32` (maintainer,
//! 2026-11-02: "give all simulators a gruvbox theme"). A `no_std` user takes
//! `default-features = false`.
//!
//! **The central GUI crate** (maintainer, 2026-11-02; gh:#1112). The GUI code
//! of `outram-park-digital-twin-engine` moves here byte for byte, each module
//! behind a std feature, with a re-export at the old path:
//!
//! - `animation` (feature `animation`): tracer kinematics, egui-free;
//! - `ascii` (feature `ascii`): schematics on a character grid;
//! - `app_scaffold` (feature `app_scaffold`): shared state, monitored
//!   threads, crash modal, CSV, frame metrics, pacing;
//! - `web_demo` (feature `web_demo`, from `dhoby-ghaut`): the browser demos'
//!   mobile-first view, panel, docks and worker plumbing;
//! - `components` (feature `widgets`): the widget art. ~~It reads
//!   `tampines`, `tampines-steam-tables`, `outram-park-fork-dwsim-libs` and
//!   `nee_soon` state through optional features of the same names.~~
//!   **CORRECTED 2026-11-05** (maintainer: "widget art can live in bugis, the
//!   combination of logic and art can live inside the digital twin engine"):
//!   the art draws from plain numbers; the physics-backed widgets are
//!   `outram_park_digital_twin_engine::components`. bugis depends on no
//!   workspace crate but `petir`.
//!
//! The crate is `no_std` exactly when the `std` feature (implied by each of
//! those) is off.
//!
//! ```no_run
//! # #[cfg(feature = "egui")]
//! # fn f(ctx: &egui::Context, ui: &mut egui::Ui) {
//! let mut theme = bugis::Theme::default(); // Gruvbox Dark
//! theme.apply(ctx);                        // once, at start-up
//! bugis::selector(ui, &mut theme);         // in the side panel: applies on change
//! # }
//! ```
//!
//! # Provenance
//!
//! The visuals are a **move**, not a rewrite, of `kovan`'s
//! `src/app/theme.rs` `gruvbox_visuals` (itself ported from
//! `tampines-steam-tables-gui`'s `theme.rs`, GitHub issue #30). Both now call
//! this crate, so the three copies cannot drift.
//!
//! The Gruvbox colour palette is based on
//! [morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the
//! MIT License. Only the published hex colour values are reproduced here
//! ([`gruvbox`], [`palette`]); no source code from that project is used.
//!
//! Aura UI (`aura-gui` 0.3.0, AGPL-3.0-only, Camilo Grajales Holguín) gave
//! the role layout of [`gruvbox::Roles`] and its Gruvbox Dark assignments, and
//! the idea of [`Rgb::mix`]; each is cited where it is used.

// `no_std` unless a std feature is on (gh:#1112): the moved GUI modules were
// written against the std prelude and stay byte for byte as they were.
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod colour;
pub mod colour_maps;
pub mod gruvbox;

pub use colour::Rgb;

/// Tracer, pebble-handling and control-rod-drive kinematics (feature
/// `animation`). Moved byte for byte from `outram-park-digital-twin-engine`'s
/// `animation/` (gh:#1112, 2026-11-02), which re-exports it at its old path.
/// egui-free, so it builds for Android.
#[cfg(feature = "animation")]
pub mod animation;

/// Render 2-D schematic geometry to a character grid, so a diagram can be
/// checked by a test or an agent (feature `ascii`). Moved byte for byte from
/// `outram-park-digital-twin-engine`'s `ascii/` (gh:#1112, 2026-11-02), which
/// re-exports it at its old path.
#[cfg(feature = "ascii")]
pub mod ascii;

/// The widget art (feature `widgets`): one `egui::Widget` per visual process
/// object, drawn from plain numbers. Moved from
/// `outram-park-digital-twin-engine`'s `components/` (gh:#1112, 2026-11-02),
/// which re-exports it at its old path.
///
/// ~~each composing its physics counterpart (`tampines`, `nee_soon`, …) with
/// visual-only fields~~ **CORRECTED 2026-11-05** (maintainer: "widget art can
/// live in bugis, the combination of logic and art can live inside the
/// digital twin engine"): the widgets that hold physics state moved to
/// `outram_park_digital_twin_engine::components` and draw through the art
/// types here; bugis depends on no physics crate.
#[cfg(feature = "widgets")]
pub mod components;

/// The `eframe`-free app scaffold (feature `app_scaffold`): `SharedState`,
/// monitored physics threads, the crash modal, CSV display and logging, frame
/// metrics, plot history and real-time pacing. Moved from
/// `outram-park-digital-twin-engine`'s `app_scaffold/` (gh:#1112, 2026-11-02),
/// which re-exports it at its old path.
#[cfg(feature = "app_scaffold")]
pub mod app_scaffold;

/// The web-demo framework of the tutorial tracks' browser demos (feature
/// `web_demo`): main view and zoom buttons, folding panel, docks, the
/// no-lag worker/thread link, loading, lessons, platform, worker pool and the
/// nuclear-data cache. Moved from `dhoby-ghaut`'s `web_demo/` (gh:#1112,
/// 2026-11-02), which re-exports it at its old path.
#[cfg(feature = "web_demo")]
pub mod web_demo;

#[cfg(feature = "egui")]
mod gui;
#[cfg(feature = "egui")]
pub use gui::*;
