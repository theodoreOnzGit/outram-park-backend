// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! # `htr10_rz` — the HTR-10 R-Z solid conduction and radiation model
//!
//! **Original work (gh:#1149 part B, epic #1146), not a GeN-Foam port.** It
//! assembles the INET THERMIX-style R-Z model of IAEA-TECDOC-1694 Sec 2.2.1.2
//! (44 homogenised regions, Tab 2.4, Fig 2.5) on the part-A solid conduction
//! field ([`crate::genfoam::thermal_hydraulics::solid_conduction`]), with the
//! gh:#1148 closures ([`crate::htgr_closures`]) for every property.
//!
//! Maintainer decisions (2026-11-03): D1 proprietary numbers carry their
//! table; D2 this lives in outram-foam and the geometry enters as a cited
//! mesh-builder input ([`geometry`]); D3 Breitbach is the default bed
//! conductivity, ZBS a comparison; D4 the benchmark specification is the
//! base, alternatives are named sensitivity cases; D5 known flaws are
//! carried openly.
//!
//! | Module | What |
//! |---|---|
//! | [`geometry`] | Fig 2.5 mesh lines, the 30 model regions (Tab 2.4 numbers), the cited region rectangles with their confidence |
//! | [`materials`] | homogenised `lambda(T)`, `rho c(T)` from the closures; gas cavities with radiation; the holds outside closure ranges |
//! | [`power_map`] | the FPIC Tab 2.6 power map, volume-exact onto any mesh |
//! | [`model`] | boundaries (RCCS at 50 deg C), the two gh:#1149 (a) cases, solve, report |
//!
//! **Solid only.** There is no gas flow here (that is gh:#1150); the gas
//! side enters as a held temperature or as stagnant cavities.
//!
//! **V&V record:** `verification_and_validation/htr10_rz_solid/README.md`
//! (methodology, the predictions written before the runs, results, the
//! drawings, mesh convergence, sensitivities).
//!
//! Status: untrusted AI-generated draft (2026-11-03) per `RESPONSIBLE_USE.md`.
//! For research, education and V&V only; not for reactor operation,
//! licensing or any safety-critical use. Every region extent is
//! **Uncertain** (read from a figure by the input sheet).

pub mod geometry;
pub mod materials;
pub mod model;
pub mod power_map;

#[cfg(test)]
mod tests;

pub use geometry::{CoreVesselPlacement, ExtentConfidence, RegionSpec};
pub use materials::{MaterialOptions, RegionMaterial, SideReflectorBelow300C};
pub use model::{
    build_mesh, HeadBoundary, Htr10RzCase, Htr10RzModel, Htr10RzOptions, Htr10RzResult,
    RccsView, RegionTemperatureRange,
};

use thiserror::Error;

/// What can go wrong building or solving the HTR-10 R-Z model.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum Htr10RzError {
    /// A caller input is out of range.
    #[error("HTR-10 R-Z input: {0}")]
    Input(String),
    /// A property closure failed inside its range.
    #[error("HTR-10 R-Z property: {0}")]
    Property(String),
    /// The mesh failed to assemble.
    #[error("HTR-10 R-Z mesh: {0}")]
    Mesh(String),
    /// The solid conduction solve failed.
    #[error("HTR-10 R-Z solve: {0}")]
    Solve(String),
}
