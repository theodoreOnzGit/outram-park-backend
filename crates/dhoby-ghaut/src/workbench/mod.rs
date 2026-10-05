//! # The guided high-fidelity simulation workbench (gh:#561)
//!
//! The track-independent half of Dhoby Ghaut's guided workbench: what the
//! wizard offers ([`catalogue`]), the steps it walks through ([`steps`]), and
//! the **recipe**, the input deck a session saves and loads ([`recipe`]).
//! The window itself, and everything that calls a solver, is the
//! `dhoby-ghaut` binary, because the solver crates are this crate's
//! dev-dependencies.
//!
//! **Design (maintainer interview, 2026-10-05).** A guided wizard. The user
//! picks a reactor type by generation, then Basic (start from a pre-built
//! model) or Advanced (from scratch), then walks Steps 0–11 with every value
//! prefilled and each prefilled value linked to its source in the literature
//! pane. Physics defaults ON; a simplification is shown as one, never left
//! silent. Every input and output file is kovan-compatible, which is why this
//! module depends on `kovan` and why the crate is AGPL-3.0-only (`NOTICE`).
//!
//! **Status (2026-10-05): first slice.** The wizard shows all twelve steps;
//! ~~Steps 0–5 work for HTGR → Basic → pebble bed (HTR-10); Steps 6–11 are
//! listed with the issue that will build each~~ **UPDATED 2026-10-05:** Steps
//! 0–5, 7 ([`meshes`], gh:#572) and 8 ([`mgxs`], gh:#573) work for HTGR →
//! Basic → pebble bed (HTR-10); the rest are listed with the issue that will
//! build each ([`steps::WizardStep::issue`]). [`meshes`] and [`mgxs`] hold the
//! hand-off types the multiphysics steps (9, 10) read.
//!
//! Not built for wasm32: `kovan` is gated off the browser (see `Cargo.toml`).

pub mod catalogue;
pub mod meshes;
pub mod mgxs;
pub mod recipe;
pub mod steps;
