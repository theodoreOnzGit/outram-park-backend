// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dragModels/
//             FFDragCoefficientModels/  (Wallis, SchillerNaumann, Bestion,
//             BestionTRACE, Autruffe, NoKazimi)
//             twoPhaseDragMultiplierModels/  (ChenKalish, constant, Kaiser74,
//             Kaiser88, KottowskiSavatteri, LockhartMartinelli, LottesFlinn,
//             LottesFlinnNguyen)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (sradman@protonmail.com; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `closures::ff_drag` — fluid-fluid interfacial drag & two-phase multipliers
//!
//! Rust port of GeN-Foam's `physicsModels/dragModels/{FFDragCoefficientModels,
//! twoPhaseDragMultiplierModels}`. Two independent, closed enum families (no
//! `dyn` dispatch):
//!
//! - [`interfacial::FfDragCoefficient`] — interfacial drag between the two
//!   fluid phases (Wallis, SchillerNaumann, Bestion/BestionTRACE, Autruffe,
//!   NoKazimi), evaluated on an [`interfacial::FfInterfacialState`].
//! - [`multipliers::TwoPhaseDragMultiplier`] — two-phase friction multipliers
//!   applied on top of a single-phase (fluid-structure) drag
//!   (LockhartMartinelli, ChenKalish, Kaiser74/88, KottowskiSavatteri,
//!   LottesFlinn, LottesFlinnNguyen, constant).
//!
//! Belongs here: the pure-algebra fluid-fluid momentum-coupling correlations.
//! Does **not** belong here: the fluid-structure wall friction
//! ([`super::fs_drag`]), heat transfer, or assembling the drag tensor fields
//! from these coefficients (mesh/field state — the solver's job, bead
//! op-p6p.7.11).
//!
//! Tracked by bead op-p6p.7.5; see `docs/genfoam-port-plan.md`.

mod interfacial;
mod multipliers;

pub use interfacial::{FfDragCoefficient, FfInterfacialState};
pub use multipliers::TwoPhaseDragMultiplier;

#[cfg(test)]
mod tests;
