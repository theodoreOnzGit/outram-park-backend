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

//! Tensor-algebra primitives (Layer 1): the `Scalar`/`Label` aliases and
//! magnitude constants (`SMALL`, `GREAT`, …), the 3-vector `Vector3`, and the
//! `SphericalTensor`/`SymmTensor`/`Tensor` rank-2 types with their algebra.
//! Mirrors OpenFOAM's `src/OpenFOAM/primitives/` VectorSpace types.

pub mod scalar;
pub mod spherical_tensor;
pub mod vector;
pub mod symm_tensor;
pub mod tensor;

pub use scalar::{Label, Scalar, GREAT, ROOT_GREAT, ROOT_SMALL, ROOT_VSMALL, SMALL, VGREAT, VSMALL};
pub use spherical_tensor::SphericalTensor;
pub use symm_tensor::SymmTensor;
pub use tensor::Tensor;
pub use vector::Vector3;
