# CLAUDE.md — outram-foam-appbuilder-lib

Solver application layer for the OUTRAM PARK OpenFOAM-in-Rust stack.
This crate provides:
1. **Solver loops** — Rust ports of pimpleFoam, rhoPimpleFoam, sonicFoam,
   rhoCentralFoam, HRMFoam, reactingTwoPhaseEulerFoam, and a
   pimpleFoam + `solidificationMelting` composition (`melt_foam`).
2. **Case I/O** — polyMesh and field readers. **The
   controlDict/fvSchemes/fvSolution parsers and every OpenFOAM/VTK writer are
   `todo!()`** — cases are configured by constructing the structs in Rust and
   results are read off the solver's public fields.
3. **Turbulence selection** — `TurbulenceClosure`, the Layer-5 adapter over
   `outram-foam-turbulence-lib`.
4. **The GeN-Foam port** (`genfoam`) — deterministic reactor neutronics,
   thermal-hydraulics, thermo-mechanics, and their multi-region coupling.

## Maturity: DECLARED MATURE (2026-09-05)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer",
and the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

- **2026-09-05 — mature.** Bar: the `rhoCentralFoam` port reproduces **Sod
  (1978) Table II** with the **exact Riemann solution (Toro, ch. 4) as the
  arbiter**, at discrete `L2` within **5% of field peak** per variable, and
  `L∞` permitted to stay O(1) at the discontinuities. Evidence class:
  **analytical / manufactured solution** (the exact Riemann solver is the
  reference, not another code), supported by cross-code comparison against an
  OpenFOAM `rhoCentralFoam` reference run in the companion tutorial case.

  Measured at declaration: 100 cells, `dt = 1e-6 s`, run to Sod's canonical
  τ = 0.2 (t = 6.3246e-3 s); `L2` norms land at **1–5% of peak**, which is the
  expected accuracy of a 2nd-order scheme on this problem. **326 tests pass,
  0 fail, 4 ignored**; the Sod validation test itself
  (`rho_central_foam_matches_sod_table_ii`) is a plain `#[test]` and runs.

  Two things make this bar unusually honest and worth preserving as the
  template for other solver crates:

  1. **`L∞` is deliberately not bounded tight.** It is dominated by the one or
     two cells straddling the shock and the contact, which a 2nd-order scheme
     necessarily smears. Demanding a small `L∞` here would be demanding the
     scheme not be what it is. Read it as "worst single cell", not as accuracy.
  2. **The exact solution, not Table II, is the arbiter.** Table II's coarse
     9-station sampling does not always resolve the local profile, so each
     station is additionally flagged for whether it is faithful. A bar written
     against the published table alone would have been measuring the table's
     sampling as much as the port.


> The `README.md` "Limitations" section is the authoritative per-module status
> and is kept current; prefer it over any summary here.

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
outram-foam-basic-lib        (Layers 1–3: primitives, fields, mesh, FV ops)
outram-foam-turbulence-lib   (Layer 4: turbulence model closures)
          ↓ ↓
outram-foam-appbuilder-lib   ← THIS CRATE  (Layer 5: solver loops + I/O)
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

### Case I/O: write purpose-built Rust parsers, never C++ interop

For OpenFOAM file readers (`polyMesh`, `controlDict`, `fvSchemes`, `fvSolution`,
field files), **write a Rust parser from scratch. Do not attempt to wrap or FFI
into OpenFOAM's own C++ reader.**

**Why:** OpenFOAM's reader (`ISstream`, `IOobject`, `IOdictionary`) is
deeply-templated C++ backed by a runtime type registry. There is no stable C API
to bind with `bindgen`, and the template depth makes generating FFI shims
impractical; no mature Rust crate wraps the format. The OpenFOAM ASCII format
(FoamFile header + comment stripping + N-element list) is simple enough that a
purpose-built ~400-line tokenizer is far less work than any interop path.

**How to apply:** if asked "can't we just pull in OpenFOAM's own parser?",
explain the interop impossibility and proceed with a Rust tokenizer. Document the
decision in a comment at the top of the parser module (see
`src/io/poly_mesh/mod.rs`).

> The same principle holds across the suite for external data formats — e.g. the
> `njoy-outram-park-fork` ENDF/ACE reader is a purpose-built Rust parser, not a
> wrap of NJOY's Fortran I/O.

## Planned modules & C++ refs (read on demand)

The OpenFOAM C++ source paths, the planned `io::` / `solvers::` module layout
(with the equations each solver assembles), and the implementation order all
live in **`docs/planned-modules.md`**.

## Conventions

- Follow the workspace `CLAUDE.md` porting workflow: update `src/prelude.rs`
  and `README.md` for every new public item.
- Boundary condition enforcement (fixedValue, zeroGradient, etc.) is handled
  inside `FvMesh` / `FvPatch` from `outram-foam-basic-lib`. This crate calls
  `.correct_boundary_conditions()` at the top of each time step, never
  re-implements BC logic.

### Time and write control — intended, not yet implemented

These two are **design targets, not descriptions of the current code.** Both
were previously written here as though already true; they are not, so do not
cite them as existing behaviour:

- **`adjustTimeStep` is not implemented.** Every solver's `run()` steps at the
  fixed `control.delta_t`; `adjust_time_step`, `max_co` and `max_delta_t` are
  carried on `ControlDict` but consulted nowhere, and there is no
  `adjust_delta_t` method. Implementing it means adding an adaptive-Δt path to
  each `run()` loop.
- **No write control is implemented,** because no writer is: every function in
  `io::output` is `todo!()`. `WriteControl::TimeStep` (every N steps) and
  `WriteControl::RunTime` (every N seconds) both need to be supported once
  field output exists.

Only `ControlDict::{start, stop, delta_t}` currently affect a run, and only
`StartControl::StartTime` / `StopControl::EndTime` do anything — the other
variants cause `run()` to take zero steps.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p outram-foam-appbuilder-lib --lib
cargo test  -p outram-foam-appbuilder-lib --lib --release
```
