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

//! Field types: the discretised quantities carried on the mesh.
//!
//! This module holds the data containers the FV operators read and write:
//!
//! - [`Field`](crate::fields::field::Field) — a flat `Vec<T>` with element-wise arithmetic; the raw storage
//!   with no mesh or dimension bookkeeping (mirrors `Foam::Field<Type>`).
//! - [`boundary`](crate::fields::boundary) — boundary conditions
//!   ([`BoundaryCondition`](crate::fields::boundary::bc::BoundaryCondition)) and
//!   per-patch boundary values
//!   ([`PatchField`](crate::fields::boundary::bc::PatchField)).
//! - [`VolField`](crate::fields::vol_field::VolField) (and the `Vol*Field` aliases) — cell-centred volume fields:
//!   one value per cell plus one `PatchField` per boundary patch.
//! - [`SurfaceField`](crate::fields::surface_field::SurfaceField) (and the `Surface*Field` aliases) — face fields: one
//!   value per internal face plus one `PatchField` per boundary patch.
//! - [`vol_field_algebra`](crate::fields::vol_field_algebra) — pure per-element tensor algebra (`tr`, `symm`,
//!   `dev`, …) lifted to whole volume fields.
//! - [`parallel`](crate::fields::parallel) — the same element-wise algebra and
//!   the field reductions, dispatched on
//!   [`ComputeBackend`](crate::compute::ComputeBackend) so a large mesh can use
//!   every CPU core. One entry point per operation; multi-threading is behind the
//!   crate's `parallel` feature and the serial path is the trusted reference.
//!
//! Physical units are not tracked at this layer; a field simply carries `f64`,
//! `Vector3`, `Tensor`, or `SymmTensor` values in whatever SI units the caller
//! assigns them.

pub mod boundary;
pub mod field;
pub mod parallel;
pub mod surface_field;
pub mod vol_field;
pub mod vol_field_algebra;

pub use boundary::*;
pub use field::Field;
// Only the two uniquely-named policy items are re-exported. The kernels
// themselves (`add`, `sum`, `scale`, …) are deliberately NOT glob-re-exported:
// their names are generic verbs that would collide on sight, so they stay
// path-qualified as `fields::parallel::add(backend, &a, &b)`.
pub use parallel::{field_parallel_crossover, should_parallelise};
pub use surface_field::*;
pub use vol_field::*;
