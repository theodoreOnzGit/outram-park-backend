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

//! # `genfoam::common` — shared multiphysics utilities
//!
//! Rust port of `GeN-Foam/src/classes/common` (~3.7k LOC): the base helpers the
//! neutronics, thermal-hydraulics, and thermo-mechanics regions build on. It is
//! the **foundation** the other `genfoam` modules code against, so its surface
//! is kept small, dimensionally honest, and thoroughly documented.
//!
//! Generic FV building blocks (tensors, `FvMesh`, fields, `SquareMatrix`,
//! `fvm`/`fvc`) come from [`outram_foam_basic_lib`] and are **not** re-ported
//! here.
//!
//! ## Module map — what lives here
//!
//! | Submodule | Ports | Role |
//! |---|---|---|
//! | [`time_profile`] | `common/timeProfile` | A simulation input as a function of time (external reactivity `ρ(t)`, source power `S(t)`, boron ramps): [`TimeProfile`]. |
//! | [`interpolate_table`] | `common/InterpolateTable` (scalar 1-D) | 1-D lookup table with step/linear method + error/clamp/extrapolate out-of-bounds policy and an integral: [`ScalarInterpolateTable`]. |
//! | [`rbf`] | `common/radialBasisFunctionInterpolation` | N-dimensional polyharmonic-spline radial-basis-function interpolation. Shared by the neutronics cross-section parametrisation ([`crate::genfoam::neutronics::xs`]) and the non-conformal mesh mapping ([`crate::genfoam::multi_region::rbf_mapping`]). |
//!
//! Both are dimensionally honest: their **time abscissa** is a `uom`
//! [`Time`](uom::si::f64::Time), but a tabulated **ordinate is a raw `f64`**
//! because the same generic table serves reactivity (dimensionless), power (W),
//! and concentration (mol/m³) consumers — the consumer attaches the ordinate's
//! unit at its own boundary. Forcing one `uom` unit here would be physically
//! wrong. See each submodule's docs.
//!
//! ## Deferred / folded-into-caller (not ported here)
//!
//! Per `docs/genfoam-port-plan.md`, the remaining `common/` files are either
//! mesh-topology helpers that only the (not-yet-ported) `multi_region` layer
//! needs, or thin things that fold into their caller:
//!
//! - `common/solver` — the abstract run-time-selectable **region-solver base
//!   class** (the PIMPLE region-solver interface). This is Layer-5 solver-loop
//!   logic; it becomes the `NeutronicsModel` / TH-solver **enum dispatch** in
//!   those modules, not a `common` helper. **Deferred** to the TH port.
//! - `common/listOperation` — `stringify` list-to-`word` conversion for
//!   dictionary I/O. Folds into the case-I/O layer; **not needed** by the
//!   physics core.
//! - `common/latticeMap`, `common/mergeOrSplitBaffles` — mesh-topology /
//!   mesh-to-mesh helpers used only by multi-region coupling. **Deferred** to
//!   `genfoam::multi_region` (tracked under the appbuilder epic `op-p6p`).
//! - The 2-D/3-D `FieldField` `InterpolateTableGF` instantiations —
//!   spatial cross-section interpolation. **Deferred** to
//!   `genfoam::neutronics::xs`, which owns the group-XS containers.
//!
//! See `docs/genfoam-port-plan.md` for the full translation order.

pub mod interpolate_table;
pub mod rbf;
pub mod time_profile;

pub use interpolate_table::{
    InterpolateTableError, InterpolationMethod, OutOfBounds, ScalarInterpolateTable,
};
pub use time_profile::{TimeProfile, TimeProfileError};
