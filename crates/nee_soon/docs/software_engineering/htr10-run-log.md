# HTR-10 run log — machine, settings, timing and statistics

> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are
> intended for journal / arXiv publication — that is the trust workflow. See
> the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not
> for nuclear facility operation, reactor control, safety-critical, or
> licensing decisions.

**One file for every HTR-10 production run.** Append a new section per run;
never rewrite a recorded one. Where a later run supersedes an earlier result,
strike the old line through and say what replaced it — the history is what
makes a correction trustworthy.

The *physics* record (ablation chains, what each term is worth, what the
residual is attributed to) lives in
`crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`. This
file is the **engineering** record: what was run, on what hardware, with what
settings, how long it took, and what came out.

---

## Run 1 — ENDF/B-VIII.0 twelve-height loading sweep, bound SiC and UO2 laws

**Date:** 2026-09-24. **Code:** `ab7f8657a`
(`feat(nee_soon): HTR-10 ENDF/B-VIII.0 loading sweep, one function per case`).
**Driver:** `cargo run --release -p nee_soon --example htr10_endf8_height_sweep -- <case>`.

### Why this run exists

The twelve-height curve already published in
`publications/.../htr10_rmc_curve/` was computed **before** `77f8ecb14c`
applied the SiC (C-in-SiC, Si-in-SiC) and UO2 (U-in-UO2, O-in-UO2) bound
thermal scattering laws and natural Si isotopics. In that data the SiC coating
carbon and the UO2 kernel scatter as a **free gas** and SiC silicon is bare
Si-28. That commit's own message recorded the open question:

> Whether the VII.0 residual slope survives the corrected treatment is not yet
> measured.

This run measures it on ENDF/B-VIII.0.

### Relationship to `docs/htr10-rmc-verification-suite.md`

**That document got there first and is the authority on the physics.** Written
the same day from `77f8ecb14c`, its §2 is a twelve-height curve at *these exact
settings* — 10 000 x [5 + 135], single seed 20260917, the same five thermal
laws, ENDF/B-VIII.0, fixed cavity — and its §7 is the constant-cavity ablation.
It already answers the question above: the slope survives, at
`+7.18 +/- 0.85 pcm/cm`, 8.4 sigma.

This run was launched before that document existed. It is kept because it is an
**independent execution of the same configuration on different hardware**, and
because of what that comparison turned up.

---

## UNRESOLVED — this run and the suite's §2 disagree, and they should not

Two runs of a **nominally identical configuration** differ by a weighted mean
of **`+122 +/- 42 pcm`, 2.9 sigma**. Same settings, same seed, same five
thermal laws, same library, same cavity treatment, same reference.

| *n*ax | suite §2 k_eff | this run k_eff | shift [pcm] | sigmas |
|---|---|---|---|---|
| 20 | 0.900313 | 0.902372 | **+206** | +1.46 |
| 21 | 0.920068 | 0.922175 | **+211** | +1.45 |
| 23 | 0.954487 | 0.956690 | **+220** | +1.55 |
| 25 | 0.989263 | 0.989744 | +48 | +0.33 |
| 27 | 1.017606 | 1.019945 | **+234** | +1.66 |
| 29 | 1.045138 | 1.044929 | −21 | −0.15 |
| 31 | 1.065898 | 1.067929 | **+203** | +1.39 |
| 33 | 1.088731 | 1.088578 | −15 | −0.10 |
| 35 | 1.105463 | 1.107711 | **+225** | +1.49 |
| 37 | 1.125712 | 1.125028 | −68 | −0.51 |
| 39 | 1.138939 | 1.139777 | +84 | +0.57 |
| 41 | 1.155702 | 1.157193 | +149 | +0.97 |

**A fixed seed with fixed settings is supposed to be deterministic.** The V&V
record states `k_eff` was returned *identically to six decimal places* across 1
thread, 16 threads, P-cores and E-cores. If that holds, these two curves should
be equal, not scattered — so one of the premises is wrong.

