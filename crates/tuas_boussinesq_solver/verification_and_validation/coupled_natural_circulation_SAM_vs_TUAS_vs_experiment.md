> **AI-GENERATED DOCUMENT — REQUIRES HUMAN REVIEW BEFORE USE.**
>
> This file was generated programmatically by a Rust test
> (`sam_vs_tuas_vs_experiment_summary.rs`) written with the assistance
> of an AI assistant (Anthropic Claude). Per the project `AI_USAGE.md`,
> AI-generated outputs are untrusted draft material: they are NOT
> accepted automatically and MUST undergo independent human review
> before being used, cited, or published. Scientific interpretation,
> verification, validation, and final acceptance remain under human
> control. Verify every number against the cited sources and the test
> outputs before relying on this document.

# CIET coupled natural circulation: SAM vs TUAS (K=17.8) vs experiment

This document compares three sources of steady-state natural-circulation
mass flow rates for the 25 coupled CIET tests (datasets A/B/C): the CIET
**experimental** measurements, the **TUAS** thermal-hydraulics library
(this crate) with the SAM-matched pipe-38 form loss **K = 17.8**
(`new_pipe_38_sam_model`), and the **SAM** code predictions.

## Methodology

- **Benchmark / references.** CIET coupled natural-circulation steady
states (25 cases, sets A/B/C at TCHX outlet 46 / 35 / 40 degC).
Experimental values and SAM predictions are from NED-2021 Table 4:
Zou, L., Hu, G., O'Grady, D., & Hu, R. (2021), *Code validation of SAM
using natural-circulation experimental data from the compact integral
effects test (CIET) facility*, Nuclear Engineering and Design, 377,
111144 (accepted manuscript openly available, OSTI ID 1774637; also
ANL/NSE-19/11, Zou, Hu & Charpentier 2019).
- **TUAS inputs.** The coupled `dataset_a/b/c` regression tests, run
with pipe 38 at the SAM-matched form loss K = 17.8. The primary loop
already uses the SAM K values (pipe 3 K=17.15, pipe 22 K=45.95).
- **Quantities.** DRACS-loop and heater-DHX (primary) subloop mass flow
rate at steady state, in kg/s.
- **Error metric.** Percentage error against the CIET experiment,
`(model - experiment) / experiment * 100`, for both TUAS and SAM.
- **Data version.** TUAS values are the K=17.8 regression outputs
measured 2026-07-15 (`cargo test --release`, 25/25 coupled tests pass).

## DRACS loop mass flow rate (kg/s)

| Case | Heater (W) | TCHX (degC) | Experiment | TUAS K=17.8 | TUAS %err | SAM | SAM %err |
|---|---|---|---|---|---|---|---|
| A1 | 1479.86 | 46 | 0.03341 | 0.03414 | +2.19 | 0.03420 | +2.37 |
| A2 | 1653.90 | 46 | 0.03544 | 0.03617 | +2.06 | 0.03624 | +2.25 |
| A3 | 2014.51 | 46 | 0.03877 | 0.03991 | +2.93 | 0.03999 | +3.14 |
| A4 | 2178.49 | 46 | 0.04011 | 0.04144 | +3.31 | 0.04152 | +3.52 |
| A5 | 2395.90 | 46 | 0.04277 | 0.04334 | +1.34 | 0.04345 | +1.59 |
| A6 | 2491.87 | 46 | 0.04465 | 0.04414 | -1.13 | 0.04424 | -0.91 |
| A7 | 2696.24 | 46 | 0.04710 | 0.04577 | -2.81 | 0.04589 | -2.57 |
| B1 | 655.16 | 35 | 0.02329 | 0.02175 | -6.62 | 0.02177 | -6.53 |
| B2 | 1054.32 | 35 | 0.02952 | 0.02870 | -2.78 | 0.02847 | -3.54 |
| B3 | 1394.70 | 35 | 0.03324 | 0.03325 | +0.04 | 0.03291 | -0.98 |
| B4 | 1685.62 | 35 | 0.03611 | 0.03656 | +1.24 | 0.03611 | +0.01 |
| B5 | 1987.75 | 35 | 0.03841 | 0.03959 | +3.08 | 0.03911 | +1.82 |
| B6 | 2282.01 | 35 | 0.04063 | 0.04225 | +3.99 | 0.04174 | +2.73 |
| B7 | 2546.60 | 35 | 0.04270 | 0.04444 | +4.09 | 0.04390 | +2.82 |
| B8 | 2874.03 | 35 | 0.04456 | 0.04695 | +5.36 | 0.04637 | +4.06 |
| B9 | 3031.16 | 35 | 0.04636 | 0.04808 | +3.71 | 0.04749 | +2.44 |
| C1 | 841.02 | 40 | 0.02686 | 0.02503 | -6.80 | 0.02505 | -6.76 |
| C2 | 1158.69 | 40 | 0.03055 | 0.03012 | -1.40 | 0.03005 | -1.63 |
| C3 | 1409.22 | 40 | 0.03345 | 0.03343 | -0.05 | 0.03331 | -0.43 |
| C4 | 1736.11 | 40 | 0.03649 | 0.03716 | +1.83 | 0.03701 | +1.42 |
| C5 | 2026.29 | 40 | 0.03869 | 0.04006 | +3.54 | 0.03989 | +3.11 |
| C6 | 2288.83 | 40 | 0.04115 | 0.04243 | +3.12 | 0.04223 | +2.63 |
| C7 | 2508.71 | 40 | 0.04312 | 0.04428 | +2.68 | 0.04409 | +2.26 |
| C8 | 2685.83 | 40 | 0.04509 | 0.04568 | +1.31 | 0.04548 | +0.87 |
| C9 | 2764.53 | 40 | 0.04699 | 0.04628 | -1.51 | 0.04606 | -1.97 |

