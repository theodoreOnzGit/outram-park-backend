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
//! OpenFOAM® primitive/finite-volume algorithms — it is not the official
//! OpenFOAM® software and is not affiliated with, endorsed by, or
//! sanctioned by OpenCFD Ltd. or the ESI Group.** OpenFOAM® is a registered
//! trademark of OpenCFD Limited. See `TRADEMARKS.md` (this crate's
//! directory, mirrored from the workspace root) for the full attribution
//! and non-affiliation notice.

/// Layer 2 — field containers (`VolField`/`SurfaceField`), boundary
/// conditions, and field-level tensor algebra.
pub mod fields;
/// Layer 4 — field-level fluid and solid thermodynamics (`FluidThermo`,
/// `SolidThermo`, `PsiThermo`, `RhoThermo`).
pub mod fluid_thermo;
/// Layer 3 — finite-volume discretisation operators (`fvc` explicit, `fvm`
/// implicit, and `adjust_phi` continuity correction).
pub mod fv_operators;
/// Layer 1f — one-dimensional data interpolation (linear and spline).
pub mod interpolation;
/// Layer 2 — asymmetric Krylov iterative solvers (BiCGStab, restarted GMRES)
/// and preconditioners (Jacobi, ILU(0)) for the sparse `LduMatrix`.
pub mod krylov;
/// Layer 2 — sparse LDU (lower/diagonal/upper) matrices, the assembled
/// `FvMatrix`, and iterative linear solvers (CG, Gauss–Seidel, GAMG).
pub mod ldu_matrix;
/// Layer 1g — mathematical special functions (inverse error function,
/// incomplete gamma functions and their inverse).
pub mod math;
/// Layer 1b — dense `SquareMatrix` with direct (LU) solve.
pub mod matrix;
/// Layer 2 — the finite-volume mesh: cells, faces, boundary patches, and
/// geometric metrics.
pub mod mesh;
/// OpenFOAM ASCII case I/O — `FoamFile` dictionaries, `polyMesh` read/write,
/// time-directory field read/write, and whole-case reading.
pub mod io;
/// Layer 1e — ordinary-differential-equation solvers (Euler, RKF45,
/// Rosenbrock23).
pub mod ode;
/// Layers 1c/1d — polynomial evaluation and closed-form equation solvers
/// (linear, quadratic, cubic).
pub mod polynomial;
/// Convenience re-exports of the most commonly used types and functions.
pub mod prelude;
/// Layer 1a — dimensionless scalar constants and the tensor-algebra
/// primitives (`Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`).
pub mod primitives;
/// Layer 1h — specie-level thermophysics: equations of state, thermo, and
/// transport models.
pub mod thermophysics;

/// this part is extension in Rust
/// Now under here, I want to expose the openfoam primitives to something
/// that can be human readable
///
/// Also useful add-ons for the underlying libraries are put here,
/// eg. generating one dimensional meshes for system code type simulations
/// in TAMPINES
pub mod interface;
