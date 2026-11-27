<!--
SPDX-License-Identifier: AGPL-3.0-only
Part of Outram Park (outram-park-backend).
Independent Rust granular-DEM library. ~~NOT a code port of GPL-2.0
LIGGGHTS/LAMMPS~~ CORRECTED 2026-11-27: partly a port. Some files are
clean-room from the DEM literature (e.g. src/contact.rs says so in its
header); others carry a "PORTED FROM UPSTREAM" LIGGGHTS-PUBLIC provenance
header (e.g. src/timestep.rs, src/multisphere.rs, src/superquadric.rs).
LIGGGHTS-PUBLIC is GPL-2.0-or-later. See NOTICE for the licensing flag.
-->

# outram-park-fork-liggghts

**Name.** `outram-park-fork-liggghts` is an OUTRAM PARK fork, named for its
upstream, LIGGGHTS-PUBLIC; the backronym is OUTRAM PARK's own, letter by
letter: **O**pen-source **U**nified **TRA**nsient **M**ulti-Physics
**A**dvanced **R**eactor simulation **K**it.

**The question it answers:** how do granular particles (here, chiefly the
pebbles of a pebble-bed reactor) pack, flow, rub and conduct heat, computed by
discrete-element mechanics that reproduce LIGGGHTS-PUBLIC?

**What this crate makes studyable and challengeable.** The contact laws
(Hooke, Hertz-Mindlin, rolling resistance, cohesion), the integrator, mesh
walls and insertion, in Rust a reader can set beside LIGGGHTS-PUBLIC's C++,
with upstream's own trajectories committed under `reference-data/liggghts/` so
every comparison can be regenerated instead of believed. Its V&V record shows
the process in full: the HTR-10 packing fraction was missed by 8.7 %, then
closed to 0.9 % by ablating friction across the graphite literature range
rather than tuning it, and a reproducibility defect was found on the way. It is
verification against another code, not evidence that the granular physics is
right for a real bed.

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

> **Licensing (see [`NOTICE`](./NOTICE)).** LIGGGHTS-PUBLIC's source headers
> declare **"GNU Public License, version 2 or later"**, which **is compatible
> with GPL-3.0** (the "or later" option permits use under GPLv3) — so
> LIGGGHTS-PUBLIC source may be ported into this ~~GPL-3.0-only~~ **AGPL-3.0-only** crate (relicensed 2026-10-20, gh:#973). *(Corrects
> an earlier note that wrongly said "GPL-2.0-only / blocked".)* When porting,
> confirm the specific file's "or later" header and keep its attribution +
> provenance. LAMMPS-proper headers are version-unspecified (murkier) — treat
> those with care. Phase 1 is clean-room from public DEM literature regardless.

## `no_std` + `alloc` (gh:#1602)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-park-fork-liggghts --lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). The DEM itself (particles, contact
models, walls, meshes, insertion, integration, thermal and capillary
models, the `htr10_fill` pour) and the `gnn` bridge build without it. What
`std` adds:

- `rayon`. Without it the parallel loops run serially through `wasm_par`,
  as on wasm, and `ThreadCount` resolves to one core.
- the GPU path (`gpu`, `wgpu`).
- files and printing.

