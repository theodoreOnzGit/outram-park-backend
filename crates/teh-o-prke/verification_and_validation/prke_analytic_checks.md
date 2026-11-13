# Point kinetics, ANS decay-heat machinery and separable feedback: analytic checks

> **Research, education and V&V only. AI draft, unreviewed.** Not for
> facility operation, licensing or safety decisions.

**Generated:** 2026-11-01 (UTC)
**Crate version / commit:** teh-o-prke 0.1.7, on top of workspace commit `c9a3afcaf`
**Issues:** gh:#1080 (ANS-5.1 data sets), gh:#1082 (N-group kinetics, `PointReactor`),
gh:#1083 (separable feedback)
**Source equations:** RELAP-7 Theory Manual (INL/EXT-14-31366) §7.1 Eqs 412-416,
§7.2 Eqs 417-420, §7.4 Eq 429, §7.5 Eq 443. The manual is held privately and is
cited by equation number only.

All numbers below were produced by `cargo test -p teh-o-prke --release --lib`
on 2026-11-01. Every check is a unit test whose doc comment carries the same
methodology, prediction and result.

## 1. Six groups bit-identical after the N-group refactor (gh:#1082)

**Methodology.** Before any code changed, a scripted transient (12 steps with
varying `dt`, reactivity from -0.05 to super-prompt +0.008, `Lambda` from 1e-6
to 5e-4 s, external sources), repeated three times, was run through
`SixGroupPRKE`'s implicit and explicit solvers for U-233, U-235 and Pu-239
data. Every returned value was folded into an FNV-1a hash of its `f64` bits;
the hashes and final densities were committed as a pin (commit `a6b65c491`).
The six-group solver was then made to delegate to the N-group core.

**Pass criterion.** Zero bits differ.

**Result.** All six fingerprints unchanged (test
`zero_power_prke::six_group_bit_identity_pin`). `PointKinetics::from_nuclide`
also matches `SixGroupPRKE` state-for-state (`assert_eq!` on the vectors).
The explicit script diverges (final densities ~1e88) because it includes steps
far past the explicit stability limit; it is pinned anyway, as arithmetic.

## 2. In-hour equation (gh:#1082)

**Methodology.** Start at critical equilibrium, apply a step reactivity,
integrate by backward Euler at `dt = 1 ms`, and measure the asymptotic growth
rate `omega = d ln n / dt` over a 100 s window. Reference: the largest root of
`rho = omega Lambda + sum_i beta_i omega / (omega + lambda_i)`, by bisection
(and, for one group, by the quadratic formula). The window starts at
`t_1 = max(400 s, 15 / gap)`, where `gap` is the distance to the second root,
so the next mode has decayed by at least `e^-15`.

**Prediction (before running).** Backward Euler's discrete rate is about
`omega (1 + omega dt / 2)`, so relative error ~ `omega dt / 2`. Gate: 1e-3.

| case | `omega` reference (1/s) | period (s) | `omega` PRKE (1/s) | relative error | predicted |
|---|---|---|---|---|---|
| ~~6 groups U-235, `rho = +0.001`~~ | ~~1.81870702e-2~~ | ~~54.98~~ | ~~1.81872357e-2~~ | ~~9.1e-6~~ | ~~9.1e-6~~ |
| ~~6 groups U-235, `rho = +0.003`~~ | ~~1.40406076e-1~~ | ~~7.122~~ | ~~1.40415934e-1~~ | ~~7.0e-5~~ | ~~7.0e-5~~ |
| ~~6 groups U-235, `rho = -0.001`~~ | ~~-7.69269389e-3~~ | ~~-130.0~~ | ~~-7.69266472e-3~~ | ~~-3.8e-6~~ | ~~4e-6~~ |
| **6 groups U-235 ENDF/B-VIII.0, `rho = +0.001`** (2026-11-10) | 2.22678814e-2 | 44.91 | 2.22681294e-2 | 1.1e-5 | same order |
| **6 groups U-235 ENDF/B-VIII.0, `rho = +0.003`** (2026-11-10) | 1.72959426e-1 | 5.782 | 1.72974385e-1 | 8.6e-5 | same order |
| **6 groups U-235 ENDF/B-VIII.0, `rho = -0.001`** (2026-11-10) | -8.56679171e-3 | -116.7 | -8.56675560e-3 | -4.2e-6 | same order |
| 1 group (`beta = 0.0065`, `lambda = 0.08`), `rho = 0.002` | 3.5464557789e-2 (closed form) | 28.20 | 3.5465186672e-2 | 1.8e-5 | 1.8e-5 |

