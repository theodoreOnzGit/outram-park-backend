# #494 — seed-pooled ensembles: V&V protocol (stub)

**Issue:** [gh:#494](https://github.com/theodoreOnzGit/outram-park-backend/issues/494), epic #493.
**Code:** `src/stats/ensemble.rs` (runner, `EnsembleReport`);
`raffles::estimators::seed_consistency` (the statistics).
**First consumer:** `examples/godiva_keff_ensemble.rs`.

> **Status: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
> Nothing below has been run. No number on this page is a result.

## What is being checked

1. **The internal `σ` is honest.** For `N` independent seeds, each with its own
   `k_std`, compute `χ² = Σ (k_i − k̄_w)² / σ_i²` against the
   inverse-variance mean. Under honest `σ`, `χ² ~ chi-square(N − 1)`.
2. **Pooling shrinks as `1/√N`.** Measured, not assumed: split the seeds in
   order into disjoint groups of `m = 4`, and compare the sample sd of the
   group means with `sd/√m`. Ratio `1` within `1/sqrt(2(n_groups − 1))`.
3. **Migration did not move anything.** `godiva_keff_ensemble` prints the same
   per-seed values and the same mean/sd/sem as before the migration.

## Pass criteria (fixed in advance, 2026-10-03)

| check | criterion | where it comes from |
|---|---|---|
| seed consistency | `χ²/dof` inside the exact two-sided 95 % chi-square band for `dof = N − 1` (for `N = 32`, `dof = 31`: `[17.539, 48.232]/31 = [0.566, 1.556]` from the published chi-square quantiles — the unit test `band_matches_published_quantiles` checks the code against them) | #494 |
| `1/√N` | `abs(ratio − 1) ≤ 2 · ratio_sigma` | #494 |
| outliers | reported, **never dropped**; Bonferroni two-sided 5 % family-wise | this module |
| no behaviour change | per-seed `pcm` vector identical to a pre-migration run at the same commit's physics | this change |

**Expected failure mode, stated before measuring:** in power iteration the
per-run `k_std` ignores cycle-to-cycle correlation, so it is expected to
**under**-state the scatter, giving `χ²/dof` above the band
(`OverDispersed`). If so, that is the finding handed to #495, not a defect in
this check, and the band is **not** widened.

## Cases

| case | command | seeds |
|---|---|---|
| Godiva (ICSBEP HEU-MET-FAST-001) | `cargo run --release -p outram-mc-libs --features endf-pebble-cases --example godiva_keff_ensemble` | 32 (default) |
| one TRISO case | to be chosen by the maintainer (#494 asks for one) — not yet wired | 32 |

## Results

| case | date | N | mean (pcm) | sd | sem | `χ²/dof` | verdict | group ratio | outliers |
|---|---|---|---|---|---|---|---|---|---|
| Godiva | — | — | NOT YET MEASURED | | | | | | |
| TRISO | — | — | NOT YET MEASURED | | | | | | |

Hardware must be recorded with any timing (crate `CLAUDE.md`, "Every timing
carries its hardware").