## Primary (heater-DHX) loop mass flow rate (kg/s)

| Case | Heater (W) | TCHX (degC) | Experiment | TUAS K=17.8 | TUAS %err | SAM | SAM %err |
|---|---|---|---|---|---|---|---|
| A1 | 1479.86 | 46 | 0.02738 | 0.02768 | +1.08 | 0.02776 | +1.38 |
| A2 | 1653.90 | 46 | 0.02819 | 0.02913 | +3.34 | 0.02917 | +3.47 |
| A3 | 2014.51 | 46 | 0.03236 | 0.03182 | -1.68 | 0.03180 | -1.74 |
| A4 | 2178.49 | 46 | 0.03255 | 0.03292 | +1.13 | 0.03287 | +0.99 |
| A5 | 2395.90 | 46 | 0.03390 | 0.03428 | +1.13 | 0.03422 | +0.94 |
| A6 | 2491.87 | 46 | 0.03355 | 0.03485 | +3.88 | 0.03478 | +3.65 |
| A7 | 2696.24 | 46 | 0.03462 | 0.03601 | +4.02 | 0.03592 | +3.76 |
| B1 | 655.16 | 35 | 0.01731 | 0.01811 | +4.62 | 0.01846 | +6.65 |
| B2 | 1054.32 | 35 | 0.02198 | 0.02321 | +5.60 | 0.02339 | +6.44 |
| B3 | 1394.70 | 35 | 0.02570 | 0.02666 | +3.74 | 0.02674 | +4.04 |
| B4 | 1685.62 | 35 | 0.02846 | 0.02918 | +2.52 | 0.02674 | -6.05 |
| B5 | 1987.75 | 35 | 0.03118 | 0.03148 | +0.96 | 0.03141 | +0.73 |
| B6 | 2282.01 | 35 | 0.03374 | 0.03348 | -0.78 | 0.03336 | -1.12 |
| B7 | 2546.60 | 35 | 0.03577 | 0.03511 | -1.86 | 0.03496 | -2.26 |
| B8 | 2874.03 | 35 | 0.03796 | 0.03694 | -2.68 | 0.03677 | -3.12 |
| B9 | 3031.16 | 35 | 0.03849 | 0.03776 | -1.90 | 0.03759 | -2.35 |
| C1 | 841.02 | 40 | 0.02003 | 0.02084 | +4.06 | 0.02112 | +5.44 |
| C2 | 1158.69 | 40 | 0.02367 | 0.02449 | +3.48 | 0.02465 | +4.15 |
| C3 | 1409.22 | 40 | 0.02635 | 0.02693 | +2.20 | 0.02700 | +2.46 |
| C4 | 1736.11 | 40 | 0.02949 | 0.02969 | +0.67 | 0.02969 | +0.67 |
| C5 | 2026.29 | 40 | 0.03190 | 0.03183 | -0.21 | 0.03178 | -0.37 |
| C6 | 2288.83 | 40 | 0.03412 | 0.03358 | -1.59 | 0.03349 | -1.86 |
| C7 | 2508.71 | 40 | 0.03562 | 0.03492 | -1.95 | 0.03481 | -2.27 |
| C8 | 2685.83 | 40 | 0.03593 | 0.03594 | +0.03 | 0.03581 | -0.32 |
| C9 | 2764.53 | 40 | 0.03547 | 0.03637 | +2.55 | 0.03624 | +2.17 |

## Maximum absolute error vs experiment

| Loop | TUAS K=17.8 max \|error\| | at case | SAM max \|error\| | at case |
|---|---|---|---|---|
| DRACS | 6.80% | C1 | 6.76% | C1 |
| Primary | 5.60% | B2 | 6.65% | B1 |

## Summary

Across all 25 coupled cases, TUAS with the SAM-matched pipe-38 form loss
K = 17.8 has a maximum DRACS-loop error of 6.80% (case C1) and a maximum
primary-loop error of 5.60% (case B2). The SAM code, on the same
experimental dataset, has a maximum DRACS error of 6.76% (case C1) and a
maximum primary error of 6.65% (case B1).

On the **primary loop**, TUAS's worst-case agreement is better than SAM's.
On the **DRACS loop**, TUAS's worst case is essentially tied with SAM's
(both codes have their largest DRACS error on the same low-power case,
C1). Both codes systematically under-predict the DRACS flow at the two
lowest-power operating points (B1, C1) and over-predict it at higher
flow; a single uniform form loss cannot correct both ends, which is why
C1/B1 sit at the edge of (or just past) the SAM-defined error band. This
is a documented limitation, not a tuned result: see the coupled-loop
module docs and the follow-up on a velocity/Reynolds-dependent pipe-38
form loss.

*Generated by `sam_vs_tuas_vs_experiment_summary.rs` in the
`tuas_boussinesq_solver` crate. Numbers are the real experiment / TUAS /
SAM values from the sources cited above; see the module documentation
for provenance. This document is AI-generated and requires human review
(see the disclaimer at the top).*
