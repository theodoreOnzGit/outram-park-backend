/// Convenience re-export of the most commonly used types and functions.
///
/// ```rust
/// use openfoam_basic_lib::prelude::*;
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
pub use crate::primitives::{
    SMALL, VSMALL, GREAT, VGREAT, ROOT_SMALL, ROOT_VSMALL, ROOT_GREAT,
};

// --- Primitive tensor types ---
pub use crate::primitives::{SphericalTensor, SymmTensor, Tensor, Vector3};

// --- Polynomial algebra ---
pub use crate::polynomial::{CubicEqn, LinearEqn, Polynomial, QuadraticEqn, RootType, Roots};

// --- Math special functions ---
pub use crate::math::{
    erf_inv, inc_gamma_p, inc_gamma_q, inc_gamma_ratio_p, inc_gamma_ratio_q, inv_inc_gamma,
};

// --- Dense matrices (Layer 1b) ---
pub use crate::matrix::SquareMatrix;

// --- ODE solvers (Layer 1e) ---
pub use crate::ode::{Euler, OdeError, OdeSystem, OdeSolverConfig, Rkf45, Rosenbrock23};

// --- Interpolation (Layer 1f) ---
pub use crate::interpolation::{interpolate_spline_xy, interpolate_xy};

// --- Specie-level thermophysics (Layer 1h) ---
pub use crate::thermophysics::quantities::Compressibility;
pub use crate::thermophysics::eos::*;
pub use crate::thermophysics::thermo::*;
pub use crate::thermophysics::transport::*;

// --- Fields (Layer 2) ---
pub use crate::fields::{
    Field,
    VolField, VolScalarField, VolVectorField, VolTensorField, VolSymmTensorField,
    SurfaceField, SurfaceScalarField, SurfaceVectorField,
    BoundaryCondition, PatchField,
};

// --- Mesh (Layer 2) ---
pub use crate::mesh::{FvMesh, FvMeshBuilder, BoundaryPatch, PatchKind};

// --- Sparse linear system (Layer 2) ---
pub use crate::ldu_matrix::{LduMatrix, FvMatrix, SolverSettings, SolverPerformance};

// --- FV operators (Layer 3) ---
pub use crate::fv_operators::{fvc, fvm};
