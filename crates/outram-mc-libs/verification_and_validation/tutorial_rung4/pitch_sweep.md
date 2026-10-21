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
- **Neither code is the reference.** A seed ensemble at the three 2σ points
  would settle it, and was not run. For scale, on the full LCT-008 lattice
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
