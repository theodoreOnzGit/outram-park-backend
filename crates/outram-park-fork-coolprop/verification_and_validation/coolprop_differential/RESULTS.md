# CoolProp differential: results (gh:#1349)

**Generated:** 2026-11-13, machine-run, not human-reviewed.
**Upstream:** CoolProp `0e67fe74b30a2fe9526af9bc64ea026a96f56ebf` (2026-07-05, MIT), the commit this crate
ports. Built from source (CMake Release, static library; `-O3 -DNDEBUG` plus `-ffp-contract=off`; no
`-ffast-math`, no `-march=native`, so no FMA), g++ 13.3.0, glibc 2.39, x86-64. The default CoolProp
configuration is used unless a row says otherwise.
**This crate:** the working tree of this change. Predictions: `PREDICTIONS.md`, committed before the first run.
Provenance and regeneration: `README.md`. Pinned by `tests/coolprop_differential.rs` and
`tests/pt_flash_stable_root.rs`.

## What was compared

137 fluids (all; CoolProp's factory rejects this crate's name `R1224yd(Z)`, whose canonical name is
`R1224YDZ`, so the driver maps it). Per fluid: 181 `(tau, delta)` points (11 `tau` log-spaced between
`Tr/Tmax` and `Tr/Ttriple` times 12 `delta`, plus a 7 x 7 near-critical block), 30 saturation temperatures,
100 `(p, T)` points. Per point, `alphar`, `alpha0` and their five derivatives, `p, u, h, s, cv, cp, w`, `mu`,
`lambda`, the saturation ancillary fits, the full saturation solve, and the `(p,T)` flash with `(p,h)` and
`(p,s)` round trips. 24 797 EOS points, 4 110 saturation temperatures, 13 700 flash points. Metric:
`|a-b| / max(|a|,|b|)`. The inputs are shared bit for bit (`in_delta`, `in_tau`: 24 797 of 24 797
bit-identical).

## Prediction scorecard

| # | predicted | observed | verdict |
|---|---|---|---|
| P1 | `alphar` + derivatives < 1e-12 per fluid and term, >= 60 % bit-identical, at most 3 fluids above 1e-13 | worst 1.2e-10 (`ar`, R21), 8.5e-10 (`ar_tt`, R14); fluids above 1e-13: 18 to 128 per term; bit-identical only 10 to 21 % (`ar` 5 291 of 24 797) | **missed, twice**: tolerance and bit-identity. Cause found (below): cancellation, not a defect |
| P2 | `alpha0` < 1e-13, >= 90 % bit-identical | worst 1.9e-12 (`a0`, MethylPalmitate; `a0_t` 3.1e-14); `a0_d`, `a0_dt` bit-identical on all 24 797; `a0` 53 %, `a0_t` 58 %, `a0_tt` 45 % | **missed** on the bound for `a0` and on bit-identity |
| P3 | no transposed or mistyped coefficient in the generated fluid files | none found. A transposed digit would show as 1e-4 or more; the worst `alphar` term difference is 1e-9 and is fully explained by cancellation | held (see "How the P1 misses were localised") |
| P4 | `(T,rho)` h, s, cv, cp, w < 1e-11; reference-state offsets on 0 to 15 fluids | h 3.4e-11, s 1.2e-11, u 1.2e-11, cv 1.2e-10, w 6.2e-12, p 3.7e-12: the 1e-11 bound is missed by 3 to 10x on h, s, cv, all cancellation at the lowest temperatures (R22, Methanol). **No reference-state offset on any fluid**: h and s agree in absolute value. `cp` is ill-conditioned near the critical point (denominator `1 + 2 delta alphar_d + delta^2 alphar_dd` crosses zero): 6.99e-8 off the exact critical point, up to 8.3e-3 at it | **partly missed** (bound), offsets prediction held (none) |
| P5 | saturation < 1e-8 for `T <= Tc - 1 K`; ancillary fits < 1e-13, >= 80 % bit-identical | ancillary fits: worst 2.0e-15, 3 862 to 3 864 of 3 867 bit-identical, **held**. Saturation solve: see "Saturation" below, **missed at low pressure** | held for the fits, missed for the solve |
| P6 | PT flash: same root on >= 99 %, at least one #917-class failure | 3 571 of 13 603 grid points (26 %) had the wrong root, 3 398 of 3 680 liquid points. After the fix 41 differ (0.3 %). Predicted >= 99 % right: **held only after the fix** | the #917-class prediction held far beyond what I expected |
| P7 | PH, PS round trip bounded by the PT flash, failures near two-phase | `(p,h)` returns no state on 1 165 of 12 535 single-phase states, `(p,s)` on 1 636 of 12 064 (about 10 % and 14 %); where it returns, the worst is 1e-3 | **missed**: not near the dome; a robustness gap of the outer Newton |
| P8 | Rust has a model for 42 / 45 fluids, near-critical lambda may differ at 1e-6; below 1e-10 elsewhere | CoolProp has mu and lambda for 23 fluids this crate returns `None` for; four real defects in transport (below); residual differences remain on Water, Helium, Methane | **missed**: four defects |
| P9 | 0 to 4 groups above 1e-10 | `alphar`: 3 fluids above 1e-10 (all explained); transport: 4 fluids with real defects, 3 with residual differences | **missed** (more, and in transport not in the EOS) |

