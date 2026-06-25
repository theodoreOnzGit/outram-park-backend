# CLAUDE.md — openfoam-appbuilder-lib

Solver application layer for the OUTRAM PARK OpenFOAM-in-Rust stack.
This crate provides:
1. **Solver loops** — Rust ports of pimpleFoam, rhoPimpleFoam, sonicFoam,
   rhoCentralFoam, and HRMFoam.
2. **Case I/O** — polyMesh reader, controlDict/fvSchemes/fvSolution parsers,
   and OpenFOAM / VTK output writers.

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md`
> for the shared dependency policy.

---

## C++ source references

| Component | OpenFOAM source path |
|---|---|
| pimpleFoam | `applications/solvers/incompressible/pimpleFoam/` |
| rhoPimpleFoam | `applications/solvers/compressible/rhoPimpleFoam/` |
| sonicFoam | `applications/solvers/compressible/sonicFoam/` |
| rhoCentralFoam | `applications/solvers/compressible/rhoCentralFoam/` |
| HRMFoam | `../HRMFoam/` — **outside this workspace** (sibling directory) |
| polyMesh | `src/OpenFOAM/meshes/polyMesh/` |
| controlDict | `src/OpenFOAM/db/Time/controlDict` |
| fvSchemes / fvSolution | `src/finiteVolume/fvMesh/fvMeshSubsets/` + `system/` directory convention |

All OpenFOAM source files at `/home/teddy0/Documents/research/openfoam/`.

---

## Crate dependency position

```
openfoam-basic-lib        (Layers 1–3: primitives, fields, mesh, FV ops)
openfoam-turbulence-lib   (Layer 4: turbulence model closures)
          ↓ ↓
