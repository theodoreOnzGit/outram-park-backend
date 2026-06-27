# CLAUDE.md — openfoam-basic-lib

This crate is a pure-Rust translation of the OpenFOAM C++ primitive and
finite-volume library layer, scoped to the primitives needed to implement
compressible solvers equivalent to **rhoPimpleFoam**, **sonicFoam**, and
**rhoCentralFoam** (Kurganov-Tadmor central-upwind, density-based explicit).

The reference C++ source lives at:
`/home/teddy0/Documents/research/openfoam/`

---

## Why this crate exists

OpenFOAM's C++ codebase is the negative example this port is designed to undo:

- **Runtime type registration** via macros (`addToRunTimeSelectionTable`) hides the
  class hierarchy from any static tool — you cannot hover over a type and find its
  implementors.
- **Dictionary-driven input** (`fvSolution`, `fvSchemes`) — valid keys exist only in
  source comments and forum posts, not in any machine-readable interface.
- **wmake** — a bespoke build system nothing else can consume; OpenFOAM cannot be
  used as a library by another project.
- **No units discipline** — `scalar` is `double`; passing pressure where velocity is
  expected compiles silently and produces a wrong answer.

This crate replaces each of those with the Rust equivalent:
- Traits make the type hierarchy explicit and statically navigable via rust-analyzer.
- Struct fields with `///` doc comments replace runtime dictionaries — valid "inputs"
  are visible on hover.
- Cargo makes this crate a normal library dependency.
- `uom` makes unit errors compile errors.

**The mandatory consequence:** every public item must be navigable with rust-analyzer
alone, by a developer with no prior OpenFOAM knowledge. See the root `CLAUDE.md`
"Human interface layer" section for the full rule.

---

**Layer 5 (solver logic — PISO/PIMPLE loops, multi-region coupling, turbulence
model registries) is intentionally excluded from this crate.** It belongs in
separate solver crates (`openfoam-icof`, `openfoam-cht`, `openfoam-rho`) that
depend on this crate. See the workspace `CLAUDE.md` for the planned crate list.

---

## Remaining work before solver crates can be written

### For `openfoam-icof` (icoFoam)

All of the following must be added to `openfoam-basic-lib` first:

1. **`fvVectorMatrix`** — a vector variant of `FvMatrix` (or make `FvMatrix<T>`
   generic).  icoFoam's momentum equation is a vector system:
   `fvm::ddt(U) + fvm::div(phi,U) − fvm::laplacian(ν,U)`.

2. **`FvMatrix::A()` and `FvMatrix::H()`** — the diagonal (`A[c] = diag[c]/V[c]`)
   and the off-diagonal residual contribution (`H = (source − off-diag·x) / V`)
   needed to form `HbyA = rAU * UEqn.H()` in the PISO pressure step.

3. **`fvc::flux(U)`** — dot a `VolVectorField` with face area vectors → `SurfaceScalarField` (φ = U·Sf).

4. **`fvc::reconstruct(phi)`** — reconstruct a `VolVectorField` from a face flux
   (inverse of `fvc::flux`; uses least-squares or Gauss).

5. **`fvc::ddtCorr(U, phi, dt)`** — ddt correction term for the PISO flux update.

6. **Reference cell constraint** — pin one cell's pressure to avoid singular
   matrix in a closed domain.

7. **`adjustPhi`** — correct face fluxes for global mass balance.

No new external Rust crates are required.

### For `openfoam-cht` (chtMultiRegionFoam)

On top of all icoFoam requirements:

1. **Turbulence models** — trait `TurbulenceModel` with `divDevRhoReff(U) →
   FvVectorMatrix` and `correct()`; concrete implementations: `LaminarModel`
   (no-op), `kOmegaSST`.  No new external crates needed — just algorithmic Rust.

2. **Multi-region mesh coupling** — a `RegionCoupledPatch` concept that maps
   interface faces between two `FvMesh` instances and exchanges T and heat-flux
   values each timestep.  Requires a geometric point-search or face-centre
   interpolation between non-matching meshes (algorithmic, no new crates).

3. **Solid energy equation assembly** — using `SolidThermo` (already in this
   crate): `fvm::ddt(rho_cp, T) − fvm::laplacian(kappa, T) == 0`.

4. **Buoyancy source** — `fvc::reconstruct(fvc::interpolate(rho) * (g & mesh.Sf()))`.

5. **Wall distance field** — `yWallDist` for near-wall turbulence corrections;
   computed via a geometric sweep over wall boundary patches.

---

## Known test failures (marked `#[ignore]`, needs investigation)

