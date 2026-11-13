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
| `pimple_foam` | Incompressible transient PISO/PIMPLE (pimpleFoam ≡ icoFoam at `nOuterCorrectors 1`). Implemented; validated against an icoFoam lid-driven-cavity reference (`tutorials/pimple_foam_cavity.rs`). Open boundaries since gh:#1154; `Gauss linear corrected` non-orthogonal correction (default, ablation `without_non_orthogonal_correction`) since gh:#1161 (`verification_and_validation/non_orthogonal_correction_gh1161.md`); laminar pipe V&V (Hagen–Poiseuille, f·Re → 64.09, development length +1.4 % against Durst et al. 2005) in `verification_and_validation/laminar_pipe_gh1155.md`. **Differential against OpenFOAM-dev icoFoam (gh:#1363)**, cavity, 20×20, 100 steps, `Gauss linear`: U, p, phi per cell agree to ≤ 1.3e-13 of the field range at all of 7 snapshots (not bit-identical: different linear solvers); record `verification_and_validation/pimple_foam_vs_openfoam.md`, test `tests/upstream_cavity_gh1363.rs`. |
| `energy` | The incompressible, constant-property energy equation (gh:#1156) — `EnergyEquation` (T, `IncompressibleThermo`, Euler or steady), attached to `PimpleFoam::energy` for forced convection (solved after the PISO loop with the corrected flux; `None` by default, which leaves every flow bit-identical) or held by a solid region. Upwind convection, the flow's Laplacian (default `Gauss linear corrected`), BiCGStab with iterative refinement. Verified: developed Nusselt numbers (constant flux → 4.364, constant T → the Graetz eigenvalue incl. axial diffusion, both converging at first order) and discrete energy closure to ≤ 2e-10, in `verification_and_validation/cht_double_pipe_gh1156.md`. Compressible terms (kinetic energy, pressure work, buoyancy) are **not** carried. ~~Radiation is not carried.~~ **CHANGED 2026-11-13 (gh:#1531):** opt-in P1 thermal radiation, `EnergyEquation::with_radiation(P1Model, g0)` (`solvers::energy_radiation::RadiationCoupling`): ESI's `rad.Sh(thermo, he)` source in the energy equation and `rad.correct()` after each solve (G warm-started, `solverFreq`); off (`None`) by default. Compared with ESI v2506 `chtMultiRegionFoam` + `radiationModel P1` on the gh:#1363 case: fluid T within 1.1e-11 K, G within 1.5e-13 relative over 50 steps (`verification_and_validation/upstream_radiation_gh1531/README.md`). The radiative wall flux `qr` is not fed to the coupled interface condition. |
| `cht_multi_region_foam` | chtMultiRegionFoam's multi-region coupling (gh:#1156): fluid regions (`PimpleFoam` + energy) and `SolidRegion`s on separate meshes, joined by `CoupledTemperature`, a port of OpenFOAM-8's `compressible::turbulentTemperatureCoupledBaffleMixed` (mixed condition, `thicknessLayers`/`kappaLayers` contact layers), solved fluid-then-solid per outer pass with upstream's `frozenFlow`. Coupling on by default; ablation `without_interface_coupling`. Verified against the analytic composite slab (to ≤ 8e-9 K, the solve tolerance) and on a counterflow double pipe against ε-NTU. Since gh:#1363 row 4 also compared with ESI v2506 `chtMultiRegionFoam` (a dump driver on the real thermo and the real `turbulentTemperatureRadCoupledMixed`) on a frozen-flow fluid at rest + solid: cells within 1.1e-11 K over 50 steps, not bit-identical (T-form vs upstream's h-form); flowing, buoyant or compressible CHT is not compared. Since gh:#1531 a fluid region can carry opt-in P1 radiation, compared with ESI on the same case. |
| `non_orthogonal` | The `Gauss linear corrected` pieces `PimpleFoam` applies (gh:#1161): `LaplacianCorrection`, the explicit face corrections, the vector-matrix re-stamp to `1/(n·d)`. |
| `melt_foam` | Incompressible buoyant PIMPLE with phase change — pimpleFoam's PISO loop plus a temperature equation and `FvModels` wiring, so `solidificationMelting` can act on **both** the momentum and the energy equation. Not a port of any single upstream application (upstream composes this at runtime through a dictionary). Implemented; **verified** against the closed-form Stefan similarity solution (-0.035 % in integrated melt thickness) and a discrete energy-conservation check (imbalance -2.71e-4 J/m², -1.27e-5 %) in `tests/melting_vv_cases/`. The gallium cavity in the same file is a **demonstration only — no benchmark data, see its `References.md`**. Since gh:#1363 row 6 its energy side is also compared with ESI v2506's `solidificationMeltingSource` fvOption (in a solid's energy equation, 1-D slab, `solidus == liquidus`): T within 3.4e-11 K over 200 steps; the momentum side (Darcy drag, buoyancy) is not compared, and ESI ships no melting solver. |
| `rho_central_foam` | Kurganov-Noelle-Petrova (KNP) central-upwind explicit compressible (rhoCentralFoam). Implemented; **validated** against Sod (1978) Table II and an OpenFOAM reference run (`tests/sod_shock_tube_validation/`, `tutorials/rho_central_foam_shock_tube.rs`). **Since gh:#1363 `step` is a translation of OpenFOAM-v2506 `rhoCentralFoam.C` (inviscid) and is BIT-IDENTICAL to it on the shockTube** (rho, U, T, p, e, 100/100 cells at 15 steps up to t = 0.007 s; `verification_and_validation/rho_central_foam_vs_openfoam.md`, `tests/upstream_shock_tube_gh1363.rs`); the gas is `PerfectGas` (default γ = 1.4; `with_gas` for a `Cp`/`molWeight` case), viscous branches not ported. |
| `rho_pimple_foam` | Compressible transient PIMPLE (rhoPimpleFoam), ideal-gas `ρ = ψ·p` closure. Implemented; exercised by a subsonic NACA 0012 tutorial (`tutorials/rho_pimple_foam_aerofoil_naca0012.rs`). `rho_pimple_foam::esi::RhoPimpleFoamEsi` translates ESI v2506 `rhoPimpleFoam` (h-form, 1-D inviscid) and matches an upstream driver bit for bit. |
| `sonic_foam` | Transonic/supersonic ψ-based solver (sonicFoam). `SonicFoam` is an explicit-convection sketch (**unexercised**); `sonic_foam::esi::SonicFoamEsi` translates ESI v2506 `sonicFoam` (implicit convection, energy equation, `hePsiThermo`) and matches an upstream driver bit for bit on the laminar shockTube (1-D, inviscid, closed ends). |
| `hrm_foam` | Homogeneous Relaxation Model two-phase (HRMFoam), Downar-Zapolski (1996) relaxation. Implemented. **No tutorial or validation case — unexercised.** |
| `reacting_two_phase_euler_foam` | Reacting two-phase Euler-Euler (OpenFOAM-dev's `multiphaseEuler`, historic `reactingTwoPhaseEulerFoam`). Composes the `outram-foam-multiphase` hydrodynamic core (`TwoFluidPimple`) and adds per-phase conservative enthalpy equations, one-resistance interfacial heat transfer (Spherical / Ranz-Marshall / constant-Nu), operator-split phase change with latent heat, an optional single-phase multicomponent composition, and a global Arrhenius reaction. Implemented; demonstrated by `examples/reacting_two_phase_euler_combustion.rs`. **Verification-tested only — no benchmark validation.** |

## Ready-made cases (`cases`)

| Module | What |
|---|---|
| `double_pipe` | `DoublePipeExchanger`: a counterflow double-pipe heat exchanger as three `ChtMultiRegionFoam` regions (tube fluid, tube wall, annulus fluid) on the conformal meshes of `outram_foam_mesh::block_primitives::DoublePipe`, with the march-freeze-iterate procedure and the report (outlet temperatures, stream and interface heats, effectiveness, local UA) the V&V and the Foam Studio demo use (gh:#1156). Since gh:#1178: an independent annulus flow rate (`annulus_flow_rate`, `None` keeps C_r = 1), the axial `profiles` (per-layer interface heat, bulk and interface temperatures, local and ΔT-weighted mean Nusselt numbers on both sides) and the apparent `pressure_drop`, which `outram-park-digital-twin-engine::multifidelity::double_pipe` reads closures from. |
| `helical_coil` | `CoilFlow` (Re, d/D_c, Dean number), `CoilCells` (per-cell arc position and Frenet frame, from `outram_foam_mesh::block_primitives::HelicalPipe::locate`), `arc_pressure_gradient` (−dp/ds over an arc window) and `dean_section` (axial-peak shift, secondary speed, core outflow, vortex-pair sign): laminar flow in a helical coil on `laminar_pipe`'s boundary conditions, developing from a uniform inlet (gh:#1157; V&V in `verification_and_validation/helical_coil_dean_gh1157.md`). |
| `laminar_pipe` | `PipeFlow`: a laminar internal flow on any mesh with `inlet` / `outlet` / wall patches (uniform inlet along the inward normal, p = 0 outlet, no-slip walls) solved by `PimpleFoam`, with Hagen–Poiseuille, f = 64/Re and the Durst et al. (2005) development length as references, and the fits (centreline velocity, −dp/dz, development length, fluxes, continuity) the pipe V&V and the Foam Studio demo use (gh:#1155). |

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
| Rhie–Chow `ddtCorr` (`ddt_corr_scheme`, follows `ddtSchemes`) | `Euler`, `Backward` (BDF2 form, since 2026-11-01), `SteadyState` (zero) | as above |
| `divSchemes` | `GaussUpwind` (default), `GaussLinear` (fully implicit central, as upstream, since 2026-11-11 gh:#1363; ~~before: implicit upwind plus a lagged explicit correction~~ **CORRECTED**, that form is now the explicit non-OpenFOAM `GaussLinearDeferred`, ~~used by the Foam Studio coil runs~~ **CORRECTED 2026-11-12 (gh:#1500): `#[deprecated]`; the Foam Studio coil runs now use `GaussLinear` at Courant number 1 (the implicit scheme diverges there at Co 2 and 4)**; the converged Ghia numbers below are unchanged: RMS 0.0057, max 0.0127, re-run 2026-11-11), `GaussLinearUpwind` (since 2026-11-04, gh:#1202, OpenFOAM-dev `linearUpwind.C`) | `vanLeer`, `MUSCL`, `limitedLinear` → `AppBuilderError::UnsupportedScheme` |
| `gradSchemes`, the `grad(U)` entry (`FvSchemes::grad_u`, else `default_grad`) | `GaussLinear` (default), `CellLimitedGaussLinear(k)` (since 2026-11-04, `cellLimitedGrad<vector, minmod>`) | `leastSquares`, `fourth` → `UnsupportedScheme` |

An unimplemented selection is an **error, never a silent fallback**. Measured
effect of choosing `Gauss linear` on the Re = 100 lid-driven cavity, 20×20 mesh
(`tests/fv_scheme_selection.rs`, 2026-08-07): centreline RMS error against
Ghia et al. (1982) falls from **0.0363 to 0.0224** (−38 %), peak error from
0.0634 to 0.0426.

**RE-MEASURED 2026-11-03 (gh:#1154).** `PimpleFoam` now sets the boundary
flux of `phi` to the prescribed `U_b·Sf` on fixed-velocity patches (upstream
`constrainHbyA`); before, the moving lid carried the zero-gradient
`HbyA_P·Sf`. Every cavity number moved, toward its reference. On the 20×20
Re = 100 cavity: upwind RMS against Ghia 0.0363 → **0.0207** (max 0.0634 →
0.0367); `Gauss linear` 0.0224 → **0.0057** (max 0.0426 → 0.0127). Against
icoFoam at Re = 10, max |ΔU| / U_lid 0.0144 → **0.0062**. The earlier numbers
here are the record of the pre-fix solver.

~~`grad`, `laplacian`, `snGrad` and `interpolation` selections are still only
**stored** — no solver consults them.~~ **CORRECTED 2026-11-04:** the
`laplacian` selection is honoured by `PimpleFoam` since gh:#1161, and the
`grad(U)` selection since gh:#1202 (by `linearUpwind` and by the momentum
Laplacian's non-orthogonal correction, as upstream's `correctedSnGrad`);
`snGrad` and `interpolation` are still only **stored**. (Nothing is *parsed* from disk either:
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
| `neutronics::diffusion` | **Implemented + verified** — multigroup diffusion, k-eigenvalue (power iteration) + backward-Euler transient; verified against closed-form one-group theory (`k_inf` to `~4e-16`, bare-slab buckling +0.3 pcm, mesh convergence, null transient). `set_initial_guess` warm-starts the power iteration from a previous flux and `k` (as upstream, whose flux fields persist between coupling iterations; `warm_start_reaches_the_same_k_in_fewer_iterations`), which `dhoby-ghaut` Step 10 uses when it rebuilds the model each Picard pass (gh:#591). |
| `neutronics::sp3`, `neutronics::sn` | **Implemented** transport solvers — `solve_eigenvalue` (k-eigenvalue) and `step` (transient) run when the model is built with cross-section data; SN is verified to converge with quadrature order (`sn_order_convergence`) and toward the diffusion limit (`sn_approaches_diffusion_limit`). The **state-only `::new` scaffold path** (no cross sections attached) returns `NeutronicsError::ModelNotImplemented` (`scaffold_reports_not_implemented`). |
| `multi_region` | **Implemented + verified end-to-end** for 0-D and mesh-based diffusion↔TH coupling with Doppler feedback. Two **degraded scaffolds** remain (see Limitations): exact conservative mesh-to-mesh mapping, and actual mesh-point motion. |
| `thermal_hydraulics::closures` | Correlation leaves ported with unit tests against published values: `fs_drag` (**verified**, analytic `f·Re → 64`), `ff_drag`, `heat_transfer`, `phase_change`, `interfacial`. `turbulence` is **partial** (closure algebra only; the k/ε transport equations and `correctNut` orchestration are deferred). |
| `thermal_hydraulics::phase` / `structure` | Field-state + structure/power-model kernels **implemented** (with tests). |
| `thermal_hydraulics::solver` | `one_phase` porous UEqn/pEqn/EEqn driver **implemented** — but a **constant-fluid-property slice only** (`he = Cp·T`, fixed-surface-temperature structure coupling). **New work, not port (gh:#1149, 2026-11-03):** an optional `PorousConductivity` makes `alphaEff = λ(T)/Cp` per region (off by default; the constant path is pinned bit for bit). `onePhaseLegacy` ~~and the **two-phase (MULES) solver are not implemented.**~~ is not implemented. **`two_phase` (gh:#1218, 2026-11-04):** GeN-Foam's `twoPhase` with `implicitUpwind` alpha (MULES not ported), runs `1D_boiling`. |
| `thermal_hydraulics::solid_conduction` | **New work, not port (gh:#1149, 2026-11-03).** 2-D R-Z (OpenFOAM-style wedge) or planar solid conduction over homogenised regions: `λ(T)`, `ρc_p(T)` by enum, Picard per steady solve and per backward-Euler step (enthalpy-form storage), fixed / convective / radiative boundaries, gas gaps with two-surface radiation between regions (Kugeler et al. 2017 §9.4). **Verified** against closed forms (uniform-source cylinder, composite cylinder with a radiating gap, Kirchhoff `λ0(1+βT)`, transient slab series, 2-D manufactured solution: observed order 2.00 in space, 1.01 in time). Drawings: `verification_and_validation/solid_conduction_rz/`. ~~HTR-10 geometry is a later part of gh:#1149.~~ **UPDATED 2026-11-03:** the HTR-10 geometry is built on it by `htr10_rz` (below); held cells (`set_fixed_temperatures`, the gas side as a prescribed temperature) added and verified. **2026-11-03 (gh:#1173):** a radiative-plus-enclosure-natural-convection boundary (`SolidBoundary::RadiativeAndConvective`, `EnclosureNaturalConvection`: MacGregor & Emery 1969, air from `air_properties`, checked against CoolProp within 2 %), the HTR-10 RPV side wall's default. Not validated against any reactor. |
| `thermal_hydraulics::thermophysical` | Bespoke **hydrogen** (H/H₂) property package implemented; not yet wired as the `one_phase` solver's fluid package. Other fluids come from basic-lib thermo / tampines-steam-tables. |
| `thermal_hydraulics::boundary_conditions` | `blackbody_radiation`, `velocity_rundown`, `time_field_table` implemented; **`nusselt_baffle` is a scaffold — every method is `unimplemented!()`.** |
| `thermal_hydraulics::function_objects` | Post-processing diagnostics (mass flow, pressure drop, T-bulk, field diffs) implemented. |
| `thermo_mechanics` | Linear-elastic thermal-expansion feedback: material card, Hooke's-law stress, axial-expansion feedback, and a displacement/heat field solve on the mechanics mesh — **implemented**. |
| `common` | `time_profile`, `interpolate_table`, RBF kernel — implemented. `latticeMap` / `mergeOrSplitBaffles` deferred. |

## Pebble-bed HTGR closures (`htgr_closures`)

Original work, **not** a GeN-Foam port; it lives here so both `tampines`
(0-D/1-D) and 2-D outram-foam work can reach it (maintainer decision D2,
2026-11-03, gh:#1148). Coefficients from IAEA-TECDOC-1694 (proprietary tier:
formulas and table numbers only) and Kugeler et al. 2017 (open).

| Item | What | Status |
|---|---|---|
| `BedConductivityModel` | Bed effective conductivity. **Default `Breitbach`** (TECDOC-1694 Tab. 2.9); `Zbs(ZbsBed)` and `Kugeler2017` are comparison variants | **Verified** against hand evaluations (1e-12); default pinned by `breitbach_is_the_physics_default`. ZBS is 32-107 % above Breitbach over 523-1273 K |
| `ZbsBed` | Zehner-Bauer-Schlunder with radiation and contact, moved here from `tampines` (re-exported there) | Analytic limits verified; the VTB table it does not reproduce is analysed (`op-jvua`) |
| `kta_3102_2_nusselt`, `kta_3102_2_heat_transfer_coefficient` | KTA 3102.2 pebble-to-gas heat transfer (Tab. 2.10) | **Verified** against hand values; 10.5 % above Wakao at the HTR-10 design point |
| `kta_3102_3_friction_factor`, `kta_3102_3_pressure_gradient` | KTA 3102.3 bed pressure drop (Tab. 2.10); the one copy `tampines::gas_phase::KtaBed` calls | **Verified** against the VTB step-2 gold values |
| `KtaHelium` | The benchmark's helium fits (Tab. 2.8) with the pressure unit **fixed to bar** | **Verified** against CoolProp: within 2.2 % (0.8 % above 500 K) |
| `StructuralMaterial`, `SteelComponent` | Fuel element, side and top/bottom reflector, carbon brick (Tab. 2.9, 2.11, 2.12); steels digitised from Fig. 2.8 | **Verified** against hand values. Steels **Uncertain** (digitised). Side reflector refused below 300 deg C (the printed fit breaks down there) |

V&V record: [`verification_and_validation/htr10_th_closures_vs_tecdoc1694.md`](./verification_and_validation/htr10_th_closures_vs_tecdoc1694.md).
None of it is validated against an HTR-10 measurement.

## HTR-10 R-Z solid model (`htr10_rz`)

Original work (gh:#1149 part B). The IAEA-TECDOC-1694 THERMIX-style R-Z
solid model (Tab 2.4 regions on the Fig 2.5 mesh lines, from the cited input
sheet `docs/reactor-scoping/htr10-th-benchmark.md`) on the
`solid_conduction` field, with every property from `htgr_closures`; helium
cavities as conduction-plus-radiation media; the RPV radiating to the RCCS
at 50 deg C. ~~Solid only (no gas flow; that is gh:#1150).~~ **UPDATED
2026-11-03:** gh:#1150 adds the gas side (below); the solid-only cases are
unchanged. Cases: the bed's
volume-mean temperature held by its own power (`BedMeanTemperature`), the
bed held isothermal (`HeldBed`), a given decay power (`DecayPower`).
**Status: AI draft, verified for energy balance and mesh convergence; every
region extent is Uncertain (read from a figure); not validated.** V&V record
with drawings: [`verification_and_validation/htr10_rz_solid/README.md`](./verification_and_validation/htr10_rz_solid/README.md).

**FPIC full power with gas flow (gh:#1150, original work).** `htr10_rz::flow`
is a KONVEK-like steady R-Z porous helium flow over the Tab 2.5 flow
regions (KTA 3102.3 bed resistance, Churchill channel and slot friction,
buoyancy in the bed and channels), with the inlet path, the risers and the
bypass paths (discharge tube, control-rod channels, gap leakage) as explicit
regions and the 87/1/2/10 split **imposed** (unmeasured, D5). The gas energy
(KTA 3102.2 in the bed, Dittus-Boelter elsewhere) is coupled to the solid
field by outer iteration through the new implicit per-cell exchange
`SolidConductionField::set_cell_exchange`. `htr10_rz::pebble` adapts the
GeN-Foam `nuclearSteadyStatePebble` port to the HTR-10 full-sphere fuel zone,
and `htr10_rz::fpic` (`Htr10FpicModel`) runs the case and samples the 22
thermocouples. The GeN-Foam `onePhase` solver was checked and not used
(constant properties, isotropic drag, transient march; see the `flow`
module docs). **Status: AI draft; plant energy balance closed to 1e-6;
compared (not gated) against the 22 measured thermocouples: RMS 49.4 K
(INET 38.7 K), all 22 inside the participants' range; not validated.** V&V
record with drawings and sensitivities:
[`verification_and_validation/htr10_rz_flow_fpic/README.md`](./verification_and_validation/htr10_rz_flow_fpic/README.md).

**Transient R-Z core (gh:#1151 part B, original work).**
`htr10_rz::transient::Htr10RzTransientCore` marches the solid field by
backward Euler and solves the gas quasi-steadily each step (THERMIX's
coupling), with buoyancy in the bed and channels so that the in-vessel
natural circulation appears once the loop stops, and a lagged fuel-zone node
per heated cell eliminated exactly into the solid row
(`SolidConductionField::set_cell_capacity_factor` keeps the capacity
counted once). The power is an input (thermal-hydraulics only); the kinetics
that sets it is `nee_soon::htr10_th_transient`. Verified: energy conserved
to the Picard tolerance over a stopped-loop march; natural circulation
vanishes with buoyancy ablated (`NC-OFF`). Drawings and the coupled LOFC /
CRW validation: `crates/nee_soon/verification_and_validation/htr10_rz_transients/README.md`.
Not validated: the coupled LOFC re-criticality is ~3400 s late (see there).

## Limitations

This crate is at an early stage of a large port. The following are **known,
real limitations** as of version 0.1.1 — grounded in the code, not aspirational.

### Verification & validation

- **Open (gh:#1202, 2026-11-04): the laminar pipe on cfMesh's tet-dual mesh
  is a few per cent off Hagen–Poiseuille.** At the Foam Studio default
  (h 4 mm, 6 353 cells, `linearUpwind grad(U)` + `cellLimited Gauss linear
  1`) u_c/2U is 0.893 and dp/dz 1.033 × HP; with `Gauss linear`, 0.981 and
  0.967. The structured O-grid is within 1–2 %. Fixed on the way: the
  interpolation weights (now upstream's `makeWeights`), the chamfered rim
  (feature-edge capture in the cfMesh port; the inlet carried 6 % too
  little flow), the inward boundary faces and the 2-layer divergence.
  Still open: 3 wall layers diverge; layers do not improve τ_w or dp/dz at
  matched cell count; the remaining gap is in the core at this resolution.
  Record: `verification_and_validation/laminar_pipe_gh1155.md`.

- **Unverified until validated (see banner).** Only the cases with an explicit
  V&V test are trusted; everything else is untrusted draft. The currently
  V&V'd slices are: `rho_central_foam` (Sod shock tube), `pimple_foam`
  (lid-driven cavity vs icoFoam and vs Ghia et al. 1982),
  `melt_foam` (Stefan similarity solution + a discrete energy-conservation
  check), `energy` + `cht_multi_region_foam` (composite slab, Graetz Nusselt
  numbers, double-pipe ε-NTU and energy closure; gh:#1156),
  `genfoam::neutronics::point_kinetics` (inhour),
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

- `energy` / `cht_multi_region_foam` (gh:#1156) carry **constant
  properties and no buoyancy**: the flow is solved once per step and only the
  energy equations are iterated across regions, which is exact only because
  the flow does not depend on T. Scalar convection is first-order upwind, so
  the exchanger's effectiveness carries an O(Δz) error (measured: the gap to
  ε-NTU falls as 1/nz). Only conformal interfaces have been exercised
  (nearest-face pairing; `max_pairing_gap` checks it).
- ~~`sonic_foam` and `hrm_foam` are **implemented but have no tutorial or
  validation case** — they are unexercised and should be treated as unverified.~~
  **CORRECTED 2026-11-12 (gh:#1363, row 3b):** `hrm_foam` still has none.
  `sonic_foam::SonicFoam::step` (the sketch below) is still unexercised and
  unverified, but `sonic_foam::esi::SonicFoamEsi` is a translation of ESI
  v2506 `sonicFoam` that is **bit-identical** to a driver of the upstream
  solver on the laminar shockTube for 700 steps (`tests/upstream_sonic_gh1363.rs`,
  `verification_and_validation/sonic_foam_vs_openfoam.md`; inviscid, 1-D,
  closed `zeroGradient` ends only, other cases are refused by name).
- `sonic_foam::SonicFoam::step` uses **explicit** convection (`fvc::div`); the
  claim that basic-lib has no implicit `fvm::div` scalar convection is
  ~~still true~~ **outdated**: `fvm::div` exists (basic-lib `fvm::div`,
  `div_vec`), and `SonicFoamEsi` assembles the implicit convection matrices
  itself, with upstream's `upwind`/`limitedLinear` weights.
- `rho_central_foam` (Sod shock tube) and `melt_foam` (Stefan similarity
  solution) are the two solvers checked against a closed-form/published
  reference. `pimple_foam` is checked against both an OpenFOAM (icoFoam)
  reference and Ghia et al. (1982) Table I. `rho_pimple_foam::RhoPimpleFoam` has a tutorial
  but no hard-asserted V&V gate; ~~`sonic_foam` and `hrm_foam` have neither~~
  **CORRECTED 2026-11-12:** `rho_pimple_foam::esi::RhoPimpleFoamEsi` and
  `sonic_foam::esi::SonicFoamEsi` are each pinned bit for bit against an
  ESI v2506 driver (`verification_and_validation/rho_pimple_foam_vs_openfoam.md`,
  `sonic_foam_vs_openfoam.md`); only `hrm_foam` has no gate.

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

- `DdtScheme::Backward` is wired but **not verified as second order**.
  ~~`outram-foam-basic-lib`'s Rhie–Chow `fvc::ddt_corr` implements only the Euler
  form, while `rAU` picks up BDF2's `1.5 V/Δt` diagonal.~~ Measured consequence
  (2026-08-07): the Euler and Backward cavity runs converge to steady states
  differing by 1.0e-2 to 2.9e-2 m/s, and the gap *grows* as Δt is refined — the
  signature of an inconsistency, not truncation error. **CORRECTED 2026-11-01**
  (gh:#1067): `Backward` now pairs with its own Rhie–Chow partner
  (`fvc::ddt_corr_backward`, via `schemes::ddt_corr_scheme`; the old pairing
  survives only as the `DdtCorrForm::EulerOnly` ablation). The steady gap falls
  2.7–3.6× but still grows as Δt is refined (3.80e-3 → 8.04e-3 m/s), and a
  Richardson temporal-order study on the cavity measured order ≈ 0 for Euler
  and BDF2 alike, because the discrete fixed point depends on Δt. Record:
  `verification_and_validation/bdf2_temporal_order.md`.
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
- ~~**adjointDiffusion and albedoSP3 are not ported.**~~ **CORRECTED 2026-11-04
  (gh:#1217 audit):** `albedoSP3` is ported as `genfoam::neutronics::albedo`
  (5 tests; verified against an upstream MSFR run to 0.0 pcm on its boundary
  row). `adjointDiffusion` is still not ported (checked: no module of that
  name under `src/genfoam/neutronics/`).
- **Point kinetics is the bare 0-D reactivity-driven ODE.** The full GeN-Foam
  `pointKineticNeutronics` couplings — temperature/density feedback fields,
  GEM and control-rod-driveline reactivity, external-source power modulation,
  FMU inputs, and the liquid-fuel precursor-advection variant — are deferred.
- ~~**Cross sections are not read from disk.** The `xs` data structures exist and
  are unit-tested, but GeN-Foam `nuclearData` dictionary-file parsing is not
  wired through `io`.~~ **CORRECTED 2026-10-05:** `io::nuclear_data::read_nuclear_data`
  reads a GeN-Foam `nuclearData` dictionary (with `#include`s) into
  `NuclearDataInput`, and `io::nuclear_data::write_nuclear_data` writes one
  (round-trip test `written_nuclear_data_reads_back_identically`). Checked by
  reading `src/io/nuclear_data/`.

### GeN-Foam thermal-hydraulics

- ~~**The two-phase solver is not implemented.** Only `solver::one_phase` exists,
  and only as a **constant-fluid-property** slice (`he = Cp·T`, isotropic drag,
  fixed-surface-temperature structure coupling). `onePhaseLegacy` and the
  MULES-limited two-phase `alpha`/pressure driver are absent.~~ **CORRECTED
  2026-11-04 (gh:#1218):** `solver::two_phase` ports GeN-Foam's `twoPhase`
  solver with the `implicitUpwind` alpha solver, face-centred momentum with
  implicit partial elimination, the compressible `p_rgh` equation, both
  enthalpy equations and `heatDriven` boiling, on basic-lib's `heRhoThermo`
  stacks; it runs upstream's `1D_boiling` end to end (V&V:
  `verification_and_validation/genfoam_two_phase/README.md`). **MULES is not
  ported** (a case selecting it runs `implicitUpwind`, recorded as a
  deviation), nor `onePhaseLegacy`; `one_phase` is still the
  constant-property slice. Gap list: `docs/genfoam-port-gap-audit.md`.
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

~~GPL-3.0-only~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`). The ported files follow OpenFOAM and GeN-Foam licensing (GPL-3.0).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
