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
//! OpenFOAM® solver-application algorithms — it is not the official
//! OpenFOAM® software and is not affiliated with, endorsed by, or
//! sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
//! trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
//! directory, mirrored from the workspace root) for the full attribution
//! and non-affiliation notice.
//!
//! # `outram-foam-appbuilder-lib` — Layer 5: solver applications and case I/O
//!
//! This crate is the **application layer** of the OUTRAM PARK OpenFOAM-in-Rust
//! stack. It sits on top of `outram-foam-basic-lib` (Layers 1–4: tensors,
//! fields, mesh, `fvm`/`fvc` operators, linear solvers) and
//! `outram-foam-turbulence-lib` (turbulence closures), and supplies the parts
//! those crates deliberately do not: the **time-advancement loops**, the
//! **case-file structures**, and the **multiphysics coupling drivers**.
//!
//! ```text
//! outram-foam-basic-lib        Layers 1–4  primitives, fields, mesh, FV operators
//! outram-foam-turbulence-lib   Layer 4     RAS/LES closures
//!            │
//!            ▼
//! outram-foam-appbuilder-lib   Layer 5     ← THIS CRATE
//! ```
//!
//! ## Where to start
//!
//! - [`solvers`] — one submodule per ported OpenFOAM application. Each owns its
//!   PISO/PIMPLE (or explicit) time loop. Construct one with `new(mesh, control,
//!   schemes, solution)`, set the field state, then call `step()` or `run()`.
//! - [`io`] — readers for `constant/polyMesh` and `0/<field>` files, plus typed
//!   `controlDict` / `fvSchemes` / `fvSolution` structs.
//! - [`turbulence`] — pick a closure for a solver run.
//! - [`prelude`] — one `use` that pulls in the commonly needed public items.
//! - `tutorials/` — runnable end-to-end cases; the intended entry point for a
//!   reader new to the crate.
//!
//! ## Maturity — read before depending on this
//!
//! This is an early (0.1.0), in-progress crate and its surface is uneven:
//! some paths are validated against published benchmarks, others are
//! unexercised, and several are `todo!()`. The **`README.md` "Limitations"
//! section is the authoritative per-module status** and is deliberately
//! detailed. Two consequences bite immediately:
//!
//! - **No OpenFOAM dictionary parsing.** [`io::control_dict::ControlDict::read`],
//!   [`io::fv_schemes::FvSchemes::read`] and
//!   [`io::fv_solution::FvSolution::read`] are `todo!()`. Configure a case by
//!   constructing the structs in Rust (`Default::default()` plus field
//!   assignment), not by reading `system/…` from disk.
//! - **No field output.** Every writer in [`io::output`] is `todo!()`, so a
//!   solver run leaves its results in memory only — read them off the solver's
//!   public field members.
//!
//! Per the workspace `RESPONSIBLE_USE.md`, nothing here is for reactor
//! operation, control, licensing, or any safety-critical or operational use.

/// The crate's single error type, [`error::AppBuilderError`].
pub mod error;
/// GeN-Foam reactor-multiphysics port (neutronics + TH + thermo-mechanics).
/// See `docs/genfoam-port-plan.md` for the module map and translation order.
pub mod genfoam;
/// OpenFOAM case input/output — polyMesh and field readers, typed
/// `controlDict`/`fvSchemes`/`fvSolution`, and (unimplemented) field writers.
pub mod io;
/// Re-exports of the crate's commonly used public items, for
/// `use outram_foam_appbuilder_lib::prelude::*;`.
pub mod prelude;
/// The ported OpenFOAM solver applications and their time-advancement loops.
pub mod solvers;
/// Turbulence-closure selection for the solver loops — the Layer-5 adapter over
/// `outram-foam-turbulence-lib`. See [`turbulence::TurbulenceClosure`].
pub mod turbulence;
