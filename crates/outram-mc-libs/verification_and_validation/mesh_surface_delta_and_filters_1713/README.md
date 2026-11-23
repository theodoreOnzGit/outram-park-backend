# Mesh-surface tallies under delta tracking and with other filters (gh:#1713)

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-drafted, untrusted
until a human has reviewed it.

Builds on gh:#1709 (upstream `raytrace_mesh` and `r_last_current`). Two gaps:

1. mesh-surface currents were scored only on the surface-tracking arm of
   `transport_history_vr_urr`, so a delta-tracked region scored none;
2. a mesh-surface tally had to carry no other filter (the live path returned
   early, the history driver returned an error).

## Upstream read first (OpenMC `8f79d9d0`)

- `src/tallies/tally_scoring.cpp:33-60` (`FilterBinIter`): every filter of the
  tally fills a `FilterMatch` of `(bin, weight)` pairs by `get_all_bins`; if
  any filter has no match the event scores nothing (`:47-51`).
- `:96-128` (`operator++`) walks every combination, last filter fastest;
  `:130-141` (`compute_index_weight`) gives the flattened index
  `sum_i bins_i * strides(i)` and the weight `prod_i weights_i`.
- `src/tallies/tally.cpp:496-509` (`set_strides`): the last filter has stride
  1, filter `i` has stride `prod_{j>i} n_bins(j)`. So for single-bin filters
  the index is the usual row-major product, `bin = bin * n_bins(f) + b_f` in
  filter order, which is what `score_crossing` already does.
- `src/tallies/filter_meshsurface.cpp:10-22`: the mesh-surface filter's match
  is every face `surface_bins_crossed(r_last_current, r, u)` returns, each with
  weight 1.0.
- `tally_scoring.cpp:2612-2650` (`score_meshsurface_tally`): for each
  combination, `wgt_last * filter_weight` is added to every score of the tally.
- `src/particle.cpp:411` (`event_collide`) calls it before the collision
  changes `E`, `u` or the cell; `:743-748` (`cross_vacuum_bc`) and
  `:785-788`, `:835-838` (reflective, periodic) before the particle leaves its
  cell. So the energy, cell, material and universe a combined filter reads are
  the pre-event ones: the energy of the flight that ends, the cell the
  particle is in when it collides or reaches the boundary.

