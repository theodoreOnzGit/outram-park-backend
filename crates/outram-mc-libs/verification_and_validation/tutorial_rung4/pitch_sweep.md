# LCT-008 pin cell: k∞ against lattice pitch (under- and over-moderation)

**Class:** verification (code-to-code against OpenMC on a few points, and a
qualitative physics expectation). **Not validation**: no measurement exists
for an infinite lattice of this pin at other pitches. AI-assisted draft under
`RESPONSIBLE_USE.md`; no human V&V sign-off. Education and research only.

Tutorial rung 4 (GitHub #526, epic #520). Example:
`crates/outram-mc-libs/examples/lct008_pitch_sweep.rs`.

## Prediction (written 2026-10-04 22:10 +08:00, before any run)

Geometry ratios of the sweep, computed from the pin radii in
`examples/common/lct008_model.rs` (`R_FUEL = 0.514858`, `R_CLAD = 0.602996` cm)
before anything was transported:

| pitch [cm] | p/d | V_mod/V_fuel |
|---|---|---|
| 1.25 | 1.036 | 0.505 |
| 1.35 | 1.119 | 0.817 |
| 1.45 | 1.202 | 1.153 |
| 1.55 | 1.285 | 1.513 |
| **1.63576 (LCT-008)** | **1.356** | **1.841** |
| 1.75 | 1.451 | 2.306 |
| 1.90 | 1.575 | 2.963 |
| 2.10 | 1.741 | 3.924 |
| 2.40 | 1.990 | 5.545 |
| 2.80 | 2.322 | 8.043 |
| 3.30 | 2.736 | 11.705 |

Predictions, with the reasoning that makes each one capable of failing:

1. **Shape.** k∞ rises with pitch from the near-touching lattice, peaks, then
   falls. Rising side: more water per pin means faster slowing down past the
   U-238 resonances (p rises) and fewer neutrons absorbed in the resonances.
   Falling side: the water's own thermal absorption (H-1 and, here, the case-1
   1511 ppm soluble B-10) takes a growing share of thermal neutrons (f falls).
   A curve that is monotone over 0.5 ≤ V_m/V_f ≤ 12 refutes this.
2. **Factors.** Across the sweep `p` increases monotonically with pitch and `f`
   decreases monotonically. `η` and `ε` change much less than either (ε falls
   slowly as the fuel rods decouple, η moves only through the thermal spectrum).
3. **Where the peak is.** Because the case-1 water is borated, the peak sits at
   a smaller moderator ratio than an unborated lattice would have: I expect it
   between V_m/V_f ≈ 2 and 3 (pitch ≈ 1.7–1.9 cm).
4. **Where LCT-008 sits.** LCT-008's pitch (V_m/V_f = 1.84) is on the
   **under-moderated** side of that peak, but close to it: within ~1000 pcm of
   the maximum k∞.
5. **Magnitude.** k∞ at the LCT-008 pitch is above 1 (the real core is critical
   with leakage), roughly 1.05–1.20.
6. **Code-to-code.** On the points run with OpenMC on the same 11-nuclide tier,
   outram-mc and OpenMC agree within 2σ of the combined uncertainty, or within
   the ~10–40 pcm route-4 vs route-1 offsets the five-route record shows on the
   full lattice (`icsbep/five_route_keff_2026_09_29.md`).

The measured result, including where these predictions fail, is recorded
below. Nothing in the example is adjusted after seeing it.

## Methodology

- **Model.** One LCT-008 rod in a square cell with reflective x, y and z
  planes, which makes an infinite lattice. The eigenvalue is k∞. The pin is
  UO₂ with r < 0.514858 cm and Al-6061 clad out to 0.602996 cm, with no gap.
  Materials come from the case-1 `materials.xml` of the committed
  `mit-crpg/benchmarks` LCT-008 model, parsed by
  `examples/common/lct008_model.rs` (outram-mc) and by
  `openmc.Materials.from_xml` (OpenMC).
- **Data.** ENDF/B-VIII.0 at 293.6 K, with H in H₂O S(α,β), URR probability
  tables and DBRC (E ≤ 1 keV) on.
  - outram-mc reads the tapes directly (route 4 of the five-route study, Fast
    tier, RECONR/BROADR at tolerance 0.001).
  - OpenMC 0.16.1-dev25 (`d7d3284a1`) reads the five-route route-1 library:
    NJOY2016 2016.79 ACE → HDF5, from `target/five_route_keff/h5_njoy/`,
    built by `icsbep/five_route_keff/scripts/make_njoy_library.sh`.
- **Nuclide tier.** The code-to-code comparison uses the 11-nuclide tier on
  both codes, because the OpenMC library has no other nuclides. The other 24
  nuclides are dropped, not renormalised. outram-mc's default 36-nuclide
  tier was run at three pitches.
