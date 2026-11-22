//! # EBR-II: cited benchmark data and a web-ready SHRT-17 / SHRT-45R model
//!
//! > **Research, education and V&V only.** Not for reactor operation,
//! > licensing, safety-critical decisions or any operational purpose
//! > (`RESPONSIBLE_USE.md`). EBR-II was shut down in 1994; these are
//! > published benchmark specifications, not operational data.
//!
//! What a web simulation author needs to build an EBR-II loss-of-flow
//! simulator from this crate alone, which then reaches the `outram-park`
//! metacrate (gh:#1331, epic gh:#1312):
//!
//! | Module | Contents |
//! |---|---|
//! | [`design`] | Subassembly, fuel, core and plant numbers; the NATDEMO pump coefficients; IHX geometry; U-5Fs properties |
//! | [`shrt`] | SHRT-17 / SHRT-45R test descriptions, initial conditions and the tabulated boundary conditions, as interpolating functions of time ([`shrt::ShrtTest`]) |
//! | [`kinetics`] | Argonne's beta-eff, generation time and feedback coefficients (IAEA-TECDOC-1819 TABLE 37) |
//! | [`pump`] | The NATDEMO homologous pump head curve |
//! | [`model`] | [`model::Ebr2Model`], the stepping model: `shrt17()` / `shrt45r()`, `step(dt)`, getters and setters |
//!
//! ```no_run
//! use outram_park_digital_twin_engine::ebr2::model::Ebr2Model;
//! use uom::si::f64::*;
//! use uom::si::length::meter;
//! use uom::si::time::second;
//!
//! // The IHX-above-core thermal-centre height is not in the sources: the
//! // caller must choose it (see `model`'s module doc).
//! let mut m = Ebr2Model::shrt17(Length::new::<meter>(2.0))?;
//! while m.time().get::<second>() < 900.0 {
//!     m.step(Time::new::<second>(0.5))?; // cheap: 10 sub-steps
//!     let _t_out = m.core_outlet_temperature();
//! }
//! # Ok::<(), outram_park_digital_twin_engine::ebr2::model::Ebr2Error>(())
//! ```
//!
//! ## Web demos: keep the UI thread free
//!
//! A step is cheap: about a hundred property evaluations and a 7 x 7
//! kinetics solve per 0.05 s sub-step. Even so, run a long fast-forward (a
//! whole 900 s test) in a worker and stream the state, as the mobile-first
//! rule requires (`docs/claude-md/mobile-first-tutorials-and-demos.md`).
//! The model is `Clone`, owns all its data, and has no GUI types, I/O or
//! randomness.
//!
//! ## Sources and their status
//!
//! - ANL-ARC-226 Rev. 1 (Sumner & Wei 2012) and ANL-ARC-228 Rev. 1 (Fei,
//!   Mohamed & Kim 2013), through the transcriptions in
//!   `reference-data/ebr2/` (pinned by `csv_equality`).
//! - IAEA-TECDOC-1819 (2017) and further ARC-226/228 values, through the
//!   transcriptions in gh:#1313, gh:#1314 and gh:#1315. No CSV holds these
//!   yet, so no test pins them.
//!
//! Used under the maintainer's EBR-II data exception (`DATA_POLICY.md`,
//! 2026-11-10: numbers only, cited per value; extended the same day to "any
//! citable information in scientific literature"). **Every transcription is
//! an AI transcription, not yet reviewed by a human.** Check a value against
//! its source before relying on it.
//!
//! ## A maintainer-directed exception to "no new physics in this library"
//!
//! Like [`crate::htr10`], this module carries cited data. Unlike it, it also
//! carries a **transient model** ([`model`]), because the maintainer asked
//! for EBR-II "in library so I can build web simulations from the
//! outram-park metacrate" (2026-11-10, gh:#1331). The model composes
//! existing physics:
//!
//! - kinetics and feedback from `teh-o-prke`;
//! - sodium properties and liquid-metal Nusselt correlations from
//!   `tuas_boussinesq_solver`.
//!
//! The lumped coupling, the pump curve and the IHX are new. They are
//! derived from the cited geometry and run **uncalibrated**.
//!
//! ## Status: NOT VALIDATED
//!
//! The tests establish three things, and no more:
//!
//! - the data equal their transcriptions;
//! - the model is finite and bounded over both tests;
//! - the model's own steady state, compared with the cited initial
//!   conditions, misses by the amounts recorded in
//!   `crates/outram-park-digital-twin-engine/docs/ebr2_library.md`.
//!
//! The measured SHRT transients exist only as plots (gh:#1316). Their
//! acceptance bands are the maintainer's call, so no transient is
//! validated here.
//!
//! GUI-free and not target-gated. It builds for `wasm32-unknown-unknown`.

pub mod design;
pub mod kinetics;
pub mod model;
pub mod pump;
pub mod shrt;

#[cfg(test)]
mod csv_equality;
#[cfg(test)]
mod model_tests;
