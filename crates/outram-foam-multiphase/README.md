<!--
SPDX-License-Identifier: GPL-3.0-only
Part of Outram Park (outram-park-backend).
A Rust translation of selected OpenFOAM multiphase solvers. Independent fork,
not the official OpenFOAM software; see the workspace TRADEMARKS.md.
-->

# outram-foam-multiphase

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

**OUTRAM-FOAM Phase II — multiphase CFD** (bead epic `op-2kk`). Pure-Rust
translation of OpenFOAM's multiphase solver family on top of
[`outram-foam-basic-lib`](../outram-foam-basic-lib)'s finite-volume framework.
This is the **authoritative high-fidelity reference** from which TAMPINES' 1D
reduced-order system-code physics (epic `op-dt3`) are derived — 1D models must
trace back to a validated 3D reference here, never be invented independently.
Independent OUTRAM PARK fork, not the official OpenFOAM (see `TRADEMARKS.md`).

## Roadmap

| Stage | Module | OpenFOAM ref | Bead | Status |
|---|---|---|---|---|
| 1 — Drift Flux | `drift_flux` | `incompressibleDriftFlux` | `op-2kk.1` | In progress |
| 2 — Euler-Euler two-fluid | — | `twoPhaseEulerFoam` | `op-2kk.2` | Planned |
| 3 — Wall boiling framework | — | OF wall boiling | `op-2kk.3` | Planned |
| 4 — CHF models | — | CHFModel/CHFSubCoolModel | `op-2kk.4` | Planned |
| 5 — Dryout / post-dryout | — | — | `op-2kk.5` | Planned |

Definition of done for every solver: theory documentation + verification tests
+ reference-benchmark comparison + unit-safe (`uom`) implementation. Humans own
physics, verification, validation, benchmarking, and engineering judgement; AI
only accelerates translation.
