# #495 — autocorrelation-corrected σ: V&V protocol (stub)

**Issue:** [gh:#495](https://github.com/theodoreOnzGit/outram-park-backend/issues/495), epic #493.
**Code:** `src/stats/correlated_sigma.rs` (`KeffUncertainty`,
`TallyBatchRecorder`); `raffles::estimators::autocorrelation` (batch means,
`τ_int`, ESS).

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## What is being checked

Whether the corrected `σ` of `k_eff` (batch means over the active
generations, default batch `⌊√n⌋`; and `s/√ESS` from Sokal's `τ_int`) predicts
the **seed-to-seed** scatter that #494's ensemble measures, where the naive
`k_std` is expected to under-predict it.

## Methodology

- Case: HTR-10 (`nee_soon` `htr10_rmc_keff`), 10 000 histories ×
  [5 inactive + 135 active] — the loosely coupled case #495 names. Entropy
  mesh as in that example.
- `N` independent seeds through `stats::ensemble::run_seeds`; for each seed,
  `KeffUncertainty::from_result(&r, 5)`.
- Compare `sd_seeds` (seed-to-seed sample sd of `k_mean`) with the mean over
  seeds of (a) the naive `k_std`, (b) the batch-means `σ`, (c) `σ_ESS`.

## Pass criterion (fixed in advance, 2026-10-03)

For estimator (b): `sd_seeds / mean(σ_batch)` within `1 ± 2u`,
`u² = 1/(2(N − 1)) + 1/(2(a − 1))/N`, `a` = number of batches (at 135 active
generations: batch `⌊√135⌋ = 11`, so `a = 12` with the 3 leading active
generations dropped). The naive `σ` is **expected to fail** the same test on the low
side; that is recorded as the finding, whichever way it comes out.

**Known limit stated before measuring:** with 135 active generations a
correlation length comparable to the run cannot be seen; a `τ_int` with
`window_converged = false` is a lower bound and is reported as such.

## Tallies

No driver exposes per-batch tally realisations yet, so the tally half of #495
cannot be measured on the shipped drivers. `TallyBatchRecorder` is ready for a
caller-stepped loop; a report-only per-batch hook in the drivers is a
follow-up issue.

## Results

| case | date | N seeds | sd_seeds (pcm) | mean naive σ | mean batch σ | mean σ_ESS | ratio (b) | pass |
|---|---|---|---|---|---|---|---|---|
| HTR-10 10k × [5+135] | — | — | NOT YET MEASURED | | | | | |
