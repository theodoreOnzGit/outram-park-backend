# Cross-code summary — `outram-park-fork-liggghts` vs upstream LIGGGHTS-PUBLIC

> **Status: verification, not validation.** Everything below establishes that
> this crate computes what LIGGGHTS computes. None of it compares either code
> against an experiment. Per
> [`VERIFICATION_AND_VALIDATION.md`](../../../VERIFICATION_AND_VALIDATION.md),
> verification asks "implemented correctly?" and validation asks "represents
> physical reality well enough?" — only the first is answered here. Two codes
> agreeing can be two codes wrong together; see "What this does NOT cover".
>
> Numbers are measured, not estimated. Each row names the test that produces
> it and the upstream file it is judged against. Narrative, defect histories
> and the reasoning behind each case live in
> [`verification-and-validation.md`](./verification-and-validation.md); this
> file is the one place that carries every case in a single table.

## Method

The upstream code was **cloned, compiled and executed** — not quoted from its
documentation — and its output committed under
[`reference-data/liggghts/`](../../../reference-data/liggghts/) together with
the input decks and the two conversion scripts, so the comparison regenerates
rather than being trusted. Same standard `petir` is held to against GSL 2.8.

| | |
|---|---|
| Upstream | LIGGGHTS-PUBLIC, `github.com/CFDEMproject/LIGGGHTS-PUBLIC` |
| Commit | `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` ("Remove logos", 2024-06-07) |
| Internal version | `LAMMPS_VERSION "23 Nov 2013"` (its LAMMPS base) |
| Build | `make stubs` then `make serial`; g++ 15.2.1, `-O2 -fPIC`, MPI stubs, no VTK |
| Host | x86-64 Linux, Intel Xeon @ 2.80 GHz, 4 cores |
| Dates run | 2026-09-15 (deterministic + bulk), 2026-09-17 (HTR-10 at `E = 5e8`) |

**The precision patch is load-bearing.** Stock LIGGGHTS writes `dump custom`
fields with `"%g "` — six significant figures — at which a comparison saturates
at ~`1e-6` relative and cannot distinguish "this port agrees with LIGGGHTS"
from "this port agrees with LIGGGHTS' `printf`". Two lines in
`src/dump_custom.cpp` print full `double` instead. A third change was forced by
the first: `format_default` is allocated `3*size_one+1` bytes, which the wider
format strings overrun, and the run segfaulted until it was enlarged to
`8*size_one+1`. All three are recorded in the reference-data README.

**"Bit-identical"** means the IEEE-754 bit patterns match exactly, on **every
sampled frame of the trajectory**, not merely at the endpoint. Where they do
not match, the worst absolute difference is reported with the ulp it
corresponds to.

### Why whole trajectories rather than final states

A settled endpoint is a weak discriminator: two different contact models can
reach the same resting configuration, and a comparison of final states would
pass against a port that had silently substituted one. The trajectory is what
identifies the model actually implemented. The same reasoning is why the two
head-on cases are kept separate below — Hertz and Hooke land on *different*
realised restitutions (`0.900007` vs `0.899972`) from the same requested
`e = 0.9`, and each code reproduces its own model's value, so the agreement
cannot be an artefact of both converging on the requested number.

## Deterministic cases — full-trajectory comparison

