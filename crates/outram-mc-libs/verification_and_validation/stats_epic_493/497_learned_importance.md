# #497 — variance reduction from learned importance: V&V protocol (stub)

**Issue:** [gh:#497](https://github.com/theodoreOnzGit/outram-park-backend/issues/497), epic #493.
**Code:** `src/stats/learned_importance.rs` (`learned_weight_windows`,
`VrArm`, `FomComparison`); `raffles::surrogate::{PolynomialSurrogate,
validation::LeaveOneOut}`.

> **Status: MEASURED 2026-11-14** (see Results). The protocol and the
> prediction below were written and committed (`3487cdaae`) before any run.

## Off by default — and why

This is an **estimator**, not physics: weight windows play fair split /
roulette games that leave every expected score unchanged. So, as for
`physics/variance_reduction.rs`, analog is the correct default and the root
`CLAUDE.md` "correct physics is the default" rule does not apply. Nothing here
runs unless a caller builds the windows and attaches them with
`VarianceReduction::with_weight_windows`.

## What is being checked

1. **Unbiasedness (the gate).** The tally mean with learned windows agrees
   with the analog mean. A biased answer fails regardless of FOM.
2. **Figure of merit.** `FOM = 1/(R²T)`, `R` the relative error of the tally,
   `T` the wall-clock, compared against (a) analog and (b) plain MAGIC
   windows (`WeightWindows::update_magic`) built from the **same** early
   tally. Ratio reported with its uncertainty
   (`FomComparison::fom_ratio_sigma`).
3. **Fill quality.** Per energy group: resolved / filled / clamped / empty
   cell counts, and the surrogate's leave-one-out RMSE in `ln φ`.

## Methodology

- Case: the shielded room of `tests/openmc_notebooks/shielded_room_weight_window.rs`
  (31 × 38 mesh, the case `variance_reduction/fom_2026_09_24.md` measured
  MAGIC on), so the MAGIC arm has a recorded precedent. Same geometry,
  materials, source and mesh in all three arms.
- Early tally: the analog arm's first batches (count fixed before the run);
  windows built once, then held fixed for the measured run.
- All three arms on one machine, hardware recorded (crate `CLAUDE.md`).

## Pass criteria (fixed in advance, 2026-10-03)

- **Unbiased:** `|z| ≤ 2` (`UNBIASED_Z_BAND`), `z = Δmean / sqrt(σ_a² + σ_v²)`,
  for the headline tally and for every mesh cell both arms resolve (with the
  expected ~5 % of cells outside 2σ by chance stated, not hidden).
  **Open question for the maintainer:** #497 says "within combined σ"; read
  literally as 1σ, a correct estimator fails 32 % of the time. 2σ is used
  here and flagged, decided before any result exists.
- **FOM:** reported, with uncertainty, against analog and MAGIC. No FOM
  threshold is a pass/fail gate — the issue asks for a measured ratio.

## Protocol for the 2026-11-14 measurement (fixed before any run)

Test: `shielded_room_learned_importance` in
`tests/openmc_notebooks/shielded_room_weight_window.rs`. Command:
`cargo test --release -j 2 -p outram-mc-libs --no-default-features --test openmc_notebooks shielded_room_learned_importance -- --nocapture --ignored`.

- **Seeds.** N = 5 independent seeds per arm; each seed is one
  `run_fixed_source` call of `n` particles in 10 batches. The estimate is the
  mean over seeds of the per-run headline; its sigma is the seed-to-seed sample
  standard deviation / sqrt(N). `VrArm.n_batches` = N = 5.
- **Headline tally.** Flux per source particle summed over mesh cells whose
  centre lies 900-1450 cm (xy distance) from the source: the "far field" band
  of `fom_2026_09_24.md`, chosen because it is the band where MAGIC was
  previously seen to pay. The 0-400 and 400-900 cm bands are reported too; all
  three were fixed here, not after seeing results.
- **Time.** T = total wall-clock of the N runs of that arm (transport only);
  the early-tally generation run (shared by MAGIC and learned) is reported
  separately and the FOM is given both without and with it charged.
  FOM = 1/(R^2 T), R = sigma/|mean| of the N-seed mean.
- **Early tally.** One analog run, 2000 particles / 10 batches, seed distinct
  from every measured seed. MAGIC (`update_magic`, threshold 1.0, ratio 5, as
  in the 2026-09-24 precedent) and learned (`learned_weight_windows`, same
  threshold/ratio, other settings default) are built once from that same tally
  and held fixed.
- **Particle counts per seed** are set from a cost probe (time per particle),
  never from tallies, so that all arms finish in well under an hour on this
  4-core box; they are printed in the results.
- **Unbiasedness**: z = (mean_vr - mean_analog)/sqrt(s_a^2 + s_v^2), reported
  against BOTH bands, |z| <= 1 and |z| <= 2. No pass/fail is chosen. Per-cell
  z is computed over cells with a nonzero score in every seed of both arms.

## Prediction (written 2026-11-14 BEFORE any run)

1. **Bias.** The argument says none. Each VR arm's headline z against analog
   is expected inside |z| <= 2 (probability about 0.95 each if unbiased) and
   inside |z| <= 1 with probability about 0.68 each, so about 0.47 that both
   pass the 1-sigma reading by chance alone. A |z| > 2 would be a real finding
   (a defect in the game), not noise to explain away. Per-cell: about 32 % of
   cells outside 1 sigma and 5 % outside 2 sigma expected by chance.
2. **FOM vs analog.** From `fom_2026_09_24.md` (MAGIC far-field ratio 0.252x,
   cost ratio 5.7x) I predict MAGIC FOM/analog in **0.1-0.5, point 0.25**:
   below 1, i.e. no net FOM gain on the cells the analog arm resolves, with the
   early tally here being 25x smaller (2000 vs 50000 particles) so fewer windows.
3. **Learned vs MAGIC.** I predict **learned/MAGIC in 0.5-1.5, point 0.8**, and
   more likely below 1 (about 60 %) than above. Reason: a degree-2 polynomial
   in ln(flux) over (x, y) cannot represent an L-shaped bunker with 200 cm
   concrete walls and a corridor; filled cells beyond the walls get windows
   from a smooth extrapolation that is wrong in level, and the extra windowed
   cells add splitting cost (the cost pathology recorded for deep MAGIC sets).
   Learned/analog is therefore predicted **0.05-0.5, point 0.2**. A learned
   arm that beats analog (ratio > 1) would surprise me (about 10 %).
4. **LOO RMSE in ln(flux)** is predicted large (> 1, i.e. worse than a factor
   e) because of the wall discontinuities.

## Results (measured 2026-11-14)

> **Status: MEASURED.** Supersedes the "NOT YET MEASURED" stub. Run A is the
> pre-declared measurement; run B is a replicate with fresh transport seeds
> (same windows), added because run A's MAGIC arm looked biased and one
> replicate was the cheapest way to tell noise from a shift. Both are shown.

**Hardware (every timing below).** Intel Xeon @ 2.80 GHz, 4 logical cores,
15 GB RAM, Linux 6.18.44, CPU only. `run_fixed_source` is single-threaded, so
each run used 1 thread. **The box was shared and loaded** (load average
2.0-2.2 during the runs, other agents compiling), so absolute seconds are not
reproducible; only same-run ratios mean anything.
**Commit:** `35b3a1cd9` (test code; run A used the same code before the
`OUTRAM_LI_SEED_OFFSET` knob was added, offset 0).
**Commands:** A: `shielded_room_learned_importance --ignored --nocapture` from
`cargo test --release -j 2 -p outram-mc-libs --no-default-features --test
openmc_notebooks`; B: the same with `OUTRAM_LI_SEED_OFFSET=5000000`.
**Sizes (set from a 2-seed cost probe, not from tallies):** early analog tally
2000 particles; analog 40 000 particles/seed; MAGIC and learned 20 000
particles/seed; N = 5 seeds per arm.
**Windows:** MAGIC 592/1178 cells carry a window; learned 1178/1178 (586
filled, 0 clamped, leave-one-out RMSE in ln flux **1.28**, i.e. a typical
held-out error of a factor 3.6). Early tally 2.0 s (not included in the arm
times; the "generation charged" variants in the raw output add it and move no
ratio at the third digit).

### Headline band (900-1450 cm from source), flux per source particle, summed over 317 cells

| run | arm | mean | sigma (seed sem) | R | T total (s) | z vs analog | FOM |
|---|---|---|---|---|---|---|---|
| A | analog | 4.7532 | 0.0651 | 0.0137 | 177.0 | - | 30.1 |
| A | MAGIC | 4.4778 | 0.0468 | 0.0105 | 357.6 | **-3.43** (\|z\|<=1 no, \|z\|<=2 no) | 25.6 |
| A | learned | 4.8074 | 0.1700 | 0.0354 | 325.1 | +0.30 (<=1 yes, <=2 yes) | 2.47 |
| B | analog | 4.7699 | 0.1040 | 0.0218 | 158.1 | - | 13.4 |
| B | MAGIC | 4.5929 | 0.0636 | 0.0138 | 340.2 | -1.45 (<=1 no, <=2 yes) | 15.3 |
| B | learned | 4.7507 | 0.1220 | 0.0257 | 328.8 | -0.12 (<=1 yes, <=2 yes) | 4.62 |

FOM = 1/(R^2 T), R and T of the N = 5 seed mean and total time. The harness's
`fom_ratio_sigma` (from the sampling error of a sigma estimated from 5 seeds)
is **~100 % of the ratio at N = 5**, so a single-run ratio is a scale, not a
measurement: run A and B give MAGIC/analog 0.85 and 1.15, learned/analog 0.082
and 0.345.

| FOM ratio, headline band | run A | run B | pooled A+B (N = 10, hand-computed from the table, +-1 sigma) |
|---|---|---|---|
| MAGIC / analog | 0.849 +- 0.849 | 1.146 +- 1.146 | **1.05 +- 0.70** |
| learned / analog | 0.082 +- 0.082 | 0.345 +- 0.345 | **0.18 +- 0.12** |
| learned / MAGIC | 0.097 +- 0.097 | 0.301 +- 0.301 | **0.17 +- 0.11** |

Pooled arm means (mean of the two runs; sem = sqrt(s_A^2 + s_B^2)/2): analog
4.762 +- 0.061, MAGIC 4.535 +- 0.040 (**-4.8 %, z = -3.1 vs analog**), learned
4.779 +- 0.105 (z = +0.14). The pooled FOM ratios use T summed over the two runs
and the relative FOM sigma 2/sqrt(2(N-1)) at N = 10.

### Other bands and per-cell (both runs, raw)

| band | run | MAGIC z | learned z | MAGIC FOM/analog | learned FOM/analog | learned/MAGIC |
|---|---|---|---|---|---|---|
| 0-400 (196 cells) | A | -1.61 | -0.94 | 0.228 | 0.461 | 2.02 |
| 0-400 | B | +0.11 | -1.65 | 0.057 | 0.240 | 4.18 |
| 400-900 (665 cells) | A | **-2.41** | -0.06 | 0.082 | 0.057 | 0.69 |
| 400-900 | B | +0.35 | +1.58 | 0.385 | 0.317 | 0.82 |

(each ratio +- 100 % at N = 5.) Per-cell z over cells scored in every seed of
both arms (about 667): outside 1 sigma, MAGIC **34.0 % / 34.5 %**, learned
**34.2 % / 34.5 %** (chance: 31.7 %); outside 2 sigma, MAGIC 7.9 % / 7.3 %,
learned 8.1 % / 8.8 % (chance: 4.6 %). Neighbouring cells and seeds are
correlated and the per-seed sigma is itself estimated from 5 values, so the
chance rates are only a guide; both arms sit at about 1.6-1.9x the 2-sigma
chance rate.

### Unbiasedness, reported under BOTH bands (no band chosen)

| arm | headline z (A, B) | \|z\|<=1 | \|z\|<=2 | comment |
|---|---|---|---|---|
| learned | +0.30, -0.12 | pass, pass | pass, pass | but 3 of 6 band-runs on the other bands sit at 1.5-1.7 |
| MAGIC | **-3.43**, -1.45 | **fail, fail** | **fail**, pass | pooled -4.8 %, z = -3.1 |

**Finding.** The learned-importance arm shows no resolvable bias on the
headline band under either reading. The existing **MAGIC arm (the
`weight_windows.rs` path, not new code) shows a repeatable negative shift** in
the far field, -5.8 % in A and -3.7 % in B, pooled -4.8 % at about 3 sigma;
it also fails 2 sigma at 400-900 cm in run A (-2.41) and passes there in B.
~~This is **not explained**.~~ **UPDATED 2026-11-15 (gh:#1579): see "MAGIC shift investigation" below; the -4.8 % did not reproduce (40 fresh seeds: -1.8 %, z -1.5) and no code defect was found.** It was either a real bias in the windowed transport
or windowed scoring (the 2026-09-24 record showed +2.21 % in total flux with
a 25 % bound, which could not see this), or a heavy tail that 5-seed sigmas
underestimate: weight windows make rare large scores rarer, and a sample sigma
from 5 seeds underestimates a heavy-tailed variance. Nothing here separates the
two, and nothing was tuned. It is filed for the maintainer, not repaired. The
learned arm inherits the same transport game, so its not showing the shift is
likely a matter of its larger sigma (0.12-0.17 against 0.05-0.06), not of
being more correct: a +-0.1 sigma is about +-2 % of the mean and could hide a
-4.8 % shift at 2 sigma, so **"learned is unbiased" is "not distinguishable
from analog or from MAGIC at this precision"**, not a proof.

### Against the prediction (written before any run)

| prediction | measured | verdict |
|---|---|---|
| no bias, \|z\|<=2 with p ~ 0.95 each | learned yes; **MAGIC fails 2 sigma in run A** (headline and 400-900) | **miss for MAGIC** |
| MAGIC/analog 0.1-0.5, point 0.25 | 0.85 and 1.15 (pooled 1.05 +- 0.70) | **miss**: MAGIC was better than predicted, about break-even with analog in the far field |
| learned/MAGIC 0.5-1.5, point 0.8, below 1 more likely | 0.097, 0.301 (pooled 0.17 +- 0.11); in the nearer bands 0.69-4.2 | **miss** on the headline: learned is clearly worse than MAGIC there; the direction (below 1) was the likelier one |
| learned/analog 0.05-0.5, point 0.2 | 0.082, 0.345 (pooled 0.18 +- 0.12) | **hit** |
| LOO RMSE in ln flux > 1 | 1.28 | **hit** |

**Which numbers to quote.** The pooled N = 10 headline ratios, with their
~65 % relative errors, and the sign: learned importance **loses on FOM** to
both analog (about 0.2x) and MAGIC (about 0.2x) in the far field, and in no
band and run did it beat analog. The nearer bands (learned/MAGIC > 1 in 0-400
cm) are single noisy ratios with ~100 % errors and do not support a claim.
Why learned loses is not tested here; the likeliest cause remains the
prediction's (a degree-2 polynomial in ln flux across an L-shaped bunker
with 200 cm walls, LOO RMSE 1.28), and it filled all 586 unresolved cells,
including those behind the shield, where a wrong level costs variance.

### Limitations

N = 5 seeds per arm per run is the floor asked for; sigma and its error are
both poor at that N. One geometry, one early-tally size (2000 particles), one
mesh (31 x 38, the 50 cm default that `fom_2026_09_24.md` calls
misconfigured for weight windows), one MAGIC iteration count (windows built
once). The gate decision (|z| <= 1 vs <= 2) is still the maintainer's.

## Source biasing (2026-11-14)

**Already in the crate, not new.** OpenMC's source `bias` (sample from `q`,
weight `p/q`) is ported in `src/source/distribution.rs`
(`Distribution::with_bias`, `discrete_biased`, `mixture_biased`,
`compute_importance_weights`; spatial biased variants too), from OpenMC commit
`8f79d9d02483c90e4134f03a9df4b75ce6b062bf`, `src/distribution.cpp:34`
(`compute_importance_weights`) and `include/openmc/distribution.h`, and
bit-compared with compiled OpenMC in
`tests/source_distributions_vs_openmc_code_to_code.rs`
(`verification_and_validation/upstream_openmc_samplers/README.md`). The OpenMC
source tree itself is not on this machine (`/home/teddy0/...` absent, no
`vendor/openmc`), so **this session did not re-read upstream's lines**; the
provenance is the port's own headers and its recorded bit-comparison. It is
opt-in (`with_bias`); an unbiased source has `w = 1` exactly. Importance-map
driven source biasing (from learned windows) is **not** implemented.

