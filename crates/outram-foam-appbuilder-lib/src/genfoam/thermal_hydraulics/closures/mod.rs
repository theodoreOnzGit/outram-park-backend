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

//! # `genfoam::thermal_hydraulics::closures` — TH closure correlations
//!
//! Rust port of GeN-Foam's `src/classes/thermalHydraulics/src/physicsModels/`
//! (~35k LOC, the single largest part of the module). These are the algebraic
//! **leaves** of the thermal-hydraulics model: small, self-contained functions
//! of local field values (Reynolds number, void fraction, quality, …) that feed
//! the porous momentum, energy, and phase-transport equations in
//! [`super::solver`].
//!
//! Upstream each correlation family is an OpenFOAM `runTimeSelectionTable` of a
//! virtual base class. Per the workspace no-`dyn`-dispatch rule, each family is
//! translated to a **closed enum** with one variant per correlation and a
//! `match`-based dispatch method — adding a correlation forces every dispatch
//! site to handle it.
//!
//! ## Sub-modules
//!
//! - [`fs_drag`] — fluid-structure (wall) Darcy friction-factor correlations.
//!   **Implemented + verified.**
//! - `ff_drag`, `heat_transfer`, `phase_change`, `interfacial`, `turbulence` —
//!   scaffolded (`// TODO(genfoam)`); see `docs/genfoam-port-plan.md`.

pub mod fs_drag;

pub mod ff_drag;
pub mod heat_transfer;
pub mod interfacial;
pub mod phase_change;
pub mod turbulence;
