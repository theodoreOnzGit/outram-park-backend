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
//! All six families are implemented, each with its own `tests` module checking
//! the correlations against published values or closed-form limits. They are
//! **unit-tested, not system-validated** — none has been exercised inside a
//! converged multiphysics run.
//!
//! - [`fs_drag`] — fluid-structure (wall) Darcy friction-factor correlations.
//!   Implemented; additionally **verified** against the analytic laminar limit
//!   `f·Re → 64`.
//! - [`ff_drag`] — fluid-fluid (interfacial) drag correlations.
//! - [`heat_transfer`] — fluid-structure and fluid-fluid heat-transfer
//!   coefficients, plus critical heat flux.
//! - [`phase_change`] — saturation properties and phase-change source terms.
//! - [`interfacial`] — interfacial area, bubble/droplet diameter, virtual mass,
//!   and the flow-regime map.
//! - [`turbulence`] — the two-phase/porous turbulence **closure algebra**. The
//!   k/ε transport equations and `correctNut` orchestration are deferred; see
//!   that module's header for the precise deferral list.
//!
//! See `docs/genfoam-port-plan.md` for the translation order and per-family
//! tracking beads.

pub mod fs_drag;

pub mod ff_drag;
pub mod heat_transfer;
pub mod interfacial;
pub mod phase_change;
pub mod turbulence;
