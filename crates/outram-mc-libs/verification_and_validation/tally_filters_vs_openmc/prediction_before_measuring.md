# gh:#261 bin-for-bin tally-filter comparison against OpenMC 0.15.3 -- PREDICTIONS

Written 2026-11-18, **before any comparison was run** (committed first, per the
workspace rule "predictions written before runs").

## What was established from the code before predicting (not from a run)

1. All 16 filters of #261's list have a `FilterKind` variant (`tally/filter.rs:1007`).
   None had an OpenMC cross-code comparison (the existing evidence is hand-computed
   answers and upstream formulas read from source: `tests/filter_extra_vs_upstream.rs`).
2. **Transport wiring.** `FilterEvent` fields `cell_from`, `cell_born`,
   `material_from`, `surface_mu`, `position_born`, `n_collision`, `event_mt`,
   `parent_nuclide`, `secondaries`, and (for the track-length path) `weight`, are set
   by **no code outside `tally/filter*.rs`**: `scoring.rs` builds its `FilterEvent`
   with `..Default::default()`. Only `cell_instance`, `time`, `direction`, `position`,
   `energy` are threaded through `score_track_length`. So a CellFrom / CellBorn /
   MaterialFrom / MuSurface / MeshBorn / MeshSurface / Collision / Weight tally put on
   a real run scores nothing (or bins everything at the default). The reviewed
   transport function `transport_history_vr` is NOT edited; the history-event driver
   that supplies these fields is new code beside it (`tally::history_events`), fed by
   the existing `TraceEvent` observer.
3. **Upstream 0.15.3 (commit 27e38e89) has no `filter_reaction.cpp` and no
   `filter_particle_production.cpp`** (`ls src/tallies/`); REACTION and
   PARTICLE_PRODUCTION cannot be compared with this OpenMC. `ParentNuclideFilter`
   exists but matches only a decay-chain parent index (`filter_parent_nuclide.cpp:42`),
   set only by a decay-photon source.

## Experiment A -- deterministic beams (exact comparison)

Dilute-O16 / void geometry, single mono-directional, mono-energetic neutron per run,
tracks identical in both codes (no collision is sampled at density 1e-10 atoms/b-cm;
a collision would show as a mismatch and is counted). Compared per site, per bin.
Pass criterion: relative difference <= 1e-9 in every bin (round-off only),
absolute 1e-12 where the value is 0.

| filter | prediction |
|---|---|
| DISTRIBCELL, CELL_INSTANCE | agreement in every bin, including the instance numbering (row-major lattice order); unit tests only checked 3 shapes by hand |
| ENERGY_FUNCTION | agreement 1e-9 provided both interpolate linear-linear (OpenMC default) |
| CELLBORN, MATERIALFROM, MESHBORN | agreement, once fed by the new driver |
| CELLFROM | agreement on leaf-cell bins. **Predicted divergence:** OpenMC loops over *all* coordinate levels of the last state (`filter_cellfrom.cpp:11`), so a bin naming the lattice-filled parent cell matches in OpenMC; ours holds one cell index |
| MUSURFACE | agreement of bin assignment for oblique crossings; the sign convention (`p.surface() < 0` flips the normal, `filter_musurface.cpp:19`) is the likely place for a mismatch |
| MESH_SURFACE | agreement of the set of crossed faces, including diagonal beams crossing several faces |
| **MESH_MATERIAL** | **predicted MISMATCH (a miss).** `MeshMaterialFilter::get_bin` bins the segment midpoint only, with no track-length splitting, so any segment that crosses a mesh-element edge inside one material is credited to one element; OpenMC splits it. Exact on segments that stay in one element |

## Experiment B -- statistical, CE O-16 gas, fixed source

Pass: |z| <= 2 per bin with z = (a-b)/sqrt(sa^2+sb^2); report bin count and worst z.
Not tuned; misses are reported.

| filter | prediction |
|---|---|
| COLLISION | agree within |z|<=2 once `n_collision` is fed by the driver |
| WEIGHT | with survival biasing both codes carry weight `(1-Sa/St)^k`; agreement within |z|<=2 |

## Not comparable here (stated before running)

REACTION, PARTICLE_PRODUCTION (absent in OpenMC 0.15.3), PARENT_NUCLIDE (needs a depletion chain and a decay source).
LEGENDRE: attempted only if the scatter-event driver is finished; otherwise reported as not done.

## Amendment 1 (2026-11-18, before any run; from reading code, not results)

