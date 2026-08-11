# outram-park-backend

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


Cargo workspace for **OUTRAM PARK** — Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit.

A suite of Rust libraries for real-time thermal-hydraulics, reactor kinetics, steam-cycle thermodynamics, and compressible CFD simulation.

> *"Best open-source nuclear simulator suite in Singapore, JB — and some say Batam!"* 🇸🇬
> — with apologies to **Phua Chu Kang**. Said in fun, lah. For the real, sober
> status, see the ⚠️ banner above: everything here is unverified until validated,
> and **not** for facility operation.

## Crates

The workspace has 31 member crates, grouped by domain below.

**Thermal-hydraulics, fluid properties & process control**

| Crate | Role | License |
|---|---|---|
| [`chem-eng-real-time-process-control-simulator`](crates/chem-eng-real-time-process-control-simulator) | PID / transfer-function process-control library for real-time simulators | GPL-3.0 (published versions <= 0.1.1 on crates.io remain Apache-2.0) |
| [`tuas_boussinesq_solver`](crates/tuas_boussinesq_solver) | Thermal-hydraulics Boussinesq single-phase solver (TUAS) | GPL-3.0 |
| [`tampines-steam-tables`](crates/tampines-steam-tables) | IAPWS-IF97 steam/water properties + steam-turbine & choked-flow equations (TAMPINES) | GPL-3.0 |
| [`tampines`](crates/tampines) | Central thermal-hydraulic framework that composes the TH crates | GPL-3.0 |
| [`outram-park-fork-coolprop`](crates/outram-park-fork-coolprop) | Pure-Rust fork of CoolProp — Helmholtz-EOS thermophysical properties (independent fork, not official CoolProp) | GPL-3.0 |
| [`outram-park-fork-offbeat`](crates/outram-park-fork-offbeat) | Pure-Rust fork of OFFBEAT — nuclear fuel performance: mechanics, rheology, gap/contact, material correlations, burnup/FGR, corrosion (independent fork, not official OFFBEAT) | GPL-3.0 |
| [`outram-park-fork-dwsim-libs`](crates/outram-park-fork-dwsim-libs) | Pure-Rust fork of DWSIM process-simulation building blocks (independent fork) | GPL-3.0 |

**CFD (OpenFOAM translations)**

| Crate | Role | License |
|---|---|---|
| [`outram-foam-basic-lib`](crates/outram-foam-basic-lib) | Pure-Rust translation of the OpenFOAM primitive + finite-volume layer (Layers 1–4): tensor algebra, polynomial/ODE solvers, interpolation, FV operators (`fvm`/`fvc`, MUSCL), thermophysics kernels, fields, mesh (independent fork, not official OpenFOAM) | GPL-3.0 |
| [`outram-foam-turbulence-lib`](crates/outram-foam-turbulence-lib) | RAS/LES turbulence closures (k-ω SST implemented; others scaffolded) on top of `outram-foam-basic-lib` | GPL-3.0 |
| [`outram-foam-appbuilder-lib`](crates/outram-foam-appbuilder-lib) | Solver application layer (pimpleFoam / rhoCentralFoam / rhoPimpleFoam) + case I/O; host of the in-progress GeN-Foam deterministic-neutronics + TH port | GPL-3.0 |
| [`outram-foam-cli`](crates/outram-foam-cli) | OpenFOAM-style command-line utilities (blockMesh, pimpleFoam, gen-foam, …) as terminal binaries (independent fork, not official OpenFOAM) | GPL-3.0 |
| [`outram-foam-multiphase`](crates/outram-foam-multiphase) | Phase-II multiphase CFD — drift-flux (Euler-Euler two-fluid, wall boiling, CHF, dryout planned); scaffold, no human V&V (independent fork, not official OpenFOAM) | GPL-3.0 |

**Mesh generation & authoring**