**New test:** `tests/source_biasing_unbiased_vs_analog.rs`, `p = Uniform(1, 4
MeV)`, `q = PowerLaw(n = 2)`, 5 seeds x 100 000 draws through
`IndependentSource::sample`. z of biased against analog, both bands:

| estimator | analog | biased | z | \|z\|<=1 | \|z\|<=2 |
|---|---|---|---|---|---|
| E[w] (truth 1) | 1 exactly | 0.99958 +- 0.00081 | -0.52 | yes | yes |
| E[wE] (2.5e6) | 2.49972e6 +- 1.42e3 | 2.49940e6 +- 8.5e2 | -0.19 | yes | yes |
| E[wE^2] (7e12) | 6.99752e12 +- 7.5e9 | 7.0000e12 (zero spread: w E^2 constant for n = 2) | +0.33 | yes | yes |
| P(E<2 MeV) (1/3) | 0.33297 +- 4.1e-4 | 0.33239 +- 7.6e-4 | -0.67 | yes | yes |

Biased is unbiased in the mean within both bands. Two test-design misses
before this table, kept on record: `n = 1` makes `wE` constant (zero-variance,
z undefined), and a first run with `n = 0.5` exposed the defect below.

**Upstream defect found (reproduced, not fixed).** With a **non-integer**
`PowerLaw` bias exponent the weight is wrong: `PowerLaw::evaluate` truncates
`int pwr = n() + 1`, so `p/q` is off by `(n+1)/trunc(n+1)`. Measured at
`n = 0.5`: **E[w] = 1.50006** (predicted 1.5 from reading `evaluate`), a 50 %
bias on every tally using that source; the sampler is exact, only the weight
is wrong. Pinned by `non_integer_power_law_bias_weight_is_upstreams_defect`.
The port copies upstream (the bit-comparison covers integer `n` only), so
fixing it would diverge from OpenMC: **maintainer decision**; whether current
OpenMC still has this was not checked here (no upstream source available).


