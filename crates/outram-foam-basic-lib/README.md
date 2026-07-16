# outram-foam-basic-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.


> **This is OUTRAM PARK's independent Rust translation of selected OpenFOAM®
> algorithms.** It is not the official OpenFOAM® software and is not
> affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. or the ESI
> Group. OpenFOAM® is a registered trademark of OpenCFD Limited — see
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`OpenFOAM/OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev),
> `master` branch — no commit is pinned (translation was done by reading the
> C++ source directly, not from an ongoing codegen-from-clone pipeline); see
> `upstream_source/README.md` for the full provenance record.

Pure-Rust translation of the OpenFOAM primitive and finite-volume library layer —
tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics,
fields, mesh, FV operators, and fluid/solid thermodynamics needed to build
compressible and conjugate-heat-transfer CFD solvers.

## Quick start

```toml
[dependencies]
outram-foam-basic-lib = "0.1.0"
```

```rust
use outram_foam_basic_lib::prelude::*;

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
let x = m.solve(&[7.0, 10.0, 15.0]);  // returns Result<Vec<f64>, MatrixError>

// FV operators
use outram_foam_basic_lib::fv_operators::{fvm, fvc};
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
| `ldu_matrix` | `gauss_seidel`, `conjugate_gradient` | Iterative LDU solvers (no external BLAS). CG is **DIC-preconditioned** (`Foam::DICPreconditioner`) and accepts an optional initial guess (`x0`) for warm starts |
| `ldu_matrix` | `gamg` | Algebraic multigrid (`Foam::GAMGSolver` + `algebraicPairGAMGAgglomeration`) for symmetric systems; near mesh-independent V-cycle count |
| `ldu_matrix` | `FvMatrix::solve_cg`, `FvMatrix::solve_cg_with_guess` | Cold- and warm-started PCG for the symmetric pressure system |
| `ldu_matrix` | `FvMatrix::solve_gamg`, `FvMatrix::solve_gamg_with_guess` | Cold- and warm-started GAMG for the symmetric pressure system |
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

## Limitations