**The shifts are not a constant offset and not obviously random.** They cluster
into two groups: six points near `+210 pcm` (20, 21, 23, 27, 31, 35) and five
near zero (25, 29, 33, 37, 39). A bimodal split is not what re-randomisation
looks like.

**Candidate causes, none eliminated:**

1. **Different driver.** The suite ran `htr10_rmc_keff` with environment knobs;
   this ran `htr10_endf8_height_sweep`, whose settings are literals. The two are
   *intended* to build the same model. If they do not, this file and that one
   are describing different models and the new example is wrong — check the
   nuclide slot order, the majorant grid, the source box and the entropy mesh
   against `htr10_rmc_keff` before trusting either curve.
2. **Thread count.** The suite used `ThreadCount::Auto` on 32 logical CPUs;
   every case here is `ThreadCount::Fixed(1)`. The recorded determinism check
   covered 1 vs 16 threads on *one* configuration at 2000 x [30 + 70]; it did
   not cover 32 threads, nor these settings.
3. **Code drift.** `77f8ecb14c` to `ab7f8657a` spans the `develop` merges,
   including transport-touching work in `outram-mc-libs`.

**Do not quote either curve as the model's answer until this is closed.** The
discrepancy is larger than the thermal-law effect this run set out to measure,
so it is the first thing to resolve. The cheapest discriminator is to re-run one
height both ways on one machine: same commit, `htr10_rmc_keff` against
`htr10_endf8_height_sweep`, 1 thread both times. If they agree, the cause is
thread count or code drift; if they differ, it is the driver.


### Machine

Single-socket workstation, **not** the machine the `htr10_affinity_timing`
table in the manuscript was taken on (that one reports 32 logical CPUs with a
P-core/E-core split; this one has neither). Absolute timings here are therefore
**not comparable** with that table — only the per-height *ratios* are.

| | |
|---|---|
| CPU | Intel Xeon W-2195 @ 2.30 GHz (max 4.30 GHz) |
| topology | 1 socket, 18 physical cores, 2 threads/core, 36 logical |
| SMT siblings | CPU *n* and CPU *n+18* share a physical core |
| L3 | 24.8 MiB, single instance |
| governor | `powersave` (not pinned — see caveats) |
| memory | 62 GiB, 44 GiB available at launch |
| OS / kernel | Linux Mint 21.3, 5.15.0-190-generic |
| toolchain | rustc 1.98.1 (48a229cea), cargo 1.98.1 |

**Affinity.** Each case was pinned to **one physical core** with
`taskset -c N`, cases on CPUs 0–11. Those are twelve distinct physical cores,
so no two cases shared one via SMT.

### Settings — hard-coded, not environment-driven

Every setting is a literal in the case function; nothing is read from the
environment. This is the point of the example (see its module doc): the
committed source *is* the specification of the run.

| quantity | value |
|---|---|
| library | ENDF/B-VIII.0 |
| graphite S(a,b) | crystalline, MAT 30 |
| SiC S(a,b) | C-in-SiC MAT 44, Si-in-SiC MAT 43 |
| UO2 S(a,b) | U-in-UO2 MAT 48, O-in-UO2 MAT 75, generated in-process from LEAPR decks |
| silicon | natural Si-28 / Si-29 / Si-30, **all three bound to Si-in-SiC** |
| boron | natural, Table 2 impurity rows |
| reflector | zone 22, carbon scale 1.000 |
| cavity | fixed core cavity 221.818 cm; void = cavity − bed |
| rings × layers | 14 × *n*, *n* = 20…41 |
| particles | 10 000 per generation |
| generations | 5 inactive + 135 active |
| histories per case | 1 400 000 |
| seed | 20260917, **single draw** |
| threads | 1 (`ThreadCount::Fixed(1)`) |