These tests were written but fail with errors larger than expected. They are
`#[ignore]`-d so the CI suite stays green. They may indicate a deeper bug in
the implementation, not just a tolerance issue — investigate before un-ignoring.

### `janaf::tests::newton_converges_from_bad_initial_guess`
Newton iteration starting from `t0 = 100 K` targeting `ha(3000 K)` stalls at
~1152 K and never converges. The JANAF discontinuity at `Tcommon = 1000 K`
(different ha values in low vs high range) may cause Newton to settle at a
spurious root. Possible causes:
- The standard JANAF coefficients used in the test (N₂ proxy, GRI-Mech 3.0)
  have a large discontinuity at Tcommon, so there is a jump in ha that Newton
  cannot cross.
- The DTMAX=500 K clamp combined with the discontinuity may pin the iterate
  near Tcommon indefinitely.

### `peng_robinson::tests::co2_nist_density_400k_10mpa`
PR EOS gives 163.1 kg/m³ vs NIST 197.6 kg/m³ (17% error at Pr = 1.36).
Much larger than the expected ≤ 8% PR EOS error.

### `peng_robinson::tests::n2_nist_density_300k_10mpa`
PR EOS gives 113.6 kg/m³ vs NIST 105.8 kg/m³ (7% error at Pr = 2.94).

### `peng_robinson::tests::n2_nist_density_200k_5mpa`
PR EOS gives 95.5 kg/m³ vs NIST 75.5 kg/m³ (26% error at Tr = 1.59, Pr = 1.47).

The three PR EOS failures may share a root cause — possibly the Z-root
selection, the Soave α-function, or a unit/constant mismatch in
`peng_robinson.rs`. Review against the OpenFOAM C++ source in
`src/thermophysicalModels/specie/equationOfState/PengRobinsonGas/`.

---

## Test backlog — must clear before adding downstream crates

The crate is now load-bearing for the planned solver crates (`openfoam-icof`,
`openfoam-cht`). Test coverage must be raised before those crates start depending
on it. Items are listed in priority order.

### 🔴 P0 — Must clear before next downstream crate

#### `SquareMatrix::solve` failure-mode tests

- **Singular matrix** — verify `Err` is returned (or a well-defined fallback), not a panic or garbage result.
- **Ill-conditioned (Hilbert n=5, n=10)** — compute the solution, check residual `‖Ax − b‖` is within tolerance given the known condition number.
- **Scaled-partial-pivoting path** — construct a matrix where naïve pivoting fails but scaled pivoting succeeds; confirm correct result.
- **API decision needed:** change the return type of `SquareMatrix::solve` from `Vec<f64>` to `Result<Vec<f64>, _>` *before* more call sites exist. The current infallible API masks singular matrices silently. Do this before `teh-o-prke` and downstream solver crates adopt it.

#### Newton `T(H)` iteration robustness (JANAF)

- Convergence from a deliberately bad `t0` (e.g. `t0 = T_MIN = 100 K` for a target T of 3000 K).
- Behaviour at the `T_MIN = 100 K` and `T_MAX = 6000 K` clamps — verify they bind correctly and do not produce NaN/panic.
- JANAF discontinuity at `Tcommon` — construct a JANAF spec where the low/high ranges give slightly different `ha(Tcommon)`, confirm the iteration crosses cleanly.
- `MAX_ITER = 50` exhaustion path — must return `Err(NonConvergent)`, not silently return the last iterate.

#### Mixture blending invariants

- `(a += b)` conserves mole fractions (sum to 1 before and after).
- Roundtrip: `t_from_ha(ha(p, T), p, T) ≈ T` to relative tolerance 1e-6.

---

### 🟠 P1 — Required before the FV operator port (Layer 3)

#### Tensor algebra invariants

- `cross(a, b) · a == 0` and `cross(a, b) · b == 0` (orthogonality of cross product).
- `T == symm(T) + skew(T)` decomposition holds element-wise.
- `det(T · T⁻¹) ≈ 1` and `(T⁻¹)⁻¹ ≈ T` (inversion roundtrip).
- `inner(T1, T2) == inner(T2, T1)` (double-contraction symmetry).
- `SymmTensor::dev()` has trace 0.
- **`dev2` regression test** — OpenFOAM's `dev2 = T − (2/3)·tr·I`, *not* the standard `(1/3)·tr·I`. This asymmetric naming convention is easy to mis-port; add a specific regression test with known values.

#### FV operator method-of-manufactured-solutions

These are the riskiest area of the port — test each operator in isolation on a uniform mesh with a known analytic field:

