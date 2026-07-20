# outram-foam-appbuilder-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> **This is OUTRAM PARK's independent Rust translation of selected OpenFOAM®
> algorithms.** It is not the official OpenFOAM® software and is not
> affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. or the ESI
> Group. OpenFOAM® is a registered trademark of OpenCFD Limited — see
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`OpenFOAM/OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev),
> `master` branch — no commit is pinned (translation was done by reading the
> C++ source directly, not from an ongoing codegen-from-clone pipeline); see
> `upstream_source/README.md` for the full provenance record. The `genfoam`
> subtree is a separate translation of
> [GeN-Foam](https://gitlab.com/foam-for-nuclear/GeN-Foam) (Generalized Nuclear
> Foam) at upstream commit `652b3da`, GPL-3.0 — see below.

Solver application layer for the **OUTRAM PARK** OpenFOAM-in-Rust stack.
Provides solver time loops, polyMesh / field I/O, and case-file structures, and
hosts the in-progress Rust port of GeN-Foam (deterministic reactor neutronics +
thermal-hydraulics + thermo-mechanics).

Depends on (both **in-workspace path crates**, not yet on crates.io):
- `outram-foam-basic-lib` — primitives, FV operators, fields, mesh
- `outram-foam-turbulence-lib` — turbulence model closures

> **This is an early (0.1.0), in-progress crate.** Large parts of the intended
> surface are scaffolds, `todo!()`, or deliberately deferred. Read the
> [**Limitations**](#limitations) section below before depending on it — it is
> the honest status, and it is long on purpose.

## OpenFOAM solvers (`solvers`)

| Solver | Status |
|---|---|
| `pimple_foam` | Incompressible transient PISO/PIMPLE (pimpleFoam ≡ icoFoam at `nOuterCorrectors 1`). Implemented; validated against an icoFoam lid-driven-cavity reference (`tutorials/pimple_foam_cavity.rs`). |
| `rho_central_foam` | Kurganov-Tadmor central-upwind explicit compressible (rhoCentralFoam). Implemented; **validated** against Sod (1978) Table II and an OpenFOAM reference run (`tests/sod_shock_tube_validation/`, `tutorials/rho_central_foam_shock_tube.rs`). |
| `rho_pimple_foam` | Compressible transient PIMPLE (rhoPimpleFoam), ideal-gas `ρ = ψ·p` closure. Implemented; exercised by a subsonic NACA 0012 tutorial (`tutorials/rho_pimple_foam_aerofoil_naca0012.rs`). |
| `sonic_foam` | Transonic/supersonic ψ-based solver (sonicFoam). Implemented, but the implicit `fvm::div` scalar-convection operator is absent from basic-lib, so convection is treated **explicitly** via `fvc::div`. **No tutorial or validation case — unexercised.** |
| `hrm_foam` | Homogeneous Relaxation Model two-phase (HRMFoam), Downar-Zapolski (1996) relaxation. Implemented. **No tutorial or validation case — unexercised.** |

## Case I/O (`io`)

| Module | Status |
|---|---|
| `io::poly_mesh` | polyMesh reader (points, faces, cells, boundary). **Implemented** (purpose-built ASCII parser). |
| `io::field_reader` | volScalarField / volVectorField reader (`uniform` + `nonuniform List`). **Implemented.** |
| `io::control_dict` | controlDict struct + `Default`. `ControlDict::read` from disk is **`todo!()` — not implemented** (construct programmatically). |
| `io::fv_schemes` | fvSchemes struct. `FvSchemes::read` is **`todo!()` — not implemented.** |
| `io::fv_solution` | fvSolution struct. `FvSolution::read` is **`todo!()` — not implemented.** |
| `io::output` | `write_scalar_field`, `write_vector_field`, `write_vtk` are all **`todo!()` — no ASCII/VTK field output yet.** |

## GeN-Foam port (`genfoam`)

The in-workspace home for the Rust port of
[GeN-Foam](https://gitlab.com/foam-for-nuclear/GeN-Foam) (upstream commit
`652b3da`, GPL-3.0), an OpenFOAM-based reactor-multiphysics solver. GeN-Foam's
neutronics is deterministic and self-contained here — it does **not** depend on
the NJOY / Monte Carlo data crates; cross sections are the GeN-Foam
`nuclearData` group-constant format, not NJOY output. Upstream is ~88k LOC of
physics; this is an incremental, multi-session port. The module map and
translation order are in
[`docs/genfoam-port-plan.md`](./docs/genfoam-port-plan.md).

| `genfoam` module | Status |
|---|---|
| `neutronics::point_kinetics` | **Implemented + verified** — 0-D point-kinetics ODE (backward-Euler), verified against the analytical inhour equation (~0.007 % on asymptotic period; `tests/genfoam_point_kinetics_inhour.rs`). Feedback / GEM / control-rod-driveline / FMU / liquid-fuel-advection coupling **deferred**. |
| `neutronics::xs` | **Implemented** — GeN-Foam-format multigroup cross-section data structures + unit tests. |
| `neutronics::diffusion` | **Implemented + verified** — multigroup diffusion, k-eigenvalue (power iteration) + backward-Euler transient; verified against closed-form one-group theory (`k_inf` to `~4e-16`, bare-slab buckling +0.3 pcm, mesh convergence, null transient). |
| `neutronics::sp3`, `neutronics::sn` | **Scaffold only** — state/field allocation exists, but `solve_eigenvalue` / `step` return `NeutronicsError::ModelNotImplemented`. No transport solve. |
| `multi_region` | **Implemented + verified end-to-end** for 0-D and mesh-based diffusion↔TH coupling with Doppler feedback. Two **degraded scaffolds** remain (see Limitations): exact conservative mesh-to-mesh mapping, and actual mesh-point motion. |
| `thermal_hydraulics::closures` | Correlation leaves ported with unit tests against published values: `fs_drag` (**verified**, analytic `f·Re → 64`), `ff_drag`, `heat_transfer`, `phase_change`, `interfacial`. `turbulence` is **partial** (closure algebra only; the k/ε transport equations and `correctNut` orchestration are deferred). |
| `thermal_hydraulics::phase` / `structure` | Field-state + structure/power-model kernels **implemented** (with tests). |
| `thermal_hydraulics::solver` | `one_phase` porous UEqn/pEqn/EEqn driver **implemented** — but a **constant-fluid-property slice only** (`he = Cp·T`, fixed-surface-temperature structure coupling). `onePhaseLegacy` and the **two-phase (MULES) solver are not implemented.** |
| `thermal_hydraulics::thermophysical` | Bespoke **hydrogen** (H/H₂) property package implemented; not yet wired as the `one_phase` solver's fluid package. Other fluids come from basic-lib thermo / tampines-steam-tables. |
| `thermal_hydraulics::boundary_conditions` | `blackbody_radiation`, `velocity_rundown`, `time_field_table` implemented; **`nusselt_baffle` is a scaffold — every method is `unimplemented!()`.** |
| `thermal_hydraulics::function_objects` | Post-processing diagnostics (mass flow, pressure drop, T-bulk, field diffs) implemented. |
| `thermo_mechanics` | Linear-elastic thermal-expansion feedback: material card, Hooke's-law stress, axial-expansion feedback, and a displacement/heat field solve on the mechanics mesh — **implemented**. |
| `common` | `time_profile`, `interpolate_table`, RBF kernel — implemented. `latticeMap` / `mergeOrSplitBaffles` deferred. |

## Limitations

This crate is at an early stage of a large port. The following are **known,
real limitations** as of version 0.1.0 — grounded in the code, not aspirational.

### Verification & validation

- **Unverified until validated (see banner).** Only the cases with an explicit
  V&V test are trusted; everything else is untrusted draft. The currently
  V&V'd slices are: `rho_central_foam` (Sod shock tube), `pimple_foam`
  (lid-driven cavity vs icoFoam), `genfoam::neutronics::point_kinetics`
  (inhour), `genfoam::neutronics::diffusion` (analytical one-group theory),
  `genfoam::thermal_hydraulics::closures::fs_drag` (analytic `f·Re → 64` +
  published turbulent values), and the `genfoam::multi_region` coupling loop.
- **Not for reactor operation, control, licensing, or any safety-critical or
  operational use** — education / research / capability-building / V&V only.
- **Correlation leaves are unit-tested, not system-validated.** The TH closure
  families (`heat_transfer`, `phase_change`, `interfacial`, `ff_drag`,
  `turbulence`) carry unit tests against individual published correlation
  values, but they are **not exercised inside a converged multiphysics run**
  and are not validated as a coupled system.

### OpenFOAM solvers

- `sonic_foam` and `hrm_foam` are **implemented but have no tutorial or
  validation case** — they are unexercised and should be treated as unverified.
- `sonic_foam` uses **explicit** convection (`fvc::div`) because basic-lib has
  no implicit `fvm::div` scalar-convection operator; expect the accompanying
  stability/CFL constraints of an explicit scheme.
- Only `rho_central_foam` is validated against an analytical/published
  reference (Sod). `pimple_foam` is validated against an OpenFOAM (icoFoam)
  reference; `rho_pimple_foam` has a tutorial but no hard-asserted V&V gate.

### Case I/O

- **No OpenFOAM dictionary parsing.** `ControlDict::read`, `FvSchemes::read`,
  and `FvSolution::read` are `todo!()`. Cases must be configured by
  constructing these structs in Rust (`Default` + field assignment), not by
  reading `system/controlDict` etc. from disk.
- **No field output.** `io::output::{write_scalar_field, write_vector_field,
  write_vtk}` are `todo!()` — the library reads meshes/fields but cannot yet
  write OpenFOAM ASCII fields or export VTK for post-processing.

### GeN-Foam neutronics

- **SP3 and SN transport do not solve.** They allocate state but return
  `NeutronicsError::ModelNotImplemented` from `solve_eigenvalue` / `step`. The
  only working spatial neutronics is multigroup **diffusion**; the only working
  0-D model is **point kinetics**.
- **adjointDiffusion and albedoSP3 are not ported.**
- **Point kinetics is the bare 0-D reactivity-driven ODE.** The full GeN-Foam
  `pointKineticNeutronics` couplings — temperature/density feedback fields,
  GEM and control-rod-driveline reactivity, external-source power modulation,
  FMU inputs, and the liquid-fuel precursor-advection variant — are deferred.
- **Cross sections are not read from disk.** The `xs` data structures exist and
  are unit-tested, but GeN-Foam `nuclearData` dictionary-file parsing is not
  wired through `io`.

### GeN-Foam thermal-hydraulics

- **The two-phase solver is not implemented.** Only `solver::one_phase` exists,
  and only as a **constant-fluid-property** slice (`he = Cp·T`, isotropic drag,
  fixed-surface-temperature structure coupling). `onePhaseLegacy` and the
  MULES-limited two-phase `alpha`/pressure driver are absent.
- **The porous fluid thermophysical package is not wired into the solver.** The
  one-phase driver runs on constant properties supplied on the fields; the
  hydrogen property package exists standalone but is not yet the solver's EOS.
- **`turbulence` closures are partial.** Only the porous / two-phase-specific
  closure *algebra* is ported. The k/ε transport equations, `correctNut`
  field-level orchestration, per-region dictionary painting, and phase-averaging
  are deferred (the generic single-phase k/ε machinery lives in
  `outram-foam-turbulence-lib`).
- **`nusselt_baffle` boundary condition is a stub** — every method is
  `unimplemented!()` (cross-patch implicit coupling not yet supported).
- The vast majority of upstream `thermalHydraulics` (~65k LOC) remains unported;
  what exists here is the closure/field/one-phase-driver foundation.

### GeN-Foam multi-region coupling

- **Mesh-to-mesh mapping is not exactly conservative.** The `imCellVolumeWeight`
  path uses nearest-cell addressing plus a global integral rescale (globally
  conservative, not the exact local polyhedral-overlap distribution) because
  basic-lib exposes no supermesh-intersection operator.
- **Mesh motion is plumbed but not applied.** The `meshDisp` displacement field
  is exchanged through the coupling, but `deformMesh` / `movePoints` are not
  performed — basic-lib does not expose mutable mesh-point geometry, so points
  do not actually move.
- **Cross-section feedback is mesh-mean, not per-cell.** Doppler feedback uses a
  domain-mean temperature into the XS; per-cell XS feedback awaits a neutronics
  API addition.

### Scope boundaries (by design, not defects)

- The `genfoam` neutronics is **deterministic and self-contained** — it does
  **not** call `njoy-outram-park-fork` (nuclear data) or `outram-mc-libs`
  (Monte Carlo). Cross sections are the GeN-Foam group-constant format.
- Generic FV building blocks (tensors, mesh, fields, `fvm`/`fvc` operators,
  linear solvers) are **not** re-implemented here — they come from
  `outram-foam-basic-lib`, and single-phase turbulence from
  `outram-foam-turbulence-lib`.
- The crate depends on those two **in-workspace path crates**, which are not yet
  published to crates.io; a standalone crates.io build requires them to be
  published (or vendored) first.

### Documentation caveat

- Some module-level `//!` status notes and `docs/genfoam-port-plan.md` predate
  the most recent GeN-Foam commits and can **understate** what is now
  implemented (e.g. several TH closure/solver modules described as "scaffold" in
  older module headers now carry real code). This README's tables reflect the
  current code; where a module doc and this README disagree, trust the code and
  this README.

## License

GPL-3.0-only (follows OpenFOAM and GeN-Foam licensing).
