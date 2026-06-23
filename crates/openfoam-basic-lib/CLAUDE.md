# CLAUDE.md — openfoam-basic-lib

This crate is a pure-Rust translation of the OpenFOAM C++ primitive and
finite-volume library layer, scoped to the primitives needed to implement
compressible sonic solvers equivalent to **rhoPimpleFoam** and **sonicFoam**.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openfoam/`

---

## Porting workflow (MANDATORY — follow for every new port)

After adding any new type, function, or module, you MUST do **both**:

### 1. Update `src/prelude.rs`

Add all new public items to the appropriate `pub use` block in `src/prelude.rs`
so that a wildcard import `use openfoam_basic_lib::prelude::*` exposes them.

### 2. Update `README.md`

Add a row for each newly ported item to the **Ported items** table in
`README.md`.  The table lives under the `## What's implemented` heading.
Format: `| Module | Rust type / fn | C++ source | Notes |`.

Verification:
```bash
cargo test -p openfoam-basic-lib --lib   # must be green before committing
```

---

## Goal and scope

The crate climbs the OpenFOAM stack from the bottom up. Each layer depends
only on the one below it:

```
Layer 5  Solver logic       rhoPimpleFoam / sonicFoam loop
Layer 4  Thermophysics      fluidThermo / psiThermo
Layer 3  FV operators       fvm:: / fvc:: (ddt, div, grad, laplacian, …)
Layer 2  Fields + Mesh      volScalarField, fvMesh, fvMatrix
Layer 1  Primitives ← THIS CRATE
           • Tensor algebra   Vector3, Tensor, SymmTensor, SphericalTensor
           • Dense matrices   scalarSquareMatrix, LU/QR/Cholesky/SVD
           • Polynomial math  linearEqn, quadraticEqn, cubicEqn, Polynomial<N>
           • ODE solvers      Euler, RKF45, RKDP45, Rosenbrock23/34, seulex, …
           • Interpolation    interpolationTable, interpolateXY, spline
           • Math functions   erfInv, incGamma, invIncGamma, …
```

`openfoam-basic-lib` covers **Layer 1** only. Layers 2–5 will live in
separate crates that depend on this one.

---

## C++ source reference map

### Layer 1 — Primitives

**Location:** `src/OpenFOAM/primitives/`

| C++ type | C++ files | Rust target |
|---|---|---|
| `Foam::scalar` | `Scalar/scalar/scalar.H` | `f64` (type alias `Scalar`) |
| `Foam::label` | `ints/label/label.H` | `i64` (type alias `Label`) |
| `Foam::Vector<Cmpt>` | `Vector/Vector.H`, `VectorI.H` | `struct Vector3` |
| `Foam::Tensor<Cmpt>` | `Tensor/Tensor.H`, `TensorI.H` | `struct Tensor` |
| `Foam::SymmTensor<Cmpt>` | `SymmTensor/SymmTensor.H`, `SymmTensorI.H` | `struct SymmTensor` |
| `Foam::SphericalTensor<Cmpt>` | `SphericalTensor/SphericalTensor.H` | `struct SphericalTensor` |
| `VectorSpace<Form,Cmpt,N>` | `VectorSpace/VectorSpace.H` | Rust traits (not a struct) |
| product type traits | `VectorSpace/products.H` | Rust trait impls |

**Key operations from VectorI.H / TensorI.H (what to implement):**

`Vector3`:
- `new(x,y,z)`, `ZERO`, `ONE`
- `mag_sqr()`, `mag()`, `dist_sqr(v)`, `dist(v)`
- `inner(v) -> f64` (dot product; C++ `operator&`)
- `cross(v) -> Vector3` (C++ `operator^`)
- `normalise(tol) -> Vector3`
- `remove_collinear(unit_vec) -> Vector3`
- `lerp(a, b, t) -> Vector3`
- `outer(v) -> Tensor` (dyadic product `a ⊗ b`)
- `+`, `-`, `*f64`, `/f64`, `neg`, compound-assign variants

`SymmTensor` (6 components: `xx, xy, xz, yy, yz, zz`):
- `trace() -> f64` (= `xx + yy + zz`)
- `dev() -> SymmTensor` (deviatoric: `self - (1/3)*trace*I`)
- `dev2() -> SymmTensor` (= `self - (2/3)*trace*I`)
- `two_symm(t: Tensor) -> SymmTensor` (`T + T^T`)
- `symm(t: Tensor) -> SymmTensor` (`0.5*(T + T^T)`)
- `inner(s: SymmTensor) -> f64` (double contraction `a:b`)
- conversions to/from `Tensor`
- arithmetic: `+`, `-`, `*f64`, compound-assign