- `fvc::grad(linear field)` — result must equal the constant gradient to machine precision on a uniform mesh.
- `fvm::laplacian(γ, T)` with a known analytic source — recover the analytic `T` solution.
- `fvc::flux(U) → fvc::reconstruct → U` roundtrip on a divergence-free field.
- Conservation: `Σ fvc::div(φψ) · V == boundary flux` (discrete divergence theorem).

---

### 🟡 P2 — Robustness; defer if time-boxed

#### Polynomial root finding (`CubicEqn`)

- Triple root `(x − 2)³` — all three roots must be `real` and equal to 2.
- One real + complex conjugate pair (negative discriminant) — correct `RootType` tags.
- Near-zero leading coefficient — should degrade gracefully to `QuadraticEqn` or return `posInf`/`negInf`.
- Correct `RootType` tagging (`real` / `complex` / `posInf` / `negInf` / `nan`) for each case.

#### ODE solvers

- Linear decay `dy/dt = −λy` — compare to exact exponential reference across all solvers.
- Order verification: halve `dt`, confirm the global error drops by `2^p` (order `p` of each solver).
- Stiffness test (Van der Pol or Robertson) — `Rosenbrock23` must converge; `RKF45` is expected to be slow or fail; validates the stiff/non-stiff split.

#### `PengRobinsonGas` Z-root selection

- **Vapour branch:** largest real Z root must be selected.
- **Liquid branch:** smallest real Z root must be selected.
- **NIST reference points for validation** — test at least these two gases across a `(p, T)` grid:
  - **CO₂:** critical point `Tc = 304.13 K`, `pc = 7.377 MPa`, `ω = 0.2239`. Test at `(10 MPa, 320 K)` (supercritical), `(5 MPa, 280 K)` (liquid), `(1 MPa, 350 K)` (vapour). Reference densities from NIST WebBook.
  - **N₂:** `Tc = 126.19 K`, `pc = 3.396 MPa`, `ω = 0.0372`. Test at `(20 MPa, 300 K)`, `(5 MPa, 150 K)`, `(0.1 MPa, 300 K)`. Reference densities from NIST WebBook.
  - Target tolerance: `|ρ_PR − ρ_NIST| / ρ_NIST ≤ 3%` for points away from the critical point; accept wider tolerance within `|T − Tc| / Tc < 0.05`.

---

## Implementation rules

### `extern "C"` policy

`extern "C"` blocks are **permitted if and only if** the called function
compiles and links natively on **all five** target platforms:

| Platform | C runtime | Notes |
|---|---|---|
| Linux | glibc / musl (`libm`) | always available |
| macOS | Apple `libSystem` | always available |
| Windows | MSVC CRT (VS 2013+) or MinGW | always available on modern toolchains |
| Android | Bionic libc | `erf`/`erfc`/`lgamma` since API 9; `tgamma` since API 21 |
| iOS | Apple `libSystem` | always available |

If a function is missing from **any** of these five (e.g. it only exists on
POSIX systems), it must be implemented in pure Rust instead.

The current `extern "C"` calls to `erf`, `erfc`, `tgamma`, and `lgamma` in
`inc_gamma.rs` and `inv_inc_gamma.rs` satisfy this rule (minimum Android
API 21 for `tgamma`, which covers ~98% of active devices).

Prefer Rust stdlib equivalents where they exist — `f64::gamma()` and
`f64::ln_gamma()` (stable since Rust 1.83) can replace `tgamma`/`lgamma`
with no FFI at all.

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
cargo test -p openfoam-basic-lib --lib --release   # must be green before committing
cargo test -p openfoam-basic-lib --doc --release   # must be green before committing
```

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

### Doc-comment code blocks

Any ```` ```rust ```` block in a doc comment is compiled **and executed** as a
doctest (`cargo test --doc`). Rules:

- **Never use `ignore` to silence a failing doctest.** Fix the code instead.
- `crate::` does not work in doctests — rustdoc compiles each snippet as an
  external user of the crate. Use the external crate name instead:
  `openfoam_basic_lib::` (e.g. `use openfoam_basic_lib::thermophysics::imports::*;`).
- Do not use `crate::` in doc-comment examples at all; always write the
  fully-qualified external path so the snippet is self-contained.
- ```` ```rust,no_run ```` is acceptable only for snippets that are genuinely
  side-effectful or require external resources at runtime.

---

## Goal and scope

The crate climbs the OpenFOAM stack from the bottom up. Each layer depends
only on the one below it:

```
Layer 5  Solver logic       rhoPimpleFoam / sonicFoam / rhoCentralFoam loops
Layer 4  Thermophysics      fluidThermo / psiThermo / rhoThermo
Layer 3  FV operators       fvm:: / fvc:: + Kurganov-Tadmor central-upwind fluxes
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

