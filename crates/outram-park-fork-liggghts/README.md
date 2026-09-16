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

## Verification & validation

Full methodology and measured results:
[`docs/verification-and-validation.md`](docs/verification-and-validation.md).

Upstream **LIGGGHTS-PUBLIC was built from source and run**; its trajectories are
committed under `reference-data/liggghts/` so the comparison regenerates rather
than being trusted. Measured 2026-09-15, every sampled frame compared:

| Case | Frames | agreement with LIGGGHTS |
|---|---|---|
| head-on collision, Hertz | 251 | **bit-identical** |
| head-on collision, Hooke | 251 | **bit-identical** |
| wall bounce + gravity (400 000 steps) | 2001 | **bit-identical** |
| rolling resistance, CDT (`µ_r = 0.1`) | 201 | **bit-identical** |
| oblique + friction (shear history, slip, torque) | 251 | round-off (1-3 ulp) |
| oblique, no-history — the stateless `contact`+`simulation` path | 251 | round-off (1-3 ulp) |
| bulk bed, 354 pebbles, `D/d = 6` | settled | packing fraction within **0.20 %** |
| angle of repose, 656 pebbles, lifting cylinder | settled heap | `12.78°` vs `15.43°` |

```bash
cargo test --release -p outram-park-fork-liggghts --lib                     # 108 unit tests
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code # 5 cross-code
cargo test --release -p outram-park-fork-liggghts --test legacy_path_cross_code # stateless path
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk -- --ignored
cargo test --release -p outram-park-fork-liggghts --test angle_of_repose -- --ignored
```

**This is verification, not validation.** It shows this crate reproduces
LIGGGHTS; it does **not** show LIGGGHTS' granular physics is right for a pebble
bed. There is still **no experimental comparison** in this repository, and both
bookkeeping axes above remain unsigned.

The **DEM / granular-mechanics pillar** of the OUTRAM PARK Phase II architecture
(bead epic `op-t3l`), kept separate from the thermophysical-property pillar
(`tampines`) and the CFD / multiphase pillar (`outram-foam-multiphase`).

## Roadmap

| Phase | Module | Content | Bead | Status |
|---|---|---|---|---|
| 1 — Particle framework | `particle` | Particle state + explicit integration | `op-t3l.1` | Foundation done |
| 2 — Contact mechanics | `contact` | Hooke, Hertz-Mindlin (enum dispatch) | `op-t3l.2` | Foundation done |
| 3 — Boundaries | `boundary` | Plane, Wall, Box, Cylinder | `op-t3l.3` | Foundation done |
| 4 — Thermal DEM | `thermal` | Particle/particle + particle/wall heat transfer | `op-t3l.4` | Foundation done |
| 5 — CFD-DEM coupling | `coupling` | Reserve architecture only | `op-t3l.5` | Reserved arch |

Phases 1-4 are **clean-room, unit-tested foundations, not benchmark-validated**
(validation is a later human step).

Definition of done for every physics deliverable: theory documentation +
verification tests + reference-benchmark comparison + unit-safe (`uom`)
implementation. Humans own physics, verification, validation, benchmarking, and
engineering judgement; AI only accelerates translation.