- **Settings.** 5000 neutrons × [50 inactive + 200 active], seed 1, one run
  per point and per code.
  - **k and its σ are the generation mean and its standard error over the
    active generations** on both sides (outram-mc's estimator). OpenMC's
    combined estimator is in the CSV too.
  - A single-run standard error ignores inter-generation correlation, so it
    is a lower bound on the true σ.
- **Source convergence.** OpenMC's Shannon entropy on an 8 × 8 × 4 mesh over
  the fuel is flat from the first inactive batch: 7.73–7.76 at every case-1
  point. The last-inactive and active-mean values agree to 0.03. On a single
  pin, 50 inactive generations is ample.
- **Pass criteria, fixed before running.**
  - Code-to-code: agreement within 2σ of the combined single-run σ, judged
    over the set (χ² over 11 points), not point by point.
  - Physics: predictions 1–5 above.
- **Hardware.** 13th Gen Intel Core i9-13900K, 16 logical cores, 62 GB RAM,
  Linux, CPU only. Every run was pinned to cores 8–9 with 2 threads. The
  machine was shared and loaded: load average 15–30 during the runs, with
  other agents' builds and rust-analyzer running. The two codes partly ran
  at the same time. Timings are indicative only.

Data:
- `data/pitch_sweep_cheap11_case1.csv`
- `data/pitch_sweep_cheap11_noboron.csv`
- `data/pitch_sweep_full36_case1.csv`
- `data/openmc_cheap11_case1.csv`
- `data/openmc_cheap11_noboron.csv`

Figures:
- `figures/kinf_vs_moderator_ratio.png`, made by `plot_pitch_sweep.py`.
- `figures/lct008_pin_cell_pitch_*.png`, one geometry drawing per pitch,
  drawn from the assembled CSG with `render_material_slice`.
  - What was checked: fuel and clad radii against the axes, square
    reflective cell edges at ±pitch/2, and no overlap colour.
  - Not drawn: an axial slice. The cell is z-invariant between reflective
    planes at ±10 cm.

### Commands

```text
RAYON_NUM_THREADS=2 taskset -c 8-9 cargo run --release -p outram-mc-libs \
    --features endf-pebble-cases --example lct008_pitch_sweep -- --cheap-nuclides \
    --particles 5000 --inactive 50 --active 200 --seed 1 \
    --csv data/pitch_sweep_cheap11_case1.csv --plots figures
# (+ --no-soluble-boron --pitches 1.45,1.63576,1.90,2.40; and without
#  --cheap-nuclides at --pitches 1.25,1.63576,2.40)
OMP_NUM_THREADS=2 taskset -c 8-9 python openmc_inputs/lct008_pin_cell_openmc.py \
    --pitch <p> --xs <h5_njoy>/cross_sections.xml --seed 1 --threads 2 \
    --particles 5000 --inactive 50 --active 200 --csv openmc_cheap11_case1.csv \
    --workdir runs/<p>   [--no-soluble-boron]
```

The OpenMC driver is committed verbatim:
[`openmc_inputs/lct008_pin_cell_openmc.py`](openmc_inputs/lct008_pin_cell_openmc.py).
outram-mc ran from the binary built at `7e9b8bd637`. The example has not
changed since that commit.

## Results (2026-10-04, seed 1)

### Case-1 borated water, 11-nuclide tier

| pitch [cm] | V_m/V_f | outram-mc k∞ | OpenMC k∞ | outram − OpenMC [pcm] | η | f | p | ε |
|---|---|---|---|---|---|---|---|---|
| 1.25 | 0.505 | 1.06481 ± 0.00128 | 1.06476 ± 0.00144 | +4 ± 193 (0.0σ) | 1.7623 | 0.8775 | 0.4662 | 1.4741 |
| 1.35 | 0.817 | 1.11386 ± 0.00112 | 1.11621 ± 0.00151 | −235 ± 188 (−1.3σ) | 1.7683 | 0.8242 | 0.5894 | 1.2964 |
| 1.45 | 1.153 | 1.11886 ± 0.00125 | 1.11599 ± 0.00137 | +288 ± 186 (+1.6σ) | 1.7719 | 0.7717 | 0.6687 | 1.2203 |
| 1.55 | 1.513 | 1.09196 ± 0.00127 | 1.09381 ± 0.00131 | −185 ± 183 (−1.0σ) | 1.7741 | 0.7214 | 0.7226 | 1.1796 |
| **1.63576** | **1.841** | **1.06403 ± 0.00117** | **1.06061 ± 0.00130** | **+342 ± 174 (+2.0σ)** | 1.7755 | 0.6808 | 0.7591 | 1.1569 |
| 1.75 | 2.306 | 1.00769 ± 0.00125 | 1.01178 ± 0.00119 | −409 ± 173 (−2.4σ) | 1.7769 | 0.6293 | 0.7944 | 1.1372 |
| 1.90 | 2.963 | 0.93855 ± 0.00113 | 0.93926 ± 0.00117 | −72 ± 163 (−0.4σ) | 1.7780 | 0.5670 | 0.8286 | 1.1215 |
| 2.10 | 3.924 | 0.83815 ± 0.00113 | 0.83991 ± 0.00104 | −175 ± 154 (−1.1σ) | 1.7791 | 0.4937 | 0.8601 | 1.1095 |
| 2.40 | 5.545 | 0.70379 ± 0.00107 | 0.70674 ± 0.00088 | −294 ± 138 (−2.1σ) | 1.7800 | 0.4040 | 0.8911 | 1.1005 |
| 2.80 | 8.043 | 0.56160 ± 0.00098 | 0.56125 ± 0.00080 | +35 ± 127 (+0.3σ) | 1.7807 | 0.3138 | 0.9158 | 1.0972 |
| 3.30 | 11.705 | 0.42922 ± 0.00092 | 0.42807 ± 0.00069 | +116 ± 115 (+1.0σ) | 1.7812 | 0.2345 | 0.9331 | 1.1001 |

η, f, p and ε are outram-mc's three-group factors (thermal below 0.625 eV,
resonance up to 0.1 MeV). Their product matches k to within −0.24 % to
+0.27 % (`consistency_gap` in the CSV). Tallied leakage is zero at every
pitch, as a reflective cell requires (P_FNL = P_TNL = 1).

### Soluble boron removed (explicit ablation `--no-soluble-boron`), 11-nuclide tier

| pitch [cm] | V_m/V_f | outram-mc k∞ | OpenMC k∞ | outram − OpenMC [pcm] |
|---|---|---|---|---|
| 1.25 | 0.505 | — | 1.13938 ± 0.00156 | — |
| 1.35 | 0.817 | — | 1.24719 ± 0.00155 | — |
| 1.45 | 1.153 | 1.30408 ± 0.00114 | 1.30303 ± 0.00161 | +105 ± 197 (+0.5σ) |
| 1.55 | 1.513 | — | 1.33612 ± 0.00155 | — |
| **1.63576** | **1.841** | **1.34838 ± 0.00112** | **1.34712 ± 0.00157** | **+127 ± 193 (+0.7σ)** |
| 1.75 | 2.306 | — | 1.35073 ± 0.00147 | — |
| 1.90 | 2.963 | 1.33811 ± 0.00121 | 1.33703 ± 0.00150 | +108 ± 193 (+0.6σ) |
| 2.10 | 3.924 | — | 1.29812 ± 0.00159 | — |
| 2.40 | 5.545 | 1.21802 ± 0.00123 | 1.21696 ± 0.00140 | +106 ± 186 (+0.6σ) |
| 2.80 | 8.043 | — | 1.09595 ± 0.00131 | — |
| 3.30 | 11.705 | — | 0.94329 ± 0.00118 | — |