## MAGIC shift investigation (2026-11-15, gh:#1579)

**Question.** Is the far-field (900-1450 cm) MAGIC shift of -4.8 % (z -3.1 at
5 + 5 seeds) a real bias in the windowed path, or a heavy tail that 5-seed
sigmas underestimate? Hypotheses and predictions were posted on #1579 before
any run (H1 heavy tail, about 60 %: 20+ seed MAGIC/analog within |z| <= 2 of 1;
H2 about 35 %: a real bias from the surface checkpoint, sign negative, -3 to
-6 %, persisting at 20 seeds and vanishing with the surface checkpoint off).

### Step 1: the port read against upstream (OpenMC `8f79d9d0`, `upstream_source/OpenMC`)

Compared: `apply_weight_window(s)`, `get_weight_window`, `search_weight_window`,
`Particle::split`, `initialize_history` (`simulation.cpp:735-760`), the call
sites in `physics.cpp:67` and `particle.cpp:385`, and `settings.cpp:89-90`
against `src/physics/weight_windows.rs` and `weight_window_checkpoint` in
`transport_csg.rs`.

**Matches line for line:** the cutoff kill; `ww_born` anchoring and
`scale(wgt_born / wgt_ww_born)`; `max_lb_ratio` / `ww_factor`; the dead-banded
split count `max(2, ceil(w / ((1+tol) ub)))` capped at `max_split` with
`weight / n_split` to every copy; `weight_survive = min(w * max_split,
survival_weight)`; roulette `survive * prn < w`; `max_history_splits`;
energy-bin lookup and no window outside the mesh or energy range.

