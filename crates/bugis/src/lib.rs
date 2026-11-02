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

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod colour;
pub mod colour_maps;
pub mod gruvbox;

pub use colour::Rgb;

#[cfg(feature = "egui")]
mod gui;
#[cfg(feature = "egui")]
pub use gui::*;
