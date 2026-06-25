# CLAUDE.md — openfoam-turbulence-lib

Pure-Rust port of the OpenFOAM turbulence model library (RAS and LES).
This crate sits **between** `openfoam-basic-lib` (Layer 1–3 primitives + FV
operators) and `openfoam-appbuilder-lib` (solver loops + I/O).

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md`
> for the shared dependency policy. All dep versions come from
> `[workspace.dependencies]` — do not pin versions locally.

---

## C++ source reference

```
/home/teddy0/Documents/research/openfoam/src/TurbulenceModels/
  turbulenceModels/   ← abstract base (ODESystem, TurbulenceModel)
  incompressible/     ← ν_t-based RAS/LES for incompressible solvers
  compressible/       ← μ_t-based RAS/LES for compressible solvers
  phaseIncompressible/
  phaseCompressible/
  RAS/
    kEpsilon/
    kOmega/
    kOmegaSST/
    SpalartAllmaras/
    realizableKE/
    SSGRSM/           ← Reynolds-Stress Model
  LES/
    Smagorinsky/
    WALE/
    dynamicKEqn/
```

---

## Crate dependency position

```
openfoam-basic-lib  (primitives, FV ops, fields, mesh)
        ↓
openfoam-turbulence-lib   ← THIS CRATE
        ↓
openfoam-appbuilder-lib  (solver loops, I/O)
```

Layer 5 solver logic (PIMPLE/PISO loops, time loops) is intentionally
**excluded** from this crate — it belongs in `openfoam-appbuilder-lib`.

---

## Core trait

```rust
/// Mirrors Foam::compressible::turbulenceModel (and the incompressible variant).
pub trait TurbulenceModel {
    /// Assemble the turbulent deviatoric stress divergence term:
    ///   ∇·(−2 μ_eff · dev(symm(∇U)))
    /// Returns an FvVectorMatrix to add to the momentum equation.
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;

    /// Update turbulence fields (k, ε/ω, ν_t/μ_t) after each time step.
    fn correct(&mut self);

    /// Effective dynamic viscosity: μ_eff = μ + μ_t
    fn mu_eff(&self, p: Pressure, t: ThermodynamicTemperature) -> DynamicViscosity;

    /// Turbulent kinematic viscosity ν_t (incompressible) or μ_t/ρ (compressible)
    fn nu_t(&self) -> &VolScalarField;

    /// Effective thermal diffusivity: α_eff = α + α_t (= κ_eff / Cp)
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField;
}
```

---

## Planned modules

| Module | C++ source | Notes |
|---|---|---|
| `laminar` | `RAS/laminar/` | No-op — zero turbulent stresses; μ_eff = μ |
| `k_epsilon` | `RAS/kEpsilon/` | Two-equation k-ε; Jones & Launder 1972 |
| `k_omega` | `RAS/kOmega/` | Two-equation k-ω; Wilcox 1988 |
| `k_omega_sst` | `RAS/kOmegaSST/` | Menter 1994; default for wall-bounded flows |
| `spalart_allmaras` | `RAS/SpalartAllmaras/` | One-equation; aerospace use |
| `les_smagorinsky` | `LES/Smagorinsky/` | Smagorinsky sub-grid model |
| `wall_functions` | `RAS/derivedFvPatchFields/` | nutWallFunction, kqRWallFunction, epsilonWallFunction, omegaWallFunction |

**Implementation order:** `laminar` → `k_omega_sst` (most used in OUTRAM PARK
solver targets) → `k_epsilon` → `spalart_allmaras` → LES.

---

## Key transport fields

| Field | Symbol | Compressible unit | Incompressible unit |
|---|---|---|---|
| Turbulent kinetic energy | k | J/kg = m²/s² | m²/s² |
| Turbulent dissipation rate | ε | m²/s³ | m²/s³ |
| Specific dissipation rate | ω | 1/s | 1/s |
| Turbulent dynamic viscosity | μ_t | Pa·s = kg/(m·s) | — |
| Turbulent kinematic viscosity | ν_t | — | m²/s |

In this crate all quantities carry `uom` types at API boundaries.

---

## k-ω SST — key constants (Menter 1994)

```
σ_k1 = 0.85,  σ_k2 = 1.00
σ_ω1 = 0.50,  σ_ω2 = 0.856
β1   = 0.075, β2   = 0.0828
β*   = 0.09
κ    = 0.41   (von Kármán constant)
a1   = 0.31   (stress-limiter coefficient)
```

Blending function F1 selects k-ω in the inner boundary layer and k-ε
(transformed) in the free stream. F2 activates the Bradshaw stress-limiter.

---

## Wall functions

Wall functions replace the near-wall turbulence boundary conditions when
the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).

Key routines:
- `y_plus(y, u_tau, nu) -> f64` — dimensionless wall distance
- `u_tau(u_wall, y, nu) -> f64` — friction velocity (Newton iteration)
- `nut_wall_function(y_plus, nu) -> f64` — ν_t at the wall cell

---

## Conventions

- All public method parameters and return values use `uom` dimensioned
  quantities — no bare `f64` SI values at API boundaries.
- `correct()` is called once per time step **after** the momentum and pressure
  solves — do not call it from inside the turbulence transport equation assembly.
- Transport equations for k and ε/ω are assembled with `fvm::` operators from
  `openfoam-basic-lib`; this crate does not re-implement FV operators.

---

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p openfoam-turbulence-lib --lib
cargo test  -p openfoam-turbulence-lib --lib --release
```

No system BLAS required (depends only on `openfoam-basic-lib`, which uses
pure-Rust LU solving).
