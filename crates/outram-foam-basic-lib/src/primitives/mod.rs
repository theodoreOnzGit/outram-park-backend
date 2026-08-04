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

//! Layer 1a — the primitive numeric types OpenFOAM builds everything on.
//!
//! This module holds the dimensionless scalar type and small-/large-number
//! constants (`scalar`) together with the fixed-size 3-D tensor-algebra
//! primitives: a 3-vector (`Vector3`), a full 3×3 tensor (`Tensor`), a
//! symmetric 3×3 tensor (`SymmTensor`), and an isotropic diagonal tensor
//! (`SphericalTensor`). All components are plain `f64` (dimensionless SI);
//! `uom`-dimensioned quantities are layered on top elsewhere in the crate.
//! Each type mirrors its `Foam::` counterpart, including component storage
//! order and the OpenFOAM operator conventions (`&`, `&&`, `^`, `*`).

/// The scalar floating-point type and the small/large numeric constants.
/// Spectral decomposition of 3x3 tensors -- eigenvalues, eigenvectors, and the
/// basis every isotropic tensor function (logarithm, exponential, square root)
/// is built on.
pub mod eigen;

pub mod scalar;
/// Isotropic diagonal tensor `ii * I` (`SphericalTensor`).
pub mod spherical_tensor;
/// Symmetric 3×3 tensor (`SymmTensor`).
pub mod symm_tensor;
/// Full (non-symmetric) 3×3 tensor (`Tensor`).
pub mod tensor;
/// 3-component vector (`Vector3`).
pub mod vector;

pub use eigen::{
    eigen_values, eigen_values_checked, eigen_values_symm, eigen_vectors, eigen_vectors_symm,
    eigen_vectors_symm_with, eigen_vectors_with,
};
pub use scalar::{
    Label, Scalar, GREAT, ROOT_GREAT, ROOT_SMALL, ROOT_VSMALL, SMALL, VGREAT, VSMALL,
};
pub use spherical_tensor::SphericalTensor;
pub use symm_tensor::SymmTensor;
pub use tensor::Tensor;
pub use vector::Vector3;
