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

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

Depends on (both **in-workspace path crates**, not yet on crates.io):
- `outram-foam-basic-lib` — primitives, FV operators, fields, mesh
- `outram-foam-turbulence-lib` — turbulence model closures

> **This is an early (0.1.1), in-progress crate.** Large parts of the intended
> surface are scaffolds, `todo!()`, or deliberately deferred. Read the
> [**Limitations**](#limitations) section below before depending on it — it is
> the honest status, and it is long on purpose.

## OpenFOAM solvers (`solvers`)

| Solver | Status |
|---|---|
| `pimple_foam` | Incompressible transient PISO/PIMPLE (pimpleFoam ≡ icoFoam at `nOuterCorrectors 1`). Implemented; validated against an icoFoam lid-driven-cavity reference (`tutorials/pimple_foam_cavity.rs`). |
| `melt_foam` | Incompressible buoyant PIMPLE with phase change — pimpleFoam's PISO loop plus a temperature equation and `FvModels` wiring, so `solidificationMelting` can act on **both** the momentum and the energy equation. Not a port of any single upstream application (upstream composes this at runtime through a dictionary). Implemented; **verified** against the closed-form Stefan similarity solution (-0.035 % in integrated melt thickness) and a discrete energy-conservation check (imbalance -2.71e-4 J/m², -1.27e-5 %) in `tests/melting_vv_cases/`. The gallium cavity in the same file is a **demonstration only — no benchmark data, see its `References.md`**. |
| `rho_central_foam` | Kurganov-Noelle-Petrova (KNP) central-upwind explicit compressible (rhoCentralFoam). Implemented; **validated** against Sod (1978) Table II and an OpenFOAM reference run (`tests/sod_shock_tube_validation/`, `tutorials/rho_central_foam_shock_tube.rs`). |
| `rho_pimple_foam` | Compressible transient PIMPLE (rhoPimpleFoam), ideal-gas `ρ = ψ·p` closure. Implemented; exercised by a subsonic NACA 0012 tutorial (`tutorials/rho_pimple_foam_aerofoil_naca0012.rs`). |
| `sonic_foam` | Transonic/supersonic ψ-based solver (sonicFoam). Implemented, but the implicit `fvm::div` scalar-convection operator is absent from basic-lib, so convection is treated **explicitly** via `fvc::div`. **No tutorial or validation case — unexercised.** |
| `hrm_foam` | Homogeneous Relaxation Model two-phase (HRMFoam), Downar-Zapolski (1996) relaxation. Implemented. **No tutorial or validation case — unexercised.** |
| `reacting_two_phase_euler_foam` | Reacting two-phase Euler-Euler (OpenFOAM-dev's `multiphaseEuler`, historic `reactingTwoPhaseEulerFoam`). Composes the `outram-foam-multiphase` hydrodynamic core (`TwoFluidPimple`) and adds per-phase conservative enthalpy equations, one-resistance interfacial heat transfer (Spherical / Ranz-Marshall / constant-Nu), operator-split phase change with latent heat, an optional single-phase multicomponent composition, and a global Arrhenius reaction. Implemented; demonstrated by `examples/reacting_two_phase_euler_combustion.rs`. **Verification-tested only — no benchmark validation.** |

## Turbulence closures (`turbulence`)

`TurbulenceClosure` is the Layer-5 adapter over `outram-foam-turbulence-lib`.
Dispatch is by **enum**, never `dyn`:

| Variant | Model |
|---|---|
| `Laminar` (**default**) | no closure; ν_eff = ν |
| `KOmegaSST` | Menter (1994) k-ω SST |
| `KEpsilon` | Jones & Launder (1972) k-ε |
| `KOmega` | Wilcox k-ω |
| `SpalartAllmaras` | Spalart-Allmaras (1992) |
| `Smagorinsky` | Smagorinsky (1963) LES |

`PimpleFoam` and `RhoPimpleFoam` each carry a `turbulence` field: the momentum
stress term comes from `div_dev_reff` (ν_eff = ν + ν_t) and `correct()` runs
after the pressure correctors, matching OpenFOAM's `turbulence->correct()`
position. `RhoPimpleFoam` converts kinematic ↔ dynamic (μ_eff = μ + ρν_t,
α_eff = α + ρν_t/Pr_t) and feeds the closures the volumetric flux φ/ρ_f.

```rust,ignore
solver.turbulence = TurbulenceClosure::k_omega_sst(mesh.clone());
solver.turbulence.set_k_omega_uniform(1.0e-2, 100.0); // k [m²/s²], ω [1/s]
```

**Read the limitations below before trusting a turbulent result** — there are no
wall functions yet, and that is disqualifying for wall-bounded RAS.

## Numerical scheme selection (`solvers::schemes`)

The `ddt` and `div` entries of `FvSchemes` are honoured by `PimpleFoam`:

| Family | Implemented | Behaviour of the rest |
|---|---|---|
| `ddtSchemes` | `Euler` (default), `Backward`, `SteadyState` | `CrankNicolson`, `LocalEuler` → `AppBuilderError::UnsupportedScheme` |
| `divSchemes` | `GaussUpwind` (default), `GaussLinear` | `linearUpwind`, `vanLeer`, `MUSCL`, `limitedLinear` → `AppBuilderError::UnsupportedScheme` |

An unimplemented selection is an **error, never a silent fallback**. Measured
effect of choosing `Gauss linear` on the Re = 100 lid-driven cavity, 20×20 mesh
(`tests/fv_scheme_selection.rs`, 2026-08-07): centreline RMS error against
Ghia et al. (1982) falls from **0.0363 to 0.0224** (−38 %), peak error from
0.0634 to 0.0426.

`grad`, `laplacian`, `snGrad` and `interpolation` selections are still only
**stored** — no solver consults them. (Nothing is *parsed* from disk either:
`FvSchemes::read` is `todo!()`, so the struct is built in Rust.)

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
| `neutronics::sp3`, `neutronics::sn` | **Implemented** transport solvers — `solve_eigenvalue` (k-eigenvalue) and `step` (transient) run when the model is built with cross-section data; SN is verified to converge with quadrature order (`sn_order_convergence`) and toward the diffusion limit (`sn_approaches_diffusion_limit`). The **state-only `::new` scaffold path** (no cross sections attached) returns `NeutronicsError::ModelNotImplemented` (`scaffold_reports_not_implemented`). |
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
real limitations** as of version 0.1.1 — grounded in the code, not aspirational.

### Verification & validation

- **Unverified until validated (see banner).** Only the cases with an explicit
  V&V test are trusted; everything else is untrusted draft. The currently
  V&V'd slices are: `rho_central_foam` (Sod shock tube), `pimple_foam`
  (lid-driven cavity vs icoFoam and vs Ghia et al. 1982),
  `melt_foam` (Stefan similarity solution + a discrete energy-conservation
  check), `genfoam::neutronics::point_kinetics` (inhour),
  `genfoam::neutronics::diffusion` (analytical one-group theory),
  `genfoam::neutronics::sp3` / `::sn` (closed-form `k_inf`, angular/mesh
  convergence, and the diffusion limit),
  `genfoam::thermal_hydraulics::closures::fs_drag` (analytic `f·Re → 64` +
  published turbulent values), the `genfoam::multi_region` coupling loop, and
  the `turbulence` coupling (homogeneous k-ω decay vs its analytic solution).
  Every one of these is a **verification** result against a closed form, a
  published table, or another solver — none is a validation against
  experiment.
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
- `rho_central_foam` (Sod shock tube) and `melt_foam` (Stefan similarity
  solution) are the two solvers checked against a closed-form/published
  reference. `pimple_foam` is checked against both an OpenFOAM (icoFoam)
  reference and Ghia et al. (1982) Table I. `rho_pimple_foam` has a tutorial
  but no hard-asserted V&V gate; `sonic_foam` and `hrm_foam` have neither.

### Turbulence

- **No wall functions — this is the dominant limitation.** The closures in
  `outram-foam-turbulence-lib` use zero-gradient near-wall boundary conditions,
  so ω is never driven to its near-wall asymptote and ν_t = k/ω is unbounded
  next to a wall. Measured (`tests/turbulence_coupling.rs`, 2026-08-07): a
  Re = 100 lid-driven cavity with Wilcox k-ω develops **ν_t/ν ≈ 260–330**, which
  is physically absurd. **No wall-bounded RAS result from this stack may be
  compared with a friction correlation and called validated.**
- What *is* verified is the **coupling**: homogeneous k-ω decay driven through
  the PIMPLE loop matches the analytic law `k = k0(1 + βω0t)^(−β*/β)` — ω to
  1.8e-5 relative, k converging at first order (observed order 1.00) — and the
  momentum operator provably picks up ν_t (exact agreement with a laminar run at
  ν + ν_t). No model here has been validated against a published turbulence
  benchmark.
- The compressible path feeds the (incompressible-form) k/ω/ε transport
  equations the volumetric flux φ/ρ_f. That is the **constant-density
  approximation** to OpenFOAM's `fvm::div(alphaRhoPhi, k)`, exact only where ρ
  is uniform.

### Numerical schemes

- `DdtScheme::Backward` is wired but **not verified as second order**:
  `outram-foam-basic-lib`'s Rhie–Chow `fvc::ddt_corr` implements only the Euler
  form, while `rAU` picks up BDF2's `1.5 V/Δt` diagonal. Measured consequence:
  the Euler and Backward cavity runs converge to steady states differing by
  1.0e-2 to 2.9e-2 m/s, and the gap *grows* as Δt is refined — the signature of
  an inconsistency, not truncation error.
- `FvSchemes::default().default_div` is now `GaussUpwind`, not `GaussLinear`.
  The old value described a scheme no solver used.

### Case I/O

- **No OpenFOAM dictionary parsing.** `ControlDict::read`, `FvSchemes::read`,
  and `FvSolution::read` are `todo!()`. Cases must be configured by
  constructing these structs in Rust (`Default` + field assignment), not by
  reading `system/controlDict` etc. from disk.
- **No field output.** `io::output::{write_scalar_field, write_vector_field,
  write_vtk}` are `todo!()` — the library reads meshes/fields but cannot yet
  write OpenFOAM ASCII fields or export VTK for post-processing.

### GeN-Foam neutronics

- **SP3 and SN solve only when built with cross sections.** `Sp3Neutronics::new`
  / `SnNeutronics::new` are state-only constructors: they allocate flux state
  and every solve on them returns `NeutronicsError::ModelNotImplemented`. Use
  `with_cross_sections` for a working model. **SN has no transient** — it
  offers `solve_eigenvalue` only, unlike `diffusion` and `sp3`, which also
  offer `step`.
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

- The module-level `//!` status notes across `genfoam` were reconciled against
  the code on **2026-08-07**; the stale "scaffold" labels that previously
  understated the thermal-hydraulics, SP3/SN and thermo-mechanics subtrees have
  been corrected. `docs/genfoam-port-plan.md` still records the *translation
  order* rather than current status — read its status table, not its older
  per-run scope notes. Where any doc and this README disagree, trust the code
  and this README.

## License

GPL-3.0-only (follows OpenFOAM and GeN-Foam licensing).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