### Layer 1h — Specie-Level Thermophysics

**Location:** `src/thermophysicalModels/specie/`

This layer provides pure per-molecule thermophysical kernels: equation of state,
heat capacity / enthalpy / entropy, and transport coefficients (viscosity,
conductivity). Everything here is mesh-independent — it is a function of `(p, T)`
only. The field-level wrappers (`fluidThermo`, `psiThermo`, `rhoThermo`) that
own `volScalarField` instances belong in a future `openfoam-thermo` crate.

#### Scope boundary

| In this crate | Future crate |
|---|---|
| Per-species EOS, thermo, transport kernels | `fluidThermo`, `psiThermo`, `rhoThermo` |
| Newton iteration T(H) on a single point | Cell-loop + field correction (`thermo.correct()`) |
| Polynomial / tabulated property evaluation | Reaction chemistry (multi-species ODE) |

#### C++ source reference map

**Specie (`specie/specie/`):**

| C++ class | C++ file | Rust target |
|---|---|---|
| `Foam::specie` | `specie.H`, `specieI.H` | `struct Specie` |

`Specie` fields: `mol_weight: f64` (W, g/mol), `y: f64` (mass fraction in mixture).
Key methods: `w() -> f64`, `r() -> f64` (= R_UNIVERSAL / W), mixture `operator+=`.

**Equation of State (`equationOfState/`):**

| C++ class | C++ dir | Rust target | Layer 1 deps |
|---|---|---|---|
| `perfectGas<Sp>` | `perfectGas/` | `struct PerfectGas` | — |
| `rhoConst<Sp>` | `rhoConst/` | `struct RhoConst` | — |
| `icoPolynomial<Sp,N>` | `icoPolynomial/` | `struct IcoPolynomial<const N: usize>` | `Polynomial<N>` |
| `PengRobinsonGas<Sp>` | `PengRobinsonGas/` | `struct PengRobinsonGas` | `CubicEqn` |

EOS interface (all four methods per EOS type):
- `rho(p, T) -> f64` — density
- `psi(p, T) -> f64` — compressibility ∂ρ/∂p|T (0 for incompressible)
- `z(p, T) -> f64` — compressibility factor (1 for ideal gas)
- `cp_m_cv(p, T) -> f64` — Cp − Cv (= R for ideal gas; Maxwell relation for real gas)
- `h_eos(p, T) -> f64` — enthalpy departure from ideal gas (0 for perfect gas)
- `e_eos(p, T) -> f64` — internal energy departure (0 for perfect gas)
- `s_eos(p, T) -> f64` — entropy contribution from EOS (−R·ln(p/pref) for perfect gas)

`PerfectGas`: `ρ = p/(R·T)`, `ψ = 1/(R·T)`, `Z = 1`, `Cp−Cv = R`. Departures all zero.
`RhoConst`: `ρ = ρ₀`, `ψ = 0`. Departures all zero.
`IcoPolynomial<N>`: `1/ρ = poly(T)` — `Polynomial<N>` evalulated at T.
`PengRobinsonGas`: cubic EOS `p = R·T/(v−b) − a(T)/(v(v+b)+b(v−b))` with
Soave α function. Solves the cubic Z equation via `CubicEqn::roots()`.

**Z-root selection rule:** for vapour, take the *largest* real root; for liquid,
take the *smallest* real root; at the critical point / two-phase region the
middle root is unphysical and must be discarded.

**NIST validation reference points** (see also P2 test backlog above):

| Gas | Tc (K) | pc (MPa) | ω | Test point | Phase |
|---|---|---|---|---|---|
| CO₂ | 304.13 | 7.377 | 0.2239 | 10 MPa, 320 K | supercritical |
| CO₂ | 304.13 | 7.377 | 0.2239 | 5 MPa, 280 K | liquid |
| CO₂ | 304.13 | 7.377 | 0.2239 | 1 MPa, 350 K | vapour |
| N₂ | 126.19 | 3.396 | 0.0372 | 20 MPa, 300 K | supercritical |
| N₂ | 126.19 | 3.396 | 0.0372 | 5 MPa, 150 K | liquid |
| N₂ | 126.19 | 3.396 | 0.0372 | 0.1 MPa, 300 K | vapour |

Target: `|ρ_PR − ρ_NIST| / ρ_NIST ≤ 3%` away from the critical point;
accept wider tolerance within `|T − Tc| / Tc < 0.05`.

**Thermo (`thermo/`):**

