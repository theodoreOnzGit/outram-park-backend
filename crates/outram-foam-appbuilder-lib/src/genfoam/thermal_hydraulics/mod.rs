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

//! # `genfoam::thermal_hydraulics` — reactor thermal-hydraulics
//!
//! Rust port of `GeN-Foam/src/classes/thermalHydraulics` (~65k LOC — by far the
//! largest GeN-Foam module): the single- and two-phase reactor thermal-hydraulics
//! (porous-medium momentum/energy for the core, sub-channel and pin models, the
//! fluid/structure heat-transfer closures, and the turbulence/friction
//! correlations) that supplies the temperature/density feedback to the
//! neutronics.
//!
//! Generic FV building blocks (fields, `fvm`/`fvc` operators, matrices) come from
//! [`outram_foam_basic_lib`]; turbulence closures from
//! [`outram_foam_turbulence_lib`]. This module ports GeN-Foam's reactor-specific
//! TH extensions on top of those, NOT the generic FV machinery.
//!
//! **Port in progress — this is a large, multi-slice sub-effort.** See
//! `docs/genfoam-port-plan.md` (the "thermalHydraulics breakdown" section) for
//! the sub-module map, translation order, and per-sub-module beads
//! (`op-p6p.7.1` … `op-p6p.7.14`).
//!
//! ## Sub-module map
//!
//! - [`units`] — named `uom` aliases (`ReynoldsNumber`, `DarcyFrictionFactor`,
//!   `HeatTransferCoefficient`, `HeatFlux`). **Implemented.**
//! - [`closures`] — the `physicsModels/` correlation leaves. Of these,
//!   [`closures::fs_drag`] (fluid-structure wall friction) is **implemented and
//!   verified**; the rest are scaffolded.
//! - [`phase`], [`structure`] — fluid-phase and solid-structure field state
//!   (scaffold).
//! - [`solver`] — the porous single-/two-phase solver drivers (scaffold).
//! - [`boundary_conditions`], [`function_objects`], [`thermophysical`] — TH
//!   boundary conditions, diagnostics, and bespoke fluid properties (scaffold).
//!
//! Only [`units`] and [`closures::fs_drag`] carry real physics so far; every
//! other sub-module is a documented `// TODO(genfoam)` stub with a tracking bead.

pub mod boundary_conditions;
pub mod closures;
pub mod function_objects;
pub mod phase;
pub mod solver;
pub mod structure;
pub mod thermophysical;
pub mod units;