Read this before depending on the crate. Every item below is grounded in the
current source; nothing here is aspirational. See also the
[unverified-until-validated banner](#outram-foam-basic-lib) at the top.

### Scope boundaries

- **Layers 1–4 only — no solver loops.** This crate provides the mathematical
  building blocks (tensor algebra, primitives, FV operators, thermophysics
  kernels, field thermodynamics). The **Layer 5** logic — PISO/PIMPLE loops,
  multi-region coupling drivers, turbulence-model registries — lives in separate
  downstream crates (`openfoam-icof`, `openfoam-cht`, `openfoam-rho`), which are
  **not yet published**. On their own the operators here do not run a
  time-marching CFD simulation.
- **No turbulence models.** `LaminarModel` / `kOmegaSST` and `divDevRhoReff` are
  planned for the CHT solver crate, not present here. Only laminar terms can be
  assembled.
- **No OpenFOAM case / `polyMesh` file I/O.** Meshes are constructed in code via
  `FvMeshBuilder` (you supply `owner`/`neighbour` connectivity and the geometric
  Vecs) or via `interface::one_dimensional_meshing::create_one_d_mesh` for 1-D
  system-code meshes. Reading an OpenFOAM `constant/polyMesh` directory is a
  downstream concern, not in this crate. (The DIC/warm-start discussion below
  refers to a `read_poly_mesh` that lives in the appbuilder crate.)
- **Serial only.** No MPI / domain decomposition; there are no processor
  boundary patches. The linear solvers and operators run single-threaded on one
  mesh.

### Finite-volume numerics

- **Implicit convection is first-order upwind only.** `fvm::div` / `fvm::div_vec`
  assemble a first-order upwind matrix (`src/fv_operators/fvm/div.rs`). There is
  **no** implicit higher-order/TVD convection (`linearUpwind`, `limitedLinear`,
  `vanLeer`, etc.). Second-order TVD *reconstruction* exists only explicitly, via
  `fvc::muscl` (`Upwind` / `Linear` / limiter variants), intended for
  density-based central-upwind fluxes — the Kurganov–Tadmor flux assembly itself
  is Layer-5 work and is not wired up here.
- **Laplacian is Gauss-orthogonal only — no non-orthogonal / skewness
  correction.** `fvm::laplacian` and `fvc::sn_grad` use the orthogonal
  face-area-over-centre-distance coefficient
  (`src/fv_operators/fvm/laplacian.rs`), ignoring the angle between the face-area
  vector and the owner→neighbour vector. Diffusion is therefore accurate on
  orthogonal meshes (hex / 1-D) but **under-resolved on non-orthogonal or skewed
  meshes**; there is no over-relaxed/minimum-correction non-orthogonal term.
- **Time integration is first-order.** Only implicit Euler `fvm::ddt` (plus
  `fvm::ddt_coeff`, the vector forms, and a `d2dt2`) is provided. No
  higher-order/backward/Crank–Nicolson time schemes.
- **Boundary conditions are a small closed set.** `BoundaryCondition`
  (`src/fields/boundary/bc.rs`) supports `FixedValue`, `FixedField`,
  `ZeroGradient`, `Symmetry`, `Empty`, and `Calculated` only. There is **no**
  `inletOutlet`, non-zero `fixedGradient`, `mixed`/Robin, wall-function,
  `cyclic`/periodic, or processor boundary condition.

### Linear solvers

- **Krylov / multigrid solvers assume symmetric positive-definite systems.**
  `conjugate_gradient` (DIC-preconditioned) and `gamg` target the symmetric
  pressure system. Asymmetric systems fall back to `gauss_seidel`; there is no
  BiCGStab/GMRES.
- **GAMG rebuilds its hierarchy on every solve** and is **slower than DIC-PCG
  below ~10^4–10^5 cells** (see the performance section below). It is
  mesh-independent in V-cycle count but not yet cached across time steps or usable
  as a CG preconditioner. PCG remains the default.
- **Dense matrices provide LU only.** `SquareMatrix` implements LU with scaled
  partial pivoting (`solve` returns `Result<Vec<f64>, MatrixError>`). The
  QR / Cholesky / SVD factorisations named in the design doc are **not
  implemented**.

### Thermophysics accuracy (known-failing / unverified)

- **Peng–Robinson EOS is inaccurate above the critical pressure.** Density errors
  of **7–26 % vs NIST** are observed for `Pr > 1` / near-critical states; the
  corresponding NIST cross-checks are `#[ignore]`-d pending a suspected
  Z-root-selection or α-function fix (`src/thermophysics/eos/peng_robinson.rs`,
  `docs/porting-roadmap.md`). Prefer `PerfectGas`, `RhoConst`, or `IcoPolynomial`
  where density accuracy matters.
- **JANAF `T(H)` inversion can fail to converge from a far initial guess.** The
  Newton solve stalls crossing the `Tcommon = 1000 K` coefficient discontinuity
  when started far away (e.g. `t0 = 100 K` targeting 3000 K); one such test is
  `#[ignore]`-d (`src/thermophysics/thermo/janaf.rs`). Seed the iteration with a
  reasonable temperature and handle `Err(NonConvergent)`.

### ODE and polynomial restrictions

- **The stiff `Rosenbrock23` solver needs a user Jacobian.** `OdeSystem::jacobian`
  has a default that **panics** (`unimplemented!`, `src/ode/mod.rs`); only the
  explicit `Euler` and `Rkf45` solvers work without one.
- **`Polynomial<N>::integral()` (degree-raising form) is not implemented.** The
  type-level definite integral that returns a `Polynomial<{N+1}>` needs nightly
  `generic_const_exprs`; use the scalar `integral(x1, x2) -> f64` /
  `integral_minus1` forms instead (`src/polynomial/polynomial.rs`).

### Validation status

- **Unverified until validated.** This crate's own test suite is unit /
  verification tests against analytic and reference values, not full-solver
  validation. The only end-to-end physics validation cited (lid-driven cavity,
  Ghia Re = 100) is run in the downstream `outram-foam-appbuilder-lib` crate, not
  here. Treat all outputs as untrusted draft until a specific V&V case covers
  your usage. Not for nuclear facility operation, reactor control,
  safety-critical, or licensing decisions.

### Platform / build

- **Pure Rust, no BLAS required by the library.** Runtime dependencies are only
  `thiserror`, `uom`, and `ndarray`, so the library builds headless (including
  Android and other no-system-BLAS targets). The LAPACK comparison benchmark
  (`tests/matrix_bench.rs`) *does* need system OpenBLAS (Linux) / Intel MKL
  (Windows/macOS); it is a **dev-dependency, target-gated off Android**, and is
  not part of the library build.

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
cargo test -p outram-foam-basic-lib --test matrix_bench --release -- --nocapture
```

## Pressure linear-solver performance (DIC preconditioner + warm start)

The pressure Poisson solve dominates the cost of an incompressible transient
run, so the PCG path here mirrors OpenFOAM's two key efficiency choices. The
reasoning, measured on the `pimple_foam_cavity` Ghia Re=100 case
(`outram-foam-appbuilder-lib`, 41×41 mesh, 6000 steps to t=12 s):

**The diagnosis.** Instrumenting every pressure solve showed 12 000 solves
(6000 steps × 2 PISO correctors), each running a *flat 272 PCG iterations* — the
same count on the first step and the last — for ≈ 3.26 million sparse
matrix-vector products. That was essentially the whole wall-clock time. Two
things were wrong:

1. **Cold start every solve.** `conjugate_gradient` began from `x = 0` each
   call, so even near steady state — where the pressure barely changes between
   steps — it paid full convergence from scratch.
2. **Jacobi preconditioner.** The preconditioner was diagonal only
   (`M⁻¹ = 1/diag`), whose PCG iteration count scales with the mesh
   (∝ √κ ≈ O(Nₓ)).

**The fix (both taken straight from the OpenFOAM source).**

- **Warm start.** `conjugate_gradient` now accepts an initial guess `x0`, and
  `FvMatrix::solve_cg_with_guess` seeds the solve with the previous time step's
  field. This is exactly what `Foam::PCG::scalarSolve` does — it computes the
  initial residual as `source − A·psi` from the *incoming* `psi`, i.e. the field
  passed in is the initial guess. A transient solver near steady state then
  converges in a few iterations (the solver returns immediately, 0 iterations,
  if the guess already meets the tolerance).
- **DIC preconditioner.** The Jacobi preconditioner was replaced with DIC
  (Diagonal-based Incomplete Cholesky), a faithful port of
  `Foam::DICPreconditioner`: a one-time reciprocal-diagonal factorisation
  (`calcReciprocalD`) plus a forward/backward face sweep (`precondition`).
  It requires faces in upper-triangular order (`owner[f] < neighbour[f]`), which
  is how `read_poly_mesh` loads OpenFOAM `polyMesh` files.

**Result.** Per-solve iterations dropped from a flat 272 to ~73 (cold, DIC
alone) and ~40 near steady state (DIC + warm start) — **6.6× fewer total CG
iterations** (3.26M → 0.50M) and the cavity test roughly halved in wall time,
with the solution field bit-for-bit unchanged (Ghia RMS 0.0113).

**GAMG (algebraic multigrid) — implemented; mesh-independent but not a win at
small sizes.** [`gamg`](src/ldu_matrix/solvers/gamg.rs) is a serial port of
OpenFOAM's `GAMGSolver` with `algebraicPairGAMGAgglomeration`: pairwise
agglomeration on the matrix coefficients (`|upper|`), Galerkin coarse operators
(`Pᵀ A P`), a recursive correction-scheme V-cycle with Gauss-Seidel pre/post
smoothing and an optimal line-search correction scale, and a dense-LU coarsest
solve. Its V-cycle count stays flat as the mesh is refined (verified in
`gamg::tests::gamg_cycle_count_is_mesh_independent`) — the property PCG lacks.

On the 1681-cell cavity, however, GAMG reproduces the PCG solution **bit-for-bit
(field diff 2.6e-9)** but runs *slower* (42.8 s vs 12.4 s): at this scale
DIC-PCG already converges in ~40 cheap iterations per corrector, below the
crossover (~10⁴–10⁵ cells) where multigrid's mesh-independence pays off, and the
free `gamg` function rebuilds the hierarchy on every solve. So PCG stays the
default; GAMG is the right tool once larger meshes are in play. Two follow-ups
would make GAMG competitive at scale: **cache the agglomeration** across time
steps (re-restrict coefficients only), and use **GAMG as a CG preconditioner**
for Krylov robustness.

## Prelude

```rust
use outram_foam_basic_lib::prelude::*;
```

Includes all tensor types, polynomial solvers, math functions, `SquareMatrix`,
ODE solvers, interpolation, all thermophysics types, all field and mesh types,
all LDU matrix types, FV operator modules (`fvc`, `fvm`), `adjust_phi`, and
all fluid/solid thermo types.

## Running tests

```bash
# Library unit tests (no external BLAS required)
cargo test -p outram-foam-basic-lib --lib --tests

# Matrix benchmark (release mode for meaningful numbers)
cargo test -p outram-foam-basic-lib --test matrix_bench --release -- --nocapture
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

## Changelog

### Unreleased

- **Added GAMG (algebraic multigrid) solver** (`ldu_matrix::gamg`,
  `FvMatrix::solve_gamg{,_with_guess}`). Serial port of `Foam::GAMGSolver` +
  `algebraicPairGAMGAgglomeration`: pairwise agglomeration, Galerkin coarse
  operators, recursive V-cycle (GS pre/post-smoothing, line-search correction
  scale, dense-LU coarsest). V-cycle count is mesh-independent. On the
  1681-cell cavity it reproduces the PCG field exactly but is slower at that
  scale (see "Pressure linear-solver performance"); PCG remains the default.

- **Pressure PCG: DIC preconditioner + warm start (≈6.6× fewer iterations).**
  `conjugate_gradient` now uses a DIC preconditioner (`Foam::DICPreconditioner`
  port) instead of Jacobi and takes an optional initial guess `x0`;
  `FvMatrix::solve_cg_with_guess` warm-starts from the previous field. On the
  `pimple_foam_cavity` Ghia Re=100 case (41×41) this cut per-solve PCG
  iterations from a flat 272 to ~40 and total CG iterations 3.26M → 0.50M, with
  the solution unchanged. See "Pressure linear-solver performance" above.
  *Note:* `conjugate_gradient` gained a parameter (`x0`) — update callers from
  `conjugate_gradient(&ldu, &b, &settings)` to
  `conjugate_gradient(&ldu, &b, None, &settings)`.

- **Fixed an exponential-memory bug in field arithmetic (the "24 GB cavity").**
  `VolField`/`SurfaceField`'s `Add`/`Sub`/`Neg` rebuilt the field's `name`
  string on every call (`self.name = format!("({} + {})", self.name, rhs.name)`).
  In a solver loop where a persistent field is reassigned from an expression
  containing itself — e.g. `rho = rho + div(phi)`, with `phi`'s name embedding
  `interpolate(rho)` — each operation glued two copies of the previous name
  together, so the `name` String **doubled every timestep** (`2^step`). The
  rhoPimpleFoam `compressible_lid_cavity` test grew to ~1.3 GB by step 24 and
  was killed by OOM at step 25, with each step running ~2× slower than the last.

  The trap: the field *data* was completely healthy — `internal`/`boundary`
  Vecs stayed the correct size and the physics (`|U|`, `|p|`, `|phi|`) stayed
  bounded — so it presented as a "leak"/hang rather than a numerical blow-up.
  It was localised by printing `field.name.len()` per step: `rho`/`phi` names
  doubled while freshly-`solve()`d fields (`U`, `p`, `he`) stayed constant.

  Fix: arithmetic operators now leave `self.name` as the left operand's name.
  This also matches OpenFOAM, where a `GeometricField` keeps its fixed
  registered `IOobject` name and solvers *print* residual labels rather than
  accumulating an expression string onto the field. After the fix the cavity
  runs at a flat ~0.45 ms/step and ~3.7 MB RSS. See the Translation-notes
  callout in `CLAUDE.md` for the full write-up.

## License

GPL-3.0-only (matching the upstream OpenFOAM sources).
