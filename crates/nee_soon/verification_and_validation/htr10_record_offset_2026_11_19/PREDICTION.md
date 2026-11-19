# HTR-10 whole core: is the 2026-11-05 record reproduced on today's code? (predictions)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft, unreviewed.
> **This file is committed before any run below.** A prediction that misses stays
> here with its outcome beside it.

## The question

The gh:#550 third re-measure (part 2, `outram-mc-libs/verification_and_validation/tutorial_rung4/sab_reference_550.md`,
row H1) ran `examples/htr10_rmc_keff.rs` as two arms on commit `6b3b64807`:
4000 x [5+90], seed 20260917, 14 rings x 12 layers, `OUTRAM_HTR10_THREADS=1`.

- Default arm: 0.991090 +- 0.001686.
- Knob-on arm (`OUTRAM_SAB_LOG_SPACED_GRID=1`): 0.991743 +- 0.001673.

Both sit below the 2026-11-05 record, which is 0.995309 +- 0.001098 at 10000 x
[5+135], seed 20260917, 16 threads, commit `7fd23529e` (`leapr_default_remeasure_2026_11_05/`).
The default arm is −422 +- 201 pcm below it (z −2.1).

The two arms share a seed and differ only in thermal emission tables, so they
are strongly correlated. Together they are about **one** independent sample,
not two.

## What has changed since the record, and what that means for a bit-level test

`git log 7fd23529e..516f559f8` touches the transport library 337 times. That
includes the OpenMC kernel and seeding work (gh:#1386, #1444: reaction order,
fission-neutron order, per-collision fission banking, `SeedingMode::OpenMc`),
which **changes the random stream** on the CSG path by design.

A bit-level comparison of the old and new commits at equal settings can
therefore only show "different". It cannot separate a physics change from a new
random stream. So the decisive test is **statistical**: draw more independent
seeds on today's code and ask whether their mean is consistent with the record.

The same holds for H1's own protocol. 4000 histories against 10000 is a
different stream from the record even at the same seed, so H1 against the
record is already a comparison of two independent draws.

## Runs (fixed now, before any is made)

All runs use the current `develop` (`516f559f8`), the default model, 14 rings x
12 layers, and `--release`.

**T: thread-count independence of the hybrid CSG driver.** The example's own doc
says this is *assumed, not tested* (`OUTRAM_HTR10_THREADS`). H1 used 1 thread and
the record used 16, so if the driver is thread-count dependent, H1 and the record
are not the same experiment.

- Settings: 500 x [1+2], seed 20260917, `OUTRAM_HTR10_THREADS=1` and then `=2`.
- **Prediction:** k bit-identical between 1 and 2 threads, because per-particle
  streams are seeded from the history index in OpenMC seeding mode.
- **Refuted if** any printed digit of k or its sd differs.
- A refutation is a finding in its own right: every single-seed HTR-10 number in
  the record then depends on the thread count.

**S: seed ensemble at H1's protocol.** 4000 x [5+90], `OUTRAM_HTR10_THREADS=1`,
seeds 20260918, 20260919 and 20260920 (`OUTRAM_HTR10_SEED`), one process each.
H1's default arm is the fourth sample (seed 20260917).

- Expected per-run sd: about 170 pcm, as in H1.
- The HTR-10 seed-to-seed scatter recorded in the example's doc (sd 179 pcm at
  8 seeds) is of the same size.
- Expected sigma of the 4-seed mean: about 90 pcm.
- Combined with the record's 110: about 140 pcm on the difference.

**Prediction:** the 4-seed mean is consistent with the record. The difference
lies within −250 .. +150 pcm and the central value is about −150. That central
value is not zero because H1's −422 is one quarter of the mean, and the three
new seeds are expected to centre on the record.

**Refuted (code drift resolved) if:**
- the 4-seed mean is more than 2 sigma below the record (below about −280 pcm), **and**
- the three new seeds alone also sit below the record by more than 2 of their own
  sigma.

If that is refuted, the next step is a bisect of `7fd23529e..516f559f8` with a
multi-seed test at each step. A single-seed bisect cannot resolve a physics
shift (the gh:#1128 bisect note, commit `0227638bb`, says the same).

## Numbers to quote

The 4-seed mean against the record, with both sigmas. H1's single-pair −422 is
not to be quoted alone once S exists.