`SphericalTensor` (1 component: `ii`, represents `ii * I`):
- `identity() -> SphericalTensor` (where `ii = 1`)
- `trace() -> f64` (= `3 * ii`)
- conversions to `SymmTensor`, `Tensor`
- arithmetic with scalar

`Tensor` (9 components row-major: `xx,xy,xz,yx,yy,yz,zx,zy,zz`):
- construction from `SymmTensor`, `SphericalTensor`, three row `Vector3`s
- `transpose() -> Tensor`
- `trace() -> f64`
- `det() -> f64`
- `inv() -> Tensor`
- `symm() -> SymmTensor` (symmetric part)
- `skew() -> Tensor` (antisymmetric part: `0.5*(T - T^T)`)
- `inner(t: Tensor) -> f64` (double contraction)
- matrix-vector multiply: `Tensor * Vector3 -> Vector3`
- matrix-matrix multiply: `Tensor * Tensor -> Tensor`
- arithmetic: `+`, `-`, `*f64`, compound-assign

**Free functions** (mirrors OpenFOAM global functions in the same headers):
- `mag(v: Vector3) -> f64`, `mag_sqr(v: Vector3) -> f64`
- `dot(a: Vector3, b: Vector3) -> f64`
- `cross(a: Vector3, b: Vector3) -> Vector3`
- `outer(a: Vector3, b: Vector3) -> Tensor`
- `symm(t: Tensor) -> SymmTensor`
- `skew(t: Tensor) -> Tensor`
- `dev(s: SymmTensor) -> SymmTensor`
- `tr(t: Tensor) -> f64`, `tr(s: SymmTensor) -> f64`
- `inner(a: Tensor, b: Tensor) -> f64` (Frobenius / double contraction)
- `lerp(a: Vector3, b: Vector3, t: f64) -> Vector3`

---

### Layer 1b — Dense Matrices

**Location:** `src/OpenFOAM/matrices/`

Used directly by the ODE solvers (Jacobian storage and solve) and by the
sparse LDU solver internals.

| C++ type | C++ files | Rust target |
|---|---|---|
| `scalarSquareMatrix` | `SquareMatrix/SquareMatrix.H`, `scalarMatrices/scalarMatrices.H` | `struct SquareMatrix` (heap-allocated n×n) |
| `scalarRectangularMatrix` | `RectangularMatrix/RectangularMatrix.H` | `struct RectangularMatrix` (m×n) |
| `scalarSymmetricSquareMatrix` | `SymmetricSquareMatrix/SymmetricSquareMatrix.H` | `struct SymmetricSquareMatrix` |
| `scalarDiagonalMatrix` | `DiagonalMatrix/DiagonalMatrix.H` | `struct DiagonalMatrix` (Vec of diag entries) |
| `LUscalarMatrix` | `LUscalarMatrix/LUscalarMatrix.H` | `struct LuMatrix` (LU factorisation + pivot) |
| `QRMatrix<M>` | `QRMatrix/QRMatrix.H` | `struct QrMatrix` (QR with optional pivoting) |
| Cholesky | `scalarMatrices/scalarMatrices.C` (`LUDecompose` for symmetric) | `fn cholesky_decompose` |
| SVD | `scalarMatrices/SVD/SVD.H` | `struct Svd` |

**Key operations on `scalarSquareMatrix` (needed by ODE stiff solvers):**
- `new(n)`, `set_size(n)`
- `operator()(i,j)` → element access
- `LU_decompose(m, pivot) → ()` (in-place)
- `LU_back_substitute(m, pivot, x)` (solve after factorisation)
- `LU_solve(m, source, x)` (combined)
- `inverse(m)` → `scalarSquareMatrix`

---

### Layer 1c — Polynomial Equation Solvers

**Location:** `src/OpenFOAM/primitives/polynomialEqns/`

Analytical root-finding for degree 1–3 polynomials, used in thermodynamics
(equation of state root finding) and geometry.