### All 36 nuclides (outram-mc's default tier), case-1 water

| pitch [cm] | full 36 | 11-nuclide | full − 11 [pcm] |
|---|---|---|---|
| 1.25 | 1.06707 ± 0.00130 | 1.06481 ± 0.00128 | +226 ± 182 (1.2σ) |
| 1.63576 | 1.06157 ± 0.00115 | 1.06403 ± 0.00117 | −246 ± 164 (1.5σ) |
| 2.40 | 0.70600 ± 0.00116 | 0.70379 ± 0.00107 | +221 ± 158 (1.4σ) |

The tiers are unpaired: their random streams diverge at the first collision
with a dropped nuclide. The three differences change sign and none is
resolved. **These runs do not measure the worth of the 24 dropped
nuclides**; they only bound it at a few hundred pcm on a pin cell.
Follow-up: #533.

### Timings (indicative: shared machine, see Methodology)

| run | nuclear data | transport per point (5000 × 250) |
|---|---|---|
| outram-mc, 11 nuclides | 91.8–162.3 s | 90–491 s. The first four points ran alongside OpenMC and rust-analyzer; the later, quieter ones took 90–146 s |
| outram-mc, 36 nuclides | 94.8–113.8 s | 212–1263 s. The 1263 s point ran alongside a `kovan-cli` build and rust-analyzer indexing |
| OpenMC, 11 nuclides | (pre-built HDF5) | 49–95 s wall per point, including start-up |

## Interpretation

**Code-to-code (verification).**
- With boron, outram-mc − OpenMC has no trend across the sweep. The signs
  alternate, and the mean over 11 points is −53 pcm.
- χ² = 21.9 over 11 points (p ≈ 0.025). Three points sit at 2.0–2.4σ: +342
  at the LCT-008 pitch, −409 at 1.75 cm and −294 at 2.40 cm.
- That is **mild tension, not a clean pass**. The single-run σ understates
  the true σ because generations are correlated. One seed per point cannot
  separate that from a small real difference.
- Without boron, all four points are +105 to +127 pcm (0.5–0.7σ each),
  combined +112 ± 96 pcm, which is not resolved.
