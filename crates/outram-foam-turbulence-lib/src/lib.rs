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

//! **This is OUTRAM PARK's independent Rust translation of selected
//! OpenFOAM® turbulence-model algorithms — it is not the official
//! OpenFOAM® software and is not affiliated with, endorsed by, or
//! sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
//! trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
//! directory, mirrored from the workspace root) for the full attribution
//! and non-affiliation notice.
//!
//! # Overview
//!
//! Pure-Rust translation of the OpenFOAM turbulence-closure library: RAS
//! (Reynolds-Averaged Simulation) and LES (Large-Eddy Simulation) models that
//! supply the turbulent-stress and effective-viscosity terms a momentum solver
//! needs. Every model implements the [`traits::TurbulenceModel`] trait; dispatch
//! is static (generics), never `dyn`.
//!
//! # Implementation status (read before depending on a model)
//!
//! Every closure now implements the full [`traits::TurbulenceModel`] contract
//! and is unit-tested; none are `todo!()` scaffolds. **Unit-tested is not
//! benchmark-validated** — the transport formulae, coefficients, and positivity
//! are checked against the upstream OpenFOAM source and analytical values, but
//! ~~the formulae match upstream~~ **CORRECTED 2026-11-13** (gh:#1362): measured
//! against ESI v2506 on a frozen field, the default `correct()` arithmetic differs
//! from upstream in `G` (no `-(2/3)(div U)^2`), the absent `div U` `SuSp` terms,
//! SST gamma1/gamma2 and `GbyNu` limiter, SA sigma (2/3 against 0.66666) and the wall
//! distance (nearest wall-face centre against `meshWave`); see
//! `verification_and_validation/upstream_turb_gh1362/README.md` and the
//! [`source_terms`] module (the ESI form is an option, not the default).
//! no model has yet been validated end-to-end against a published turbulence
//! benchmark, and all use zero-gradient (not wall-function) near-wall boundary
//! conditions. See each module's "Honest scope" section and `README.md`.
//!
//! | Module | Model | Status |
//! |---|---|---|
//! | [`k_omega_sst`] | Menter (1994) k-ω SST | Implemented + unit-tested |
//! | [`laminar`] | No-op laminar (ν_t ≡ 0) | Implemented + unit-tested |
//! | [`k_epsilon`] | Jones & Launder (1972) k-ε | Implemented + unit-tested |
//! | [`k_omega`] | Wilcox k-ω | Implemented + unit-tested |
//! | [`spalart_allmaras`] | Spalart-Allmaras (1992) | Implemented + unit-tested |
//! | [`les`] | Smagorinsky (1963) LES | Implemented + unit-tested |
//! | [`les::wale`] | WALE (Nicoud & Ducros 1999) LES, gh:#1377 | AI draft; terms bit-compared with ESI v2506 (libm ulps pinned) |
//! | [`realizable_ke`] | Realizable k-epsilon (Shih et al. 1995), gh:#1377 | AI draft; terms bit-compared with ESI v2506 (libm ulps pinned) |
//! | [`rng_k_epsilon`] | RNG k-epsilon (Yakhot et al. 1992), gh:#1377 | AI draft; terms bit-identical to ESI v2506 |
//! | [`les::k_eqn`] | kEqn one-equation LES, gh:#1544 | AI draft; terms bit-compared with ESI v2506 (cbrt ulps pinned) |
//! | [`des`] | SpalartAllmaras DES/DDES/IDDES, kOmegaSSTDES, gh:#1545 | AI draft; TERMS ONLY (no `correct()`), bit-compared with ESI v2506 (libm ulps pinned) |
//!
//! [`wall_functions`] provides standalone log-law helpers (`y_plus`, `u_tau`,
//! `nu_t_wall`); they are not yet wired into any model as boundary conditions.
//! See `README.md` ("Limitations") for the full scope/validation caveats.

pub mod des;
pub mod error;
pub mod k_epsilon;
pub(crate) mod k_epsilon_step;
pub mod k_omega;
pub mod k_omega_sst;
pub mod laminar;
pub mod les;
pub mod prelude;
pub mod realizable_ke;
pub mod rng_k_epsilon;
pub mod source_terms;
pub mod spalart_allmaras;
pub mod traits;
pub mod wall_functions;