The pass criterion "relative difference <= 1e-9" in Experiment A is too tight by
construction: the transport nudges the particle `NUDGE = 1.0e-9 cm` at every
surface and lattice crossing without scoring that step (`transport_csg.rs:2051`,
`geometry/crossing/mod.rs:1020`), OpenMC does not. A beam of ~5 cm segments with a
handful of crossings therefore carries a systematic relative difference up to
~`n_cross * 1e-9 / segment` ~ 1e-9 to 1e-8 per bin. The criterion is fixed here, before
any comparison, at **relative 2e-8, or absolute 2e-8 cm for bins under 1 cm**; a bin
exceeding that is reported as a miss, whatever the cause. Crossing-count filters
(cell-from, mesh-surface, ...) are integer-valued and must match **exactly**.
Amendment 2 is at the end of this file (written after the first Experiment A run).

Geometry used (fixed now): 3x1x1 lattice, pitch 6 cm, tile = sphere r=2 cm (cell A) in
cell B; dilute O-16 materials M1 (A) and M2 (B); regular mesh 6x2x1 over the block.

## Amendment 2 (2026-11-18, WRITTEN AFTER the first Experiment A run; post-hoc, labelled as such)

What the first run showed (A1, registered geometry, nothing tuned): all filters agree
bin-for-bin except (a) MESH_MATERIAL through the shipped runner (predicted miss, 25 bins),
and (b) MESH_SURFACE, 16 bins at 4 of 18 sites. Every mesh-surface miss is at a beam that
crosses a tile boundary (x = +-3) or the grid vertices of the diagonal beam: in the
registered mesh (6x2x1 over x in [-9,9]) the mesh planes x = +-3, 0, +-6 coincide with the
lattice tile edges and sphere centres, so a flight ends or starts *exactly on a mesh plane*.
Whether that crossing is counted then depends on the last ulp of `r + d*u` against the
plane in each code (OpenMC's `raytrace_mesh` offsets the start by TINY_BIT and returns when
`traveled_distance >= total_distance`, `mesh.cpp:1028,1061`), so these 16 bins test
floating-point tie-breaking and not bin assignment. They are reported as misses of the
registered experiment, not removed.

Supplementary experiment **A2 (post-hoc)**: same sites, same geometry, mesh 5x3x1 over the
same block (planes at x = -9, -5.4, -1.8, 1.8, 5.4, 9 and y = -3, -1, 1, 3, none on a tile
edge or sphere centre). Prediction, fixed before A2 is run: MESH_SURFACE, MESH_BORN and
MESH_MATERIAL (split) agree in every bin (currents exactly, flux within 2e-8), i.e. 0
misses; the diagonal beam passes no grid vertex.

**A2 outcome (recorded here, after the run):** MESH_BORN 0/270 misses, MESH_MATERIAL
(split) 0/540 misses, MESH_SURFACE 1 miss in 3240 bins (site 17, bin 142: z-max outward
of element 11, ours counts it, OpenMC does not). The prediction "0 misses" was
**falsified** for MESH_SURFACE: the diagonal beam leaves the model exactly through the
box edge y = 3 cm, z = 3 cm (at x = -3 cm), which I overlooked when I wrote "passes no
grid vertex". It is another tie (two faces hit at one point), not a bin-assignment finding.

## Experiment B -- written before it is run (2026-11-18)

CE O-16 gas (ENDF/B-VIII.0, 293.6 K, N = 0.03 atoms/b-cm), sphere of r = 10 cm,
vacuum, isotropic 1 MeV point source at the centre, analog; ours through
`trace_csg_history` + `history_events`. 20 batches x 50 000 histories on both sides
(OpenMC: 20 batches of 50 000 in fixed-source mode; ours: 20 consecutive blocks of
50 000 histories). z = (a-b)/sqrt(sa^2 + sb^2) with sa, sb the batch standard errors.
Pass: every |z| <= 2; the count of bins and the worst z are reported.

| tally | bins | prediction |
|---|---|---|
| COLLISION, track-length flux, collision numbers 0..5 | 6 | all |z| <= 2 (expected worst about 1.5 for 6 bins) |
| LEGENDRE order 3 x `scatter` (analog), bare P_l(mu) | 4 | all |z| <= 2. P0 = scatters per source neutron; P1 = sum of mu |

