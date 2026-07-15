// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
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

//! # `genfoam::neutronics` — deterministic reactor neutronics
//!
//! Rust port of GeN-Foam's `src/classes/neutronics/`. GeN-Foam offers several
//! run-time-selectable neutronics models; upstream they share the abstract
//! `Foam::neutronics` base class. In this port the model set is a **closed enum**
//! (per the workspace no-`dyn`-dispatch rule) rather than a virtual base — adding
//! a model forces every `match` site to handle it.
//!
//! ## What belongs here
//!
//! - [`point_kinetics`] — 0-D point-kinetics (implemented).
//! - Multigroup diffusion, SP3, SN, adjoint diffusion (planned — see
//!   `docs/genfoam-port-plan.md`).
//! - The shared flux / power / precursor / power-density state and the
//!   cross-section (`XS`) data structures (planned).
//!
//! Cross sections are read from GeN-Foam `nuclearData` dictionaries; this module
//! does **not** pull data from the NJOY / Monte Carlo crates.

pub mod point_kinetics;
