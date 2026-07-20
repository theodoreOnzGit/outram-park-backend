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

---

# thermalHydraulics breakdown

Added 2026-07-15. This expands step 8 above. The upstream subtree
`src/classes/thermalHydraulics/` is **~65k LOC in 351 C++ files**, split into a
class library (`src/`, ~57k LOC) and top-level solver drivers (`solvers/`, ~7.3k
LOC). It is the single largest GeN-Foam module. Tracked under bead **`op-p6p.7`**;
one child bead per sub-module below.

## Upstream LOC map (measured)

| Upstream area | LOC | Files | What it is |
|---|---|---|---|
| `src/physicsModels/` | 34,727 | 231 | **The bulk** — closure correlations (drag/friction, heat transfer, phase change, interfacial area, turbulence, …), all runtime-selectable |
| `src/phaseModels/` | 15,070 | 44 | `phaseBase`, the `fluid` phase, and `structureModels` (fuel pins, power models, heat exchangers, pumps) |
| `solvers/` | 7,320 | 40 | Top-level solver drivers: `onePhase`, `onePhaseLegacy`, `twoPhase` (porous momentum/energy/pressure, MULES alpha transport) |
| `src/functionObjects/` | 3,062 | 15 | Diagnostics (massFlow, pressureDrop, TBulk, fieldDiffExtents, …) |
| `src/boundaryConditions/` | 2,415 | 8 | NusseltThermalBaffle1D, blackBodyRadiation, velocityRundown, timeFieldTable |
| `src/thermophysicalProperties/` | 1,415 | 3 | Bespoke fluid property packages (H2) |
| `src/include/` + `customPimpleControl/` | 1,541 | 10 | IOFieldField helpers; the custom PIMPLE loop control |

## Architecture (how the pieces fit)

GeN-Foam's TH is a **porous-medium two-fluid + structure** formulation. Cells
carry, simultaneously, a `fluid` phase (volume-fraction `alpha`, velocity,
enthalpy) and an unresolved `structure` (fuel pins / cladding / grid, occupying
the complementary volume). The solver drives:

- a **porous momentum** equation `UEqn` with an anisotropic drag tensor `Kd`
  (assembled from the fluid-structure friction closures) and optional
  tortuosity-modified turbulent diffusion,
- a **porous energy** equation `EEqn` coupled to the structure through the
  fluid-structure heat-transfer coefficient,
- a **pressure** equation `pEqn` (PIMPLE), and, in two-phase, a MULES-limited
  **`alpha` transport** with interfacial drag, phase change, and interfacial-area
  closures.

The `physicsModels/` correlations are the leaves: each is a small,
self-contained algebraic function of local field values (Reynolds number, void
fraction, quality, …). They are the natural first slices — pure functions with
published reference values, no mesh/solver coupling.

## Sub-module map → Rust modules → beads

All under `src/genfoam/thermal_hydraulics/`. Closure model **sets are closed
enums** (workspace no-`dyn` rule); each upstream `runTimeSelectionTable` family
becomes one enum with one variant per correlation.

