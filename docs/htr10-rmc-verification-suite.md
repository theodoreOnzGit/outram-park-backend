# HTR-10 — code-to-code verification against RMC, across the loading range

**Status:** complete for ENDF/B-VIII.0, 2026-09-24. Twelve fuel loadings, one
seed each. **This is verification, not validation, and it is not an agreement
result** — see "The gate is not met".
**Parent:** `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`,
which holds the model's construction history and the ablation chain.
**Source commit:** `77f8ecb14c` (develop) for the transport and data; the
cavity treatment has since changed — see "What has moved since".

---

## 1. Methodology

### What is computed

`k_eff` for the HTR-10 first-criticality core at **every fuel-loading height
the reference tabulates**, compared against RMC's value interpolated to the
height actually modelled.

### Reference

Li Wanlin, Yu Ganglin and Wei Chunlin, *"Research on Benchmark Calculation and
Analysis of HTR-10 with RMC Code"*, HTR 2014, Weihai — Tables 3 and 4, twelve
`(height, k_eff)` points. **The reference quotes no uncertainty on any of its
twelve values**, so every `±` below is ours alone.

### Model inputs

| | |
|---|---|
| Geometry | explicit TRISO, 14 rings × `n_axial` layers, 33 497 hex tiles, R-Z reflector |
| Fuel | Li et al. Table 2 — 6 cm pebble, 2.5 cm fuelled zone, 8335 particles, 250 µm kernel, UO₂ 10.4 g/cm³, 17 wt % |
| Reflector | IAEA-TECDOC-1382 Table 4-3, **single homogenised zone 22** |
| Data | ENDF/B-VIII.0 |
| Thermal laws | graphite (MAT 30), **C-in-SiC (44)**, **Si-in-SiC (43)**, **U-in-UO₂ (48)**, **O-in-UO₂ (75)** |
| Tracking | hybrid — delta-tracked bed inside a surface-tracked reflector |
| Temperature | 300.15 K, every material |

The five thermal laws are the state **after** 2026-09-23: SiC's carbon and
silicon and the UO₂ kernel previously scattered as free gases. The two UO₂
laws are **generated in-process** by the ported LEAPR from decks committed in
`njoy-outram-park-fork`; no UO₂ tape ships with this repository.

### Settings

```
10 000 histories per cycle × [5 inactive + 135 active] = 1 400 000 transported
seed 20260917, SINGLE SEED per height
32 logical CPUs
```

Five inactive cycles is **the reference's own choice**, not this repository's
default of 30. See §4 for why that turned out to be adequate.

### Comparison rule

Each point is compared against RMC **interpolated to the bed height actually
built**, not to the 123.576 cm headline. The curve rises ~270 pcm/cm through
this region, so comparing a 122.474 cm bed against the 123.576 cm point is
worth ~360 pcm of pure bookkeeping.

### Pass criterion

Agreement gate for this code-to-code reference is **500–1000 pcm**.

---

## 2. Results — the eigenvalue curve

`htr10_rmc_curve_2026-09-24.csv`:

```csv
naxial,heightcm,kmean,ksem,rmck,dkpcm,dksempcm,entropyfirst,entropylast
20,97.980,0.900313,0.000947,0.911138,-1082,103.9,5.4735,5.4621
21,102.879,0.920068,0.001041,0.932352,-1228,111.7,5.4725,5.4603
23,112.677,0.954487,0.000970,0.968224,-1374,100.2,5.4739,5.4720
25,122.474,0.989263,0.001169,1.000676,-1141,116.8,5.6159,5.5991
27,132.272,1.017606,0.000992,1.027764,-1016,96.5,5.6622,5.6512
29,142.070,1.045138,0.001035,1.053928,-879,98.2,5.7051,5.7134
31,151.868,1.065898,0.001077,1.076021,-1012,100.1,5.7244,5.7234
33,161.666,1.088731,0.001104,1.095242,-651,100.8,5.7508,5.7522
35,171.464,1.105463,0.001129,1.112818,-735,101.5,5.7706,5.7672
37,181.262,1.125712,0.000971,1.131730,-602,85.8,5.7638,5.7742
39,191.060,1.138939,0.001000,1.146030,-709,87.3,5.7803,5.7791
41,200.858,1.155702,0.001086,1.160581,-488,93.6,5.8066,5.8126
```