**B first-run outcome (recorded after the run):** LEGENDRE 4 bins, worst |z| 1.49. COLLISION 6
bins, worst |z| **2.87** (collision number 4: ours 0.753318 +- 0.002031, OpenMC
0.744105 +- 0.002480, +1.2 %), a **miss** of the registered criterion. Ten bins were tested, so
a |z| this large arises by chance about 4 % of the time. The miss stays reported as it is;
the first run is not discarded. To tell noise from a defect, a **replication plan is fixed
here, before it is run**: 8 further independent runs of both codes (ours seeds 1001..1008,
OpenMC `seed` 1001..1008, same model), pooled per bin as mean of the 8 run means with
standard error from their scatter; pass criterion unchanged, |z| <= 2 on the pooled bins.
Expectation: if collision bin 4 is noise the pooled |z| falls under 2 (about 1); if it is a
defect of ours the pooled |z| exceeds ~2.5 and the +1.2 % persists.

## Amendment 3 -- PREDICTIONS for the real-transport wiring pass (2026-11-18, written and committed BEFORE any run of the wired code)

Maintainer-confirmed edits (gh:#261, 2026-11-18): (1) fill the event-history fields of
`FilterEvent` in the real transport; (2) split MESH_MATERIAL tracks across mesh elements in
`tally/scoring.rs`. Design (from reading the code, not from a run): the transport body that
carries the logic is `transport_history_vr_urr` (the `kovan`-reviewed `transport_history_vr`
is a 40-line forwarding wrapper that calls it and is NOT edited). A per-particle
`EventHistory` (cell_born, position_born, n_collision, material, material_last) is kept in
the loop and handed to new scoring functions; no random number is drawn by any of it.

**Bit-identity of k and of every existing tally (predicted before running):** bit-identical.
The new code reads state and adds to tallies; it takes no branch the transport takes and
draws no variate. Existing pins (`stats_move_fingerprints`, `variance_reduction_is_bit_identical_when_analog`,
the Godiva k-trace pin, ...) predicted to stay bit-identical, **no re-pin**. The one existing
behaviour that changes: a `WeightFilter` on a track-length tally used to see the default
weight 1.0 and now sees the particle weight `w`; any test that pins a WeightFilter
track-length tally is predicted to move (none expected; a grep before the run will say).

**Experiment A through the real runner (`run_fixed_source_traced`), same 18 sites, same
criteria as amendment 1:**

| filter | prediction |
|---|---|
| CELLBORN, MESHBORN (flux) | 0 misses of 36 / 216 (+270 on A2) bins, worst scaled diff about 1e-9 |
| CELLFROM, MATERIALFROM, MUSURFACE (current) | 0 misses of 252 / 252 / 1008 bins, **exact**: the live scoring replays the verified driver's rules (sign in the level frame, vacuum-leak `material_last`, global-position normal) |
| MESH_MATERIAL, registered mesh and A2 | **0 misses** of 432 and 540 bins after the splitting fix (the driver version of the same split gave 0), worst scaled diff about 1.6e-8 (the transport's 1e-9 cm nudge) |
| MESH_SURFACE, registered 6x2x1 mesh | **16 misses at the same 4 sites (ties on mesh planes)**: the live segment start/end are the same floating-point numbers the driver reconstructs |
| MESH_SURFACE, A2 mesh | **1 miss** (site 17, the box-edge tie), unchanged |

**Experiment B through the real runner** (CE O-16 sphere, 1 MeV isotropic point source, 20 x
50 000 histories; a different random stream from the driver run, since the runner draws the
source itself): COLLISION flux, 6 bins: all |z| <= 2 expected (the earlier first-run 2.87 was
judged noise by the 8-seed pooling; if the real-transport run again shows bin 4 high by
about +1.2 % the noise reading is wrong). LEGENDRE is not reachable through real transport
(no analog-scatter Legendre scoring exists in the CSG transport) and stays on the driver.

**WEIGHT (Experiment C, new): B-10 gas sphere, survival biasing on in both codes** (OpenMC
`survival_biasing`, cutoff 0.25, survive 1.0, its defaults; ours `VarianceReduction`
likewise), 1 MeV source, track-length flux binned in weight. Bins: [0, 0.1, 0.25, 0.5, 0.75, 1.0].
Prediction: all |z| <= 2; weights are (1 - Sa/St)^k so several bins are populated. OpenMC's
filter reads `wgt_last` (weight at the start of the event) and ours the flight weight; they
agree because the weight only changes at collisions, after the segment is scored. Expected
worst |z| about 1.5 over 5 bins.

**The 4 red gates (predicted):** `mesh_material_vs_openmc` goes green (shipped path split);
`mesh_surface_vs_openmc` and `a2_mesh_surface_vs_openmc` stay red on the same ties (the
driver-based tests are kept as the oracle; new real-transport tests are added beside them and
are predicted to show the same 16 and 1); the first-run COLLISION gate of the driver stays
red as recorded (its numbers do not change), while its real-transport twin is predicted
green.

### Amendment 4 (2026-11-18, written BEFORE Experiment C is run; two things that happened after amendment 3)

1. **Measured since amendment 3 (Experiment A through the real runner, recorded in the README):**
   every prediction in the table above held (all filters, including the 16 and 1 tie misses and
   the exact currents); existing bit-identity pins unmoved. That is not repeated here.
2. **Experiment C absorber changed, forced by the data tool, not by a result.** NJOY2016's
   ACER refuses ENDF/B-VIII.0 B-10 (`***error in change***Undefined law for dlwh block: 0`,
   exit 77), so B-10 cannot be built into an OpenMC library here. **Li-6** (n,t) replaces it:
   sphere r = 10 cm, Li-6 0.05 atoms/b-cm, isotropic **300 keV** point source (on the (n,t)
   resonance, where Sa/St is large so weights spread), survival biasing on in both codes
   (cutoff 0.25, survive 1.0), 20 x 50 000 histories, track-length flux in WeightFilter bins
   [0, 0.1, 0.25, 0.5, 0.75, 1.0]. Prediction unchanged in kind: all 5 bins |z| <= 2, worst
   about 1.5. If a bin is a miss at the first run, the replication plan is fixed here: 8
   further seed pairs (1001..1008) pooled as for Experiment B, criterion |z| <= 2 pooled.

### Amendment 5 (2026-11-18, written after the first real-transport run of Experiment B/C, before any replication)

First real-transport runs (seed 261, recorded in the README): WEIGHT worst |z| 1.06 (prediction
held); **COLLISION worst |z| 2.56 (bins 2 and 4: +2.56 and +2.55), a miss of the registered
criterion**, in the same direction as the driver's first run (bin 4 +1.1 % here, +1.2 % there).
Stays reported as a miss. Replication plan fixed now, before running it: the same 8 seed pairs
as before (ours seeds 1001..1008 through the real runner, OpenMC `expB_collision_s1001..1008`
already committed), pooled per bin as mean of the 8 run means with standard error from their
scatter, criterion |z| <= 2 on the pooled bins. Expectation: the OpenMC first-run bin-4 value
(0.744105) is itself low against OpenMC's own 8-seed pooled 0.7473, so the first-run miss is
mostly the reference's draw; the pooled worst |z| falls under 2 (about 1.2). If it stays above
2.5 the noise reading is wrong and the collision-number binning (n_collision off by one at a
secondary or after a split) is the suspect.

## Amendment 6 -- the χ² gate, registered BEFORE any fresh run (gh:#1711, 2026-11-22)

Written and committed before OpenMC or our code is run on any of the seeds below. Maintainer
(gh:#1709/#1711, 2026-11-22): "Bin by bin shldnt make sense"; asked for "the chi sq gate".

**Why the instrument changes.** A bin-by-bin |z| <= 2 gate fails a correct code with probability
1 - 0.9545^k per run (24 % for k = 6, 17 % for k = 4, 13 % for k = 3), and the worst |z| of k bins
is not a statistic whose distribution the gate states. The question each tally asks is "are the
k bins of the two codes consistent with the same expectation, given both codes' noise?", which is
a χ² test with k degrees of freedom. Nothing below is chosen from data: the seeds below have never
been run by either code, and no result on them exists when this is committed.

**Statistic (per tally, per seed).** For bin i, with each code's 20-batch mean and batch standard
error (OpenMC: `std_dev` of the statepoint; ours: standard error over the 20 batch values),

    z_i = (ours_i - openmc_i) / sqrt(se_ours_i^2 + se_openmc_i^2),   χ² = Σ_i z_i^2,

summed over the bins with **nonzero combined variance**; k = the number of such bins. A bin whose
combined variance is zero must hold the same value in both codes (both empty), else the tally
**fails** outright (a deterministic disagreement). Expected k: COLLISION 6, LEGENDRE 4, WEIGHT 3
(bins [0, 0.1) and [0.1, 0.25) are empty in both codes under roulette at 0.25, as on every seen
seed).

**p-value.** p = P(χ²_k >= χ²_obs) = Q(k/2, χ²_obs/2), the regularised upper incomplete gamma,
taken from the workspace (`outram_foam_basic_lib::math::inc_gamma_ratio_q`, the DiDonato-Morris
port of OpenFOAM's `incGammaRatio_Q`), checked in the test against the closed forms
Q(1, x) = e^-x (k = 2) and Q(1/2, x) = erfc(sqrt x) (k = 1) before it is used.

**Gate.** **Pass when p >= 0.01**, for every tally on every fresh seed, each judged separately.

**Fresh seeds.** 2001, 2002, 2003, 2004 -- for both codes, one run per seed, 20 batches x 50 000
histories, the committed scripts verbatim (`exp_b_collisions.py ... <seed>`,
`exp_c_weight.py ... <seed>`, output suffix `_s<seed>`); ours with the same seed number:

| test (new) | route | tallies (k) | per-seed gates |
|---|---|---|---|
| B driver | `trace_csg_history` + `history_events` | COLLISION (6), LEGENDRE (4) | 8 |
| B real | `run_fixed_source_traced` | COLLISION (6) | 4 |
| C real | `run_fixed_source_traced`, survival biasing | WEIGHT (3) | 4 |

**Family-wise false alarm, stated now.** 16 per-seed gates at 0.01: a correct code fails at least
one with probability 1 - 0.99^16 = 15 % (if the gates were independent; the B driver and real
routes share the OpenMC reference per seed, so slightly less). A single failure at p just under 0.01
is therefore not by itself evidence of a defect; it is reported, and the gate is **not** moved.

**Pooled over the four seeds (reported, not gated).** (a) Σ_s χ²_s with Σ k dof (4k), p from the
same Q. (b) A bias statistic sensitive to a shift that repeats on every seed: per bin
z̄_i = Σ_s z_{s,i} / sqrt(4), χ²_bias = Σ_i z̄_i^2 with k dof. Per-seed sensitivity: COLLISION
bin 4 has a combined se of about 0.0032 (0.43 %), so one seed flags a bias of roughly 1.3 % in a
single bin; the 4-seed bias statistic flags roughly 0.6 %.

**Assumptions the gate rests on, and how each would show.**
1. *Bins are independent.* They are not, within one code: the collision-number bins of one history
   share its flight (a history that scatters more fills higher bins and less of bin 0), and the
   Legendre moments P0..P3 are sums over the same scatters. The two codes are independent of each
   other, so each z_i is still N(0,1) and **E[χ²] = k holds**; correlation only widens the
   distribution (var χ² = 2 Σ_ij ρ_ij², not 2k), so the 0.01 tail is entered more often than 1 %.
   Detection, registered here as a report: the bin-to-bin correlation matrix of our batch values
   (20 batches x 4 seeds), the variance-inflation factor Σ_ij ρ_ij² / k, and the spread of the four
   per-seed χ² against 2k. If the factor exceeds about 2, the independence assumption is wrong
   enough to matter and the finding is recorded; the gate is not re-derived on these seeds.
2. *Batch standard errors are exact.* They are estimated from 20 batches each, so z_i is
   t-like (Welch dof about 30-38) and E[z_i^2] is about 1.06: χ² runs about 6 % high on average.
   Stated, not corrected.
3. *Seeds are independent streams.* The seeds are new numbers; each code's stream for seed s is
   not a shift of any seen seed's. Nearby LCG seeds are what 1001..1008 also used.

**Prediction.** All 16 per-seed gates pass; χ²/k scattered around 1 (roughly 0.2-2.5 per tally);
pooled Σχ² p > 0.05 for each tally; bias χ² p > 0.05 (the seen tendency of COLLISION bin 4,
pooled z +1.2 to +1.5 on seeds 1001..1008, would show as z̄_4 of about +1 here if it is a real
~0.2 % effect, below the sensitivity above). Variance-inflation factor: above 1 for both COLLISION
and LEGENDRE (LEGENDRE larger, about 2, because P0 and P2 share every scatter), about 1 for WEIGHT.

**The seen seeds (261, 1001..1008).** Their tests (`b_collision_and_legendre_vs_openmc`,
`real_b_collision_vs_openmc`, `b_replicates_pooled_vs_openmc`, `real_b_replicates_pooled_vs_openmc`,
`real_c_weight_vs_openmc`) become **reports**: they print the bin-by-bin z and the χ² and p of the
same statistic, and gate nothing, because those seeds were used to make decisions (gh:#1709) and
cannot test the gate. Reference data for them is kept.
