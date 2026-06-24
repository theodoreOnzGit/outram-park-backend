# openfoam-basic-lib

Pure-Rust translation of the OpenFOAM primitive and finite-volume library layer —
tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics,
fields, mesh, FV operators, and fluid/solid thermodynamics needed to build
compressible and conjugate-heat-transfer CFD solvers.

## Quick start

```toml
[dependencies]
openfoam-basic-lib = "0.1.4"
```

```rust
use openfoam_basic_lib::prelude::*;

// Tensor algebra
let u = Vector3::new(1.0, 0.0, 0.0);
let v = Vector3::new(0.0, 1.0, 0.0);
let cross = u.cross(v);              // z-axis unit vector

// Polynomial root finding
let cubic = CubicEqn::new(1.0, -6.0, 11.0, -6.0); // (x-1)(x-2)(x-3)
let roots = cubic.roots();

// Dense LU solve (no external BLAS required)
let mut m = SquareMatrix::new(3);
m.set(0, 0, 2.0); m.set(0, 1, 1.0); m.set(0, 2, 0.0);
m.set(1, 0, 1.0); m.set(1, 1, 3.0); m.set(1, 2, 1.0);
m.set(2, 0, 0.0); m.set(2, 1, 1.0); m.set(2, 2, 4.0);
let x = m.solve(&[7.0, 10.0, 15.0]);  // returns Vec<f64>

// FV operators
use openfoam_basic_lib::fv_operators::{fvm, fvc};
// fvm::ddt, fvm::laplacian, fvm::div, fvm::laplacian_vec, fvm::div_vec,
// fvm::ddt_coeff, fvm::ddt_coeff_vec
// fvc::grad, fvc::div, fvc::flux, fvc::reconstruct, fvc::buoyancy_flux, ...
```

## What's implemented

### Layers 1a–1h — Primitives and thermophysics

| Module | Rust type / fn | Notes |
|---|---|---|
| `primitives` | `Vector3`, `Tensor`, `SymmTensor`, `SphericalTensor` | Full OpenFOAM tensor algebra; `SMALL`, `VSMALL`, `GREAT`, etc. |
| `polynomial` | `LinearEqn`, `QuadraticEqn`, `CubicEqn` | FMA-accurate discriminants; all root branches |
| `polynomial` | `Polynomial<const N>` | Horner eval, derivative, integral, integral_minus1 (log term) |
| `polynomial` | `Roots<const N>`, `RootType` | 3-bit-per-root type encoding |
| `math` | `erf_inv`, `inc_gamma_ratio_p/q`, `inc_gamma_p/q`, `inv_inc_gamma` | DiDonato–Morris (1986) |
| `matrix` | `SquareMatrix` | Row-major n×n; LU with scaled partial pivoting; `lu_decompose`, `lu_back_substitute`, `solve` |
| `ode` | `Euler`, `Rkf45`, `Rosenbrock23` | Adaptive explicit (RKF45) and stiff (W-method) solvers; `OdeSystem` trait |
| `interpolation` | `interpolate_xy`, `interpolate_spline_xy` | Linear and Catmull-Rom cubic 1-D |
| `thermophysics::eos` | `PerfectGas`, `RhoConst`, `IcoPolynomial<N>`, `PengRobinsonGas` | `EquationOfState` trait; ρ, ψ, Z, departure functions |
| `thermophysics::thermo` | `HConstThermo`, `JanafThermo`, `HPolynomialThermo<N>`, `HTabulatedThermo` | `ThermoModel` trait; Cp, Ha, Hs, S; Newton T(H) iteration |
| `thermophysics::transport` | `ConstTransport`, `SutherlandTransport`, `PolynomialTransport<N>`, `TabulatedTransport` | `TransportModel` trait; μ, κ |

### Layer 2 — Fields and mesh

