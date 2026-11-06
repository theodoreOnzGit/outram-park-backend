# HTR-10 and HTTR records re-measured under LEAPR-by-default thermal laws (gh:#1145)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One seed per arm. Research, education and
> V&V only; not for any operational use. AI-assisted runs (Claude Opus 5.5 in
> Claude Code), not yet reviewed by the maintainer.

Predictions, design and reasoning: [`PREDICTION.md`](PREDICTION.md), committed
in `7fd23529e` before the first run. Each case runs as two arms on one commit,
seed and statistics: **LEAPR** (the default since gh:#1137) and **TAPE**
(`OUTRAM_HTR10_TAPE_LAWS=1`, the published ENDF/B-VIII.0 tapes as every record
before 2026-11-03 used).

## Run parameters

| Item | Value |
|---|---|
| Commit | `7fd23529e` (`logs/commit.txt`) |
| Build | `cargo build --release -p nee_soon --example htr10_rmc_keff --example httr_keff` |
| Hardware | Intel Xeon W-2195 @ 2.30 GHz, 18 cores / 36 threads, 62.5 GiB RAM, Linux 5.15.0-190-generic, CPU only; two arms at a time, each pinned (`taskset`) to its own 16 or 17 logical CPUs |
| HTR-10 | `examples/htr10_rmc_keff.rs`, `OUTRAM_HTR10_HISTORIES=10000 _INACTIVE=5 _ACTIVE=135 _RINGS=14 _LAYERS=12 _THREADS=16`, seed default 20260917: the 2026-10-07 10k record's statistics and seed |
| HTTR | `examples/httr_keff.rs`, `OUTRAM_HTTR_HISTORIES=20000 _INACTIVE=100 _ACTIVE=300 _SEED=20261016 _THREADS=17`: the start-up core record's |

## Results

### HTR-10, N = 12 (the critical loading), ENDF/B-VIII.0 (2026-11-05)

| Arm | k | lost / stuck | data / transport |
|---|---|---|---|
| TAPE | 0.994504 +- 0.001007 | 0 / 0 | 420 s / 2948 s |
| **LEAPR (default)** | **0.995309 +- 0.001098** | 0 / 0 | 439 s / 2945 s |
| **LEAPR - TAPE** | **+80 +- 149 pcm** (uncorrelated sigma) | | |

The LEAPR arm generated `c_Graphite`, `Si_SiC` and `c_SiC` by LEAPR at the
run's temperature; the TAPE arm read `tsl-reactor-graphite-30P.endf` (MAT 32)
(logs).

**Against the predictions:**

| Quantity | Predicted | Measured | |
|---|---|---|---|
| `LEAPR - TAPE` | -40 pcm, range -100 to +20 | **+80 +- 149 pcm** | **unresolved**: the central value is above the range, but the measurement cannot resolve tens of pcm; consistent with the prediction and with zero |
| TAPE against the 2026-10-07 record (0.995125 +- 0.001055) | within 2 sigma | **-62 +- 146 pcm** | inside: the code changes since 2026-10-07 did not measurably move this point |

**Why the difference is not tighter.** The two arms share a seed, but their
histories diverge at the first thermal collision, where the laws differ, so
correlated sampling buys little: the uncorrelated sigma is close to the real
one. Resolving a 40 pcm law effect at 2 sigma would need sigma of about 20 pcm
on the difference, ~50x these histories; not affordable here. The finding is
therefore a bound: |LEAPR - TAPE| below ~380 pcm at 2 sigma at N = 12.

**New record value (the default model), N = 12: k = 0.995309 +- 0.001098**,
+18 +- 152 pcm from the 2026-10-07 record's 0.995125 +- 0.001055.

### HTTR critical (2026-11-05), partial

| Arm | k | data / transport |
|---|---|---|
| **LEAPR (default)** | **1.02387 +- 0.00049** | 476 s / 3725 s, 34 threads alone |
| TAPE | **not measured**: stopped by the maintainer before it finished, to test a refactor on develop first | |

Against the start-up core record's tape-law value 1.02400 +- 0.00049 (a
different commit and machine): **-13 +- 69 pcm**, consistent with the -40 pcm
(-100 to +20) predicted for the law change, but not a paired measurement: the
paired TAPE arm is still to run. Two earlier attempts at both arms side by side
on 17 threads each were stopped by the 2 h background limit before transport
finished (`logs/*_killed_2h.log`).

**Still to run (gh:#1145):** the HTTR critical TAPE arm and the subcritical
pair; the DEM-bed point; the rod curves; the other HTR-10 heights; then the web
demo's reference lines.
