# GeN-Foam → Rust port plan

Status: **living document.** First revision 2026-07-15.

This is the module map and recommended translation order for porting
[GeN-Foam](https://gitlab.com/foam-for-nuclear/GeN-Foam) (commit `652b3da`,
GPL-3.0) into `outram-foam-appbuilder-lib`. GeN-Foam is a large multiphysics
reactor solver (~88k LOC of physics under `src/classes/`). This is a
multi-session effort; the plan states dependency order so each slice can be
built and verified independently.

## Ground rules

- The port lives under a **new `src/genfoam/` subtree**, cleanly separated from
  the existing hand-written OpenFOAM solver ports (`src/solvers/`) and case I/O
  (`src/io/`).
- Reuse `outram-foam-basic-lib` (Layers 1–4: tensors, `FvMesh`, fields,
  `SquareMatrix`, FV operators `fvm`/`fvc`, ODE solvers) and
  `outram-foam-turbulence-lib`. **Do not re-port primitives.**
- **Do not** reach into `njoy-outram-park-fork` or `outram-mc-libs`. GeN-Foam's
  neutronics is deterministic (diffusion/SP3/SN/point-kinetics) and self-contained
  here; cross sections are read from GeN-Foam-format `nuclearData` dictionaries,
  not from the NJOY/MC data crates.
- uom on public signatures; enum dispatch (no `dyn`); no `Box`/lifetimes; doc
  comments on every public item; 1000-line file cap; GPLv3 provenance headers.

## GeN-Foam source layout (`src/classes/`) and LOC

| GeN-Foam module | LOC | Role |
|---|---:|---|
| `common/` | 3.7k | InterpolateTable, latticeMap, listOperation, mergeOrSplitBaffles, radialBasisFunctionInterpolation (RBF mesh-to-mesh), a linear `solver` helper, `timeProfile` (time-dependent tabulated inputs) |
| `neutronics/XS/` | 2.7k | Cross-section data structures (`nuclearDataOneEnergy`, group XS containers) |
| `neutronics/pointKinetics/` | 1.8k | Point-kinetics (0-D) neutronics — solid & liquid fuel |
| `neutronics/diffusion/` | 1.2k | Multigroup diffusion |
| `neutronics/SP3/` | 1.1k | SP3 (simplified P3) transport |
| `neutronics/SN/` | 1.3k | Discrete-ordinates (SN) transport |
| `neutronics/adjointDiffusion/` | 0.8k | Adjoint diffusion (for perturbation/feedback weighting) |
| `neutronics/albedoSP3/` | 0.6k | Albedo boundary condition for SP3 |
| `neutronics/neutronics.{C,H}` | 0.5k | Abstract `neutronics` base class (common flux/power/precursor state, run-time selection) |
| `multiRegion/` | 2.4k | Multi-mesh region coupling (maps fields between neutronics / TH / TM meshes) |
| `thermalHydraulics/` | 65.5k | The bulk: 1-phase & 2-phase TH, phase models, physics models, boundary conditions, custom PIMPLE control, thermophysical properties |
| `thermoMechanics/` | 3.1k | Thermal-expansion / mechanical mesh displacement feedback |
| `offbeat/`, `openFoamImportedSolvers/` | — | Glue to imported OpenFOAM solvers |

## Target Rust module map (`src/genfoam/`)

