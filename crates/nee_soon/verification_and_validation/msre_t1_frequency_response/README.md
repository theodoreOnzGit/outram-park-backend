# MSRE T-1: natural period of oscillation against power, U-235 (gh:#1132)

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI draft,
> unreviewed. Validation against a measurement.

## Methodology

**Measured reference:** ORNL-TM-1647 (Kerlin, Ball and Steffy, 1966),
Fig. 29: the natural periods of oscillation of the uncontrolled reactor
from 76 kW to 7.3 MW, seven points. They were digitised from the scan
(PDF p. 46, 300 dpi; both log axes calibrated on the gridlines; each value is
the centroid of the dot's dense core, good to about ±2 %). The method is in
`nee_soon::msre_frequency_response::MEASURED_PERIODS_TM1647`.

**Model:** `nee_soon::msre_frequency_response`, a linear model of the whole
heat-transport chain, solved at each frequency as one complex system. It is
built from ORNL-TM-1070 (Ball and Kerlin, 1965):

- six-group circulating-fuel point kinetics (as T-C1, linearised);
- a single-region core: two fuel lumps and a graphite lump (A.1)-(A.3);
- a delayed-gamma lag;
- heat-exchanger and radiator lumps (A.32)-(A.37);
- plug-flow piping delays.

All coefficients come from TM-1070 App. B and Table 1, with page numbers in
the module doc. The radiator air side scales with power as ORNL scaled it.
**No parameter was adjusted.** The natural period is 2π/ω at the peak of
|δP/δρ|.

**Verification of the implementation** (unit tests, all passing):

- with zero coefficients the response equals the closed-form circulating
  transfer function (TM-1070 A.30), to 1e-10, at five frequencies;
- one heat-exchange section reproduces TM-1070's closed form (A.38), to
  1e-10, at four frequencies.

**T-1's result uses the fuel and graphite coefficients from this workspace's
Monte Carlo** (N-3, gh:#1129), not TM-1070's. **Not yet measured:** those
runs are in progress.

## Results

### Baseline: ORNL's own coefficients in this model (2026-11-09)

TM-1070 Table 1 coefficients: fuel −4.84, graphite −3.7 × 10⁻⁵ /°F. This is
not T-1's result. It shows what the model structure gives with ORNL's
inputs.

| P [MW] | measured [min] | model [min] | model / measured |
|---|---|---|---|
| 0.076 | 29.91 | 24.64 | 0.824 |
| 0.467 | 11.01 | 10.08 | 0.915 |
| 1.004 | 7.16 | 6.77 | 0.946 |
| 2.448 | 4.33 | 4.00 | 0.925 |
| 4.848 | 2.41 | 2.38 | 0.989 |
| 6.735 | 1.85 | 1.82 | 0.985 |
| 7.306 | 1.71 | 1.71 | 1.002 |

The model runs short at every power except the highest, by 1–8 % above
0.4 MW and by 18 % at 76 kW. The low-power end is where the long thermal
time constants dominate.

### T-1 under the RVS default (2026-11-10, gh:#1129, #1311)

N-3's split was re-measured after the resonance-scattering default moved
from DBRC to RVS: **fuel −4.48 ± 0.54, graphite −3.33 ± 0.40 × 10⁻⁵ /°F**
(six seeds per run, Mo omitted; record in the outram-mc-libs V&V README).
Both lie inside the band posted before the run, so by the pre-registered
rule the DBRC-based table below stands. It was re-run anyway, because it
is cheap:
`--fuel-per-f -4.48e-5 --graphite-per-f -3.33e-5`.

| P [MW] | measured [min] | model [min] | model / measured |
|---|---|---|---|
| 0.076 | 29.91 | 25.72 | 0.860 |
| 0.467 | 11.01 | 10.52 | 0.955 |
| 1.004 | 7.16 | 7.08 | 0.989 |
| 2.448 | 4.33 | 4.22 | 0.974 |
| 4.848 | 2.41 | 2.55 | 1.056 |
| 6.735 | 1.85 | 1.94 | 1.047 |
| 7.306 | 1.71 | 1.82 | 1.063 |

**−14 % to +6 %**, against −12 % to +19 % with the DBRC coefficients. The
high-power residual shrinks because the fuel coefficient grew. The two
coefficient sets differ by about 1σ each, so the improvement is not a
resolved physics result; it shows again that the coefficients' statistics
set this comparison. ~~**Quote this table under the current default.**~~ **Superseded 2026-11-11 by the arm-C table above.**

### T-1 on the default N-1 model, arm C with natural Mo (2026-11-11, gh:#1129)

The split above was measured on arm A (Mo omitted). On the default model,
arm C, it is **fuel −4.44 ± 0.40, graphite −2.82 ± 0.43 × 10⁻⁵ /°F** (six
seeds per run, RVS; record in the outram-mc-libs V&V README). Run with
`--fuel-per-f -4.44e-5 --graphite-per-f -2.82e-5`.

| P [MW] | measured [min] | model [min] | model / measured |
|---|---|---|---|
| 0.076 | 29.91 | 26.58 | 0.889 |
| 0.467 | 11.01 | 10.79 | 0.980 |
| 1.004 | 7.16 | 7.23 | 1.010 |
| 2.448 | 4.33 | 4.30 | 0.992 |
| 4.848 | 2.41 | 2.59 | 1.073 |
| 6.735 | 1.85 | 1.96 | 1.061 |
| 7.306 | 1.71 | 1.84 | 1.076 |

**−11 % to +8 %**, against −14 % to +6 % with arm A's split. The graphite
coefficient is 0.5 smaller in magnitude (about 1σ), which lengthens the
low-power periods and shortens nothing; the change is inside the
coefficients' statistics. **~~Quote the arm-A RVS table~~ Quote this table
under the current default model** (arm C, RVS).

### A finding on the way: the circulation loss

With TM-1070 Table 1's transits (core 8.46 s, external loop 16.73 s), the
model's steady circulation loss is **0.2467 % δk/k**, against the measured
0.212 % (+16 %). N-4 (gh:#1130) has the same lumped physics at 0.2301 %
with TM-1626's transits. The loss sets only the kinetics' operating point
here.

### T-1 proper: N-3's Monte Carlo coefficients (2026-11-09, final)

**Coefficients:** fuel **−3.82 ± 0.44** and graphite **−3.77 ± 0.65** ×
10⁻⁵ /°F. They are route 4 on `outram_mc_libs::vv::msre_n1`, arm A (Mo
omitted), with σ from the seed spread: 6 seeds at the 1131 °F baseline and
3 per split run. Record in
`crates/outram-mc-libs/verification_and_validation/msre_n1_first_criticality/`.
Run with
`cargo run --release -p nee_soon --example msre_t1_natural_periods -- --fuel-per-f -3.82e-5 --graphite-per-f -3.77e-5`.

| P [MW] | measured [min] | model [min] | model / measured |
|---|---|---|---|
| 0.076 | 29.91 | 26.23 | 0.877 |
| 0.467 | 11.01 | 10.88 | 0.989 |
| 1.004 | 7.16 | 7.41 | 1.035 |
| 2.448 | 4.33 | 4.53 | 1.045 |
| 4.848 | 2.41 | 2.83 | 1.175 |
| 6.735 | 1.85 | 2.17 | 1.173 |
| 7.306 | 1.71 | 2.03 | 1.188 |

**Within −12 % to +19 % at every power, inside the ±25 % posted before
the coefficients existed** (gh:#1132). ~~An earlier pass with seeds 1–3
only (fuel −4.10, graphite −4.05) gave −15 % to +12 %~~: superseded by the
6-seed baseline; it shows how much the comparison moves with the
coefficients.

**The coefficients' statistics set this comparison's uncertainty, not the
loop model.** ORNL's own coefficients (−4.84 / −3.7) in the same model give
1.002 at 7.3 MW, while these give 1.188. The residual's shape (short at the
lowest power, long at the highest) is the trend a fuel coefficient larger
in magnitude than the graphite one would correct. It is not resolved at
this precision and is not claimed.