| C++ type | C++ files | Description |
|---|---|---|
| `Roots<N>` | `Roots.H`, `RootsI.H` | Tagged root container: up to N real or complex roots, each tagged `real`/`complex`/`posInf`/`negInf`/`nan` |
| `linearEqn` | `linearEqn/linearEqn.H` | Solves `a*x + b = 0` → `roots() -> Roots<1>` |
| `quadraticEqn` | `quadraticEqn/quadraticEqn.H` | Solves `a*x² + b*x + c = 0` → `roots() -> Roots<2>` |
| `cubicEqn` | `cubicEqn/cubicEqn.H` | Solves `a*x³ + b*x² + c*x + d = 0` → `roots() -> Roots<3>` (Cardano + numerical refinement) |

**`Roots<N>` interface:**
- `type(i)` — `RootType` tag (real, complex pair, inf, nan)
- `operator[](i) -> scalar` — the root value
- `real(i) -> scalar`, `imag(i) -> scalar` — complex components when tagged complex

**Rust target:** `enum RootType`, `struct Roots<const N: usize>`, then `struct LinearEqn`, `QuadraticEqn`, `CubicEqn` each with a `roots()` method.

---

### Layer 1d — Polynomial Function Evaluation

**Location:** `src/OpenFOAM/primitives/functions/Polynomial/`

Used by thermophysical models (Cp, mu, kappa as polynomial fits in T).

| C++ type | C++ files | Description |
|---|---|---|
| `Polynomial<N>` | `Polynomial.H`, `Polynomial.C` | Fixed-order: `sum(coeffs[i]*x^i) + logCoeff*log(x)` |
| `polynomialFunction` | `polynomialFunction.H`, `polynomialFunction.C` | Variable-order (heap); value, derivative, integral |

**Key methods on `Polynomial<N>`:**
- `value(x) -> scalar`
- `derivative(x) -> scalar`
- `integral(x1, x2) -> scalar`
- `integral() -> Polynomial<N+1>` (returns integrated polynomial)
- `integralMinus1() -> Polynomial<N+1>` (integral where base starts at order −1)

**Rust target:** `struct Polynomial<const N: usize>` with `coeffs: [f64; N]` and `log_coeff: f64`.

---

### Layer 1e — ODE Solvers

**Location:** `src/ODE/`

Stand-alone ODE integration library. Used by combustion/chemistry sub-stepping
and by turbulence model source-term integration. Independent of mesh.

#### Interface (`ODESystem.H` / `ODESolver.H`)

```
trait OdeSystem {
    fn n_eqns(&self) -> usize;
    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut [f64]);
    fn jacobian(&self, x: f64, y: &[f64], dfdx: &mut [f64], dfdy: &mut SquareMatrix);
}

struct StepState { dx_try, dx_did, first, last, reject, prev_reject }

trait OdeSolver {
    fn solve_step(&self, x: &mut f64, y: &mut [f64], dx_try: &mut f64);
    fn solve_range(&self, x_start: f64, x_end: f64, y: &mut [f64], dx_est: &mut f64);
}
```

`normalizeError(y0, y, err) -> f64` — weighted RMS error used by all
adaptive solvers to accept/reject steps and scale the next step.

#### Solver inventory (`src/ODE/ODESolvers/`)

| Solver | C++ dir | Type | Order | Notes |
|---|---|---|---|---|
| `Euler` | `Euler/` | explicit | (0)1 | simplest; error = O(h²) |
| `Trapezoid` | `Trapezoid/` | explicit | (1)2 | 2-stage; embedded error |
| `RKF45` | `RKF45/` | explicit | (4)5 | Fehlberg; classic adaptive |
| `RKCK45` | `RKCK45/` | explicit | (4)5 | Cash-Karp coefficients |
| `RKDP45` | `RKDP45/` | explicit | (4)5 | Dormand-Prince; dense output |
| `EulerSI` | `EulerSI/` | semi-implicit | (0)1 | stiff; needs Jacobian |
| `Rosenbrock12` | `Rosenbrock12/` | stiff (L-stable) | (1)2 | Jacobian required |
| `Rosenbrock23` | `Rosenbrock23/` | stiff (L-stable) | (2)3 | recommended for stiff |
| `Rosenbrock34` | `Rosenbrock34/` | stiff (L-stable) | (3)4 | high accuracy stiff |
| `rodas23` | `rodas23/` | stiff (L-stable) | (2)3 | atmospheric chemistry |
| `rodas34` | `rodas34/` | stiff (L-stable) | (3)4 | atmospheric chemistry |
| `SIBS` | `SIBS/` | semi-implicit | variable | Bader-Deuflhard midpoint |
| `seulex` | `seulex/` | semi-implicit | variable | extrapolation; very high order |
| `adaptiveSolver` | `adaptiveSolver/` | wrapper | — | wraps any solver with sub-stepping |

