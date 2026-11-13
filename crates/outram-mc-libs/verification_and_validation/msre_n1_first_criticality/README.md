# MSRE N-1: first criticality, U-235 (gh:#1128)

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI draft,
> unreviewed. **Validation** against a measurement, on a model with stated
> simplifications.

## Numbers to quote (summary, 2026-11-12)

The default model is **arm C (natural Mo), RVS, 30P graphite, cold
dimensions, no shield, rods out, and since 2026-11-12 U-234/U-236 in the
salt** (TM-0730's 93 % uranium; maintainer: "More physical model is
better"). σ is the larger of the seed sem and the internal σ/√n. Every row
links to the section that holds its method, data and caveats. This summary
adds no number of its own.

| quantity | this model | reference | section |
|---|---|---|---|
| N-1 k at 1181 °F (6 seeds) | 1.00536 ± 0.00023, **+455 ± 23 pcm** (~~1.00882, +801 ± 22~~ before U-234) | k_ref 1.00080 (ORNL-4233); measurement ±200–270 pcm | [re-measured](#re-measured-on-the-2026-11-12-default-salt-with-u-234-and-u-236-gh1128-gh1129) |
| N-1 against the 1000 pcm band (maintainer, 2026-11-10) | **passes** | — | [Methodology](#methodology) |
| ²³⁵U concentration coefficient | **0.235 ± 0.006** (~~0.231 ± 0.005~~) | measured 0.223; ORNL calc. 0.234–0.248 | re-measured |
| equivalent ²³⁵U excess behind +455 pcm | about 0.63 g/L (1.9 %) | measured 32.8 ± 0.3 g/L | same |
| α_iso (×10⁻⁵ /°F) | **−7.69 ± 0.35** (~~−7.54 ± 0.40~~) | measured −7.24 | same |
| fuel coefficient | **−4.18 ± 0.35** (~~−4.44 ± 0.40~~) | measured −4.7 ± 0.7 (found after the arm-C run) | same |
| graphite coefficient | **−3.08 ± 0.35** (~~−2.82 ± 0.43~~) | not measured | same |
| T-1 natural periods, with this split | **−11 % to +12 %** of TM-1647 (~~−11 % to +8 %~~) | — | nee_soon T-1 README |

**Ablations.** None of these is in the default; each is the maintainer's
call. ~~Measured on the previous default salt (without U-234/U-236); the
shifts are differences and were not re-run. The last column adds each
shift to the current +455 pcm, an estimate, not a run.~~ **RE-MEASURED
2026-11-13 on the current default (U-234/U-236 salt, arm C, RVS)**, three
seeds per arm (six for hot dimensions), each against the default's
1.00536 ± 0.00023. The shift's σ is the arm's (the larger of seed sem and
internal σ/√n) combined in quadrature with the default's. The last column
is now the arm's own N-1, a run, not an estimate. Every shift fell inside
the range posted on gh:#1128 before the runs.

| change | effect on N-1 k | predicted (posted) | N-1 against the band |
|---|---|---|---|
| hot dimensions | **+224 ± 32 pcm** (6 seeds; ~~+193 ± 23, 12 seeds paired, old salt~~) | +120 to +270 | **+679 ± 22, passes** (~~about +648~~) |
| 10P graphite law | **−52 ± 48 pcm** (~~−71 ± 32~~) | −150 to +10 | **+404 ± 42, passes** |
| crystalline graphite law | −70 ± 35 pcm (old salt, not re-run) | — | passes |
| TM-0730 end regions | **+36 ± 46 pcm** (~~+20 ± 45~~) | −70 to +110 | **+491 ± 40, passes** |
| thermal shield cylinder, stated (`--shield tm0728`) | **+818 ± 52 pcm** (~~+779 ± 43~~) | +690 to +870 | **+1273 ± 47, fails** (~~about +1234~~) |
| thermal shield cylinder, close-packed | +895 ± 66 pcm (old salt, not re-run) | — | about +1350, fails |
| **whole thermal shield, TM-0728 Fig. 5.2** (measured on the current default) | **+1063 ± 39 pcm** | +850 to +1200 | **+1519 ± 31, fails** |
| 99.9926 % Li-7 | **+714 ± 39 pcm** (~~+730 ± 60~~) | +600 to +860 | **+1169 ± 32, fails** (~~about +1185~~) |
| INOR-8's Mo omitted (arm A) | **+418 ± 39 pcm** (~~+441 ± 45, under DBRC~~) | +350 to +530 | **+874 ± 32, passes** (~~about +896~~) |
| ~~U-234/U-236 per TM-0730's 93 % uranium~~ now the default; `--minor-uranium none` removes them | −326 ± 83 pcm | — | (the default) |

Every re-measured shift is within 1σ of its old-salt value, as expected
for a difference: the U-234/U-236 change does not move them. Hardware:
llvmpipe / Vulkan, 4 cores, Intel Xeon, 15.7 GiB RAM, 4 threads, about
13 min a seed plus 145 s of data loading. Data: `data/keff.csv`, labels
`+u234+<ablation>` (Mo omitted: `mo-omitted@…+u234`).

**Open, waiting on the maintainer:**
- the rod worth, whose geometry is built but needs the Gd tapes;
- the defaults in the ablation table above.

## Methodology

**Reference: ORNL-4233** (Prince et al. 1968), §2, Table 1, p. 11.

- **State:** 1181 °F, pump off, rod poisoning 0.08 % δk/k.
- **Critical ²³⁵U concentration:** 32.8 ± 0.3 g/L (book, corrected for flush-salt dilution) or 32.4 ± 0.3 g/L (analytical).
- **Concentration coefficient:** (δk/k)/(δC/C) = 0.223, measured, p. 19. So ±0.3 g/L is worth ±200 pcm.
- **Pass criterion:** ~~none is set. This is a validation, and the finding is the disagreement.~~ **SET 2026-11-10 by the maintainer (gh:#1128): |Δρ| ≤ 1000 pcm is the acceptable band for a full-core model.** The maintainer set it after the results below were known. So it judges these numbers and does not pre-register them, and it was not chosen by fitting any input. Against it: *(the list below is the record at the time the band was set, on the old default; the current numbers are the ablation table at the top, re-measured 2026-11-13: the same three ablations fail)*
  - the default model (arm C, RVS, +801 ± 22 pcm) **passes**;
  - these pass:
    - hot dimensions, +959 ± 45 pcm, with 41 pcm of margin;
    - the 10P and crystalline graphite laws, +689 and +701 pcm;
    - TM-0730's end regions, about +821 pcm.
  - these **fail**:
    - the TM-0728 thermal shield, about +1581 pcm;
    - TM-0730's 99.9926 % Li-7, +1530 pcm;
    - arm A, which omits the Mo, +1197 pcm.
  
  The ablations that make the model more like the real reactor raise k, and two of them take it out of the band.
  
  The disagreement is still the finding. The band does not make +801 pcm an agreement: it is about 3–4× the measurement's ±200–270 pcm.
- **Quantity reported:** `Δρ = (k − k_ref)/k_ref` in pcm, with `k_ref = 1/(1 − 0.0008)`. The model is rods-out at 32.78 g/L.

**Model:** `outram_mc_libs::vv::msre_n1`. Every dimension and every
simplification is cited in that module's doc and was posted on gh:#1128
before any run. The headline simplifications:

- the heads are volume-equivalent salt plenums (TM-0728 Table 5.4 volumes);
- the ~5 ft³ of end-region salt is omitted (tested 2026-11-10: ORNL's own
  end layers, `--ends tm0730`, move k by +20 ± 45 pcm; see below);
- there is no thermal shield (tested 2026-11-10: its cylinder, `--shield
  tm0728`, moves k by +779 ± 43 pcm; see below);
- cold dimensions;
- the opening diameter is read from Fig. 5.7 by scale;
- the sample baskets are homogenised.

**Composition scripts:**

- `scripts/salt.py`: the salt from ORNL-4233's inventory. It reproduces ORNL's 1.408 wt % and 32.78 g/L.
- `scripts/inor8.py`: INOR-8 from TM-0728 Table 2.2.

~~**Molybdenum data gap:** INOR-8 is 16.5 wt % Mo, and only a beta ⁹⁵Mo tape is
in `reference-data/endf/`. Two arms bound the effect:~~ **CLOSED 2026-11-09:**
the seven stable Mo tapes (ENDF/B-VIII.0 release) are in `reference-data/ace`
(`ba58c2e`), and arm C carries INOR-8's Mo as specified. **Arm C is the model;
A and B are kept as ablations.** INOR-8 is 16.5 wt % Mo:

| arm | INOR-8 Mo |
|---|---|
| A, `--mo omitted` | left out |
| B, `--mo mo95` | beta ⁹⁵Mo tape at the thermal absorption of natural Mo (σ_a 2.48 / 13.4 b, quoted from memory) |
| **C, `--mo natural`** | natural Mo, ⁹²⁻¹⁰⁰Mo at IUPAC 2013 abundances, same element density (`scripts/inor8.py`) |

**Reading the `arm` labels in `data/` (added 2026-11-12).** The example
writes no resonance-scattering tag. The **`+rvs`** in some labels was added by
hand when those rows were recorded, on 2026-11-09/10, to tell them from the
DBRC runs before them. Since then the binary's untagged rows are RVS. So:

- **DBRC:** `mo-natural@salt1181F/graphite1181F` with no further suffix
  (seeds 1–6, some duplicated), every `mo-omitted@…` row without `+rvs`,
  `mo95-equivalent…`, and the `msre_n1_salt…` rows in `n3.csv`.
- **RVS:** every `+rvs` row, and every other untagged `mo-natural@…` row:
  - `+ends-tm0730`, `+shield-*`, `+u235x*`;
  - `+hot-dims` seeds 4–6 in `keff.csv`;
  - every `mo-natural@…` row in `n3.csv`.

  Each of these is stated as RVS in its own section.
- ~~The 6-seed default of 1.00882~~ The previous default's 6-seed 1.00882
  is the `mo-natural+rvs@salt1181F/graphite1181F` seeds 1–6. Seeds 1–3
  appear twice, with identical values.
- **U-234 and U-236 in the salt:** `+minorU-tm0730` marks the three-seed
  sensitivity run (pre-#1386 binary, when they were an option), and
  **`+u234`** every run on the 2026-11-12 default (merged binary), which
  the example now writes itself. Both are RVS. The 2026-11-12 default's N-1
  is the six `mo-natural@salt1181F/graphite1181F+u234` seeds.

**Data and code:**

- **Route 4:** `examples/msre_n1_first_criticality.rs`, reading ENDF/B-VIII.0 at 911.5 K.
- **Default physics:** BROADR, URR and DBRC as `from_endf_file_with_speed` defaults them.
- **Graphite:** reactor graphite 30P S(α,β) regenerated by LEAPR at 911.5 K on C-12 and C-13.
- **Statistics:** 20 000 histories × [100 inactive + 200 active] per seed; seeds pooled.

## Geometry, drawn from the assembled CSG

`cargo run --release -p outram-mc-libs --example msre_n1_first_criticality -- --draw <dir>`

| image | what it shows, and what was checked |
|---|---|
| `images/msre_n1_xy_core.png` | The whole core at mid-height. The stringer lattice is clipped to 55¼ in; then the salt gap, the ¼ in core can, the downcomer and the vessel. Checked: a stringer sits on the axis, the lattice reaches the can on every side, and the rings are in Table 5.2's order. |
| `images/msre_n1_xy_centre.png` | The centre. Three 2 in INOR-8 thimbles (void inside) in salt-filled 2.5 in openings at (±2, ±2) in, and the homogenised basket at (+2, −2) in, as Fig. 5.7 places them. The openings cut the neighbouring passages, as in the figure. |
| `images/msre_n1_xy_stringer.png` | One stringer: four half-stadium passages. |
| `images/msre_n1_xz.png` | An x-z section at y = +2 in, through two thimbles. 64 in of graphite, a 16.6 cm lower plenum (10.0 ft³), a 17.4 cm upper plenum (10.5 ft³), 1 in heads. The thimbles run from the core bottom through the upper plenum. |
| `images/msre_n1_xz_top.png` | Close-up of the top of the core: the thimbles entering the upper plenum, the can, the downcomer and the vessel. |

**Not checkable from these images:**

- the stringer count against TM-0728's 513 + 104, which the lattice does not reproduce (see gh:#1128);
- the 2.5 in opening diameter, which comes from the figure's scale.

**Measured on the assembled geometry** (`vv::msre_n1` tests, 2026-11-09):

| quantity | measured | source |
|---|---|---|
| stringer fuel fraction | 0.22284 | analytic 0.22283; TM-0728 Table 5.4: 0.224 |
| graphite volume | 68.60 ft³ | Table 5.4: 69 |
| plenum volumes | 10.0 and 10.5 ft³ | Table 5.4 (exact by construction) |

## Results

**Hardware for every run here:** Intel Xeon (4 logical cores, all four
threads used), 15.7 GiB RAM, Linux, CPU only, in a cloud container shared
with other jobs. Nuclear data loading takes 170 s (28 nuclides in
parallel); transport takes 770-1290 s per seed.

### N-1 (2026-11-09)

**k_ref = 1/(1 − 0.0008) = 1.00080.** σ comes from the seed spread.

| arm | seeds | k | vs k_ref |
|---|---|---|---|
| A, Mo omitted | 5 | **1.01278 ± 0.00028** | **+1197 ± 28 pcm** |
| B, Mo as Mo-95 equivalent | 1 | 1.00869 ± 0.00062 (internal) | +788 pcm |
| C, natural Mo, DBRC (before gh:#1311) | 6 | 1.00853 ± 0.00020 | +772 ± 20 pcm |
| **C, natural Mo, RVS (current default)** | 6 | **1.00882 ± 0.00022** | **+801 ± 22 pcm** |

Both arms A and B are **inside the predictions** posted before the runs (A:
1.02 ± 0.02; B: 200–800 pcm below A). ~~The bounded worth of INOR-8's Mo is
about −410 pcm.~~ **Measured with natural Mo (arm C):** INOR-8's Mo is worth
**−441 ± 45 pcm** (C − A).

**Arm C** (seeds 1.00863, 1.00809, 1.00841; 2026-11-09): the three seeds
spread only 28 pcm, below the ≈63 pcm internal σ, which three seeds cannot
resolve, so the quoted ±36 pcm is the internal σ/√3, the larger of the two.
It is **inside the prediction** posted on gh:#1128 before the run
(+640 to +790 pcm, i.e. 0–150 pcm below B): C − B = −31 ± 72 pcm. (Three
seeds, DBRC; the six-seed DBRC value is +772 ± 20 pcm, still inside.) The
thermal-absorption stand-in therefore captured Mo's worth to within the
statistics; the extra resonance absorption estimated at about 20 % is not
resolved. Transport took 781–801 s per seed (machine otherwise idle).

**Re-measured under RVS (2026-11-09, develop `0ce531bf3`),** as the
correct-physics rule requires after a default change; six seeds on each
arm (rows `mo-natural+rvs` in `data/keff.csv`; DBRC seeds 4–6 from the
pre-merge binary). ~~After three seeds: RVS − DBRC = +78 ± 51 pcm (1.5σ),
flagged on gh:#1311 as a possible wrong-sign RVS worth.~~ **With six seeds:
RVS − DBRC = +29 ± 30 pcm (1.0σ)**, per seed +84, +43, +107, −20, −67, +27.
Consistent with zero and inside the prediction posted on gh:#1128
(|Δ| < 60 pcm); the three-seed excess was noise, and the flag is
withdrawn. **Quote +801 ± 22 pcm under the current default** (six seeds,
seed-spread σ; sd 53 pcm, below the ≈63 pcm internal σ).

**Resonance scattering in every other run on this page:** DBRC, except the
N-3 RVS rows and the entropy study. Every other N-1, N-3 and
convergence run here was made before the default changed to OpenMC's RVS
method (gh:#1311, 2026-11-09). DBRC itself is worth −19 ± 51 pcm on this
model (ablation below); ~~the RVS-vs-DBRC difference has not been measured on
it~~ the RVS-vs-DBRC difference has been measured on arm C only (above).

The remaining +801 pcm (arm C under RVS; ~~+757 pcm, arm C with DBRC, 3 seeds~~; ~~+800 pcm, arm B~~) is **not** a validated
code bias. The source-convergence check below allows that it reads up to
about 70 pcm low. It contains:

- the measurement's own ±0.3 g/L, which is ±200 pcm through the measured
  concentration coefficient of 0.223;
- the book-vs-analytical spread of 270 pcm;
- the model's simplifications: the heads, ~5 ft³ of end-region salt, no
  thermal shield, cold dimensions;
- ~~the Mo stand-in itself.~~ (closed by arm C)
- the gap by which route 4 sits above route 1 (OpenMC) on this model:
  +80 ± 44 pcm at 1181 °F (three seeds a side, resonance-scattering ablation
  below), up to 239 ± 21 pcm at 1231 °F; graphite law, URR and DBRC are ruled
  out, and the maintainer chose not to pursue it further (gh:#1311).

### N-1 residual: hot dimensions (2026-11-10, gh:#1128)

**Method.** `--hot-dims` (`vv::msre_n1::Dims`): every graphite length scales
by 1 + 1.5e-6 × (1181 − 70) and every INOR-8 length by 1 + 7.8e-6 × 1111
(TM-0728 §5.3: graphite "1.3 to 1.7 × 10⁻⁶", INOR-8 "7.8 × 10⁻⁶ in./in.-°F
(in the 70-1200°F range)"); head volumes scale with the INOR-8 cube, solid
densities thin by the cube of their scale, salt density is already at
temperature, the sample basket is unchanged. Drawn
(`images/*_hot.png`, sent in-session before the run). Pinned: the cold
build is byte-identical to the recorded geometry, and the can gap grows
0.49 cm against TM-0728's "3/16 in." Arm C (natural Mo), RVS, seeds 1–3.
Prediction posted on gh:#1128 before the run: k up by +50 to +300 pcm.

| geometry | seeds | k | vs k_ref |
|---|---|---|---|
| cold (as drawn) | 6 | 1.00882 ± 0.00022 | +801 ± 22 pcm |
| **hot (1181 °F)** | 3 (1.01103, 1.01062, 1.00954) | **1.01040 ± 0.00045** | **+959 ± 45 pcm** |

- ~~**Hot − cold = +158 ± 50 pcm (3.2σ)**, inside the prediction.~~ **With six seeds (2026-11-12, below): +161 ± 38 pcm**; the three-seed value stands within its σ.
- **The residual grows**, as the screen on gh:#1128 expected: the core
  grows 0.17 % each way (less leakage) and the can gap, downcomer and
  plenums gain salt.
- **Not adopted as the default.** Hot dimensions are the more physical
  state, but making them the default re-states every N-1 and N-3 number
  on this page; that is the maintainer's call. The N-3 coefficients
  ignore expansion over their ±50 °F swing (~~not measured~~ **measured 2026-11-11 as an ablation, N-3 section: −0.69 ± 0.54, not resolved**). If the +158 pcm over 1111 °F
  is linear, expansion adds about **+0.14 × 10⁻⁵ /°F** (14 pcm over the
  100 °F swing): about 2 % of α_iso, positive, and inside N-3's ±0.5–0.7
  σ. Not measured at N-3's temperatures.

### N-1 residual: graphite thermal-scattering law (2026-11-10, gh:#1128)

**Why.** Every record above uses ENDF/B-VIII.0's reactor graphite **30P**
law (30 % porosity). CGB is 1.87 g/cm³ with **17.7 %** total porosity
(TM-0728 Table 5.3: 7.9 % accessible, 9.8 % inaccessible), between the
tabulated laws. `--graphite-law 30p|10p|crystalline` regenerates the
chosen law by LEAPR at 911.5 K from the embedded ENDF/B-VIII.0 deck (the
default stays 30P). Arm C (natural Mo), RVS, cold, seeds 1–3; rows
`+graphite-*` in `data/keff.csv`.

| law | seeds | k | vs k_ref | vs 30P |
|---|---|---|---|---|
| 30P (default) | 6 | 1.00882 ± 0.00022 | +801 ± 22 pcm | — |
| 10P | 1.00761, 1.00754, 1.00796 | 1.00770 ± 0.00037 | +689 ± 37 pcm | **−112 ± 43** |
| crystalline | 1.00829, 1.00840, 1.00675 | 1.00781 ± 0.00053 | +701 ± 53 pcm | **−101 ± 58** |

(σ for the three-seed arms is the larger of the seed spread and the
internal σ/√3.)

- **Predictions, posted on gh:#1128 before each run:**
  - 10P: predicted 0 to +400 pcm against 30P. **Refuted: the sign was
    wrong.**
  - Crystalline, re-predicted after the 10P result: −100 to −400 pcm
    against 30P, and below 10P. The first part is met only at the edge;
    "below 10P" is **not** supported (crystalline − 10P = +11 ± 65).
- **Six seeds per law (2026-11-10),** seeds 4–6 added to 10P and
  crystalline: 10P 1.00811 ± 0.00023 (+730 pcm), crystalline
  1.00812 ± 0.00028 (+731 pcm). **10P − 30P = −71 ± 32 pcm (2.2σ),
  crystalline − 30P = −70 ± 35 (2.0σ), crystalline − 10P = +1 ± 36.**
  ~~The three-seed −112 ± 43 and −101 ± 58~~ are superseded: the shift is
  smaller, about −70 pcm, and still resolved at 2σ.
- **Reading:** 30P is the outlier. The two less porous laws agree with
  each other and sit ~~about 105~~ **about 70** pcm below it. A CGB law (17.7 %) would sit
  between 10P and 30P; interpolating linearly in porosity, an assumption,
  gives roughly ~~−70~~ **−45 pcm** from the default (six-seed values). This is the **first lever found
  that moves the residual toward zero**, and it covers only a small part
  of the +801 pcm.
- **The default is not changed:** no tabulated law is CGB's, and choosing
  one is the maintainer's call.

### N-1 residual: lithium-6 content (2026-11-10, gh:#1128)

**The sources disagree on the enrichment:**

| source | Li-7 | Li-6 |
|---|---|---|
| ORNL-TM-0728, text ("99.99% Li7 in both fuel and coolant salts") | 99.99 % | 100 ppm (**the model**, `scripts/salt.py`) |
| ORNL-TM-0730 Table 3.2 footnote (its calculation salts) | 99.9926 % | 74 ppm |
| ORNL-TM-0730, survey section | 99.997 % | 30 ppm |

None states the as-loaded value at first criticality.

**Run:** `--li7-atom-percent 99.9926`, which re-splits the salt's lithium
with the total unchanged (the sample basket keeps the default split). Arm
C, RVS, 30P, cold, seeds 1–3 (1.01618, 1.01705, 1.01512).

| Li-7 | k | vs k_ref | vs default |
|---|---|---|---|
| 99.99 % (default) | 1.00882 ± 0.00022 | +801 pcm | — |
| 99.9926 % | 1.01612 ± 0.00056 | +1530 pcm | **+730 ± 60 pcm (12σ)** |

- **Inside the prediction** posted on gh:#1128 (+200 to +800 pcm).
- **About 28 pcm per ppm of Li-6.** Across the sources' 30–100 ppm, the
  enrichment alone moves N-1 by up to about 2000 pcm. **This is the
  largest lever found on the residual, larger than every geometric
  simplification.** ORNL-4233's ±200 pcm budget on the measurement does
  not include it, because it is a model input.
- ~~**Read backwards** (the slope only, not a claim about the salt): the
  +801 pcm residual would close at about 129 ppm Li-6 (99.987 % Li-7).
  Whether the as-loaded salt's Li-6 was higher or lower than 100 ppm is
  the question this result raises. It needs the salt's isotopic assay,
  which none of the reports read so far gives.~~ **Settled the same day
  by the production report.** ORNL-4616 (J.H. Shaffer, *Preparation and
  Handling of Salt Mixtures for the MSRE*, 1971, PDF p. 6): "all lithium
  fluoride used in the MSRE production operation ... Its analyzed
  isotopic assay was at least 99.99% ⁷Li." So **Li-6 was at most
  100 ppm**. The model's 100 ppm is the upper bound, and any lower value
  raises k. **Lithium cannot close the residual; if anything, the
  residual is larger than +801 pcm.**
- **Hafnium, checked the same way:** ORNL-4616 (same page) names hafnium
  in zirconium and ⁶Li as the only major poison impurities. The ZrF₄ was
  "essentially 'hafnium free'" (the OCR of the limit is garbled; it reads
  as about 50 ppm Hf in Zr). At that level hafnium's thermal absorption
  is about 0.4 % of the model's Li-6 absorption, of order 1 pcm. It is
  not modelled and is not a candidate.
- **The default is not changed.** Choosing an enrichment is the
  maintainer's call, and the most-cited text value (99.99 %) stays.

### N-1 residual: ORNL's end regions (2026-11-10, gh:#1128)

**Model:** `Ends::Tm0730` (`--ends tm0730`). It replaces the
volume-equivalent salt plenums with the homogenised axial layers of
ORNL-TM-0730 Table 3.1, ORNL's 20-region R-Z model. Each layer spans the
vessel, and the thimbles pass through.

Layers above the graphite:

| layer | thickness | fuel / graphite / INOR-8 (vol %) |
|---|---|---|
| H | 0.94 in | 63.3 / 36.5 / 0.2 |
| G | 0.69 in | 94.6 / 5.4 / 0 |
| E | 1.25 in | 93.7 / 3.5 / 2.8 |

Above E, salt (D) fills to 10.33 in above the graphite.

Layers below the graphite:

| layer | thickness | fuel / graphite / INOR-8 (vol %) |
|---|---|---|
| O (support grid) | 1.41 in | 66.9 / 15.3 / 17.8 |
| P | 7.73 in | 90.8 / 0 / 9.2 |

- **Not represented:** Table 3.1's sub-regions at r < 3 in.
- **Anchoring:** the layers sit at this model's graphite faces. ORNL's
  graphite top is at 64.59 in; this model's is at 64 in.
- **Default unchanged:** the default build is pinned unchanged by
  `tm0730_ends_build_beside_the_default`.
- **Drawn from the assembled CSG:** `images/msre_n1_*_tm0730.png`.

**Prediction** (posted on gh:#1128 before the run): **−50 to −500 pcm**.
The reasoning was that INOR-8 at the core bottom, in high flux, would
outweigh the extra salt.

**Run:** arm C, RVS, 30P, cold, seeds 1–3: 1.008591, 1.008648 and
1.009806 (each ± 0.00064–0.00066). About 750 s per seed (4-core Intel
Xeon, 4 threads, 15.7 GiB RAM, Linux, CPU only). Seed 3 was killed by a
container restart and rerun alone; its random stream does not depend on
the other seeds.

| ends | k | vs k_ref |
|---|---|---|
| volume-equivalent (default, 6 seeds) | 1.00882 ± 0.00022 | +801 pcm |
| TM-0730 Table 3.1 (3 seeds) | 1.00902 ± 0.00040 | +821 pcm |
| **shift** | **+20 ± 45 pcm** | not resolved |

- **Outside the prediction, with the wrong sign.** The central value is
  +20, and the whole band (−50 to −500) lies more than 1.5σ below it. The
  INOR-8 at the core bottom does not dominate. The salt the end layers
  add (about 14 ft³ below and 15.8 ft³ above, against 10.0 and 10.5 in the
  default) offsets it to within the statistics.
- **The end regions are not where the residual sits.** Their effect is at
  most about ±100 pcm (2σ) against a +801 pcm residual.
- ~~An interim post on gh:#1128 from seeds 1–2 gave −20 ± 50 pcm~~:
  superseded by seed 3, the same comment thread.
- **The default is not changed.**

### N-1 residual: the thermal shield (2026-11-10, gh:#1128)

**Source:** ORNL-TM-0728 §5.3.10.1 (report p. 128). ~~I had recorded the
shield's dimensions as unsourced~~: they are in the report.

**What TM-0728 gives:**
- **Cylinder:** 151 in high, 125 in OD, 93 in ID. It is 1 in Type 304 plate
  around a 14 in annulus of 1 in carbon-steel balls in cooling water.
- **Overall:** 16 in thick, "50% of which is Fe and 50% is H2O" (p. 68 and
  §5.3.10.4).
- **Also present:** a 16 in top cover with a 22⅝ in hole, an 18 in base
  tank, and a 6 in insulation lining. Between the lining and the vessel is
  the 11 in furnace annulus.

**Model:** `Shield::Tm0728Cylinder` (`--shield tm0728`).
- The cylinder only: two 304 plates at nominal composition, and the ball
  annulus at 6/14 iron so the 16 in is half iron, in water.
- The shield is at 293.6 K, with H in H₂O S(α,β) from `tsl-HinH2O`.
- **Not modelled:** the base, the cover and the plugs. The insulation and the
  furnace annulus are void.
- **Assumed:** the cylinder is centred on the vessel, because its elevation is
  not given.
- Drawn in `images/msre_n1_shield_*.png`. The vessel inside is pinned as the
  default build cell for cell by `tm0728_shield_wraps_the_default_vessel`.

**Prediction** (posted on gh:#1128 before the run): **+100 to +600 pcm**.

**Run:** arm C, RVS, 30P, cold, seeds 1–3: 1.01683, 1.01613, 1.01689 (each
± 0.00063). About 750 s per seed (4-core Intel Xeon, 4 threads, 15.7 GiB RAM,
Linux, CPU only).

| model | k | vs k_ref |
|---|---|---|
| vacuum outside the vessel (default, 6 seeds) | 1.00882 ± 0.00022 | +801 pcm |
| thermal-shield cylinder (3 seeds) | 1.01661 ± 0.00037 | +1581 pcm |
| **shift** | **+779 ± 43 pcm (18σ)** | |

- **Outside the prediction, above it.** The per-seed shifts (+735, +761,
  +740 pcm) agree with each other.
- **This is the largest geometric lever on N-1 found so far, and it moves
  the residual away from zero.** With the shield, the model sits about
  +1580 pcm above the measurement.
- **ORNL's own design model did not have it either.** TM-0730 Table 3.1's
  twenty-region R-Z model ends at the vessel side (region B, r = 29.56 in).
  So the vacuum boundary is not a simplification peculiar to this model.
- **What the number rests on, untested:**
  - the water and steel fractions (TM-0728's 50/50, against about 2/3 steel
    implied by its 70 tons of balls);
  - the insulation, modelled as void;
  - the cylinder's elevation;
  - the missing base and cover, which would add to the worth, not subtract.
- **The default is not changed.** Whether the comparison should carry the
  shield is the maintainer's call. A real reactor had it, so the model
  without it is missing a positive term of this size.

### N-1 residual: the shield's steel against its water (2026-11-10, gh:#1128)

**Ablation:** `--shield tm0728-drained` (`shield_materials_drained`).
- The same cylinder and the same 304 plates.
- The ball annulus keeps its iron at 6/14 volume, but its water is removed
  (void). This splits the shield's +779 pcm between its steel and its water.
- **This is a decomposition, not a candidate model.** The real shield is
  water-filled.

**Prediction** (posted on gh:#1128 before the run): the drained shield's shift
against no shield is **+150 to +450 pcm**, so the water carries 330–630 pcm.
The post said a drained shift at or above the full +779 pcm would mean the
water is net-absorbing, and that I would record it as a miss.

**Run:** arm C, RVS, 30P, cold, seeds 1–3: 1.02103, 1.02311, 1.02057 (each
± 0.0006–0.0007). About 790 s per seed (4-core Intel Xeon, 4 threads,
15.7 GiB RAM, Linux, CPU only).

| model | k | vs k_ref | shift vs no shield |
|---|---|---|---|
| no shield (default, 6 seeds) | 1.00882 ± 0.00022 | +801 pcm | — |
| shield, water-filled (3 seeds) | 1.01661 ± 0.00037 | +1581 pcm | +779 ± 43 pcm |
| **shield, drained (3 seeds)** | **1.02157 ± 0.00078** | **+2075 pcm** | **+1275 ± 81 pcm** |

- **A miss, on the side the post named.** The drained shield is worth more
  than the full one. **The water is worth −496 ± 82 pcm (6σ)**, net
  absorbing, against a predicted +330 to +630.
- **What that says:** the shield's worth comes from its iron. Iron reflects
  fast and epithermal neutrons back into the core. The water moderates some
  of them and, I infer, captures more than it returns: H capture, and iron
  capture of what thermalises there. That mechanism has not been checked with
  a tally, and the measured fact is only the sign and size.
- **What I got wrong:** I treated the water as a thermalising reflector in
  front of the core. It is not in front of the core. It sits behind a 1 in
  304 plate and amid 6/14 iron, both strong thermal absorbers, with a void
  furnace annulus between it and the vessel.
- **Consequence for the untested inputs above:** the steel fraction is now
  the sensitive one. About 2/3 steel (from the 70 tons of balls) instead of
  1/2 would likely raise the shield's worth further.
- **Statistics:** the three seeds spread 0.00135 (sd), about twice their
  internal σ. The quoted σ is the seed sem, as everywhere in this README.
- **The default is not changed.**

### N-1 residual: the shield's steel fraction (2026-11-11, gh:#1128)

**Why:** the drained run showed the shield's worth is its iron. TM-0728
gives two steel fractions that disagree:
- **"50 % Fe / 50 % H₂O"** over the 16 in. This is the model's input, and it
  puts the balls at 6/14 = 0.43 of the annulus.
- **70 tons of balls (p. 128).** The annulus holds 11.86 m³, and iron is
  7870 kg/m³. That gives 0.68 of the annulus in short tons, 0.75 in metric
  and 0.76 in long. **No ton is consistent with the 50 %**, and every one is
  above random close packing of equal spheres, **0.64** (Scott & Kilgour
  1969).

**Sensitivity:** `--shield tm0728-packed`
(`shield_materials_packed`, `SHIELD_BALL_FRACTION_PACKED`). The annulus is at
0.64 balls and 0.36 water, which is 0.685 iron overall. The plates and the
geometry are unchanged. `shield_ball_zone_with_fraction` reproduces the
stated zone at 6/14 to 1e-12 (tested).

**Prediction** (posted on gh:#1128 before the run): a shift of **+900 to
+1300 pcm** against no shield.

**Run:** arm C, RVS, 30P, cold, seeds 1–3: 1.01858, 1.01818, 1.01654 (each
± 0.0006–0.0007). About 770 s per seed (4-core Intel Xeon, 4 threads,
15.7 GiB RAM, Linux, CPU only).

| shield | balls in annulus | k (3 seeds) | vs k_ref | shift vs no shield |
|---|---|---|---|---|
| none (default, 6 seeds) | — | 1.00882 ± 0.00022 | +801 pcm | — |
| stated 50/50 | 0.43 | 1.01661 ± 0.00037 | +1581 pcm | +779 ± 43 pcm |
| **close-packed** | **0.64** | **1.01777 ± 0.00062** | **+1695 pcm** | **+895 ± 66 pcm** |
| drained | 0.43, no water | 1.02157 ± 0.00078 | +2075 pcm | +1275 ± 81 pcm |

- **On the prediction's lower edge.** +895 sits 5 pcm below the band's
  +900, well inside 1σ. So the result neither confirms nor misses the band.
  I record it as at the edge, not inside.
- **Packed − stated = +115 ± 67 pcm (1.7σ), not resolved.** Raising the
  shield's iron from 0.50 to 0.685 moves k by about a tenth of the shield's
  worth or less. **The steel-fraction disagreement in TM-0728 does not
  change the conclusion:** the shield is worth +780 to +900 pcm either way.
- **Against the maintainer's 1000 pcm band:** with the shield the model is
  +1580 to +1700 pcm. It fails at either steel fraction.
- **Statistics:** the seeds spread 0.00108 (sd), about 1.7× their internal
  σ, as in the drained run. The quoted σ is the seed sem.
- **The default is not changed.**

### N-1 residual: the whole thermal shield, base, cylinder and cover (2026-11-12, gh:#1128)

**Why:** every shield run above (`tm0728-cylinder`, `-packed`, the drained
and steel/water splits) models the cylinder only, centred on the vessel,
with the vessel's top and bottom open to vacuum. TM-0728 §5.3.10 and
Fig. 5.2 also give an 18 in base under the vessel and a 16 in cover inside
the cylinder's top, and Fig. 5.2 places the vessel about 7 in below the
cylinder's centre. Fig. 5.2 was digitised into
`data/tm0728_fig5_2_reactor_cell.csv` (method and caveats in the `.md`
beside it): it is a dimensioned drawing, not a schematic, and every scaled
length carries about ±3 in.

**Ablation:** `--shield tm0728-full` (`Shield::Tm0728Full`,
`geometry_with_full_shield`, `full_shield_materials`):

- **Placement:** the vessel's mid-height 68.5 in above the cylinder's bottom
  (`FULL_SHIELD_VESSEL_MID_ABOVE_BOTTOM`; Fig. 5.2, ±3 in).
- **Base:** 18 in thick at the cylinder's OD, homogenised to 8.3 % 304,
  4.8 % carbon steel, the rest water.
- **Cover:** 16 in thick, inside the cylinder's top, 79¾ in OD with a
  22⅝ in hole; 22.1 % 304 (plates and twelve radial webs), 25 % carbon
  steel, the rest water.
- **Ball annulus:** 0.60 balls, from "70 tons (475 ft³)" in short tons
  (`SHIELD_BALL_FRACTION_TM0728_TONS`; the stated 50/50 and close-packed
  0.64 are the runs above), with the twelve 1 in webs at 3.5 %.
- **Left void:** the 6 in insulation and the furnace annulus, whose
  materials are not given.
- **Vacuum boundaries:** the shield's outer radius, the base's bottom and
  the cylinder's top.

Drawn from the assembled CSG and shown to the maintainer before the runs:
`images/msre_n1_fullshield_xz.png`, `…_xy_cover.png`, `…_xy_base.png`, with
TM-0728's own figure in `images/tm0728_fig5_2_vessel_in_shield.png`. A unit
test (`full_tm0728_shield_places_base_cylinder_and_cover_by_fig_5_2`) pins
the placement and each zone's extent.

**Prediction** (posted on gh:#1128 before the run): **+850 to +1200 pcm**
against no shield. The cylinder alone was worth +779 ± 43 (stated) to
+895 ± 66 (packed); the base and cover were expected to add reflection at
the vessel's ends.

**Run:** on the 2026-11-12 default (U-234/U-236, merged binary), arm C,
RVS, 30P, cold, seeds 1–3: 1.01630, 1.01583, 1.01584 (each ± 0.0005–0.0006).
About 890 s per seed (4-core Intel Xeon, 4 threads, 15.7 GiB RAM, Linux,
CPU only). Rows `…+u234+shield-tm0728-full` in `data/keff.csv`.

| shield | k | vs k_ref | shift vs no shield |
|---|---|---|---|
| none (2026-11-12 default, 6 seeds) | 1.00536 ± 0.00023 | +455 pcm | — |
| **whole shield, Fig. 5.2** (3 seeds) | **1.01599 ± 0.00031** | **+1519 pcm** | **+1063 ± 39 pcm** |
| cylinder only, stated 50/50 (previous default salt) | — | — | +779 ± 43 pcm |
| cylinder only, close-packed (previous default salt) | — | — | +895 ± 66 pcm |

- **Inside the prediction**, in its upper half.
- **The base and cover add roughly +170 to +280 pcm** over the cylinder-only
  runs. That comparison crosses the salt change, so it is an estimate: the
  shield's worth is a difference, which the U-234 change should move far
  less than these σ, but the cylinder-only runs were not repeated.
- **Against the maintainer's 1000 pcm band: fails**, at +1519 ± 31 pcm. The
  more completely the shield is modelled, the further N-1 moves from the
  measurement. The finding is the disagreement; nothing was adjusted.
- **Statistics:** the three seeds spread 27 pcm (sd), below their ≈54 pcm
  internal σ; the quoted σ is the internal σ/√3.
- **The default is not changed.** It is the maintainer's call. If the shield
  is adopted, the residual it exposes, about +1500 pcm, is what the model
  then has to explain; the neutron balance above already locates the
  missing absorption in the salt (Zr and F), not in leakage.

### N-1: the ²³⁵U concentration coefficient (2026-11-11, gh:#1128)

**Reference:** ORNL-4233 §3.4, p. 19. The measured (δk/k)/(δM/M) is
**0.223**, the slope of Fig. 7 during the capsule additions, "very nearly
independent of ²³⁵U mass". ORNL's own calculation gave 0.248 for the
approach to critical and 0.234 averaged over the excess additions. No
uncertainty is quoted for the measured value.

**Perturbation:** the one ORNL made, `--u235-scale X`
(`salt_with_enriching_salt`). It adds or removes enriching salt,
73LiF-27UF₄ with 93 wt % ²³⁵U (ORNL-4233 pp. 3–4), until the salt's ²³⁵U is
X times the default.
- The other 7 % of that uranium is taken as ²³⁸U; the report does not
  split it.
- Li is added in the salt's own isotopic ratio, and F balances U and Li.
- The salt volume is fixed: ±5 % of the ²³⁵U is about ±0.1 wt % of the salt.
- The sample basket keeps the default salt.
- A unit test pins each of these ratios, and that removal inverts addition.

**Estimator:** the central difference (k₊ − k₋)/(k₀ · 0.10), with k₀ the
6-seed default 1.00882. σ is each arm's seed sem in quadrature.

**Prediction** (posted on gh:#1128 before the run): **0.24 ± 0.02**
(window 0.22–0.26), nearer ORNL's calculation than the measurement.

**Run:** arm C, RVS, 30P, cold, no shield; seeds 1–3 at each of X = 0.95
and 1.05. About 730 s per seed (4-core Intel Xeon, 4 threads, 15.7 GiB RAM,
Linux, CPU only).

| ²³⁵U | seeds | k (3 seeds, sem) |
|---|---|---|
| × 0.95 | 0.99661, 0.99767, 0.99607 | 0.99678 ± 0.00047 |
| × 1.00 (default, 6 seeds) | — | 1.00882 ± 0.00022 |
| × 1.05 | 1.02011, 1.02045, 1.01964 | 1.02007 ± 0.00023 |

| quantity | value |
|---|---|
| **(δk/k)/(δM/M), central** | **0.231 ± 0.005** |
| same, logarithmic form ln(k₊/k₋)/ln(1.05/0.95) | 0.231 |
| upper half, (k₊ − k₀)/(k₀ · 0.05) | 0.223 ± 0.006 |
| lower half, (k₀ − k₋)/(k₀ · 0.05) | 0.239 ± 0.010 |

- **Inside the prediction.** 0.231 is in 0.22–0.26, though below the 0.24
  centre.
- **Against the measurement: +3.5 %, 1.5σ** of our statistics alone, with
  no σ on the measured 0.223. Our coefficient sits between the measurement
  and ORNL's own calculation (0.234–0.248), nearer the measurement.
- **The halves differ by 0.016 ± 0.012, not resolved.** Their sign (a
  smaller coefficient at higher loading) is what a thermal core should do,
  but this run cannot show it.
- **What it implies for the residual.** At 0.231, the default model's
  +801 pcm is the reactivity of **3.5 % too much ²³⁵U, about 1.14 g/L**
  against the measured 32.8 ± 0.3 g/L. Put the other way, the model would
  go critical (k = k_ref) at about 31.7 g/L.
- **Against the 1000 pcm band:** this is a coefficient, not a k, so the
  band does not apply. The two rows above at ±5 % are perturbations, not
  the model's state.
- **The default is not changed.**

### N-1: neutron balance against ORNL's design calculation (2026-11-12, gh:#1128)

**Question:** where does the +801 pcm sit? Is it leakage, structure, or
the salt?

**Reference:** ORNL-TM-0730 Table 3.6, the balance for fuel C, clean and
critical, per 10⁵ neutrons produced.
- It is a design calculation, not a measurement.
- Fuel C is ORNL's 35 %-enriched model salt; ours is the as-loaded salt, at
  about 31 % by atoms.
- So this comparison **locates** a difference. It validates nothing.

**Instrument:** `--balance-csv` (opt-in), a material-filter tally of
absorption and ν-fission, read per 10⁵ ν-fission. Leakage is
`10⁵/k − Σ absorption`, so it also carries the Be-9 (n,2n) gain. ORNL's
table, which closes at exactly 10⁵, folds that gain the same way.
- k is bit-identical with the tally on: seeds 1–3 reproduce
  `data/keff.csv`'s RVS rows to every digit.
- Arm C, RVS, 30P, cold, seeds 1–3, at about 750 s per seed (4-core Intel
  Xeon, 4 threads, 15.7 GiB RAM, Linux, CPU only).
- Per-seed sums are in `data/balance.csv`.

| per 10⁵ neutrons produced | this model (3 seeds) | ORNL fuel C | difference | predicted |
|---|---|---|---|---|
| salt (all nuclides) | 64,957 ± 19 | 65,826 | −869 | 63,000–67,000 |
| graphite | 1,244 ± 1 | 796 | +448 | 900–1,500 |
| INOR-8 | 8,304 ± 4 | 9,682 | −1,378 | 7,500–9,500 |
| sample baskets | 817 ± 2 | in ORNL's main-core columns | — | — |
| leakage, with (n,2n) gain | 23,771 ± 33 | 23,696 | +75 | 22,000–26,000 |
| **total losses (10⁵/k)** | **99,092 ± 32** | 100,000 | **−908** | — |

- **Every row is inside the prediction** posted on gh:#1128. A short
  smoke run had been seen before the posting, and the post said so.
- **Leakage agrees with ORNL's to 0.3 %.** The geometry's escape
  probability, with its plenums, downcomer and cold dimensions, is not where
  the excess is.
- **The 908 missing losses are absorption.** Of these:
  - the salt carries 869;
  - graphite, INOR-8 and the baskets together carry a net 113;
  - within that net 113, INOR-8 is 1,378 lower than ORNL's (561 lower even
    after adding all of our basket absorption to it), and graphite is 448
    higher.
- **Graphite reads 56 % higher than ORNL's.** That is the boron (0.8 ppm)
  and the 30P S(α,β), against ORNL's 1960s graphite cross section. It works
  against the excess.
- **The salt's deficit is the lead this gives.** Per neutron produced,
  U-235's own absorption is fixed by η. So a salt shortfall is a shortfall
  in its other absorbers: U-238, Li-6, F-19, Be-9 or Zr. Our salt has *more*
  U-238 per U-235 than fuel C (31 % against 35 % enriched), so U-238 should
  pull the other way. **This instrument cannot split the salt by nuclide**,
  because the tally has no nuclide filter. That split is the next measurement.
- **Not a validation, and the default is not changed.**

### N-1: the salt's absorption by nuclide (2026-11-12, gh:#1128)

**Question:** the balance above puts the excess in the salt. Which salt
nuclides carry it?

**Reference:** ORNL-TM-0730 Table 3.7 (p. 52, read from the rendered page,
because the OCR garbled the column heads), fuel C, per 10⁵ produced.

**Instrument:** `--salt-spectrum-csv` (opt-in). It tallies the salt's flux and
absorption on 20 000 logarithmic energy bins:
- 1 000 bins over 1e-5–1 eV;
- 17 000 over 1 eV–100 keV, 3 400 per decade, so a bin at U-238's 6.67 eV
  resonance is a fifth of its width;
- 2 000 over 100 keV–20 MeV.

It then folds that flux with each salt nuclide's σ_a at the bin's geometric
midpoint, at the salt temperature, infinitely dilute in the unresolved
range.
- **Self-check:** the folded total reproduces the directly tallied salt
  absorption to **4.8–5.2e-5** on all three seeds.
- That tallied absorption equals the balance run's to 1e-6, and k is
  identical.
- A first version with 100 000 bins was killed for memory at 14 GB, because
  the tally is copied per batch. This one peaked at 5.8 GB.
- Seeds 1–3, arm C, RVS, about 1 700 s per seed (4-core Intel Xeon, 4
  threads, 15.7 GiB RAM, Linux, CPU only). Sums are in `data/salt_split.csv`.

| per 10⁵ produced | this model | ORNL fuel C | difference | predicted |
|---|---|---|---|---|
| U-235 | 50,698 ± 3 | 51,329 | −631 | — |
| U-238 | 9,562 ± 18 | 8,967 | +595 | 9,000–10,000 |
| U-234 + U-236 | **0 (not in the model)** | 262 | −262 | not predicted |
| Li-6 | 1,821 | 1,056 | +765 | 1,500–2,100 |
| Li-7 | 881 | 599 | +282 | — |
| Be | 310 | 315 | −5 | 235–395 |
| **Zr** | **800 ± 1** | **1,850** | **−1,050** | 600–1,100 |
| **F**, (n,α) + (n,γ) | **889** | **1,449** | **−560** | 700–1,100 |

- **Every predicted row is inside its band.** A short smoke run had been
  seen before the prediction was posted, and the post said so.
- **Zr and F carry the deficit.** Our model has 1,610 fewer absorptions
  per 10⁵ produced in them than ORNL's. Li-6, Li-7 and U-238 go the other way:
  - Li-6 is +765, partly because our 100 ppm is above fuel C's 74 ppm;
  - U-238 is +595, because our salt has more U-238 per U-235.
- **These are nuclear-data or composition differences, and the comparison
  cannot say whose are right.** ORNL's are 1960s cross sections. Ours are
  ENDF/B-VIII.0. A Zr deficit of a factor 2.3 at a near-equal Zr/Li ratio is
  a cross-section question. The next step is to read both evaluations'
  thermal and resonance Zr and F absorption against independent
  measurements.
- **Found on the way: U-234 and U-236 are missing from our salt.** ORNL's
  table carries 262 absorptions in them. `scripts/salt.py` counts the
  enriched lot's 7 % non-²³⁵U as U-238. TM-0730 Table 3.2 (p. 21) assumes
  the 93 % uranium to be **1 / 93 / 1 / 5 atom %** U-234 / U-235 / U-236 /
  U-238. That is a design assumption, not an assay of the as-loaded lot.
  ~~A sensitivity run with that composition is the next measurement.~~
  **Measured the same day:** see the next section.
- **Not a validation, and the default is not changed.**

**Follow-up: is the Zr and F gap composition or cross section? (2026-11-12, gh:#1128).**
Fuel C is TM-0730's design salt (Table 3.2, p. 21): LiF-BeF₂-ZrF₄-UF₄
65 / 29.2 / 5 / 0.8 mol %, uranium 0.3 / 35 / 0.3 / 64.4 atom % U-234 / 235
/ 236 / 238. Ours is the as-loaded salt of `scripts/salt.py`, 31.3 % U-235.
Dividing each nuclide's absorptions by U-235's removes most of the flux
level; dividing again by the atom ratio removes the composition:

| nuclide | atoms per U-235, ours / fuel C | absorptions per U-235 absorption, ORNL / ours | per-atom, ORNL / ours |
|---|---|---|---|
| Zr | 19.6 / 17.9 | 2.28 | **2.50** |
| F | 591 / 524 | 1.61 | **1.82** |
| Be | 118 / 104 | 1.00 | 1.14 |
| Li-7 | 264 / 232 | 0.67 | 0.76 |

- **Composition does not explain the gap.** Per U-235 atom our salt has 10–14 %
  *more* Zr, F, Be and Li-7 than fuel C. So the per-atom gap is larger
  than the raw one: ORNL's Zr absorbs 2.5 times, and its F 1.8 times, what
  ours does, relative to U-235.
- **Caveat on the instrument.** The last column assumes both calculations
  give U-235 the same spectrum-averaged cross section. A harder or softer
  spectrum moves every row together; it cannot make Zr 2.5 and Li-7 0.76.
- **Where TM-0730's cross sections came from** (§3.4, pp. 20–23; refs. 2, 3,
  10, 11): BNL-325 (2nd ed., 1958) for the early GNU runs; the GAM-1 library
  for 32 fast groups, which **lacked Li-6, Li-7 and F-19**, so their fast
  cross sections "were compiled from basic cross-section data" and their
  slowing-down was simulated with oxygen; THERMOS and a Wilkins heavy-gas
  model for the thermal group. TM-0730 gives no Zr or F values and does not
  mention hafnium.
- **Not resolved, and not guessed.** BNL-325 and ORNL CF-60-3-35 (Nestor's
  multigroup set) are not in the open corpus, so the 1960s Zr and F values
  cannot be read against ENDF/B-VIII.0 here. The pattern is a large gap in
  Zr and F, a small one in Be, and the opposite sign in Li-7, whose
  absorption is 1/v and so thermal. That suggests the epithermal and fast
  data rather than the thermal. **This is a hypothesis.** ~~The salt-spectrum
  instrument records totals per nuclide, not per energy band, so this page
  has not measured how much of our Zr and F absorption is epithermal. An
  energy-banded fold of the existing tally would test that cheaply.~~
  **MEASURED 2026-11-13**, below: most of our Zr and F absorption is above
  thermal, and Li-7's is not. **The maintainer supplies the literature**; this is
  recorded as a request on gh:#1128.

**Energy bands of the salt absorption (2026-11-13, gh:#1128).**

- **Method.** `--salt-spectrum-csv` now splits each salt nuclide's
  absorption, folded from the salt's group flux tally as before, into
  thermal (< 0.625 eV), epithermal (0.625 eV – 100 keV) and fast
  (> 100 keV). Each group goes whole to the band that holds its lower edge,
  with the cross section taken at the group's geometric midpoint, as the
  totals always were. The three bands sum to the old total exactly, by
  construction, so that sum checks the bookkeeping and not the physics. Default model, seeds 1–3: k = 1.00498,
  1.00600, 1.00594 (mean 1.00564, +28 pcm against the default's 1.00536,
  within σ). About 37–41 min a seed with the tally on, same hardware.
- **Prediction** (posted on gh:#1128 before the run), the share above
  0.625 eV: Zr 40–70 %, F 60–90 %, Be 30–70 %, Li-7 < 10 %, U-235 15–35 %.

| element | thermal | epithermal | fast | above thermal | predicted | |
|---|---|---|---|---|---|---|
| Zr | 0.293 | 0.672 | 0.035 | **70.7 %** | 40–70 % | just outside |
| F | 0.406 | 0.163 | 0.431 | **59.4 %** | 60–90 % | just outside |
| Be | 0.244 | 0.036 | 0.720 | **75.6 %** | 30–70 % | miss |
| Li-7 | 0.871 | 0.128 | 0.000 | **12.9 %** | < 10 % | miss |
| U-235 | 0.801 | 0.195 | 0.003 | **19.9 %** | 15–35 % | inside |

  Shares are summed over the three seeds; per-seed shares agree within
  0.1 percentage point, so the statistics do not limit them. Zr-91 is 63 %
  of the element's absorption, 67 % of it above thermal. Data:
  `data/salt_bands.csv`.
- **What it says.**
  - **Only U-235 fell inside its range.** Zr and F missed by less than one
    point. Be and Li-7 missed by more. I under-predicted the
    above-thermal share of Be, whose (n,α) is a fast threshold reaction,
    and of Li-7, whose 1/v tail still takes 13 %.
  - **The ordering supports the hypothesis, and does not prove it.** The
    nuclides with large ORNL/ours gaps, Zr (2.50) and F (1.82), absorb
    71 % and 59 % above thermal. Li-7, with the opposite gap (0.76),
    absorbs 87 % thermally. **Be breaks the pattern.** It absorbs 76 %
    above thermal yet has a small gap (1.14), so "above thermal" alone
    does not predict a gap. What separates Zr is *epithermal resonance*
    absorption, 67 %. F's is split between epithermal (16 %) and a fast
    threshold (n,α) part (43 %). Be's is almost all fast.
  - **For F, TM-0730 itself is a check.** Table 3.7 splits F into (n,α) 828
    and (n,γ) 621. That is 57 % (n,α), against our 43 % fast. Our fast band
    is a proxy for (n,α), since the tally does not split by reaction. So the
    two calculations put F's absorption in similar places, and the 1.82 is
    more likely in magnitude than in where it sits.
  - **Still not resolved.** A 2.5× gap concentrated in Zr's epithermal
    resonances is what a 1950s Zr resonance integral, or hafnium left in
    the zirconium, would produce. Neither is measured here. BNL-325 and
    CF-60-3-35 are still the request on gh:#1128.

### N-1 residual: U-234 and U-236 in the salt (2026-11-12, gh:#1128)

**Ablation:** `--minor-uranium tm0730` (`salt_with_tm0730_minor_uranium`). It
splits the enriched lot's non-²³⁵U as TM-0730 Table 3.2 assumes for 93 %
uranium: 1 / 93 / 1 / 5 atom % U-234 / U-235 / U-236 / U-238. This replaces
`salt.py`'s all-U-238. U-235 is unchanged, and U-234 and U-236 are added at
8.99e-7 atoms/b-cm each. A unit test pins every ratio. **The composition is a
design assumption.** No as-loaded assay was found in ORNL-4233, TM-0728,
TM-0730, ORNL-4396 or ORNL-4449.

**Prediction** (posted on gh:#1128 before the run): **−150 to −350 pcm**,
estimated at about −240.

**Run:** arm C, RVS, 30P, cold, seeds 1–3: 1.00398, 1.00668 and 1.00587
(each ± 0.0006–0.0007). About 750 s per seed, same hardware.

| salt | k | vs k_ref |
|---|---|---|
| default (`salt.py`, no U-234/236; 6 seeds) | 1.00882 ± 0.00022 | +801 pcm |
| **TM-0730's 93 % composition** (3 seeds) | **1.00551 ± 0.00080** | **+471 pcm** |
| **shift** | **−326 ± 83 pcm** (seed-paired −359 ± 105) | |

- **Inside the prediction,** on the larger side of the estimate.
- **The first composition lever that moves N-1 toward the measurement by
  several σ.** It removes about 40 % of the residual. That holds if the
  1965 lot was like TM-0730's assumed 93 % uranium.
- **The three seeds spread 138 pcm (sd),** about twice their internal σ,
  as on other runs of this model. The σ quoted is the larger of the seed
  sem and the internal σ/√3.
- ~~**The default is not changed.** Real 93 % uranium does carry U-234
  (TM-0730 itself puts it at 1 %), so a model with U-234 and U-236 is the
  more physical one. Which isotopics to adopt without an assay is the
  maintainer's call.~~ **CHANGED 2026-11-12** (maintainer: "More physical
  model is better"): `default_salt()` now carries U-234 and U-236, and
  `--minor-uranium none` is the ablation. Re-measured below.


### Re-measured on the 2026-11-12 default salt, with U-234 and U-236 (gh:#1128, gh:#1129)

**Why:** the correct-physics rule requires every V&V number that depended on
a default to be re-measured when the default changes. Two defaults changed on
2026-11-12: the salt (above), and develop's transport defaults (#1386:
seeding, reaction order, per-collision fission banking, fission-neutron
order), merged before these runs. Every run below is on that merged binary
(branch commit `090fb2703`), arm C, RVS, 30P, cold dimensions, no shield.
About 800 s per seed (4-core Intel Xeon, 4 threads, 15.7 GiB RAM, Linux, CPU
only; nuclear data 144 s, 37 nuclides). Rows carry `+u234` in their label
in `data/keff.csv` and `data/n3.csv`. σ is the larger of the seed sem and
the internal σ/√n.

**Predictions** (posted on gh:#1128 before the runs): N-1 +380 to +560 pcm;
α_iso within ±0.6 of −7.54; each split component within ±0.8 of its
previous value; concentration coefficient 0.219–0.243.

| quantity | previous default | **2026-11-12 default** | reference | prediction |
|---|---|---|---|---|
| N-1 k at 1181 °F (6 seeds) | 1.00882 ± 0.00022 (+801) | **1.00536 ± 0.00023, +455 ± 23 pcm** | k_ref 1.00080 | inside |
| α_iso (×10⁻⁵ /°F) | −7.54 ± 0.40 | **−7.69 ± 0.35** | measured −7.24 | inside |
| fuel | −4.44 ± 0.40 | **−4.18 ± 0.35** | measured −4.7 ± 0.7 | inside |
| graphite | −2.82 ± 0.43 | **−3.08 ± 0.35** | not measured | inside |
| (δk/k)/(δM/M), central | 0.231 ± 0.005 | **0.235 ± 0.006** | measured 0.223 | inside |

**N-1 seeds:** 1.00498, 1.00600, 1.00594, 1.00493, 1.00561, 1.00470 (sd
57 pcm, internal σ ≈ 57 pcm). Against the three-seed U-234 run on the
pre-#1386 binary (+471 ± 80): −16 ± 83 pcm, so **the #1386 transport
defaults make no visible difference on this model.**

**N-3 runs** (6 seeds each):

| run (salt / graphite °F) | k | seed sd | internal σ |
|---|---|---|---|
| 1131 / 1131 | 1.00917 ± 0.00027 | 67 pcm | 55 pcm |
| 1231 / 1231 | 1.00140 ± 0.00022 | 40 pcm | 54 pcm |
| C 1231 / 1131 | 1.00493 ± 0.00022 | 47 pcm | 54 pcm |
| D 1131 / 1231 | 1.00605 ± 0.00023 | 43 pcm | 56 pcm |

- **α_iso −7.69 ± 0.35** is −0.45 from the measured −7.24 (1.3σ), and moves
  −0.15 ± 0.53 from the previous default: not resolved.
- **The split adds:** −4.18 − 3.08 = −7.26 against −7.69 isothermal.
- **Fuel −4.18 ± 0.35** is +0.52 ± 0.78 (0.7σ) from ORNL's measured
  −4.7 ± 0.7, which was found after the previous runs (see N-3 below).
- **Consistency with N-1:** the mean of 1131 and 1231 °F, 1.00528, sits
  −8 ± 40 pcm from the 1181 °F value.
- **T-1 re-run with this split:** −11 % to +12 % of TM-1647's measured
  periods (nee_soon T-1 README).

**Concentration coefficient** (3 seeds at each scale): × 0.95 0.99384,
0.99326, 0.99319 → 0.99343 ± 0.00031; × 1.05 1.01782, 1.01624, 1.01712 →
1.01706 ± 0.00046.

| form | value |
|---|---|
| **central, (k₊ − k₋)/(k₀ · 0.10)** | **0.235 ± 0.006** |
| logarithmic | 0.235 |
| upper half | 0.233 ± 0.010 |
| lower half | 0.237 ± 0.008 |

- **+5 % from the measured 0.223 (2σ** of our statistics alone, with no σ on
  the measurement), and inside ORNL's own calculated 0.234–0.248. The
  previous 0.231 ± 0.005 is +0.004 ± 0.008 away: not resolved.
- **The halves now agree** (0.004 ± 0.013); the previous 0.016 split was
  noise, as flagged.
- **What it implies for the residual.** At 0.235, the +455 pcm is the
  reactivity of **1.9 % too much ²³⁵U, about 0.63 g/L** against the measured
  32.8 ± 0.3 g/L; the model would go critical at about 32.2 g/L. That is
  about twice the measurement's own ±0.3 g/L.

**Quote these numbers for the default model.** ~~The ablation rows in the
summary table were measured on the previous default salt; their shifts are
differences, which the U-234 change should move by much less than their σ,
but they were not re-run.~~ **2026-11-13:** the ablation rows were re-run on
this default; each moved by less than 1σ.

### N-3: isothermal temperature coefficient (2026-11-09, gh:#1129)

Salt density follows −1.18e-4 /°F (ORNL-4233 §4.1). Runs at 1131 °F and
1231 °F; the split runs move one material only.

| run (salt / graphite °F) | seeds | k |
|---|---|---|
| A 1131 / 1131 | 6 | 1.01719 ± 0.00038 |
| B 1231 / 1231 | 6 | 1.00980 ± 0.00034 |
| C 1231 / 1131 | 3 | 1.01325 ± 0.00024 |
| D 1131 / 1231 | 3 | 1.01331 ± 0.00055 |

| coefficient (×10⁻⁵ /°F) | this model | ORNL calc. (ORNL-4233 p. 38) | measured (p. 37) |
|---|---|---|---|
| **isothermal** | **−7.19 ± 0.50** | −8.1 | **−7.24** (69.9 kg), −7.3 (71.7 kg) |
| fuel | −3.82 ± 0.44 | −4.1 | ~~—~~ **−4.7 ± 0.7** (p. 42; see below) |
| graphite | −3.77 ± 0.65 | −4.0 | — |

- **Isothermal against the measurement:** +0.05 ± 0.50, i.e. 0.1σ.
- **Every value is inside its posted prediction**, and the posted precision
  target σ ≤ 0.5 is met.
- **The components add:** −7.59 against −7.19.
- **Mo:** arm A only (~~not measured~~ **measured on arm C 2026-11-11, below**). Mo absorption is nearly flat in temperature, so the
  coefficient should be insensitive to it. That expectation was not
  measured.

**Re-measured under RVS (2026-11-09, gh:#1311),** isothermal only, seeds
1–3 (rows `mo-omitted+rvs` in `data/n3.csv`): A 1131 °F 1.01690 ± 0.00067
(1.01821, 1.01598, 1.01650), B 1231 °F 1.00966 ± 0.00026 (1.00971,
1.00919, 1.01008). **α_iso = −7.05 ± 0.70 × 10⁻⁵ /°F** (seed-spread σ;
±0.75 with the internal σ as a floor), against −7.19 ± 0.50 with DBRC
(difference +0.14 ± 0.86) and the measured −7.24 (0.3σ). Inside the
prediction posted on gh:#1129 (−7.7 to −6.7). The fuel/graphite split was
not re-run.

**With the 10P graphite law (2026-11-10, gh:#1129),** isothermal, RVS, Mo
omitted, seeds 1–3 (rows `+graphite-10p` in `data/n3.csv`): A 1131 °F
1.01667 (1.01713, 1.01631, 1.01657), B 1231 °F 1.00837 (1.00858, 1.00834,
1.00819). **α_iso(10P) = −8.10 ± 0.53 × 10⁻⁵ /°F** (σ: the larger of seed
spread and internal σ/√3). Against 30P under RVS (−7.05 ± 0.70):
−1.05 ± 0.92 (1.1σ); against the measured −7.24: −0.86 ± 0.53 (1.6σ).
**Prediction** posted before the run, −8.0 to −6.0: the central value falls
0.1 outside, within its σ. So 10P lowers N-1's k toward the measurement
(N-1 section) but moves α_iso slightly away from it; neither shift is
resolved at three seeds.

**Six seeds per law (2026-11-10, gh:#1129),** seeds 4–6 added to both laws
at both temperatures (RVS, Mo omitted; σ from the seed spread, which at
86–103 pcm exceeds the ≈66 pcm internal σ at 1131 °F):

| law | A 1131 °F | B 1231 °F | α_iso (×10⁻⁵ /°F) | vs measured −7.24 |
|---|---|---|---|---|
| 30P | 1.01701 | 1.00970 | **−7.12 ± 0.39** | 0.3σ |
| 10P | 1.01623 | 1.00821 | **−7.82 ± 0.53** | 1.1σ |

**10P − 30P = −0.70 ± 0.66 (1.1σ):** not resolved, and inside the
|Δα| ≲ 1 band posted before the run. ~~The three-seed −8.10 ± 0.53 (10P)
and −7.05 ± 0.70 (30P)~~ are superseded by these. At this precision the
graphite law matters for N-1's k (about −105 pcm), not for α_iso.

**Fuel/graphite split under RVS (2026-11-10, gh:#1129),** C (salt 1231 /
graphite 1131 °F) and D (1131 / 1231 °F), seeds 1–6 each, against A under
RVS (six seeds); rows `mo-omitted+rvs@salt1231F/graphite1131F` and
`...salt1131F/graphite1231F` in `data/n3.csv`:

| coefficient (×10⁻⁵ /°F) | RVS | DBRC | change |
|---|---|---|---|
| fuel | **−4.48 ± 0.54** | −3.82 ± 0.44 | −0.66 ± 0.70 |
| graphite | **−3.33 ± 0.40** | −3.77 ± 0.65 | +0.44 ± 0.76 |
| isothermal | −7.12 ± 0.39 | −7.19 ± 0.50 | +0.07 ± 0.63 |

Both components lie inside the band posted on gh:#1129 before the run
(−4.6 to −3.0). The components add to −7.81 against −7.12 for the
isothermal run. T-1 re-run with these: −14 % to +6 % of the measured
periods (nee_soon T-1 README).

**On the default model, arm C with natural Mo (2026-11-11, gh:#1129).**
Every coefficient above was measured on arm A (Mo omitted); the N-1
default has been arm C since 2026-11-09. Isothermal, RVS, 30P, cold
dimensions, seeds 1–6 at each temperature; rows `mo-natural@salt1131F…`
and `…salt1231F…` in `data/n3.csv`. About 720 s per seed (4-core Intel
Xeon, 4 threads, 15.7 GiB RAM, Linux, CPU only).

| temperature | k (6 seeds) | seed sd | internal σ |
|---|---|---|---|
| 1131 °F | 1.01245 ± 0.00032 | 78 pcm | 63 pcm |
| 1231 °F | 1.00478 ± 0.00024 | 58 pcm | 64 pcm |

| | α_iso (×10⁻⁵ /°F) |
|---|---|
| **arm C (natural Mo, the default)** | **−7.54 ± 0.40** |
| arm A (Mo omitted), RVS, six seeds | −7.12 ± 0.39 |
| C − A | −0.42 ± 0.56 (0.8σ), not resolved |
| measured (ORNL-4233 p. 37) | −7.24; arm C is −0.30 from it (0.8σ) |

- **Inside the prediction** posted on gh:#1129 before the run: |C − A| ≤ 0.5
  and α_iso(C) in −7.6 to −6.6. It lands near the band's negative edge.
  The predicted sign (C slightly less negative) is not what the central
  value shows, but the difference is not resolved.
- **The Mo does not change N-3** at this precision. ~~The split itself was
  not re-measured on arm C.~~ **The split was re-measured on arm C the same
  day (below), and T-1 re-run with it.**
- **Consistency with N-1:** the mean of the two temperatures, 1.00862, sits
  20 ± 40 pcm from arm C's 1181 °F value of 1.00882.
- **Quote −7.54 ± 0.40 for the default model.** Arm A's −7.12 stays as the
  Mo-omitted ablation.

**Fuel/graphite split on arm C (2026-11-11, gh:#1129).** Runs C (salt
1231 / graphite 1131 °F) and D (1131 / 1231 °F), arm C, RVS, 30P, cold,
seeds 1–6 each, against the arm-C 1131 °F run above; rows
`mo-natural@salt1231F/graphite1131F` and `…salt1131F/graphite1231F` in
`data/n3.csv`. About 720 s per seed, same hardware.

| run | k (6 seeds, sem) |
|---|---|
| 1131 / 1131 (reference) | 1.01245 ± 0.00032 |
| C 1231 / 1131 | 1.00792 ± 0.00026 |
| D 1131 / 1231 | 1.00957 ± 0.00031 |

| coefficient (×10⁻⁵ /°F) | arm C (default) | arm A, RVS | posted band |
|---|---|---|---|
| fuel | **−4.44 ± 0.40** | −4.48 ± 0.54 | −5.3 to −3.7 |
| graphite | **−2.82 ± 0.43** | −3.33 ± 0.40 | −4.1 to −2.5 |
| sum | −7.26 | −7.81 | within ~1 of −7.54 |

- **Both components are inside the prediction** posted on gh:#1129 before the
  run, and they add to within 0.3 of the arm-C isothermal value.
- **T-1 re-run with them** (nee_soon T-1 README): **−11 % to +8 %** of
  TM-1647's measured periods. **Quote these for the default model.**
- **Against ORNL's measured fuel coefficient (added 2026-11-11).** ~~This
  page listed no measured fuel coefficient.~~ **CORRECTED:** ORNL-4233 §4.2
  (pp. 41–42) reports two transient tests in which a hot slug of salt
  entered the core before the graphite could follow: **−(4.9 ± 2.3)** and,
  repeated with less noise, **−(4.7 ± 0.7) × 10⁻⁵ /°F**. ORNL notes that
  measurement-lag effects keep the uncertainty of the second from being well
  assessed. Arm C's −4.44 ± 0.40 is **+0.26 ± 0.81 (0.3σ)** from −4.7 and
  inside −4.9 ± 2.3. **This comparison was not predicted before the runs:**
  the reference was found after them, so it carries no pre-registered band.
  The graphite coefficient was not measured separately.

**Thermal expansion in N-3, an ablation (2026-11-11, gh:#1129).** Every N-3
coefficient above uses cold dimensions. `--hot-dims` expands the graphite and
INOR-8 to each run's own temperature (TM-0728 §5.3 coefficients, as in the
N-1 hot-dims section). Arm C, RVS, 30P, seeds 1–6 at each temperature; rows
`…+hot-dims` in `data/n3.csv`. About 720 s per seed, same hardware.

| temperature | cold k (6 seeds) | hot-dims k (6 seeds) | hot − cold |
|---|---|---|---|
| 1131 °F | 1.01245 ± 0.00032 | 1.01532 ± 0.00027 | +279 ± 41 pcm |
| 1231 °F | 1.00478 ± 0.00026 | 1.00690 ± 0.00026 | +210 ± 37 pcm |

| | α_iso (×10⁻⁵ /°F) |
|---|---|
| cold (the default) | −7.54 ± 0.40 |
| hot dimensions | **−8.23 ± 0.37** |
| hot − cold | **−0.69 ± 0.54 (1.3σ)** |

- **A miss on both predicted quantities** (posted on gh:#1129 before the
  run). I predicted the shift at +0.14, inside ±0.6, and α_hot in −8.0 to
  −6.8. The shift came out −0.69, and α_hot −8.23. **The sign was the
  opposite of the prediction**, though at 1.3σ the shift is not resolved.
- **What the prediction assumed:** that hot − cold grows linearly with
  temperature (N-1's +158 ± 50 pcm over 1111 °F, so about +14 pcm per
  100 °F). The two pairs show the opposite: +279 pcm at 1131 °F and +210
  at 1231 °F. Neither agrees well with the linear estimate (about +150 and
  +165). The 1131 °F point is the outlier. The test cannot tell whether that
  is a fluctuation or a real non-linearity: the difference between the two
  pairs is 69 ± 55 pcm.
- **Bound:** at 2σ, expansion moves α_iso by less than about 1.8 × 10⁻⁵ /°F
  in magnitude, on either side. The README's earlier "about 2 % of α_iso"
  estimate is neither confirmed nor excluded.
- **Against the measurement:** the hot-dims value is −0.99 ± 0.37 from the
  measured −7.24 (2.7σ), and the cold value −0.30 (0.8σ). Whether the
  comparison should use hot dimensions is part of the open hot-dims default
  question, which is the maintainer's call.
- **The default is not changed.**

**Follow-up: six more seeds at 1131 °F (2026-11-11, gh:#1129).** To test
whether the +279 pcm pair was a fluctuation, seeds 7–12 were run at 1131 °F,
cold and hot. The prediction posted before the run: seeds 7–12 alone give
+150 to +250 pcm, and the 12-seed pool lands at about +200–260.

| 1131 °F | cold k | hot-dims k | hot − cold |
|---|---|---|---|
| seeds 1–6 | 1.01245 ± 0.00032 | 1.01532 ± 0.00027 | +279 ± 41 pcm |
| seeds 7–12 | 1.01240 ± 0.00026 | 1.01492 ± 0.00032 | **+245 ± 41 pcm** |
| seeds 1–12 | 1.01243 ± 0.00018 | 1.01512 ± 0.00021 | **+262 ± 28 pcm** |

- **Seeds 7–12 are inside their predicted band; the pool is 2 pcm above
  its own.** The new seeds reproduce the excess, so the +279 pcm was not
  a fluctuation of seeds 1–6.
- **What stays unresolved** is the trend with temperature. 1231 °F gives
  +210 ± 37, so the difference between the two pairs is 52 ± 47 pcm.
  Against the linear estimate of about +150 from N-1's three-seed 1181 °F
  pair (+158 ± 50), 1131 °F is 111 ± 57 pcm high (1.9σ). ~~The likelier reading
  is that the three-seed N-1 hot-dims value is low, not that expansion is
  non-linear. Neither is tested.~~ **TESTED 2026-11-12 and refuted:** N-1's
  seeds 4–6 at 1181 °F give +168 ± 56 pcm, below the +180 to +300 posted on
  gh:#1128 before the run, and the six-seed pool is **+161 ± 38**, below
  its predicted 190–240. The 1181 °F value was not low.
- **The three temperatures now read +262 ± 28 (1131 °F), +161 ± 38
  (1181 °F) and +210 ± 37 (1231 °F).** Smooth expansion should raise hot −
  cold steadily with temperature, and this pattern does not: 1181 °F sits
  101 ± 47 pcm (2.1σ) below 1131 °F. At 2σ, with seed spreads that have
  run up to twice the internal σ on this model, I record it as **not
  resolved**, not as a physics effect. No mechanism has been identified, and
  none has been tested.
- **Paired by seed** (cold and hot runs of the same seed), hot − cold is
  +262 ± 19 (1131 °F, 12 pairs), +161 ± 37 (1181 °F, 6 pairs; seed 3
  pairs to only +5) and +210 ± 24 (1231 °F, 6 pairs). The dip at 1181 °F
  is 2.4σ against 1131 °F on this estimator. Pairing shrinks the σ at 1131
  and 1231 °F, so cold and hot runs of a seed are correlated. It does not
  shrink it at 1181 °F. ~~The dip is still unresolved.~~
- **The dip, tested (2026-11-12):** seeds 7–12 at 1181 °F, cold and hot.
  The prediction posted on gh:#1128 before the run was a seed-paired
  +170 to +300 pcm, and a 12-seed pool of +190–250.

  | 1181 °F, seed-paired hot − cold | |
  |---|---|
  | seeds 1–6 | +161 ± 37 pcm |
  | seeds 7–12 | **+225 ± 22 pcm** (inside the predicted band) |
  | seeds 1–12 | **+193 ± 23 pcm** (inside the predicted band) |

  **The dip was a fluctuation of seeds 1–6.** The twelve-seed pattern is
  +262 ± 19 (1131 °F), +193 ± 23 (1181 °F) and +210 ± 24 (1231 °F). That is
  roughly flat at about +220 pcm, with 1131 °F now the high point (2.3σ
  above 1181 °F). It still does not show the slow rise the linear estimate
  assumed. At this statistics, hot − cold does not vary resolvably over
  1131–1231 °F.
- **N-1 with twelve seeds at 1181 °F** (`data/keff.csv`): cold **1.00876 ±
  0.00019 (+795 pcm)**, hot dimensions **1.01073 ± 0.00022 (+992 pcm)**.
  - The cold value agrees with the six-seed +801 ± 22, which stays the
    number quoted throughout this page.
  - The hot-dims model sits inside the 1000 pcm band with only 8 pcm to
    spare. Within its σ, it could be on either side.
- **Consequence for N-3:** the expansion term in α_iso is not determined at
  this statistics. Its sign is uncertain and its size is below about
  1.5 × 10⁻⁵ /°F.
- **α_iso with twelve seeds at 1131 °F** (six at 1231 °F): cold **−7.52 ±
  0.32**, hot **−8.04 ± 0.33**, shift **−0.52 ± 0.46 (1.1σ)**. The cold value
  agrees with the six-seed −7.54 ± 0.40, which stays the quoted number.

### A finding about the statistics

At 1131 °F (N-3, gh:#1129) three seeds spread 133 pcm (sd), against an
internal σ of about 64 pcm per run; at 1231 °F they agree within 19 pcm.
Either the internal (batch) σ understates the error through
inter-generation correlation, or 100 inactive generations do not fully
converge this large thermal core's source.

**Every uncertainty quoted for this model therefore comes from the
seed-to-seed spread.** ~~A convergence study (Shannon entropy, more inactive
generations) has not been done.~~ **UPDATED 2026-11-09:** a first check on
inactive generations has been done (next section); ~~a Shannon-entropy study
has not~~ and a Shannon-entropy study (the section after it).

### Source convergence: 300 against 100 inactive generations (2026-11-09)

Arm A at 1181 °F, seeds 1–3, **300 inactive** + 200 active, 20 000
histories, nothing else changed (`data/conv.csv`). Criterion posted on
gh:#1128 before the run: converged if |Δk| is below about 2 combined σ
(≈ 80 pcm) and the spread does not shrink.

| inactive | seeds | k (mean) | seed sd | sem |
|---|---|---|---|---|
| 100 | 1–5 | 1.01278 | 63 pcm | ±28 pcm |
| 300 | 1–3 | 1.01352 | 41 pcm | ±24 pcm |

- **Δk = +74 ± 37 pcm (2.0σ).** Paired on seeds 1–3: −35, +135, +23 pcm,
  mean +41 ± 50.
- **Against the criterion:** inside the posted 80 pcm, so the pre-registered
  verdict is "converged". It is marginal: a bias of order +70 pcm from 100
  inactive generations is **not excluded** at this statistics.
- **The spread:** at 1181 °F the 100-inactive seed sd (63 pcm) already equals
  the internal σ (≈ 66 pcm); the 2× excess was seen at 1131 °F (N-3). Three
  seeds cannot tell 41 from 63 pcm.
- **Effect on the records above:** N-1's +1197 ± 28 pcm (arm A) may read up to
  about 70 pcm low. The N-3 coefficients difference runs that share settings,
  so a common bias cancels to first order. Nothing above is re-stated on this
  result alone.

Hardware: as above (4 threads); 1269–1324 s transport per seed at 300
inactive.

### Source convergence by Shannon entropy (2026-11-09)

**Method.** `--entropy-csv` routes the run through `run_keff_csg_hybrid`
with no majorants and an 8×8×8 mesh over the source box (512 bins, 9 bits
at most), writing k and H per generation (`data/entropy.csv`).
`stats::convergence::SourceConvergence` runs MSER-5, Geweke and a change
point on both traces. Arm C (natural Mo), RVS, develop `d2da89932` plus the
flag. Prediction posted on gh:#1128 before the run: MSER truncates well
under 100 generations.

| run | k | MSER truncation (H / k) | Geweke z (H / k) |
|---|---|---|---|
| seed 1, 100 + 200 (identity check) | 1.009476 | 10 / 10 | — |
| seed 1, 300 + 200 | 1.00871 ± 0.00057 | 10 / 10 | −0.58 / +0.43 |
| seed 2, 300 + 200 | 1.00879 ± 0.00059 | 5 / 5 | +0.12 / −0.84 |

- **Identity check passed:** the entropy path reproduces the plain run's
  k = 1.009476 to every printed digit, as `run_keff_csg_hybrid`'s docs
  state.
- **H falls from 8.59 bits to a plateau of 8.52–8.53 within about 10
  generations** (the uniform source box over-spreads the source), and stays
  there: mean H over generations 11–150 and 151–300 agree to 0.001 bit on
  both seeds. No drift (Geweke), no change point after the first batch.
- **Inside the prediction.** 100 inactive generations are ten times what
  either trace needs.
- **Against the earlier +74 ± 37 pcm:** the two 300-inactive RVS seeds
  average 1.00875, against 1.00882 ± 0.00022 for the six 100-inactive RVS
  seeds: −7 pcm. The convergence bias that check could not exclude is not
  seen here.
- **Limit of the instrument:** an 8×8×8 entropy is a coarse source
  measure. It cannot see a slow tilt within a bin, so this bounds gross
  non-convergence, not every higher mode.

Hardware: as above (4 threads); 1311–1321 s transport per seed at 300
inactive.

### Resonance-scattering ablation (2026-11-09, gh:#1311)

**Question.** Route 1 (OpenMC on NJOY2016 ACE) sits below route 4 (this
crate) at 1181 °F. Does the resonance elastic scattering treatment explain
the gap?

**Method.** Both codes were run with DBRC on and off at 1181 °F, three seeds
each, Mo omitted, 20 000 particles × [100 + 200], same seeds in both arms.
OpenMC: `msre_n1_openmc.py --no-dbrc`. This crate:
`examples/msre_n1_first_criticality.rs` with `--no-dbrc` (every nuclide
through `without_dbrc()`). The DBRC-on route-1 seeds are those in
`data/per_seed_keff_msre_route1_2026_11_09.csv`; the route-4 DBRC-on seeds
were re-run and reproduce `data/keff.csv` seeds 1–3 to every printed digit.
Uncertainties are the seed-to-seed standard error (see the next section).
Data: `data/per_seed_keff_msre_dbrc_route{1,4}_2026_11_09.csv`.

| | route 1 (OpenMC) | route 4 (this crate) | gap, route 4 − route 1 |
|---|---|---|---|
| DBRC on | 1.01232 ± 0.00026 | 1.01311 ± 0.00036 | **+80 ± 44 pcm** |
| DBRC off | 1.01253 ± 0.00042 | 1.01330 ± 0.00037 | **+77 ± 55 pcm** |
| worth (on − off) | −21 ± 49 pcm | −19 ± 51 pcm | |

**Result.** DBRC is worth nothing resolvable in either code, and the two
worths agree. The gap is the same with it on or off, so **resonance
scattering is not the cause of the route 1/4 gap.** Graphite's thermal law
and URR were ruled out the same day. The maintainer chose not to pursue the
gap further ("leave it be").

These runs predate the switch of both codes' default to RVS (gh:#1311,
same day). They compare DBRC with nothing, so the switch does not affect them.
