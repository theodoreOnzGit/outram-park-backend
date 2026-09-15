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
//! Every sub-module below is ported and carries unit tests against published
//! correlation values or closed-form results. What remains unported is listed
//! under "Known gaps".
//!
//! - [`units`] — named `uom` aliases (`ReynoldsNumber`, `DarcyFrictionFactor`,
//!   `HeatTransferCoefficient`, `HeatFlux`). Implemented.
//! - [`closures`] — the `physicsModels/` correlation leaves: `fs_drag`,
//!   `ff_drag`, `heat_transfer`, `phase_change`, `interfacial` and
//!   `turbulence`. All six families are implemented with their own `tests`
//!   modules; [`closures::fs_drag`] additionally carries an analytic
//!   verification (laminar `f·Re → 64`).
//! - [`phase`] / [`structure`] — fluid-phase and solid-structure field state,
//!   including the power/heat-exchanger/pump structure models. Implemented.
//! - [`solver`] — the porous solver drivers. [`solver::one_phase`] (UEqn/pEqn/
//!   EEqn) is implemented; see "Known gaps" for its property limitation.
//! - [`boundary_conditions`] — `blackbody_radiation`, `velocity_rundown` and
//!   `time_field_table` implemented.
//! - [`function_objects`] — post-processing diagnostics (mass flow, pressure
//!   drop, bulk temperature, field diffs). Implemented.
//! - [`thermophysical`] — the bespoke dissociating-hydrogen (H/H₂) property
//!   package: EOS, thermodynamics, viscosity, conductivity. Implemented.
//!
//! ## Known gaps
//!
//! - **The two-phase (MULES) solver is not implemented**, nor is
//!   `onePhaseLegacy`. Only [`solver::one_phase`] exists.
//! - [`solver::one_phase`] runs on **constant fluid properties** (`he = Cp·T`,
//!   fixed-surface-temperature structure coupling): [`thermophysical`] is
//!   ported but not yet wired in as the driver's fluid package.
//! - `boundary_conditions::nusselt_baffle` is a **stub** — every method is
//!   `unimplemented!()` (cross-patch implicit coupling is not supported).
//! - [`closures::turbulence`] ports the closure *algebra* only; the k/ε
//!   transport equations and `correctNut` orchestration are deferred (the
//!   generic single-phase machinery lives in `outram-foam-turbulence-lib`).
//! - The correlation leaves are **unit-tested, not system-validated** — they
//!   have not been exercised inside a converged multiphysics run.
//! - The great majority of upstream `thermalHydraulics` (~65k LOC) is still
//!   unported; what exists here is the closure/field/one-phase-driver
//!   foundation.

pub mod boundary_conditions;
pub mod closures;
pub mod function_objects;
pub mod phase;
pub mod solver;
pub mod structure;
pub mod thermophysical;
pub mod units;