### The residual is not constant

Weighted fit over the twelve points:

| fit | result | chi-square |
|---|---|---|
| constant | `−881 ± 28 pcm` | **85.6 on 11 dof — rejected** |
| linear | slope **`+7.18 ± 0.85 pcm/cm`** | 15.0 on 10 dof |

`Δchi² = 70.6` on one degree of freedom, i.e. **8.4 sigma for the slope**.

### It agrees with two earlier, independent measurements

| measurement | library | seeds | slope [pcm/cm] |
|---|---|---|---|
| earlier curve | ENDF/B-VII.0 | 3 | `+6.71 ± 1.06` |
| earlier curve | ENDF/B-VIII.0 | 1 | `+5.74 ± 2.41` |
| **this suite** | ENDF/B-VIII.0 + 5 thermal laws | 1 | **`+7.18 ± 0.85`** |

Three measurements, two libraries, two physics revisions, all consistent.
**The drift with loading height is a property of the model**, not of the
evaluation and not of the thermal laws. What this suite adds is resolution:
8.4 sigma against 2.4.

### The gate is not met

At the benchmark loading (`n_axial` 25, 122.474 cm) the residual is
**`−1141 ± 117 pcm`**, outside the 500–1000 pcm gate. Every point is a single
seed; the seed-to-seed sd on this case is **179 pcm**, so no single point is
quotable to better than a few hundred pcm even at this history count.

---

## 3. Results — timing

**Nuclear-data processing and transport are timed separately**, because they
scale with different things: data prep with the nuclide count and the thermal
laws asked for, transport with histories × cycles × tiles. One combined number
makes a run impossible to reason about. Recorded per run by
`outram_mc_libs::run_diagnostics`.

### Per height — `htr10_height_timing_2026-09-24.csv`

```csv
naxial,heightcm,dataseconds,transportseconds,totalseconds,datafractionpercent
20,97.980,101.9,992.6,1094.5,9.3
21,102.879,101.8,1011.4,1113.2,9.1
23,112.677,101.8,1041.9,1143.7,8.9
25,122.474,102.0,1073.1,1175.1,8.7
27,132.272,101.8,1087.5,1189.3,8.6
29,142.070,101.9,1109.4,1211.3,8.4
31,151.868,101.8,1134.7,1236.5,8.2
33,161.666,102.0,1147.9,1249.9,8.2
35,171.464,101.9,1152.4,1254.3,8.1
37,181.262,102.0,1172.9,1274.8,8.0
39,191.060,102.2,1180.0,1282.2,8.0
41,200.858,101.8,1210.5,1312.3,7.8
```

**Data cost is flat at ~102 s** — the same nuclides and the same five thermal
laws are built whatever the bed height — while transport grows 993 → 1211 s
with the tile count. The data share therefore falls 9.3 % → 7.8 %. Of that
102 s, **15 s is LEAPR generating the two UO₂ laws** (U 6.0 s, O 9.0 s).

Total: 12 heights × ~20 min ≈ **4.1 hours** on 32 logical CPUs.

### P-cores vs E-cores — `htr10_affinity_timing_2026-09-24.csv`

2000 histories × [30 + 70], identical model on every row, each run in series
from an idle machine (1-minute load 1.18–1.46 at start).

```csv
affinity,threads,dataseconds,transportseconds,totalseconds,transportspeedup,peakrssmb
P-cores 0--15 (8 cores),16,103.5,262.7,366.2,8.06,271.2
E-cores 16--31,16,184.3,248.4,432.7,8.52,262.7
single P-core,1,165.6,2117.3,2282.9,1.00,222.4
single E-core,1,312.5,3449.3,3761.8,0.61,222.5
```

**Separating the two totals reverses the conclusion.** On combined wall time
the core types look comparable at 16 threads. On **transport alone the 16
E-cores are faster than the 8 P-cores** — 248.4 s against 262.7 s, E/P =
0.946 — which is what 16 physical cores should do against 8 with SMT once the
per-core deficit of 1.63× is accounted for (16/1.63 = 9.8 P-core equivalents).