| C++ class | C++ dir | Rust target | Layer 1 deps |
|---|---|---|---|
| `hConstThermo<EOS>` | `hConst/` | `struct HConstThermo<E>` | — |
| `janafThermo<EOS>` | `janaf/` | `struct JanafThermo<E>` | — |
| `hPolynomial<EOS,N>` | `hPolynomial/` | `struct HPolynomialThermo<E,N>` | `Polynomial<N>` |
| `hTabulated<EOS>` | `hTabulated/` | `struct HTabulatedThermo<E>` | `interpolate_xy` |

Thermo interface — all thermo models add these on top of the EOS they wrap:
- `cp(p, T) -> f64` — specific heat at constant pressure
- `ha(p, T) -> f64` — absolute enthalpy (sensible + formation + EOS departure)
- `hs(p, T) -> f64` — sensible enthalpy = ha − hc
- `hc() -> f64` — heat of formation (combustion reference)
- `s(p, T) -> f64` — specific entropy
- `cv(p, T) -> f64` — Cv = Cp − cp_m_cv
- `t_from_ha(ha, p, t0) -> f64` — Newton iteration: find T such that ha(p,T) = ha
- `t_from_hs(hs, p, t0) -> f64` — same but for sensible enthalpy
- `t_from_e(e, p, t0) -> f64` — same for internal energy

`HConstThermo`: `Cp = const`, `H = Cp·(T − Tref) + Href`, `S = Cp·ln(T/Tref)`.
Fields: `cp: f64, hf: f64, tref: f64, href: f64`.

`JanafThermo`: NASA 7-coefficient polynomial, dual range split at `Tcommon`.
Coefficients stored scaled by R (i.e. `a[i]·R`). Formulas:
```
Cp/R  = a[0] + a[1]·T + a[2]·T² + a[3]·T³ + a[4]·T⁴
H/RT  = a[0] + a[1]/2·T + a[2]/3·T² + a[3]/4·T³ + a[4]/5·T⁴ + a[5]/T
S/R   = a[0]·ln(T) + a[1]·T + a[2]/2·T² + a[3]/3·T³ + a[4]/4·T⁴ + a[6]
```
Fields: `tlow: f64, thigh: f64, tcommon: f64, low: [f64;7], high: [f64;7]`.

`HPolynomialThermo<E, N>`: `Cp = Polynomial<N>::value(T)`, enthalpy via
`Polynomial<N>::integral(Tref, T)`, entropy via `Polynomial<N>::integral_minus1(Tref, T)`.

`HTabulatedThermo<E>`: stores a `(T, Cp)` table; evaluates via `interpolate_xy`.

**Newton T(H) iteration** (shared default method, maps to `Foam::species::thermo<T>::T()`):
```
const DTMAX = 500.0;   // max step per iteration
const MAX_ITER = 50;
let mut t = t0.max(T_MIN);
for _ in 0..MAX_ITER {
    let f = self.ha(p, t) - ha_target;
    let cp = self.cp(p, t);
    let dt = (-f / cp).clamp(-DTMAX, DTMAX);
    t += dt;
    if dt.abs() / t < 1e-6 { break; }
}
t
```

**Transport (`transport/`):**

| C++ class | C++ dir | Rust target | Layer 1 deps |
|---|---|---|---|
| `constTransport<Thermo>` | `const/` | `struct ConstTransport<T>` | — |
| `sutherlandTransport<Thermo>` | `sutherland/` | `struct SutherlandTransport<T>` | — |
| `polynomialTransport<Thermo,N>` | `polynomial/` | `struct PolynomialTransport<T,N>` | `Polynomial<N>` |
| `tabulatedTransport<Thermo>` | `tabulated/` | `struct TabulatedTransport<T>` | `interpolate_xy` |

Transport interface — all transport models add on top of the thermo they wrap:
- `mu(p, T) -> f64` — dynamic viscosity [Pa·s]
- `kappa(p, T) -> f64` — thermal conductivity [W/(m·K)]
- `alpha_h(p, T) -> f64` — thermal diffusivity = κ/Cp [kg/(m·s)]

`ConstTransport`: `mu = const`, `Pr = const`, `kappa = mu·Cp/Pr`.
`SutherlandTransport`: `mu = As·√T / (1 + Ts/T)`, `kappa = mu·Cv·(1.32 + 1.77·R/Cv)`.
Sutherland coefficients from two reference points: `(mu1, T1)` and `(mu2, T2)`.
`PolynomialTransport<T, N>`: two `Polynomial<N>` — one for mu, one for kappa.
`TabulatedTransport<T>`: two `(T, value)` tables evaluated via `interpolate_xy`.

