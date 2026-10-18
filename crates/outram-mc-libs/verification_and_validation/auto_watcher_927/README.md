# Auto watcher thresholds: experiments for gh #927

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or any use `RESPONSIBLE_USE.md` excludes.
> AI-written; untrusted until a human has reviewed it.

**Question.** `MCSettings::Auto`'s inactive-cycle watcher
(`crates/outram-mc-libs/src/model/run.rs`) has six thresholds whose values
were chosen before any measurement: `WATCH_WINDOW_CYCLES` (30),
`WATCH_EVERY_CYCLES` (5), `SETTLE_CYCLES` (10), `ENTROPY_MESH_DIVISIONS` (5),
`WATCH_PARTICLES` (1000) and, since #944, `WATCH_NORMALITY_ALPHA` (0.05).
Does measurement justify them? **No constant was changed.** Recommendations
only; the maintainer decides.

Design and predictions were posted on #927 before any run
(comment 6254417988, 2026-10-19). Instrument:
`crates/outram-mc-libs/examples/auto_watcher_927.rs`. Raw tables:
`results/`. Traces (CSV, about 12 MB) are regenerable and not committed.

## Headline

1. **The window criterion is the problem, not any single threshold.** On
   windows that are stationary by construction, MSER-5 plus Geweke on k and H
   rejects most windows: the joint k-and-H test passes **22 %** of stationary
   30-cycle windows (Godiva), **24 %** (HST-009), **14 %** (pair A), **16 %**
   (pair B). A well-behaved test would pass about 90 %. The Geweke half alone
   passes 46-50 %; the MSER "optimum in the first half" half passes 64-68 %
   (30 cycles are only 6 MSER batches, and Geweke's first window is 3 cycles).
2. **So Auto runs to its 100-cycle cap in 40-70 % of runs on every case**, and
   its inactive cost is about **0.53 Mhistories whatever the case**: median 95-100
   inactive cycles on Godiva, which is converged from cycle 5. Against the
   standard-practice baseline (10 000 per cycle until k and H converge) that
   is **10x the cost on Godiva**, and cheaper only where the baseline itself
   needs more than about 50 cycles.
3. **It does not see a slow transient.** On a deep fast transient (pair A) the
   entropy half passes 6 % of transient windows against 31 % of stationary
   ones (about 5x separation). On a slow one (pair B, 120 cycles to converge)
   it passes 31 % against 34 %: no discrimination. When it does declare
   convergence on pair B, **all 12 of 12 declarations were false** (declared
   before cycle 120), and the active-cycle k is **+49 +/- 20 pcm** (2.4 sigma)
   from the converged k.
4. k bias is otherwise unresolved on all four cases (|bias| at most 0.3 sigma
   of a run, Godiva -24 +/- 23, pair A -8 +/- 17, HST-009 -10 +/- 32 pcm),
   mostly because the cap (100) forces the discard to be long.

## Cases (beginner API, ENDF/B-VIII.0 processed from `reference-data/endf`)

| case | what | why |
|---|---|---|
| `godiva` | HMF-001 bare sphere (`vv::godiva`) | fast; converged from the start |
| `hst009` | HST-009 case 1, nested spheres, light water (`vv::hst009`) | thermal, larger. LCT-008 not used: 4961 pins, Auto takes hours |
| `pairA` | two Godiva-material spheres, centres 40 cm apart in vacuum; A has r = 8.7407 cm, **B has r = 8.2 cm** | slow source. The uniform start puts about 45 % of the source in B, which is subcritical alone; the transient is deterministic |
| `pairB` | the same with **r_B = 8.6 cm** | much slower. Radii fixed from the physics before any run |

No existing loosely coupled case was found (searched `examples/` and `tests/`
for slab, decoupled, loosely: only the RMC note in
`tests/shannon_entropy_in_keff.rs`).

## Method

**Instrument.** `run.rs`'s thresholds are `const`, so the example re-implements
the watcher's control flow (`watch`) with every threshold a parameter, on the
same `CsgPowerIteration::step_par`, `set_population`, `source_entropy`,
`TraceDiagnosis` (MSER-5 and Geweke) and `raffles` D'Agostino-Pearson.
**Fidelity test:** `auto_watcher_927 check godiva` runs the library's
`MCSettings::Auto` (seed 1) and the emulator at the default thresholds:
**`k_by_cycle` identical and the entropy over the inactive cycles identical,
bit for bit** (Auto: low-population pass at cycle 60, no production pass,
100 inactive; `results/check_godiva.txt`, run 2026-10-19).

**Recorded traces.** Per seed: one low-population trace at each of
P = 500, 1000, 2000, 5000 (80 cycles) and one production trace at 10 000 per
cycle from the uniform start (no watcher; 250 cycles, HST-009 200, pair B 500),
each with the entropy on 3^3, 5^3, 8^3 and 12^3 meshes. Seeds: 40 (Godiva,
both pairs), 24 (HST-009).

**Reference ("truth").** From the production traces: the ensemble-mean
entropy trace H-bar(c); the converged level H_inf and `k_ref` from the cycles
at or after `D_ref` (150; HST-009 100; pair B 300); `tau` = within-run
per-cycle sd of H; **`c_true`** = the first cycle after which every 5-cycle
block of H-bar stays within `tau` of H_inf ("visibly moving" means the
departure exceeds one run's own scatter). `sigma_run` = sd across seeds of a
100-cycle active mean.

**Replay.** One threshold at a time around the default: W in {20, 50}, E in
{1, 10}, S in {0, 5, 20}, mesh in {3, 8, 12}, P in {500, 2000, 5000}, alpha in
{off, 0.01, 0.10}. The decision is replayed from recorded traces, the
production phase read at the same cycle index. **This assumes the
deterministic source transient does not depend on the population.** It was
tested: Godiva, pair B and HST-009 default were also run live (the real
protocol, new seeds), below.

**Metrics.** `n_in` (inactive cycles chosen); the share that never pass (hit
the cap: "not converged"); **false pass** = production confirmation declared
and `n_in < c_true`; inactive cost in histories against the baseline
`10 000 x c_true`; **bias** = mean of k over 100 active cycles starting at
`n_in` minus `k_ref`, mean over seeds +/- standard error.

## Predictions against results

| # | prediction (posted before running) | result | verdict |
|---|---|---|---|
| 1 | emulator = Auto bit for bit | identical k and H | confirmed |
| 2a | Godiva `c_true` <= 5 | 5 on every mesh | confirmed |
| 2b | Godiva: passes at the first low check in about 60 %; median `n_in` = 70 | median **95**; **16/40 never pass** (26-55 %); mean 92.2 | **refuted** |
| 2c | Godiva false pass 0; bias 0 within 1 sigma | 0/24; bias -24.4 +/- 22.8 pcm (-0.18 sigma_run) | confirmed |
| 2d | cost about 8-10x the baseline | 0.530 vs 0.050 Mhist = 10.6x | confirmed (for a different reason: the cap, not the window) |
| 3a | HST-009 `c_true` of order 10-25 | **0** on all meshes (uniform start already at the entropy level) | refuted |
| 3b | HST-009 median `n_in` 70-80 | **100**; 13/24 never pass | refuted |
| 3c | false pass 0, bias consistent with 0 at about 15 pcm | 0/11; -9.5 +/- 32.3 pcm | confirmed (resolution 32 pcm, not 15) |
| 4 | pair A `c_true` 40-70; false pass 5-40 % | `c_true` 70; **0/13 declared** (0-23 %); 27/40 never pass | `c_true` confirmed; false pass at the bottom of the range |
| 5 | pair B `c_true` near the cap; default false-passes in > 30 %; W = 50 only partly helps | `c_true` 120; **12/12 declared false** (= 30 % of all runs); W = 50: 8/8 false (20 % of all runs) | confirmed |
| 6a | larger W lowers false passes, raises cost linearly | W 20/30/50: never-pass 60/40/78 % (Godiva) so fewer declarations, but the false share of declarations stays 100 % on pair B; cost 0.52/0.53/0.64 Mhist | partly: fewer declarations, not fewer false ones |
| 6b | E has almost no effect on false passes | **refuted**: E = 1 on pair B: 29/40 runs falsely declare (vs 12/40 at E = 5), never-pass 28 % (vs 70 %). More looks, more declarations | refuted |
| 6c | S = 0 cheaper, bias unresolved | cost -0.045 to -0.06 Mhist (-9 to -11 %); bias change at most 10 pcm (SE 17-32) | confirmed |
| 6d | mesh 3 misses the slow mode; mesh 12 refuses more | pair A: transient windows pass 0.03/0.06/0.04/0.03 (3/5/8/12), stationary 0.29-0.31; never-pass 30/27/23/23 of 40 | **not supported**: meshes indistinguishable |
| 6e | P = 500 refuses more, P = 5000 fewer, cost up | stationary-window size equal across P (0.14-0.27 at W 30); cost 0.52/0.53/0.56/0.71 Mhist; never-pass within binomial noise | cost confirmed; no benefit found |
| 6f | alpha: off removes about 5.7 % of refusals; no bias change | normality passes 0.94-0.96 of stationary windows; alpha off changes never-pass by 1-2 runs of 40; bias change <= 3 pcm | confirmed |

## Results

Replay, default row first, one setting per row (full tables:
`results/replay_*.md`). `never` = never pass (cap). Cost = inactive Mhistories.

### Default thresholds, all cases

| case | seeds | `c_true` (5^3) | n_in median [min-max] | never | false pass / declared | cost | baseline | bias [pcm] |
|---|---|---|---|---|---|---|---|---|
| Godiva | 40 | 5 | 95 [70-100] | 16/40 | 0/24 | 0.530 | 0.050 | -24.4 +/- 22.8 |
| HST-009 | 24 | 0 | 100 [75-100] | 13/24 | 0/11 | 0.529 | 0 | -9.5 +/- 32.3 |
| pair A | 40 | 70 | 100 [85-100] | 27/40 | 0/13 | 0.499 | 0.700 | -8.1 +/- 17.4 |
| pair B | 40 | 120 | 100 [80-100] | 28/40 | **12/12** | 0.553 | 1.200 | **+49.3 +/- 20.2** |

`k_ref` and `sigma_run`: Godiva 1.00041 / 139 pcm; HST-009 1.00120 / 140 pcm;
pair A 1.00107 / 109 pcm; pair B 1.00216 / 155 pcm. HST-009's `c_true` of the
k trace (90) is ensemble noise (24 seeds, block noise above the 1-sigma_run
tolerance) and is not used.

### Size and power of the window criterion (the main finding)

Pass rate of each part on windows from the production traces; windows start
every 5 cycles, so they overlap and are correlated (the counts in
`results/windows_*.md` are not independent). "Stationary" = start at or after
`c_true`; "transient" = end at or before `c_true`.

| case | W | k MSER | k Geweke | k both | H(5^3) both | k and H | normality p >= .05 | **all (default)** |
|---|---|---|---|---|---|---|---|---|
| Godiva stationary | 20 | 1.00 | 0.35 | 0.35 | 0.36 | 0.12 | 0.95 | **0.11** |
| | 30 | 0.65 | 0.46 | 0.46 | 0.47 | 0.22 | 0.95 | **0.21** |
| | 50 | 0.55 | 0.55 | 0.51 | 0.52 | 0.27 | 0.95 | **0.26** |
| HST-009 stationary | 20 | 1.00 | 0.36 | 0.36 | 0.39 | 0.13 | 0.94 | **0.12** |
| | 30 | 0.64 | 0.48 | 0.48 | 0.48 | 0.24 | 0.93 | **0.22** |
| | 50 | 0.55 | 0.56 | 0.52 | 0.54 | 0.28 | 0.95 | **0.26** |
| pair A stationary | 30 | 0.65 | 0.49 | 0.49 | 0.31 | 0.14 | 0.94 | **0.13** |
| pair A **transient** | 30 | 0.66 | 0.47 | 0.47 | 0.06 | 0.02 | 0.87 | **0.02** |
| pair B stationary | 30 | 0.64 | 0.48 | 0.48 | 0.34 | 0.16 | 0.94 | **0.15** |
| pair B **transient** | 30 | 0.66 | 0.48 | 0.48 | 0.31 | 0.13 | 0.92 | **0.12** |

(Godiva low-population traces at 500/1000/2000/5000: same size, 0.16-0.23 at
W = 30.) The k half of the criterion does not discriminate transient from
stationary on any case (Geweke 0.47-0.48 against 0.49); the entropy half
carries all the discrimination, and only for the fast transient.

Entropy half alone, W = 30, by mesh (pair A, `results/windows_pairA.md`):
stationary / transient pass 3^3 0.29 / 0.03, 5^3 0.31 / 0.06, 8^3 0.31 / 0.04,
12^3 0.30 / 0.03. Pair B: 0.32-0.34 / 0.29-0.35 on every mesh.

### One threshold at a time (replay)

Never-pass share, false pass among declared, bias in pcm. Standard errors on
bias 17-32 pcm; a count of n/40 has a binomial 95 % interval about +/- 15
points.

| setting | Godiva never / bias | pair A never / bias | pair B never / false / bias | HST-009 never / bias | cost (Godiva) |
|---|---|---|---|---|---|
| default | 16 / -24 | 27 / -8 | 28 / 12 of 12 / +49 | 13 of 24 / -10 | 0.530 |
| W = 20 | 24 / -26 | 25 / -13 | 29 / 11 of 11 / +43 | 13 / +1 | 0.518 |
| W = 50 | 31 / -9 | 34 / -9 | 32 / 8 of 8 / +50 | 22 / 0 | 0.640 |
| E = 1 | 2 / -41 | 10 / -30 | 11 / **29 of 29** / +32 | 0 / +1 | 0.493 |
| E = 10 | 31 / -15 | 31 / -10 | 31 / 9 of 9 / +48 | 19 / 0 | 0.520 |
| S = 0 | 15 / -17 | 24 / -7 | 24 / 16 of 16 / +47 | 9 / 0 | 0.485 |
| S = 5 | 15 / -22 | 24 / -7 | 25 / 15 of 15 / +49 | 12 / -9 | 0.501 |
| S = 20 | 20 / -19 | 30 / -9 | 32 / 8 of 8 / +51 | 15 / -6 | 0.592 |
| mesh 3 | 23 / -22 | 30 / -9 | 26 / 14 of 14 / +40 | 10 / -3 | 0.521 |
| mesh 8 | 18 / -23 | 23 / -4 | 29 / 11 of 11 / +44 | 14 / -8 | 0.543 |
| mesh 12 | 18 / -11 | 23 / -4 | 22 / 18 of 18 / +39 | 12 / -8 | 0.540 |
| P = 500 | 20 / -25 | 27 / 0 | 27 / 13 of 13 / +47 | 11 / -7 | 0.516 |
| P = 2000 | 19 / -16 | 30 / -4 | 26 / 14 of 14 / +47 | 10 / -3 | 0.556 |
| P = 5000 | 23 / -12 | 32 / -7 | 27 / 13 of 13 / +48 | 6 / -6 | 0.711 |
| alpha off | 14 / -24 | 25 / -5 | 27 / 13 of 13 / +49 | 12 / -9 | 0.532 |
| alpha 0.01 | 15 / -24 | 25 / -5 | 27 / 13 of 13 / +49 | 12 / -9 | 0.532 |
| alpha 0.10 | 18 / -22 | 29 / -9 | 28 / 12 of 12 / +49 | 13 / -10 | 0.525 |

On pair B every declaration at the default was false; none of the
declarations on the other three were (0 of 24, 13 and 11 declared). Differences between rows of 10
percentage points or less are inside the binomial noise.

### Replay against live

The same protocol run live on new seeds (`results/live_*.md`, 12 seeds Godiva
and pair B, 6 seeds HST-009 default). Agreement is within binomial error:

| | replay | live |
|---|---|---|
| Godiva default: mean `n_in`; never pass | 92.2; 16/40 | 93.3; 7/12 |
| Godiva E = 1: mean `n_in`; never pass | 81.8; 2/40 | 82.0; 0/12 |
| Godiva W = 50: never pass | 31/40 | 10/12 |
| pair B default: mean `n_in`; never pass | 98.1; 28/40 | 96.2; 8/12 |
| pair B E = 1: mean `n_in`; never pass | 88.6; 11/40 | 82.0; 1/12 |
| HST-009 default: mean `n_in`; never pass | 93.8; 13/24 | 93.3; 4/6 |

The live pair B active-cycle k at the default is 1.00168 +/- 0.00061, which is
-48 +/- 61 pcm from the 500-cycle `k_ref` (replay +49 +/- 20): 1.6 sigma
apart, so the live runs do not resolve the replay bias. The replay's
assumption (transient independent of population) is not contradicted by any
of the compared counts; it is not proven by 12 seeds.

## Deviations from the posted design (all disclosed)

1. **Pair B's reference was lengthened.** The first reference used 250-cycle
   traces with `D_ref` 150; its plateau had a slope of -0.48 `tau` per 100
   cycles (not flat) and gave `k_ref` 1.00265 and a bias of +0.4 +/- 20.2 pcm
   (`results/replay_pairB_250cycle_reference_SUPERSEDED.md`). I re-recorded
   pair B at 500 cycles with `D_ref` 300 (slope +0.04 `tau`/100 cycles, flat);
   `c_true` is 120 on both and `k_ref` fell to 1.00216, which moves the bias
   to **+49.3 +/- 20.2 pcm**. The 250-cycle bias was an artefact of a
   reference still converging. The README quotes the 500-cycle numbers.
2. **The size/power analysis (`windows`) was added after seeing** that Godiva
   hits the cap in 40 % of runs. It is a diagnostic written after the first
   result, not part of the posted design.
3. HST-009 used 24 seeds (design said 8) because a seed costs 51 s; live
   HST-009 used 6 seeds at the default only. The baseline for a `c_true` of 0
   is 0 histories.
4. Bias uses 100 active cycles, not the preset's 250 (it is a mean shift;
   fewer cycles only add noise).

## Recommendations (nothing is changed in `run.rs`)

| constant | verdict | evidence |
|---|---|---|
| `WATCH_WINDOW_CYCLES` = 30 | **Cannot be justified at this cost, and neither can 20 or 50.** | Per-window size is 0.11 / 0.21 / 0.26 at W = 20 / 30 / 50 on stationary Godiva windows; the cap is hit in 60 / 40 / 78 % of Godiva runs. A longer window raises the pass rate slowly while spending W more production cycles; it lowers the share of declarations, not the share of false ones. The cause is in the tests at these lengths (30 cycles = 6 MSER-5 batches; Geweke's first window is 3 cycles), not in the number 30. |
| `WATCH_EVERY_CYCLES` = 5 | **Keep at 5 or larger; do not go to 1.** | E = 1 cuts the never-pass share (Godiva 40 -> 5 %) but on the slow pair it makes 72 % of all runs falsely declare convergence (29/40, against 30 % at E = 5). More looks at a test with 13-16 % per-window pass on a drifting source buy false passes. E = 5 against 10 is not resolved at these sizes. |
| `SETTLE_CYCLES` = 10 | **Not shown to matter.** | S = 0 / 5 / 10 / 20 change bias by at most 10 pcm (SE 17-32) on all four cases; S = 0 saves 0.045-0.06 Mhist (9-11 % of the default inactive cost, by shortening the low-population stage's cap) for no measured loss. A candidate to reduce, but no case here exercises what the settle is for (a population change that disturbs the source), so this is absence of evidence. |
| `ENTROPY_MESH_DIVISIONS` = 5 | **Neither supported nor refuted; 3, 5, 8, 12 are indistinguishable here.** | Pair A transient windows pass 0.03-0.06 on every mesh. The cases test only a two-lump asymmetry; a source with fine structure (LCT-008 pins) was not tested. |
| `WATCH_PARTICLES` = 1000 | **No benefit from larger; no cost shown for 500.** | Size of the window criterion is the same at 500-5000; 5000 raises cost from 0.53 to 0.71 Mhist with no change in bias or n_in. 500 saves 0.04 Mhist. The low-population stage is not where the cost is (the confirmation at 10 000 is). |
| `WATCH_NORMALITY_ALPHA` = 0.05 | **Harmless; its value is not tested by these traces.** | It passes 0.94-0.96 of stationary windows and changes the cap-hit count by 1-2 runs of 40 and bias by <= 3 pcm. Every window here was near-normal, so the gate never had a skew to refuse; its power is in #944's size/power table. alpha 0.01 and 0.05 are indistinguishable. |
| `MAX_INACTIVE_CYCLES` = 100 (not asked) | **Binding on the slow pair.** | Pair B needs 120 cycles; the bias of +49 pcm is the cost of stopping at 100. |

**Overall.** The six thresholds cannot be justified as a set, because the
criterion they feed is badly sized (rejects 78-87 % of converged windows) and
blind to a slow source transient. Choosing among values of W, E, S, mesh, P or
alpha cannot repair that; the follow-up is a different or re-sized stationarity
test (Brown's practice, "examine k and H_src", is a human reading of the
trace). Not tried here, and not the maintainer's decision yet.

**What the evidence does support.** On a converged problem Auto costs the cap
(about 0.5 Mhist) regardless of the problem; on a fast transient it reaches the
right discard through the cap, not through the watcher; on a slow transient it
will sometimes declare early and wrongly.

## Open questions

1. Is a per-window pass rate of 13-26 % on stationary data an intended
   property of `TraceDiagnosis::converged` (it is a pair of tests built for a
   whole run trace, `stats::convergence`), or a defect of applying it to a
   30-cycle window? The module's own tests only exercise clear cases.
2. Which cap is intended for a slow-source model: 100 stops pair B 20 cycles
   short.
3. The settle step needs a case that exercises it (a source that is not yet
   in its fundamental mode at the population change).
4. The mesh needs a case with fine source structure.

## Compute and hardware

Intel i9-13900K (32 logical cores, shared with other jobs; this campaign pinned
to logical cores 0-3 with `taskset`, `RAYON_NUM_THREADS=4`), 62 GiB RAM, Linux
7.2.9, CPU only (Vulkan ICD hidden). About **71 minutes wall** of 4 cores
(2026-10-19 03:58-05:09) plus about 10 minutes of earlier trials and builds. By
case: recording Godiva / pair A / pair B 40 seeds at 5.1 s per seed, pair B at
500 cycles 9.2 s per seed, HST-009 24 seeds at 51 s per seed; live runs about
10 minutes each; every process start spends about 75 s processing the ENDF
tapes.

## Reproduce

```text
export RAYON_NUM_THREADS=4 VK_ICD_FILENAMES=/nonexistent VK_DRIVER_FILES=/nonexistent
cargo build --release -p outram-mc-libs --example auto_watcher_927
B=target/release/examples/auto_watcher_927
taskset -c 0-3 $B check godiva
taskset -c 0-3 $B record godiva 0 40 /tmp/w927/data           # also pairA, pairB
taskset -c 0-3 $B record hst009 0 24 /tmp/w927/data x 200
taskset -c 0-3 $B record pairB 0 40 /tmp/w927/data_long x 500
$B analyse godiva /tmp/w927/data 150                           # pair B: data_long 300; hst009: data 100
$B windows godiva /tmp/w927/data 150 20
taskset -c 0-3 $B live godiva 0 12 all                         # hst009: 0 6 default
```

## Data provenance

ENDF/B-VIII.0 tapes in `reference-data/endf/` (open data, processed by the
crate's NJOY port at run time); Godiva and HST-009 specifications from
`vv::godiva` and `vv::hst009` (HEU-MET-FAST-001, HEU-SOL-THERM-009, public
benchmark descriptions as recorded in those modules). The two-sphere cases are
synthetic and make no benchmark claim. Brown, LA-UR-08-06558 (M&C 2009) is the
practice cited. No restricted data used.

## Bookkeeping status

| axis | state |
|---|---|
| V&V stage | [ ] unchecked |
| Human review | [ ] unchecked |
