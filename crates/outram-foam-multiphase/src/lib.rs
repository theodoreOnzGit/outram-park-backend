// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # outram-foam-multiphase
//!
//! **OUTRAM-FOAM Phase II — multiphase CFD** (bead epic `op-2kk`). Pure-Rust
//! translation of OpenFOAM's multiphase solver family on top of
//! [`outram_foam_basic_lib`]'s finite-volume framework (`FvMesh`, fields,
//! `fvc`/`fvm` operators). This is the **authoritative high-fidelity reference**
//! from which TAMPINES' 1D reduced-order system-code physics (epic `op-dt3`)
//! are derived — 1D models must trace back to a validated 3D reference here,
//! never be invented independently.
//!
//! > **⚠️ Unverified until validated — early/scaffold.** Everything here is a
//! > work-in-progress translation with no human V&V yet. Not for nuclear
//! > facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official OpenFOAM
//! > software (see the workspace `TRADEMARKS.md`).
//!
//! ## Roadmap (per the Phase II architecture; each solver bead has its own DoD:
//! ## theory docs + verification tests + reference-benchmark comparison + uom)
//!
//! - **Stage 1 — Drift Flux** ([`drift_flux`]) — mixture continuity/momentum,
//!   void-fraction transport, algebraic slip / drift-velocity closures
//!   (Zuber-Findlay, terminal velocity, user-defined). Ref OpenFOAM
//!   `incompressibleDriftFlux`. Foundation done (bead `op-2kk.1`); the mixture
//!   momentum + pressure coupling it deliberately leaves out is provided by
//!   [`pimple`] (see below).
//! - **Stage 2 — Euler-Euler two-fluid** ([`two_fluid`]) — per-phase
//!   continuity + drag closures (Schiller-Naumann, Wen-Yu), 6-equation
//!   architecture scaffolded. Ref OpenFOAM `multiphaseEuler`. Foundation done
//!   (bead `op-2kk.2`).
//! - **Stage 3 — Wall boiling framework** ([`wall_boiling`]) — RPI heat-flux
//!   partitioning (Kurul & Podowski). Foundation done (bead `op-2kk.3`).
//! - **Stage 4 — CHF models** ([`chf`]) — Biasi / W-3 / Bowring correlations +
//!   Groeneveld LUT framework. Foundation done (bead `op-2kk.4`).
//! - **Stage 5 — Dryout / post-dryout framework** ([`dryout`]) — reserved
//!   interfaces + Dougall-Rohsenow worked example. Foundation done (`op-2kk.5`).
//!
//! ### Pressure-velocity coupling
//!
//! Two segregated PISO/PIMPLE loops close the momentum + pressure solve the
//! drift-flux and two-fluid foundations deliberately leave out:
//!
//! - [`pimple`] — drift-flux **mixture** PISO/PIMPLE: a Rhie-Chow
//!   pressure-correction loop on the single mixture-momentum field, advancing
//!   `U_m`–`p`–`α` together.
//! - [`two_fluid_pimple`] — **shared-pressure Euler-Euler** PISO: two per-phase
//!   momentum predictors coupled through one mixture-continuity pressure
//!   equation, advancing `U_d`–`U_c`–`p`–`α_d` together.
//!
//! All modules here are **unit-tested foundations, not validated solvers**
//! (verification checks — hydrostatic balance, at-rest stability, boundedness —
//! only; benchmark validation is a later human step) — see each module's
//! "Honest scope".
//!
//! ## Design rules (workspace `CLAUDE.md`)
//!
//! Enum dispatch (no `Box<dyn>`), no lifetime parameters (`Arc`, index ids),
//! `uom`-typed API boundaries, GPLv3 + OpenFOAM provenance headers on ported
//! files, Android-buildable (pure-Rust, no system BLAS/GUI).

pub mod chf;
pub mod drift_flux;
pub mod dryout;
pub mod heat_transfer;
pub mod pimple;
pub mod two_fluid;
pub mod two_fluid_pimple;
pub mod wall_boiling;

/// Errors produced by the multiphase solvers in this crate.
#[derive(Debug, thiserror::Error)]
pub enum MultiphaseError {
    /// A model input was outside its valid physical range.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// A requested feature is scaffolded but not yet implemented.
    #[error("not yet implemented: {0}")]
    NotImplemented(String),
    /// A numerical failure (non-convergence, non-physical state) occurred.
    #[error("solver error: {0}")]
    Solver(String),
}
