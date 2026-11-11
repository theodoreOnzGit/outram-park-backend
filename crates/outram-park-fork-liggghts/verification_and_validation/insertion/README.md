# Particle insertion against LIGGGHTS-PUBLIC (gh:#1414, differential issue #1406 prio 3)

Translation of `fix insert/pack`, `fix insert/rate/region`, `fix
particletemplate/sphere`, `fix particledistribution/discrete` and the
`RanPark` generator (`src/insert.rs`), judged against the compiled upstream
(LIGGGHTS-PUBLIC `3d5c00f2`, `reference-data/liggghts/README.md` for the build
and the `%.17g` dump patch). Test: `tests/insertion_cross_code.rs`.

## Methodology

Five decks (`reference-data/liggghts/make_pour_decks.sh` writes `in.pour_*`) pour
into the `D/d = 6` cylinder of `in.pebble_bed` (R = 30 mm, floor, Hertz +
history, `E = 5e6`, `mu = 0.3`, `dt = 5 us`, `neighbor 0.003 bin`). Each is one
continuous LIGGGHTS run, frames written with `run N pre no` (no set-up in
between), `%.17g`, at steps 1, 1000, 1001, 6001, 10001, 20000, 24000, 26000,
30000 (`pour_*.csv`, reshaped by `pour2csv.sh`). The Rust test builds an empty
`GranularSystem` with `with_liggghts_neighbor_order`, attaches the translated
fix (`GranularSystem::add_insertion`, which runs it at `pre_exchange`: after the
first half-step, before the neighbour build and forces) and compares `x y z vx
vy vz omega` of every particle on bit patterns.

| deck | what it exercises |
|---|---|
| `pour_pack` | `insert/pack particles_in_region 200`, one template, `all_in yes`, `insert_every 1000`; first contacts near step 20 000 |
| `pour_poly` | two templates 70/30 mass %, mass-to-number weights, volume order, `volumefraction_region 0.2` (rounding draw), `vel uniform` (fix RNG every trial), neighbour cut of the largest *future* particle |
| `pour_rate` | `insert/rate/region nparticles 120 particlerate 8000` |
| `pour_block` | block region, `mass_in_region`, `numberbased`, `random_distribute uncorrelated`, `vel gaussian` (Box-Muller), `omega constant`, `all_in no` |
| `pour_nocheck` | `overlapcheck no` |

## Prediction (written before the first comparison run, 2026-11-11)

Everything bit-identical: insertion (same RNG stream, same operation order),
free flight, and, after contact, the contact sums (neighbour order already
translated, atoms appended at the end of the local list, rebuild and sort at
insertion steps as upstream). Fallback criterion if bit-identity broke: counts
equal and every field within 1e-8 of the field's largest magnitude.

## Result (2026-11-11, Rust 1.99.1, `cargo test --release`)

| deck | particles at the last frame | frames bit-identical (all particles, all 9 fields) |
|---|---|---|
| `pour_pack` | 287 | **9 / 9** |
| `pour_poly` | 577 | **9 / 9** |
| `pour_rate` | 120 | **9 / 9** |
| `pour_block` | 300 | **9 / 9** |
| `pour_nocheck` | 8 | **9 / 9** |

Particle counts equal at every frame; worst difference **0** (bit-identical) in
every field, through 30 000 steps including the contacts after ~20 000 and
insertions (up to 12 per step) into a pile already falling. The prediction is met.

## Notes and what is not covered

* **Order and RNG** come out right only because three things are translated, not
  approximated: `RanPark` (integer state, `(1/IM)*seed`), the region's own
  stream reset to `seed + 12` and *advanced by `ntry_mc` Monte-Carlo points* by
  `Region::volume_mc` before the first insertion, and the `ntry == maxtry`
  quirk that discards the last generated trial unchecked.
* **Overlap test**: upstream's `RegionNeighborList` is a bin grid answering a
  boolean; here a uniform grid with the same `rsq <= (r+r_j)^2` predicate. Same
  answer, different speed.
* **Lost atoms**: a particle leaving the box is deleted by upstream (`thermo_modify
  lost ignore`) and not by this crate; the decks keep every inserted particle
  inside the cylinder (a first `pour_block` with a region corner outside the
  R = 30 mm wall lost one atom upstream and was redesigned, not worked around).
* **Not translated**: ~~`fix insert/stream`~~ **CORRECTED 2026-11-11: translated, see below** (`/predefined` is not), multi-sphere and
  superquadric templates, `orientation random` (rejected: it would need
  `random_unit_quat`), `set_property`, `side out`/dynamic regions, radius/density
  distributions other than `constant` (upstream's `particletemplate/sphere`
  accepts only `constant` at this commit), multi-process
  `distribute_ninsert_this`, `region` types other than `block` and `cylinder`.
* The `D/d = 6` pour **is** the "Rust pour vs LIGGGHTS pour" comparison: with a
  bit-identical pour a separate phi / g(r) comparison would be a comparison of
  identical numbers. At HTR-10 scale (`htr10_fill`, 27 000 pebbles) the same
  fix works but has not been run against LIGGGHTS' 60 000-step `in.htr10` pour
  (maintainer: do not rerun the long tests).

## `fix insert/stream` (gh:#1414, 2026-11-11)

`src/insert_stream.rs`, `tests/insert_stream_cross_code.rs`, decks
`in.stream_ol` / `in.stream_noch` (`make_stream_decks.sh`, face
`stream_face.stl`: 0.04 m square at z = 0.15, two triangles; `r = 3 mm`,
`extrude_length 0.05`, `vel` 1 m/s into the face; insertions at steps 1, 10001,
20001, 30001). `stream_ol`: `overlapcheck yes`, `vel uniform`, 200 particles;
`stream_noch`: `overlapcheck no`, `vel constant 0.05 0 -1`, 100 particles.
Translated: the mesh's own `RanPark(179424799)` area-weighted facet choice and
barycentric draw, `calc_ins_fraction` (the `ntry_mc` draws that advance both
streams), `generate_random`, `is_nearby` (face projection test), both
`x_v_omega` branches, `finalize_insertion` (release step, per-particle velocity
draws) and `end_of_step` (rail motion, release, force/torque zeroing).

**Prediction (before the run):** bit-identical in every frame.
**Result (Rust 1.99.1, LIGGGHTS 3d5c00f2): 21 of 21 frames bit-identical on
every field of every particle** (11 frames `stream_ol`, 11 `stream_noch`; counts
50..200 and 25..100 per frame; the sums are in the test output), including
frames while particles ride the rail, at the release step, and in free fall.
Not translated: `all_in yes` (needs `TriMeshPlanar::generateRandomOwnedGhostWithin`),
`parallel yes`, `duration_time`, tracers, multi-sphere, a moving face (upstream
rejects it too), `insert/stream/predefined`. Particles do not reach the floor
by step 40 000 in these decks, so the stream-then-contact path is only covered
through the already verified pair/wall translation.