**Which solver to use (following OpenFOAM convention):**
- Non-stiff (e.g. kinematic equations): `RKDP45`
- Mildly stiff (e.g. simple chemistry): `Rosenbrock23`
- Stiff (e.g. fast chemistry, radiation): `Rosenbrock34` or `rodas34`
- Very stiff with wide timescale spread: `seulex`

**Adaptive step-size control** (all adaptive solvers share this pattern):
```
err = normalizeError(y0, y_new, y_err)
if err <= 1.0: accept step, scale dx up
else:          reject step, scale dx down (with safety factor 0.9)
```

---

### Layer 1f — Interpolation Utilities

**Location:** `src/OpenFOAM/interpolations/`

Used by thermophysical property tables and boundary condition profiles.
All are mesh-independent.

| C++ | Files | Description |
|---|---|---|
| `interpolateXY` | `interpolateXY/interpolateXY.H` | Piecewise-linear 1D: remap `yOld(xOld)` to new `xNew` points |
| `interpolateSplineXY` | `interpolateSplineXY/interpolateSplineXY.H` | Catmull-Rom spline 1D |
| `interpolationTable<T>` | `interpolationTable/interpolationTable.H` | Lookup table (scalar→T) with clamping/repeating out-of-bounds; reads CSV |
| `interpolation2DTable<T>` | `interpolation2DTable/interpolation2DTable.H` | Bilinear table (2 independent axes) |

**`interpolationTable<T>` key interface:**
- `operator()(x) -> T` — evaluate at x; out-of-bounds: `clamp`, `warn`, `error`, or `repeat`
- Constructed from a `(scalar, T)` list or from a CSV file path
- Stores as `List<Tuple2<scalar, T>>`; searches by bisection

**Rust target:**
- `fn interpolate_xy(x_new: &[f64], x_old: &[f64], y_old: &[T]) -> Vec<T>` (linear)
- `fn interpolate_spline_xy(x_new: &[f64], x_old: &[f64], y_old: &[f64]) -> Vec<f64>` (Catmull-Rom)
- `struct LookupTable<T>` with `eval(x: f64) -> T` and `OutOfBounds` enum

---

### Layer 1g — Math Special Functions

**Location:** `src/OpenFOAM/primitives/functions/Math/`

| C++ function | Description | Rust target |
|---|---|---|
| `erfInv(y)` | Inverse error function | `fn erf_inv(y: f64) -> f64` |
| `incGammaRatio_P(a, x)` | Regularised lower incomplete gamma P(a,x) | `fn inc_gamma_ratio_p(a: f64, x: f64) -> f64` |
| `incGammaRatio_Q(a, x)` | Regularised upper incomplete gamma Q(a,x) = 1−P | `fn inc_gamma_ratio_q(a: f64, x: f64) -> f64` |
| `incGamma_P(a, x)` | Lower incomplete gamma γ(a,x) = Γ(a)·P(a,x) | `fn inc_gamma_p(a: f64, x: f64) -> f64` |
| `incGamma_Q(a, x)` | Upper incomplete gamma Γ(a,x) = Γ(a)·Q(a,x) | `fn inc_gamma_q(a: f64, x: f64) -> f64` |
| `invIncGamma(a, P)` | Inverse: find x such that P(a,x) = P | `fn inv_inc_gamma(a: f64, p: f64) -> f64` |

These are used in turbulence and combustion sub-models. The C++ source files
are `erfInv.C`, `incGamma.C`, `invIncGamma.C` in the same directory.

---

### Layer 2 — Fields and Mesh

**Location:** `src/OpenFOAM/fields/` and `src/finiteVolume/`

