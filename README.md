# outram-park-backend

Cargo workspace for **OUTRAM PARK** — Open-source TRAnsient Multi-Phase Advanced Reactor simulator Kit.

A suite of Rust libraries for real-time thermal-hydraulics, reactor kinetics, steam-cycle thermodynamics, and compressible CFD simulation.

## Crates

| Crate | Role | License |
|---|---|---|
| [`chem-eng-real-time-process-control-simulator`](crates/chem-eng-real-time-process-control-simulator) | PID / transfer-function process-control library for real-time simulators | Apache-2.0 |
| [`tuas_boussinesq_solver`](crates/tuas_boussinesq_solver) | Thermal-hydraulics Boussinesq single-phase solver (TUAS) | GPL-3.0 |
| [`teh-o-prke`](crates/teh-o-prke) | Point Reactor Kinetics for the Teh-O transport/eigenvalue solver | GPL-3.0 |
| [`tampines-steam-tables`](crates/tampines-steam-tables) | IAPWS-IF97 steam/water properties + steam-turbine cycle equations (TAMPINES) | GPL-3.0 |
| [`openfoam-basic-lib`](crates/openfoam-basic-lib) | Pure-Rust translation of the OpenFOAM primitive layer — tensor algebra, polynomial solvers, ODE integrators, interpolation utilities, and specie-level thermophysics kernels — building toward compressible CFD solvers equivalent to **rhoPimpleFoam** and **sonicFoam** | GPL-3.0 |

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
