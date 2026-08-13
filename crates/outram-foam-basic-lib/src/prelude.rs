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
pub use crate::ode::{
    DynSystemIntegrator, Euler, NoTypedSystem, OdeError, OdeIntegrator, OdeSolver, OdeSolverConfig,
    OdeSystem, Rkf45, Rosenbrock23, SharedOdeSystem, TypedStateIntegrator,
};

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
pub use crate::ldu_matrix::{FvMatrix, FvVectorMatrix, LduMatrix, SolverPerformance, SolverSettings};

// --- FV operators (Layer 3) ---
pub use crate::fv_operators::{adjust_phi, fvc, fvm};

// --- Non-orthogonal (mesh-quality) correction for the Laplacian ---
pub use crate::fv_operators::fvc::grad_least_squares;
pub use crate::fv_operators::fvm::{
    laplacian_corrected, max_non_orthogonality_deg, non_ortho_geometry,
    solve_laplacian_non_orthogonal, NonOrthoGeometry, NonOrthoScheme,
};

// --- Optional equation sources, OpenFOAM `fvOptions` / `fvModels` (Layer 3) ---
pub use crate::fv_options::{
    CellSelection, EquationField, FvModel, FvModels, MomentumEquationForm, SemiImplicitSource,
    SolidificationMelting, SolidificationMeltingCoefficients, SolidificationPorosity,
    SourceContribution, TemperatureTable, VofSolidificationMelting,
};

// --- Field-level tensor algebra (tr/symm/two_symm/dev/dev2 on vol fields) ---
pub use crate::fields::vol_field_algebra;

// --- Field-level fluid thermodynamics (Layer 4) ---
pub use crate::fluid_thermo::{ConstSolidThermo, FluidThermo, PsiThermo, RhoThermo, SolidThermo};

// --- LDU solvers ---
pub use crate::ldu_matrix::{conjugate_gradient, gamg, gauss_seidel};

// --- FvMatrix/FvVectorMatrix bridge onto the asymmetric Krylov solvers ---
pub use crate::ldu_matrix::{krylov_solve, KrylovMethod, KrylovOptions, PreconditionerKind};

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

// --- Hybrid execution backend (dispatch only, no kernels) ---
//
// `ComputeBackend::Serial` is always available and is the oracle every other
// backend is verified against; `CpuMulti` needs the `parallel` feature and
// `Gpu` the `gpu` feature plus an actual adapter. `select_backend` is the one
// named policy function that decides between them.
pub use crate::compute::{
    gpu_adapter_present, select_backend, ComputeBackend, ThreadCount, CPU_MULTI_MIN_WORK_ITEMS,
    GPU_MIN_WORK_ITEMS,
};

// --- Parallel field algebra: policy items ONLY ---
//
// The kernels themselves (`add`, `sub`, `sum`, `min`, `max`, `scale`, `dot`,
// ...) are deliberately NOT re-exported here: those names would collide on
// sight with `std` and with `ldu_matrix::parallel`. Call them path-qualified,
// e.g. `fields::parallel::add(backend, &a, &b)`.
pub use crate::fields::parallel::{
    field_parallel_crossover, should_parallelise, FIELD_PARALLEL_CROSSOVER, REDUCTION_CHUNK,
};

// --- Hybrid LDU sparse matrix-vector product + Krylov vector operations ---
//
// `HybridLdu` carries the cell-gather topology that makes the parallel SpMV
// bit-for-bit identical to the serial oracle. The free vecops are exported
// under `ldu_` prefixes here because bare `dot`/`axpy` would read ambiguously
// beside `fields::parallel`'s reductions; call them path-qualified if the
// prefix is unwanted.
pub use crate::ldu_matrix::parallel::{
    axpy as ldu_axpy, dot as ldu_dot, norm_l1 as ldu_norm_l1, norm_l2 as ldu_norm_l2,
    spmv_backend_for, vecop_backend_for, HybridLdu, LduTopology, CELL_BLOCK, REDUCTION_BLOCK,
    SPMV_MIN_CELLS, VECOP_MIN_ELEMENTS,
};

// --- Batched root finding on the hybrid backend (Layer 1) ---
pub use crate::math::parallel::{
    cubic_roots_batch, linear_roots_batch, poly_roots_backend_for, quadratic_roots_batch,
    root_batch_backend_for, solve_bracketed_batch, solve_newton_batch, RootBatch,
    RootBatchFailure, RootMethod, RootProblem, RootSettings, RootSolution, RootStatus,
};

// --- Batched 1-D golden-section extremum search (Layer 1) ---
//
// Generalised from `tampines-steam-tables`' choked-flow `golden_section_max_g`
// rather than written afresh. `Sense` exists because the production caller
// MAXIMISES, and negating internally would flip the sign of returned values
// under the caller's feet.
pub use crate::math::minimise::{
    golden_section_batch, minimise_backend_for, MinBatch, MinBatchFailure, MinProblem,
    MinSettings, MinSolution, MinStatus, Sense, GOLDEN_RATIO, MINIMISE_BATCH_MIN_PROBLEMS,
    SQRT_EPSILON,
};
