# outram-park-backend

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


Cargo workspace for **OUTRAM PARK** — Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit.

A suite of Rust libraries for real-time thermal-hydraulics, reactor kinetics, steam-cycle thermodynamics, and compressible CFD simulation.

## Crates

| Crate | Role | License |
|---|---|---|
| [`chem-eng-real-time-process-control-simulator`](crates/chem-eng-real-time-process-control-simulator) | PID / transfer-function process-control library for real-time simulators | Apache-2.0 |
| [`tuas_boussinesq_solver`](crates/tuas_boussinesq_solver) | Thermal-hydraulics Boussinesq single-phase solver (TUAS) | GPL-3.0 |
| [`teh-o-prke`](crates/teh-o-prke) | Point Reactor Kinetics for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| [`tampines-steam-tables`](crates/tampines-steam-tables) | IAPWS-IF97 steam/water properties + steam-turbine cycle equations (TAMPINES) | GPL-3.0 |
| [`outram-foam-basic-lib`](crates/outram-foam-basic-lib) | Pure-Rust translation of the OpenFOAM primitive layer — tensor algebra, polynomial solvers, ODE integrators, interpolation utilities, FV operators (`fvm`/`fvc`, MUSCL reconstruction), and specie-level thermophysics kernels — building toward compressible CFD solvers equivalent to **rhoPimpleFoam** and **sonicFoam** | GPL-3.0 |
| [`outram-foam-turbulence-lib`](crates/outram-foam-turbulence-lib) | RAS/LES turbulence closures (k-ω SST, …) on top of `outram-foam-basic-lib` | GPL-3.0 |
| [`outram-foam-appbuilder-lib`](crates/outram-foam-appbuilder-lib) | Solver application layer — pimpleFoam / rhoCentralFoam / rhoPimpleFoam loops and OpenFOAM case I/O | GPL-3.0 |

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
| 1 | `outram-foam-basic-lib` | — (no internal deps; can go in parallel with chem-eng) |
| 2 | `outram-foam-turbulence-lib` | `outram-foam-basic-lib` |
| 3 | `tuas_boussinesq_solver` | `outram-foam-basic-lib` (+ dev-dep `chem-eng…`) |
| 4 | `outram-foam-appbuilder-lib` | `outram-foam-basic-lib`, `outram-foam-turbulence-lib` |
| 5 | `teh-o-prke` | `outram-foam-basic-lib` (+ dev-deps `tuas…`, `chem-eng…`) |
| 6 | `tampines-steam-tables` | `tuas_boussinesq_solver` (+ dev-dep `teh-o-prke`) |

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