**Differences (none is a weight-bookkeeping error; each game is fair):**

| # | upstream | port | status |
|---|---|---|---|
| 1 | surface checkpoint default OFF (`settings.cpp:89`), collision only | plays the window at both | knob added, default left ON (`with_weight_window_checkpoint_surface`); measured below |
| 2 | window applied at birth (`simulation.cpp:759`) | first applied at the first surface/collision, so `ww_born` anchors there | not changed, not measured |
| 3 | `split()` banks `wgt_born`, `wgt_ww_born`, `n_split`, not `ww_factor` | daughters copy `ww_factor` too | no effect while `max_lb_ratio = 1` (this test); not changed |
| 4 | daughter banked with `surf_id` | daughter at a surface re-located with `on_surface = NONE` | measured via 1 (surface checkpoint off removes it) |
| 5 | no window at a surface checkpoint sets `wgt_ww_born = 1` | left at -1 | not changed |

### Step 2: measurement

Test `shielded_room_magic_bias_hunt_arm` + `shielded_room_magic_bias_report`
(`tests/openmc_notebooks/shielded_room_weight_window.rs`). Same windows as the
2026-11-14 run (early analog tally 2000 histories, seed 424 242, threshold 1,
ratio 5). Arms: analog (40 000 histories per seed), MAGIC (20 000), MAGIC with
the surface checkpoint OFF (20 000, the upstream default). 20 seeds per arm per
replicate, two replicates with fresh transport seeds (offset 0 and
500 000 000) = 40 seeds per arm. Each seed is split into one-batch chunks of
200 histories, so the chunk-score spread is a tail diagnostic.
**Hardware:** Intel Xeon @ 2.80 GHz, 4 logical cores, 15 GB RAM, Linux
6.18.44, CPU only; three single-threaded processes at once on a **shared and
loaded box** (load average 4-9), so no timings are quoted. **Sizes** were set
to fit about an hour on this box.