| Crate | Role | License |
|---|---|---|
| [`outram-blender`](crates/outram-blender) | GPL fork of Blender's mesh-authoring architecture — headless surface-mesh frontend with opt-in Monte Carlo (`mc-export`) and OpenFOAM volume-meshing (`foam-mesh`) solver bridges (not affiliated with the Blender Foundation) | GPL-3.0 |
| [`outram-park-fork-cfmesh`](crates/outram-park-fork-cfmesh) | Pure-Rust fork of cfMesh — Cartesian/tetrahedral/polyhedral volume meshing with boundary layers; consumes an `outram-blender` surface and emits an `outram-foam` polyMesh (independent fork, not official cfMesh) | GPL-3.0 |
| [`outram-foam-mesh`](crates/outram-foam-mesh) | OpenFOAM mesh generation & conversion (blockMesh, snappyHexMesh, ideasUnvToFoam, polyDualMesh) (independent fork, not official OpenFOAM) | GPL-3.0 |

**Neutronics & nuclear data**

| Crate | Role | License |
|---|---|---|
| [`teh-o-prke`](crates/teh-o-prke) | Point Reactor Kinetics (PRKE) for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| [`njoy-outram-park-fork`](crates/njoy-outram-park-fork) | NJOY2016 ENDF port — all nuclear data (RECONR/BROADR/THERMR/ACER/…, ν̄/χ, windowed multipole); exposes `XsProvider` (independent fork, not official NJOY) | GPL-3.0 |
| [`outram-mc-libs`](crates/outram-mc-libs) | Monte Carlo transport — CSG geometry, particle tracking, k-eigenvalue, Woodcock/delta tracking, depletion; data-free (pulls cross sections from `njoy-outram-park-fork`) | GPL-3.0 |
| [`boon-lay`](crates/boon-lay) | TRISO-particle / Lagrangian decay simulator (BOON-LAY); includes the TRISO-ATOPS fork | GPL-3.0 |
| [`nee_soon`](crates/nee_soon) | Integration / coupling layer — composes MC + deterministic/TH + nuclear data + PRKE | GPL-3.0 |
| [`outram-park-fork-liggghts`](crates/outram-park-fork-liggghts) | Pure-Rust granular-DEM library — particles, contact mechanics, thermal DEM, pebble/packed-bed physics (ports LIGGGHTS/LAMMPS-granular; GPL-2-or-later, see NOTICE); scaffold | GPL-3.0 |

**Subsurface & infrastructure**

| Crate | Role | License |
|---|---|---|
| [`outram-park-fork-pflotran`](crates/outram-park-fork-pflotran) | Pure-Rust fork of PFLOTRAN — subsurface flow & reactive transport, no PETSc/MPI/FFI; scaffold, no human V&V (independent fork) | GPL-3.0 |
| [`outram-park-mpi`](crates/outram-park-mpi) | Pure-Rust MPICH subset — the MPI-3 API surface over a shared-memory threads-as-ranks transport, Android-buildable, no C/FFI; scaffold (not affiliated with MPICH) | GPL-3.0 |

**Digital twin**

| Crate | Role | License |
|---|---|---|
| [`outram-park-digital-twin-engine`](crates/outram-park-digital-twin-engine) | Offline digital-twin engine + egui GUI example simulators (offline demonstrations only) | GPL-3.0 |

**KOVAN knowledge layer** (offline / Android-first)

| Crate | Role | License |
|---|---|---|
| [`kovan-common`](crates/kovan-common) | Shared canonical KOVAN types (`KovanDocument`, `KovanSymbol`, …) | GPL-3.0 |
| [`kovan-discovery`](crates/kovan-discovery) | File discovery + text search (`ignore`/ripgrep) and git-awareness (`gix`) | GPL-3.0 |
| [`kovan-literature`](crates/kovan-literature) | Literature archive — PDF → Markdown → `KovanDocument` → BibTeX | GPL-3.0 |
| [`kovan-semantics`](crates/kovan-semantics) | Repo understanding — ripgrep-first, escalating to language servers | GPL-3.0 |
| [`kovan-codegen`](crates/kovan-codegen) | Deterministic code generation for known numerical methods | GPL-3.0 |
| [`kovan-cli`](crates/kovan-cli) | Agent-facing CLI (`kovan`) — line-oriented output for coding agents | GPL-3.0 |
| [`kovan-tui`](crates/kovan-tui) | Human-facing TUI (`ratatui`); CLI-redirect stub on Android | GPL-3.0 |

