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
//! - [`case_runner`] — the layer above that: read an OpenFOAM case directory,
//!   build the solver's initial state from it, march the loop and write the
//!   fields back. `CaseRun::from_case(dir, SolverKind::PimpleFoam)` is the
//!   one-line way in; the CLI binaries in `outram-foam-cli` are thin wrappers
//!   over it, and it is what the Python bindings expose.
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

// `no_std` + `alloc` without the default `std` feature (gh:#1633, epic
// gh:#1585). Unit tests always build with `std`.
#![cfg_attr(not(any(feature = "std", test)), no_std)]
// Without `std` the gated case readers leave their imports and helpers
// unused; the `std` build still warns about every one.
#![cfg_attr(not(feature = "std"), allow(dead_code, unused_imports, unused_variables, unused_mut))]

extern crate alloc;
extern crate ndarray_nostd as ndarray;
extern crate thiserror_nostd as thiserror;

/// What the `std` prelude gives every module and a `no_std` build has to
/// import: the `alloc` types and macros, and `petir::real::Real` for the
/// float methods `core` lacks (`sqrt`, `floor`, ...). Glob-imported by each
/// module in `no_std` builds only, so a `std` build resolves every name, and
/// computes every number, exactly as before.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
pub(crate) mod nostd_prelude {
    pub(crate) use alloc::borrow::ToOwned;
    pub(crate) use alloc::boxed::Box;
    pub(crate) use alloc::format;
    pub(crate) use alloc::string::{String, ToString};
    pub(crate) use alloc::vec;
    pub(crate) use alloc::vec::Vec;
    pub(crate) use petir::real::Real;
}

/// Paths (gh:#1633). With `std` these are `std::path::{Path, PathBuf}`, so
/// every signature and error is exactly as before. Without it there is no
/// filesystem: `PathBuf` is a `String`, which is all the solvers use it for
/// (the case label in [`AppBuilderError::Case`]), and the readers that take a
/// `&Path` are `std` only.
pub mod path {
    #[cfg(feature = "std")]
    pub use std::path::{Path, PathBuf};
    /// A case label: no filesystem without `std`.
    #[cfg(not(feature = "std"))]
    pub type PathBuf = alloc::string::String;
}

/// Hash maps: std's with `std` (and in tests), so every result is as before;
/// `hashbrown` without it, the workspace rule (gh:#1633).
pub(crate) mod hash {
    #[cfg(any(feature = "std", test))]
    pub(crate) use std::collections::HashMap;
    #[cfg(not(any(feature = "std", test)))]
    pub(crate) use hashbrown::HashMap;
}

/// The crate's single error type, [`error::AppBuilderError`].
/// Driving a solver from an OpenFOAM case directory: read the case, build the
/// solver's initial state, march the time loop, write the fields back.
/// `std` only (gh:#1633): it reads OpenFOAM case directories.
#[cfg(feature = "std")]
pub mod case_runner;
/// Ready-made flow cases (boundary conditions + diagnostics) shared by V&V
/// tests and demos, e.g. [`cases::laminar_pipe`].
pub mod cases;

pub mod error;
/// GeN-Foam reactor-multiphysics port (neutronics + TH + thermo-mechanics).
/// See `docs/genfoam-port-plan.md` for the module map and translation order.
pub mod genfoam;
/// Pebble-bed HTGR closures (original work, not a GeN-Foam port): bed
/// effective conductivity (Breitbach default, ZBS comparison), KTA 3102.2 heat
/// transfer, KTA 3102.3 pressure drop, and the IAEA-TECDOC-1694 HTR-10 solid
/// conductivities. See [`htgr_closures`].
pub mod htgr_closures;
/// The HTR-10 R-Z solid conduction and radiation model (original work,
/// gh:#1149 part B): the TECDOC-1694 44-region THERMIX-style model built from
/// the cited input sheet on the solid conduction field, with the
/// [`htgr_closures`] properties. See [`htr10_rz`].
pub mod htr10_rz;
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