Headline band, flux per source particle summed over 317 cells, mean +- seed-to-seed sem:

| replicate | analog | MAGIC | z | MAGIC, surface checkpoint OFF | z |
|---|---|---|---|---|---|
| C (seeds 0-19) | 4.7623 +- 0.0686 | 4.6328 +- 0.0598 (0.9728) | -1.42 | 4.6406 +- 0.0758 (0.9744) | -1.19 |
| D (seeds 20-39) | 4.7217 +- 0.0552 | 4.6828 +- 0.0438 (0.9917) | -0.55 | 4.6828 +- 0.0742 (0.9918) | -0.42 |
| C + D, 40 seeds | 4.7420 +- 0.0436 (0.92 %) | 4.6578 +- 0.0368 (0.9822, **-1.8 %**) | **-1.48** | 4.6617 +- 0.0524 (0.9831) | -1.18 |

Tail diagnostics (chunk of 200 histories; C + D pooled; for a Gaussian the
relative VOV is about 2 / n_chunks = 0.0005 analog, 0.0010 MAGIC):

| arm | chunk VOV | skew | largest chunk / mean chunk | top-5 chunks' share of the total |
|---|---|---|---|---|
| analog | 0.0007 | 1.49 | 7.6x | 0.43 % (uniform 0.06 %) |
| MAGIC | 0.0024 | 1.90 | 7.3x | 0.70 % (uniform 0.125 %) |
| MAGIC, surface OFF | 0.0031 | 2.20 | 7.4x | 0.81 % |