#### Physical constants

```rust
// src/thermophysics/constants.rs
pub const R_UNIVERSAL: f64 = 8314.46261815324;   // J/(kmol·K)  — matches Foam::RR
pub const T_MIN:       f64 = 100.0;              // K floor for Newton iteration
pub const T_MAX:       f64 = 6000.0;             // K ceiling for JANAF range
pub const P_REF:       f64 = 101325.0;           // Pa — standard atmosphere (for entropy)
```

#### Rust trait hierarchy

```rust
use uom::si::f64::{
    DynamicViscosity, MassDensity, MolarMass, Pressure, Ratio,
    SpecificEnergy, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};
use crate::thermophysics::quantities::Compressibility;

pub trait EquationOfState {
    fn mol_weight(&self) -> MolarMass;
    fn r(&self) -> SpecificHeatCapacity;         // R_UNIVERSAL / mol_weight; same dim as Cp
    fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity;
    fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility;
    fn z(&self, p: Pressure, t: ThermodynamicTemperature) -> Ratio;  // dimensionless
    fn cp_m_cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
    fn h_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnergy;
    fn e_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnergy;
    fn s_eos(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
}

pub trait ThermoModel: EquationOfState {
    fn cp(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
    fn ha(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnergy;
    fn hs(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificEnergy;
    fn hc(&self) -> SpecificEnergy;
    fn s(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
    fn cv(&self, p: Pressure, t: ThermodynamicTemperature) -> SpecificHeatCapacity;
    // Newton iteration — default impl provided in traits.rs
    fn t_from_ha(&self, ha: SpecificEnergy, p: Pressure, t0: ThermodynamicTemperature) -> ThermodynamicTemperature;
    fn t_from_hs(&self, hs: SpecificEnergy, p: Pressure, t0: ThermodynamicTemperature) -> ThermodynamicTemperature;
    fn t_from_e(&self, e: SpecificEnergy, p: Pressure, t0: ThermodynamicTemperature) -> ThermodynamicTemperature;
}

pub trait TransportModel: ThermoModel {
    fn mu(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;
    fn kappa(&self, p: Pressure, t: ThermodynamicTemperature) -> ThermalConductivity;
    // default impl: kappa / cp; same dimension as DynamicViscosity
    fn alpha_h(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;
}
```

All three traits use static dispatch (generics, not `dyn`) to match C++ template
zero-overhead composition. Typical concrete type:
```rust
type AirLow = SutherlandTransport<JanafThermo<PerfectGas>>;
```

#### Rust module plan (within this crate)

```
src/thermophysics/
  mod.rs
  constants.rs          ← R_UNIVERSAL, T_MIN, T_MAX, P_REF
  eos/
    mod.rs
    traits.rs           ← trait EquationOfState
    perfect_gas.rs      ← struct PerfectGas { mol_weight }
    rho_const.rs        ← struct RhoConst { mol_weight, rho0 }
    ico_polynomial.rs   ← struct IcoPolynomial<const N> { mol_weight, poly }
    peng_robinson.rs    ← struct PengRobinsonGas { mol_weight, tc, pc, omega }
  thermo/
    mod.rs
    traits.rs           ← trait ThermoModel; default Newton t_from_ha/hs/e
    h_const.rs          ← struct HConstThermo<E: EquationOfState>
    janaf.rs            ← struct JanafThermo<E: EquationOfState>
    h_polynomial.rs     ← struct HPolynomialThermo<E, const N>
    h_tabulated.rs      ← struct HTabulatedThermo<E>
  transport/
    mod.rs
    traits.rs           ← trait TransportModel; default alpha_h
    const_transport.rs  ← struct ConstTransport<T: ThermoModel>
    sutherland.rs       ← struct SutherlandTransport<T: ThermoModel>
    polynomial.rs       ← struct PolynomialTransport<T, const N>
    tabulated.rs        ← struct TabulatedTransport<T>
```

#### uom integration

Layer 1h uses **full uom** — all public trait method parameters and return values
use `uom::si::f64::*` named quantity types. Internal arithmetic extracts the raw
`f64` via `.get::<unit>()`, computes, then re-wraps with `Quantity::new::<unit>()`.
This matches the pattern already used in `tuas_boussinesq_solver`.

**Named uom types used (all from `uom::si::f64::*`):**