The root workspace entry is `default-features = false`, so a member that
wants `std` asks for it. With `std` on (the default) nothing changes.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream: LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz, GPL-2.0-or-later), commit `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` (2024-06-07), provenance in `upstream_source/README.md` and `docs/verification-and-validation.md` section 1. HTR-10 design point from IAEA-TECDOC-1382 (`docs/htr10-bed-geometry-and-positions.md`). Signed-distance functions after I. Quilez, "Distance functions" (`AGENTS.md`). Clean-room modules cite their DEM literature in a "References" block. No literature in `crates/kovan-literature/CATALOGUE.md` is tied to this crate. |
| Mathematical equations | Module docs of `src/contact.rs` (Hooke, Hertz-Mindlin), `src/rolling.rs` (CDT, EPSD), `src/capillary.rs` (liquid-bridge cohesion), `src/thermal.rs`, `src/heat_gran.rs`, `src/thermal_radiation.rs`, `src/integrator.rs`, `src/timestep.rs` (Rayleigh / Hertz time-step estimate), `src/boundary.rs` (signed distances); `docs/verification-and-validation.md` sections 4.1 to 4.11. |
| Source code | `src/particle.rs`, `src/contact.rs`, `src/rolling.rs`, `src/granular_system.rs` and `src/simulation.rs` (the two engines), `src/boundary.rs`, `src/mesh_wall.rs` and `src/tri_mesh.rs`, `src/insert.rs` and `src/insert_stream.rs`, `src/multisphere.rs`, `src/superquadric.rs`, `src/bonded.rs`, `src/thermal.rs`, `src/compute.rs` (scalar / threads), `src/gpu/`, `src/htr10_fill.rs`, `src/rdf.rs`, `src/gnn_bridge.rs`, `src/coupling.rs` (reserved seam, no physics), `src/prelude.rs`. |
| Interactive simulations | Browser: the HTR-10 pour runs in `dhoby-ghaut`'s `dem_web` since 2026-10-07 (gh:#787, per `AGENTS.md`). CLI examples in `examples/` (e.g. `htr10_fresh_fill.rs`, `pebble_bed.rs`, `htr10_recirculation_sweep.rs`, `gpu_pour_compare.rs`). |
| Visualisations | None committed in this crate. `examples/bed_rdf_check.rs` prints `g(r)` diagnostics as text. |
| Verification and validation evidence | `docs/verification-and-validation.md` and `docs/cross-code-summary.md`, cross-code against LIGGGHTS-PUBLIC built from source: contact cases bit-identical on every sampled frame (2026-09-15, oblique cases bit-identical since 2026-11-10); bulk bed of 354 pebbles 0.20 %; HTR-10 full core of 27 554 pebbles `phi = 0.5732` vs `0.5732`, median pebble 61 um apart (2026-09-17); HTR-10 conus slump all 27 554 pebbles within 1 mm at step 2 000 (median 11 um); angle of repose 14.09 vs 15.43 degrees (2026-09-18). Friction ablation (section 4.9, 2026-09-17): whole-core `phi` 0.6047 against the benchmark's quoted 0.61 at `mu = 0.1`, `mu_r = 0`. Tests: `tests/*_cross_code.rs`. No experimental comparison. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. Struck-through corrections in `AGENTS.md` and `docs/verification-and-validation.md` (the angle of repose 12.78 degrees, from a nondeterministic build, corrected to 14.09; the oblique-contact round-off rows corrected to bit-identical; defects in sections 4.1 to 4.3). |

**Status:** declared mature by the maintainer on 2026-09-06 for its API, not
its physics (`AGENTS.md`). The contact laws, integrator, mesh walls and the
HTR-10 beds have machine-run cross-code evidence against LIGGGHTS-PUBLIC; none
of it is human-reviewed, there is no experimental comparison, and the CFD-DEM
`coupling` module holds no physics.

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
| oblique + friction (shear history, slip, torque) | 251 | ~~round-off (1-3 ulp)~~ **bit-identical** (CORRECTED 2026-11-10) |
| oblique, no-history — the stateless `contact`+`simulation` path | 251 | ~~round-off (1-3 ulp)~~ **bit-identical** (CORRECTED 2026-11-10) |
| oblique Hooke + history + CDT rolling | 251 | **bit-identical** (2026-11-10) |
| two atom types, head-on and oblique | 251 each | **bit-identical** (2026-11-10) |
| bulk bed, 354 pebbles, `D/d = 6` | settled | packing fraction within **0.20 %** |
| angle of repose, 656 pebbles, lifting cylinder | settled heap | ~~`12.78°` vs `15.43°`~~ **`14.09°` vs `15.43°`** (**CORRECTED 2026-11-27**, from `AGENTS.md`: corrected there on 2026-09-18, 12.78° came from a nondeterministic build and is not reproducible) |
| **HTR-10 full core**, 27 554 pebbles, `D/d = 30` | settled | packing fraction to **4 decimals**; median pebble **61 µm** apart |
| **HTR-10 conus slump**, 27 554 pebbles, **STL mesh** conus + discharge tube | 4 checkpoints | packing fraction to **0.01 %**; **all 27 554 pebbles within 1 mm** at early time (median 11 µm) |

