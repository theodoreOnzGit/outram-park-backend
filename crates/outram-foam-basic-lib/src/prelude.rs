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

/// Convenience re-export of the most commonly used types and functions.
///
/// ```rust
/// use outram_foam_basic_lib::prelude::*;
/// ```
///
/// # What's included
///
/// **Primitives** (Layer 1a)
/// - Scalar constants: `SMALL`, `VSMALL`, `GREAT`, `VGREAT`, `ROOT_SMALL`, `ROOT_VSMALL`
/// - Types: `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor`
///
/// **Polynomial algebra** (Layers 1c + 1d)
/// - Root types: `RootType`, `Roots`
/// - Equation solvers: `LinearEqn`, `QuadraticEqn`, `CubicEqn`
/// - Function evaluation: `Polynomial`
///
/// **Math special functions** (Layer 1g)
/// - `erf_inv`, `inc_gamma_ratio_p`, `inc_gamma_ratio_q`, `inc_gamma_p`, `inc_gamma_q`, `inv_inc_gamma`
///
/// **Specie-level thermophysics** (Layer 1h)
/// - Custom quantity: `Compressibility` (ψ = ∂ρ/∂p|T, s²/m²)
/// - EOS traits/types: `EquationOfState`, `PerfectGas`, `RhoConst`
/// - Thermo traits/types: `ThermoModel`, `HConstThermo`, `JanafThermo`
/// - Transport traits/types: `TransportModel`, `ConstTransport`, `SutherlandTransport`
// --- Primitive scalars ---
pub use crate::primitives::{GREAT, ROOT_GREAT, ROOT_SMALL, ROOT_VSMALL, SMALL, VGREAT, VSMALL};

// --- Primitive tensor types ---
pub use crate::primitives::{SphericalTensor, SymmTensor, Tensor, Vector3};

// --- Spectral decomposition (Layer 1a) ---
pub use crate::primitives::{
    eigen_values, eigen_values_checked, eigen_values_symm, eigen_vectors, eigen_vectors_symm,
    eigen_vectors_symm_with, eigen_vectors_with,
};

// --- Polynomial algebra ---
pub use crate::polynomial::{CubicEqn, LinearEqn, Polynomial, QuadraticEqn, RootType, Roots};

// --- Math special functions ---
pub use crate::math::{
    erf_inv, inc_gamma_p, inc_gamma_q, inc_gamma_ratio_p, inc_gamma_ratio_q, inv_inc_gamma,
};

// --- Dense matrices (Layer 1b) ---
pub use crate::matrix::{MatrixError, SquareMatrix};

// --- ODE solvers (Layer 1e) ---
pub use crate::ode::{Euler, OdeError, OdeSolverConfig, OdeSystem, Rkf45, Rosenbrock23};

// --- Interpolation (Layer 1f) ---
pub use crate::interpolation::{interpolate_spline_xy, interpolate_xy};

// --- Specie-level thermophysics (Layer 1h) ---
pub use crate::thermophysics::eos::*;
pub use crate::thermophysics::error::ThermoError;
pub use crate::thermophysics::quantities::Compressibility;
pub use crate::thermophysics::thermo::*;
pub use crate::thermophysics::transport::*;

// --- Fields (Layer 2) ---
pub use crate::fields::{
    BoundaryCondition, Field, PatchField, SurfaceField, SurfaceScalarField, SurfaceVectorField,
    VolField, VolScalarField, VolSymmTensorField, VolTensorField, VolVectorField,
};

// --- Mesh (Layer 2) ---
pub use crate::mesh::{
    AmiCoupling, AmiOverlap, AmiWeight, BoundaryPatch, CyclicCoupling, FvMesh, FvMeshBuilder,
    MeshError, PatchKind, RegionInterface,
};

// --- AMI (arbitrary mesh interface) overlap weighting ---
pub use crate::mesh::ami::overlap_weights_1d;

// --- Sparse linear system (Layer 2) ---
pub use crate::ldu_matrix::{
    FvMatrix, FvVectorMatrix, LduMatrix, SolverPerformance, SolverSettings,
};

// --- FV operators (Layer 3) ---
pub use crate::fv_operators::{adjust_phi, fvc, fvm};

// --- Optional equation sources, OpenFOAM `fvOptions` / `fvModels` (Layer 3) ---
pub use crate::fv_options::{
    CellSelection, EquationField, FvModel, FvModels, SemiImplicitSource, SolidificationMelting,
    SolidificationMeltingCoefficients, SourceContribution,
};

// --- Field-level tensor algebra (tr/symm/two_symm/dev/dev2 on vol fields) ---
pub use crate::fields::vol_field_algebra;

// --- Field-level fluid thermodynamics (Layer 4) ---
pub use crate::fluid_thermo::{ConstSolidThermo, FluidThermo, PsiThermo, RhoThermo, SolidThermo};

// --- LDU solvers ---
pub use crate::ldu_matrix::{conjugate_gradient, gamg, gauss_seidel};

// --- Asymmetric Krylov solvers + preconditioners ---
pub use crate::krylov::{
    bicgstab, gmres, Ilu0Preconditioner, JacobiPreconditioner, KrylovResult, KrylovSettings,
    Preconditioner,
};

// -- Interface ---
//
// Basically, OpenFOAM primitives aren't easy to use.
// But I want some functions that can help construct and use them
pub use crate::interface;

// TVD flux limiters (translated from OpenFOAM limitedSchemes)
pub use crate::limiters::FluxLimiter;
