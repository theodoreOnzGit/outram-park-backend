<!-- Moved verbatim from the workspace CLAUDE.md on 2026-09-21 (maintainer direction: physics
     rules stay in the root, everything else here behind a pointer). STILL BINDING. -->

## Members

**44 member crates.** The table below is a one-line index: what each crate is
for, and whether it is declared mature. **Each crate's own
`crates/<crate>/CLAUDE.md` and `README.md` are the authority** for its status,
its bar, its known gaps and its correction history — this table is a pointer,
not a status record, and must not be cited as evidence of anything.

> Full per-crate prose, with every status note, caveat and dated correction as
> it stood on 2026-09-21:
> [`docs/claude-md-rationale/crate-roster.md`](../claude-md-rationale/crate-roster.md).

All crates are **GPL-3.0** except `kovan` (**AGPL-3.0-only**, workspace
exception — see `crates/kovan/NOTICE`) and
`chem-eng-real-time-process-control-simulator` (GPL-3.0 since 2026-08-11;
published versions <= 0.1.1 stay Apache-2.0 — see its `NOTICE`). "Mature" in
the last column means the maintainer has declared it so — see "When this
applies: only to crates declared mature".

| Crate (`crates/…`) | Role | Mature |
|---|---|---|
| `petir` | **Core numerics** — polynomials, equations, transforms, integration, roots. `no_std`, dependency-lean, ported from GSL. Every other crate's numerics floor. | ✅ |
| `outram-foam-basic-lib` | OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, solvers, interpolation, thermophysics, fields, mesh, FV operators | ✅ |
| `outram-foam-turbulence-lib` | OpenFOAM turbulence closures (k-ω SST implemented; others scaffolded) | |
| `outram-foam-appbuilder-lib` | OpenFOAM solver-application layer + case I/O; host of the **GeN-Foam** deterministic-neutronics + TH port | ✅ |
| `outram-foam-mesh` | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, …) | |
| `outram-foam-cli` | OpenFOAM-style command-line utilities as terminal binaries | |
| `outram-foam-multiphase` | Phase-II multiphase CFD — drift-flux first. Scaffold, no human V&V | |
| `njoy-outram-park-fork` | **All nuclear data** — NJOY2016 ENDF port (RECONR/BROADR/THERMR/ACER), Faddeeva, windowed multipole, ν̄/χ. Exposes `XsProvider` | ✅ |
| `outram-mc-libs` | **Monte Carlo transport** — CSG geometry, tracking, k-eigenvalue, delta tracking, depletion. **Data-free**: pulls from `njoy-outram-park-fork` | ✅ |
| `teh-o-prke` | Point Reactor Kinetics (PRKE) | ✅ |
| `nee_soon` | Integration / coupling layer — MC ⟷ deterministic/TH ⟷ nuclear data ⟷ PRKE. Steady state only, no validation | |
| `bedok` | Systems-level multiphysics — 3-D nodal diffusion + channel TH, above 1-D neutronics and below CFD. IAEA-3D matches published `k_eff`; stage-2 corrections on by default | |
| `tuas_boussinesq_solver` | Thermal-hydraulics (Boussinesq single-phase) — TUAS | ✅ |
| `tampines` | Central thermal-hydraulic framework — composes TUAS, CoolProp, steam tables, outram-foam, chem-eng | |
| `tampines-steam-tables` | IAPWS-IF97 steam/water properties + steam-turbine equations | ✅ |
| `chem-eng-real-time-process-control-simulator` | PID / transfer-function process-control library | ✅ |
| `outram-park-fork-coolprop` | Pure-Rust fork of **CoolProp** — Helmholtz-EOS properties (137 fluids, incompressibles, humid air, mixtures) | |
| `outram-park-fork-dwsim-libs` | Pure-Rust fork of **DWSIM** process-simulation building blocks. **EOS layer only** has met its bar; flash layer open | ✅ |
| `outram-park-fork-offbeat` | Pure-Rust fork of **OFFBEAT** — nuclear fuel performance (eigenstrain, rheology, gap/contact, burnup, FGR, corrosion) | |
| `farrer-park` | **FEM structural mechanics** — small-strain elasticity, J2 plasticity, crystal plasticity. **Shear locking measured and NOT cured** (`op-uqqg`) | ✅ |
| `outram-park-fork-liggghts` | Granular DEM — contact mechanics, thermal DEM, pebble-bed physics (ports LIGGGHTS). Cross-code verified; **no experimental validation** | ✅ |
| `outram-park-fork-pflotran` | Pure-Rust fork of **PFLOTRAN** — subsurface flow & reactive transport. Scaffold | |
| `outram-park-fork-cfmesh` | Pure-Rust fork of **cfMesh** — Cartesian/tet/polyhedral volume meshing with boundary layers | |
| `outram-park-fork-moltres` | **Circulating-fuel MSR** — multigroup diffusion + precursor drift + salt heat transfer on the FV layer. Steady eigenvalue only, no consumer yet | |
| `outram-park-fork-onix` | Pure-Rust fork of **ONIX** — Bateman/CRAM depletion + fission-product inventory | |
| `outram-park-fork-thermochimica` | Pure-Rust fork of **ORNL Thermochimica** — molten-salt Gibbs-energy minimisation. Scaffold | |
| `boon-lay` | TRISO-particle / Lagrangian decay simulator; includes the TRISO-ATOPS fork | |
| `kaki-bukit` | **KAKI BUKIT** — agent-based nuclear fuel-cycle kernel, `no_std` fork of CYCLUS/CYCAMORE, numerics from `petir`. Scaffold, no human V&V | |
| `changi` | **CHANGI** — atmospheric dispersion, plume transport, deposition, ground contamination. Two ports: FLEXPART v10.4 (scalar kernels only) and the Gaussian puff model of `Hammerling-Research-Group/puff` (physics complete, MIT→GPL one-way). Plus `changi::activity` (2026-09-21, **not a port, no upstream**): unit-release chi/Q, decay in transit, dry deposition; deposition velocities are uncited placeholders. **Research/education/V&V only** | |
| `sembawang` | **SEMBAWANG** — ~~source term for CHANGI.~~ **Severe accident and source term, and orchestrator of the offsite chain** (2026-09-21, #235): drives CHANGI today, RAFFLES and REDHILL later; no dose. ~~Placeholder: nothing implemented~~ **CORRECTED 2026-09-21** — the TRISO fission-product **release** path exists (on `boon-lay`'s TRISO-ATOPS fork, under a caller-**prescribed** temperature transient and inventory). **Severe-accident progression (melt, relocation, vessel failure, MCCI, hydrogen, aerosols) is NOT implemented.** No human V&V | |
| `redhill` | **REDHILL** — groundwater and geological transport of radionuclides after deposition. **Placeholder: nothing implemented** | |
| `dover` | **DOVER** — *Deck-based Open-source Visualisation Engine for Reactors*. **Empty skeleton** (2026-09-25): scope to be decided, nothing implemented, no dependencies | |
| `raffles` | **RAFFLES** — UQ / risk analysis ported from RAVEN. **Owned by Adolphus Lye.** Apache-2.0 → GPL-3.0 is **one-way**. Implemented in part, no human V&V | |
| `outram-park-mpi` | Pure-Rust **MPICH** subset over a shared-memory threads-as-ranks transport. No C/FFI, Android-buildable. Scaffold | |
| `outram-blender` | Mesh-authoring frontend (GPL fork of Blender's mesh architecture) + the MC and OpenFOAM export bridges | |
| `dhoby-ghaut` | **DHOBY GHAUT** — intended GUI home for the meshing and MC studios. **Placeholder**; holds the `mc_studio` / `mesh_studio` examples | |
| `outram-park-digital-twin-engine` | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only) | |
| `kovan-common` | KOVAN shared canonical types (`KovanDocument`, `KovanSymbol`, …) | |
| `kovan-discovery` | KOVAN file discovery + text search (`ignore` walker, `grep-*` engine) | |
| `kovan-literature` | KOVAN literature archive — PDF → Markdown → `KovanDocument` → BibTeX. `open/` committable, `proprietary/` gitignored | |
| `kovan-semantics` | KOVAN repo understanding — ripgrep-first, escalating to language servers | |
| `kovan-codegen` | KOVAN deterministic code generation for known numerical methods. Not an AI assistant | |
| `kovan-metrics` | KOVAN repository accounting — token trailers and the historian report | |
| `kovan` (bins `kovan`, `kovan-cli`, `kovan-tui`) | KOVAN's three front ends: `kovan` = **human GUI** (egui, digitiser window); `kovan-cli` = **agent CLI**; `kovan-tui` = **human TUI** (ratatui, Android/Termux-usable) | |

> **KOVAN** is the deterministic *knowledge* layer (literature + semantics +
> codegen), interfaced three ways, all binaries of the single `kovan` crate.
> Offline / Android-first, no cloud, no Tree-sitter/SQLite/vector-store. Full
> design spec: **`docs/kovan.md`** (+ `docs/kovan-architecture.md`). Non-GUI
> kovan crates build for Android; only the `kovan` GUI's egui/eframe stack is
> Android-hostile and stays behind the `gui` feature.

> **MSRE digital-twin group:** `outram-park-fork-moltres`,
> `outram-park-fork-onix` and `outram-park-fork-thermochimica`, tracked under
> the **`op-6w0`** epic. All three are AI-assisted drafts with no human V&V and
> none is wired into a simulator yet — do not describe any as validated.
> Scoping: `docs/reactor-scoping/msre.md`.

> **Consequence chain:** SEMBAWANG → CHANGI → REDHILL (source term →
> atmospheric dispersion → groundwater). Two of the three are placeholders.

> **Neutronics architecture:** the responsibility split (nuclear data ⟂ Monte
> Carlo ⟂ deterministic/TH ⟂ coupling), the dependency graph and phasing live
> in **`docs/architecture.md`**. Rule of thumb: *all* cross-section /
> nuclear-data code belongs in `njoy-outram-park-fork`; transport crates are
> data-free and pull from it.

**Planned future crates** (not yet in the workspace): `openfoam-icof`
(**icoFoam**), `openfoam-cht` (**chtMultiRegionFoam**), `openfoam-rho`
(**rhoPimpleFoam** / **sonicFoam**) — all on `outram-foam-basic-lib`.
**GenFOAM** is *ported inside* `outram-foam-appbuilder-lib` (`src/genfoam/`,
~32k lines / ~262 tests as of 2026-08-07; AI-assisted draft, no human V&V).

**Layer 5 (solver loop logic) MUST live in these separate crates**, not in
`outram-foam-basic-lib`. `outram-foam-basic-lib` provides the mathematical
building blocks (Layers 1–4) only; the PISO/PIMPLE loop, multi-region coupling
logic, and turbulence model registries belong in solver-specific crates so
that `outram-foam-basic-lib` stays publishable independently.

**Internal dependency edges** are all by **path**, not crates.io. The ones
worth knowing: `teh-o-prke → {tuas (dev), chem-eng (real)}`; `tuas` dev-deps →
`{chem-eng, teh-o-prke}`; `nee_soon → teh-o-prke`;
`outram-park-digital-twin-engine → nee_soon`; `tampines` dev-deps →
`{tuas, teh-o-prke, chem-eng}` (the **library** itself is TUAS-free);
`outram-mc-libs → njoy-outram-park-fork` (cross sections).
`outram-foam-basic-lib` has no internal deps, and `njoy-outram-park-fork` is
kept lean (`thiserror`, `uom`; no BLAS) so data consumers stay light.

**`farrer-park → outram-foam-basic-lib` is for the shared numerical backend
ONLY.** Its FEM `CsrMatrix` implements
`outram_foam_basic_lib::linear_operator::LinearOperator` and drives that
crate's `cg_op`/`gmres_op`/`bicgstab_op`. It does **not** take the FV
discretisation — `LduMatrix` stays FVM-optimised and FEM stays FEM
(GitHub issue #175, epic `op-vrtt`). `linear_operator` was added for this and
is purely additive. GAMG and Gauss-Seidel are *not* on the contract, since
coarsening needs face addressing, so `farrer-park` carries its own CSR ILU(0).

## Dependency policy — single source of truth

All third-party versions live in the root `[workspace.dependencies]`. Members
inherit them with `<dep>.workspace = true`, so versions **cannot drift**. **When
changing a shared dependency, edit the root `Cargo.toml` only.** The one
exception is `ndarray-linalg`, whose BLAS backend feature is chosen per-target
(`openblas-system` on unix, `intel-mkl-static` on windows/macos) — as of
2026-08-07 exactly one member still declares it, `outram-foam-basic-lib`, and
only as a target-gated **dev-dependency** for `tests/matrix_bench.rs`. TUAS's
`ndarray-linalg` removal is **done** (TUAS v0.1.2/0.1.3), not planned.

A second exception: **`kopitiam-pdf`** (AGPL-3.0-only, GitHub issue #30's
PDF-reader work) is declared only in `crates/kovan/Cargo.toml`'s own
`[dependencies]`, never in the workspace table — `kovan` is the sole
AGPL-3.0-only crate in this workspace, and keeping the dependency
crate-local is what stops another crate from picking it up (and the
AGPL question that comes with it) by accident. See `crates/kovan/NOTICE`.

See `docs/workspace-maintenance.md` for the rationale and history.

### `burn` is this workspace's PyTorch (maintainer direction, 2026-09-16)

Several of the codes this suite ports from reach for **PyTorch** for their
machine-learning parts — RAVEN's surrogates are the first case to land. The
Rust replacement is **`burn`** (tracel-ai, MIT OR Apache-2.0, so permissive
into GPL-3.0 and one-way like the rest): **reach for `burn` wherever the
upstream reaches for PyTorch, as far as `burn` will go.** Do not introduce a
second ML framework, and do not hand-roll a tensor/autodiff layer beside it.

- Declared once in the root `[workspace.dependencies]` as
  `burn = { version = "0.21.0", default-features = false }` — i.e. **`no_std`**,
  which is deliberate: `burn` runs on `core` + `alloc` (`alloc` is implicit,
  there is no feature to name), and that is what keeps it inside the Android
  and wasm rules below. `default-features = false` also drops `burn`'s
  std-only dataset/network/train/sqlite stack. A `std` binary links a `no_std`
  `burn` without complaint, so do not turn defaults back on for convenience.
- **The backend is the consumer's choice, and the safe one is `ndarray`** —
  pure Rust, libm-backed, no system BLAS. `accelerate`, `blas-netlib`,
  `openblas`, `openblas-system`, `tch`, `candle`, `cuda` and `rocm` all want a
  system BLAS/LAPACK or a C/C++ toolchain and must never appear in an
  unconditional library build.
- **GPU is out of scope for the first pass.** `burn`'s
  `wgpu`/`vulkan`/`metal`/`webgpu` backends inherit the `wgpu` rule: behind a
  feature, example or optional bin under `cfg(not(target_os = "android"))`.
- **MSRV:** `burn` 0.21 declares `rust-version = "1.92"` and `edition = "2024"`,
  so any crate that actually enables it raises its own toolchain floor to 1.92.
  Upstream has deprecated `burn-ndarray` in the 0.22 pre-releases in favour of
  `burn-flex`/`burn-cpu`; check that crate's `no_std` story before bumping.
- First consumer: `raffles`, optionally, behind its own `burn` feature (off by
  default) — see `crates/raffles/CLAUDE.md` "Machine learning".