- **Neither code is the reference.** ~~A seed ensemble at the three 2σ points
  would settle it, and was not run.~~ **CORRECTED 2026-11-18 (gh:#550):** it
  was run, 16 seeds per code: 1.63576 and 1.75 cm agree (z −0.97 and −0.01),
  2.40 cm misses the |z| ≤ 2 gate (−117 ± 39 pcm, z −2.97). See "Seed ensemble"
  at the end of this file. For scale, on the full LCT-008 lattice
  the two codes agree at −8 ± 9 pcm over 96 seeds
  (`icsbep/five_route_keff_2026_09_29.md`).

**Physics: predictions against results.**

| # | prediction (written before running) | result |
|---|---|---|
| 1 | k∞ rises, peaks, falls | **held**, on both codes, with and without boron |
| 2 | p rises and f falls monotonically; η and ε change much less | **p and f held**: p 0.466 → 0.933, f 0.878 → 0.234. **η held**: 1.762 → 1.781 (1 %). **ε did not change "much less"**: it fell from 1.474 to 1.100 (−25 %), mostly at the tightest pitches, where fast neutrons reach the next rod before the water |
| 3 | with borated water, peak at V_m/V_f ≈ 2–3 (pitch 1.7–1.9 cm) | **refuted.** With 1511 ppm boron both codes peak between 1.35 and 1.45 cm (V_m/V_f ≈ 0.8–1.15). Without boron OpenMC peaks near 1.75 cm (V_m/V_f ≈ 2.3), which is where I had put the borated peak. I underestimated how much 1511 ppm of B-10 penalises water |
| 4 | LCT-008 under-moderated, within ~1000 pcm of the peak | **refuted for the borated lattice**: LCT-008 is **over-moderated**, about 5500 pcm below the peak (k∞ 1.064 against ≈ 1.119). **Held for the unborated lattice**: 1.347 against 1.351 at 1.75 cm (OpenMC), slightly under-moderated and within ~400 pcm |
| 5 | k∞ at the LCT-008 pitch between 1.05 and 1.20 | **held**: 1.064 (outram-mc), 1.061 (OpenMC) |
| 6 | the codes agree within 2σ over the set | **mostly**: 8 of 11 borated points are within 2σ, and all 4 unborated points are within 1σ (see above) |

**What it means for the lesson.** Soluble boron moves the optimum. The same
lattice is under-moderated in clean water and over-moderated in LCT-008
case 1's 1511 ppm water. In an over-moderated lattice, removing water raises
k∞: voiding it, or its thermal expansion, adds reactivity. That is the sign
problem soluble boron creates, and the reason power reactors limit their
boron concentration.

Two cautions:
- **This is k∞ of an infinite pin lattice, not the finite core**, which
  leaks and contains all-water tiles.
- **The moderator-coefficient statement is an inference from k∞(pitch).** No
  temperature or void coefficient was computed.

## Seed ensemble at the three 2σ points (GitHub #550, started 2026-11-18)

### Prediction (written 2026-11-18, committed before any run of this ensemble)

Setting: case-1 borated water, 11-nuclide tier, 5000 × [50 + 200], pitches
1.63576, 1.75 and 2.40 cm, seeds 1–16 on each code (16 runs per code per
pitch). The seeds are paired by *setting* only: the two codes have different
random-number generators and different physics routines, so seed `i` of one
is not correlated with seed `i` of the other. The difference is therefore an
unpaired comparison of two means.

Reasoning, from numbers already on record and not from the outcome:

- Single-run σ is 88–130 pcm per code at these pitches (table above). The
  generation-mean standard error understates the true run-to-run σ because
  generations are correlated; I assume the true per-seed sd is 1.0–1.5× that.
- With N = 16 the standard error of a code's mean is sd/4, i.e. about 30–45 pcm,
  and the difference of two codes about **45–60 pcm**. The gate
  (|z| ≤ 2) is then a window of about ±90–120 pcm.
- The single-run differences were +342, −409 and −294 pcm. A difference of
  that size is 1.9–2.4σ of a single-run σ. Two or three points at 2σ among 11
  is what noise gives about half the time, but the χ² of 21.9 (p ≈ 0.025) is
  mildly high, and the neighbouring borated points (1.55: −185, 1.90: −72,
  2.10: −175 pcm) lean the same way as 1.75 and 2.40. The unborated points
  lean positive (+105 to +127 pcm).

Predicted 16-seed outram-mc − OpenMC:

| pitch [cm] | predicted difference | predicted z | gate |
|---|---|---|---|
| 1.63576 | +50 pcm (within −50 … +150) | about +1 | pass. +342 was a fluctuation: no neighbour supports it |
| 1.75 | −100 pcm (within −200 … 0) | about −2 | borderline. A small real offset of ~−100 pcm is plausible and would sit at the gate |
| 2.40 | −100 pcm (within −200 … 0) | about −2 | borderline, for the same reason |

If the single-run tension was statistical, all three move to |diff| < 100 pcm
and |z| ≤ 2. A real offset of ≥ 200 pcm would persist at |z| > 3. Differences
I am aware of and cannot rule out in advance: the OpenMC binary used here is
0.15.3 (the original record used 0.16.1-dev25), and the HDF5 library is
rebuilt from the same NJOY2016 ACE recipe, not the original files. Nothing is
adjusted after the runs.

### Method of the ensemble (run 2026-11-18)

- **Settings, unchanged from the single runs:** case-1 borated water, 11-nuclide
  tier, 5000 × [50 inactive + 200 active], 293.6 K, URR on, resonance
  scattering on, S(α,β) on the water. Only the seed changed: seeds 1–16 on each
  code at each of 1.63576, 1.75 and 2.40 cm, so 48 runs per code.
- **Pairing.** By setting and seed number only. The codes have different
  random-number generators and physics routines, so seed `i` of one code is not
  correlated with seed `i` of the other, and the comparison is unpaired (Welch):
  σ_diff = √(sem_outram² + sem_OpenMC²).
- **Estimator.** Per-seed k is each code's generation mean over the 200 active
  generations (outram-mc's `k_mean`; OpenMC's `k_gen_mean`, as the original
  single runs). Mean, sd (n − 1) and sem = sd/√16 are over the 16 seeds. OpenMC's
  own combined estimator is in `openmc_seeds.csv` too; it is quoted below.
- **outram-mc:** `examples/lct008_pitch_sweep.rs` with `--cheap-nuclides
  --particles 5000 --inactive 50 --active 200`, `OUTRAM_PITCH_SEEDS=n`, at
  HEAD `46ac0fab4` (the prediction commit, on `5dace1194`). Four processes ran
  it in pieces, because the machine was heavily loaded and I split the seeds:
  seeds 1–8, 9–12, 13–14 and 15–16, each with `--seed` set to its first seed
  (`ensemble_550/parts/`). Every run is an independent master seed; the split
  changes nothing but the load.
  **The example has changed since the original record:** the single runs used
  the binary built at `7e9b8bd637`; the per-material S(α,β) migration (gh:#1581)
  and later commits touched the example and the library. This is not the same
  code as the single runs.
- **OpenMC:** 0.15.3, commit `27e38e894697bb32a1dac7848d2618818b6b8daf`, built
  from source, no MPI (the original used 0.16.1-dev25, `d7d3284a1`). Deck:
  `openmc_inputs/lct008_pin_cell_openmc.py`, **unchanged and verbatim**, run
  by `ensemble_550/run_openmc_ensemble.sh`. `OMP_NUM_THREADS=2`, `--threads 2`.
- **OpenMC library, rebuilt the way the original record says:** route 1 of the
  five-route study, NJOY2016 2016.79 ACE → HDF5, made with
  `icsbep/five_route_keff/scripts/make_njoy_library.sh` and
  `openmc_inputs/ace_to_hdf5_route.py`, for the 11 nuclides in
  `ensemble_550/nuclides_11.tsv` (build log `make_njoy_library.log`). U-234,
  U-235, U-238 at 293.6 K and U-235 at 0 K were taken from the `reference-data/ace`
  submodule as the script does; everything else (including the other 0 K
  companions and H in H₂O at iwt = 1) was generated here with the same NJOY2016
  build. The tables are **not the original files**, they are a rebuild from
  the same recipe. B-10 went through the script's Type-2 fallback, as
  recorded there.
- **Environment fix (not a repository change):** the shared venv had no
  `openmc/lib/libopenmc.so`, which `Model.run` imports. I symlinked the
  installed `openmc-install/lib/libopenmc.so` into it. It does not alter the
  transport.
- **Hardware.** Intel Xeon @ 2.10 GHz (`ensemble_550/hardware.txt`), 4 logical
  cores, 15 GB RAM, Linux 6.18.44, CPU only, a VM **shared with at least two
  other agents; load average 6–13 on 4 cores throughout**. Threads: 2 per
  process, at most 2 heavy processes of mine at a time (one outram-mc and one
  OpenMC, later two outram-mc). Timings:
  - OpenMC: 48 runs, 51–137 s each, mean 72 s, 3450 s in total.
  - outram-mc: 48 runs, 199–588 s of transport each, mean 375 s, 18 011 s of
    transport in total (sum over processes). Nuclear data load (RECONR + BROADR)
    346–471 s per process, four times.
  - Timings are indicative only: the load was high and varied.
- **Raw outputs** (small text): `ensemble_550/outram_seeds.csv`,
  `openmc_seeds.csv`, `parts/` (per-process CSVs and console logs),
  `openmc_stdout.log`, `analysis_output.md`, produced by `analyse.py`.
- **Source convergence.** OpenMC's Shannon entropy columns in the CSV: the
  last-inactive and active-mean values sit at 7.7 in every run, as in the
  original record.

### Results (16 seeds per code, case-1 borated, 11-nuclide tier)

| pitch [cm] | code | mean k∞ | sd over seeds | sem | mean single-run σ | sd / single-run σ |
|---|---|---|---|---|---|---|
| 1.63576 | outram-mc | 1.06216 | 126 pcm | 31 pcm | 106 pcm | 1.19 |
| 1.63576 | OpenMC | 1.06253 | 87 pcm | 22 pcm | 126 pcm | 0.70 |
| 1.75 | outram-mc | 1.01104 | 63 pcm | 16 pcm | 103 pcm | 0.61 |
| 1.75 | OpenMC | 1.01104 | 132 pcm | 33 pcm | 120 pcm | 1.10 |
| 2.40 | outram-mc | 0.70536 | 107 pcm | 27 pcm | 97 pcm | 1.10 |
| 2.40 | OpenMC | 0.70653 | 116 pcm | 29 pcm | 91 pcm | 1.27 |

| pitch [cm] | outram-mc − OpenMC [pcm] | σ [pcm] | z | gate (\|z\| ≤ 2) | single run, 2026-10-04 |
|---|---|---|---|---|---|
| 1.63576 | −37 | 38 | −0.97 | pass | +342 ± 174 |
| 1.75 | −1 | 37 | −0.01 | pass | −409 ± 173 |
| **2.40** | **−117** | 39 | **−2.97** | **MISS** | −294 ± 138 |

Per-seed values for both codes are in `ensemble_550/analysis_output.md`.
With OpenMC's own combined estimator instead of the generation mean, OpenMC's
means are 1.06204, 1.01123 and 0.70650 (sem 17, 21 and 20 pcm); at 2.40 cm the
difference is −114 pcm, z = −3.4, so the miss does not depend on the choice of
estimator.

### Prediction against outcome

| pitch | predicted difference, z | measured | verdict |
|---|---|---|---|
| 1.63576 | +50 pcm, about +1 | −37 pcm, −0.97 | gate passes as predicted. The sign was wrong; the size is within the window I gave (−50 … +150) |
| 1.75 | −100 pcm, about −2 | −1 pcm, −0.01 | gate passes, but **my predicted offset did not appear**: the −409 pcm single run was a fluctuation |
| 2.40 | −100 pcm, about −2 | −117 pcm, −2.97 | **gate missed.** The size matched my prediction; I had called it borderline and the sem came out smaller than the one implied by my −2 |

- The predicted sem of the difference was 45–60 pcm. It came out at 37–39 pcm,
  because the true per-seed sd was 0.6–1.3 times the single-run σ rather than the
  1.0–1.5 times I assumed.
- **Two of the three single-run 2σ points are gone** (1.63576, 1.75 cm). They
  were statistical.
- **The 2.40 cm point remains and is now a 3σ miss.** I report it as a miss.
  The 16 seeds put outram-mc low by −117 ± 39 pcm (−114 ± 34 pcm with OpenMC's
  combined estimator). Nothing was changed to improve it: no input, seed
  selection, threshold or estimator was altered after seeing the numbers.
- **What it does not show.** The ensemble does not say which code is
  right; neither is the reference. It does not show a trend (the other two
  pitches are consistent with zero). It does not separate a code defect from a
  data difference: the OpenMC data are NJOY2016 ACE, outram-mc's are its own
  RECONR/BROADR from the tapes, and the OpenMC version and the outram-mc code
  both differ from the original record. The gate is pass / pass / miss on three
  points; the unborated points (all +105 to +127 pcm) were not rerun here.
- ~~**Follow-up not done:** locating the 2.40 cm difference needs a
  factor-by-factor tally comparison against OpenMC, or reruns with single
  physics options switched off. No hypothesis was tested here.~~ **CORRECTED
  2026-11-18 (gh:#550):** done, see "2.40 cm miss: diagnosis" below; the
  difference is carried by the H-in-H2O S(a,b) table (A minus B +135 +- 35 pcm).

## 2.40 cm miss: diagnosis (GitHub #550, started 2026-11-18)

**This section is committed with its hypotheses and predictions BEFORE any
diagnostic run.** Results are appended below it afterwards; a prediction that
misses stays in place with its outcome beside it. Research, education and V&V
only. Gate throughout: |z| <= 2 on a seed ensemble (>= 8 seeds per arm), no
threshold moved, no input tuned.

### The finding being explained

Case-1 borated water, 11-nuclide tier, 5000 x [50 + 200], 16 seeds per code,
2.40 cm: outram-mc 0.70536 +- 0.00027, OpenMC 0.70653 +- 0.00029, difference
**-117 +- 39 pcm (z -2.97)**; -114 pcm (z -3.4) with OpenMC's combined
estimator. 1.63576 cm (-37 +- 38) and 1.75 cm (-1 +- 37) pass. outram-mc's
2.40 cm factors (16-seed means): eta 1.7800, f 0.4041, p 0.8913, eps 1.1004
(`ensemble_550/outram_seeds.csv`). This is the most over-moderated point:
f is 0.40 because 1511 ppm of B-10 and the water absorb most thermal
neutrons, so k there measures the thermal spectrum and the thermal absorption
in water and boron far more than at 1.63576 cm (f 0.68).

### Upstream read first (OpenMC 0.15.3, `openmc-src/`), and what it implies

What OpenMC does with a hydrogen atom in this water, read from source before
forming any hypothesis:

- `src/material.cpp:855-870`: the S(a,b) table is used for H-1 while
  `E <= energy_max_` of the thermal table (the top of the inelastic grid, 4 eV
  for NJOY THERMR `emax`), and dropped above it. `sab_frac = 1` here (all H-1
  is in the water).
- `src/nuclide.cpp:841-869` (`calculate_sab_xs`): inside the table the
  scattering cross section is the table's elastic + inelastic, and the
  free-atom elastic is removed (`micro.elastic = thermal + (1 - sab_frac)
  elastic`).
- `src/thermal.cpp:297-312` (`ThermalData::sample`): elastic or inelastic is
  chosen by `thermal_elastic / thermal`; the outgoing law is the one NJOY
  THERMR/ACER wrote into the table.
- `src/physics.cpp:857-862` (`sample_target_velocity`): a non-resonant nuclide
  gets the free-gas treatment only below `400 kT` and only for `awr > 1`; a
  resonant nuclide (the deck switches resonance scattering on for every
  nuclide with a 0 K table, 0.01 eV .. 1 keV) keeps it to 1 keV.
- `src/physics.cpp:700-714`: the free-gas `kT` is `nuc->kTs_[i_temp]`, the kT
  stored with the data.

outram-mc already carries each of these (`ThermalScattering::beyond_cutoff`
cites `material.cpp:866`; `Nuclide::free_gas_kt`; the 400 kT threshold). What
differs between the two arms by construction is therefore **where the data
come from**, not the transport rules:

1. neutron data: outram-mc reconstructs from the ENDF tapes (RECONR+BROADR);
   OpenMC reads the NJOY2016 ACER output converted to HDF5;
2. **S(a,b) of H in H2O: outram-mc builds it from `tsl-HinH2O.endf` with its
   own THERMR-equivalent (`ThermalScattering::from_endf_file`); OpenMC reads
   NJOY2016's THERMR+ACER table (`c_H_in_H2O.h5`).** They are meant to be the
   same physics; they are different code, with different alpha/beta grids,
   cutoffs and outgoing-energy binning.

### Hypotheses, with predicted sign and magnitude (outram-mc minus OpenMC)

Reference arm: outram-mc, ENDF route, ENDF S(a,b), the existing 16 seeds,
0.70536. Every new outram-mc arm is 5000 x [50 + 200] at 2.40 cm, 8 seeds
(seeds 1-8) unless stated.

| id | hypothesis | discriminator | prediction if TRUE | prediction if FALSE |
|---|---|---|---|---|
| H1b | (cheap precursor of H1, no transport) the S(a,b) cross sections of the two constructions differ | evaluate `inelastic_xs(E)`, `elastic_xs(E)` of `from_endf_file` and of the NJOY ACE (`from_ace`) on a common grid, 1e-5 .. 4 eV | a band of >= 0.3 % relative difference somewhere in 0.01 .. 0.3 eV (a 0.3 % change of the thermal hydrogen scattering cross section moves a f = 0.40 lattice by ~ 50-100 pcm) | < 0.1 % everywhere |
| H1 | the S(a,b) built from the ENDF tape differs from NJOY's table in a way that matters most when the spectrum is most thermal | outram-mc with the S(a,b) read from the NJOY ACE `HinH2O.ace` (293.6 K) that OpenMC's `c_H_in_H2O.h5` was made from, everything else unchanged | k rises by **+60 to +130 pcm** (towards OpenMC; outram-mc is the low code) | shift within +-50 pcm |
| H2 | the neutron data (RECONR/BROADR vs NJOY ACE) differ | outram-mc reading the very same `.h5` library OpenMC reads (`--hdf5-dir`, gh:#303), ENDF S(a,b) kept | **0 +- 50 pcm** (the full LCT-008 lattice agrees at -8 +- 9 pcm over 96 seeds; the h5 route against the ENDF route measured +39 +- 65 pcm at 1.63576 cm). A shift beyond +-80 pcm would make it a neutron-data effect, with B-10 (1/v absorption, weighted by f = 0.40) the first suspect | within +-50 pcm |
| H3 | the gap tracks thermal physics at all | S(a,b) switched off in BOTH codes (H-1 free gas instead). k falls by thousands of pcm in both; the question is the difference | the outram-mc minus OpenMC difference changes by **> 100 pcm** from -117 | the difference stays near -117 +- 60 (gap lives outside the S(a,b)) |
| H4 | a statistical fluke of the first ensemble (z = -2.97 at one of three pitches; p ~ 0.003 two-sided, ~ 0.01 after three looks) | independent new seeds: outram-mc ENDF route seeds 17-24 and OpenMC seeds 17-32 at 2.40 cm | the new difference is -40 +- 50 pcm (regression to the mean) | the new difference is again about -117 |
| H5 | the gap sits in the thermal absorption share, which a tally comparison localises | OpenMC tallies (absorption by nuclide, thermal/resonance/fast groups) against outram-mc's factors on record | f differs by about -0.3 % relative (a harder or softer thermal spectrum changes the B-10 and H-1 absorption share) | p or eps differ instead: a resonance/fast difference, and H1 is wrong |

Order of work, cheapest first: H1b (no transport), H2 (one outram-mc arm), H1
(one outram-mc arm), H4 and H3 as the budget allows, H5 for localisation.

Prior, stated honestly: H1 ~ 40 %, H4 ~ 15 %, H2 ~ 15 %, ~ 30 % on something
not on this list. The transport rules were ported from OpenMC and are pinned
elsewhere; the thermal tables are the one component with two independent
derivations.

### Added 2026-11-18 after H1b, H1 and H2 were measured, BEFORE H6 is run

H1b, H1 and H2 returned first (results are in the section below); they point
at the S(a,b) outgoing-energy kernel, not at the cross sections or the neutron
data. One more hypothesis, with its prediction, before it is run:

| id | hypothesis | discriminator | prediction |
|---|---|---|---|
| H6 | the tape-built S(a,b) kernel (384 incident energies x 64 outgoing bins by default) is not converged in a way that matters, and refining it moves k towards NJOY's table | outram-mc from the OpenMC `.h5`, ENDF-built S(a,b) on a 1536 x 256 emission grid (`--sab-grids 1536,256`), 2.40 cm, seeds 1-8, compared with the same arm at 384 x 64 (arm B) | the k shift (fine minus default) is **+60 pcm** (range -50 .. +150): in the one single-energy check made before this (`sab_endf_vs_ace_550`), the fine table's mean outgoing energy sits above the default's at 0.1 and 1.5 eV (+3.7 % and +2.4 %) and the NJOY table sits between them, so refinement should move the default towards NJOY, but the sign of the net thermalisation effect is not something I can derive |

### Results (2026-11-18)

Hardware for every number: Intel Xeon @ 2.10 GHz, 4 logical cores, 15 GB RAM,
Linux 6.18.44, CPU only, a VM **shared with other agents (load average 7-10 on
4 cores for most of it)**. 2 transport threads per process, at most 2 heavy
processes of mine at a time. Settings as in the ensemble: 5000 x [50 + 200],
2.40 cm, case-1 water, 11-nuclide tier, 293.6 K, URR and resonance scattering
on. Code: this worktree at `755275f9a` (the new flags only add options; the
default path is unchanged). Raw per-seed CSVs are not committed (scratch);
the per-seed values are in the tables below. OpenMC 0.15.3 (`27e38e894`) with
the same HDF5 library as the ensemble. Statistics are unpaired (Welch):
sem of each mean, difference sigma = quadrature sum.

**OpenMC reference at 2.40 cm, now 32 seeds** (the ensemble's 1-16 plus new
seeds 17-32 of the same deck): 0.70652 +- 0.00020 (sd 113 pcm). Seeds 17-32
minus seeds 1-16: -2 +- 41 pcm, so the OpenMC mean reproduces.

| arm (outram-mc) | n seeds | k at 2.40 cm | minus OpenMC (32 seeds) | z |
|---|---|---|---|---|
| ensemble baseline: ENDF neutron data, ENDF-built S(a,b) (384 x 64) | 16 | 0.70536 +- 0.00027 | -116 +- 33 pcm | -3.47 |
| **B**: neutron data from the OpenMC `.h5`, ENDF-built S(a,b) | 8 | 0.70559 +- 0.00027 | -93 +- 33 | -2.78 |
| **A**: neutron data from the OpenMC `.h5`, **S(a,b) from NJOY's `HinH2O.ace`** (the table `c_H_in_H2O.h5` comes from) | 8 | 0.70694 +- 0.00023 | **+42 +- 31** | **+1.36** |
| **D**: as B, ENDF-built S(a,b) on a 1536 x 256 emission grid | 8 | 0.70560 +- 0.00034 | -92 +- 39 | -2.34 |
| **C**: as B but **no S(a,b)** (H-1 free gas) | 8 | 0.72045 +- 0.00027 | +10 +- 43 against OpenMC no-S(a,b) (8 seeds, 0.72035 +- 0.00033) | +0.23 |

Differences between outram-mc arms: A - B = **+135 +- 35 pcm (z +3.80)**;
B - baseline = +23 +- 38 (z +0.60); D - B = -1 +- 43 (z -0.03); A - D = +134
+- 41 (z +3.27).

#### Hypotheses, predicted against measured

| id | prediction (written before the run) | measured | verdict |
|---|---|---|---|
| H1b | a >= 0.3 % band in sigma_inel if the cross sections differ; < 0.1 % if not | the tables agree to **worst 1.3e-4 relative** (at 0.417 eV) over 291 points; Maxwellian-weighted integral differs by -3.4e-5. Cutoff 10 eV on both, temperature 293.600 / 293.594 K. **But** the sampled outgoing-energy kernel differs: mean outgoing energy of `sample_inelastic` (2e6 samples, statistical error << the differences) is 0.0814 vs 0.0823 eV at 0.1 eV incident (+1.1 %), 0.8273 vs 0.8220 at 1.5 eV (-0.6 %), 1.7858 vs 1.8259 at 3.5 eV (+2.2 %), 0.03021 vs 0.02994 at 0.025 eV (-0.9 %); identical to 1e-4 at the incident energies that are nodes of both grids (0.01, 0.05, 0.3, 0.625 eV) | cross-section half of the hypothesis **refuted**; a kernel difference found that I had not predicted |
| H2 | 0 +- 50 pcm | **+23 +- 38 pcm** (z +0.60; B minus baseline) | **held**: the neutron data (RECONR/BROADR vs NJOY ACE, B-10 included) are not the cause |
| H1 | +60 .. +130 pcm | **+135 +- 35 pcm** (A minus B), and A against OpenMC +42 +- 31 (z +1.36, passes the gate) | **held** on sign and size (just above the top of the range). Swapping only the S(a,b) for the NJOY table removes the miss |
| H3 | the outram-minus-OpenMC difference changes by > 100 pcm; I also predicted k falls by *thousands* of pcm in both codes without S(a,b) | difference +10 +- 43 pcm (z +0.23), i.e. changed by +127 pcm from -117, **gap gone**. **The k prediction was wrong**: without S(a,b) both codes give **higher** k (outram-mc 0.72045, OpenMC 0.72035), +1400 pcm, not thousands lower, because here f = 0.40 is set by boron and hydrogen absorption and free-gas H-1 leaves a harder, less absorbed spectrum | **held** for the difference; the k-direction guess **missed** |
| H4 | new independent seeds give about -40 +- 50 pcm | OpenMC seeds 17-32 reproduce the OpenMC mean (-2 +- 41 pcm vs seeds 1-16); the outram-mc side with other seeds and other data routes (B: seeds 1-8, D: seeds 1-8, both with the h5 neutron data) gives -93 and -92 pcm against 32 OpenMC seeds. A fresh ENDF-route outram-mc ensemble was **not** run | **fluke not supported**: the deficit reproduces on the OpenMC side and on two further outram-mc arms. (I did not re-run the ENDF-route baseline itself with new seeds.) |
| H5 | f differs by about -0.3 % relative, p/eps unchanged | OpenMC tallies (seeds 17-32, thermal < 0.625 eV, resonance to 0.1 MeV, fast): eta 1.78012 +- 0.00001, f 0.40436 +- 0.00007, p 0.89135 +- 0.00006, eps 1.10036 +- 0.00006 (product 0.70599) against outram-mc baseline eta 1.78003, f 0.40412 +- 0.00009, p 0.89131, eps 1.10041. **f is the only factor that moves, by -0.06 % relative (2.2 sigma)**; eta, p and eps agree to 5e-5. Thermal absorption shares in OpenMC: B-10 0.4393, H-1 0.1513, O-16 0.00004 of all thermal absorption. Not run on the A arm | **direction held, size missed** (-0.06 %, not -0.3 %): the difference lives in the thermal utilisation, none in the resonance or fast factors |
| H6 | +60 pcm (range -50 .. +150) | **-1 +- 43 pcm** (D minus B) | **refuted**: refining the ENDF-built table's incident-energy grid by 4x and its outgoing-energy bins by 4x changes k by nothing resolvable, so the grid density is not the cause. The single-energy comparison above also showed that the 384 x 64 and 1536 x 256 tables differ from each other (0.1 eV: 0.0814 vs 0.0844; 1.5 eV: 0.8273 vs 0.8472) by as much as either differs from NJOY's, so the first moment at a single energy is not a good predictor of k |

#### Cause: where it was found, and what is not shown

- **Localised, with the data identical on every other axis:** the 2.40 cm miss
  is carried by the **H in H2O S(a,b) table**. With the neutron data taken
  from the very same `.h5` OpenMC reads, replacing only this crate's ENDF-built
  table by the NJOY2016 ACER table moves k by +135 +- 35 pcm and closes the gap
  to OpenMC (+42 +- 31 pcm, z +1.36). Removing the S(a,b) from both codes
  removes the gap (+10 +- 43 pcm). The neutron data, the free-gas and
  resonance-scattering treatment, and the S(a,b) cross sections (1.3e-4) are
  not the cause.
- **What it is NOT, ruled out:** statistics of the first ensemble (OpenMC
  reproduces; two more outram-mc arms sit at -93 and -92 pcm); the neutron
  data pipeline (+23 +- 38); the S(a,b) cross sections; the emission-grid
  density of the ENDF-built table (-1 +- 43); resonance/fast factors (p, eps
  agree to 5e-5).
- ~~**What is not shown:** *which* feature of the ENDF-built outgoing-energy
  kernel differs.~~ **CORRECTED 2026-11-19 (gh:#550, `sab_reference_550.md`):**
  it is the incident-energy labelling of the emission tables (`emission`
  returns the nearest `calcem` record, so the table labelled E carries a node
  up to 5 % away: a staircase of up to 2.3 % in `<E'>`); NJOY's table is the
  one within 1.9e-4 of a direct integral of the tape, and tables on THERMR's
  own grid close the 2.40 cm gap (E - OpenMC -1 +- 44 pcm). Also the claims
  below that the density of the grid "is not the cause" and that neither
  table is closer to the evaluation are superseded there. Original text:
  *which* feature of the ENDF-built outgoing-energy
  kernel differs. Both tables use 64 outgoing-energy bins, the same discrete
  sampling scheme and the same cross sections to 1e-4, and the difference does
  not shrink on a 4x finer grid, so it is not interpolation resolution. The
  remaining candidates are the content of the kernel itself (this crate's
  `IncoherentInelasticScattering::emission` against NJOY `calcem` + `acer`
  thermal's equiprobable-bin construction, including the 8 against 16 cosines
  per bin), and I did not isolate it. **Nor is it shown which table is closer
  to the evaluation:** the NJOY table is not an oracle of the ENDF law, it is
  another processing of it. The reference for that is the evaluation's own
  S(alpha,beta), which this study did not integrate independently. Until it is
  done, the honest statement is "the two S(a,b) tables give k that differ by
  +135 +- 35 pcm at 2.40 cm; the outram-mc-minus-OpenMC miss is that
  difference", not "outram-mc is wrong". At 1.63576 and 1.75 cm, where f is
  larger and the thermal spectrum matters less, the same difference is below
  the resolution (-37 +- 38, -1 +- 37 pcm).
- **Reviewed functions implicated: none edited, none identified as the
  cause.** No source under `src/` was changed. `src/material/review.md` holds
  no review block targeting a thermal function. The code that builds the table
  lives in `njoy-outram-park-fork`'s THERMR port, which I did not check for
  review stamps and did not touch. Any change there, for example to the
  emission-table construction, needs the kovan-reviewed check and the
  maintainer's confirmation first.
- **Recommended next step (not done):** compare the two tables' *emission
  kernels* against an independent integral of the ENDF S(alpha,beta) at a
  handful of incident energies (0.1, 0.3, 1.5, 3.5 eV): mean and variance of
  E' and the E'-mu correlation, with the tape-built table, the NJOY ACE table
  and the direct integral side by side, then decide which is to be changed.
  Also the same A/B at the other eleven pitches, to see whether the effect
  follows f.

#### Instruments and files

- `examples/sab_endf_vs_ace_550.rs`: the table comparison (H1b and the
  moments), run as `sab_endf_vs_ace_550 .../293.6K/HinH2O.ace`.
- `examples/lct008_pitch_sweep.rs`: new options `--sab-ace FILE`, `--no-sab`,
  `--sab-grids N_EMIT,N_OUT`, all off by default (the default path and every
  recorded number are unchanged).
- `openmc_inputs/lct008_pin_cell_openmc_550.py`: diagnostic copy of the
  OpenMC deck with `--no-sab` and `--tallies`; the original deck is unchanged.
  Without the flags it is the same model.
- Timings (indicative only, shared VM): outram-mc transport 320-490 s per
  seed with S(a,b) at load average ~9, 190 s when the machine was quiet;
  OpenMC 75-115 s per seed on 2 threads. Data loading from the `.h5` plus S(a,b): 35-88 s per process (the logs print 55, 55, 88 and 35 s for arms A-D under varying load), so it is not a useful timing.


## CORRECTED 2026-11-19 (gh:#550): the default S(a,b) build changed

The ensemble and sweep numbers above were taken with the log-spaced emission
build. `ThermalScattering::from_tape` now puts its tables on THERMR's own
incident-energy grid. Re-measured on the tape-data route (8 seeds, same
protocol): 2.40 cm 0.70568 +- 0.00024 (was 0.70536, +32 +- 36 pcm), 1.63576 cm
1.06323 +- 0.00040 (was 1.06216, +107 +- 50), 1.75 cm 1.01168 +- 0.00033 (was
1.01104, +64 +- 37). The 2.40 cm gap to OpenMC is now -85 +- 38 pcm (z -2.26),
~~-117 +- 39 (z -2.97)~~. Record: `sab_reference_550.md`, "Flip and re-measure".
