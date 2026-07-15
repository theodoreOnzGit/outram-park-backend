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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `genfoam` — Rust port of GeN-Foam's reactor-multiphysics solver
//!
//! GeN-Foam (Generalized Nuclear Foam) is an OpenFOAM-based multiphysics solver
//! for nuclear reactors, coupling **neutronics** (point-kinetics, multigroup
//! diffusion, SP3, SN), **thermal-hydraulics** (single- and two-phase), and
//! **thermo-mechanics** (thermal-expansion feedback) across multiple meshes.
//!
//! This subtree is OUTRAM PARK's independent Rust translation of GeN-Foam
//! (upstream commit `652b3da`, GPL-3.0). It is kept cleanly separated from this
//! crate's hand-written OpenFOAM solver ports (`crate::solvers`) and case I/O
//! (`crate::io`).
//!
//! ## What belongs here
//!
//! Reactor-physics models specific to GeN-Foam: the neutronics models and their
//! reactivity-feedback machinery, the multi-region coupling that maps fields
//! between the neutronics / TH / TM meshes, and the reactor-specific
//! thermal-hydraulics extensions. The generic FV building blocks these rest on
//! (tensors, `FvMesh`, fields, `SquareMatrix`, `fvm`/`fvc` operators) come from
//! [`outram_foam_basic_lib`] and are **not** re-ported here.
//!
//! ## What does not belong here
//!
//! Cross-section generation and Monte Carlo transport. GeN-Foam's neutronics is
//! deterministic and self-contained; it reads GeN-Foam-format `nuclearData`
//! dictionaries. It does **not** depend on `njoy-outram-park-fork` or
//! `outram-mc-libs`.
//!
//! ## Port status
//!
//! This is an incremental, multi-session port. See
//! `docs/genfoam-port-plan.md` for the full module map and translation order.
//! Currently implemented:
//!
//! - [`neutronics::point_kinetics`] — the 0-D point-kinetics ODE core.
//!
//! Everything else (XS data structures, diffusion/SP3/SN, multi-region coupling,
//! thermal-hydraulics, thermo-mechanics) is planned but not yet translated.

pub mod neutronics;
