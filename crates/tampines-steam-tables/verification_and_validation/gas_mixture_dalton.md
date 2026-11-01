# Steam + non-condensable gas mixture (gh:#1093)

> Research, education and V&V only. AI draft, unreviewed.

Part of the MELCOR-style control-volume epic (gh:#1091). Code:
`src/gas_mixture/` (`GasMixture`, `GasMixtureModel`, `NonCondensable`,
`melcor_ncg`, `nasa7`). Tests: `src/gas_mixture/tests.rs` and
`melcor_ncg`'s test, which moved with the code.

## Method

The mixture follows Dalton's law: one volume, one temperature, and each
component at its own partial pressure.

- **Non-condensables** (N2, H2, O2, CO, CO2, Ar) are ideal gases.
  - N2 and H2 use MELCOR's NCG `c_v(T)` fits, moved **verbatim** from
    `sembawang::bwr_mark1::gas`, which now forwards here. This is the default
    for those two.
  - All six have NASA 7-coefficient polynomials from GRI-Mech 3.0.
  - Internal energy is measured from 0 K, as in `sembawang`.
- **Steam** is IAPWS-IF97 region 2 at its partial pressure, found by Newton
  on `ρ_s = 1/v(T, p_s)`. This is the default.
- **Ablations:**
  - `without_real_steam` uses `sembawang`'s ideal steam,
    `u = 2.506e6 + 1500 (T − 373.15)`.
  - `without_ncg_cv_t` uses constant heat capacities.
- **`temperature(U)`:** Newton with a bisection safeguard. With IF97 steam it
  searches only above the temperature at which the steam's density is
  saturated vapour; below that the steam would condense, which is the control
  volume's job.

## Data provenance

| Data | Source | Licence / access | Accessed |
|---|---|---|---|
| NASA-7 `c_p/R` for N2, H2, O2, CO, CO2, Ar | GRI-Mech 3.0 (G. P. Smith, D. M. Golden, M. Frenklach et al., <http://combustion.berkeley.edu/gri-mech/>), via Cantera v3.0.0 `data/gri30.yaml` | GRI-Mech: open; Cantera: BSD-3-Clause | 2026-11-01, `https://raw.githubusercontent.com/Cantera/cantera/v3.0.0/data/gri30.yaml` |
| N2, H2 MELCOR NCG fits | MELCOR 2.2 Vol. 2 (SAND2023-10994 O), NCG/H2O-RM-33/34 | private manual: cited only, coefficients as already in `sembawang` | — |
| Molar masses of O2, CO, CO2, Ar | IUPAC standard atomic weights | public | — |
| Reference `c_v(298.15)` in the test | JANAF/NIST ideal-gas tables (`c_p` 29.12, 29.38, 29.14, 37.12, 20.79 J/(mol K) less R) | public | from memory, not re-checked against the tables |

## Predictions (written before the first run, 2026-11-01)

1. NASA-7 `c_v(298.15)` is within 0.5 % of the reference values. `u`
   integrates `c_v` to 1e-6 and is continuous at `T_mid`.
2. Partial pressures sum to the total, and `p_i V = n_i R T` holds exactly.
3. IF97 steam at low partial pressure is within 0.5 % of ideal, and its
   partial pressure reproduces `ρ_s` to 1e-12.
4. The dew point equals `T_sat(p_s)`, and saturated steam reads RH 1 to 1e-9.
5. `temperature(U)` inverts `state` to 1e-9 K.
6. With both ablations, the energy is `sembawang`'s formula exactly.

## Results (2026-11-01, release)

| # | Measured | Prediction |
|---|---|---|
| 1 | N2 −0.24 %, O2 +0.30 %, CO +0.03 %, CO2 +0.04 %, Ar +0.01 %; `u` and continuity asserted | held |
| 2 | sum to 1e-15; N2 `p V = n R T` bit-exact | held |
| 3 | `p_s` 16 100.7 Pa against ideal 16 153.5 Pa (−0.33 %) at 350 K, 0.1 kg/m³; `ρ_s` back to 1e-12 | held |
| 4 | dew 328.5952 K = `T_sat(p_s)`; RH 1.000000000000 | held |
| 5 | 420 K back to ≤ 1e-12 K, for all three model settings | held |
| 6 | `sembawang`'s formula to 1e-15 | held |

**Instrument corrected during the work (case 5).** The first `temperature(U)`
searched from 273.16 K. With 2 kg of steam in 10 m³ the steam is 40×
supersaturated there, and the region-2 partial-pressure Newton failed. The
search now starts at the steam's saturation temperature, for the physical
reason given above. No tolerance was changed.

## Limits

- The non-condensables are ideal.
- Steam–non-condensable interactions are not modelled (no enhancement
  factor).
- Supersaturated steam is reported, not condensed.
- The two components' energy references differ (see the module docs).
