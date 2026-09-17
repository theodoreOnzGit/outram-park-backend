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

//! OpenFOAM-style tensor-algebra primitives: generic, dimensionless
//! numeric containers (`Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`,
//! plus scalar type aliases/tolerance constants). The physical unit of a
//! given instance is whatever `uom` quantity the caller pairs it with
//! elsewhere (e.g. a velocity field stores `Vector3`-shaped m/s components);
//! this layer itself only implements the algebra (dot/cross/outer products,
//! trace, deviatoric/symmetric decomposition, inversion, eigen-invariants).

/// Scalar type aliases (`Scalar`, `Label`) and OpenFOAM-style
/// small/great tolerance constants.
pub mod scalar;
/// Isotropic (single-component) tensor — see [`SphericalTensor`].
pub mod spherical_tensor;
/// 3x3 symmetric tensor (6 independent components) — see [`SymmTensor`].
pub mod symm_tensor;
/// General 3x3 tensor — see [`Tensor`].
pub mod tensor;
/// 3-component vector — see [`Vector3`].
pub mod vector;

pub use scalar::{Label, Scalar, GREAT, ROOT_GREAT, ROOT_SMALL, ROOT_VSMALL, SMALL, VGREAT, VSMALL};
pub use spherical_tensor::SphericalTensor;
pub use symm_tensor::SymmTensor;
pub use tensor::Tensor;
pub use vector::Vector3;
