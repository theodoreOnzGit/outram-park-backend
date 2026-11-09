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

### A finding on the way: the circulation loss

With TM-1070 Table 1's transits (core 8.46 s, external loop 16.73 s), the
model's steady circulation loss is **0.2467 % δk/k**, against the measured
0.212 % (+16 %). N-4 (gh:#1130) has the same lumped physics at 0.2301 %
with TM-1626's transits. The loss sets only the kinetics' operating point
here.

### T-1 proper: N-3's Monte Carlo coefficients (2026-11-09)

**Coefficients:** fuel **−4.10 ± 0.77** and graphite **−4.05 ± 0.91** ×
10⁻⁵ /°F. They are route 4 on `outram_mc_libs::vv::msre_n1`, arm A (Mo
omitted), seeds 1–3, with σ from the seed spread; record in
`crates/outram-mc-libs/verification_and_validation/msre_n1_first_criticality/`.
Run with
`cargo run --release -p nee_soon --example msre_t1_natural_periods -- --fuel-per-f -4.10e-5 --graphite-per-f -4.05e-5`.

| P [MW] | measured [min] | model [min] | model / measured |
|---|---|---|---|
| 0.076 | 29.91 | 25.34 | 0.847 |
| 0.467 | 11.01 | 10.51 | 0.955 |
| 1.004 | 7.16 | 7.15 | 0.998 |
| 2.448 | 4.33 | 4.34 | 1.002 |
| 4.848 | 2.41 | 2.68 | 1.112 |
| 6.735 | 1.85 | 2.05 | 1.108 |
| 7.306 | 1.71 | 1.92 | 1.123 |

**Within −15 % to +12 % at every power, inside the ±25 % posted before
the coefficients existed** (gh:#1132).

**Coefficient sensitivity:** with both coefficients moved by one σ
together:

- at −3.33 / −3.14 the periods are 28.3, 8.0 and 2.28 min at 0.076, 1.0
  and 7.3 MW;
- at −4.87 / −4.96 they are 23.2, 6.5 and 1.68 min.

The ±1σ band brackets the measurement at most powers. **The Monte Carlo
coefficients' statistics, not the loop model, set the uncertainty of this
comparison.**

The shape of the residual is short at the lowest power and long at the
highest. That is the trend expected if the real fuel coefficient were
larger in magnitude than the graphite one at high power (where the fuel
term acts faster). It is not resolved at this precision and is not
claimed.