| C++ | Files | Description |
|---|---|---|
| `Foam::Field<T>` | `Fields/Field/Field.H` | Flat array `Vec<T>` with element-wise ops |
| `Foam::DimensionedField<T,Mesh>` | `DimensionedFields/DimensionedField/DimensionedField.H` | Field + SI dimensions + mesh ref |
| `Foam::GeometricField<T,PF,Mesh>` | `GeometricFields/GeometricField/GeometricField.H` | Internal field + boundary patch fields |
| `Foam::volScalarField` | `finiteVolume/fields/volFields/volFields.H` | `GeometricField<scalar, fvPatchField, volMesh>` |
| `Foam::volVectorField` | same | `GeometricField<Vector, fvPatchField, volMesh>` |
| `Foam::volTensorField` | same | `GeometricField<Tensor, …>` |
| `Foam::volSymmTensorField` | same | `GeometricField<SymmTensor, …>` |
| `Foam::surfaceScalarField` | `finiteVolume/fields/surfaceFields/surfaceFields.H` | `GeometricField<scalar, fvsPatchField, surfaceMesh>` |
| `Foam::surfaceVectorField` | same | `GeometricField<Vector, fvsPatchField, surfaceMesh>` |
| `Foam::fvMesh` | `finiteVolume/fvMesh/fvMesh/fvMesh.H` | Mesh + connectivity |

**fvMesh key data (what fvMesh.H exposes):**
- `V()` — cell volumes `[nCells]` (volScalarField)
- `Sf()` — face area vectors `[nFaces]` (surfaceVectorField, outward-pointing)
- `magSf()` — face areas `[nFaces]` (surfaceScalarField)
- `C()` — cell centres `[nCells]` (volVectorField)
- `Cf()` — face centres `[nFaces]` (surfaceVectorField)
- `owner()` — owner cell index per face `[nFaces]`
- `neighbour()` — neighbour cell index per internal face `[nInternalFaces]`
- Boundary patches (list of `fvPatch`)

**Boundary condition base classes:**
- `src/finiteVolume/fields/fvPatchFields/basic/` — `fixedValue`, `zeroGradient`, `calculated`, `symmetry`
- `src/finiteVolume/fields/fvsPatchFields/basic/` — surface-field BCs

**fvMatrix (sparse linear system):**
- `src/finiteVolume/fvMatrices/fvMatrix/fvMatrix.H`
- Storage: `diag[nCells]`, `lower[nInternalFaces]`, `upper[nInternalFaces]`, `source[nCells]`
- Extends `lduMatrix` (lower-diagonal-upper sparse format)
- Key methods: `A()` (diagonal), `H()` (off-diagonal contribution to rhs), `relax()`, `solve()`, `flux()`
- Specializations: `fvScalarMatrix = fvMatrix<scalar>`, `fvVectorMatrix = fvMatrix<Vector>`

---

### Layer 3 — Finite Volume Operators

**Location:** `src/finiteVolume/finiteVolume/`

#### fvm:: (implicit — adds to matrix)

| Operator | C++ file | What it assembles |
|---|---|---|
| `fvm::ddt(rho, U)` | `ddtSchemes/` | `∂(ρU)/∂t` — adds to diagonal and source |
| `fvm::div(phi, U)` | `divSchemes/` | `∇·(φU)` — convection, adds off-diagonal |
| `fvm::laplacian(α, U)` | `laplacianSchemes/` | `∇·(α∇U)` — diffusion, adds off-diagonal |
| `fvm::div(phid, p)` | same div | `∇·(ψ_d p)` — transonic pressure convection |
| `fvm::ddt(psi, p)` | same ddt | `∂(ψp)/∂t` — compressibility storage term |
| `fvm::SuSp(sp, p)` | `fvmSup.H` | source/sink coupling |

**Time schemes** (`ddtSchemes/`): `EulerDdtScheme` (first order), `backwardDdtScheme` (second order), `CrankNicolsonDdtScheme`, `localEulerDdtScheme` (local time-stepping for LTS/pseudo-transient)

**Gradient schemes** (`gradSchemes/`): `gaussGrad`, `leastSquaresGrad`

**Divergence/convection schemes** (`divSchemes/`, `convectionSchemes/`): `gaussConvectionScheme` with upwind, linear, limitedLinear, MUSCL interpolations

**Laplacian schemes** (`laplacianSchemes/`): `gaussLaplacianScheme` with orthogonal or non-orthogonal snGrad

#### fvc:: (explicit — returns a field)