The P-cores' advantage lives almost entirely in **nuclear-data processing**,
which is 1.78× slower on E-cores and **barely parallelises**: 165.6 s on one
P-core against 103.5 s on sixteen threads, a speed-up of 1.6× against
transport's 8.1×. Adding the two together hid both facts, and before
`run_diagnostics` there was one wall-clock number per row.

Settings were **recorded, not controlled** — governor left at `powersave`, so
absolute times are representative of a working desktop and the *ratios* are
the defensible quantity.

---

## 4. Source convergence — the five-cycle warning does not hold here

`htr10_rmc_keff` warns that the reference's five inactive cycles "are not a
model to copy" for a loosely coupled 1.8 m pebble core, because a thin
inactive stage biases `k`. **Tested rather than assumed.** Shannon entropy at
all twelve heights:

- first-to-last change within **±0.02 bits** and **non-monotonic**
- stationary from the first cycle at every height, *including the 2 m beds
  where the warning should bite hardest*

Entropy sits at ~5.47–5.81 bits against a 6.000 ceiling. That is expected —
the ceiling is a uniform source over the mesh and a real fission source is not
uniform. **Stationarity is the diagnostic, not proximity to the ceiling.**

So for this geometry and this initial source, five inactive cycles were
enough. That is a measurement of one configuration, not a general licence: it
says the starting source guess was good, not that a thin inactive stage is
safe.

The cost of the reference's cycle structure is **variance, not bias**: at
2000 × [5 + 14] the uncertainty was 434 pcm against 263 pcm for 2000 × [30 +
70] at the same particle count, because 14 active cycles is a thin sample from
which to estimate the cycle-to-cycle variance.

---

## 5. Determinism — measured five times, not assumed

The V&V record states that thread-count independence is *tested* for
`run_keff` and merely **assumed** for `run_keff_csg_hybrid`, which is the
driver this case uses. It now has direct evidence.

`k_eff = 0.988174 ± 0.002636` was returned **identically to six decimal
places** by: 16 threads on P-cores, 16 threads on E-cores, 1 thread on a
P-core, and once more after the material set was factored into
`htr10_rmc::materials`. The affinity sweep repeated the check at its own
settings: all four rows returned `0.987538 ± 0.003362`, again identical.

---

## 6. What is NOT modelled, and which way each pushes

- **The reflector is a single homogenised TECDOC zone (22)** — the cleanest
  graphite in Table 4-3, highest carbon, near-zero boron — with the boronated
  zones omitted. Both simplifications push `k` **up**.
- **The core-height reflector zone map is not placed.** The derived geometry
  record places only the bottom two axial layers.
- **Control-rod borings are solid graphite**, and no absorber is modelled at
  all: the model represents rods fully withdrawn and cannot represent any
  other position.
- **ENDF/B-VIII.0 against the reference's VII.0.** A whole-library swap was
  previously priced at `+1644 ± 438 pcm`, but **that number was measured
  under an asymmetry since corrected** (the UO₂ laws were withheld from the
  VII.0 arm) and needs re-measuring. Do not quote it.

---

## 7. The constant-cavity ablation — a deleted option, measured before it went

`htr10_constant_cavity_ablation_2026-09-24.csv`:

```csv
naxial,heightcm,kmean,ksem,dkpcm
20,97.980,0.913317,0.001084,+218
21,102.879,0.930498,0.001096,-185
23,112.677,0.962999,0.000992,-522
25,122.474,0.992269,0.001031,-841
27,132.272,1.014195,0.001151,-1357
29,142.070,1.037543,0.000955,-1639
31,151.868,1.058214,0.001019,-1781
33,161.666,1.073002,0.000936,-2224
35,171.464,1.089047,0.001056,-2377
37,181.262,1.105189,0.001148,-2654
39,191.060,1.119121,0.001072,-2691
41,200.858,1.131201,0.001027,-2938
```

