//! **The web-demo framework**: the track-independent half of OUTRAM PARK's
//! browser demos, so every tutorial track (Monte Carlo, nuclear data,
//! dispersion, fuel performance, …) builds its demo on the same pieces instead
//! of copying them (gh:#521; extracted from `examples/monte_carlo_web/`, the
//! first app built on it, on 2026-10-04).
//!
//! It exists to make two workspace HARD RULES cheap to obey
//! (`docs/claude-md/mobile-first-tutorials-and-demos.md`):
//!
//! - **Mobile-first.** [`view::View`] (zoom about a point, fit to the screen),
//!   [`view::zoom_buttons`] (+ / − / Reset, always on the main view),
//!   [`panel::Panel`] (a side panel that folds, opens folded on a narrow
//!   screen, with a "« Hide" button in it and a "Controls »" button on the
//!   main view), [`view::scale_bar`].
//! - **No lagging.** [`link`]: the UI thread only draws. Physics runs in a
//!   background thread natively ([`link::NativeEngine`]) or a Web Worker in the
//!   browser ([`link::WorkerEngine`]) running the SAME wasm module, and talks
//!   to the UI only in messages, through a [`link::Link`] the UI drains each
//!   frame without blocking. Long work is split into steps (a nuclide, a
//!   generation, a batch) so results stream and it can be paused.
//!
//! Plus the pieces every track repeats: [`loading::Loading`] (progress of a
//! list of data-processing jobs, as a card on the main view and a table in the
//! panel), [`lesson`] (the rung table and the "What's happening here?" link
//! to the lesson page), [`platform`] (clock, page title, URL query), and
//! [`pool`] (how many Web Workers to start for work that splits, sized to
//! the device's cores, memory and screen; gh:#786).
//!
//! # A new track app, step by step
//!
//! 1. **An example** `examples/<track>_web/main.rs` (with its Android stub
//!    `main`, as `monte_carlo_web` has), declared in `Cargo.toml` like
//!    `monte_carlo_web`. In `main` on wasm, run your [`link::WorkerEngine`]
//!    with [`link::worker_main`] when the module finds itself in a worker,
//!    else start the eframe app.
//! 2. **Messages**: a `Request` and an `Event` enum, each implementing
//!    [`link::Message`] (on wasm it turns them into JS objects; use
//!    [`link::js`]).
//! 3. **The engine**: one type implementing [`link::NativeEngine`] (native
//!    thread) and [`link::WorkerEngine`] (browser worker); both just call your
//!    track's own "serve one request" function.
//! 4. **The app**: an `eframe::App` holding a [`link::Link`], a
//!    [`panel::Panel`], a [`view::View`] and, while data load, a
//!    [`loading::Loading`]. Each frame: `link.drain()`, handle the events,
//!    `panel.show(..)`, then draw the main view and call
//!    [`view::zoom_buttons`] and `panel.reopen_button(..)` on it.
//! 5. **Rungs** (optional): an enum implementing [`lesson::Rung`] gives the
//!    `?rung=` URL parameter and the lesson link.
//! 6. **The web shell**: copy `web/monte_carlo/{index.html, worker.js,
//!    build.sh}`, renaming the module, and add the build to
//!    `scripts/build-pages.sh`.
//!
//! Everything here is presentation and plumbing: no physics (the crate rule).
//! Gated off Android with the rest of the windowing stack.

pub mod lesson;
pub mod link;
pub mod loading;
pub mod panel;
pub mod platform;
pub mod pool;
pub mod view;