openfoam-appbuilder-lib   ← THIS CRATE  (Layer 5: solver loops + I/O)
```

**Layer 5 is where this crate lives.** PISO/PIMPLE outer loops, time
advancement, boundary condition enforcement, and file I/O all belong here,
NOT in the lower crates.

---

## Planned modules

### `io/` — Case file I/O

#### `io::poly_mesh` — polyMesh reader

Reads the `constant/polyMesh/` directory of an OpenFOAM case:

| File | Content | Rust type |
|---|---|---|
| `points` | Vertex coordinates | `Vec<Vector3>` |
| `faces` | Face → vertex index lists | `Vec<Vec<usize>>` |
| `owner` | Owner cell per face | `Vec<usize>` |
| `neighbour` | Neighbour cell per internal face | `Vec<usize>` |
| `boundary` | Patch name, type, startFace, nFaces | `Vec<BoundaryPatch>` |

Entry point: `fn read_poly_mesh(case_dir: &Path) -> Result<FvMesh, IoError>`

#### `io::control_dict` — controlDict parser

Key fields to parse:

| Key | Rust type | Notes |
|---|---|---|
| `application` | `String` | solver name |
| `startFrom` / `startTime` | `StartControl` enum | `latestTime`, `startTime`, `firstTime` |
| `stopAt` / `endTime` | `StopControl` enum | `endTime`, `writeNow`, `noWriteNow`, `nextWrite` |
| `deltaT` | `f64` | time step (seconds) |
| `writeControl` | `WriteControl` enum | `timeStep`, `runTime`, `adjustableRunTime`, … |
| `writeInterval` | `f64` | |
| `purgeWrite` | `usize` | 0 = keep all |
| `writeFormat` | `WriteFormat` enum | `ascii`, `binary` |
| `writePrecision` | `usize` | significant digits |
| `runTimeModifiable` | `bool` | re-read dict on disk change |
| `adjustTimeStep` | `bool` | Courant-number-based dt adjustment |
| `maxCo` | `f64` | max Courant number (Euler/PIMPLE) |
| `maxDeltaT` | `f64` | upper bound for adjustable dt |

#### `io::fv_schemes` — fvSchemes parser

Scheme selections used to configure which `fvm::`/`fvc::` kernel is built:

| Sub-dict | Examples |
|---|---|
| `ddtSchemes` | `Euler`, `backward`, `CrankNicolson 0.9`, `localEuler` |
| `gradSchemes` | `Gauss linear`, `leastSquares` |
| `divSchemes` | `Gauss linearUpwind grad(U)`, `Gauss upwind`, `Gauss vanLeer` |
| `laplacianSchemes` | `Gauss linear corrected`, `Gauss linear limited 0.33` |
| `interpolationSchemes` | `linear`, `upwind phi` |
| `snGradSchemes` | `corrected`, `uncorrected`, `limited 0.5` |

#### `io::fv_solution` — fvSolution parser

| Sub-dict | Keys | Notes |
|---|---|---|
| `solvers.<field>` | `solver`, `preconditioner`, `tolerance`, `relTol`, `maxIter`, `smoother` | Per-field linear solver config |
| `PIMPLE` / `PISO` | `nOuterCorrectors`, `nCorrectors`, `nNonOrthogonalCorrectors`, `consistent` | PIMPLE/PISO loop parameters |
| `relaxationFactors` | per-field `f64` under `fields` / `equations` | Under-relaxation |

Rust target: `struct FvSolution { solvers: HashMap<String, LinearSolverConfig>, pimple: PimpleControl, relaxation: RelaxationFactors }`

#### `io::output` — field and mesh output

- **OpenFOAM ASCII format** — write `p`, `U`, `T`, `k`, `omega`, etc. to
  `<timeDir>/<fieldName>` following the OpenFOAM file header convention.
- **VTK legacy format** — write `*.vtk` for ParaView post-processing.

---

### `solvers/` — Solver application loops

#### `solvers::pimple_foam` — pimpleFoam (incompressible)

Incompressible transient PIMPLE solver. Solves:
```
∂U/∂t + ∇·(UU) − ∇·(ν ∇U) = −∇p
∇·U = 0
```
PIMPLE outer loop → momentum predictor → PISO pressure correctors.
Requires: `FvMesh`, `VolVectorField U`, `VolScalarField p`, `TurbulenceModel`.

#### `solvers::rho_pimple_foam` — rhoPimpleFoam (compressible)

Compressible density-based PIMPLE. The dominant solver for subsonic/transonic
compressible flow with heat transfer. Solves continuity + momentum + energy
(h- or e-form, runtime selection) + EOS closure.

Key loop files (mirrors C++ `pEqn.H`, `UEqn.H`, `EEqn.H`, `rhoEqn.H`):
- `rho_eqn` — `∂ρ/∂t + ∇·(ρU) = 0`
- `u_eqn` — `fvm::ddt(ρ,U) + fvm::div(φ,U) + turbulence.divDevRhoReff(U) == −fvc::grad(p)`
- `e_eqn` — `fvm::ddt(ρ,he) + fvm::div(φ,he) − fvm::laplacian(α_eff,he) == dpdt + sources`
- `p_eqn` — pressure Poisson; subsonic or transonic branch

#### `solvers::sonic_foam` — sonicFoam (compressible, psi-based)

Transient solver for trans/supersonic compressible flow using ψ = ρ/p
(compressibility) as the primary thermodynamic variable. Simpler than
rhoPimpleFoam — no PIMPLE outer correctors; uses single-pass PISO.

Key equation:
```
∂(ψp)/∂t + ∇·(ψ_d p) − ∇·(ρ/A ∇p) = 0
→  fvm::ddt(psi,p) + fvm::div(phid,p) − fvm::laplacian(rhorAUf, p)
```

#### `solvers::rho_central_foam` — rhoCentralFoam (density-based explicit)

Kurganov-Tadmor central-upwind scheme for high-speed compressible flows
(Mach > ~0.3, shocks). Fully explicit — no matrix solve needed for convection.
The flux splitting is in `openfoam-basic-lib` (Kurganov-Tadmor fluxes);
this module provides the time-stepping loop and CFL-limited `deltaT` adjustment.

#### `solvers::hrm_foam` — HRMFoam (two-phase, homogeneous relaxation)

Homogeneous Relaxation Model (HRM) for flashing two-phase flow.
C++ source: `../HRMFoam/` — a sibling directory **outside** this workspace.

When porting:
1. Read the C++ source at `../HRMFoam/`.
2. The HRM transport equation is:
   `∂(ρ x)/∂t + ∇·(ρ U x) = ρ (x_eq − x) / Θ`
   where `x` = vapour quality, `x_eq` = equilibrium quality, `Θ` = relaxation time.
3. The relaxation time model: `Θ = Θ₀ · (1 − x)^a · x^b · |∂p/∂t|^c` (Downar-Zapolski).
4. Couples to TAMPINES steam tables (`tampines-steam-tables`) for `x_eq(p, h)`.
5. Near-bubble-point accuracy is limited by the same HEM artifact documented in
   `tampines-steam-tables/CLAUDE.md` — the HRM relaxation term is what corrects this.

---

## Implementation order

1. `io::poly_mesh` — needed by all solvers (can test without turbulence)
2. `io::control_dict` — needed to drive the time loop
3. `io::fv_schemes` + `io::fv_solution` — scheme/solver config
4. `solvers::pimple_foam` — simplest solver, validates the PIMPLE loop abstraction
5. `solvers::rho_pimple_foam` — main compressible target
6. `solvers::sonic_foam` — psi-based; reuses most of rhoPimpleFoam infrastructure
7. `solvers::rho_central_foam` — explicit; independent of PIMPLE
8. `io::output` — OpenFOAM + VTK writers
9. `solvers::hrm_foam` — two-phase; depends on `tampines-steam-tables`

---

## Conventions

- Follow the workspace `CLAUDE.md` porting workflow: update `src/prelude.rs`
  and `README.md` for every new public item.
- The `controlDict` time loop must honour `adjustTimeStep` — call
  `adjust_delta_t(co_max, dt_max)` at the end of each step.
- Boundary condition enforcement (fixedValue, zeroGradient, etc.) is handled
  inside `FvMesh` / `FvPatch` from `openfoam-basic-lib`. This crate calls
  `.correct_boundary_conditions()` at the top of each time step, never
  re-implements BC logic.
- `WriteControl::TimeStep` writes every N steps; `WriteControl::RunTime` writes
  every N seconds (wall time). Both must be supported.

---

## Build and test

```bash
cargo check -p openfoam-appbuilder-lib --lib
cargo test  -p openfoam-appbuilder-lib --lib
```
