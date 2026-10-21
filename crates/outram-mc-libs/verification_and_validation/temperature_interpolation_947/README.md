# Temperature-interpolation error: OpenMC's stochastic table choice against exactly processed data (GitHub #947)

**Issue:** [#947](https://github.com/theodoreOnzGit/outram-park-backend/issues/947),
step 1 of [#945](https://github.com/theodoreOnzGit/outram-park-backend/issues/945)
(epic #925). **Status:** methodology and predictions written 2026-10-17
**before any run**; results below them.

**Code:**
- the port: `src/material/temperature_interpolation.rs`
  (`TemperatureTables`, `bracket`, `InterpolationVariable`, the Chebyshev arm);
- the instruments: `src/vv/temperature_interpolation_947.rs`;
- the per-collision draw in a walk: `InfiniteMediumMc::run_with_tables`
  (`src/physics/slowing_down.rs`);
- the study: `examples/temperature_interpolation_947.rs`;
- the pins: `tests/temperature_interpolation_947.rs`.

**This is a measurement, not a decision.** Which default temperature grid
#945 uses is the maintainer's call. Nothing here is tuned, and no
transport default changes: the port is not on any transport path.

## 1. What upstream does (read first)

OpenMC `d7d3284a` (`/home/teddy0/Documents/research/openmc`):

- `src/nuclide.cpp:171-183` loads the two tabulated temperatures that
  bracket each model temperature.
- `src/nuclide.cpp:686-708`: at each cross-section lookup,
  `f = (kT - kT_i)/(kT_{i+1} - kT_i)` and `if (f > prn) ++i_temp`. The upper
  table is used with probability `f`, **linear in kT** (so in T).
- `src/nuclide.cpp:826-828`: the URR probability table is
  `urr_data_[micro.index_temp]`, the same draw.
- `src/physics.cpp:697`: free-gas kinematics take `kTs_[i_temp]`.
- `src/thermal.cpp:195-215`: S(α,β) uses the same rule with its own draw.
- Outside the tabulated range upstream **snaps** to the end table
  (`nuclide.cpp:688-696`). This port refuses (#945 acceptance).

**A correction found on the way.** The 2026-09-24 record
`njoy-outram-park-fork/verification_and_validation/temperature_interpolation/sqrt_t_interpolation_2026_09_24.md`
and #269 say upstream interpolates in `sqrt(T)`. The code above is linear in
`kT`. That doc is corrected in this change (struck through, with this
reference).

## 2. Methodology

### 2.1 Data and processing

- **U-238**: ENDF/B-VIII.0 `n-092_U_238.endf` (MAT 9237), `reference-data/endf/`.
- **Moderator for the slowing-down instrument**: C-12, ENDF/B-VIII.0
  `n-006_C_012-ENDF8.0.endf`, held at one temperature (300 K) in every arm so
  only U-238's representation differs.
- **Processing**: njoy's `process::reconstruct` (RECONR at 0 K, tolerance
  1e-3) **once**, then `process::cross_sections_at` per temperature (#934):
  BROADR, with PURR and HEATR off for the pointwise data. Each result goes
  through `Nuclide::from_processed`, so the numbers are what the transport
  lookup (`Nuclide::xs_at_energy`) returns.
- **BROADR thinning tolerance.** NJOY's default `errthn = 1e-3` lets each
  table differ from the true broadened curve by up to 0.1 % between its points,
  which would put a ~2e-3 floor under every pointwise comparison. The study
  therefore broadens at **`errthn = 1e-4`**, and reports the floor separately:
  the exact table at one temperature broadened at 1e-3 against 1e-4. RECONR's
  error is common to every temperature (one reconstruction) and does not enter
  a difference to first order.
- **URR**: PURR at NJOY's production settings (20 bands, 64 ladders, 10 000
  samples, seed −101) per temperature,
  `UrrProbabilityTables::from_endf`. PURR's ladders are drawn from the same
  seed at every temperature, so its sampling noise is correlated across
  temperature and largely cancels in a difference (checked, §2.6).
- **H in H₂O**: `ThermalScattering::from_embedded_leapr_deck(HInH2O, T)`, i.e.
  LEAPR run on the ENDF/B-VIII.0 deck's own block for `T` (maintainer rule:
  LEAPR always, #870). The deck re-specifies its frequency spectrum at each of
  its 18 temperatures, so "exact at T" exists **only at the deck's
  temperatures**: 283.6, 293.6, 300, 323.6, 350, 373.6, 400, …, 650, 800 K.

### 2.2 Temperature grids and the arms compared

**Pointwise and URR (U-238), range [300, 1500] K:**

| arm | tables | how a lookup at T is answered |
|---|---|---|
| stochastic, uniform Δ = 600, 300, 150, 75 K | 3, 5, 9, 17 | OpenMC's draw between the bracketing pair, `f` linear in **T** (upstream), **√T** or **log T** |
| plug-in `E[σ]` (T only) | as above | the expected σ used deterministically, as a mixture would |
| Chebyshev fit in √T and in T, n = 3, 4, 5, 6 | n | the degree n−1 interpolant through tables at the Chebyshev nodes of [300, 1500] K |
| exact | 1 per test T | data processed at T |

**Test temperatures:** for the stochastic arms, the **midpoint in T of every
bracket** (the 37.5 K grid points not on the Δ grid), the worst case of a
linear interpolant in T. For the fit arms, every point of the 37.5 K grid,
so the fit's worst point is found wherever it lies.

**S(α,β) (H in H₂O):** brackets of deck temperatures with a deck temperature
strictly inside, Δ ≈ 50, 100, 200, 300 and 400 K (listed with the results).

### 2.3 What is measured, and why each

Stochastic interpolation is unbiased **only for a quantity linear in the
table**. A collision's reaction probability `σ_x/σ_t`, the flight length and
any self-shielded rate are not linear, and transport sees `E[g(σ)]`, not
`g(E[σ])`. So:

1. **`E[σ](E)` against `σ_exact(E, T)`**, total, capture and elastic, on the
   0 K RECONR grid (every table's points lie on it): the largest relative
   error of each sign and where, in 10⁻⁵–1 eV, 1–20 keV (resolved), and as
   **band integrals** `∫σ dE/E` over 10⁻⁵–1 eV, 1–10 eV, 10–100 eV,
   0.1–1 keV, 1–20 keV.
2. **Narrow-resonance self-shielded integrals** at background
   σ₀ = 10, 100, 1000 b: `N = ∫σ_c/(σ_t+σ₀) dE/E` and `D = ∫1/(σ_t+σ₀) dE/E`
   over 1 eV–20 keV. In an infinite medium a per-lookup draw gives the
   **mixture of the tables' N and D**; the plug-in arm gives N and D **of
   the mixed σ**. Both are reported against exact.
3. **The transport quantity: resonance escape probability `p`** in an
   infinite U-238 + C-12 medium, source at 10 keV, scored to 1 eV, at
   dilutions σ_b = N_C σ_p,C/N_U = 100 b and 1000 b, with `σ_p,C` taken from
   the C-12 data at 100 eV. Solved **deterministically**
   (`slowing_down::solve_on_grid`, exact for isotropic-CM scattering off a
   target at rest; validated against Monte Carlo to ≤ 1.8σ at five dilutions,
   2026-09-11). The stochastic arm's grid is the weighted sum of the tables'
   collision fractions (`mix_slowing_down_grids`), which is what a draw per
   collision gives in an infinite medium (flight length does not matter
   there).
4. **Monte Carlo check of item 3**, because item 3's stochastic arm is a
   derived expectation, not a transport run: `InfiniteMediumMc` with
   `run_with_tables`, one draw per nuclide per collision as upstream, at the
   coarsest bracket (tables 300 and 1500 K, T = 900 K, T variable), against
   the exactly processed 900 K table. 10⁶ histories per arm, independent seeds;
   σ is binomial.
5. **URR**: per PURR energy, the flux-weighted band-averaged
   `σ_c,eff = N/D` and `σ_t,eff` at σ₀ = 10, 100, 1000 b and infinite
   dilution, with the band expectation integrated **exactly** over the
   variate (`urr_band_weights`). The stochastic arm mixes the two tables' N
   and D (one draw serves pointwise and URR, `nuclide.cpp:826-828`).
6. **S(α,β)**: `σ_sab(E)` per H below the cutoff, pointwise and as band
   integrals; and, as the transport quantity, the fraction of neutrons
   absorbed in Pu-239 in an infinite H₂O + Pu-239 medium (JENDL-3.3
   `n-094_Pu_239-JENDL3.3.endf`, chosen as an **instrument**: its 0.3 eV
   resonance makes the absorption sensitive to the thermal spectrum, where a
   1/v absorber would not be), source at 4 eV (below the S(α,β) cutoff),
   `ScatterKernel::Production`, exact table against stochastic, 10⁶ histories
   per arm.

### 2.4 Which numbers to quote (decided before running)

- For **#945's grid decision**: item 3 (`p`, the transport quantity) and
  item 2 at σ₀ = 100 b, per spacing and variable, at the **low end of the
  range** (300–600 K), where the curvature in T is largest. The pointwise
  maximum is reported but is not the figure of merit: it sits in a few eV of
  resonance wing and says nothing about how many neutrons see it.
- Item 1's band integrals are reported to show they **cannot** see the error
  (Doppler broadening conserves resonance area), not as evidence of accuracy.
- Item 4 is a check on item 3's derivation, not a separate result.

### 2.5 Predictions (written 2026-10-17, before any run)

Basis: a strongly Doppler-broadened resonance has peak σ ∝ 1/√T and far-wing
σ ∝ T; the effective resonance integral follows I(T) ≈ I₀[1 + β(√T − √T₀)]
(Hellstrand's empirical law), β ≈ 0.5–1 % per √K at σ₀ ≈ 100 b, smaller when
more dilute. A linear interpolant of a convex function lies above it; of a
concave one, below. Errors of a linear interpolant scale as Δ².

| # | quantity | arm | prediction (sign, size) |
|---|---|---|---|
| P1 | pointwise capture, max \|err\|, lowest bracket | T, Δ = 600 / 300 / 150 / 75 K | 5–20 % / 2–6 % / 0.5–1.5 % / 0.15–0.4 %; the largest error **positive** (convex at peaks); 6–9× smaller in the top bracket |
| P2 | same | √T, log T | at peaks log T < √T < T; in far wings T best; overall no variable removes the error, √T best by ≲ 2× |
| P3 | infinitely dilute band integrals | all | \|err\| ≤ 1e-4 for Δ ≤ 300 K, ≤ 1e-3 at Δ = 600 K (area conservation hides the error) |
| P4 | NR capture RI, σ₀ = 100 b, stochastic | T | **negative** (I concave in T): Δ = 600 at 600 K −0.5 to −0.8 %; Δ = 300 lowest −0.2 to −0.3 %; Δ = 75 ≈ −0.02 % |
| P5 | same | √T / log T | √T: \|err\| < 1/10 of T's; log T: **positive**, magnitude ≈ T's |
| P6 | plug-in E[σ] vs stochastic mixture | T | plug-in RI **larger** than stochastic (σ/(σ+σ₀) is concave), difference comparable to P4 |
| P7 | resonance escape p, σ_b = 100 b | T | **p overestimated**: Δ = 300 lowest +0.3 to +0.5 %; Δ = 75 +0.02 to +0.03 %; σ_b = 1000 b about 3–5× smaller |
| P8 | same | √T / log T | √T \|Δp/p\| ≤ 1/5 of T's; log T opposite sign to T |
| P9 | MC check, 900 K from {300, 1500}, σ_b = 100 b | T | Δp/p **+1.5 to +3 %** against exact, resolved at > 5σ; MC stochastic agrees with the deterministic stochastic arm within 2σ |
| P10 | Chebyshev fit in √T, [300, 1500] | n = 3 / 5 | pointwise max \|err\| ≤ 2 % / ≤ 0.1 %; p and RI ≤ 0.01 % at n = 5; fit in T ~10× worse at equal n; no negative σ |
| P11 | URR σ_c,eff at σ₀ = 10 b | T | negative; Δ = 300 ≤ 0.1 %, Δ = 150 ≤ 0.03 %; infinite dilution ≤ 1e-4 |
| P12 | S(α,β) σ_sab pointwise max \|err\| | T, Δ ≈ 50 / 100 / 200 / 400 | ≤ 0.3 % / ≤ 1 % / ≤ 3 % / ≤ 8 %, largest at the lowest energies; the per-temperature frequency spectra may make it less smooth than that |
| P13 | Pu-239 absorbed fraction, H₂O | T, Δ = 400 | \|Δ\| < 1e-3 absolute, not resolved at 10⁶ histories (a bound) |
| P14 | thinning floor (1e-3 vs 1e-4 at one T) | — | max \|Δσ/σ\| ≈ 1e-3 pointwise, ≤ 1e-5 in band integrals |

### 2.6 Checks on the instruments

- `E[σ]` at a tabulated temperature equals that table (f = 0 or 1): the
  error there must be exactly 0.
- The stochastic slowing-down arm with weights (1, 0) reproduces that table's
  own solve bit for bit.
- PURR noise: the URR comparison is meaningful only if the difference between
  two temperatures' tables is smooth in T; it is checked by the
  zero-spacing case (a table against itself, 0) and by the Δ² scaling.

## 3. Results (run 2026-10-17/18, commit of this change)

All numbers are `(approx/exact − 1)` in %, unless marked absolute. Raw
output of every stage is reproduced by the example; the tables below are
copied from it.

### 3.0 Instrument checks (all passed)

- Rebuilt slowing-down grid against the library's own `slowing_down_grid`:
  `p = 0.1749516501` both, difference **3.3e-16**.
- `E[σ]` at a tabulated temperature (600 K between 300/600/900) against
  that table: **0** at every point.
- **Thinning floor (P14):**

| errthn pair (600 K) | channel | pointwise 1e-5–1 eV | pointwise 1–20 keV (over / under) | band integrals (5 bands) | p σ_b = 100 b |
|---|---|---|---|---|---|
| 1e-3 vs 1e-4 | capture | +0.096 | +1.29 @ 5.13 keV / −1.18 @ 18.7 keV | ≤ 0.011 | −0.020 |
| 1e-4 vs 1e-5 | capture | +0.001 | +0.97 @ 19.0 keV / −1.25 @ 13.0 keV | ≤ 0.0009 | −0.0020 |
| 1e-3 vs 1e-4 | total | +0.006 | +0.26 / −0.12 | ≤ 0.010 | — |

  **P14 refuted, and it matters for reading §3.1.** BROADR's thinning
  bounds the *total*; capture in the keV valleys between resonances moves by
  ~1 % even between 1e-4 and 1e-5. So **pointwise capture maxima above
  ~1 keV are not resolved below ~1 %** at any practical tolerance. Integrals
  are converged at 1e-4 (p moves 2e-5). Production libraries (1e-3) carry a
  −0.02 % shift in p from thinning alone.

  **ADDED 2026-10-18 (#948, `../wmp_local_temperature_948/README.md` §5.4):**
  this floor varies BROADR only. RECONR's tolerance (1e-3 in every arm
  here) is a larger floor: on U-238 ENDF/B-VII.0 at 900 K, RECONR/BROADR
  1e-3/1e-4 (this study's exact arm) is **−0.030 %** in p against
  1e-5/1e-5, and production 1e-3/1e-3 is **−0.050 %**. The differences
  reported below are between tables from one reconstruction, which share
  most of it, so they stand; the absolute "exact" arm is not converged to
  2e-5.

### 3.1 U-238 pointwise, stochastic arms (expected σ against exact)

Worst bracket over [300, 1500] K, test T = bracket midpoint in T.

| Δ K | arm | capture 1 eV–20 keV: max over @ E / max under | capture 1 eV–1 keV | capture 1e-5–1 eV | total 1 eV–20 keV max abs |
|---|---|---|---|---|---|
| 600 | T | +119.7 @ 4.27 keV / −17.4 | +96.1 / −17.0 (763–778 eV) | 0.0012 | 65.0 |
| 600 | √T | +143.9 @ 4.27 keV / −12.7 | +115.3 / −12.4 | 0.0007 | 80.3 |
| 600 | log T | +167.8 @ 4.27 keV / −9.1 | +134.3 / −8.9 | 0.0013 | 95.9 |
| 300 | T | +56.3 @ 6.41 keV / −7.9 | +43.5 / −7.7 (777 eV) | 0.0005 | 29.6 |
| 300 | √T | +64.8 / −5.7 | +50.8 / −5.5 | 0.0002 | 36.0 |
| 300 | log T | +73.3 / −4.1 | +58.1 / −3.9 | 0.0004 | 42.6 |
| 150 | T | +20.5 @ 9.30 keV / −2.9 | +15.8 / −2.8 (778 eV) | 0.0002 | 10.7 |
| 150 | √T | +23.7 / −2.1 | +18.5 / −2.0 | 0.0001 | 13.0 |
| 150 | log T | +26.9 / −1.5 | +21.2 / −1.4 | 0.0001 | 15.4 |
| 75 | T | +6.2 @ 5.14 keV / −0.9 | +4.8 / −0.9 (778 eV) | 0.0000 | 3.4 |
| 75 | √T | +7.2 / −0.7 | +5.6 / −0.6 | 0.0001 | 4.1 |
| 75 | log T | +8.1 / −0.5 | +6.4 / −0.5 | 0.0001 | 4.8 |

All maxima sit at the lowest bracket (300 K to 300+Δ) and in the
**interference valleys** between resonances (≈ 777 eV below 1 keV, 4–10 keV
above), where σ_c is milli-barns and changes by large factors
with T. They are an order of magnitude above P1's prediction, which was
reasoned from resonance peaks. In those valleys the largest positive error
**grows** from T to √T to log T (P2's ordering was for peaks). Thermal
(1e-5–1 eV) is unaffected (≤ 1.3e-5).

### 3.2 U-238 integrals: what a neutron sees

Worst signed error over all brackets (which is always the lowest bracket).

| Δ K | arm | capture band integral (any of 5 bands) | NR RI σ₀ = 10 b | σ₀ = 100 b | σ₀ = 1000 b | **p, σ_b = 100 b** | p, σ_b = 1000 b |
|---|---|---|---|---|---|---|---|
| 600 | T (upstream) | +0.0017 | −0.41 | −0.56 | −1.29 | **+1.16** | +0.87 |
| 600 | √T | +0.0008 | −0.05 | +0.13 | −0.14 | **−0.32** | +0.13 |
| 600 | log T | +0.0015 | +0.31 | +0.80 | +1.01 | **−1.77** | −0.60 |
| 600 | plug-in E[σ] (T) | +0.0017 | +0.14 | +0.63 | +0.32 | **−1.58** | −0.19 |
| 300 | T (upstream) | +0.0005 | −0.16 | −0.21 | −0.47 | **+0.40** | +0.30 |
| 300 | √T | +0.0002 | −0.02 | +0.05 | −0.04 | **−0.12** | +0.03 |
| 300 | log T | +0.0004 | +0.11 | +0.30 | +0.42 | **−0.63** | −0.24 |
| 300 | plug-in E[σ] (T) | +0.0005 | +0.06 | +0.23 | +0.17 | **−0.56** | −0.10 |
| 150 | T (upstream) | +0.0002 | −0.053 | −0.067 | −0.151 | **+0.127** | +0.091 |
| 150 | √T | +0.0003 | −0.008 | +0.014 | −0.009 | **−0.036** | +0.009 |
| 150 | log T | +0.0003 | +0.036 | +0.094 | +0.147 | **−0.198** | −0.083 |
| 150 | plug-in E[σ] (T) | +0.0002 | +0.018 | +0.071 | +0.065 | **−0.169** | −0.038 |
| 75 | T (upstream) | +0.0002 | −0.016 | −0.020 | −0.043 | **+0.037** | +0.026 |
| 75 | √T | +0.0002 | −0.003 | +0.004 | −0.002 | **−0.010** | +0.002 |
| 75 | log T | +0.0002 | +0.011 | +0.027 | +0.045 | **−0.057** | −0.025 |
| 75 | plug-in E[σ] (T) | +0.0002 | +0.005 | +0.020 | +0.020 | **−0.046** | −0.012 |

The **top** bracket (1500 K end) is 3.5–6× smaller than the bottom at every
spacing (e.g. Δ = 300, T: p +0.107 % at [1200, 1500] against +0.403 % at
[300, 600]). Errors scale close to Δ² (T arm, p: 1.16 → 0.40 → 0.127 → 0.037,
ratios 2.9, 3.2, 3.5).

**Plug-in ≠ stochastic.** The deterministic `E[σ]` arm and the stochastic
draw between the same two tables give errors of **opposite sign** and similar
size. Interpolating σ is not what the stochastic scheme does to a
self-shielded rate; anyone checking the scheme against a σ interpolant checks
the wrong thing.

### 3.3 Monte Carlo check of the stochastic arm (P9)

U-238 + C-12, σ_b = 100 b, 900 K from tables {300, 1500} K, T variable,
`IsotropicCmAtRest`, per-collision draw as upstream.

| arm | deterministic p | MC p ± 1σ | histories |
|---|---|---|---|
| exact 900 K | 0.139911 | 0.140029 ± 0.000174 | 4 × 10⁶ |
| stochastic | 0.144174 | 0.144073 ± 0.000124 | 8 × 10⁶ (two seeds) |

Stochastic − exact: deterministic **+0.004263 (+3.05 %)**, MC
**+0.004044 ± 0.000213 (19.0σ)**. MC against deterministic: stochastic arm
−0.8σ, exact arm +0.7σ. **The deterministic "mixture of collision fractions"
is what a per-collision draw produces in transport**, so §3.2's p column is a
transport result, not a model of one.

### 3.4 Chebyshev fit in temperature (the fourth arm)

Tables at the n Chebyshev nodes of [300, 1500] K; the degree-(n−1)
interpolant at every point of the 37.5 K grid; worst over T.

| variable | n | capture 1–20 keV max abs | capture 1 eV–1 keV max abs | min fitted capture (b) | RI σ₀ = 100 b | **p σ_b = 100 b** | p σ_b = 1000 b |
|---|---|---|---|---|---|---|---|
| √T | 3 | 112.6 | 86.7 | **−1.5e-3 (negative)** | +0.147 | **−0.384** | +0.269 |
| √T | 4 | 56.0 | 37.6 | 1.0e-3 | −0.042 | **+0.108** | −0.017 |
| √T | 5 | 14.8 | 10.2 | 1.0e-3 | +0.0038 | **−0.0097** | +0.0062 |
| √T | 6 | 4.9 | 3.0 | 1.0e-3 | −0.0010 | **+0.0027** | −0.0012 |
| T | 3 | 218.8 | 154.1 | **−5.3e-2 (negative)** | −0.363 | **+0.866** | +0.254 |
| T | 4 | 48.9 | 35.9 | 1.0e-3 | −0.039 | **+0.085** | +0.082 |
| T | 5 | 23.0 | 15.5 | 1.0e-3 | −0.0054 | **+0.016** | +0.019 |
| T | 6 | 9.4 | 6.1 | 1.0e-3 | −0.0019 | **+0.0059** | +0.0034 |

Worst point is always 300 K (the range's low end). **At equal table count
the fit is far ahead of stochastic interpolation on the transport quantity:**
5 tables, p at σ_b = 100 b: stochastic T **+0.40 %**, stochastic √T
−0.12 %, fit in √T **−0.010 %** (40× better than upstream's scheme). At
n = 3 the fit goes **negative** in capture valleys; n ≥ 4 does not.

What the fit arm would need to become a transport path (not built): all n
tables on one union energy grid (memory ≈ n tables, the same as stochastic);
per lookup an n-term dot product instead of one draw; a floor or a check
against negative values at low n; and, for #945's majorant, a **continuous**
bound in T over a region's range — the finite "largest tabulated value"
bound that stochastic interpolation allows (#945 note 3) no longer holds,
so it would need the same machinery as WMP (#948).

### 3.5 URR (U-238, 20–149 keV, 81 interior PURR energies)

PURR 20/64/10 000 at each of the 75 K grid temperatures. Worst over energies
and brackets. For scale: σ_c,eff at σ₀ = 10 b moves by at most **4.98 %**
between 300 and 1500 K.

| Δ K | arm | σ_c,eff σ₀ = 10 b | σ₀ = 100 b | σ₀ = 1000 b | σ_c inf. dilute | σ_t,eff σ₀ = 10 b | group σ_c,eff σ₀ = 10 b |
|---|---|---|---|---|---|---|---|
| 600 | T | −0.66 | −0.18 | −0.023 | 0.0000 | −0.41 | −0.37 |
| 600 | √T | −0.42 | −0.12 | −0.015 | 0.0000 | −0.23 | −0.24 |
| 600 | log T | −0.18 | −0.055 | −0.007 | 0.0000 | −0.069 | −0.12 |
| 300 | T | −0.29 | −0.080 | −0.010 | 0.0000 | −0.17 | −0.16 |
| 300 | √T | −0.19 | −0.052 | −0.007 | 0.0000 | −0.096 | −0.11 |
| 300 | log T | −0.081 | −0.025 | −0.003 | 0.0000 | −0.028 | −0.051 |
| 150 | T | −0.11 | −0.029 | −0.004 | 0.0000 | −0.059 | −0.060 |
| 150 | √T | −0.070 | −0.020 | −0.003 | 0.0000 | −0.034 | −0.039 |
| 150 | log T | −0.032 | −0.010 | −0.001 | 0.0000 | −0.012 | −0.018 |

Always negative, smooth in Δ (PURR's shared seed keeps its noise out of the
difference, as §2.1 expected), and **log T is best here**, unlike the
resolved range.

### 3.6 H in H₂O, S(α,β) from the LEAPR deck

σ_sab per H against the deck block run at the target temperature
(comparison grid: the 13 tables' breakpoints plus 4000 log points,
1e-5 eV to the 10 eV cutoff). Selected rows; T arm unless noted:

| bracket → T | Δ K | pointwise (over / under) | band 1e-5–0.01 eV | 0.01–0.1 | 0.1–1 | 1–10 |
|---|---|---|---|---|---|---|
| [300, 350] → 323.6 | 50 | +0.49 / −0.02 | +0.29 | +0.042 | +0.006 | +0.021 |
| [500, 550] → 523.6 | 50 | +0.10 / −0.54 | −0.45 | +0.068 | +0.004 | −0.009 |
| [300, 400] → 350 | 100 | +2.19 / −0.10 | +1.35 | +0.059 | +0.012 | −0.015 |
| [400, 500] → 450 | 100 | +0.27 / −1.14 | −0.91 | +0.157 | +0.017 | +0.019 |
| [300, 500] → 400 | 200 | +2.08 / −0.08 | +1.17 | +0.383 | +0.058 | −0.003 |
| [400, 600] → 500 | 200 | +0.42 / −3.55 | −2.80 | +0.195 | +0.026 | +0.006 |
| [300, 600] → 450 | 300 | +1.45 / −3.78 | −3.22 | +0.735 | +0.100 | +0.022 |
| [350, 650] → 500 | 300 | +1.12 / −7.39 | −5.96 | +0.577 | +0.057 | +0.016 |
| [400, 800] → 600 | 400 | +2.33 / −14.10 | −11.76 | +1.557 | +0.118 | +0.057 |

The pointwise extreme is always at the lowest energy (1e-5 eV), as predicted,
but its **sign changes with the bracket** (positive below ~450 K, negative
above) and no variable is best everywhere (√T and log T are worse than T in
the low brackets, better in the high ones). The deck re-specifies its
frequency spectrum at each temperature, so σ_sab(T) is not one smooth
function; brackets that straddle a spectrum change are worse than their Δ
alone suggests.

**Transport (Pu-239 absorbed fraction, infinite H₂O + Pu-239,
N_Pu/N_H₂O = 6.5e-4, source 4 eV, `Production` kernel, T arm):**

| bracket → T | Δ K | exact | stochastic | difference (absolute) | relative |
|---|---|---|---|---|---|
| [300, 400] → 350 | 100 | 0.548717 ± 0.000249 | 0.550276 ± 0.000176 | **+0.00156 ± 0.00031 (5.1σ)** | +0.28 % |
| [300, 600] → 450 | 300 | 0.579713 ± 0.000247 | 0.590890 ± 0.000174 | **+0.01118 ± 0.00030 (37σ)** | +1.93 % |
| [400, 800] → 600 | 400 | 0.638515 ± 0.000480 | 0.641734 ± 0.000339 | **+0.00322 ± 0.00059 (5.5σ)** | +0.50 % |

4 × 10⁶ histories per arm (Δ = 400: 10⁶). The error is **not monotone in
Δ**: [300, 600] is 4× worse than the wider [400, 800], the same signature as
the σ_sab table.

### 3.7 Predictions against results

| # | predicted | measured | verdict |
|---|---|---|---|
| P1 | pointwise T, lowest bracket: 5–20 / 2–6 / 0.5–1.5 / 0.15–0.4 % | 1–20 keV: +120 / +56 / +21 / +6.2 %; 1 eV–1 keV: +96 / +44 / +16 / +4.8 % (both in valleys) | **refuted** (10×, located in valleys, not peaks) |
| P2 | log T < √T < T at peaks; √T best overall | in the keV valleys T < √T < log T on the positive side | **refuted** for the valley maxima |
| P3 | band integrals ≤ 1e-4 (Δ ≤ 300) | ≤ 5e-6 (Δ ≤ 300), 1.7e-5 (Δ = 600) | held |
| P4 | RI σ₀ = 100, T: −0.5…−0.8 / −0.2…−0.3 / ≈ −0.02 % | −0.56 / −0.21 / −0.020 % | **held** |
| P5 | √T < 1/10 of T; log T positive ≈ T | √T +0.045 (≈ 1/5, other sign); log T +0.30 | log T held; √T 2× outside |
| P6 | plug-in RI larger than stochastic | +0.23 vs −0.21 % | held |
| P7 | p σ_b = 100, T: +0.3…+0.5 / +0.02…+0.03 %; σ_b = 1000 3–5× smaller | +0.40 / +0.037 %; σ_b = 1000 only 1.3× smaller | Δ = 300 held; Δ = 75 slightly over; dilution ratio **refuted** |
| P8 | √T ≤ 1/5 of T; log T opposite sign | √T 0.29 of T (opposite sign); log T opposite sign | log T held; √T marginal |
| P9 | +1.5…+3 %, > 5σ, MC within 2σ | +3.05 % det., +2.89 ± 0.15 % MC (19σ), agreement −0.8σ | held (at the top edge) |
| P10 | √T n = 3/5 pointwise ≤ 2 / 0.1 %; p ≤ 0.01 % at n = 5; T fit 10× worse; no negatives | 113 / 15 % (valleys); p −0.0097 %; T fit 1.7× worse; **negative at n = 3** | p held; pointwise, ratio and negativity refuted |
| P11 | URR T: Δ = 300 ≤ 0.1 %, Δ = 150 ≤ 0.03 %, negative; inf. dil. ≤ 1e-4 | −0.29 / −0.11 %, negative; 0 | magnitude **refuted** (3×); sign held |
| P12 | σ_sab ≤ 0.3 / 1 / 3 / 8 % | 0.54 / 2.2 / 3.5 / 14.1 % | **refuted** (≈ 2×), sign irregular |
| P13 | Pu-239 fraction unresolved, \|Δ\| < 1e-3 | +0.0032 ± 0.0006 (5.5σ) | **refuted** |
| P14 | thinning floor ≈ 1e-3 pointwise, ≤ 1e-5 integrals | 1.3 % pointwise capture, 1.1e-4 band integrals | **refuted** |

### 3.8 Error budget, and which numbers to quote

**Quote the p column (σ_b = 100 b) and the RI at σ₀ = 100 b of §3.2, lowest
bracket**, as decided in §2.4; §3.3 shows they are transport results. The
pointwise maxima (§3.1) are reported but are valley- and floor-dominated.

U-238 resolved range, worst (lowest) bracket, p at σ_b = 100 b (≈ Δk/k for
this strongly self-shielded instrument; a real LWR lattice, with
−ln p ≈ 0.3 against 1.7 here, would see roughly a fifth):

| tables over 300–1500 K | upstream stochastic, T | stochastic, √T | stochastic, log T | Chebyshev fit, √T |
|---|---|---|---|---|
| 3 (Δ = 600) | +1.16 % | −0.32 % | −1.77 % | −0.38 % (negative σ) |
| 5 (Δ = 300) | +0.40 % | −0.12 % | −0.63 % | **−0.010 %** |
| 9 (Δ = 150) | +0.127 % | −0.036 % | −0.198 % | — |
| 17 (Δ = 75) | +0.037 % | −0.010 % | −0.057 % | — |

Unresolved range (σ_c,eff, σ₀ = 10 b): upstream T −0.29 % at Δ = 300,
−0.11 % at Δ = 150. S(α,β): +0.28 % in a spectrum-sensitive absorption
fraction at Δ = 100 K, +1.9 % at Δ = 300 K.

## 4. Recommendation (not a decision)

1. **If upstream's scheme (stochastic, T) is kept, the resolved-range error
   sets the grid.** For ≤ 0.1 % on a strongly shielded p it needs Δ ≈ 130 K
   at the cold end (300–600 K); the hot end tolerates ~2× that, since the
   error falls 3.5–6× from the bottom to the top bracket. A grid uniform in
   √T or geometric in T matches that shape better than a uniform one.
2. **√T as the selection variable is worth ~3.5× on p and ~4–13× on the RI**
   at every spacing, at no cost, but it is a departure from upstream and it is
   *worse* in the URR and in the keV valleys pointwise. Measured, not
   recommended outright.
3. **The Chebyshev fit is the most accurate per table by a wide margin**
   (5 tables in √T: 0.01 % on p), but it is deterministic, needs n ≥ 4 to stay
   positive, and loses the finite majorant property; it belongs with WMP
   (#948) and the continuous-in-T majorant (#945), not as a drop-in for A.
4. **S(α,β) should use the deck's own temperatures (≈ 25–50 K apart for H in
   H₂O) and nothing coarser.** Even Δ = 100 K is resolved in transport, and
   the error is not monotone in spacing because the deck changes its spectrum
   with temperature.
5. **A production library at errthn = 1e-3 carries a −0.02 % thinning shift
   in p by itself**; spacing errors below that are not worth buying.

## 5. Limitations

- One resolved nuclide (U-238), one moderator for the slowing-down
  instrument (C-12 at 300 K), isotropic-CM scattering off a target at rest in
  the p instrument (the MC check uses the same kernel). Thermal-range U-238
  effects are not in the p instrument (band 1 eV–10 keV).
- The U-238 test temperatures are bracket **midpoints in T**; the worst point
  for √T and log T lies elsewhere in the bracket and is not mapped.
- S(α,β): exact data only exist at deck temperatures, so the brackets are
  irregular; the transport instrument (Pu-239, JENDL-3.3) is chosen for
  spectral sensitivity, not as a reactor case.
- No k_eff of a real lattice was run; p and the Pu-239 fraction stand in
  for it.
- The port is not on a transport path; #945 stage 2 wires it.

## 6. Hardware and timings

All runs: `CPU only, 3 cores (taskset 0-2), linux, 13th Gen Intel(R)
Core(TM) i9-13900K, 62.5 GiB RAM`, 3 threads, shared machine (other agents
building concurrently).

| stage | wall |
|---|---|
| RECONR U-238 (tol 1e-3) | 9.6–12.5 s |
| BROADR U-238 at 300 K, errthn 1e-4 | 23–30 s |
| 69 temperatures (17 with PURR 20/64/10 000), 3 threads | 1258 s |
| whole `pointwise --urr` stage | 1400 s |
| `pointwise` re-run without PURR (added the 1 eV–1 keV column; every integral reproduced to the printed digit) | 1271 s |
| MC check, 3 × (4 × 10⁶) histories | 362 s |
| LEAPR + bake, H in H₂O, 13 temperatures | 100 s |
| Pu-239 MC, 3 × (4 × 10⁶) histories | 274–297 s |