**The cavity treatment changed in this run's code.** `082cd413d` deleted the
constant-void option and the `OUTRAM_HTR10_FIXED_CAVITY` opt-in; the fixed core
cavity is now unconditional. Results recorded before that commit **without**
the flag were computed with the constant void and are not comparable here.

### Results — k_eff and the residual

Compared against RMC (Li, Yu & Wei 2014) **interpolated to the height actually
modelled**, not the 123.576 cm headline. Uncertainties are the within-run Monte
Carlo sigma.

| case | *n*ax | height [cm] | k_eff | sigma [pcm] | RMC(interp) | dk [pcm] |
|---|---|---|---|---|---|---|
| n20 | 20 |  97.980 | 0.902372 ± 0.001051 | 105.1 | 0.911138 |  −877 |
| n21 | 21 | 102.879 | 0.922175 ± 0.001014 | 101.4 | 0.932352 | −1018 |
| n23 | 23 | 112.677 | 0.956690 ± 0.001037 | 103.7 | 0.968224 | −1153 |
| n25 | 25 | 122.475 | 0.989744 ± 0.000893 |  89.3 | 1.000676 | −1093 |
| n27 | 27 | 132.272 | 1.019945 ± 0.001006 | 100.6 | 1.027764 |  −782 |
| n29 | 29 | 142.070 | 1.044929 ± 0.000974 |  97.4 | 1.053928 |  −900 |
| n31 | 31 | 151.868 | 1.067929 ± 0.000983 |  98.3 | 1.076021 |  −809 |
| n33 | 33 | 161.666 | 1.088578 ± 0.001063 | 106.3 | 1.095242 |  −667 |
| n35 | 35 | 171.464 | 1.107711 ± 0.001006 | 100.6 | 1.112818 |  −511 |
| n37 | 37 | 181.262 | 1.125028 ± 0.000939 |  93.9 | 1.131730 |  −670 |
| n39 | 39 | 191.060 | 1.139777 ± 0.001080 | 108.0 | 1.146030 |  −625 |
| n41 | 41 | 200.858 | 1.157193 ± 0.001090 | 109.0 | 1.160581 |  −339 |

**Every point is low**, mean `−787 pcm`, range `−1153` to `−339`.

### The residual is not constant — the slope survives the correction

Weighted fits over all twelve points:

| fit | result | chi-square |
|---|---|---|
| constant | `−799 ± 29 pcm` | **60.9 on 11 dof — rejected** |
| linear | slope **`+6.01 ± 0.90 pcm/cm`** | 16.1 on 10 dof |

`dchi2 = 44.8` on one degree of freedom, i.e. **6.7 sigma** for the slope.

**This CONFIRMS what the verification suite measured independently**, rather
than discovering it. Four measurements now agree:

| measurement | library | thermal laws | slope [pcm/cm] |
|---|---|---|---|
| earlier curve | VII.0 | free gas | `+6.71 ± 1.06` |
| earlier curve | VIII.0 | free gas | `+5.74 ± 2.41` |
| suite §2 | VIII.0 | bound | `+7.18 ± 0.85` |
| **this run** | VIII.0 | bound | **`+6.01 ± 0.90`** |

Note the two bound-law measurements are of the *same configuration* and differ
by 1.0 sigma in slope — consistent, but see the unresolved discrepancy above
before treating them as independent confirmation of each other.

So:

- the slope is **not** an artefact of the free-gas thermal treatment;
- the slope is **not** specific to one evaluation.

That leaves the geometric reading — the axial reflector / void treatment at
high loading, where the void closes to 21 cm at 200.86 cm and has never been
tested in that regime — as the surviving hypothesis. It is not confirmed here.

### What the bound thermal laws were worth — NOT RESOLVED

Against the pre-correction ENDF/B-VIII.0 curve at the same twelve heights, the
mean shift is **`−81 pcm`**. The per-point shifts run `−482` to `+484` pcm,
which is what two independent single draws should scatter by: the old points
carry a within-run sigma of 270–350 pcm and the seed-to-seed scatter on this
problem is `sd ~ 179–211 pcm`. Propagated, the error on the mean shift is
roughly `±90 pcm`.