| Module | Rust type | Notes |
|---|---|---|
| `fields` | `Field<T>`, `VolField<T>`, `SurfaceField<T>` | Generic field containers; `BoundaryCondition<T>`, `PatchField<T>` |
| `fields` | `VolScalarField`, `VolVectorField`, `VolTensorField`, `VolSymmTensorField` | Typed aliases |
| `fields` | `SurfaceScalarField`, `SurfaceVectorField` | Face-centred typed aliases |
| `mesh` | `FvMesh`, `FvMeshBuilder`, `BoundaryPatch`, `PatchKind` | Unstructured polyhedral mesh |
| `mesh` | `RegionInterface` | Matching and non-matching multi-region face coupling for CHT |
| `ldu_matrix` | `LduMatrix`, `FvMatrix`, `FvVectorMatrix` | Sparse LDU system; scalar and vector implicit equation assembly |
| `ldu_matrix` | `gauss_seidel`, `conjugate_gradient` | Iterative LDU solvers (no external BLAS) |
| `ldu_matrix` | `SolverSettings`, `SolverPerformance` | Tolerance / iteration control and convergence reporting |

### Layer 3 — Finite-volume operators

| Function | Description |
|---|---|
| `fvm::ddt(phi, phi_old, dt)` | Implicit Euler ∂φ/∂t → `FvMatrix` |
| `fvm::ddt_coeff(coeff, phi, phi_old, dt)` | Density/rho_cp-weighted implicit ddt: ∂(coeff·φ)/∂t → `FvMatrix` |
| `fvm::ddt_vec(U, U_old, dt, mesh)` | Implicit Euler ∂U/∂t → `FvVectorMatrix` |
| `fvm::ddt_coeff_vec(coeff, U, U_old, dt, mesh)` | Density-weighted implicit ddt: ∂(ρU)/∂t → `FvVectorMatrix` |
| `fvm::laplacian(gamma, phi)` | Diffusion −∇·(γ∇φ) → `FvMatrix` |
| `fvm::laplacian_vec(gamma, U)` | Diffusion −∇·(γ∇U) → `FvVectorMatrix` |
| `fvm::div(phi, psi)` | Upwind convection ∇·(φψ) → `FvMatrix` |
| `fvm::div_vec(phi, U)` | Upwind convection ∇·(φU) → `FvVectorMatrix` |
| `fvc::grad(phi)` | Explicit cell-centred gradient → `VolVectorField` |
| `fvc::div(phi, psi)` | Explicit scalar divergence → `VolScalarField` |
| `fvc::div_flux(phi)` | Divergence of face flux → `VolScalarField` |
| `fvc::interpolate(phi)` | Linear face interpolation → `SurfaceScalarField` |
| `fvc::sn_grad(phi)` | Surface-normal gradient → `SurfaceScalarField` |
| `fvc::flux(U)` | Face flux φ = U·Sf → `SurfaceScalarField` |
| `fvc::reconstruct(phi)` | Least-squares VolVectorField from face flux → `VolVectorField` |
| `fvc::ddt_corr(U_old, phi_old, dt)` | PISO flux consistency correction → `SurfaceScalarField` |
| `fvc::buoyancy_flux(rho, g)` | ρ_f·(g·Sf) per face → `SurfaceScalarField` |
| `adjust_phi(phi, U)` | Global mass-balance correction |

### Layer 4 — Field-level fluid thermodynamics

| Type | Description |
|---|---|
| `FluidThermo` | Trait: `rho`, `mu`, `kappa`, `alpha_h`, `T`, `he`, `update` |
| `PsiThermo<M>` | Compressible ψ-based thermo for sonicFoam / rhoPimpleFoam: ρ = ψ·p |
| `RhoThermo<M>` | Density-based thermo: ρ from EOS |
| `SolidThermo` | Trait for solid region CHT: `rho_cp`, `kappa`, `T` |
| `ConstSolidThermo` | Constant-property solid thermo |

## SquareMatrix vs LAPACK benchmark

