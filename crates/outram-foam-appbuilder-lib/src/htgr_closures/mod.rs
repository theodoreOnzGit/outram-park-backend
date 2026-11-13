// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # `htgr_closures` — pebble-bed HTGR thermal-hydraulic closures
//!
//! Algebraic closures for a helium-cooled pebble bed and the graphite and
//! steel structures around it. They are the inputs a 2-D (r-z) porous or
//! conduction solve, or a 0-D/1-D `tampines` model, needs.
//!
//! **This is original work, not a GeN-Foam port.** It sits beside
//! [`crate::genfoam`] because of maintainer decision D2 (2026-11-03, gh:#1146):
//! 2-D, CFD-like, pure thermal-hydraulics belongs in `outram-foam`, and
//! `tampines` depends on this crate, so a closure placed here reaches both.
//! Nothing here is ported from GeN-Foam source.
//!
//! ## What is here
//!
//! | Module | Closure | Source |
//! |---|---|---|
//! | [`bed_conductivity`] | Bed effective conductivity, enum [`BedConductivityModel`]. **Default: Breitbach** (D3) | IAEA-TECDOC-1694 Tab. 2.9; Kugeler et al. 2017 eq. (108)-(109) |
//! | [`zbs`] | Zehner-Bauer-Schlunder bed conductivity, a comparison variant | moved from `tampines::pebble_bed::zbs` |
//! | [`kta`] | KTA 3102.2 pebble-to-gas Nusselt number; KTA 3102.3 bed pressure drop | IAEA-TECDOC-1694 Tab. 2.10 |
//! | [`helium`] | The KTA 3102.1 helium fits as printed in the benchmark, with the pressure unit **fixed to bar** | IAEA-TECDOC-1694 Tab. 2.8 |
//! | [`structural_materials`] | Fuel-element, side/top/bottom-reflector and carbon-brick conductivity; steel conductivity digitised from a figure | IAEA-TECDOC-1694 Tab. 2.9, 2.11, 2.12, Fig. 2.8 |
//!
//! ## Provenance and data policy
//!
//! IAEA-TECDOC-1694 (2013), *Evaluation of HTGR Performance: Benchmark
//! Analysis Related to the PBMR-400, PBMM, GT-MHR, HTR-10 and the ASTRA
//! Critical Facility*, Chapter 2, is a **proprietary-tier** document
//! (an IAEA publication with no reuse licence; held in the maintainer's
//! private literature repository as `iaea2013tecdoc1694`). Following
//! maintainer decision D1 (2026-11-03) and the precedent of
//! `docs/reactor-scoping/htr10-plant-data.md`, only **coefficients and
//! formulas** enter this repository, each with its table number. No text is
//! copied. Every coefficient here was re-read on 2026-11-03 from the PDF with
//! `pdftotext` and checked against the page rendered at 400 dpi.
//!
//! Kugeler, Nabielek & Buckthorpe (2017), *The High Temperature Gas-cooled
//! Reactor: Safety considerations of the (V)HTR-Modul*, EUR 28712 EN, is
//! **open** (EU reuse decision 2011/833/EU) and is in
//! `crates/kovan-literature/reactor-literature/kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf`.
//!
//! ## Status
//!
//! **Verified, not validated.** Each closure is checked against hand
//! evaluations of the printed formula, the benchmark's own consistency
//! checks, or an independent source (CoolProp helium, the open Kugeler fit).
//! No closure has been compared against an HTR-10 measurement here.
//! AI-assisted draft pending human review per `RESPONSIBLE_USE.md`. Not for
//! reactor operation, licensing or any safety-critical use. The V&V record is
//! `verification_and_validation/htr10_th_closures_vs_tecdoc1694.md`.

pub mod bed_conductivity;
pub mod helium;
pub mod kta;
pub mod structural_materials;
pub mod zbs;

pub use bed_conductivity::{BedConductivityModel, BedState};
pub use helium::KtaHelium;
pub use kta::{
    kta_3102_2_nusselt, kta_3102_3_friction_factor, kta_3102_3_pressure_gradient, MassFlux,
    PressureGradient,
};
pub use structural_materials::{SteelComponent, StructuralMaterial};
pub use zbs::ZbsBed;

use thiserror::Error;

/// Errors from the HTGR closures.
///
/// A closure never clamps a bad input into range silently. It reports it, so a
/// caller cannot get a plausible-looking number from a state the correlation
/// does not describe.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum HtgrClosureError {
    /// A caller-supplied input is outside the correlation's domain (a
    /// non-positive conductivity or temperature, a porosity outside (0, 1),
    /// a negative fluence). Caught before any physics runs.
    #[error("HTGR closure invalid input: {0}")]
    InvalidInput(String),

    /// The correlation left the region where it is physical: a unit-cell
    /// denominator crossed zero, or a printed fit returned a non-positive
    /// conductivity.
    #[error("HTGR closure unphysical: {0}")]
    Unphysical(String),

    /// Arithmetic produced a non-finite result.
    #[error("HTGR closure numerical failure: {0}")]
    Numerical(String),
}
