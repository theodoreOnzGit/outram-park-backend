# CLAUDE.md — openfoam-basic-lib

This crate is a pure-Rust translation of the OpenFOAM C++ primitive and
finite-volume library layer, scoped to the primitives needed to implement
compressible sonic solvers equivalent to **rhoPimpleFoam** and **sonicFoam**.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openfoam/`

---

## Goal and scope

The crate climbs the OpenFOAM stack from the bottom up. Each layer depends
only on the one below it:

```
Layer 5  Solver logic       rhoPimpleFoam / sonicFoam loop
Layer 4  Thermophysics      fluidThermo / psiThermo
Layer 3  FV operators       fvm:: / fvc:: (ddt, div, grad, laplacian, …)
Layer 2  Fields + Mesh      volScalarField, fvMesh, fvMatrix
Layer 1  Primitives ← THIS CRATE  Vector3, Tensor, SymmTensor, SphericalTensor
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
  primitives/
    mod.rs          ← re-exports
    scalar.rs       ← type Scalar = f64; type Label = i64; free math fns
    vector.rs       ← struct Vector3; operators; free fns (mag, cross, outer, …)
    symm_tensor.rs  ← struct SymmTensor; trace, dev, dev2, symm(Tensor), …
    spherical_tensor.rs  ← struct SphericalTensor; into SymmTensor/Tensor
    tensor.rs       ← struct Tensor; transpose, det, inv, symm, skew, …
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