```bash
cargo test --release -p outram-park-fork-liggghts --lib                      # unit tests
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code # cross-code
cargo test --release -p outram-park-fork-liggghts --test legacy_path_cross_code # stateless path

# fast analysis + backend tests (seconds)
cargo test --release -p outram-park-fork-liggghts --test htr10_rdf                   # g(r) structure
cargo test --release -p outram-park-fork-liggghts --test compute_backend_equivalence # bit-identity

# the long cases: gated behind the default-on `long-tests` feature, so
# they run in an ordinary `cargo test` and are skipped only under
# `--no-default-features`. They do NOT need `-- --ignored`.
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk         #  317 s
cargo test --release -p outram-park-fork-liggghts --test angle_of_repose         # 2313 s
cargo test --release -p outram-park-fork-liggghts --test htr10_pebble_bed        # 2714 s
cargo test --release -p outram-park-fork-liggghts --test htr10_conus_cross_code  #  787 s
cargo test --release -p outram-park-fork-liggghts --test htr10_recirculation     # see V&V 4.9
```

~~117 unit tests, 5 cross-code~~ **CORRECTED 2026-11-27** — counts removed
from the commands above: on 2026-11-27 `src/` holds 201 `#[test]` attributes
and `tests/liggghts_cross_code.rs` holds 13 (counted with `grep`; `cargo` was
not run, so the number that run on a given target is *not re-checked*).

Runtimes above were measured on **two different hosts and before/after the
2026-09-17 performance work**, which cut the HTR-10 timestep from 55.95 ms to
12.85 ms — so they are not comparable with each other. Re-measure rather than
trusting them.

**Choosing a compute backend.** The timestep runs scalar by default. For a large
bed, opt into threads — the parallel path is **bit-identical**, so this cannot
change a result, only the wall clock:

```rust
use outram_park_fork_liggghts::compute::{ComputeType, ThreadCount};
let sys = GranularSystem::new(particles, boundaries, model, gravity, dt)?
    .with_compute(ComputeType::CpuMultiThread(ThreadCount::Auto));
```

**This is verification, not validation.** It shows this crate reproduces
LIGGGHTS; it does **not** show LIGGGHTS' granular physics is right for a pebble
bed. There is still **no experimental comparison** in this repository, and both
bookkeeping axes above remain unsigned.

That caveat applies in full to the packing-fraction work of V&V § 4.9, which
reaches the HTR-10 benchmark's published filling fraction of 0.61 at
graphite-literature friction. Reaching a **quoted specification figure** is not
experimental validation, and the benchmark records that 0.61 as quoted rather
than measured.

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

~~Phases 1-4 are **clean-room, unit-tested foundations, not benchmark-validated**
(validation is a later human step).~~ **CORRECTED 2026-11-27** — phases 1 to 4
are no longer only unit-tested foundations: they are cross-code verified
against LIGGGHTS-PUBLIC (the table above), and the crate has grown well past
this roadmap (mesh walls, insertion, multisphere, superquadric, bonds,
capillary cohesion, radiation, GPU and threaded backends, the HTR-10 fill; see
`src/lib.rs` and `AGENTS.md`). Not all of it is clean-room: several modules
are ported from LIGGGHTS-PUBLIC. They are still **not validated** against
experiment (validation is a later human step). Phase 5, `coupling`, is still
reserved architecture only (verified in `src/coupling.rs`).

Definition of done for every physics deliverable: theory documentation +
verification tests + reference-benchmark comparison + unit-safe (`uom`)
implementation. Humans own physics, verification, validation, benchmarking, and
engineering judgement; AI only accelerates translation.
