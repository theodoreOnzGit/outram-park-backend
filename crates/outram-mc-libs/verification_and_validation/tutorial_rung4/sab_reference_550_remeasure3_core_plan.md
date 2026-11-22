# gh:#550 third re-measure: HTR-10 whole core and MSRE N-1, paired knob-on / knob-off (2026-11-19)

Research, education and V&V only. **Committed BEFORE any paired run.** A
prediction that misses stays in place; outcomes go in
`sab_reference_550.md` ("Third re-measure"). Base commit `6b3b64807`. The named
ablation `OUTRAM_SAB_LOG_SPACED_GRID=1` ("knob on") reproduces the pre-flip
log-spaced build; unset ("knob off") is the new default, THERMR's calcem node
grid. Sign convention: **difference = default (node grid) minus knob-on
(log-spaced)**.

Why: the second re-measure (`sab_reference_550.md`) left two items open:
`nee_soon::htr10_rmc` (record protocol about 6.5 h on 2 threads) and MSRE N-1
(2 seeds, no knob-on control, so -183 +- 73 pcm against the 2026-11-13 record
was not a measurement of the flip).

## Timing probe (made before this plan; its k is not used)

One probe of `htr10_rmc_keff` (1000 x [2+3], 2 threads, knob off) to size the
protocol: nuclear data 432 s (the same on 2 threads as on 16: the load is mostly
serial), transport 44.8 s for 5000 histories, i.e. 9 ms of wall time per history
on 2 threads, about 18 ms per history per thread. Its k (generations 1 to 4:
1.050, 1.020, 1.014, 0.956 at 1000 histories) is noise and is not a result.

## Item 1: HTR-10 whole core (`htr10_rmc_keff`, the example behind `nee_soon::htr10_rmc`)

Reached by the flip: yes. `ThermalLawSource::Leapr` is the default;
`data_jobs.rs` builds graphite, C-in-SiC and Si-in-SiC through
`ThermalScattering::from_leapr` -> `from_tape`, which the knob reaches (U-in-UO2
and O-in-UO2 also come from `from_leapr`). The structural test
`htr10_correct_physics_is_default` builds no data (it asserts the config), so it
cannot move; it is run as a test, not as a measurement.

Record for reference: 0.995309 +- 0.001098 (N=12, 10000 x [5+135], 16 threads,
seed 20260917, 2026-11-05,
`nee_soon/verification_and_validation/leapr_default_remeasure_2026_11_05/`). It
is not the control: the code has moved since. The control is the knob-on arm.

**Protocol (paired, same commit, same seed 20260917, same statistics):**
`OUTRAM_HTR10_HISTORIES=4000 _INACTIVE=5 _ACTIVE=90 _RINGS=14 _LAYERS=12
_THREADS=1`, ENDF/B-VIII.0, default model. Two processes at once, one thread
each, one with `OUTRAM_SAB_LOG_SPACED_GRID=1`. Budget: 432 s of data plus 380000
histories x 18 ms, about 2.0 h wall. This is not the record's 10000 x 135 (6.5 h
on 2 threads): 360000 active histories is 0.27 of the record's 1.35 million.

**Expected sigma, stated before running:** one arm's sd scales as the square
root of the active histories: 1.098e-3 x sqrt(1.35e6 / 3.6e5) = **213 pcm per
arm, 300 pcm on the difference** (the arms share a seed but diverge at the first
thermal collision, so no correlation is assumed; the 2026-11-05 pair did not
profit from it). Generation-to-generation correlation could make the real sigma
larger; this is the optimistic figure.

**Prediction:** difference **-10 pcm (range -80 .. +60)**, the second plan's
basis (the node grid's graphite stationary distribution is +2.1 K hotter, so
small and negative). At 300 pcm sigma this is **unresolved by construction**;
the result will be reported as a bound. **Refutation:** a difference outside
+-600 pcm (2 sigma) would contradict a tens-of-pcm effect and would be a finding
in its own right; a difference inside +-600 pcm neither confirms nor refutes the
prediction, it only bounds |flip| below about 600 pcm at this size. Each arm
should read 0.995 +- 0.004 (the record, plus 2 sigma of 213 pcm and a drift
allowance).

## Item 2: MSRE N-1 (`msre_n1_first_criticality`)

Command: `--mo natural --seeds 1-2 --threads 2` (20000 x [100+200]; the default
model: natural Mo, hot dimensions, whole TM-0728 shield, U-234/U-236), the
second re-measure's. **Knob-off arm = the second re-measure's seeds 1 and 2**
(`remeasure2_550/msre_new.csv`: 1.016934 +- 0.000488, 1.015550 +- 0.000573),
built from the committed knob commit `66efb2945`. Between it and base
`6b3b64807` no file under `crates/*/src` changed (`git diff --stat`: docs, V&V
logs, and a 4-line `[[example]]` entry in `outram-mc-libs/Cargo.toml` for an
unrelated example). Runs are deterministic for a fixed seed and thread count (2
here), so these are reused rather than re-run (a re-run costs 1.9 h of the
budget). **This is an assumption, stated as such.** **Knob-on arm: seeds 1 and
2, run now, same command, `OUTRAM_SAB_LOG_SPACED_GRID=1`**, about 1.8 h on 2
threads. Seed 3 (both arms) only if time remains, which I do not expect. Wall
time of the second re-measure's seeds: 3340 s and 2852 s (load 447 s).

**Expected sigma:** 49 and 57 pcm per seed; the arms' streams do not correlate,
so each seed difference is about 75 pcm and the mean of two is **about 53 pcm**.

**Prediction (two statements, both made now):**
1. The flip's own effect on N-1: **-10 pcm (-80 .. +60)**, the second plan's W1
   (graphite at 911 K and H2O in the shield: small, mixed signs).
2. The earlier -183 +- 73 pcm **is not the flip**. The knob-on arm should read
   about **1.0162 +- 0.0010** (seeds 1, 2 mean), as low as the default.
   **Refutation:** if the knob-on mean is 1.0180 or above (within 100 pcm of the
   2026-11-13 record 1.01807), the flip carries the -183 and the prediction is
   wrong. If the difference (default minus knob-on) lies within +-106 pcm
   (2 sigma), the flip is unresolved from zero at this size.

## Order and rules

HTR-10 arms first (two processes, one thread each, about 2 h), then MSRE knob-on
(2 threads, about 1.8 h). No input, tolerance or threshold is touched; a k pin
is re-set only for a resolved move caused by the flip. Anything not run is
reported as such.
