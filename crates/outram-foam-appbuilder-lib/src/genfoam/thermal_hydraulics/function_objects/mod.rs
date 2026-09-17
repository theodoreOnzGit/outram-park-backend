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

//! # `genfoam::thermal_hydraulics::function_objects` — TH post-processing diagnostics
//!
//! Run-time post-processing and diagnostic function objects, ported from
//! upstream `src/classes/thermalHydraulics/src/functionObjects/**`. These are
//! **pure post-processing/monitoring hooks over solver fields — no physics is
//! computed here.** No solver, closure, or run-loop logic belongs in this
//! module; every function here is a stateless reduction over a
//! `VolField`/`SurfaceField`/`FvMesh` snapshot passed in by the caller (the
//! eventual solver driver decides *when* and *how often* to call these — that
//! wiring is out of scope here, per bead op-p6p.7.14).
//!
//! ## Sub-module map
//!
//! | Module | Upstream function object | What it computes |
//! |---|---|---|
//! | [`mass_flow`] | `massFlow` | Total mass flow rate through a patch/face set: `mdot = sum\|alphaRhoPhi\|` |
//! | [`pressure_drop`] | `pressureDrop` | Area-weighted-average pressure difference between two patches (or a patch vs. a reference) |
//! | [`t_bulk`] | `TBulk` | Flow-weighted (bulk/mixing-cup) temperature over a patch |
//! | [`patch_scalar_value`] | `patchScalarFieldValue` | Raw per-face patch values, plus a selectable reduction (sum/average/min/max/integral) |
//! | [`field_diff_extents`] | `fieldDiffExtents` | Spatial bounding-box extents of where one field exceeds a mask field (scalar only) |
//! | [`stop_if_max_field_diff`] | `stopIfMaxFieldDiff` | Stop-criterion decision: `max_cell(field1 - field2) > 0` |
//! | [`field_integral`] | `fieldIntegralToFMU` | Volume integral `sum(field * V)` (the FMU co-simulation export itself is out of scope) |
//!
//! Every module documents exactly where it is a literal port vs. a
//! documented simplification/generalisation relative to the upstream C++ —
//! see each module's doc comment before assuming 1:1 behavioural parity.
//!
//! ## Design notes
//!
//! - **No `dyn`/trait-object dispatch.** Where upstream uses a `regionType`
//!   enum + `switch`, this port uses a plain closed Rust enum
//!   ([`mass_flow::MassFlowRegion`], [`patch_scalar_value::PatchReduceOp`]).
//! - **`uom` at the API boundary.** Every function returning a genuine
//!   physical quantity (mass rate, pressure, temperature, area) returns a
//!   named `uom::si::f64` type, not a bare `f64`. The two exceptions —
//!   [`patch_scalar_value::reduce_patch_scalar_field`] and the
//!   [`field_integral`] functions — operate on a field of *generic*,
//!   caller-defined physical meaning (any scalar field), so a single named
//!   `uom` quantity would misrepresent them; both document this explicitly.
//! - **Panics over silent fallback**, per this crate's guardrails: an
//!   out-of-range patch index, a field/mesh size mismatch, or an empty
//!   reduction domain panics rather than returning a default value.

pub mod field_diff_extents;
pub mod field_integral;
pub mod mass_flow;
pub mod patch_scalar_value;
pub mod pressure_drop;
pub mod stop_if_max_field_diff;
pub mod t_bulk;

mod patch_geometry;
