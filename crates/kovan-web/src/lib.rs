//! # kovan-web — web-kovan, the Code Review UI
//!
//! **KOVAN-WEB**: **K**nowledge **O**riented **V**&V **A**nalysis for
//! **N**uclear **S**ciences, on the **Web**. The read-only Code Review UI of the kovan
//! family (GitHub #735, #736, #738), written once in egui so that the same
//! UI runs in the browser (wasm32, published on GitHub Pages at
//! `code-review/`) and inside desktop kovan's Code Review tab (#820).
//!
//! ## What it shows
//!
//! ```text
//!  code map ──click crate──► crate view ──open module──► module view
//!  (every crate in its      (the crate's whole mod     (its functions as a
//!   topic box; ▸ shows a     tree as module cards,      ring; ▸ callees and
//!   crate's top modules      examples as a group;       ◂ callers unfold,
//!   in place)                no functions)              level by level)
//!                                                         │ select a function
//!                                                         ▼
//!                      review bar: stamp state, maturity, blocked-by links
//!                      source panel: the function at the built commit
//! ```
//!
//! ## Modes
//!
//! [`Mode::Web`] is read-only: the published site.
//! [`Mode::Desktop`] is the same UI embedded in desktop kovan's Code Review
//! tab (#740, #820), with the review bar's Stamp and Needs-fix enabled. This
//! crate does not stamp: a press queues a `ui::HostRequest` naming the
//! function (`ui::FunctionRef`), the host takes it with
//! `CodeReview::take_host_request` and runs its own dialog, and pushes the
//! new stamp states back with `CodeReview::set_stamps`. In desktop mode the
//! host's theme is kept.
//!
//! ## Layout of the crate
//!
//! - [`model`]: the pure model (module tree, crate-view layout, call
//!   hierarchy layout, deep links, stamp lookups, rustdoc URLs, search).
//!   No egui; tested on every target.
//! - [`data`]: background loading into `Arc<RwLock<_>>` shared state.
//! - [`platform`]: clock, URL hash, browser differences.
//! - `ui` (not on Android): the egui UI, [`ui::CodeReview`].
//!
//! The data and layout types come from `kovan-common` (`code_map`,
//! `call_graph`, `mindmap_view`, `geometry`, `fuzzy`), which were moved
//! there from `kovan` on 2026-10-06 so this crate needs nothing from the
//! AGPL `kovan` crate and builds for wasm32.

#![forbid(unsafe_code)]

pub mod data;
pub mod model;
pub mod platform;
pub mod search;
#[cfg(not(target_os = "android"))]
pub mod ui;

/// Which front end the UI is running in (#736).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// web-kovan: read-only, static JSON from the site build.
    Web,
    /// Desktop kovan: the review bar's Stamp and Needs fix are enabled and
    /// handed to the host as a `ui::HostRequest` (#740).
    Desktop,
}

impl Mode {
    /// Whether the review bar's Stamp and Needs-fix actions are enabled.
    pub fn can_stamp(self) -> bool {
        matches!(self, Mode::Desktop)
    }
}
