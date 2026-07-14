# outram-foam-turbulence-lib

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
> `upstream_source/README.md` for the full provenance record.

Pure-Rust port of the OpenFOAM turbulence model library, part of the
**OUTRAM PARK** (Open-source TRAnsient Multi-Phase Advanced Reactor simulator
Kit) suite.

Provides RAS (Reynolds-Averaged Simulation) and LES (Large Eddy Simulation)
turbulence closures for use with `outram-foam-appbuilder-lib` solver loops.

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
