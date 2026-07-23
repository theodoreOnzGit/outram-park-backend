<!--
SPDX-License-Identifier: GPL-3.0-only
Part of Outram Park (outram-park-backend).
Independent Rust granular-DEM library. NOT a code port of GPL-2.0
LIGGGHTS/LAMMPS — see NOTICE for the licensing flag.
-->

# outram-park-fork-liggghts

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

> **Licensing (see [`NOTICE`](./NOTICE)).** LIGGGHTS-PUBLIC's source headers
> declare **"GNU Public License, version 2 or later"**, which **is compatible
> with GPL-3.0** (the "or later" option permits use under GPLv3) — so
> LIGGGHTS-PUBLIC source may be ported into this GPL-3.0-only crate. *(Corrects
> an earlier note that wrongly said "GPL-2.0-only / blocked".)* When porting,
> confirm the specific file's "or later" header and keep its attribution +
> provenance. LAMMPS-proper headers are version-unspecified (murkier) — treat
> those with care. Phase 1 is clean-room from public DEM literature regardless.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

The **DEM / granular-mechanics pillar** of the OUTRAM PARK Phase II architecture
(bead epic `op-t3l`), kept separate from the thermophysical-property pillar
(`tampines`) and the CFD / multiphase pillar (`outram-foam-multiphase`).

## Roadmap

| Phase | Module | Content | Bead | Status |
|---|---|---|---|---|
| 1 — Particle framework | `particle` | Particle state + explicit integration | `op-t3l.1` | In progress |
| 2 — Contact mechanics | — | Hooke, Hertz (`trait ContactModel`) | `op-t3l.2` | Planned |
| 3 — Boundaries | — | Plane, Wall, Box, Cylinder | `op-t3l.3` | Planned |
| 4 — Thermal DEM | — | Particle/particle + particle/wall heat transfer | `op-t3l.4` | Planned |
| 5 — CFD-DEM coupling | — | Reserve architecture only | `op-t3l.5` | Planned |

Definition of done for every physics deliverable: theory documentation +
verification tests + reference-benchmark comparison + unit-safe (`uom`)
implementation. Humans own physics, verification, validation, benchmarking, and
engineering judgement; AI only accelerates translation.