`Lambda = 1e-4 s` throughout. The bisection root equals the closed-form
one-group root to better than 1e-12.

**Re-measured 2026-11-10 (gh:#1319).** The crate's U-235 default changed
from the textbook set (`beta = 0.0065`, rounded Keepin half-lives, source
unidentified) to ENDF/B-VIII.0 (`beta = 0.0065230`, MAT 9228 MF=1/MF=5
MT=455). The struck rows are the textbook-set results; the bold rows were
measured with `cargo test --release -p teh-o-prke --lib
inhour_equation_six_group -- --nocapture`. The solver's error is unchanged
in order, as predicted on gh:#1319. The roots moved with the data: `omega`
rose 22-23 % for the positive steps, more than the +15 to +18 % predicted
from the 15 % shorter mean precursor life (at `omega ~ 0.02 /s` the
small-`omega` limit does not hold; group 1's `lambda` is 0.013 /s).

**A miss in the first protocol, recorded.** The first run used a fixed
400-500 s window for every case, on the claim that all other roots had decayed
by `e^-12`. That holds for the positive steps, not for `rho = -0.001`, whose
second root is only 8.2e-3 /s below the first; the measured error was
**6.7e-3**, failing the 1e-3 gate. The gate was kept and the window rule above
replaced the fixed window; the result then matched the prediction.

## 3. Nordheim-Fuchs prompt excursion (gh:#1082)

**Methodology.** `PointReactor` with zero delayed groups (prompt-only, the
Nordheim-Fuchs assumption), `Lambda = 1e-4 s`, step `rho_0 = 0.005` from
`P_0 = 1 W`, adiabatic fuel `C = 1e5 J/K`, linear feedback `alpha = -1e-5 /K`
(`b = |alpha|/C = 1e-10 /J`). Power from neutron density through Eqs 414-416.
Backward Euler at `dt = 1e-5 s` to 1.5 s, feedback lagged one step. Closed
forms from `dP/dE = (rho_0 - b E)/Lambda`:
`P_max = P_0 + rho_0^2 / (2 b Lambda)`,
`E_tot = (rho_0 + sqrt(rho_0^2 + 2 b Lambda P_0)) / b`.

**Prediction.** Errors of order `dt rho_0 / Lambda = 5e-4`; gate 1e-2.

| quantity | closed form | PRKE | relative error |
|---|---|---|---|
| peak power | 1.250000e9 W | 1.250208e9 W | 1.7e-4 |
| energy released | 1.000000e8 J | 1.000083e8 J | 8.3e-5 |
| time of peak | (not a closed-form check) | 0.4466 s | |

For comparison only (not gated, delayed neutrons are outside the closed
form): six U-235 groups at `rho_ext = beta + rho_0` peak ~~2.8 % higher
(1.285e9 W) and have released 31 % more energy by 1.5 s~~ **3.3 % higher
(1.291e9 W, at 0.4261 s) and have released 34.6 % more energy by 1.5 s
(re-measured 2026-11-10 with the ENDF/B-VIII.0 U-235 default, gh:#1319)**,
the delayed source continuing after the prompt burst.

## 4. Density -> fission rate -> power, and `P_T` (gh:#1082)

- Eqs 414-416 by hand (`n = 1e12 /m³`, `v = 2200 m/s`, `V = 2 m³`,
  `Sigma_f = 5 /m`, `Q_f = 200 MeV`): `phi = 2.2e15`, `psi = 2.2e16 /s`,
  `P_f = 7.04958e5 W`; agreement 1e-14 relative.
- Eq 429 at equilibrium, 10 MW immediate, U-235 decay heat saturated:
  decay/immediate = 0.065915 = 13.183/200 (to 1e-5); with `G = 1.01` the total
  matches the hand sum and holds over a 100 s critical transient to 1e-9.
- The actinide term is the `ActinideDecayModel::None` slot (zero); the
  actinide equations are gh:#1081.

## 5. ANS decay-heat machinery (gh:#1080)

- `AnsDecayHeat` with the built-in 1978 draft set, U-235 only, `F_gamma = 1`,
  reproduces the existing `DecayHeat` **bit for bit** over 1000 s at 10 MW and
  1000 s of shutdown (both use one shared group update).
- `F_gamma` and fission fractions by hand: saturated mix 0.6 U-235 / 0.25
  Pu-239 / 0.15 U-238, `F_gamma = 1.2`, `psi = 1e17 /s`: hand 2.51466e5 W from
  the 3-decimal recorded totals, agreement within 1e-4; ratio to `F_gamma = 1`
  is 1.2 to 1e-14.
- `G(t)` (Eq 419) by hand, `t = 100 s`, `T = 3e7 s`, `psi_g = 2`: 1.006447.

**Not done: each standard against its own published tables.** gh:#1080 asks
for the 1979, 1994 and 2005 sets to be checked against their published tables
and against the 1978 fit. The coefficient tables are in the copyrighted
ANSI/ANS-5.1 standards; no open reproduction was found in the workspace on
2026-11-01, and `DATA_POLICY.md` forbids using them otherwise. The machinery
accepts them as user-supplied data; the comparison waits for a licensed copy.

## 6. Separable feedback, Eq 443 (gh:#1083)

Each term against a hand evaluation (all to 1e-17 absolute):

| term | inputs | hand value | model |
|---|---|---|---|
| scram curves | (0,0),(0.5 s,-0.01),(2 s,-0.05) from trip at 10 s, plus constant -0.001 | -0.001, -0.006, -0.031, -0.051 at 5, 10.25, 11.25, 20 s | same |
| `W R_rho(rho)` | table (600,-0.02),(700,-0.005),(750,0); weights 0.6/0.4 at 650 and 760 kg/m³ | -0.0075 | same |
| `a_M dT_M` | -2e-5 /K x 10 K, -3e-5 /K x -5 K | -5e-5 | same |
| `W R_F(T_F)` | table of the crate's sqrt Doppler law at 300/600/900/1200 K | nodes exact; 750 K chord -0.0099269 vs law -0.0100656 | same |

**Linear-only model vs the existing coefficient feedback.** (a) Against
`NordheimFuchsExactTimestepper`'s `alpha_f (T_f - T_ref)`: bit-identical at
300, 451.7 and 1234.5 K. (b) Against
`obtain_fuel_temperature_feedback_coeff_thermal_spectrum(a, T_ref) x dT`:
bit-identical at 451.7 and 1234.5 K; at `dT = 0` the prediction of
bit-identity **missed by the sign of zero** (`-0.0` by hand, `+0.0` from the
model's sum); values equal. (c) Against GeN-Foam's volume-weighted linear
Doppler form, ported by hand: equal within 1e-17.

## Interpretation

These checks verify the implementation against the equations it implements:
time integration (in-hour, Nordheim-Fuchs), unit conversion, the shared
decay-heat group update, and each separable term. They validate nothing
against measurement. The delayed-neutron data, the 1978 decay-heat fit and the
feedback tables remain whatever the caller supplies.