## Term table (full run, 137 fluids)

`full_run_totals.csv` is the file; the columns are compared values, bit-identical values, worst relative
difference, the fluid that has it, values CoolProp has and this crate does not, and the reverse.

| term | compared | bit-identical | worst rel | worst fluid |
|---|---|---|---|---|
| `alphar` | 24 797 | 5 291 | 1.2e-10 | R21 |
| `alphar_d` | 24 797 | 2 580 | 1.0e-11 | R236FA |
| `alphar_t` | 24 797 | 4 002 | 2.5e-10 | R21 |
| `alphar_dd` | 24 797 | 2 135 | 3.6e-10 | n-Nonane |
| `alphar_dt` | 24 797 | 2 517 | 4.2e-12 | Water |
| `alphar_tt` | 24 797 | 3 377 | 8.5e-10 | R14 |
| `alpha0` | 24 797 | 13 245 | 1.9e-12 | MethylPalmitate |
| `alpha0_d` | 24 797 | 24 797 | 0 | |
| `alpha0_t` | 24 797 | 14 335 | 3.1e-14 | MM |
| `alpha0_dd` | 24 797 | 19 379 | 2.2e-16 | CycloPropane |
| `alpha0_dt` | 24 797 | 24 797 | 0 | |
| `alpha0_tt` | 24 797 | 11 102 | 5.8e-15 | ParaHydrogen |
| `p` | 24 797 | 6 322 | 3.7e-12 | HeavyWater |
| `u` | 24 797 | 9 067 | 1.2e-11 | Water |
| `h` | 24 797 | 8 468 | 3.4e-11 | R22 |
| `s` | 24 797 | 8 423 | 1.2e-11 | Methanol |
| `cv` | 24 797 | 7 639 | 1.2e-10 | R22 |
| `cp` (not the critical point) | 24 660 | 3 143 | 7.0e-8 | R1234ze(Z) |
| `cp` at the exact critical point | 137 | 49 | 8.3e-3 | Tetrahydrofuran |
| `w` | 21 073 | 4 941 | 6.2e-12 | R1123 |
| `w` at the critical point | 137 | 58 | 2.2e-14 | D5 |
| ancillary `p_sat` | 3 867 | 3 862 | 5.0e-16 | R218 |
| ancillary `rho'` | 3 867 | 3 862 | 1.2e-15 | MDM |
| ancillary `rho''` | 3 867 | 3 864 | 2.0e-15 | MethylStearate |

`w` has 21 073 compared values rather than 24 797: CoolProp returns no speed of sound where `w^2 < 0`
(inside the dome). Per fluid and term: `full_run_per_fluid.csv.gz`. Number of fluids whose worst exceeds
1e-12: `alphar` 10, `alphar_d` 8, `alphar_t` 10, `alphar_dd` 82, `alphar_dt` 3, `alphar_tt` 12, `alpha0` 2, `p` 6,
`u` 4, `h` 7, `s` 4, `cv` 2, `w` 3.