| Operator | C++ file | Returns |
|---|---|---|
| `fvc::ddt(rho, K)` | `fvcDdt.H/C` | `volScalarField` |
| `fvc::div(phi, K)` | `fvcDiv.H/C` | `volScalarField` |
| `fvc::div(phi, U)` | same | `volVectorField` |
| `fvc::grad(p)` | `fvcGrad.H/C` | `volVectorField` |
| `fvc::laplacian(α, he)` | `fvcLaplacian.H/C` | `volScalarField` |
| `fvc::interpolate(rho)` | via `surfaceInterpolation/` | `surfaceScalarField` |
| `fvc::flux(HbyA)` | `fvcFlux.H/C` | `surfaceScalarField` (dot with Sf) |
| `fvc::ddtCorr(rho,U,phi,rhoUf)` | `fvcDdt.H/C` | `surfaceScalarField` (ddt correction) |
| `fvc::snGrad(p)` | `fvcSnGrad.H/C` | `surfaceScalarField` (face-normal gradient) |
| `fvc::reconstruct(phi)` | `fvcReconstruct.H/C` | `volVectorField` (from flux) |
| `fvc::absolute(phi, rho, U)` | `fvcMeshPhi.H` | absolute flux (mesh motion) |
| `fvc::makeRelative(phi, rho, U)` | same | modifies phi in-place |

**Surface interpolation** (`src/finiteVolume/interpolation/surfaceInterpolation/`):
- `schemes/`: linear, upwind, linearUpwind, limitedLinear, vanLeer, Gamma
- `limitedSchemes/`: TVD limiters

---

### Layer 4 — Thermophysical Models

**Location:** `src/thermophysicalModels/basic/`

| C++ | Files | Notes |
|---|---|---|
| `Foam::basicThermo` | `basicThermo/basicThermo.H` | Abstract base |
| `Foam::fluidThermo` | `fluidThermo/fluidThermo.H` | Adds `psi()`, `correctRho()` |
| `Foam::psiThermo` | `psiThermo/psiThermo.H` | ρ via ψ·p; used by **sonicFoam** |
| `Foam::rhoThermo` | `rhoThermo/rhoThermo.H` | Explicit ρ; used by **rhoPimpleFoam** |

**Interface (what the solvers call):**
```
thermo.p()      → volScalarField&   (pressure)
thermo.T()      → volScalarField&   (temperature)
thermo.rho()    → volScalarField    (density, computed)
thermo.he()     → volScalarField&   (enthalpy h or internal energy e)
thermo.psi()    → volScalarField&   (compressibility ψ = ∂ρ/∂p|T)
thermo.mu()     → volScalarField    (dynamic viscosity)
thermo.kappa()  → volScalarField    (thermal conductivity)
thermo.alpha()  → volScalarField    (thermal diffusivity = kappa/Cp)
thermo.correct()                    (recompute T, rho, etc. after he/p update)
thermo.correctRho(psi*p - psip0, rhoMin, rhoMax)  (density bound after pEqn)
```

---

### Layer 5 — Solver Equations

Both solvers solve the same set of PDEs. The difference is thermodynamic
closure and whether the mesh can move.

#### Field variables (from `createFields.H`)

| Variable | C++ type | Physical meaning |
|---|---|---|
| `p` | `volScalarField` | pressure |
| `rho` | `volScalarField` | density |
| `U` | `volVectorField` | velocity |
| `phi` | `surfaceScalarField` | mass flux `= ρ U·Sf` |
| `he` (or `e`) | `volScalarField` | enthalpy / internal energy |
| `K` | `volScalarField` | kinetic energy `= 0.5*|U|²` |
| `dpdt` | `volScalarField` | `∂p/∂t` (for enthalpy form) |
| `psi` | `volScalarField` | compressibility `ψ = ρ/p` |
| `rhoMin`, `rhoMax` | `dimensionedScalar` | density bounds |

#### Continuity equation (`rhoEqn.H`, common to both)
```
∂ρ/∂t + ∇·(ρU) = 0
→  fvm::ddt(rho) + fvc::div(phi) == 0
```

#### Momentum equation (`UEqn.H`, identical in both solvers)
```
∂(ρU)/∂t + ∇·(ρUU) + τ_turbulent = -∇p + sources
→  fvm::ddt(rho,U) + fvm::div(phi,U) + turbulence->divDevRhoReff(U)
   == fvOptions(rho,U)
then solve: UEqn == -fvc::grad(p)
```
`divDevRhoReff(U)` = `∇·(−2μ_eff · dev(symm(∇U)))` (turbulent deviatoric stress)

