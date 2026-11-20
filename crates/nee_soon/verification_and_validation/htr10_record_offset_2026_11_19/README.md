# HTR-10 whole core: is the 2026-11-05 record reproduced on today's code?

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.
> Not validated. Predictions: [`PREDICTION.md`](PREDICTION.md), committed in
> `34515406d` before any run.

**Answer: yes, within the statistics.** Four independent seeds on today's code
average **0.993020**, which is **−229 ± 175 pcm** from the record 0.995309 ±
0.001098 (z −1.3). The −422 pcm that prompted this check was one seed's draw.

The thread count does not change the answer: 1 and 2 threads give a
bit-identical k.

## Why this was run

The gh:#550 third re-measure (part 2) ran the HTR-10 whole core as a pair of
arms: the new S(α,β) default, and the old build via `OUTRAM_SAB_LOG_SPACED_GRID=1`.
Both arms read 350–420 pcm below the 2026-11-05 record
(`../leapr_default_remeasure_2026_11_05/`). That record is 0.995309 ± 0.001098,
at 10000 × [5+135], 16 threads, seed 20260917, commit `7fd23529e`.

Because the old-build arm was as low as the default, the offset was not the
#550 change. There were three remaining explanations:
- code drift since 2026-11-05;
- the different protocol (1 thread instead of 16, and a smaller run);
- a seed draw.

## Methodology

**Model.** Every run below uses `examples/htr10_rmc_keff.rs`, the default model
(LEAPR graphite and SiC laws, the #550 node-grid S(α,β)), 14 rings × 12 layers,
`--release`, and the binary built from `516f559f8` (`logs/commit.txt`).

**Why the test is statistical.** A bit-level comparison against `7fd23529e`
cannot decide anything. Between the two commits, the OpenMC kernel and seeding
work (gh:#1386, #1444) changed the CSG path's random stream by design, so the
two commits differ at equal settings even if the physics is identical.

**T: thread-count independence.** The example's doc said this was assumed and
never tested for the hybrid CSG driver.
- Settings: 500 × [1+2], seed 20260917, `OUTRAM_HTR10_THREADS=1` and `=2`.
- Pass criterion, fixed in advance: every printed digit of k and its sd is
  identical.

**S: seed ensemble.**
- Settings: H1's protocol, 4000 × [5+90] with `OUTRAM_HTR10_THREADS=1`.
- New seeds: 20260918, 20260919 and 20260920 (`OUTRAM_HTR10_SEED`), one
  process each, run side by side.
- The fourth sample is H1's default arm (seed 20260917, commit `6b3b64807`).
  No `src/` file reached by this example changed between `6b3b64807` and
  `516f559f8`. That is assumed, not re-run.

**Uncertainty on the mean.** It is the larger of two estimates:
- the seed-to-seed standard error;
- the propagated per-run sd.

The record's sd is added in quadrature.

## Results (2026-11-19/20)

**Hardware.** Intel Xeon @ 2.80 GHz (KVM), 4 logical cores shared with two
other jobs, 15 GiB RAM, Linux 6.18, CPU only.

**Timings.** Data processing took about 1470 s and transport about 12 500 s per
seed run, each run getting about half a core. Wall times are indicative only.

**T: 1 thread against 2 threads** (`logs/T1.out`, `logs/T2.out`):

| Threads | k_eff | Entropy trace |
|---|---|---|
| 1 | 0.976148 ± 0.014659 | 5.342 5.330 5.391 |
| 2 | 0.976148 ± 0.014659 | 5.342 5.330 5.391 |

**Bit-identical: the prediction held.** This shows thread-count independence of
`run_keff_csg_hybrid` between 1 and 2 threads on this model. 16 threads (the
record's count) was not tested, because this machine has 4 cores.

**S: four seeds** (`logs/S*.out`). Every run had 0 lost locates and 0 stuck
events.

| Seed | k_eff | Minus record (pcm) |
|---|---|---|
| 20260917 (H1 default arm) | 0.991090 ± 0.001686 | −422 ± 201 |
| 20260918 | 0.997066 ± 0.001751 | +176 ± 207 |
| 20260919 | 0.991760 ± 0.001848 | −355 ± 215 |
| 20260920 | 0.992162 ± 0.001635 | −315 ± 197 |

| Set | Mean k | Seed-to-seed sd | Uncertainty on mean | Minus record | z |
|---|---|---|---|---|---|
| **All four (quote this)** | **0.993020** | 273 pcm | 137 pcm (SEM) | **−229 ± 175 pcm** | **−1.31** |
| Three new seeds only | 0.993663 | 295 pcm | 171 pcm (SEM) | −165 ± 203 pcm | −0.81 |

**Predictions against outcomes.**
- The prediction was −250 .. +150 pcm, with a central value of about −150.
  **Outcome −229: inside the range, so it held.**
- The refutation condition was not met. The 4-seed mean is not below −280 pcm,
  and the three new seeds alone are not more than 2σ below the record.

## Interpretation

- **No code drift is resolved.** Today's code is consistent with the 2026-11-05
  record at about 1.3σ. Drift smaller than about 350 pcm (2σ) cannot be excluded
  at this size.
- **The −422 pcm was one seed.** H1's seed 20260917 is the lowest of the four.
  A single 4000-history run of this core scatters by about 270 pcm from seed to
  seed. That scatter is **larger than the 165–185 pcm sd each run reports
  internally**, so a single-seed sd understates the uncertainty here.
  - This matches the example's own doc, which says single-seed residuals are
    not quotable to better than a few hundred pcm.
- **Quote the 4-seed mean**, not any single seed. The record 0.995309 stays
  quotable as the 2026-11-05 value, and nothing here contradicts it.

## Not done

- **16 threads**, and the record's own 10000 × [5+135] protocol.
- **A bisect:** not needed, since no drift is resolved.
- **Commits after `516f559f8`.** They landed on develop after this binary was
  built, and include gh:#692's uniform fission-site weighting in the CSG
  drivers. None of these runs covers them. If #692 changes the default CSG
  physics, HTR-10 needs re-measuring on the commit that carries it.