## How the P1 misses were localised (no defect)

First divergent operation, by high-precision evaluation (60 digits, mpmath, from the fluid JSON; script
`mp_check.py`, output `mp_localisation_alphar.csv`). For the 20 worst (fluid, point) pairs, the exact
`alphar` and its first and second derivatives were computed and **both** codes compared with it:

| fluid, point | term | this crate vs exact | CoolProp vs exact | sum of abs terms | error / that sum |
|---|---|---|---|---|---|
| R21, `tau` max, `delta` 1e-4 | `ar` | 4.3e-12 | 1.25e-10 | 1.6e3 | 3e-18 / 9e-17 |
| R21, near `tau` 1 | `ar_t` | 9.6e-11 | 1.6e-10 | 2.8e2 | |
| R14 | `ar_tt` | 1.3e-9 | 4.2e-10 | | |
| n-Nonane | `ar_dd` | 1.0e-13 | 3.6e-10 | | |
| Novec649 | `ar_dd` | 1.2e-10 | 1.7e-11 | | |

For `ar` the error divided by the sum of the absolute values of the terms is at most 1.1e-16 (this crate)
and 5.7e-16 (CoolProp) over all 20 points. The result `alphar` is a sum of terms of order 1e2 to 1e3 that
cancels to 1e-3 (R21: the `d = 0` terms of the Power and Exponential groups), so a 1e-16 error in each term
is a 1e-10 relative error in the result. This crate is closer to the exact value than CoolProp in most of
the cases, because CoolProp forms `n * exp(t*ln(tau) + d*ln(delta) + u)` (`Helmholtz.cpp`,
`ResidualHelmholtzGeneralizedExponential::all`) and this crate forms `n * delta^d * tau^t * exp(u)`.
The conclusion is that **the two differ by round-off against a cancelling sum**, not by a coefficient, and
that bit-identity is not reachable without copying CoolProp's operation order; the pins therefore use
bit-identical counts that may only rise and worst-case bounds that may only fall. Water has the
non-analytic terms and was not checked in high precision (4.2e-12 at worst on `ar_dt`).

## Defects found and fixed (every old and new number)