## Build

Requires a system BLAS (OpenBLAS on Linux):

```bash
# Arch / EndeavourOS
sudo pacman -S openblas
# Debian / Ubuntu
sudo apt install libopenblas-dev
```

```bash
cargo build --workspace
cargo test  --workspace --lib --tests
```

## Publishing (mandatory crate order)

`cargo publish` resolves **all** dependencies — normal *and* dev — against
crates.io, so each crate can only be packaged once everything it depends on
(directly or as a dev-dependency) is already live. Publish in this order:

| # | Crate | Must be published after |
|---|---|---|
| 1 | `chem-eng-real-time-process-control-simulator` | — (no internal deps) |
| 1 | `outram-foam-basic-lib` | — (no internal deps) |
| 1 | `outram-park-fork-coolprop` | — (no internal deps; only `uom`/`approx`) |
| 2 | `outram-foam-turbulence-lib` | `outram-foam-basic-lib` |
| 2 | `tuas_boussinesq_solver` | — (only dev-dep `chem-eng…`; the former `outram-foam-basic-lib` dep was removed in 0.1.3) |
| 3 | `outram-foam-appbuilder-lib` | `outram-foam-basic-lib`, `outram-foam-turbulence-lib` |
| 3 | `teh-o-prke` | `chem-eng…` (real dep) + dev-deps `tuas…`, `chem-eng…` |
| 4 | `tampines-steam-tables` | `tuas…` + dev-deps `teh-o-prke`, `chem-eng…` |

> This table covers the core inter-dependent crates. The remaining members
> (`tampines`, `njoy-outram-park-fork`, `outram-mc-libs`,
> `outram-park-fork-dwsim-libs`, `boon-lay`, `nee_soon`,
> `outram-park-digital-twin-engine`, and the `kovan-*` crates) publish per their
> own dependency edges; not all have been published yet. As of 2026-07-16,
> `tuas_boussinesq_solver`, `tampines-steam-tables`, `outram-park-fork-coolprop`,
> `outram-foam-basic-lib`, and `outram-foam-turbulence-lib` are live on crates.io
> (`outram-foam-appbuilder-lib` publish pending).

Publish each from the workspace root with `cargo publish -p <crate>` (commit
first; `cargo publish` refuses a dirty tree). Internal deps are
`{ path = …, version = … }` in the root `[workspace.dependencies]`, so a
crate's pinned `version` must be bumped here whenever an upstream crate it
depends on is bumped.

> **Keep this list in sync.** Whenever crate dependencies or versions change,
> update this table (and the per-crate version pins in the root `Cargo.toml`)
> so the publish order stays correct.

`cargo publish --dry-run` for crates 2–6 will fail with "failed to select a
version" until their upstreams are live — that is expected, not a packaging
error.

## Responsible Use

Outram Park is an open-source nuclear engineering simulation ecosystem for education, research, capability building, and verification and validation.

The project uses only open-source data, public literature data, and properly licensed public benchmark data. It does not use confidential, restricted, proprietary, operational, or unpublished facility data.

AI-assisted development may be used for coding, translation, refactoring, documentation, and test generation. AI-assisted outputs are treated as draft material and must undergo human review, licence provenance checks, testing, verification, and validation where applicable.

Outram Park is not intended for nuclear facility operation, reactor control, licensing decisions, emergency response, safety-critical decision-making, safeguards-sensitive analysis, or security-sensitive analysis.

Digital twin examples are offline education and research demonstrations only. They do not connect to live operational systems, plant systems, safety-critical infrastructure, institutional production systems, or restricted infrastructure.

Simulation results should be interpreted with care. Users are responsible for checking assumptions, numerical limitations, data provenance, validation status, error estimates, and applicability to their intended use.

