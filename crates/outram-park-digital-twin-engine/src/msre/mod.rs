//! # MSRE in one place: cited plant data and a web-ready stepping model
//!
//! Everything a web-simulation author needs to build a Molten-Salt Reactor
//! Experiment simulator, under one path. Through the `outram-park` metacrate
//! it is `outram_park::backend::outram_park_digital_twin_engine::msre`.
//!
//! | Path | What | Origin |
//! |---|---|---|
//! | [`design`] | Cited plant constants (TM-0728 design point, TM-1070 kinetics and lumps, TM-1626 measured circulation loss) | here, re-exporting `nee_soon`'s where they exist |
//! | [`model::MsreModel`] | A stepping simulator: circulating-fuel kinetics with fuel and graphite feedback, heat exchanger, coolant loop, radiator, piping lags | here; composes `teh_o_prke` and TM-1070's equations |
//! | [`frequency_response`] | The linear frequency-domain model and TM-1647's measured natural periods (MSRE T-1, gh:#1132) | `nee_soon::msre_frequency_response` |
//! | [`first_criticality`] | The full-core Monte Carlo geometry and materials of first criticality (MSRE N-1, gh:#1128) | `outram_mc_libs::vv::msre_n1` |
//! | [`salts`] | The fuel and coolant salts as TUAS liquids (MSRE TH-6, gh:#1133) | `tuas_boussinesq_solver` |
//! | [`circulating_kinetics`] | The circulating-fuel point kinetics alone (MSRE T-C1, gh:#1131) | `teh_o_prke::zero_power_prke::circulating` |
//!
//! ## Using it from a web demo
//!
//! [`model::MsreModel::step`] costs about a microsecond per 0.01 s of plant
//! time (measured in the tests, `docs/msre_library.md`), so a Web Worker can
//! run it faster than real time and post the getters' values to the page.
//! Per the workspace's no-lag rule the UI thread should still not call it in
//! a loop; it holds no GUI type, no clock and no thread, so it moves into a
//! worker unchanged.
//!
//! ## The library rule this module is an exception to
//!
//! This crate's library holds no new physics (crate `AGENTS.md`). [`model`]
//! is a maintainer-directed exception (2026-11-10, gh:#1317, gh:#1330: "I
//! would like to have MSRE and EBR-II in library so I can build web
//! simulations from the outram-park metacrate"), on the pattern of
//! [`crate::htr10`]: cited inputs, reuse of the workspace's MSRE pieces, and
//! tests that reproduce published numbers. What it adds that is new is listed
//! in [`model`]'s docs and was run uncalibrated.
//!
//! The full-scope MSRE simulator stays in the MSRE repository
//! (`molten-salt-reactor-lit-and-sims`, maintainer 2026-11-03, gh:#1125);
//! this is the reduced-order model a browser demo runs.
//!
//! ## Status: NOT VALIDATED
//!
//! The tests reproduce published numbers and a code-to-code check; they do
//! not validate a simulator. AI-drafted (Claude Opus 5.5 in Claude Code),
//! untrusted until reviewed. Research, education and V&V only; an offline
//! demonstration, never for operation (`RESPONSIBLE_USE.md`).
//!
//! GUI-free and not target-gated: it builds for `wasm32-unknown-unknown`.

pub mod design;
pub mod model;

/// MSRE T-1's linear frequency-domain model and TM-1647's measured natural
/// periods: `nee_soon::msre_frequency_response`, re-exported.
pub use nee_soon::msre_frequency_response as frequency_response;
/// MSRE N-1's full-core Monte Carlo model (geometry, materials, source box):
/// `outram_mc_libs::vv::msre_n1`, re-exported.
pub use outram_mc_libs::vv::msre_n1 as first_criticality;
/// Circulating-fuel point kinetics (MSRE T-C1): `teh_o_prke`'s, re-exported.
pub use teh_o_prke::zero_power_prke::circulating as circulating_kinetics;

/// The MSRE salts as TUAS liquids (MSRE TH-6, gh:#1133), re-exported from
/// `tuas_boussinesq_solver`'s liquid database.
pub mod salts {
    pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_coolant_salt_tm2316;
    pub use tuas_boussinesq_solver::boussinesq_thermophysical_properties::liquid_database::msre_fuel_salt_tm0728;
}