| Physical quantity | uom type | SI unit |
|---|---|---|
| Pressure | `Pressure` | Pa |
| Temperature | `ThermodynamicTemperature` | K |
| Density | `MassDensity` | kg/m³ |
| Specific heat capacity Cp, Cv | `SpecificHeatCapacity` | J/(kg·K) |
| Specific enthalpy / energy | `SpecificEnergy` | J/kg |
| Specific entropy | `SpecificHeatCapacity` | J/(kg·K) — same dimension |
| Specific gas constant R | `SpecificHeatCapacity` | J/(kg·K) — same dimension |
| Dynamic viscosity | `DynamicViscosity` | Pa·s = kg/(m·s) |
| Thermal conductivity | `ThermalConductivity` | W/(m·K) |
| Thermal diffusivity αh = κ/Cp | `DynamicViscosity` | kg/(m·s) — identical dimension |
| Molar mass | `MolarMass` | kg/mol |

**Note on molar mass convention:** OpenFOAM stores molecular weight in g/mol
(i.e. kg/kmol internally, since its `R_universal = 8314 J/(kmol·K)`). When
constructing a `PerfectGas` etc. the caller must convert: if they have 28.97 g/mol
for air, pass `MolarMass::new::<gram_per_mole>(28.97)`. The R_UNIVERSAL constant
in `constants.rs` must be stored in J/(mol·K) = 8.314462618... so that
`r = R_UNIVERSAL / mol_weight` gives J/(kg·K) when `mol_weight` is in kg/mol.

**Unnamed quantity — Compressibility ψ = ρ/p (dimensions s²/m²):**

```rust
// src/thermophysics/quantities.rs
use uom::si::{Quantity, SI};
use uom::typenum::{N2, Z0, P2};

/// Compressibility ψ = ∂ρ/∂p|T — SI units s²/m² (L⁻²·T²)
/// Used as the return type of EquationOfState::psi().
pub type Compressibility = Quantity<
    uom::si::ISQ<N2, Z0, P2, Z0, Z0, Z0, Z0>,
    SI<f64>,
    f64,
>;
```

To construct a value: `Compressibility::new::<uom::si::compressibility::per_pascal_second_squared>(v)` —
or equivalently compute it as a `MassDensity / Pressure` operation directly from
uom arithmetic (uom tracks dimensions through arithmetic automatically).

**Dimensionless quantities (Z, Prandtl number):** return `Ratio`
(`uom::si::f64::Ratio`), not plain `f64`.

**Standard imports in every thermophysics source file:**
```rust
use uom::si::f64::{
    DynamicViscosity, MassDensity, MolarMass, Pressure, Ratio,
    SpecificEnergy, SpecificHeatCapacity, ThermalConductivity,
    ThermodynamicTemperature,
};
```

**Arithmetic pattern — let uom compose dimensions directly:**

Never extract to `f64` and re-wrap unless there is no alternative. Let uom
operators track and check dimensions automatically:

```rust
// ρ = p / (R · T) — all three are already uom quantities; result is MassDensity
fn rho(&self, p: Pressure, t: ThermodynamicTemperature) -> MassDensity {
    p / (self.r() * t)
}

// ψ = ρ / p — MassDensity / Pressure = s²/m² = Compressibility
fn psi(&self, p: Pressure, t: ThermodynamicTemperature) -> Compressibility {
    self.rho(p, t) / p
}

// αh = κ / Cp — ThermalConductivity / SpecificHeatCapacity = kg/(m·s) = DynamicViscosity
fn alpha_h(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity {
    self.kappa(p, t) / self.cp(p, t)
}
```

Only use `.get::<unit>()` / `::new::<unit>()` for genuinely scalar expressions
(e.g. the JANAF polynomial `a0 + a1·T + …`) where there is no existing uom
quantity to receive an intermediate step. Even then, re-wrap immediately so the
return type is always a uom quantity.

---

#### Implementation order

1. `constants.rs` — R_UNIVERSAL, T_MIN, T_MAX, P_REF
2. `eos/traits.rs` + `eos/perfect_gas.rs` — simplest, anchors all thermo tests
3. `eos/rho_const.rs` — trivial incompressible EOS
4. `thermo/traits.rs` + `thermo/h_const.rs` — Newton iteration + simplest thermo
5. `thermo/janaf.rs` — the workhorse; most test coverage needed here
6. `transport/traits.rs` + `transport/const_transport.rs` — constant mu/Pr
7. `transport/sutherland.rs` — standard compressible air transport
8. `eos/ico_polynomial.rs` — exercises `Polynomial<N>`
9. `thermo/h_polynomial.rs` — exercises `Polynomial<N>` for Cp
10. `transport/polynomial.rs` — polynomial mu + kappa
11. `transport/tabulated.rs` + `thermo/h_tabulated.rs` — exercises `interpolate_xy`
12. `eos/peng_robinson.rs` — real-gas EOS; most complex, uses `CubicEqn`