| Case | Upstream model file | Frames | `max abs dx` [m] | `max abs dv` [m/s] | `max abs domega` [rad/s] | Test |
|---|---|---|---|---|---|---|
| head-on, Hertz | `normal_model_hertz.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| head-on, Hooke | `normal_model_hooke.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| wall bounce + gravity, 400 000 steps | `fix_wall_gran.cpp`, `fix_wall_gran_base.h` | 2001 | `0` | `0` | — | `liggghts_cross_code.rs` |
| rolling, counter-spinning pair | `rolling_model_cdt.h` | 201 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| oblique + friction | `tangential_model_history.h`, `surface_model_default.h` | 251 | `0` | ~~`1.11e-16`~~ **`0`** | ~~`5.68e-14`~~ **`0`** | `liggghts_cross_code.rs` |
| oblique, no-history (stateless path) | `tangential_model_no_history.h` | 251 | `0` | ~~`1.11e-16`~~ **`0`** | ~~`1.42e-14`~~ **`0`** | `legacy_path_cross_code.rs` |
| oblique **Hooke** + history + CDT rolling (new 2026-11-10) | `normal_model_hooke.h`, `tangential_model_history.h`, `rolling_model_cdt.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| **two atom types**, head-on (new 2026-11-10): `E` 1e7/3e7, `ν` 0.3/0.2, `e` and `µ` matrices, `d` 10/12 mm, `ρ` 2500/4000 | `global_properties.cpp` `createYeff`/`createBetaEff`, `normal_model_hertz.h` | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| **two atom types**, oblique (new 2026-11-10) | adds `createGeff`, `createCoeffFrict`, unequal-radius torque | 251 | `0` | `0` | `0` | `liggghts_cross_code.rs` |
| **rotating mesh plate** (new 2026-11-11) | `tri_mesh_I.h` `resolveTriSphereContactBary`, `mesh_mover_rotation.cpp`, `fix_wall_gran.cpp` `post_force_mesh` | 251 | `0` | `0` | `0` | `rotating_mesh_cross_code.rs` |
| EPSD, EPSD2, EPSD3 rolling; Luding normal+tangential+rolling; Thornton-Ning; Edinburgh (+stiffness, fixKc); SJKR, SJKR2 cohesion; hooke/hysteresis wall (new 2026-11-11, gh:#1410 #1411; V&V section 4.11) | `rolling_model_epsd*.h`, `*luding*.h`, `normal_model_thornton_ning.h`, `normal_model_edinburgh*.h`, `cohesion_model_sjkr*.h` | 251 | `0` | `0` | `0` | `liggghts_contact_models_cross_code.rs` |
| **rotating drum**, 64 facets, two spheres (new 2026-11-11) | + edge/corner activity (`surface_mesh_I.h`) | 251 | `0` | `0` | `0` | `rotating_drum_cross_code.rs` |

~~**Four of the six are bit-identical over the whole trajectory.** The two
oblique cases agree to round-off: `1.11e-16 m/s` is one ulp at `|v| ~ 1 m/s`,
and `5.68e-14 rad/s` is about 3 ulp at the final `omega_z = -83.1815 rad/s`.
That residual is the expected consequence of summing identical terms in a
different association order, not a modelling difference.~~

**CORRECTED 2026-11-10 (gh:#1406): all nine deterministic rows are
bit-identical, every `f64` of every frame** (x, v and ω: 251 frames x 2 atoms
x 9 values = 4518 per case). The "association order" explanation was right in
kind and is now removed at its source. Matching the port to upstream operation
by operation found these lines (the old port differed from upstream at each):

| Port (before) | Upstream (after) | Upstream source |
|---|---|---|
| `omega += t * (half * (1/(0.4 m r²)))` | `dtirotate = (dtf/INERTIA)/(r*r*m); omega += dtirotate*torque` | `fix_nve_sphere.cpp`, `initial_integrate`/`final_integrate` |
| slip rescale of stored shear `ft * (-1/kt)` | `shear = -Ft/kt` | `tangential_model_history.h`, slip branch |
| no-history: rescale `Ft` by `µ|Fn|/|Ft|` when over the cap | cap the dashpot coefficient: `gamma = (Ft_friction < gammat*vrel) ? Ft_friction/vrel : gammat; Ft = -gamma*vtr` | `tangential_model_no_history.h` |
| `Y_eff = E/(2(1-ν²))`, `G_eff = E/(4(2-ν)(1+ν))` | `1/((1-νi²)/Ei + (1-νj²)/Ej)`, `1/(2(2-νi)(1+νi)/Ei + 2(2-νj)(1+νj)/Ej)` | `global_properties.cpp` `createYeff`, `createGeff` |
| wall branch `wr = ω * (cr * rinv)` | `wr = cr*ω*rinv`, i.e. `(cr*ω)*rinv` | `surface_model_default.h` (not exercised by a deterministic deck here) |

The stateless `contact` + `simulation` Hertz path no longer keeps its own copy
of the arithmetic: `HertzContact::contact_force` delegates to the engine path's
translation (`ContactKinematics::pair` + `GranularNormalModel::Hertz` +
`TangentialModel::NoHistory`), so the two engines cannot drift.

**Prediction written before the new runs (2026-11-10):** Hooke + history + CDT
rolling and the two-type cases bit-identical, because each piece
(head-on Hooke, history, CDT, per-pair `Y_eff`/`G_eff`/`β_eff`) is a
line-for-line translation. **Result: confirmed, 4518/4518 values bit-identical
in each of the three new cases** (`liggghts_cross_code.rs`,
`oblique_hooke_history_rolling_matches_liggghts`,
`two_type_headon_matches_liggghts`, `two_type_oblique_matches_liggghts`).
Reference decks: `reference-data/liggghts/in.oblique_hooke_rolling`,
`in.twotype_headon`, `in.twotype_oblique`, built with the recipe in
`crates/outram-park-fork-liggghts/upstream_source/README.md`. Toolchain:
Rust 1.99.1, g++ 15.2.1 for upstream.

**Rotating mesh wall (gh:#1406 prio 2, gh:#1416): CORRECTED 2026-11-11, now
bit-identical.** ~~NOT bit-identical: 1915 of 2259 values differed
(`max|Δx| = 5.6e-15 m`, `max|Δv| = 8.1e-12 m/s`, `max|Δω| = 1.1e-9 rad/s`);
the mesh contact path was not translated.~~ The mesh contact path is now a
translation of `TriMesh::resolveTriSphereContactBary` (face, edge and corner
regions, inactive edges and corners skipped, barycentric node velocity), the
surface-model wall branch (`r = radi - deltan`, `en = delta*rinv`,
`cr = radi - 0.5*deltan`), `fix move/mesh rotate` (incremental quaternion
rotation of the nodes **and of the cached edge vectors and normals**, node
velocities zero until the first `initial_integrate`) and the neighbour-rebuild
schedule that decides when upstream recomputes those cached vectors from the
nodes (`GranularSystem::with_neighbor_skin`). Measured 2026-11-11, Rust 1.99.1:

| Case | Frames | values bit-identical / total | Test |
|---|---|---|---|
| rotating square plate, one sphere (`in.rotating_plate`) | 251 | **2259 / 2259** | `rotating_mesh_cross_code.rs` |
| rotating drum, 64-facet closed cylinder (side quads, centre-fan end caps), two spheres that touch the drum and each other (`in.rotating_drum`, `make_drum_stl.py`; upstream made 83 neighbour builds) | 251 | **4518 / 4518** | `rotating_drum_cross_code.rs` |

Both predictions (bit-identical) were written before the run and are met.

**Particle insertion (gh:#1414, gh:#1406 prio 3), 2026-11-11.** `fix insert/pack`,
`insert/rate/region`, `particletemplate/sphere`, `particledistribution/discrete`
and `RanPark` are translated (`src/insert.rs`). Five pour decks into the `D/d = 6`
cylinder, 9 frames each to step 30 000 (contacts after ~20 000, up to 12
insertions per step into a falling pile): **every field of every particle
bit-identical in 45 / 45 frames** (`insertion_cross_code.rs`;
`verification_and_validation/insertion/README.md`). `fix insert/stream` (`src/insert_stream.rs`, `insert_stream_cross_code.rs`): 21 / 21 frames bit-identical (overlapcheck yes and no, `vel uniform`/`constant`). Not translated: `insert/stream/predefined`.

**Per-contact dump and `fix check/timestep/gran` (gh:#1406 prio 2, gh:#1418),
2026-11-11.** `compute pair/gran/local` is translated as
`GranularSystem::pair_gran_local_for` / `pair_gran_local`. Deck
`in.pgl_pebble`, the settled `D/d = 6` bed (354 pebbles) under the cylinder,
floor and gravity, dumped with `%.17g`:

Measured **with the default cell-grid pair order** (before the neighbour-order
translation below; superseded, kept as the ablation record):

| Step | contacts | values bit-identical / total (23 per contact) | worst `|d|/max|field|` | `pair_gran_local_cross_code.rs` |
|---|---|---|---|---|
| 0 (set-up) | 793 | **18239 / 18239** (793/793 contacts) | 0 | every field of every contact |
| 1 | 793 | 14799 / 18239 (232 contacts fully) | `3.1e-16` | ulp-level |
| 10 | 793 | 14440 / 18239 (147 contacts fully) | `2.6e-16` | ulp-level |
| 100 | 792 | 11950 / 18216 (6 contacts fully) | `9.0e-16` | ulp-level |

~~The step-1 "bit-identical" prediction was **not met** beyond step 0: the
remaining differences are 1-3 ulp and occur only on particles with three or
more pair contacts, the signature of the per-particle force sum being
accumulated in a different order (upstream: bin-stencil neighbour-list order;
port: cell-grid order). Making the sums bit-identical needs the LIGGGHTS
bin-stencil order translated (open, filed on gh:#1418).~~

**CORRECTED 2026-11-11 (gh:#1406, #1418): translated, and now bit-identical at
every step.** `src/neighbor_list.rs` translates `Atom::setup_sort_bins` /
`Atom::sort` (the spatial atom sort at set-up and every 1000 steps),
`Neighbor::setup_bins`, `bin_atoms`, `coord2bin`, `stencil_half_bin_3d_no_newton`
and `granular_bin_no_newton` (the half list: `i` ascending, stencil bins in
`k, j, i` order, each bin ascending), with the stale-between-rebuilds list life
(`check_distance`, `neigh_modify delay 0 every 1 check yes`);
`GranularSystem::with_liggghts_neighbor_order(LiggghtsNeighbor)` selects it
(pairs are then visited in sorted-local orientation, the shear store flipping
the pair around each evaluation, as `FixContactHistory` does with `-hist`).
**Prediction** (written before the run): every field of every contact
bit-identical at steps 0, 1, 10, 100. **Result:**

| Step | contacts | values bit-identical / total | ablation: default cell grid | ablation: LIGGGHTS list, no atom sort |
|---|---|---|---|---|
| 0 | 793 | **18239 / 18239** | 0 differ | 0 differ |
| 1 | 793 | **18239 / 18239** | 3440 differ | 4442 differ |
| 10 | 793 | **18239 / 18239** | 3799 differ | 4923 differ |
| 100 | 792 | **18216 / 18216** | 6266 differ | 6831 differ |

(`pair_gran_local_cross_code.rs`, Rust 1.99.1.) The ablations show that both
ingredients matter: the list order and the set-up atom sort. The option is
**named, not the default**, because the order depends on the deck's box and
skin, which the library cannot know; the rayon backend stays bit-identical to
the scalar one under it (`compute_backend_equivalence.rs`,
`parallel_is_bit_identical_to_serial_with_liggghts_neighbor_order`). Not
translated: `newton on`, periodic/shrink-wrapped boxes (ghost atoms), triclinic
boxes, `neighbor multi`, `neigh_modify exclude`; see `neighbor_list.rs`.

**Heat conduction (gh:#1412, 2026-11-11).** `heat_gran.rs` translates
`fix heat/gran/conduction` (contact area `overlap`/`constant`/`projection`,
the conductance `4 k_i k_j/(k_i+k_j) sqrt(A)`, the explicit
`advanceQtyExplicit` update); the clean-room `thermal.rs` stays callable.
`heat_conduction_cross_code.rs`, decks `in.heat_pair`, `in.heat_bed`,
`dump2heat.sh`:

| Case | quantity | values bit-identical / total |
|---|---|---|
| two spheres at `T = 400/300 K`, 251 frames | `T`, heat flux of both atoms | **1004 / 1004** |
| settled `D/d = 6` bed, 354 pebbles, `T = 300 + 200 z/0.35`, steps 0, 1, 10, 100, LIGGGHTS order | `T`, heat flux | **0 of 708 differ at each step** |
| same bed, default cell-grid order (ablation) | `T`, heat flux | 63 / 61 / 63 of 708 differ at steps 1/10/100, worst relative `1.1e-13` (sum order) |

Clean-room against translated: upstream `hc = 2 sqrt(pi) k_s a` with the
geometric chord radius, clean-room `2 k_s a_c` with `a_c = sqrt(R* delta)`:
measured flux ratio **2.4813** at `delta = 0.4 mm`, `r = 5 mm`, against the
closed form `sqrt(2 pi (1 - delta/(4 r))) = 2.4815`. The test's first-written
prediction (`sqrt(2 pi) = 2.507` within "< 1 %") was **wrong** (the measured
value is 1.01 % low; the exact chord radius carries the `delta/(4r)` term) and
the test header says so.

**Stiffness and hysteresis contact models (gh:#1410, 2026-11-11).**
`GranularNormalModel::{HertzStiffness, HookeStiffness, HookeHysteresis}`,
oblique collisions, 251 frames of x, v, omega each: see the table in
`verification-and-validation.md` section 4.10 for the measured counts.

`fix check/timestep/gran` (`in.timestep_pebble`, `in.timestep_htr10`,
`timestep_check.txt`): Rayleigh, Hertz and skin fractions at step 1,
**6 / 6 bit-identical** (`timestep_cross_code.rs`; `timestep.rs` gained
`skin_fraction` and `estimate_with_walls`, upstream's mesh-speed and `vmax`
inputs).

The two-type cases use `MaterialTable` (new, `granular.rs`): per-type `E`, `ν`
and symmetric per-pair `e` and `µ` matrices, with upstream's `createYeff` /
`createGeff` / `createBetaEff` mixing rules, selected per contact by
`GranularSystem::with_particle_types`. At most 4 types; wall contacts use the
particle's own type (no separate wall material).

The last row verifies a **second engine**: the crate's stateless
`contact` + `simulation` path, against the upstream model it actually
implements (`no_history`) rather than the one the history-spring tests use.

## Bulk cases — statistical comparison

These start from LIGGGHTS' own post-insertion state, so both codes integrate
the identical configuration; reproducing LIGGGHTS' `fix insert/pack` RNG stream
is not attempted and is not the point. Packing fraction is measured over a slab
excluding `4 r` at the floor and at the free surface, by exact sphere-cap
integration.

~~The comparison is statistical.~~ **CORRECTED 2026-09-17** — that framing was
written on the assumption that settling is chaotic enough to make a
per-particle comparison meaningless. For the HTR-10 case it is not: see
"Per-particle agreement" below, where the median pebble lands 61 micrometres
from where LIGGGHTS puts it. The bulk quantities below are still the *asserted*
gate, because a single-number gate is what a test can act on, but "statistical"
undersold what the codes actually agree on.

| Case | Geometry | N | Quantity | Ours | LIGGGHTS | Agreement | Test |
|---|---|---|---|---|---|---|---|
| bulk bed settling | cylinder, `D/d = 6` | 354 | solid fraction | 0.5571 | 0.5582 | 0.20 % | `pebble_bed_bulk.rs` |
| angle of repose, lifting cylinder (re-run 2026-11-11 after the #1451 shear-history change: **15.66 deg**, gap 0.23 deg; previously 16.28 deg, gap 0.85 deg) | STL mesh wall, `R = 0.050 m` | 656 | repose angle | 12.78 deg | 15.43 deg | ~~2.65~~ **1.34** deg (corrected 2026-09-18) | `angle_of_repose.rs` |
| HTR-10 full core, `E = 1e8` | cylinder, `D/d = 30` | 27 558 | solid fraction | 0.5811 | 0.5810 | 0.02 % | `htr10_pebble_bed.rs` |
| HTR-10 full core, `E = 3e8` | cylinder, `D/d = 30` | 27 558 | solid fraction | 0.5754 | 0.5754 | 4 decimals | `htr10_pebble_bed.rs` |
| **HTR-10 conus slump**, `E = 5e8` | **STL mesh** conus + tube + valve, `D/d = 30` | 27 554 | solid fraction | 0.5769 | 0.5769 | **0.01 %** | `htr10_conus_cross_code.rs` |
| HTR-10 full core, `E = 5e8` | cylinder, `D/d = 30` | 27 554 | solid fraction | **0.5732** | **0.5732** | 4 decimals | `htr10_pebble_bed.rs` |

The angle-of-repose case is the weakest agreement on this page and is reported
as such rather than being tuned: a heap angle is an emergent property of the
whole settling history, and the two codes' heaps differ by ~~2.65~~ **1.34** degrees — corrected 2026-09-18, the 2.65 figure came from the pre-`op-t3l.9` nondeterministic build. See
`verification-and-validation.md` section 4.6.

### Per-particle agreement — stronger than the bulk numbers suggest

Both codes' settled HTR-10 beds are committed
(`htr10_settled.csv` from LIGGGHTS, `htr10_settled_ours.csv` from this port, kept in ~~`reference-data/liggghts/`~~ **MOVED 2026-10-21 (gh:#1008)** `crates/outram-park-fork-liggghts/examples/data/`, excluded from the crates.io package),
so they can be compared pebble by pebble rather than only in aggregate. Each of
the 27 554 pebbles was integrated **50 000 steps independently** by the two
codes from the same initial state. Displacement between the two final
positions, measured 2026-09-17 at `E = 5e8`:

| statistic | value | as a fraction of a 60 mm pebble |
|---|---|---|
| median | `6.09e-05 m` | 0.10 % |
| mean | `7.12e-05 m` | 0.12 % |
| p90 | `1.24e-04 m` | 0.21 % |
| p99 | `1.59e-04 m` | 0.27 % |
| p99.9 | `6.99e-04 m` | 1.2 % |
| max | `2.91e-02 m` | 49 % (one outlier) |
| under 1 mm | **27 535 of 27 554 (99.93 %)** | |

The median pebble ends **61 micrometres** from where LIGGGHTS puts it. That is
a much stronger statement than the packing fraction agreeing to four decimals,
which a wrong model could reach by luck; agreeing on where 27 554 individual
pebbles come to rest, after 50 000 independent steps each, is not something a
different contact model does.

**It is still not bit-identical, and the residual is expected.** A granular bed
is a chaotic system with tens of thousands of contacts making and breaking;
round-off differences (the oblique cases showed 1-3 ulp until 2026-11-10 and
are now bit-identical; a bed still sums contacts in a different neighbour order
from LIGGGHTS) are amplified by contact-order sensitivity. The 0.49-diameter outlier is one pebble
that resolved a contact differently and settled into a neighbouring void — the
kind of event a bulk measure is deliberately insensitive to, and the reason the
asserted gate is the bulk number rather than a per-particle tolerance.

### Packing fraction is not stiffness-independent

The HTR-10 rows above were run at three moduli, and `phi` falls monotonically
as the pebbles stiffen: **0.5811 -> 0.5754 -> 0.5732**, about 1.4 % in total,
and both codes track each other at every point of that sweep.
Softer pebbles interpenetrate, and interpenetration reads as extra solid in a
centre-based cap integration.

This contradicts the strict form of the usual justification for DEM stiffness
softening ("packing is set by geometry and friction, not by stiffness"), and it
was visible only because both codes were run at three stiffnesses rather than
one. It also means the number is **not converged in `E`** — at graphite's true
~9 GPa it would presumably fall a little further. The cross-code agreement was
unaffected throughout, which is the point worth keeping separate: what the soft
modulus threatened was never whether the port is correct, only whether the bed
is a fair stand-in for a real one.

## The conus slump — the mesh-wall path, verified where it matters

The angle-of-repose row above is the weakest in this table, and it is a **mesh
wall** case. Since the HTR-10 recirculation study runs on a mesh conus, that
gap sat directly under the pebble-bed work, so the conus was given its own
comparison rather than assumed to inherit the flat-floor result.

Both codes start from LIGGGHTS' own settled bed (converted by `csv2data.sh`),
read the **same STL**, and integrate 20 000 steps independently.

| step | `φ` ours vs LIGGGHTS | Δ surface height | per-particle median | within 1 mm |
|---|---|---|---|---|
| 2 000 | −0.00 % | +0.0 mm | **11 µm** | **27 554 / 27 554** |
| 6 000 | −0.00 % | +0.1 mm | 1.10 mm | 12 755 / 27 554 |
| 12 000 | +0.00 % | +2.7 mm | 3.55 mm | 739 / 27 554 |
| 20 000 | **−0.01 %** | **+2.8 mm** | 3.82 mm | 592 / 27 554 |

**Read the first row and the last row as two different statements.** The first
verifies the mesh-wall contact path: every one of 27 554 pebbles within 1 mm,
median 11 µm. The last records that a bed draining 17 cm into a funnel is a
**chaotic** rearrangement, so per-particle trajectories must diverge — while
the bulk quantity the comparison actually gates on stays at four decimals
throughout.

This is why the HTR-10 settling case (§ per-particle agreement below) and this
one use different instruments: settling is a small perturbation about an
already-settled bed, and draining is not.

## Defects this comparison found

All three were in code that compiled, passed its own tests, and carried doc
comments asserting the opposite. None would have been caught without running
upstream.

| Defect | Symptom measured | Fixed in |
|---|---|---|
| `Particle::integrate` was not symplectic, though its docs said it was | `E/E0 = 2.13e43` at `dt = 0.1/omega`; restitution 1.0031 on a nominally elastic collision at `dt = 1 us` | `src/integrator.rs` (velocity-Verlet), call site removed |
| `contact.rs` used lever arm `r` where upstream uses the contact radius `r - delta/2` | wrong contact torque, so wrong spin-up | `src/contact.rs` |
| `rolling.rs` CDT diverged three ways, chiefly damped `abs(F_n)` where upstream uses the elastic `k_n * delta_n` | 21 % apart on rolling torque | `src/rolling.rs` |
| Set-up force evaluation advanced the shear history (`update->setupflag` makes upstream skip it, `shearupdate = 0`) | a bed with non-zero velocities got one extra `v_tr*dt` of shear at step 0: one-step velocity off by `4e-7` (relative) on the worst particle, `fix check/timestep/gran` Hertz fraction `5.3e-10` and skin fraction `2.65e-9` off | `src/granular_system.rs` `compute_forces_setup` (2026-11-11, gh:#1418) |
| Cylinder (and plane) walls used `en = normal`, `r = radius` where upstream uses `r = radi - deltan`, `en = delta*rinv` with `PrimitiveWall::resolveContact`'s delta | 1-3 ulp on contacts of particles touching the cylinder (found by the per-contact dump) | `Boundary::upstream_contact`, `ContactKinematics::mesh_wall` (2026-11-11); the 400 000-step wall bounce stays bit-identical |

**One claim of mine was wrong and is retracted, not quietly dropped.** I
reported a tangential-damping defect in `contact.rs`; it came from comparing
that module against upstream's *history* model when it implements
`no_history`. Against the right upstream file it was already correct. Recorded
in `verification-and-validation.md` section 4.2.

## What this does NOT cover

- **No experimental comparison.** Nothing here is validation. The settled
  voidage both codes agree on at `D/d = 6` (`eps ~ 0.442`) sits **2.2
  percentage points above** the Dixon (1988) correlation for that ratio — a
  disagreement with the literature that this cross-code work neither explains
  nor resolves.
- **No published-benchmark agreement.** The HTR-10 bulk fraction (~0.573) sits
  about 6 % below the published filling fraction of 0.61, and the two are not
  the same quantity: 0.61 is a whole-core design closure including the
  near-wall region and the bed surface, while these are bulk-only slab
  measurements. Closing that gap would mean calibrating `mu` and `mu_r`
  downward, which would convert the benchmark into an input and destroy the
  check.
- **Serial only.** Upstream was built with MPI stubs and run on one rank. No
  domain-decomposition or parallel-consistency claim is made for either code.
- **`units si` only.** The port drops LIGGGHTS' `force->nktv2p` and
  `force->ftm2v` conversions, which are exactly 1.0 for SI and are **not** 1.0
  for `units real` (1.6021765e6) or `units metal` (2.94210108e13). A deck in
  those unit systems is out of scope and would be silently wrong.
- **Not every upstream model.** Ported and compared: Hertz, Hooke, tangential
  history and no-history, CDT rolling, the default surface model, primitive and
  mesh walls, `fix nve/sphere`, `fix check/timestep/gran`. ~~Everything else in
  LIGGGHTS' granular package — cohesion models, liquid bridges, superquadrics,
  fibres, CFD-DEM coupling — is neither ported nor compared.~~ **CORRECTED
  2026-11-12 (gh:#1487):** the SJKR/SJKR2 cohesion and the Easo/Washino
  liquid-bridge models (pairs, and since gh:#1487 walls) are ported and
  cross-coded (`docs/verification-and-validation.md` § 4.11). ~~Superquadrics,
  fibres and CFD-DEM coupling are neither ported nor compared.~~ **CORRECTED
  2026-11-13 (gh:#1415):** superquadrics (plane walls, pairs, `nve/superquadric`;
  and, the same day, cylinder and mesh walls, the curvature-limited radius and
  the neighbour list, § 4.17.1) are ported and compared (§ 4.17); fibres and CFD-DEM coupling are neither
  ported nor compared.
- **Bookkeeping axes unsigned.** Both axes of the crate README's
  `## Bookkeeping status` block remain **not yet manually checked**. Nothing in
  this document flips them; only the maintainer does.