1. **`density_pt` returned the vapour root for liquids (gh:#917 class).** Every `(p,T)` solve started Newton
   from the ideal-gas density. Reference: `tests/pt_flash_stable_root.rs`. Liquid-phase points (CoolProp
   phase 0) with a differing density: **3 398 of 3 680 -> 12**; supercritical-liquid (3): 171 of 1 866 -> 0;
   all points differing at more than 1e-9: **3 597 of 13 603 -> 41**. Water, 350 K, 2 MPa: was vapour,
   now 974.573279182657 against CoolProp 974.573279182652 (5e-15). The branch is now chosen first
   (`pt_branch`), by the Maxwell saturation pressure (CoolProp compares with its superancillary), the
   dense root is solved from the ancillary liquid density (as CoolProp does) and the gas root as before.
   The fluids that have no saturation solve fall back to the lower Gibbs energy of the two roots. An
   ancillary fit alone cannot decide: it is 47 % off for R1234yf at 192 K. Runtime: the full 13 700-point
   flash set went from 163 s to about 8 s (the old path fell into a 400-step bisection for every liquid).
   `(p,h)` and `(p,s)`, which solve through it: points differing: 1 934 -> 17 and 1 944 -> 13; failures
   2 096 -> 1 165 and 2 115 -> 1 636.
2. **Saturation labels swapped near `Tc`.** Cyclopentane, 1 mK below its critical temperature, returned
   `rho' < rho''`. The Newton solve is symmetric in the two roots. Ordered now. `rho'` worst difference
   from CoolProp's default configuration: **1.2e-1 -> 2.4e-10**.
3. **Ethane dilute conductivity** used the EOS reducing `tau = 305.322/T` where CoolProp's
   `conductivity_dilute_hardcoded_ethane` forms its own `tau = 305.33/T`. Ethane `lambda` worst
   **4.8e-5 -> 8.0e-11** (all 180 grid points were off).
4. **Chung viscosity used the EOS critical constants.** CoolProp keeps its own `T_critical`,
   `rhomolar_critical`, `acentric` and `molar_mass` in the `TRANSPORT.viscosity` block; for Cyclopentane they
   are 3 820 mol/m3 and 0.1521 against the EOS's 3 920 and 0.2019. Cyclopentane `mu` worst **2.3e-1 ->
   4.2e-16**, and its `lambda` (through the Olchowy-Sengers term) **7.4e-3 -> 2.0e-11**. Isopentane's block
   equals its EOS and did not move.
5. **Olchowy-Sengers conductivity used the critical pressure, CoolProp uses the reducing-state pressure.**
   Air 3 786 000 against 3 785 020 Pa, Methanol 8.090 against 8.216 MPa, R1234ze(E), n-Pentane. Air `lambda`
   worst **1.6e-5 -> 2.0e-13**; Methanol **2.0e-3 -> 9.7e-10**.
6. **Methane residual conductivity used the saturated-liquid density where CoolProp uses the vapour's**
   (`saturation_ancillary(iDmolar, 1, ...)`). Methane `lambda` mismatching points above 1e-9:
   **22 -> 15**, worst unchanged (5.2e-1, at two near-critical points not yet localised).
7. **Conductivity returned a value silently missing its critical term** for R152A, R1234yf, R1234ze(E) and
   EthylBenzene (the Olchowy-Sengers term divides by a viscosity these fluids do not have here, and the code
   added 0.0). Up to 0.43 to 0.57 relative on the grid. Now `None`. Items 3 to 7 together, `lambda`
   values differing from CoolProp by more than 1e-9: **901 -> 148**, by more than 1e-3: **440 -> 106**,
   values returned `None` where CoolProp has one 2 929 -> 3 760; `mu` differing by more than 1e-9:
   **231 -> 50** (all Water). **A gate moved with it, deliberately:** `tests/transport_vle.rs`
   `coverage_counts` required conductivity for >= 45 fluids and now >= 41, because the four fluids that fell
   out were counted as covered for a value that lacked its critical term (the thing gated was wrong, not the
   gate). Recorded in the test and flagged for the maintainer: whether callers would rather have the
   partial value is a one-line revert in `ConductivityModel::eval`.
8. The gas-branch Newton tolerance had an absolute floor of 1e-9 Pa, which left 1e-6 relative error near
   `p = 1e-3 Pa` (D5, 226 K): the floor is removed. Gas points differing at 1e-9 to 1e-6: **26 -> 0**.

Changed functions: `flash::density_pt` (and new `pt_branch`, `density_pt_dense_branch`, `low_branch_newton`,
`density_pt_low_branch`, `reduced_gibbs`), `vle::saturation_at_temperature`, `transport::ConductivityModel::eval`,
the Ethane branch of the dilute conductivity, `chung_viscosity` (and the new `ChungBlock`),
`CriticalConductivity::eval`, `methane_conductivity`, the `ViscosityModel::Chung` and
`SimplifiedOlchowySengers` variants (new fields; `dev/gen_fluid.py` updated, `cyclopentane.rs`,
`isopentane.rs` and 34 fluid files edited to the generator's output values: not re-run through the
generator, which needs the gitignored upstream clone). No function carried a kovan review stamp
(`review.md`, no `kind = "review"` block, checked 2026-11-13). Every changed function is reached by the
differential test or `tests/pt_flash_stable_root.rs`.

## Saturation

CoolProp's default configuration uses its superancillaries (`SATS` rows); with them off (`SAT` rows) its
own solver is *less* accurate than this crate's near the critical point. Maxwell residuals in 60-digit
arithmetic (`mp_sat.py`, `mp_localisation_saturation.txt`):

| point | who | rel(p_L - p_V) | abs(dg/RT) |
|---|---|---|---|
| R134a 372.401 K | CoolProp default | 4e-16 | 1e-16 |
| R134a 372.401 K | CoolProp, superancillaries off | 1.7e-3 | 6e-4 |
| R134a 372.401 K | this crate | 1.3e-15 | 1.6e-16 |
| R14 227.36 K | CoolProp, superancillaries off | 7.1e-4 | 2.0e-4 |
| R14 227.36 K | this crate | 1.4e-17 | 1.2e-16 |

So **the reference for saturation is the default configuration (`SATS`)**; `SAT` is kept in the data as the
evidence that CoolProp without superancillaries is not a reference near `Tc`.

Against the default, `p_sat` by CoolProp's pressure (values this crate returns, relative difference above
1e-9 counted):