#### Testing strategy

Each EOS, thermo, and transport struct gets unit tests that verify:
- A known point vs NIST or textbook values (e.g. air at 300 K, 1 atm)
- Self-consistency: `t_from_ha(ha(p, T), p, T) ≈ T` (roundtrip)
- Mixture blending: `(a += b)` conserves mole fractions

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

### Layer 4 — Thermophysical Models (future `openfoam-thermo` crate)

> **Note:** This layer requires `volScalarField`, `fvMesh`, and field correction
> loops — it belongs in a future **`openfoam-thermo`** crate that depends on
> this one plus the future `openfoam-fields` crate. The per-species kernels
> (EOS, Cp, transport) live here in Layer 1h above.

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
  thermophysics/           ← Layer 1h: specie-level thermophysics (mesh-independent)
    mod.rs
    constants.rs           ← R_UNIVERSAL, T_MIN, T_MAX, P_REF
    eos/
      mod.rs
      traits.rs            ← trait EquationOfState
      perfect_gas.rs       ← struct PerfectGas { mol_weight }
      rho_const.rs         ← struct RhoConst { mol_weight, rho0 }
      ico_polynomial.rs    ← struct IcoPolynomial<const N> { mol_weight, poly }
      peng_robinson.rs     ← struct PengRobinsonGas { mol_weight, tc, pc, omega }
    thermo/
      mod.rs
      traits.rs            ← trait ThermoModel; default Newton t_from_ha/hs/e
      h_const.rs           ← struct HConstThermo<E: EquationOfState>
      janaf.rs             ← struct JanafThermo<E: EquationOfState>
      h_polynomial.rs      ← struct HPolynomialThermo<E, const N>
      h_tabulated.rs       ← struct HTabulatedThermo<E>
    transport/
      mod.rs
      traits.rs            ← trait TransportModel; default alpha_h
      const_transport.rs   ← struct ConstTransport<T: ThermoModel>
      sutherland.rs        ← struct SutherlandTransport<T: ThermoModel>
      polynomial.rs        ← struct PolynomialTransport<T, const N>
      tabulated.rs         ← struct TabulatedTransport<T>
```

Future crates (not in this one):
- `openfoam-fields` — Field, GeometricField, volScalarField, fvMesh
- `openfoam-fv` — fvm:: / fvc:: operators, fvMatrix
- `openfoam-thermo` — fluidThermo / psiThermo / rhoThermo (field-level; wraps Layer 1h types over vol fields)
- `openfoam-solvers` — PIMPLE loop, rhoPimpleFoam, sonicFoam

---

## Translation notes

- OpenFOAM's primitives are templated over `Cmpt` (component type). In Rust
  we target `f64` (`scalar = double` in OpenFOAM) directly for now. Generics
  can be added later if needed.
- OpenFOAM uses expression-template `tmp<T>` to avoid heap copies.
  In Rust this maps to returning values by move (zero-cost in most cases).
- **⚠ Never build a field's `name` String compositionally inside an arithmetic
  operator.** `Field`/`VolField`/`SurfaceField`'s `Add`/`Sub`/`Neg`/`Mul` must
  leave `self.name` as the left operand's name — they must **not** do
  `self.name = format!("({} + {})", self.name, rhs.name)`. Rationale:
  - A solver repeatedly reassigns a persistent field from an expression that
    contains that same field, e.g. `rho = rho + div(phi)` where `phi`'s name in
    turn embeds `interpolate(rho)`. With compositional naming the `name` string
    then **doubles in length every timestep** — pure `2^step` growth that is
    invisible in the field *data* (internal/boundary Vecs stay the right size)
    but blows the process up to tens of GB within ~25 steps and makes each step
    ~2× slower than the last. This exact bug hung the rhoPimpleFoam
    `compressible_lid_cavity` test (24 GB, SIGTERM) — see
    `openfoam-appbuilder-lib`. Diagnosed by printing `field.name.len()` per
    step; the fix is in `fields/vol_field.rs` and `fields/surface_field.rs`.
  - This also matches OpenFOAM semantics: a `GeometricField` carries a fixed
    registered name (its `IOobject` name, e.g. `"rho"`). Arithmetic produces
    `tmp` results whose names are decorative and transient; the solver
    **prints/logs** them (e.g. residual lines keyed by `"rho"`, `"Ux"`) rather
    than accumulating a giant expression string onto the persistent field. The
    name is a label for I/O and diagnostics, not an audit trail — keep it short
    and stable.
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
