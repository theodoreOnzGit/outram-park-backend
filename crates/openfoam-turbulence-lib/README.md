# openfoam-turbulence-lib

Pure-Rust port of the OpenFOAM turbulence model library, part of the
**OUTRAM PARK** (Open-source TRAnsient Multi-Phase Advanced Reactor simulator
Kit) suite.

Provides RAS (Reynolds-Averaged Simulation) and LES (Large Eddy Simulation)
turbulence closures for use with `openfoam-appbuilder-lib` solver loops.

## Planned turbulence models

| Model | Type | C++ class |
|---|---|---|
| Laminar | RAS no-op | `laminar` |
| k-ε | RAS two-equation | `kEpsilon` |
| k-ω | RAS two-equation | `kOmega` |
| k-ω SST | RAS two-equation | `kOmegaSST` |
| Spalart-Allmaras | RAS one-equation | `SpalartAllmaras` |
| Smagorinsky | LES sub-grid | `Smagorinsky` |

## Status

Scaffold only — no models implemented yet. See `CLAUDE.md` for the
implementation plan.

## License

GPL-3.0-only (follows OpenFOAM licensing).