| GeN-Foam | → Rust module | Notes / basic-lib reuse |
|---|---|---|
| `common/timeProfile` | `genfoam::common::time_profile` | Time-tabulated scalar inputs. Reuse basic-lib `interpolate_xy`. New. |
| `common/InterpolateTable` | `genfoam::common::interpolate_table` | Reuse basic-lib `interpolation`. Thin wrapper. |
| `common/listOperation`, `common/solver` | (fold into callers) | `solver` is a dense linear solve → **use basic-lib `SquareMatrix::solve`**. |
| `common/latticeMap`, `mergeOrSplitBaffles`, `radialBasisFunctionInterpolation` | `genfoam::common::*` (deferred) | Mesh-topology helpers; needed only by multiRegion. Genuinely new. |
| `neutronics/XS` | `genfoam::neutronics::xs` | Group-XS data structures. New (GeN-Foam `nuclearData` format, **not** njoy). |
| `neutronics/neutronics.{C,H}` | `genfoam::neutronics` (base state) | Common flux/power/precursor/powerDensity state. Model set = **enum** `NeutronicsModel`, not a `dyn` base class. |
| `neutronics/pointKinetics` | `genfoam::neutronics::point_kinetics` | **First slice.** Core 0-D PK ODE reuses basic-lib `SquareMatrix`. Feedback/GEM/FMU/liquid-fuel field coupling deferred. |
| `neutronics/diffusion` | `genfoam::neutronics::diffusion` | Multigroup diffusion; reuse basic-lib `fvm`/`fvc`, `FvMatrix`, `FvMesh`. |
| `neutronics/SP3`, `albedoSP3` | `genfoam::neutronics::sp3` | Builds on diffusion machinery. |
| `neutronics/SN` | `genfoam::neutronics::sn` | Discrete ordinates; new angular quadrature. |
| `neutronics/adjointDiffusion` | `genfoam::neutronics::adjoint_diffusion` | After diffusion. |
| `multiRegion` | `genfoam::multi_region` | Cross-mesh field mapping. Reuse basic-lib `RegionInterface`. |
| `thermalHydraulics` (1-phase core) | `genfoam::thermal_hydraulics` | The bulk — port incrementally, single-phase first. Reuse basic-lib fluid thermo + turbulence-lib closures + existing `src/solvers` PIMPLE machinery. |
| `thermoMechanics` | `genfoam::thermo_mechanics` | Mesh-displacement feedback. After TH. |

## Recommended translation order (dependencies first)

1. **`neutronics/pointKinetics` core ODE** ✅ *this run (first verified slice).*
   Self-contained; verifiable against the analytical inhour equation. Does
   **not** need XS, meshes, or TH — the 0-D ODE stands alone.
2. **`common/timeProfile` + `InterpolateTable`** — small, feed point-kinetics'
   time-dependent external reactivity / source / boron inputs and everything else.
3. **`neutronics/XS`** — cross-section data structures; prerequisite for all
   spatial (mesh-based) neutronics.
4. **`neutronics` base state** — shared flux/power/precursor/powerDensity fields
   + the `NeutronicsModel` dispatch enum.
5. **`neutronics/diffusion`** — first mesh-based neutronics; establishes the FV
   assembly pattern reused by SP3/SN.
6. **`neutronics/SP3` + `albedoSP3`**, then **`SN`**, then **`adjointDiffusion`**.
7. **`multiRegion`** coupling — needs at least one neutronics model + one TH
   model to couple.
8. **`thermalHydraulics`** (65.5k LOC) — the bulk; single-phase first, then
   two-phase. Long multi-session effort of its own.
9. **`thermoMechanics`** — expansion feedback; couples back into point-kinetics'
   `coeffTStructMech` term and diffusion XS.

## What maps onto existing crates vs. genuinely new

**Reused (do not re-port):**
- Dense linear solve → basic-lib `SquareMatrix` (Crout LU). GeN-Foam's PK uses
  `Foam::scalarSquareMatrix` + `solve()`; this is a direct match.
- FV assembly (`fvm::laplacian`, `fvm::ddt`, `fvc::*`), `FvMatrix`, `FvMesh`,
  fields (`VolScalarField` etc.), `RegionInterface` → basic-lib.
- Interpolation (`interpolate_xy`, spline) → basic-lib.
- Turbulence closures for TH → turbulence-lib.
- PISO/PIMPLE loop scaffolding → this crate's existing `src/solvers/`.

**Genuinely new (no upstream-Rust equivalent yet):**
- Point-kinetics ODE state machine and its feedback/reactivity model.
- GeN-Foam `nuclearData` cross-section format + group-XS containers.
- SP3 / SN transport operators and angular quadrature.
- Multi-region field mapping specific to GeN-Foam's neutronics⟂TH⟂TM mesh split.
- RBF mesh-to-mesh interpolation, GEM / control-rod-driveline reactivity models.

## Scope note (honesty)

Only step 1 (point-kinetics core ODE) is translated this run. Everything below it
is planned, not implemented. The full `pointKineticNeutronics` class in GeN-Foam
additionally couples to fvMesh feedback fields (fuel/clad/coolant/structure
temperatures, densities), GEM and control-rod-driveline reactivity, an external
neutron source with power-monitoring modulation, FMU inputs, and a liquid-fuel
precursor-advection variant — all of which need the mesh/TH/multiRegion layers
above and are **deferred**. The verified slice is the reactivity-driven 0-D ODE,
which is the physics core those layers feed.