| p_sat | CoolProp values | this crate differs > 1e-9 | worst |
|---|---|---|---|
| < 1e-2 Pa | 22 | 22 | 1.0 |
| 1e-2 to 1 Pa | 38 | 38 | 0.85 |
| 1 to 100 Pa | 80 | 72 | 1.7e-3 |
| 100 to 1e4 Pa | 230 | 24 | 8.2e-8 |
| > 1e4 Pa | 3 738 | **0** | 0 |

A change that took the pressure from the vapour side (well conditioned) moved 72 -> 64 in the 1 to 100 Pa
row and nothing else, so it was reverted: the densities, not the output pressure, are the limit. Below
about 1e-5 Pa not even CoolProp is self-consistent (`SAT` and `SATS` differ by 3x for MD3M at 192 K), so
those points are unresolved in double precision and are reported, not counted as a defect.
Missing: this crate returns `None` on 487 of 4 110 `p_sat` values: 6 fluids without a saturation ancillary
(Air, R404A, R407C, R410A, R507A: 150 rows; SES36: 28; CoolProp has them) and 338 within about 1 K of `Tc`.

## `(p,T)` flash, after the fix

| CoolProp phase | points | differ > 1e-9 |
|---|---|---|
| liquid | 3 680 | 12 (Propylene glycol at 2 Pa, where `p_sat` is unresolved) |
| gas | 3 546 | 29, all > 1e-3 (the pseudo-pure Air, R407C, R410A, SES36; 1-Butene and the methyl esters at `p` < 1e-3 Pa); 26 more points at 1e-9 to 1e-6 before the tolerance fix |
| supercritical liquid | 1 866 | 0 |
| supercritical | 1 859 | 0 |
| supercritical gas | 2 576 | 0 |
| CoolProp refuses (`T` below the melting line, `p` above `p_max`) | 97 | this crate returns a state |

## Transport, after the fixes

`mu`: 7 704 compared, 6 624 bit-identical, 50 differ above 1e-9, **all Water** (33 above 1e-3; up to
5.8e-1 near the critical point). `lambda`: 7 181 compared, 5 476 bit-identical; 148 differ above 1e-9: Water
72 (grid up to 2.6e-1), Helium 56 (grid up to 2.0e-2, near critical up to 1.0), Methane 15 (two near-critical
points at 5.2e-1 and 3.7e-1), CO2/m-Xylene/p-Xylene 5 at 1.5e-9 to 7e-9. Not localised: the hardcoded
critical-enhancement terms of Water, Helium and Methane.
**Gap inventory:** CoolProp has `mu` and `lambda` for 23 fluids this crate returns `None` for: EthylBenzene,
Propylene, R11, R1132a, R116, R12, R1234yf, R1234ze(E), R124, R13, R14, R141b, R142b, R143a, R152A, R218, R22,
R227EA, R236EA, R236FA, R245fa, R32, RC318 (3 852 `mu` values and 3 760 `lambda` values).

## What remains (not done in this change)

Rows 6 to 8 of the issue (incompressibles, humid air, mixture `alphar`), melting lines and surface tension
(not ported), the `(p,h)` / `(p,s)` robustness gap, Water/Helium/Methane transport, pseudo-pure
saturation, low-pressure saturation accuracy: see gh:#1350.