So: **the mesh-surface filter contributes each of its matches, each other
filter contributes its one bin (and its weight, 1.0 except
`EnergyFunctionFilter`'s `y(E)`), the flattened index is the product index in
filter order, and the score is `w * prod(weights)`.** A combination with a
filter that itself matches several bins (a second mesh-surface,
mesh-material, particle-production, an expansion filter, a track-splitting
mesh) is outside what is implemented here: the history driver returns an
error for it and the live path scores nothing for it (stated in the docs).

## Delta tracking: new work, not upstream

OpenMC has no delta tracking, so there is nothing upstream to port. The
hybrid driver bounds every delta flight by the nearest surface over every
coordinate level (`physics/delta_tracking/handoff.rs` module docs), so a delta
flight is, like a surface-tracked one, a straight line from the flight start
`r_flight` along `u` that ends either in a REAL collision at `d_col`
(`RegionFlightEnd::Collision`) or at the nearest surface at `d_bound`
(`RegionFlightEnd::StreamToSurface`, which sets `d_col = INFINITY`). Virtual
(rejected) collisions do not end the flight and change neither `u` nor `E`.
The mesh-surface raytrace `r_last_current -> event point` therefore needs no
per-segment geometry: the same `mesh_surface_span` (collision: the collision
point; vacuum leak: `+TINY_BIT`; reflective/white/periodic: `-TINY_BIT`;
transmissive or lattice: nothing, the span continues) applies unchanged.

Design: call `score_mesh_surface_segment` on the delta arm with the same
arguments as on the surface arm. `EventHistory::end_flight` is already called
once per flight **outside** the `match path.tracking`, so `r_last_current` is
updated on both arms. Each flight takes exactly one arm, so a flight can be
scored at most once; a particle moving between a surface-tracked and a
delta-tracked region crosses a transmissive surface, which scores nothing and
leaves `r_last_current` alone, so the span continues into the next region.

## Predictions, written before any run (2026-11-22)

P1. **No mesh-surface tally: bit-identical.** Scoring draws no random number
and the new calls return before reading anything when the tally has no
mesh-surface filter. `tests/stats_move_fingerprints.rs`,
`tests/delta_tracking_bit_identity.rs` and every pinned `k` stay as they are.

P2. **Delta vs surface tracking agree within statistics.** Model: the
three-shell reflective sphere of `tests/delta_tracking_audit.rs` (r = 5 cm,
fuel / graphite / dilute gas, LOW-tier U-235, U-238, C), the whole sphere
delta-tracked in one arm and surface-tracked in the other, `k`-eigenvalue,
2000 particles x [5 inactive + 20 active], seed 3, the audit's majorant. Tally:
`Current` on a regular 3x3x3 mesh over `[-3, 3]^3` (324 face bins; faces lie
both inside the zones and across the zone interfaces). Statistic: per bin
with a nonzero mean in both arms, `z = (delta - surface) / sqrt(s_d^2 + s_s^2)`
from the batch standard deviations of the means; `chi2 = sum z^2` over those
`N` bins. Criterion, fixed now: `chi2 / N < 1.5` and `max |z| < 5` (the bins
share histories, so `chi2` is not exactly chi-square distributed; the mean of
`z^2` is still about 1 for unbiased arms, and `|z| < 5` over ~300 bins has a
false-alarm probability near 2e-4). Prediction: passes, `chi2 / N` within
0.8-1.2. A double count on the delta arm would show as delta currents about
twice the surface ones (`chi2 / N` in the hundreds); a missing score would
show as delta currents of zero (the test also asserts the delta tally is not
all zero).

P3. **Energy x mesh-surface summed over energy equals the plain
mesh-surface tally, bit for bit.** On the Experiment A sites of
`tests/tally_filters_vs_openmc.rs` (one particle per run, analog, weight
exactly 1, so every per-bin sum is a small integer and the order of summation
cannot change the bits), and also `Cell x mesh-surface` summed over cells.
Prediction: exact equality on every site and bin.

P4. **Hand-built bin index.** A two-element mesh crossed once at its internal
face gives two face bins `m0, m1` (from `MeshSurfaceFilter::matches`); with a
two-group energy filter the flattened bins are `e * n_ms + m` for
`[Energy, MeshSurface]` and `m * 2 + e` for `[MeshSurface, Energy]`, every
other bin 0. An energy outside the energy filter scores nothing. Prediction:
exact.

P5. **Driver = live** for `Energy x mesh-surface` on the same sites (the
driver is the oracle). Prediction: exact.

P6. `tests/tally_filters_vs_openmc.rs` mesh-surface comparisons stay at 0
misses (single-filter path unchanged).

### Amendment 1, written after the first P2 run and before the control

The first P2 run passed its criterion but gave `chi2 / N = 1.22`, just above
the predicted 0.8-1.2. Hypothesis, stated before measuring: the batch standard
deviation of a `k`-eigenvalue tally underestimates the true one because
successive generations are correlated, so `z^2` averages above 1 for any two
independent arms, delta tracking or not. Control added to the same test:
surface tracking at seed 3 against surface tracking at seed 4, same `chi2 / N`
statistic. Prediction: the control also gives `chi2 / N` about 1.1-1.3; if it
gives about 1.0 instead, the 1.22 is not the instrument and is investigated.
The pass criterion of P2 is not changed.

### Amendment 2, written after the first P3 run and before the added check

P3's `Cell x mesh-surface` split on the Experiment A sites put every score in
the outer cell (groups `[18, 38]`, cells `[0, 56]`): in that dilute model the
scoring events are vacuum leaks, made from the outer cell, so the cell split
checks the sum only. Added check (P3b): on the sphere of P2, surface tracking,
seed 3, `Cell [0, 1, 2] x mesh-surface` summed over cells against the plain
tally of the same transport (scoring draws nothing, so `k` is bit-identical).
Per-batch values are integer counts before normalisation, so the criterion is
relative difference `< 1e-12` (normalisation rounding only), and every cell
must score. Prediction: passes, worst difference at the 1e-16 level.

## Results (2026-11-22/23)

Hardware: 4-core Intel Xeon @ 2.30 GHz, 15 GiB RAM, Linux, CPU only, shared
with other runs. The agent's first runs measured P2 to P6; the agent stopped
when the disk filled. The rows were checked again in the full suite below.

| prediction | result |
|---|---|
| P1, bit-identical without a mesh-surface tally | **held**: `stats_move_fingerprints` passes; the hybrid `k` and its sigma are bit-identical with and without the tally |
| P2, delta vs surface (324 bins) | **passes the fixed criterion**: `chi2 / N` **1.223**, max \|z\| **3.61** (criterion < 1.5 and < 5). Above the predicted 0.8-1.2 band, see amendment 1 |
| Amendment 1 control, surface seed 3 vs seed 4 | `chi2 / N` **1.10**: the instrument (correlated generations) carries an excess over 1 for two independent arms too, so 1.22 is not attributed to delta tracking. Summed current delta 448.88 against surface 453.34 (ratio 0.990), not resolved at this budget |
| P3, energy x mesh-surface summed over energy | **exact** on all 18 Experiment A sites |
| P3, cell x mesh-surface summed over cells | exact; it checks the sum only (amendment 2) |
| P3b, cells on the sphere | worst relative difference **5.3e-16**; all three cells score |
| P4, hand-built bin index | **exact** |
| P5, driver equals live | **exact** |
| P6, OpenMC mesh-surface comparisons | **0 misses** |

**Full suite on the merged code** (#1709, #1711, #1712, #1713 together):
`cargo test --release -p outram-mc-libs --lib --tests --no-fail-fast`
gives **205 test binaries, 1401 passed, 0 failed, 23 ignored**. This file's
tests: 3 passed in 105 s. The `no_std` check
`cargo check --release -p outram-mc-libs --no-default-features --lib --target x86_64-unknown-none`
passes.

**Not resolved:** the 1 % lower summed current on the delta arm is within this
budget's noise, and a higher-statistics run would be needed to bound it.
