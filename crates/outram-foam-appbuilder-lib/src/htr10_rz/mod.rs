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
//! | [`flow`] | gh:#1150: KONVEK-like R-Z porous helium flow over the Tab 2.5 flow regions with the bypass paths, and the gas energy equation |
//! | [`pebble`] | gh:#1150: pebble-internal temperatures, the GeN-Foam pebble port adapted to the HTR-10 pebble |
//! | [`fpic`] | gh:#1150: the FPIC full-power case, solid and gas coupled by outer iteration, and the 22 thermocouples |
//! | [`transient`] | gh:#1151 part B: the core in time (backward-Euler solid, quasi-steady gas with buoyancy, lagged fuel zones); the power is an input |
//!
//! ~~**Solid only.** There is no gas flow here (that is gh:#1150); the gas
//! side enters as a held temperature or as stagnant cavities.~~
//! **CORRECTED 2026-11-03 (gh:#1150):** the gas flow is [`flow`], coupled
//! in [`fpic`]; the gh:#1149 cases above remain solid-only (stagnant).
//! **V&V record of the coupled case:**
//! `verification_and_validation/htr10_rz_flow_fpic/README.md`.
//!
//! **V&V record:** `verification_and_validation/htr10_rz_solid/README.md`
//! (methodology, the predictions written before the runs, results, the
//! drawings, mesh convergence, sensitivities).
//!
//! Status: untrusted AI-generated draft (2026-11-03) per `RESPONSIBLE_USE.md`.
//! For research, education and V&V only; not for reactor operation,
//! licensing or any safety-critical use. Every region extent is
//! **Uncertain** (read from a figure by the input sheet).

pub mod flow;
pub mod fpic;
pub mod geometry;
pub mod materials;
pub mod model;
pub mod pebble;
pub mod power_map;
pub mod transient;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod transient_tests;

pub use geometry::{CoreVesselPlacement, ExtentConfidence, RegionSpec};
pub use materials::{CarbonBrickConductivity, MaterialOptions, RegionMaterial, SideReflectorBelow300C};
pub use flow::{BypassSplit, FlowKind, FlowRegion, GasNetwork, GasState, OperatingPoint};
pub use fpic::{reading_at, FpicOptions, FpicResult, Htr10FpicModel, Instrument, Thermocouple, ThermocoupleReading};
pub use transient::{
    dummy_pebble_conductance, CoreFeedbackTemperatures, CoreStepReport, DummyPebbleModel, FuelZoneModel, Htr10RzTransientCore,
    ModeratorTemperature, NaturalCirculation, PowerShape,
    ReflectorAverage, SteadyOuterPass, TransientCoreOptions,
};
pub use pebble::{PebbleMatrix, PebbleProfile};
pub use model::{
    build_mesh, rpv_cavity_enclosure, HeadBoundary, Htr10RzCase, Htr10RzModel, Htr10RzOptions, Htr10RzResult,
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
