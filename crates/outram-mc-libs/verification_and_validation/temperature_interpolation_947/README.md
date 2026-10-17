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

## 3. Results

_Not yet run._

## 4. Hardware

Every timing below is from: _to be filled from the run_.