#### Energy equation (`EEqn.H`)

*rhoPimpleFoam* (h or e form, runtime selection):
```
∂(ρh)/∂t + ∇·(ρUh) + ∂(ρK)/∂t + ∇·(ρUK)
  − ∇·(α_eff ∇h) + [−∂p/∂t  if h-form  |  ∇·(U p)  if e-form]
  == sources
```

*sonicFoam* (internal energy e only):
```
∂(ρe)/∂t + ∇·(ρUe) + ∂(ρK)/∂t + ∇·(ρUK)
  + ∇·(p U) − ∇·(α_eff ∇e)
  == sources
```

#### Pressure equation (`pEqn.H`)

*sonicFoam* (simpler):
```
∂(ψp)/∂t + ∇·(ψ_d p) − ∇·(ρ/A · ∇p) = 0
→  fvm::ddt(psi,p) + fvm::div(phid,p) - fvm::laplacian(rhorAUf, p)
   == fvOptions(psi,p,rho)
```

*rhoPimpleFoam* (transonic or subsonic branch):
- Subsonic: `fvc::ddt(rho) + psi*fvm::ddt(p) + ∇·φ_HbyA - ∇·(ρ/A ∇p) = 0`
- Transonic: adds `fvm::div(phid, p)` convective pressure term

After each pressure solve:
```
U = HbyA - rAU * fvc::grad(p)
rho = thermo.rho()           (or thermo.correctRho(psi*p - psip0))
phi = phiHbyA + pEqn.flux()
K = 0.5 * magSqr(U)
```

#### PIMPLE outer loop structure
```
while time:
  [rhoEqn — rhoPimpleFoam only on first PIMPLE iter]
  while pimple.loop():          # outer correctors (SIMPLE-rho or PISO)
    UEqn  → momentum predictor
    EEqn  → energy
    while pimple.correct():     # inner pressure correctors
      pEqn  → pressure + flux
    turbulence.correct()
  rho = thermo.rho()
```

---

## Rust module plan for this crate

```
src/
  lib.rs
  primitives/              ← Layer 1a: tensor algebra
    mod.rs
    scalar.rs              ← type Scalar = f64; type Label = i64
    vector.rs              ← struct Vector3; operators; mag, cross, outer, …
    symm_tensor.rs         ← struct SymmTensor; trace, dev, dev2, symm(Tensor)
    spherical_tensor.rs    ← struct SphericalTensor; into SymmTensor/Tensor
    tensor.rs              ← struct Tensor; transpose, det, inv, symm, skew
  matrices/                ← Layer 1b: dense linear algebra
    mod.rs
    square_matrix.rs       ← struct SquareMatrix (heap n×n, row-major)
    rectangular_matrix.rs  ← struct RectangularMatrix (m×n)
    symmetric_matrix.rs    ← struct SymmetricSquareMatrix
    diagonal_matrix.rs     ← struct DiagonalMatrix
    lu.rs                  ← LU factorisation + back-substitution + solve
    qr.rs                  ← QR decomposition (with optional column pivoting)
    cholesky.rs            ← Cholesky factorisation for symmetric positive-definite
    svd.rs                 ← SVD (thin)
  polynomial/              ← Layer 1c+1d: polynomial algebra
    mod.rs
    roots.rs               ← enum RootType; struct Roots<const N: usize>
    linear_eqn.rs          ← struct LinearEqn; roots()
    quadratic_eqn.rs       ← struct QuadraticEqn; roots()
    cubic_eqn.rs           ← struct CubicEqn; roots() (Cardano)
    polynomial.rs          ← struct Polynomial<const N: usize>; value, deriv, integral
  ode/                     ← Layer 1e: ODE solvers
    mod.rs
    ode_system.rs          ← trait OdeSystem { n_eqns, derivatives, jacobian }
    ode_solver.rs          ← trait OdeSolver; struct StepState; normalize_error
    euler.rs               ← explicit O(1)
    trapezoid.rs           ← explicit O(2)
    rkf45.rs               ← explicit O(4/5) Fehlberg
    rkck45.rs              ← explicit O(4/5) Cash-Karp
    rkdp45.rs              ← explicit O(4/5) Dormand-Prince (default non-stiff)
    euler_si.rs            ← semi-implicit O(1)
    rosenbrock12.rs        ← stiff L-stable O(1/2)
    rosenbrock23.rs        ← stiff L-stable O(2/3) (default stiff)
    rosenbrock34.rs        ← stiff L-stable O(3/4)
    rodas23.rs             ← stiff L-stable O(2/3) (chemistry)
    rodas34.rs             ← stiff L-stable O(3/4) (chemistry)
    sibs.rs                ← semi-implicit variable-order Bader-Deuflhard
    seulex.rs              ← semi-implicit extrapolation (very stiff)
    adaptive_solver.rs     ← wrapper: sub-step any solver over a fixed interval
  interpolation/           ← Layer 1f: interpolation utilities
    mod.rs
    linear.rs              ← interpolate_xy (piecewise linear remap)
    spline.rs              ← interpolate_spline_xy (Catmull-Rom)
    lookup_table.rs        ← struct LookupTable<T>; eval; OutOfBounds enum
    lookup_table_2d.rs     ← struct LookupTable2d<T>; bilinear eval
  math/                    ← Layer 1g: special functions
    mod.rs
    erf_inv.rs             ← fn erf_inv(y: f64) -> f64
    inc_gamma.rs           ← inc_gamma_ratio_p/q, inc_gamma_p/q, inv_inc_gamma
```