For details, see [`RESPONSIBLE_USE.md`](RESPONSIBLE_USE.md) and [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Changelog

Human-in-the-loop decision log — the maintainer's directions and sign-offs,
recorded for provenance. AI agents executed these; a human decided them. Code
remains **unverified until a V&V case demonstrates otherwise** (see the banner
above).

### 2026-07-15

**Vendoring & forks** (upstreams cloned into each crate's gitignored
`upstream_source/`, per the workspace vendoring rule):

- **GeN-Foam** (GPL-3.0, `652b3da`) vendored into `outram-foam-appbuilder-lib`;
  Rust port begun (point-kinetics slice verified against the inhour equation).
- **OpenFOAM core** (v2606, `481094fd`) vendored into `outram-foam-basic-lib` as
  the primitive-layer reference.
- **TRISO-ATOPS** (MIT, Idaho National Laboratory, `de374c8`) forked into
  `boon-lay` as `triso_atops_fork` — a Eulerian TRISO fission-product-release
  model complementing boon-lay's Lagrangian one. All non-GUI functionality
  ported; GUI excluded; MIT attribution preserved. **Decision:** incorporate
  `uom` into its activity-bookkeeping layer (explicit sign-off overriding the
  unit-convention guardrail).

**Digital-twin engine:**

- Renamed `outram-park-digital-twin-gui` → **`outram-park-digital-twin-engine`**.
- Roadmap set: rework/build `fhr_sim_v2`, `htgr_sim_v1`, `ipwr_sim_v1` on the
  engine's reusable widgets (fhr + htgr are the current priority; ABWR and others
  are future work). `fhr_sim_v2` migrated into the engine crate as the first
  consumer.

**Dependency policy:**

- **`ndarray-linalg` removed** from all non-benchmark code — the OpenFOAM-port
  pure-Rust dense-LU `SquareMatrix` replaces the sole remaining linear solve
  (`fhr_sim_v2` secondary-loop mass balance). It now survives only in the
  `outram-foam-basic-lib` matrix benchmark.
- Approved new pure-Rust, Android-friendly workspace dependencies for KOVAN:
  `regex`, `pdf-extract`, `lopdf`, `tempfile`.

**KOVAN knowledge layer:**

- Directed a best-effort implementation pass across all seven `kovan-*` crates
  (per-agent decisions in
  [`docs/kovan-agent-decisions-for-review.md`](docs/kovan-agent-decisions-for-review.md)).
- Approved: the `Method::Pde` codegen-catalogue addition; the `kovan-common`
  shared-type additions requested by downstream crates (`KovanSymbol` location
  fields, `KovanDocument` asset/page/journal-locator fields + builder,
  `GeneratedArtifact`); and keeping `kovan-discovery`'s `require_git(false)` fix
  (honour `.gitignore` outside a `.git` repository).

**TUAS validation:**

- Adopted the SAM-matched CIET coupled-DRACS **pipe-38 form loss K = 17.8**
  (validated on the isolated loop; shared constructor `new_pipe_38_sam_model`),
  lowering mean |DRACS error| vs experiment from **3.83 % to 2.76 %** and
  tightening every mid/high-flow case ~2 pp (matching SAM's NED-2021 Table-4
  predictions). The ~275 regression references were recomputed at K = 17.8 and
  **all 25 coupled cases pass**. A single uniform K cannot correct a bias that is
  over-prediction at high flow and under-prediction at low flow, so the two
  lowest-flow cases (b1 655 W, c1 841 W) — and, via reduced DHX heat removal, two
  high-power heater-surface-temperature bounds (b7, b9) — take **per-point,
  documented tolerance widenings** with in-source justification; **no global
  benchmark (mass-flow) tolerance was loosened**. The proper velocity-dependent
  pipe-38 loss model is deferred to bead `op-4wl.5`. Per-case plotting CSVs, plus
  41 redirected pre-existing test CSV writers, now land in the gitignored
  `verification_and_validation/` folder. This calibration is documented as a
  human-in-the-loop V&V decision (see `tuas_boussinesq_solver`'s README).