`SquareMatrix::solve` (pure-Rust LU, no external BLAS) compared to
`ndarray-linalg` / OpenBLAS `Array2::solve` (LAPACK DGESV) — release mode,
Linux x86-64, 2026-06-24:

| n | SquareMatrix (ns) | OpenBLAS (ns) | ratio |
|---|---|---|---|
| 5 | 193 | 371 | **0.52 — SquareMatrix 1.9× faster** |
| 10 | 352 | 512 | **0.69 — SquareMatrix 1.5× faster** |
| 20 | 1 446 | 1 614 | **0.90 — roughly equal** |
| 50 | 17 018 | 7 891 | 2.16 — OpenBLAS faster |
| 100 | 135 705 | 27 845 | 4.87 — OpenBLAS faster |
| 200 | 1 112 109 | 357 281 | 3.11 — OpenBLAS faster |

`SquareMatrix` is faster for n ≤ 10 because OpenBLAS DGESV has ~300–400 ns
of per-call FFI overhead that dominates at small sizes. The crossover is around
n ≈ 20–50. For typical finite-volume networks (10–50 unknowns per implicit
system), `SquareMatrix` eliminates the system BLAS dependency with no
performance penalty. Reproduce with:

```bash
cargo test -p openfoam-basic-lib --test matrix_bench --release -- --nocapture
```

## Prelude

```rust
use openfoam_basic_lib::prelude::*;
```

Includes all tensor types, polynomial solvers, math functions, `SquareMatrix`,
ODE solvers, interpolation, all thermophysics types, all field and mesh types,
all LDU matrix types, FV operator modules (`fvc`, `fvm`), `adjust_phi`, and
all fluid/solid thermo types.

## Running tests

```bash
# Library unit tests (no external BLAS required)
cargo test -p openfoam-basic-lib --lib --tests

# Matrix benchmark (release mode for meaningful numbers)
cargo test -p openfoam-basic-lib --test matrix_bench --release -- --nocapture
```

## Layer roadmap

```
✅ Layer 1a — Tensor algebra      (Vector3, Tensor, SymmTensor, SphericalTensor)
✅ Layer 1b — Dense matrices      (SquareMatrix: LU with scaled partial pivoting)
✅ Layer 1c — Polynomial eqns     (LinearEqn, QuadraticEqn, CubicEqn, Roots<N>)
✅ Layer 1d — Polynomial eval     (Polynomial<N>: Horner, derivative, integral)
✅ Layer 1e — ODE solvers         (Euler, Rkf45, Rosenbrock23; OdeSystem trait)
✅ Layer 1f — Interpolation       (interpolate_xy, interpolate_spline_xy)
✅ Layer 1g — Math functions      (erf_inv, inc_gamma_*, inv_inc_gamma)
✅ Layer 1h — Thermophysics       (EOS, Thermo, Transport traits + 4 impls each)
✅ Layer 2  — Fields + Mesh       (VolField, SurfaceField, FvMesh, LduMatrix,
                                   FvMatrix, FvVectorMatrix, RegionInterface,
                                   Gauss-Seidel + CG solvers)
✅ Layer 3  — FV operators        (fvm: ddt, ddt_coeff, ddt_vec, ddt_coeff_vec,
                                        laplacian, laplacian_vec, div, div_vec
                                   fvc: grad, div, interpolate, sn_grad, flux,
                                        reconstruct, ddt_corr, buoyancy_flux
                                   adjust_phi)
✅ Layer 4  — Field thermodynamics (FluidThermo, PsiThermo, RhoThermo,
                                    SolidThermo, ConstSolidThermo)
⬜ Layer 5  — Solver logic         (icoFoam PISO loop → openfoam-icof;
                                    chtMultiRegionFoam → openfoam-cht;
                                    rhoPimpleFoam → openfoam-rho)
```

## License

GPL-3.0-only (matching the upstream OpenFOAM sources).