The model used to hold the void above the bed at a constant 98.758 cm — exact
only at the benchmark loading — with the correct treatment behind an
`OUTRAM_HTR10_FIXED_CAVITY=1` opt-in. `cavity_above_bed`'s doc predicted the
sign of that approximation: too little void at low loading so `k` reads high,
too much at high loading so `k` reads low. **The prediction had never been
measured across the range.**

It is confirmed at both ends: slope **`−29.22 ± 0.89 pcm/cm`** at **32.9
sigma**, a constant residual rejected at chi² = 1121 on 11 dof, a **3156 pcm
swing** over 103 cm. Against the fixed-cavity suite in §2 the approximation
was worth about **−36 pcm/cm of spurious height dependence**.

**How this was obtained is worth recording.** It was run by mistake — the
first pass of the verification sweep omitted the flag, because the reference
curve's own README was not read before launching four hours of transport. The
resulting −29 pcm/cm trend was briefly mistaken for a physics finding. It is
kept as an ablation because it measures a documented approximation that
nothing else had, and because the failure mode is instructive: the run
completed, the entropy was stationary, every diagnostic was green, and the
curve was simply wrong by up to 3000 pcm with a convincing-looking trend.

It says **nothing** about the thermal laws — both sweeps carry them, and the
entire difference is the cavity treatment.

---

## 8. What has moved since

- **2026-09-25: the bottom reflector was MIRRORED from the top, and is now
  placed from the reactor.** The outer box was symmetric about the bed
  mid-height, so the bottom plane rode up with the bed and the bottom
  reflector was `314.872 - bed height` cm — 216.9 cm at 97.98 cm, 192.4 cm at
  the benchmark, 114.0 cm at 200.86 cm — against Terry's fixed 221.236 cm
  (z 388.764 → 610). **Every curve in §2 and §7 predates the fix** and must be
  re-measured before it is quoted. `the_axial_stack_matches_terry_at_every_loading`
  now pins top reflector, cavity, conus, bottom reflector and the 610 cm total
  at three loadings, on the planes the transport tracks.
- **2026-09-25: the ENDF/B-VII.0 arm now takes Si-29 and Si-30 from VII.0.**
  They were hardcoded to the VIII.0 tapes; 7.7 % of the SiC silicon came from
  the other evaluation.
- **`082cd413d9` deleted the constant-void option entirely.** The fixed cavity
  is now the only behaviour, so §2's curve describes current code and §7
  documents a path that no longer exists. Filed as gh:#292; resolved more
  strongly than that issue proposed.
- **`5618d72ed8` put the UO₂ laws in both library arms.** They had been gated
  alongside the SiC laws, which was wrong: SiC is genuinely absent from VII.0,
  but the UO₂ laws are generated from library-independent LEAPR decks, so
  withholding them put an artefact into the measured library term.
- **`48f25f561f` and `a4a0a773c9` repaired eight examples** that construct
  `Htr10Nuclides` and were not updated when `c_sic`, `si29` and `si30` were
  added. Worth knowing why it survived: `--lib --tests` does not reach
  examples and `-p <crate> --examples` does not reach another crate's. Only
  **`cargo check --release --workspace --all-targets`** catches it.

---

## 9. Reproducing this

```bash
# one height
OUTRAM_HTR10_HISTORIES=10000 OUTRAM_HTR10_INACTIVE=5 OUTRAM_HTR10_ACTIVE=135 \
OUTRAM_HTR10_RINGS=14 OUTRAM_HTR10_LAYERS=25 OUTRAM_HTR10_THREADS=32 \
  cargo run --release -p nee_soon --example htr10_rmc_keff
```

`n_axial` ∈ {20, 21, 23, 25, 27, 29, 31, 33, 35, 37, 39, 41} reproduces the
reference's twelve heights. Each run writes a diagnostics record naming every
data file used and the two timing totals; set `OUTRAM_MC_DIAGNOSTICS` to
choose the path.

The manuscript package carrying these CSVs, the figures and the typeset tables
is `publications/outram_park/outram_park_intro_paper/outram_park_double_heterogeneity_arxiv`,
commit `872c2a8`.
