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

## Why this crate exists

OpenFOAM's case setup is the worst pain point for new users:

- **Input files** (`fvSolution`, `fvSchemes`, `controlDict`) are free-form
  text dictionaries — valid keys and their meaning live only in source comments
  and forum posts. A typo silently falls back to a default or crashes at runtime.
- **Meshing** requires learning `blockMesh` or `snappyHexMesh` dictionary syntax
  with no static validation.
- **wmake** means OpenFOAM cannot be embedded in another project as a library —
  you can only run it as a standalone executable with its own case directory structure.

This crate replaces those with Rust structs:
- `controlDict` fields become a `ControlDict` struct — invalid values are
  compile errors or `Result` variants, not silent runtime misbehaviours.
- `fvSchemes` / `fvSolution` become typed enums — rust-analyzer shows every
  valid scheme option on hover.
- The solver loops are normal Rust functions a downstream crate can call,
  not executables that own their own I/O convention.

**The mandatory consequence:** every public item must be navigable with rust-analyzer
alone, by a developer with no prior OpenFOAM knowledge. See the root `CLAUDE.md`
"Human interface layer" section for the full rule.

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

## Design rules (see also root CLAUDE.md)

### Shared simulation state: `Arc<RwLock<T>>`

The solver loop runs across multiple threads. Shared state follows this pattern:

```rust
// Read-only after construction — no lock needed
Arc<FvMesh>

// Fields written once per timestep, read many times during compute phase
Arc<RwLock<VolScalarField>>   // pressure, temperature, density, …
Arc<RwLock<VolVectorField>>   // velocity

// Solver configuration — read-only after startup
Arc<ControlDict>
Arc<FvSchemes>
Arc<FvSolution>
```

Use `RwLock<T>` over `Mutex<T>` — multiple threads can hold a read lock
simultaneously during the cell-loop compute phase; `Mutex` would serialise them
unnecessarily.

Do not use channels for simulation state. The timestep loop is a shared-state
pattern (compute → sync → advance), not a pipeline.

### Enum dispatch for I/O schemes and solver variants

Numerical scheme selection (ddt, grad, div, laplacian) and solver variants
(subsonic/transonic branch, h-form/e-form energy) use enums, not trait objects.

```rust
pub enum DdtScheme { Euler, Backward, CrankNicolson(f64), LocalEuler }
pub enum EnergyForm { Enthalpy, InternalEnergy }
```

No `Box<dyn Scheme>`. No lifetime parameters anywhere in this crate.

## Planned modules & C++ refs (read on demand)

The OpenFOAM C++ source paths, the planned `io::` / `solvers::` module layout
(with the equations each solver assembles), and the implementation order all
live in **`docs/planned-modules.md`**.

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

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p openfoam-appbuilder-lib --lib
cargo test  -p openfoam-appbuilder-lib --lib --release
```