**So the correction is consistent with zero on this curve, and bounded below
about 180 pcm at 2 sigma.** It is not the explanation for the residual. Do not
quote the per-point shifts individually — they are noise.

This does **not** say the laws are unimportant in general; it says they are
small *here*, against *this* reference, at *this* precision.

### Source convergence — 5 inactive generations

The entropy trace is the only diagnostic that says whether so thin an inactive
stage converged the fission source. A still-rising trace would mean every
active generation after it is biased.

| case | first [bits] | last [bits] | drift |
|---|---|---|---|
| n20 | 5.4735 | 5.4713 | −0.0022 |
| n21 | 5.4725 | 5.4561 | −0.0164 |
| n23 | 5.4739 | 5.4873 | +0.0134 |
| n25 | 5.6159 | 5.6034 | −0.0124 |
| n27 | 5.6622 | 5.6366 | −0.0256 |
| n29 | 5.7051 | 5.6787 | −0.0265 |
| n31 | 5.7244 | 5.7223 | −0.0021 |
| n33 | 5.7508 | 5.7475 | −0.0033 |
| n35 | 5.7706 | 5.7433 | −0.0272 |
| n37 | 5.7638 | 5.7727 | +0.0090 |
| n39 | 5.7803 | 5.7861 | +0.0058 |
| n41 | 5.8066 | 5.7994 | −0.0073 |

All drifts are within `±0.028` bits against a 6.0000-bit ceiling, in both
signs, with no trend against height. **No evidence of an unconverged source at
any loading**, and in particular the drift does not grow in the tall cores
where the residual slope lives — so the slope cannot be blamed on source
convergence. This is the same conclusion the twelve-height VII.0 study reached
by a different route.

### Timing and transport diagnostics

| case | tiles | data [s] | transport [s] | ms/history | coll/history | leak [%] |
|---|---|---|---|---|---|---|
| n20 | 29 412 | 482.9 | 30 794.3 | 22.00 | 377.10 | 0.518 |
| n21 | 30 229 | 481.8 | 30 979.2 | 22.13 | 371.26 | 0.514 |
| n23 | 31 863 | 482.1 | 34 040.2 | 24.31 | 361.42 | 0.507 |
| n25 | 33 497 | 482.3 | 32 664.1 | 23.33 | 352.56 | 0.512 |
| n27 | 35 131 | 483.3 | 33 280.6 | 23.77 | 344.66 | 0.524 |
| n29 | 36 765 | 483.4 | 34 925.4 | 24.95 | 337.73 | 0.522 |
| n31 | 38 399 | 480.6 | 35 155.3 | 25.11 | 331.29 | 0.535 |
| n33 | 40 033 | 480.3 | 34 640.2 | 24.74 | 325.29 | 0.564 |
| n35 | 41 667 | 484.3 | 35 204.3 | 25.15 | 319.92 | 0.579 |
| n37 | 43 301 | 481.7 | 35 372.6 | 25.27 | 314.97 | 0.623 |
| n39 | 44 935 | 479.5 | 35 763.2 | 25.55 | 310.65 | 0.676 |
| n41 | 46 569 | 486.0 | 36 450.7 | 26.04 | 306.36 | 0.749 |

**Totals.** 16 800 000 histories; **113.7 core-hours** of transport plus
**1.61 core-hours** of data processing, **115.3 core-hours** in all. Twelve
cases in parallel on twelve cores: launched 08:46:09, last case finished about
19:01, **wall clock 10.26 h**, set by n41.

**Health.** `lost locate = 0`, `stuck events = 0`, `negative distances = 0` on
every one of the twelve. Planned and transported history counts agree exactly.

Three things the table shows:

- **Data processing is flat at ~482 s** (479.5–486.0, a 1.4 % spread) and does
  not depend on bed height — the same nuclides and the same five thermal laws
  are built regardless. Of that, the two UO2 LEAPR generations are ~25 s.
  Single-core and contended, this is ~2.9x the ~165 s the other machine's
  affinity table reports for one core; data prep barely parallelises and is
  allocation-heavy, so it suffers most under twelve-way contention.
- **Collisions per history FALL with height**, 377 to 306. A short bed leaks a
  larger fraction of its neutrons into the graphite reflector, where they
  scatter many times before returning or being absorbed; a tall core absorbs
  them sooner. Transport time nevertheless *rises*, because the per-flight
  geometry work grows with the tile count.
- **Vacuum leakage rises with height**, 0.518 % to 0.749 %, as the void above
  the bed closes and the bed approaches the axial boundary. This is the same
  region the residual slope lives in and is worth following up.

### Caveats — read before quoting anything above

1. **Single seed, every point.** Seed-to-seed scatter is `sd ~ 179–211 pcm`,
   which the within-run sigma does **not** contain. One draw of a case
   re-randomises by roughly twice its quoted sigma. The fitted slope is robust
   to this (it is a twelve-point trend, and 6.7 sigma), but an individual
   residual is not quotable to better than a few hundred pcm.
2. **The machine was not idle.** Builds, the `nee_soon` test suite and a timing
   calibration ran on other cores during the sweep. `k_eff` is unaffected —
   single-threaded with a fixed seed, so every case is deterministic — but the
   **timings carry contention** and are working-desktop numbers, not benchmarks.
   The governor was left at `powersave`, so clocks were not pinned either.
3. **n23's transport time is anomalous** at 34 040 s: it exceeds n25 and n27
   despite having fewer tiles, and sits ~2 400 s above the trend. A timing
   calibration was mistakenly pinned to CPU 20, the SMT sibling of CPU 2, and
   shared a physical core with n23 for about eight minutes; unpinned build jobs
   landing on the same core account for the rest. n23's *data* time
   (482.1 s) shows no such effect, and its physics is unaffected. **Do not use
   n23 in a timing fit.**
4. **One loading axis only.** One temperature, rods out, no control rods or
   absorber balls, R-Z homogenised reflector, realised TRISO count 8340 rather
   than 8335.
5. **The reference quotes no uncertainty** on any of its twelve values, so
   every `dk` here carries our sigma alone.

### Reproducing it

```bash
cargo build --release -p nee_soon --example htr10_endf8_height_sweep
./target/release/examples/htr10_endf8_height_sweep --list
for i in 0:20 1:21 2:23 3:25 4:27 5:29 6:31 7:33 8:35 9:37 10:39 11:41; do
  taskset -c ${i%%:*} ./target/release/examples/htr10_endf8_height_sweep n${i##*:} &
done; wait
```

Each case prints a `CSV,` line and an `ENTROPY,` line for machine extraction.
The collected data, the figure and its script are in the publications repo at
`outram_park/outram_park_intro_paper/outram_park_double_heterogeneity_arxiv/src/results_and_discussion/htr10_rmc_curve/`:

- `data/htr10_endf8_sweep_boundtsl_2026-09-24.csv`
- `scripts/plot_endf8_boundtsl_sweep.py`
- `figures/htr10_endf8_boundtsl_sweep.png`

### Timing model, for sizing the next run

Measured on this machine, single core, under twelve-way contention: **24.0
ms/history** at *n*ax = 20 (10 000 histories in 240.0 s). Per-height transport
cost scales roughly linearly with tile count over this range, from 22.0 to 26.0
ms/history across *n*ax 20 to 41.

To size a run: `hours ~ particles * generations * 24e-3 / 3600`, plus a flat
~480 s of data processing. The 1.4 M histories per case here bought
`sigma ~ 90–110 pcm`. For **ablation** work that is more precision than the
comparison needs — an ablation shares a seed and a geometry between arms, so
most of the noise is common-mode — and ~300 pcm is the right target, which is
about the 3000 × [20 + 60] the V&V record already uses.
