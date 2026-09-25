# Rationale: the crate roster, in full prose

> Split out of the root `CLAUDE.md` on 2026-09-21 to keep that file under the
> 150k-character context limit. **This is the full original Members table,
> verbatim**, including every per-crate status note, correction stamp and
> caveat. `CLAUDE.md` keeps a short role/licence table; each crate's own
> `crates/<crate>/CLAUDE.md` remains the authority for that crate.

## Members

| Crate (`crates/…`) | Role | License |
|---|---|---|
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library (real-time simulators) | GPL-3.0 (relicensed from Apache-2.0 on 2026-08-11; published versions <= 0.1.1 stay Apache-2.0 — see crate `NOTICE`) |
| `teh-o-prke` | Point Reactor Kinetics (PRKE) for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| `tuas_boussinesq_solver` | Thermal-hydraulics (Boussinesq single-phase) solver — TUAS | GPL-3.0 |
| `tampines-steam-tables` | IAPWS-IF97 steam/water properties + steam-turbine equations — TAMPINES | GPL-3.0 |
| `outram-foam-basic-lib` | Pure-Rust translation of the OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, polynomial solvers, ODE solvers, interpolation, thermophysics kernels, fields, mesh, FV operators, fluid/solid thermo | GPL-3.0 |
| `njoy-outram-park-fork` | **All nuclear data** — NJOY2016 ENDF port (RECONR/BROADR/THERMR/ACER), the Faddeeva kernel, windowed-multipole evaluation, lean-ACE + WMP data blobs, ν̄/χ. Exposes the `XsProvider` surface other crates pull cross sections from. | GPL-3.0 |
| `outram-mc-libs` | **Monte Carlo transport** — CSG geometry, particle tracking, k-eigenvalue, delta (Woodcock) tracking for doubly heterogeneous media, depletion. **Data-free**: pulls cross sections from `njoy-outram-park-fork`. | GPL-3.0 |
| `tampines` | Central thermal-hydraulic framework — composes `tuas`, `outram-park-fork-coolprop`, `tampines-steam-tables`, `outram-foam-basic-lib`, `chem-eng…` | GPL-3.0 |
| `outram-park-fork-coolprop` | Pure-Rust fork of **CoolProp** — Helmholtz-EOS thermophysical properties (137 fluids, incompressibles, humid air, mixtures). Independent fork, not official CoolProp. | GPL-3.0 |
| `outram-park-fork-offbeat` | Pure-Rust fork of **OFFBEAT** (foam-for-nuclear) — nuclear fuel performance: solid mechanics with eigenstrain, rheology (plasticity/creep), fuel-cladding gap and contact, ~70 material property correlations, burnup/fast-flux/FGR, cladding corrosion. Independent fork, not official OFFBEAT. | GPL-3.0 |
| `farrer-park` | **FEM structural mechanics** — Finite-element Analysis for Reactor Reliability, Engineering Response, Plasticity And Risk. Small-strain linear elasticity and J2 plasticity with a consistent tangent, on Lagrange Tri3/Tri6/Quad4/Tet4/Hex8. Ported from **MOOSE**, **PRISMS-Plasticity** and **PRISMS-Fatigue** (all LGPL-2.1; GPL-3.0 via LGPL-2.1 §3, one-way). Depends on `outram-foam-basic-lib` for the shared Krylov/preconditioner backend **only, never for its discretisation** — it is genuinely FEM and must not be reformulated as finite volume (GitHub issue #175, epic `op-vrtt`). Verified against analytical/manufactured solutions (MMS orders match theory; patch test at machine precision; Lamé; Newton order 2.004) — **verification only, no human V&V, not a validated RPV or piping life-assessment tool**. **B-bar (mean dilatation)** and **plane stress** landed 2026-09-11 (`op-vrtt.1`, `op-vrtt.2`), both as explicit enums with the conservative option still the default: volumetric locking is measured and cured (nearly-incompressible MMS order 0.63-1.24 under full integration against 2.00-2.05 under B-bar; plastic collapse +23.9 % against +1.06 % versus the closed-form limit load). Known gaps: **shear locking is measured and NOT cured** (11.25 % too stiff at two square Quad4 elements through the depth, 66.7 % at aspect ratio 4 — `op-uqqg`), ILU(0) stagnates on nearly incompressible systems (`op-ldaz`), no curved elements. **Rate-dependent crystal plasticity landed 2026-09-11** (`op-q75c`): FCC {111}<110> and BCC {110}<111>, power-law flow, saturating self-and-latent hardening, cubic or isotropic single-crystal elasticity, consistent algorithmic tangent — **small strain**, so no lattice reorientation and no backstress. Verified against the analytic Schmid yield stress (1.5e-15 relative), frame indifference (7.6e-16), and a Richardson-extrapolated numerical Jacobian (3.6e-9, difference-scheme order 2.0006); **still verification only, no validation**. Microstructure-sensitive fatigue (`op-q1zn`) is **partial**: the Fatemi-Socie indicator parameter and region averaging, but no slip-band geometry and no fatigue life. Independent fork, not affiliated with INL/MOOSE or the PRISMS Center. | GPL-3.0 |
| `outram-park-fork-dwsim-libs` | Pure-Rust fork of **DWSIM** process-simulation building blocks. Independent fork. | GPL-3.0 |
| `outram-foam-turbulence-lib` | OpenFOAM turbulence closures (k-ω SST implemented; k-ε / k-ω / Spalart-Allmaras / Smagorinsky scaffolded) on `outram-foam-basic-lib` | GPL-3.0 |
| `outram-foam-appbuilder-lib` | OpenFOAM solver-application layer (pimpleFoam / rhoCentralFoam / rhoPimpleFoam) + case I/O; host of the in-progress **GeN-Foam** deterministic-neutronics + TH port | GPL-3.0 |
| `boon-lay` | TRISO-particle / Lagrangian decay simulator (BOON-LAY); includes the TRISO-ATOPS fork | GPL-3.0 |
| `nee_soon` | Integration / coupling layer — composes MC + deterministic/TH + nuclear data + PRKE. ~~Mostly scaffold.~~ **CORRECTED 2026-09-19** — the MC and deterministic ends are now wired and tested: `htr10_rmc` (1918 lines) builds one shared HTR-10 geometry for both; `mgxs` condenses a Monte Carlo run into multigroup constants (flux-weighted rates, a nu-scatter matrix, a measured fission spectrum); `genfoam_xs` hands them to GeN-Foam through its own `nuclearData` path; `coupling::McToGenFoam` is the facade; `direct_coupling::McGenFoamDirect` iterates MC against GeN-Foam's lumped thermal region to a steady state. Measured: GeN-Foam reproduces the MC eigenvalue to **+328 pcm (1.7 sigma)** on a leakage-free medium, and the HTR-10 core condenses to 4 zones GeN-Foam accepts. The prompt-excursion path remains wired to `teh-o-prke`. **Steady state only** — no delayed-neutron data is tallied — and **no validation**. | GPL-3.0 |
| `bedok` | Systems-level multiphysics coupling — 3-D nodal-diffusion neutronics coupled to channel TH, at the fidelity band **above 1-D neutronics and below CFD**. Rust translation of a MATLAB implementation by Than Yan Ren (SNRSI), used with the author's permission. **The file-by-file translation of the MATLAB snapshot is complete** (all 50 files, 2026-08-18). The **IAEA-3D** benchmark matches the published `k_eff` to **-1.1 pcm** (PARCS) / **+0.2 pcm** (ADPRES) — the crate's only validation evidence. The coupled steady loop and the transient both run, but are asserted **structurally only**: the NEACRP specification is not in `kovan-literature`, so there is no published curve or eigenvalue to compare against. **Every runnable case is verified against the running MATLAB** (2026-08-19), steady and transient; the X1 critical-boron discrepancy is **resolved** (causes: defect Z1, the silently rounded axial mesh, and defect N1, an unstable default nodal-update interval — run case A2 with `nodalupd >= 20`). **Stage-2 corrections opened 2026-08-21** and are **on by default**, so on the cases they touch this crate deliberately no longer reproduces the MATLAB: the diffusion face coupling and `gradterms` (G1/G2/G3), the W-3 `K4` enthalpy (T5/T6), and the hottest-channel search (C2/T4). Build from `Params::reference_faithful()` to get the reference's numbers back. Measured: A2's critical boron moves 1138.8 -> 1152.5 ppm against a published 1160.6, A1's 551.4 -> 561.0 against 567.7, and A2's reported DNBR falls 18.2%. See `docs/bedok-reference-defects.md`. | GPL-3.0 |
| `outram-park-digital-twin-engine` | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only; formerly `outram-park-digital-twin-gui`) | GPL-3.0 |
| `kovan-common` | **KOVAN** knowledge layer — shared canonical types (`KovanDocument`, `KovanSymbol`, …). The Rust struct is the source of truth. | GPL-3.0 |
| `kovan-discovery` | KOVAN file discovery + text search — the `fd` (`ignore`) walker and ripgrep (`grep-*`) engine. Offline, deterministic. | GPL-3.0 |
| `kovan-literature` | KOVAN literature archive — PDF → Markdown (`pulldown-cmark`) → `KovanDocument` → BibTeX. `open/` committable, `proprietary/` gitignored. | GPL-3.0 |
| `kovan-semantics` | KOVAN repo-understanding — ripgrep-first, escalating to language servers (rust-analyzer / clangd / Pyright / fortls). Does not reimplement compilers. | GPL-3.0 |
| `kovan-codegen` | KOVAN deterministic code generation — templates for known numerical methods (root finders, linear/nonlinear/ODE solvers). Not an AI assistant. | GPL-3.0 |
| `kovan-metrics` | KOVAN repository accounting — per-commit API-token trailers (read from the Claude Code session transcripts) and the pre-merge historian report. Replaced `docs/historian/*.py` on 2026-08-13 so the toolchain needs no Python. | GPL-3.0 |
| `kovan` (bins `kovan`, `kovan-cli`, `kovan-tui`) | KOVAN's three front ends over the knowledge layer, per GitHub issue #30's final interface spec (2026-08-21): `kovan` is the **human-facing GUI** (egui, the graph digitiser window); `kovan-cli` is the **agent-facing** CLI (`clap`, line-oriented output for Claude Code and other coding agents, incl. `digitise`); `kovan-tui` is the **human-facing** TUI (`ratatui`; genuinely Android/Termux-usable, not just buildable — its Android module gate was lifted the same day). Consolidated 2026-08-21 from the former separate `kovan-cli`/`kovan-tui` crates, then restructured from five binaries down to these three later the same day. **Relicensed to AGPL-3.0-only 2026-08-21** — the one crate in this workspace that differs from the default, so it can depend on `kopitiam-pdf` (also AGPL-3.0-only, GitHub issue #30's PDF-reader work). See `crates/kovan/NOTICE` and `crates/kovan/DECISIONS.md`. | **AGPL-3.0** (workspace exception — see NOTICE) |
| `outram-blender` | Mesh-authoring frontend (GPL fork of Blender's mesh architecture) — headless surface authoring with opt-in **Monte Carlo** (`mc-export` → `sim` → MC Studio) and **OpenFOAM volume-meshing** (`foam-mesh` → `foam_mesh` → tet-dual Mesh Studio) solver bridges. The two studio **examples** moved to `dhoby-ghaut` on 2026-09-17; the export bridges themselves stay here. Not affiliated with the Blender Foundation. | GPL-3.0 |
| `dhoby-ghaut` | **DHOBY GHAUT** (*Digital High-fidelity Orchestration by GUI for a Human-friendly Automated Unified Toolkit*) — the intended GUI home for OUTRAM PARK's meshing and Monte Carlo studios. **Placeholder: no GUI is implemented yet** (`src/lib.rs` only). Holds the `mc_studio` and `mesh_studio` **examples**, moved here verbatim (100 % rename) from `outram-blender` on 2026-09-17. | GPL-3.0 |
| `outram-park-fork-cfmesh` | Pure-Rust fork of **cfMesh** — Cartesian/tetrahedral/polyhedral volume meshing with boundary layers; `pipeline::surface_to_tet_dual_mesh` consumes an `outram-blender` surface and emits an `outram-foam` polyMesh. Independent fork, not official cfMesh. | GPL-3.0 |
| `outram-foam-mesh` | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, ideasUnvToFoam, polyDualMesh). Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-foam-cli` | OpenFOAM-style command-line utilities (blockMesh, pimpleFoam, gen-foam, …) as terminal binaries. Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-foam-multiphase` | Phase-II multiphase CFD — drift-flux first (Euler-Euler two-fluid, wall boiling, CHF, dryout planned). Reference physics for TAMPINES reduced-order models. Scaffold, no human V&V. Independent fork, not official OpenFOAM. | GPL-3.0 |
| `outram-park-fork-liggghts` | Pure-Rust granular-DEM library — particles, contact mechanics, thermal DEM, pebble/packed-bed physics (ports LIGGGHTS/LAMMPS-granular). LIGGGHTS-PUBLIC is GPL-2-or-later (GPL-3-compatible; see `NOTICE`). ~~Scaffold.~~ **CORRECTED 2026-09-17** — **declared mature 2026-09-15** and cross-code verified against compiled upstream LIGGGHTS on six deterministic cases and the full HTR-10 core; "Scaffold" had been wrong since that declaration. Still no experimental validation of the granular physics. | GPL-3.0 |
| `outram-park-fork-pflotran` | Pure-Rust fork of **PFLOTRAN** — subsurface flow & reactive transport; enum-dispatched, `uom`-typed, no PETSc/FFI/MPI. Scaffold, no human V&V. Independent fork. | GPL-3.0 |
| `outram-park-mpi` | Pure-Rust **MPICH** subset — the MPI-3 API surface (communicators, datatypes, point-to-point, core collectives) over a shared-memory threads-as-ranks transport. No C/FFI, Android-buildable. Scaffold. Not affiliated with MPICH. | GPL-3.0 |
| `outram-park-fork-moltres` | **Circulating-fuel MSR** multiphysics on the `outram-foam-basic-lib` FV layer — multigroup neutron diffusion + delayed-neutron **precursor drift** + salt heat transfer, reimplemented from the LGPL-2.1 **Moltres** formulation on `FvMesh`/`fvm` rather than MOOSE/PETSc finite elements. Steady eigenvalue only (no coupled flux transient), and **no crate depends on it yet**. Untrusted AI-assisted draft, no human V&V. Independent fork, not affiliated with Moltres/ARFC. | GPL-3.0 |
| `outram-park-fork-onix` | Pure-Rust fork of **ONIX** (MIT upstream) — Bateman/CRAM depletion + fission-product inventory for the MSRE digital twin. Untrusted AI-assisted draft, no human V&V. Independent fork, not affiliated with ONIX. | GPL-3.0 |
| `outram-park-fork-thermochimica` | Pure-Rust fork of **ORNL Thermochimica** (BSD-3) — molten-salt Gibbs-energy-minimisation thermochemistry (fission-product speciation, redox, solubility) for the MSRE digital twin. Scaffold, no human V&V. Independent fork, not affiliated with ORNL. | GPL-3.0 |
| `changi` | **CHANGI** (*Consequence and Hazard Analysis for Nuclear Ground-level and atmospheric Impacts*) — atmospheric consequences ("what happens after release?"). **Current scope, research/educational only:** dispersion, plume transport, radionuclide deposition, ground contamination. Radiological consequence assessment, dose assessment, emergency-planning and Level 3 PSA support are recorded as **future** scope, not current capability (maintainer direction, 2026-09-15). Middle link of SEMBAWANG → CHANGI → REDHILL, taking source terms from SEMBAWANG. Rust port of **FLEXPART v10.4** (NILU, GPL-3.0-or-later, commit `3d7eebf`). **Surface-layer and deposition scalar kernels ported and verified code-to-code** against the upstream Fortran compiled at two precisions — 11 of 12 function groups bit-exact against a `-fdefault-real-8` build, the rest of the 5e-8..3.4e-6 spread being FLEXPART's own single precision (its makefile passes no `-fdefault-real-8`). The particle advection loop, Hanna turbulence, CBL scheme, wet scavenging, mixing-height diagnostic and met readers are **not** ported. Uses `petir` for `erf` rather than porting FLEXPART's `erf.f90`. Untrusted AI-assisted draft, no human V&V, not declared mature. **Research, education and V&V only — never emergency planning, emergency response, dose assessment for real populations, or Level 3 PSA** (`docs/ecosystem-naming.md` decision 3, reaffirmed 2026-09-15). Independent fork, not affiliated with NILU. Epic `op-k9em`. | GPL-3.0 |
| `raffles` | **RAFFLES** (Risk Analysis Framework For Learning & Ensemble Simulation) — independent pure-Rust port of the UQ / risk-analysis core of **RAVEN** (Apache-2.0, Idaho National Laboratory): distributions, samplers, Sobol/correlation sensitivity, surrogates. **Owned by Adolphus Lye.** Apache-2.0 into GPL-3.0 is **one-way** — code cannot flow back to RAVEN (see the crate `NOTICE`). ~~Scaffold only, nothing implemented, no human V&V.~~ **CORRECTED 2026-09-17** — implemented in part: distributions, samplers, sensitivity, Bayesian model updating, distances, ABC, imprecise probability, model selection, GNNs and surrogates (~17.8k lines under `src/`, `src/bayesian/`, `src/gnn/`, `src/surrogate/`) all carry working, tested code, per the crate's own `CLAUDE.md`. All AI-assisted draft, **still no human V&V**. Independent fork, not affiliated with RAVEN/INL. Scoping: `docs/raven-port-scoping.md`. | GPL-3.0 |
| `dover` | **DOVER** (*Deck-based Open-source Visualisation Engine for Reactors*) — **empty skeleton, scope to be decided** (added 2026-09-25, after this table was split out; maintainer direction). No code, no public API, no dependencies. The name points towards visualisation driven by input decks; no scope is to be inferred from it. See `crates/dover/CLAUDE.md`. | GPL-3.0 |

> **KOVAN** is the deterministic *knowledge* layer (literature + semantics +
> codegen), interfaced three ways, all binaries of the single `kovan` crate:
> the `kovan-cli` **CLI** for agents, the `kovan-tui` **TUI** for humans, and
> the `kovan` **GUI** (the graph digitiser window) for humans. Offline /
> Android-first, no cloud, no Tree-sitter/SQLite/vector-store. Full design
> spec: **`docs/kovan.md`** (+ `docs/kovan-architecture.md`). Non-GUI kovan
> crates build for Android; `kovan-cli` and `kovan-tui` (including its
> Digitiser tab) are genuinely Android/Termux-usable — `ratatui` is an
> unconditional dependency, not target-gated off Android (see
> `crates/kovan/README.md` "Android"); only the `kovan` GUI's egui/eframe
> stack is Android-hostile and stays behind the `gui` feature, target-gated
> off Android on top of that.

> **MSRE digital-twin group:** `outram-park-fork-moltres` (circulating-fuel
> neutronics), `outram-park-fork-onix` (depletion) and
> `outram-park-fork-thermochimica` (salt thermochemistry) exist to serve the
> MSRE digital twin and are tracked under the **`op-6w0`** epic. All three are
> AI-assisted drafts with no human V&V, and none is wired into a simulator yet
> — do not describe any of them as validated. Scoping: `docs/reactor-scoping/msre.md`.

> **Neutronics architecture:** the responsibility split (nuclear data ⟂ Monte
> Carlo ⟂ deterministic/TH ⟂ coupling), the dependency graph, and phasing live in
> **`docs/architecture.md`**. Rule of thumb: *all* cross-section /
> nuclear-data code belongs in `njoy-outram-park-fork`; transport crates are
> data-free and pull from it.

**Planned future crates** (not yet in the workspace):

| Crate | Depends on | Targets |
|---|---|---|
| `openfoam-icof` | `outram-foam-basic-lib` | **icoFoam** (incompressible laminar PISO) |
| `openfoam-cht` | `outram-foam-basic-lib` | **chtMultiRegionFoam** (conjugate heat transfer, multi-region) |
| `openfoam-rho` | `outram-foam-basic-lib` | **rhoPimpleFoam** / **sonicFoam** (compressible) |
| **GenFOAM** (deterministic + TH) | *ported inside* `outram-foam-appbuilder-lib` | Deterministic neutronics + thermal hydraulics. **No longer on hold — substantially ported**: `src/genfoam/` is ~32k lines / ~262 tests as of 2026-08-07 (AI-assisted draft, no human V&V). |

> `nee-soon` is no longer "planned" — it exists as the `nee_soon` member crate
> (see the Members table above); it remains mostly scaffold.

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`outram-foam-basic-lib`.  `outram-foam-basic-lib` provides the mathematical building
blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling logic,
and turbulence model registries belong in solver-specific crates so that
`outram-foam-basic-lib` stays publishable independently and is reusable by other
projects.

Internal dependency edges (all by **path**, not crates.io):
`teh-o-prke → tuas` (dev); `teh-o-prke → chem-eng` (real, non-dev -- `nordheim_fuchs`'s
optional reactivity-input driver reuses `chem-eng`'s `TransferFnFirstOrder`);
`tuas` dev-deps → `chem-eng`, `teh-o-prke`;
`nee_soon → teh-o-prke` (real -- `NeeSoon::new_prompt_excursion_model` exposes
`teh-o-prke::nordheim_fuchs::NordheimFuchsExactTimestepper`);
`outram-park-digital-twin-engine → nee_soon` (real -- `components::ReactorVesselVisual`
wraps `NordheimFuchsExactTimestepper`);
`tampines` dev-deps → `{tuas, teh-o-prke, chem-eng}` (the FHR simulator examples use TUAS —
the `tampines` **library** itself is TUAS-free).
`farrer-park → outram-foam-basic-lib` (real -- for the **shared numerical backend only**:
`farrer-park`'s FEM `CsrMatrix` implements `outram_foam_basic_lib::linear_operator::LinearOperator`
and drives that crate's `cg_op`/`gmres_op`/`bicgstab_op`. It does **not** take the FV
discretisation; `LduMatrix` stays FVM-optimised and FEM stays FEM. `linear_operator` was added
to `outram-foam-basic-lib` for this and is **purely additive** — no existing solver signature
changed. Note GAMG and Gauss-Seidel are *not* on the contract, since coarsening needs face
addressing, so `farrer-park` carries its own CSR ILU(0)).
`outram-foam-basic-lib` has no internal deps (pure third-party: `uom`, `ndarray`, `thiserror`).
`njoy-outram-park-fork` is lean (`thiserror`, `uom`; no BLAS) so data consumers stay light.
Neutronics edges (target): `outram-mc-libs → njoy-outram-park-fork` (cross sections; declared in
root workspace deps, wiring deferred); `nee-soon → {outram-mc-libs, njoy-outram-park-fork, teh-o-prke, outram-foam-appbuilder-lib}`.