| # | Rust module | Upstream | Bead | Notes |
|---|---|---|---|---|
| 1 | `units` | (dimensions used across TH) | op-p6p.7.1 | Named `uom` aliases: `ReynoldsNumber`, `DarcyFrictionFactor`, `HeatTransferCoefficient`, `HeatFlux`, `DragCoefficient`. Foundation — blocks all. |
| 2 | `phase` | `phaseModels/{phaseBase,fluid,phasePairs}` | op-p6p.7.2 | Fluid phase state (alpha, U, h, rho fields) + phase-pair Reynolds/relative-velocity. Blocks closures & solvers. |
| 3 | `structure` | `phaseModels/structureModels/**` | op-p6p.7.3 | Solid structure + `powerModels` (heatedPin, nuclearFuelPin, lumped, pebble), heatExchanger, pump, powerOff criteria. Couples to neutronics power. |
| 4 | `closures::fs_drag` | `physicsModels/dragModels/FSDragCoefficientModels/**` | op-p6p.7.4 | **Fluid-structure wall-friction factors** (Churchill, Colebrook, Rehme, ReynoldsPower, Engel, modifiedEngel, BaxiDalleDonne, NoKazimi). **← ported this run.** |
| 5 | `closures::ff_drag` | `physicsModels/dragModels/{FFDrag,twoPhaseDragMultiplier}` | op-p6p.7.5 | Fluid-fluid interfacial drag + two-phase multipliers (Wallis, SchillerNaumann, LockhartMartinelli, …). |
| 6 | `closures::heat_transfer` | `physicsModels/heatTransferModels/**` | op-p6p.7.6 | FS Nusselt / multi-regime boiling / CHF / post-CHF / TONB / suppression + FF HTC. Largest closure family. |
| 7 | `closures::phase_change` | `physicsModels/phaseChangeModels/**` | op-p6p.7.7 | Saturation models, latent-heat models, heat-driven & forced phase change. |
| 8 | `closures::interfacial` | `physicsModels/{interfacialAreaModels,fluidDiameterModels,virtualMassModels,dispersionModels,contactPartitionModels,regimeMapModels,templatedModels}` | op-p6p.7.8 | Two-phase geometry/regime closures. |
| 9 | `closures::turbulence` | `physicsModels/turbulenceModels/**` | op-p6p.7.9 | Porous / Lahey / mixture k-epsilon (two-phase turbulence on top of turbulence-lib). |
| 10 | `thermophysical` | `thermophysicalProperties/**` | op-p6p.7.10 | Bespoke property packages (H2); most fluids come from tampines-steam-tables / basic-lib thermo. |
| 11 | `solver::one_phase` | `solvers/onePhase/**` | op-p6p.7.11 | Porous UEqn/EEqn/pEqn PIMPLE driver. Needs phase + structure + fs_drag + heat_transfer. |
| 12 | `solver::two_phase` | `solvers/twoPhase/**` | op-p6p.7.12 | MULES alpha transport + two-phase pEqn. Needs one_phase + ff_drag + phase_change + interfacial + turbulence. |
| 13 | `boundary_conditions` | `boundaryConditions/**` | op-p6p.7.13 | NusseltThermalBaffle1D, blackBodyRadiation, velocityRundown, timeFieldTable. |
| 14 | `function_objects` | `functionObjects/**` | op-p6p.7.14 | Post-processing diagnostics; port last (non-physics). |

## Recommended translation order

1. **`units`** (op-p6p.7.1) — trivial, unblocks everything.
2. **`closures::fs_drag`** (op-p6p.7.4) — **done this run.** Pure algebra, exact
   analytical laminar limit (`f·Re → 64`), published turbulent values → the
   cleanest first V&V slice; no phase/mesh state needed.
3. **`phase`** (op-p6p.7.2) then **`structure`** (op-p6p.7.3) — the field-state
   backbone the solvers and remaining closures need.
4. The remaining **closures** (7.5–7.9) — each independently testable against
   published correlation values, in any order once `phase` exists.
5. **`solver::one_phase`** (7.11) — first end-to-end porous single-phase run.
6. **`solver::two_phase`** (7.12), then **BCs** (7.13) and **function objects**
   (7.14).

## What maps onto existing crates vs. genuinely new

**Reused (do not re-port):**
- Porous momentum/energy FV assembly → basic-lib `fvm`/`fvc` (`div`, `laplacian`,
  `ddt`, `Sp`/`SuSp`), `FvVectorMatrix`, `FvMesh`, fields.
- Single-phase turbulence (`nuEff`, `divDevRhoReff`) → turbulence-lib.
- PIMPLE loop scaffolding, `p_rgh`/flux reconstruction → this crate's
  `src/solvers/pimple_foam.rs`.
- Fluid thermo (rho, cp, saturation for water) → basic-lib thermo /
  tampines-steam-tables.
- Time-profile / table interpolation → `crate::genfoam::common`.

**Genuinely new (no upstream-Rust equivalent):**
- The porous **two-fluid + structure** cell model (`alpha` fluid ⟂ structure),
  the anisotropic drag tensor `Kd` assembly, tortuosity-modified diffusion.
- The full `physicsModels/` correlation zoo (friction, boiling/CHF, interfacial
  area, phase change, two-phase turbulence).
- `structureModels` fuel-pin conduction + power coupling to neutronics.
- MULES-limited `alpha` transport for the two-phase solver.

## Scope note (honesty)

This run delivers: this plan, the bead breakdown (op-p6p.7.1–.14), a compiling
scaffold of the module tree, and **one fully ported + V&V'd slice**:
`closures::fs_drag` (the fluid-structure wall-friction factor family). Everything
else in the table is scaffold-only (`// TODO(genfoam)`), i.e. **~63k of the ~65k
LOC remains**. The fs_drag slice was chosen because it is pure algebra with a
closed-form analytical check and published reference values, requiring none of
the phase/structure/mesh machinery above it.
