// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `genfoam::thermal_hydraulics::boundary_conditions` — GeN-Foam TH boundary conditions
//!
//! GeN-Foam-specific thermal-hydraulics boundary conditions layered on top of
//! basic-lib's generic BC set. Generic OpenFOAM BC machinery (fixedValue,
//! zeroGradient, and the rest of basic-lib's BC set) does **not** belong
//! here — only the GeN-Foam-specific closures below it.
//!
//! Ports upstream `src/classes/thermalHydraulics/src/boundaryConditions/**`
//! (commit 652b3da). Each is a **physical-value closure**: a plain struct
//! plus methods computing boundary values from `uom`-dimensioned inputs
//! (temperatures, time, per-face slices), not a full `fvPatchField` — the
//! per-face mesh loop and "wire this into the solver" plumbing belong to the
//! porous-solver bead (op-p6p.7.11).
//!
//! ## Module map
//!
//! | Submodule | Ports | Status |
//! |---|---|---|
//! | [`blackbody_radiation`] | `blackBodyRadiation` | Ported — [`BlackBodyRadiationBc`] |
//! | [`velocity_rundown`] | `velocityRundown` | Ported — [`VelocityRundownBc`] |
//! | [`time_field_table`] | `timeFieldTable` | Ported — [`TimeFieldTable`] |
//! | [`nusselt_baffle`] | `NusseltThermalBaffle1D` | **Scaffold only** — [`NusseltThermalBaffle1DBc`]; every method is `unimplemented!()`. See the module doc for why (cross-patch implicit coupling) and bead op-p6p.7.13 for the follow-up. |

pub mod blackbody_radiation;
pub mod nusselt_baffle;
pub mod time_field_table;
pub mod velocity_rundown;

pub use blackbody_radiation::BlackBodyRadiationBc;
pub use nusselt_baffle::{BaffleSide, NusseltCorrelationCoefficients, NusseltThermalBaffle1DBc};
pub use time_field_table::{TimeFieldTable, TimeFieldTableError};
pub use velocity_rundown::VelocityRundownBc;