### Verdict

1. **No code defect found, and nothing was changed to move the number.** The
   game matches upstream where it matters (above); the five differences are
   fair games. The surface checkpoint (differences 1 and 4) is **not** the
   cause: with it off, MAGIC reads 0.9831 against 0.9822 (H2's prediction,
   that the shift vanishes with it off, **missed**).
2. **The -4.8 % did not reproduce.** On 40 fresh seeds the shift is **-1.8 %,
   z -1.48** (replicates -2.7 % z -1.42 and -0.8 % z -0.55), so the
   original pair was a selected outlier (run A, z -3.43, was the one that
   prompted the replicate). H1's prediction (|z| <= 2 of 1.00) **hit**.
3. **A small negative residual is not excluded.** All four runs (A, B, C, D)
   have a negative z. Pooling every run by inverse variance gives about -3.3 %,
   z about -3.7, but that includes the selected run A and is not a clean
   estimate; B, C, D alone give Stouffer z about -2.0. The honest reading is
   **MAGIC reads between 0 and -3 % low in the far field, unresolved at this
   precision**. Resolving 1.8 % at 3 sigma needs the sem on the ratio down to
   0.6 %, about 9 times the histories of this run (several hours here).
4. **The tail is real and heavier under MAGIC.** MAGIC's chunk VOV is 3.4x
   analog's (2.4x and 5x the Gaussian expectation), skew 1.9 against 1.5. The
   largest chunk is about 7x the mean for both arms, so the sampled tail is
   similar; what a chunk of 200 histories cannot show is a rarer, larger event
   that none of 400 000 histories drew. A right-skewed score with an unsampled
   far tail gives sample means that sit **below** the truth in most
   replicates, which is the sign seen, but this was **not tested** (it needs a
   per-history maximum-weight diagnostic in the transport loop).
5. **Which numbers to quote:** the 40-fresh-seed ratio 0.982 (about -1.8 %,
   z -1.5, sem on the ratio about 1.2 %), with the statement above; not the
   -4.8 %.

### Reviewed functions edited

`outram_mc_libs::physics::transport_csg` history kernel: one line in the
surface-crossing checkpoint (`if vr.weight_window_checkpoint_surface && ...`),
default behaviour unchanged. `review.md` was not touched. Added
`VarianceReduction::with_weight_window_checkpoint_surface` and the field
`weight_window_checkpoint_surface` (default `true`).

### Open follow-ups

Upstream parity for differences 2, 3 and 5, and whether the default for 1
should become upstream's (OFF), would change every recorded window V&V number
and are left to the maintainer.