Future crates (not in this one):
- `openfoam-fields` — Field, GeometricField, volScalarField, fvMesh
- `openfoam-fv` — fvm:: / fvc:: operators, fvMatrix
- `openfoam-thermo` — fluidThermo / psiThermo
- `openfoam-solvers` — PIMPLE loop, rhoPimpleFoam, sonicFoam

---

## Translation notes

- OpenFOAM's primitives are templated over `Cmpt` (component type). In Rust
  we target `f64` (`scalar = double` in OpenFOAM) directly for now. Generics
  can be added later if needed.
- OpenFOAM uses expression-template `tmp<T>` to avoid heap copies.
  In Rust this maps to returning values by move (zero-cost in most cases).
- C++ `operator&` = dot product on vectors; `operator^` = cross product.
  Rust uses named methods `inner()` / `cross()` and free functions `dot()` / `cross()`.
- C++ `operator&&` on tensors = double contraction (Frobenius). Rust: `inner()`.
- `magSqr(U)` on a field means element-wise `|u_i|²`; on a single `Vector3`
  it means `u.x² + u.y² + u.z²` — same function, different dispatch.
- The `dev(symm(grad(U)))` chain (needed for `divDevRhoReff`) requires: grad
  (Layer 3) → SymmTensor at each cell → `dev()` → divergence. Only the
  `SymmTensor::dev()` method belongs in Layer 1.
- `fvc::interpolate` is linear by default (central differencing face value).
  Upwinding is a scheme choice registered at runtime in C++; in Rust this
  will likely be a trait or enum parameter.
- OpenFOAM's `scalarSquareMatrix` is heap-allocated and dynamically sized.
  The `ndarray` crate (`Array2<f64>`) is the natural Rust equivalent.
  The dense matrix module will wrap `ndarray::Array2` rather than
  reinventing storage.
- The ODE `ODESystem` is a pure virtual interface in C++; in Rust it becomes
  a trait. The Jacobian method is optional for explicit solvers — use a
  default no-op impl in the trait so explicit solvers don't force users to
  implement it.
- `Polynomial<N>` in C++ is templated on a compile-time size `N`. In Rust
  this maps to a const-generic struct `Polynomial<const N: usize>`. The
  `logCoeff` term (for NASA polynomial fits) must be preserved — it is not
  zero in thermodynamic use.
- `cubicEqn::roots()` uses a two-substitution Cardano method followed by one
  Newton-Raphson refinement step. The refinement is important for numerical
  accuracy when two roots are close; preserve it in the translation.
- `interpolationTable` internally bisects a sorted list. The Rust translation
  should use `slice::partition_point` (binary search) rather than a linear
  scan for O(log n) lookup.
- The `Roots<N>` root-tagging scheme (`real`, `complex`, `posInf`, `negInf`,
  `nan`) maps cleanly to a Rust enum. Complex root pairs are stored as two
  consecutive entries (real part, imag part); the tag on the first entry is
  `complex` and the second is implicitly the imaginary companion.