## Reproducing

```bash
# the deterministic cases (seconds)
cargo test --release -p outram-park-fork-liggghts --test liggghts_cross_code
cargo test --release -p outram-park-fork-liggghts --test legacy_path_cross_code

# the bulk cases, gated behind the default-on `long-tests` feature
cargo test --release -p outram-park-fork-liggghts --test pebble_bed_bulk    # 317 s
cargo test --release -p outram-park-fork-liggghts --test angle_of_repose    # 2313 s
cargo test --release -p outram-park-fork-liggghts --test htr10_pebble_bed  # 2714 s
```

Runtimes measured on the host named above, 2026-09-16 and 2026-09-17. They are
machine-dependent -- re-measure rather than trusting them.

Regenerating the upstream side from source — the clone, the precision patch,
the build, and the two dump-to-CSV converters — is described in
[`reference-data/liggghts/README.md`](../../../reference-data/liggghts/README.md).

## Force fixes, diagnostics and variables translated (gh:#1418, #1416), 2026-11-11

Translated this session, each with a cross-code deck against LIGGGHTS-PUBLIC
`3d5c00f2` (`reference-data/liggghts/in.*`; methodology and results in each
test's `///` doc):

| Item | Source | Test | Result |
|---|---|---|---|
| force fixes `addforce`, `viscous`, `freeze` (+ `v_` variables) | `src/force_fixes.rs`, `src/variable.rs` | `force_fixes_cross_code.rs` | bit-identical (see `verification_and_validation/force_fixes.md`) |
| `compute ke`, `compute erotate/sphere`, `fix ave/time` (one/running), free-fall deck `in.avetime` | `src/diagnostics.rs` | `ave_time_cross_code.rs` | **55 / 55** bit-identical |
| same, contact deck `in.avetime_contact` (settled bed, tilted gravity, `erotate/sphere != 0` on 20 rows) | `src/diagnostics.rs`, `GranularSystem::particles_in_local_order` | `ave_time_cross_code.rs` | **105 / 105** bit-identical (33 / 105 before summing in upstream's local atom order) |
| `fix ave/euler` (centre, vol. fraction, mass-averaged velocity; 44 cells), deck `in.aveeuler` (new 2026-11-11; stress and radius columns, `basevolume_region` not ported; stock upstream FPEs under thermo, global_freq=0, reference relinked with a one-line fix) | `AveEuler` in `src/diagnostics.rs` | `ave_euler_cross_code.rs` | **1540 / 1540** bit-identical |
| `fix ave/atom` (`x z vx vy vz`), deck `in.aveatom` | `AveAtom` in `src/diagnostics.rs` | `ave_time_cross_code.rs` | **7080 / 7080** bit-identical |
| `fix insert/pack`, `insert/rate/region`, templates | `src/insert.rs` | `insertion_cross_code.rs` | 45 / 45 frames bit-identical |
| `fix insert/stream` | `src/insert_stream.rs` | `insert_stream_cross_code.rs` | 21 / 21 frames bit-identical |
| `compute pair/gran/local` | `granular_system.rs` | `pair_gran_local_cross_code.rs` | bit-identical at steps 0, 1, 10, 100 |

| `fix property/atom/tracer`, `compute nparticles/tracer/region` | `src/tracer.rs` | `tracer_cross_code.rs` | **2440 / 2440** bit-identical (`verification_and_validation/tracer/README.md`) |
| `compute wall/gran/local` (mesh floor, 2 triangles) | `granular_system.rs` `wall_gran_local` | `wall_gran_local_cross_code.rs` | **1449 / 1449** values bit-identical at steps 1, 10, 100 (21 contacts each); the first run found 45 sign-of-zero differences (torque z, `-0` vs `+0`), fixed in `granular.rs` (upstream's `delta_torque` starts at +0) |
| `mesh_module_stress_servo` `ctrlPV torque`, `mode auto` (gh:#1416) | `mesh_stress.rs`, `mesh_wall.rs` | `mesh_servo_cross_code.rs` (`in.mesh_servo_{torque,auto,torque_auto}`) | 3 x **9 990 / 9 990** (501 frames x 18 + 81 x 12) bit-identical; the auto deck covers cruise (0 mesh contacts) and contact phases, the torque deck a rotating plate (torque is friction-saturated, so insensitive to the set point: the test checks the code path, not control quality) |
| `dump mesh/stl` ascii, binary, `region` (gh:#1416) | `mesh_dump.rs` | `mesh_dump_cross_code.rs` (`in.mesh_dump_stl`) | 3 ascii + 2 region + 1 binary file of a rotating drum identical to upstream (text / bytes) |
| `fix massflow/mesh` moving mesh, `delete_atoms yes`, `sum_property`, `massflow/mesh/sieve` (gh:#1416) | `massflow.rs`, `granular_system.rs` | `massflow_options_cross_code.rs` (`in.massflow_{move,delete,sieve}`) | 3 decks x 121 frames bit-identical (positions, velocities, `f_<fix>[1..7]`; delete: atom count 12 -> 5 matched by id) |
| `fix massflow/mesh` x3 and `fix mesh/surface/stress` on the HTR-10 conus, 27 554 pebbles, 20 000 steps (gh:#1416) | `massflow.rs`, `mesh_stress.rs`, `granular_system.rs` | `htr10_conus_massflow_stress_cross_code.rs` (`in.htr10_conus_massflow`) | pebbles **27 554 / 27 554** bit-identical at steps 2 000 and 20 000; all mass-flow values and the reference point bit-identical (41 samples); ~~conus force/torque bit-identical to step 2 500, then worst 8.46e-16 of the norm (summation order; V&V 4.12)~~ **CORRECTED 2026-11-12 (gh:#1491):** conus force/torque bit-identical at all 41 samples once summed in the mesh neighbour list's within-facet bin order (V&V 4.12) |
| exterior (`side out`) `fix wall/region`, block + cylinder (gh:#1491) | `force_fixes.rs`, `insert.rs` | `wall_region_cross_code.rs` (`in.wallregion_out`) | **7 938 / 7 938** bit-identical |
| `wear finnie` of `fix mesh/surface/stress` (gh:#1491) | `mesh_stress.rs` | `mesh_stress_cross_code.rs` (`in.mesh_wear`; wear from an instrumented scratch build) | **5 263 / 5 263** bit-identical, per-facet wear included |
| `fix massflow/mesh point_at_outlet`, `inside_out` (gh:#1491) | `massflow.rs` | `massflow_options_cross_code.rs` (`in.massflow_outlet`) | **14 036 / 14 036** bit-identical |
| servo `target_val v_name` (gh:#1491) | `mesh_stress.rs` | `mesh_servo_cross_code.rs` (`in.mesh_servo_var`) | 9 963 / 9 990; 27 trajectory values differ, cause PETIR `sin` vs glibc (A/B: 9 990 / 9 990 with glibc `sin`), pinned (V&V 4.13) |
| multi-`run` decks: `init()` + `Verlet::setup()` at each `run` (`GranularSystem::begin_run`, gh:#1510) | `granular_system.rs`, `mesh_wall.rs`, `mesh_stress.rs` | `mesh_servo_cross_code.rs` (`in.mesh_servo_2run`, `in.mesh_servo_torque_2run`), `insertion_cross_code.rs` (`in.pour_pack_runs`), `rotating_mesh_cross_code.rs` (`in.rotating_plate_2run`), `mesh_movers_cross_code.rs` (`in.mover_wiggle_2run`) | all bit-identical: 2 x **10 002 / 10 002**, **9 / 9** frames, 2 x **2 259 / 2 259**; a continuous run misses each reference that can tell them apart (V&V 4.14) |
| servo `fix_modify` `integrate`, `vel_max`, `target_val`, `ctrlParam`, `ratio` (gh:#1510) | `mesh_stress.rs` (`MeshServo::modify_param`) | `mesh_servo_cross_code.rs` (`in.mesh_servo_modify`, `in.mesh_servo_auto_modify`) | **10 014 / 10 014** and **10 002 / 10 002** bit-identical; without the `fix_modify` 2 394 and 1 608 values differ (V&V 4.14) |
| `wear finnie` and per-facet `sigma_n`/`sigma_t` on a ROTATING mesh: area refresh at re-neighbouring, per-step `clearReverse` of `f`/`wear_step` (gh:#1512) | `mesh_wall.rs` (`MovingBoundary::refresh_mesh`), `mesh_stress.rs` (`MeshStress::clear_reverse`) | `mesh_stress_cross_code.rs` (`in.mesh_wear_rot`; per-facet values from an instrumented scratch build) | **5 743 / 5 743** bit-identical, facet areas included; before the change 637 differ; control (stale areas) 217 / 5 583 differ (V&V 4.15) |
| `fix multisphere` + `particletemplate/multisphere` rigid clumps (MC template, insertion, clump-wall and clump-clump contact, rigid-body integration) and the `massflow/mesh` volume weight (gh:#1415) | `multisphere.rs`, `insert.rs`, `granular_system.rs`, `massflow.rs` | `multisphere_cross_code.rs` (`in.ms_drop`, `in.ms_collide`, `in.ms_massflow`) | **27 / 27**, **70 / 70**, **98 / 98** atom-frames and **25 / 25** massflow totals bit-identical (V&V 4.16) |
| superquadric particles: `set` mass properties, `nve/superquadric` (scheme 1), plane-wall and pair contact detection (OBB, Newton contact point, line-surface overlap), Hertz + history at the contact point (gh:#1415; upstream rebuilt with `-DSUPERQUADRIC_ACTIVE_FLAG`) | `superquadric.rs` | `superquadric_cross_code.rs` (`in.sq_drop`, `in.sq_collide`) | **36 / 130** and **116 / 260** values bit-identical, worst relative `3.5e-12` and `2.4e-10`; cause libm vs glibc ~~`cbrt`~~ `cbrt` and `pow` (CORRECTED 2026-11-13: `cbrt` alone gives 83 / 130 on `sq_drop`) (A/B: **130 / 130** and **260 / 260** with glibc maths), pinned (V&V 4.17) |
| superquadric cylinder and STL mesh walls (face, edge and corner contacts, inactive edges), curvature-limited radius (mean, Gaussian), upstream neighbour list (`every`/`check` rebuilds, binned pair order, history across separations) (gh:#1415) | `superquadric.rs` | `superquadric_cross_code.rs` (`in.sq_cyl`, `in.sq_mesh`, `in.sq_ridge`, `in.sq_curv_mean`, `in.sq_curv_gauss`, `in.sq_pile_check`, `in.sq_pile_every`) | with glibc maths **every value bit-identical** (130, 130, 130, 260, 260, 1 560, 1 560); committed PETIR maths **91, 41, 44, 65, 63, 379, 379** bit-identical, cause `pow` + `cbrt` (A/B), pinned. **Above the 1e-8 allowance** on `sq_mesh` (last frame, `4.3e-8`), `sq_ridge` (`7.6e-5`) and both piles (chaotic growth to O(1)): pinned exactly and ~~flagged for the maintainer~~ accepted under gh:#1455 (maintainer, 2026-11-13) (V&V 4.17.1) |
| superquadric insertion: `particletemplate/superquadric` + `insert/pack` with the `check_obb` overlap test (`capsules_intersect`, `obb_intersect`), `orientation random` and `constant`, growing history arrays (gh:#1415) | `superquadric.rs`, `insert.rs` | `superquadric_cross_code.rs` (`in.sq_pour`, `in.sq_pour_mesh`) | upstream's atom count at every frame (22 bodies in 5 batches; 5 bodies); with glibc maths **every value bit-identical** (1 651, 585); committed PETIR maths **732, 245**, cause `sin`/`cos` (random quaternion) + `pow` + `cbrt` (A/B), worst relative `1.6e1` (pile) and `1.8e-6`, pinned and accepted under gh:#1455 (V&V 4.17.2) |
| superquadric `rolling_friction cdt` (pair and wall, volume-equivalent and curvature-limited `reff`) (gh:#1415) | `superquadric.rs` | `superquadric_cross_code.rs` (`in.sq_cdt`, `in.sq_cdt_curv`) | with glibc maths **260 / 260** each; committed PETIR maths **63, 61**, worst relative `5.8e-12`, `2.2e-10`, cause `pow` + `cbrt`, pinned; `mu_r = 0` ablation 26 / 260 (V&V 4.17.3) |

`tri_mesh_deform` has nothing to translate: upstream's `tri_mesh_deform.h` is
the single line `typedef TriMesh TriMeshDeformable;` (style
`mesh/surface/stress/deform` is not registered). `dump mesh/vtk`, `mesh/vtm`
and `mesh/gran/vtk` need the VTK library, which the compiled upstream lacks
(`lmp_serial -h` lists `mesh/stl` only), so they cannot be compared and are
not ported.

~~Still **not translated** (gh:#1418, #1414): `compute wall/gran/local` (mesh
walls only upstream), the tracer fixes (`property/atom/tracer`,
`nparticles/tracer/region`), `fix insert/stream/predefined`, `ave/spatial`,~~
**CORRECTED 2026-11-11** (wall/gran/local and the tracers are translated, rows above). Still **not translated** (gh:#1418, #1414): `fix insert/stream/predefined`, `ave/spatial`,
`ave/euler`, the remaining computes and force fixes.
