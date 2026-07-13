# outram-foam-turbulence-lib — model reference

Reference material: the OpenFOAM C++ source tree for turbulence models, the
planned module list, turbulent transport-field units, the k-ω SST constants
(Menter 1994), and wall-function routines. Consulted on demand when implementing
a model — not per-turn guidance. The crate's core trait, design rules, and
conventions live in CLAUDE.md.

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


## Key transport fields

| Field | Symbol | Compressible unit | Incompressible unit |
|---|---|---|---|
| Turbulent kinetic energy | k | J/kg = m²/s² | m²/s² |
| Turbulent dissipation rate | ε | m²/s³ | m²/s³ |
| Specific dissipation rate | ω | 1/s | 1/s |
| Turbulent dynamic viscosity | μ_t | Pa·s = kg/(m·s) | — |
| Turbulent kinematic viscosity | ν_t | — | m²/s |

In this crate all quantities carry `uom` types at API boundaries.


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


## Wall functions

Wall functions replace the near-wall turbulence boundary conditions when
the mesh is too coarse to resolve the viscous sublayer (y⁺ > ~11).

Key routines:
- `y_plus(y, u_tau, nu) -> f64` — dimensionless wall distance
- `u_tau(u_wall, y, nu) -> f64` — friction velocity (Newton iteration)
- `nut_wall_function(y_plus, nu) -> f64` — ν_t at the wall cell
