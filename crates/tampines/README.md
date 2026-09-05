# TAMPINES

**T**hermal-hydraulic **A**rtificial-intelligence **M**ulti-**P**hase
**IN**tegrated **E**mulator **S**ystem — the central thermal-hydraulic
framework of the [OUTRAM PARK](https://github.com/theodoreOnzGit/outram-park-backend)
suite.

> ⚠️ **Scaffold only — version 0.0.1 is a placeholder release.** This crate is
> being built out incrementally and is **not** ready for use. It is published to
> reserve the name and to let downstream OUTRAM PARK crates resolve against
> crates.io. See the `op-dt3` epic in the workspace issue tracker for the live
> module plan and progress.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified
> and untrusted** unless a specific verification & validation (V&V) case
> demonstrates otherwise. See the workspace `VERIFICATION_AND_VALIDATION.md` and
> `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control,
> safety-critical, or licensing decisions.

## What it is

TAMPINES owns all fluid flow, thermal-hydraulics, thermophysical properties,
heat transfer, balance-of-plant components, humid-air psychrometrics, and
multiphase thermal-hydraulics for OUTRAM PARK.

It is distinct from **`tampines-steam-tables`**, which is only the IAPWS-IF97
property library — one of the backends TAMPINES composes.

## What it composes

| Piece | Provided by | Role |
|---|---|---|
| Single-phase liquid thermal-hydraulics | `tuas_boussinesq_solver` | Boussinesq single-phase pipe/component flow |
| Compressible / two-phase properties | `outram-park-fork-coolprop` | CoolProp-derived thermophysical properties |
| IAPWS-IF97 steam/water properties | `tampines-steam-tables` | Steam-turbine and choked-flow equations |
| Finite-volume building blocks | `outram-foam-basic-lib` | Tensor algebra, ODE/polynomial solvers, FV operators |
| Process control | `chem-eng-real-time-process-control-simulator` | PID / transfer-function control loops |
| Equipment-model correlations | `outram-park-fork-dwsim-libs` | Pipe/valve/heat-exchanger/expander/pump sizing and rating equations |

## What belongs here, and what does not

- **Belongs here:** fluid-flow and thermal-hydraulic component models (pipes,
  pumps, valves, heat exchangers, steam generators, turbines, condensers,
  cooling towers), balance-of-plant composition, humid-air psychrometrics, and
  multiphase thermal-hydraulics (HEM, drift-flux, CHF).
- **Does not belong here:** raw property-table equations (those live in
  `tampines-steam-tables` / `outram-park-fork-coolprop`), reactor physics
  (`teh-o-prke`, `outram-mc-libs`, `njoy-outram-park-fork`), or GUI /
  visualization code (`outram-park-digital-twin-engine`).

## Intended use

Education, research, capability building, and verification/validation only. This
crate is **not** for nuclear facility operation, reactor control, licensing
decisions, safety-critical decision-making, emergency response,
safeguards-sensitive analysis, or real-time plant monitoring.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0-only. The full licence text ships with this crate as `LICENSE`.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore,
Ethan Yew Hoe, Wong,
Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
